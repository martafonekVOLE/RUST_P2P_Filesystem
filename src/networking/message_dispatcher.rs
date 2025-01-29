use crate::networking::messages::{Request, Response};
use log::info;
use serde_json;
use std::io::Error;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

pub struct MessageDispatcher {
    socket: Mutex<UdpSocket>,
}

impl MessageDispatcher {
    ///
    /// Creates a new MessageDispatcher with a single UDP socket.
    /// For future iterations, this could be expanded to support multiple sockets in a pool.
    ///
    pub async fn new() -> Self {
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .expect("Failed to bind UDP socket");

        MessageDispatcher {
            socket: Mutex::new(socket),
        }
    }

    ///
    /// Sends a request the single UDP socket.
    /// Uses lock to abstract away the thread safety of the socket.
    ///
    pub async fn send_request(&self, request: Request) -> Result<usize, Error> {
        if request.receiver == request.sender {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "Sender and receiver must not be the same (possible message to self)",
            ));
        }

        let serialized_request =
            serde_json::to_vec(&request).expect("Failed to serialize request type");
        let socket = self.socket.lock().await;
        let len = socket
            .send_to(&serialized_request, request.receiver.address)
            .await?;

        info!("Sent request: {}", request);
        Ok(len)
    }

    ///
    /// Sends a response using the single UDP socket.
    /// Uses lock to abstract away the thread safety of the socket.
    ///
    pub async fn send_response(&self, response: Response) -> Result<usize, Error> {
        let serialized_response =
            serde_json::to_vec(&response).expect("Failed to serialize response type");
        let socket = self.socket.lock().await;
        let len = socket
            .send_to(&serialized_response, response.receiver.address)
            .await?;

        info!("Sent response: {}", response);
        Ok(len)
    }
}
