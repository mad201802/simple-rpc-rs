use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RpcMessageType {
    Request = 0x00,
    Notification = 0x02,
    Unsubscribe = 0x03,
    Response = 0x80,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RpcReturnCode {
    Ok = 0x00,
    Error = 0x01,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcPacket {
    pub service_id: u8,               // Defines a service
    pub method_id: u8,                // Defines a method/event
    pub request_id: u8,               // Defines a request
    pub message_type: RpcMessageType, // Defines the type of message
    pub return_code: RpcReturnCode,   // Defines the return code
    pub payload: Vec<u8>,             // Contains the actual data
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcErrorMessage {
    pub error_code: u8,
    pub error_message: String,
}

impl RpcPacket {
    pub fn new(
        service_id: u8,
        method_id: u8,
        request_id: u8,
        message_type: RpcMessageType,
        return_code: RpcReturnCode,
        payload: Vec<u8>,
    ) -> Self {
        RpcPacket {
            service_id,
            method_id,
            request_id,
            message_type,
            return_code,
            payload,
        }
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<RpcPacket, Vec<u8>> {
        match rmp_serde::from_slice::<RpcPacket>(&bytes) {
            Ok(packet) => Ok(packet),
            Err(err) => {
                println!("Error deserializing packet: {:?}", err);
                Err("Error deserializing packet".as_bytes().to_vec())
            }
        }
    }

    pub fn to_bytes(packet: &RpcPacket) -> Result<Vec<u8>, Vec<u8>> {
        match rmp_serde::to_vec(packet) {
            Ok(bytes) => Ok(bytes),
            Err(err) => {
                println!("Error serializing packet: {:?}", err);
                Err("Error serializing packet".as_bytes().to_vec())
            }
        }
    }
}
