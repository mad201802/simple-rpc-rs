use std::sync::Arc;

use crate::packets::{RpcErrorMessage, RpcPacket};

/// The maximum number of messages that can be stored in the broadcast channel.
pub const CHANNEL_CAPACITY: usize = 64;

/// Tuple type containing the raw message data and the address of the sender.
pub type RawMessageData = (RpcPacket, std::net::SocketAddr);

/// Callback type for handling method invocations.
pub type MethodInvokeCallback =
    Arc<dyn Fn(Vec<u8>) -> Result<Vec<u8>, RpcErrorMessage> + Send + Sync>;

/// Callback type for handling method invocations.
pub type MethodResponseCallback =
    Arc<dyn Fn(Result<Vec<u8>, RpcErrorMessage>) -> Result<Vec<u8>, RpcErrorMessage> + Send + Sync>;

/// Callback type for handling events.
pub type OnEventInvokeCallback = Arc<dyn Fn(Vec<u8>) + Send + Sync>;
