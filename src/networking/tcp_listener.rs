use crate::constants::TCP_TIMEOUT_MILLISECONDS;
use anyhow::{Context, Error};
use log::info;
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::time::timeout;

pub const LOCALHOST: &str = "127.0.0.1:0";

///
/// A struct for managing a TCP listener for the purpose of receiving data.
/// Encapsulates a single TCP listener that is used for receiving data.
///
pub struct TcpListenerService {
    listener: TcpListener,
    port: u16,
}

impl TcpListenerService {
    ///
    /// Default constructor.
    ///
    pub async fn new() -> Result<TcpListenerService, Error> {
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

    ///
    /// Port getter.
    ///
    pub fn get_port(&self) -> u16 {
        self.port
    }

    ///
    /// Receives data over TCP.
    ///
    /// Returns a vector of bytes containing the data received or an error if the data receive operation
    /// fails within the `TCP_TIMEOUT_MILLISECONDS` timeout.
    ///
    pub async fn receive_data(&self) -> Result<Vec<u8>, Error> {
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
