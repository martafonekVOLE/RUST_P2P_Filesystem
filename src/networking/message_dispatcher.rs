use crate::constants::K;
use crate::networking::messages::{Request, Response, MAX_MESSAGE_SIZE};
use anyhow::{bail, Context, Result};
use log::info;
use serde_json;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

pub struct MessageDispatcher {
    socket: Mutex<UdpSocket>,
}

impl MessageDispatcher {
    /// Creates a new MessageDispatcher with a single UDP socket.
    pub async fn new() -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .with_context(|| "Failed to bind UDP socket")?;
        Ok(MessageDispatcher {
            socket: Mutex::new(socket),
        })
    }

    /// Sends a request using the single UDP socket.
    /// Checks the serialized size against MAX_MESSAGE_SIZE before sending.
    pub async fn send_request(&self, request: Request) -> Result<usize> {
        if request.receiver == request.sender {
            bail!("Sender and receiver must not be the same (possible message to self)");
        }

        let serialized_request =
            serde_json::to_vec(&request).with_context(|| "Failed to serialize request type")?;

        if serialized_request.len() > MAX_MESSAGE_SIZE {
            bail!(
                "Serialized request size {} exceeds maximum allowed size {}",
                serialized_request.len(),
                MAX_MESSAGE_SIZE
            );
        }

        let socket = self.socket.lock().await;
        let len = socket
            .send_to(&serialized_request, request.receiver.address)
            .await
            .with_context(|| format!("Failed to send request to {}", request.receiver.address))?;

        info!("Sent request: {}", request);
        Ok(len)
    }

    /// Sends a response using the single UDP socket.
    /// Checks the serialized size against MAX_MESSAGE_SIZE before sending.
    pub async fn send_response(&self, response: Response) -> Result<usize> {
        let serialized_response =
            serde_json::to_vec(&response).with_context(|| "Failed to serialize response type")?;

        if serialized_response.len() > MAX_MESSAGE_SIZE {
            bail!(
                "Serialized response size {} exceeds maximum allowed size {}",
                serialized_response.len(),
                MAX_MESSAGE_SIZE
            );
        }

        let socket = self.socket.lock().await;
        let len = socket
            .send_to(&serialized_response, response.receiver.address)
            .await
            .with_context(|| format!("Failed to send response to {}", response.receiver.address))?;

        info!("Sent response: {}", response);
        Ok(len)
    }
}
