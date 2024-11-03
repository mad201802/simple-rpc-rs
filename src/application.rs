use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use log::{debug, error, info, trace, warn};
use rand::Rng;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::Mutex,
};

use crate::{
    packets::{RpcErrorMessage, RpcMessageType, RpcReturnCode},
    types::{
        MethodInvokeCallback, MethodResponseCallback, OnEventInvokeCallback, RawMessageData,
        CHANNEL_CAPACITY,
    },
};

use super::packets::RpcPacket;

type SocketAddrSet = Arc<Mutex<HashSet<std::net::SocketAddr>>>;
type MethodMap = HashMap<u8, MethodInvokeCallback>;
type EventSubscribers = Arc<Mutex<HashMap<u8, HashSet<std::net::SocketAddr>>>>;
type EventCallbacks = Arc<Mutex<HashMap<u8, OnEventInvokeCallback>>>;
type RequestCallbacks = Arc<Mutex<HashMap<u8, MethodResponseCallback>>>;

#[derive(Clone)]
pub struct RpcApplication {
    config: RpcConfig,
    state: RpcState,
}

#[derive(Clone)]
pub struct RpcConfig {
    service_id: u16,
    address: std::net::SocketAddr,
}

#[derive(Clone)]
pub struct RpcState {
    connected_sockets: SocketAddrSet,
    message_process_tx: tokio::sync::broadcast::Sender<RawMessageData>,
    client_response_tx: tokio::sync::broadcast::Sender<RawMessageData>,
    offered_methods: MethodMap,
    offered_events: EventSubscribers,
    subscribed_events: EventCallbacks,
    open_requests: RequestCallbacks,
}

impl RpcApplication {
    pub fn new(service_id: u16, address: std::net::SocketAddr) -> Self {
        RpcApplication {
            config: RpcConfig {
                service_id,
                address,
            },
            state: RpcState {
                connected_sockets: Arc::new(Mutex::new(HashSet::new())),
                message_process_tx: tokio::sync::broadcast::channel::<RawMessageData>(
                    CHANNEL_CAPACITY,
                )
                .0,
                client_response_tx: tokio::sync::broadcast::channel::<RawMessageData>(
                    CHANNEL_CAPACITY,
                )
                .0,
                offered_methods: HashMap::new(),
                offered_events: Arc::new(Mutex::new(HashMap::new())),
                subscribed_events: Arc::new(Mutex::new(HashMap::new())),
                open_requests: Arc::new(Mutex::new(HashMap::new())),
            },
        }
    }

    async fn handle_message_data(&self) {
        let mut message_process_rx = self.state.message_process_tx.subscribe();

        while let Ok((packet, addr)) = message_process_rx.recv().await {
            trace!("Handling packet: {:?}", packet);

            // Creating a response packet with default values
            let mut response_packet = RpcPacket {
                service_id: packet.service_id,
                method_id: packet.method_id,
                request_id: packet.request_id,
                message_type: RpcMessageType::Response,
                return_code: RpcReturnCode::Ok,
                payload: vec![],
            };

            match packet.message_type {
                // If the message is a request, we will check if the method/event is offered and call it
                RpcMessageType::Request => {
                    trace!("Received Request Message: {:?}", packet);

                    if let Some(callback) = self.state.offered_methods.get(&packet.method_id) {
                        let result = callback(packet.payload);
                        response_packet = match result {
                            Ok(response_data) => RpcPacket {
                                payload: response_data,
                                ..response_packet
                            },
                            Err(err) => RpcPacket {
                                return_code: RpcReturnCode::Error,
                                payload: rmp_serde::to_vec(&err).unwrap(),
                                ..response_packet
                            },
                        };
                    }

                    // Handle Event Subscriptions
                    let mut offered_events = self.state.offered_events.lock().await;
                    if let Some(connected_clients) = offered_events.get_mut(&packet.method_id) {
                        debug!("New event subscription: {:?}", packet.method_id);

                        // Add the client to the event
                        connected_clients.insert(addr);
                    }

                    self.state
                        .client_response_tx
                        .send((response_packet, addr))
                        .unwrap();
                }

                // If the message is a response, we will check if the request callback is open and call the callback
                RpcMessageType::Response => {
                    trace!("Received Response Message: {:?}", packet);
                    let mut open_requests = self.state.open_requests.lock().await;
                    if let Some(callback) = open_requests.remove(&packet.request_id) {
                        if packet.return_code == RpcReturnCode::Ok {
                            let _result = callback(Ok(packet.payload));
                        } else {
                            let error_message =
                                rmp_serde::from_slice::<RpcErrorMessage>(&packet.payload).unwrap();
                            let _result = callback(Err(error_message));
                        }
                    }
                }

                RpcMessageType::Notification => {
                    trace!("Received Notification Message: {:?}", packet);
                    let subscribed_events = self.state.subscribed_events.lock().await;
                    if let Some(callback) = subscribed_events.get(&packet.method_id) {
                        callback(packet.payload);
                    }
                }

                RpcMessageType::Unsubscribe => {
                    trace!("Received Unsubscribe Message: {:?}", packet);
                    let mut offered_events = self.state.offered_events.lock().await;
                    if let Some(connected_clients) = offered_events.get_mut(&packet.method_id) {
                        connected_clients.remove(&addr);
                    }
                }
            }
        }
    }

