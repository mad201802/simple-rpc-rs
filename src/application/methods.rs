use std::{future::Future, net::SocketAddr, time::Duration};

use log::trace;

use crate::{
    application::util::generate_random_request_id,
    packets::{RpcMessageType, RpcPacket, RpcReturnCode},
    types::{MethodInvokeCallback, MethodResponseCallback},
};

use super::RpcApplication;

pub trait RpcMethods {
    fn offer_method(
        &mut self,
        method_id: u8,
        callback: MethodInvokeCallback,
    ) -> impl Future<Output = ()> + Send;
    fn call_method(
        &mut self,
        ip_address: SocketAddr,
        method_id: u8,
        payload: Vec<u8>,
        callback: MethodResponseCallback,
    ) -> impl Future<Output = ()> + Send;
}

impl RpcMethods for RpcApplication {
    async fn offer_method(&mut self, method_id: u8, callback: MethodInvokeCallback) {
        self.state.offered_methods.insert(method_id, callback);
        trace!("Method {} offered", method_id);
    }

    async fn call_method(
        &mut self,
        ip_address: SocketAddr,
        method_id: u8,
        payload: Vec<u8>,
        callback: MethodResponseCallback,
    ) {
        trace!("Calling method {} on {:?}", method_id, ip_address);
        let request_id = generate_random_request_id().await;

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

        // Why the hell do we need to sleep here?
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
}
