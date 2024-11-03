use std::{net::SocketAddr, sync::Arc, time::Duration};

use log::info;
use simplerpcrs::application::{events::RpcEvents, methods::RpcMethods, RpcApplication};

const RPC_ID: u16 = 0x02;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // If RUST_LOG is not set to a specific level, set the default log level to INFO
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "trace");
    }

    env_logger::init();

    let address: SocketAddr = "127.0.0.1:6970".parse()?;
    let server_address: SocketAddr = "127.0.0.1:6969".parse()?;

    let mut app = RpcApplication::new(RPC_ID, address);
    app.run(false).await;

    app.wait_for_availability(server_address, 5, Duration::from_secs(5))
        .await;

    info!("Calling method 0x01");
    app.call_method(
        server_address,
        0x01,
        vec![],
        Arc::new(|data| {
            match data {
                Ok(data) => {
                    log::info!("Received data: {:?}", data);
                }
                Err(err) => {
                    log::error!("Error: {:?}", err);
                }
            }

            Ok(vec![])
        }),
    )
    .await;

    info!("Subscribing to event 0x02");
    app.subscribe_event(
        server_address,
        0x02,
        Arc::new(|data| {
            log::info!("Received event data: {:?}", data);
        }),
    )
    .await;

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
