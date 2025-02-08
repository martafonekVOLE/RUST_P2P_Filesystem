use crate::constants::TCP_TIMEOUT_MILLISECONDS;
use anyhow::{bail, Context};
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
        let tcp_listener = TcpListener::bind(LOCALHOST)
            .await
            .with_context(|| format!("Error opening TCP Listener at {}", LOCALHOST))?;

        let port = tcp_listener
            .local_addr()
            .context("Error reading the local socket address")?
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
        // Wait for a connection with a timeout.
        let (mut stream, _addr) = timeout(
            Duration::from_secs(TCP_TIMEOUT_MILLISECONDS),
            self.listener.accept(),
        )
        .await
        .with_context(|| {
            format!(
                "Timed out after {} seconds while waiting for a TCP connection",
                TCP_TIMEOUT_MILLISECONDS
            )
        })??;

        info!("Established TCP connection: {:?}", stream);

        // Read data from the stream.
        let mut data = Vec::new();
        stream
            .read_to_end(&mut data)
            .await
            .with_context(|| "An error occurred while reading data from the TCP stream")?;

        info!("Successfully received data over TCP.");
        Ok(data)
    }
}
