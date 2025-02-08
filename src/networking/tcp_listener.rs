use anyhow::bail;
use log::{error, info};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::time::timeout;

const LOCALHOST: &str = "127.0.0.1:0";

pub struct TcpListenerService {
    listener: TcpListener,
    port: u16,
}

impl TcpListenerService {
    pub async fn new() -> Result<TcpListenerService, anyhow::Error> {
        let tcp_listener = match TcpListener::bind(LOCALHOST).await {
            Ok(listener) => listener,
            Err(e) => {
                bail!("Error opening TCP Listener.");
            }
        };

        let port = match tcp_listener.local_addr() {
            Ok(addr) => addr,
            Err(e) => {
                bail!("Error reading Socket Address.");
            }
        }
        .port();

        Ok(TcpListenerService {
            listener: tcp_listener,
            port,
        })
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub fn get_tcp_listener_ref(&self) -> &TcpListener {
        &self.listener
    }

    pub async fn receive_data(&self) -> Result<Vec<u8>, anyhow::Error> {
        match timeout(Duration::from_secs(10), self.listener.accept()).await {
            Ok(Ok((mut stream, _addr))) => {
                info!("Established TCP connection.");

                let mut data = Vec::new();
                let received_bytes = stream.read_to_end(&mut data).await;

                match received_bytes {
                    Ok(_) => {
                        info!("Successfully received the data over TCP.");
                        Ok(data)
                    }
                    Err(_) => {
                        bail!("An error has occurred while reading the TCP Stream.");
                    }
                }
            }
            Ok(Err(_err)) => {
                bail!("Failure while accepting data.");
            }
            Err(_) => {
                bail!("Timed out while waiting for connection.");
            }
        }
    }
}
