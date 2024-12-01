use crate::routing::kademlia_messages::KademliaMessage;
use std::net::SocketAddr;
use tokio::net::UdpSocket;

pub struct MessageSender {
    socket: UdpSocket,
}

impl MessageSender {
    pub async fn new(address: &str) -> Self {
        let socket = UdpSocket::bind(address).await.expect("failed");

        MessageSender { socket }
    }

    pub async fn send_ping(&self, message: KademliaMessage, target: SocketAddr) {
        let msg_type = message.get_type();
        self.socket
            .send_to(&*msg_type.to_bytes(), target)
            .await
            .expect("Failed to send Ping");
    }
}