    pub async fn offer_method(&mut self, method_id: u8, callback: MethodInvokeCallback) {
        self.state.offered_methods.insert(method_id, callback);
        trace!("Method {} offered", method_id);
    }

    pub async fn offer_event(&mut self, event_id: u8) {
        self.state
            .offered_events
            .lock()
            .await
            .insert(event_id, HashSet::new());
        trace!("Event {} offered", event_id);
    }

    pub async fn call_event(&self, event_id: u8, payload: Vec<u8>) {
        trace!("Calling event {}", event_id);
        // Send a notification to all clients that have subscribed to the event
        let offered_events = self.state.offered_events.lock().await;
        if let Some(connected_clients) = offered_events.get(&event_id) {
            for client in connected_clients {
                let notification_packet = RpcPacket {
                    service_id: self.config.service_id as u8,
                    method_id: event_id,
                    request_id: 0,
                    message_type: RpcMessageType::Notification,
                    return_code: RpcReturnCode::Ok,
                    payload: payload.clone(),
                };

                self.state
                    .client_response_tx
                    .send((notification_packet, *client))
                    .unwrap();
            }
        }
    }

    async fn _connect_client(&self, ip_address: std::net::SocketAddr) {
        trace!("Connecting to client: {:?}", ip_address);
        let mut connected_sockets = self.state.connected_sockets.lock().await;
        if !connected_sockets.contains(&ip_address) {
            let socket = tokio::net::TcpStream::connect(ip_address).await.unwrap();
            connected_sockets.insert(ip_address);

            let server = Arc::new(self.clone());
            let client_response_rx = server.state.client_response_tx.subscribe();
            tokio::spawn(async move {
                server.handle_client(socket, client_response_rx).await;
            });

            trace!("Connected client handler registered")
        }
    }

    pub async fn call_method(
        &mut self,
        ip_address: std::net::SocketAddr,
        method_id: u8,
        payload: Vec<u8>,
        callback: MethodResponseCallback,
    ) {
        trace!("Calling method {} on {:?}", method_id, ip_address);
        let request_id = self.generate_random_request_id().await;

        let mut open_requests = self.state.open_requests.lock().await;
        open_requests.insert(request_id, callback);

        let request_packet = RpcPacket {
            service_id: self.config.service_id as u8,
            method_id,
            request_id,
            message_type: RpcMessageType::Request,
            return_code: RpcReturnCode::Ok,
            payload,
        };

        self.state
            .client_response_tx
            .send((request_packet, ip_address))
            .unwrap();
    }

    pub async fn subscribe_event(
        &mut self,
        ip_address: std::net::SocketAddr,
        event_id: u8,
        callback: OnEventInvokeCallback,
    ) {
        trace!("Subscribing to event {} on {:?}", event_id, ip_address);
        let request_id = self.generate_random_request_id().await;

        let request_packet = RpcPacket {
            service_id: self.config.service_id as u8,
            method_id: event_id,
            request_id,
            message_type: RpcMessageType::Request,
            return_code: RpcReturnCode::Ok,
            payload: vec![],
        };

        self.state
            .client_response_tx
            .send((request_packet, ip_address))
            .unwrap();

        let mut subscribed_events = self.state.subscribed_events.lock().await;
        subscribed_events.insert(event_id, callback);
    }

    pub async fn unsubscribe_event(&mut self, ip_address: std::net::SocketAddr, event_id: u8) {
        trace!("Unsubscribing from event {} on {:?}", event_id, ip_address);
        let request_id = self.generate_random_request_id().await;

        let request_packet = RpcPacket {
            service_id: self.config.service_id as u8,
            method_id: event_id,
            request_id,
            message_type: RpcMessageType::Unsubscribe,
            return_code: RpcReturnCode::Ok,
            payload: vec![],
        };

        self.state
            .client_response_tx
            .send((request_packet, ip_address))
            .unwrap();

        let mut subscribed_events = self.state.subscribed_events.lock().await;
        subscribed_events.remove(&event_id);
    }

