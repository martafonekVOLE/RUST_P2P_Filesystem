use crate::networking::node_info::NodeInfo;
use crate::routing::kademlia_messages::{KademliaMessage, KademliaMessageType};
use std::fmt::format;
use std::net::SocketAddr;
use std::sync::mpsc::Sender;
use tokio::net::UdpSocket;

pub struct MessageSender {
    socket: UdpSocket,
}

impl MessageSender {
    pub fn new(address: &str) -> Self {
        let socket = UdpSocket::bind(address).expect("failed");

        MessageSender { socket }
    }

    async fn send(
        &self,
        message_type: KademliaMessageType,
        receiver: &NodeInfo,
    ) -> Result<usize, Err()> {
        let response = self
            .socket
            .send_to(&message_type.to_bytes(), receiver.get_address_unwrapped())
            .await;

        response
    }

    pub async fn send_ping(&self, receiver: &NodeInfo) {
        self.send(KademliaMessageType::Ping, receiver)
            .await
            .expect(&format!(
                "Sending PING message to {} failed.",
                receiver.get_address_unwrapped()
            ));
    }

    pub async fn send_pong(&self, receiver: &NodeInfo) {
        self.send(KademliaMessageType::Pong, receiver)
            .await
            .expect(&format!(
                "Sending PONG message to {} failed.",
                receiver.get_address_unwrapped()
            ));
    }
}
