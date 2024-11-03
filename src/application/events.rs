use std::{collections::HashSet, future::Future, net::SocketAddr, time::Duration};

use log::trace;

use crate::{
    application::util::generate_random_request_id,
    packets::{RpcMessageType, RpcPacket, RpcReturnCode},
    types::OnEventInvokeCallback,
};

use super::RpcApplication;

pub trait RpcEvents {
    fn offer_event(&mut self, event_id: u8) -> impl Future<Output = ()> + Send;
    fn call_event(&self, event_id: u8, payload: Vec<u8>) -> impl Future<Output = ()> + Send;

    fn subscribe_event(
        &mut self,
        ip_address: SocketAddr,
        event_id: u8,
        callback: OnEventInvokeCallback,
    ) -> impl Future<Output = ()> + Send;

    fn unsubscribe_event(
        &mut self,
        ip_address: SocketAddr,
        event_id: u8,
    ) -> impl Future<Output = ()> + Send;
}

impl RpcEvents for RpcApplication {
    async fn offer_event(&mut self, event_id: u8) {
        self.state
            .offered_events
            .lock()
            .await
            .insert(event_id, HashSet::new());
        trace!("Event {} offered", event_id);
    }

    async fn call_event(&self, event_id: u8, payload: Vec<u8>) {
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

    async fn subscribe_event(
        &mut self,
        ip_address: SocketAddr,
        event_id: u8,
        callback: OnEventInvokeCallback,
    ) {
        trace!("Subscribing to event {} on {:?}", event_id, ip_address);
        let request_id = generate_random_request_id().await;

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

        // Why the hell do we need to sleep here?
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    async fn unsubscribe_event(&mut self, ip_address: SocketAddr, event_id: u8) {
        trace!("Unsubscribing from event {} on {:?}", event_id, ip_address);
        let request_id = generate_random_request_id().await;

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
}
