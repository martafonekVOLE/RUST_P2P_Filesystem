use crate::networking::node_info::NodeInfo;
use crate::routing::kademlia_messages::KademliaMessageType;
use std::io::Error;
use tokio::net::UdpSocket;

pub struct MessageSender {
    socket: UdpSocket,
}

impl MessageSender {
    pub async fn new(address: &str) -> Self {
        let socket = UdpSocket::bind(address)
            .await
            .expect("Building MessageSender failed!");

        MessageSender { socket }
    }

    async fn send(
        &self,
        message_type: KademliaMessageType,
        receiver: &NodeInfo,
    ) -> Result<usize, Error> {
        

        self
            .socket
            .send_to(&message_type.to_bytes(), receiver.get_address())
            .await
    }

    pub async fn send_ping(&self, receiver: &NodeInfo) {
        self.send(KademliaMessageType::Ping, receiver)
            .await
            .unwrap_or_else(|_| panic!("Sending PING message to {} failed.",
                receiver.get_address()));
    }

    pub async fn send_pong(&self, receiver: &NodeInfo) {
        self.send(KademliaMessageType::Pong, receiver)
            .await
            .unwrap_or_else(|_| panic!("Sending PONG message to {} failed.",
                receiver.get_address()));
    }
}