    async fn generate_random_request_id(&self) -> u8 {
        let mut rng = rand::thread_rng();
        rng.gen::<u8>()
    }

    pub async fn wait_for_availability(
        &self,
        ip_address: std::net::SocketAddr,
        max_retry: u32,
        timeout: Duration,
    ) {
        info!("Waiting for {:?} to be available ...", ip_address);

        let mut connected_sockets = self.state.connected_sockets.lock().await;
        if !connected_sockets.contains(&ip_address) {
            let mut attempts = 0;
            let socket = loop {
                match tokio::net::TcpStream::connect(ip_address).await {
                    Ok(socket) => break socket,
                    Err(e) => {
                        attempts += 1;
                        if attempts >= max_retry {
                            error!(
                                "Failed to connect to {} after {} attempts: {}",
                                ip_address, max_retry, e
                            );
                            return;
                        }
                        warn!(
                            "Retrying to connect to {} (attempt {}/{})",
                            ip_address, attempts, max_retry
                        );
                        tokio::time::sleep(timeout).await;
                    }
                }
            };

            connected_sockets.insert(ip_address);

            let server = Arc::new(self.clone());
            let client_response_rx = server.state.client_response_tx.subscribe();
            tokio::spawn(async move {
                server.handle_client(socket, client_response_rx).await;
            });
        }
    }

    async fn handle_client(
        &self,
        mut socket: tokio::net::TcpStream,
        mut client_response_rx: tokio::sync::broadcast::Receiver<(RpcPacket, std::net::SocketAddr)>,
    ) {
        trace!("Starting client handler: {:?}", socket.peer_addr().unwrap());
        let ip_addr = socket.peer_addr().unwrap();

        {
            let mut connected_sockets = self.state.connected_sockets.lock().await;
            connected_sockets.insert(ip_addr);
        }

        let (reader, mut writer) = socket.split();
        let mut reader = BufReader::new(reader);
        let mut buffer: Vec<u8> = Vec::new();

        loop {
            tokio::select! {
                result = reader.read_buf(&mut buffer) => {
                    if result.unwrap() == 0 {
                        info!("[Disconnected] {:?}", ip_addr);
                        {
                            let mut connected_sockets = self.state.connected_sockets.lock().await;
                            connected_sockets.remove(&ip_addr);

                            let mut offered_events = self.state.offered_events.lock().await;
                            for (_, connected_clients) in offered_events.iter_mut() {
                                connected_clients.remove(&ip_addr);
                            }
                        }
                        break;
                    }

                    match RpcPacket::from_bytes(buffer.clone()) {
                        Ok(packet) => {
                            self.state.message_process_tx.send((packet, ip_addr)).unwrap();
                        },
                        Err(err) => {
                            error!("Error deserializing packet: {:?}", err);
                        }
                    }

                    buffer.clear();
                },
                result = client_response_rx.recv() => {
                    let (msg, other_addr) = result.unwrap();

                    if ip_addr == other_addr {
                        trace!("Sending message to {:?}:{:?}", ip_addr, msg);

                        match RpcPacket::to_bytes(&msg) {
                            Ok(packet) => {
                                writer.write_all(&packet).await.unwrap();
                            },
                            Err(err) => {
                                error!("Error serializing packet: {:?}", err);
                            }
                        }
                    } else {
                        debug!("Ignoring message for {:?} as it is not the intended recipient (current {:?})", other_addr, ip_addr);
                    }
                }
            }
        }
    }

    pub async fn run(&self, blocking: bool) {
        let listener = TcpListener::bind(self.config.address).await.unwrap();

        info!(
            "RPC Application started on {:?}:{:?}",
            self.config.address.ip(),
            self.config.address.port()
        );

        let server = Arc::new(self.clone());
        tokio::spawn(async move {
            server.handle_message_data().await;
        });

        if blocking {
            loop {
                let (socket, _addr) = listener.accept().await.unwrap();
                info!("[Connected] {:?}", _addr);

                let client_response_rx = self.state.client_response_tx.subscribe();

                let server = Arc::new(self.clone());
                tokio::spawn(async move {
                    server.handle_client(socket, client_response_rx).await;
                });
            }
        } else {
            let server = Arc::new(self.clone());
            tokio::spawn(async move {
                loop {
                    let (socket, _addr) = listener.accept().await.unwrap();
                    info!("[Connected] {:?}", _addr);

                    let server = Arc::new(server.clone());
                    let client_response_rx = server.state.client_response_tx.subscribe();
                    tokio::spawn(async move {
                        server.handle_client(socket, client_response_rx).await;
                    });
                }
            });
        }
    }
}
