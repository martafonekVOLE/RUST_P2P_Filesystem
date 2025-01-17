use crate::networking::messages::{Request, Response};
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
        let serialized_request =
            serde_json::to_vec(&request).expect("Failed to serialize request type");
        let socket = self.socket.lock().await;
        socket
            .send_to(&serialized_request, request.receiver.address)
            .await
    }

    ///
    /// Sends a response using the single UDP socket.
    /// Uses lock to abstract away the thread safety of the socket.
    ///
    pub async fn send_response(&self, response: Response) -> Result<usize, Error> {
        let serialized_response =
            serde_json::to_vec(&response).expect("Failed to serialize response type");
        let socket = self.socket.lock().await;
        socket
            .send_to(&serialized_response, response.receiver.address)
            .await
    }
}
