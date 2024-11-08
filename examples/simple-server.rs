use std::sync::Arc;

use simplerpcrs::{application::RpcApplication, packets::RpcErrorMessage};

const RPC_ID: u16 = 0x01;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // If RUST_LOG is not set to a specific level, set the default log level to INFO
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "trace");
    }

    env_logger::init();

    let address = "127.0.0.1:6969".parse()?;
    let mut app = RpcApplication::new(RPC_ID, address);

    app.offer_method(
        0x01,
        Arc::new(|payload| {
            log::info!("Received data: {:?}", payload);
            if payload.is_empty() {
                log::error!("Payload is empty");
                return Err(RpcErrorMessage {
                    error_code: 0x01,
                    error_message: "Payload is empty".to_string(),
                });
            }

            Ok(vec![])
        }),
    )
    .await;

    app.run(true).await;

    Ok(())
}
