// request_map.rs
use crate::networking::messages::RequestId;
use crate::networking::messages::Response;
use log::{error, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};

#[derive(Clone)]
pub struct RequestMap {
    map: Arc<RwLock<HashMap<RequestId, oneshot::Sender<Response>>>>,
}

impl RequestMap {
    pub fn new() -> Self {
        RequestMap {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Adds a new request entry.
    /// A thread has to pass in an oneshot sender to receive the response.
    /// The caller is responsible for ensuring that request IDs are unique.
    /// The caller is responsible for removing the request entry if it reaches some timeout and
    /// no longer awaits the response on the channel
    pub async fn add_request(&self, request_id: RequestId, sender: oneshot::Sender<Response>) {
        let mut map = self.map.write().await;
        map.insert(request_id, sender);
    }

    /// Retrieves and removes a request entry by its ID.
    /// Used to match incoming responses to the corresponding request.
    pub async fn take_request(&self, request_id: &RequestId) -> Option<oneshot::Sender<Response>> {
        let mut map = self.map.write().await;
        map.remove(request_id)
    }

    /// Checks if a request ID exists in the map.
    pub async fn contains_request(&self, request_id: &RequestId) -> bool {
        let map = self.map.read().await;
        map.contains_key(request_id)
    }

    pub async fn remove_request(&self, request_id: &RequestId) {
        let mut map = self.map.write().await;
        map.remove(request_id);
    }

    /// Clears all entries in the request map.
    pub async fn clear(&self) {
        let mut map = self.map.write().await;
        map.clear();
    }
}

impl RequestMap {
    /// Handles an incoming response.
    /// Checks if the response's `request_id` matches a pending request, and sends it to the corresponding oneshot.
    /// If the request ID is unknown, logs a warning and discards the incoming response.
    /// This method is thread-safe.
    pub async fn handle_response(&self, response: Response) {
        let request_id = response.request_id;

        // Check if the incoming response matches a pending request
        if !self.contains_request(&response.request_id).await {
            warn!("Received response with unknown request ID {}", request_id);
            return;
        }

        // Remove the request from the map and send the response to the corresponding oneshot
        if let Some(sender) = self.take_request(&request_id).await {
            if sender.send(response).is_err() {
                error!("Failed to resolve request with ID {}", request_id);
            }
        } else {
            warn!("Received response with unknown request ID {}", request_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::key::Key;
    use crate::networking::messages::ResponseType;
    use crate::networking::node_info::NodeInfo;
    use std::net::SocketAddr;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_request_map() {
        let request_map = RequestMap::new();

        let sender_node = NodeInfo::new(
            Key::new_random(),
            SocketAddr::new("127.0.0.1".parse().unwrap(), 8080),
        );
        let receiver_node = NodeInfo::new(
            Key::new_random(),
            SocketAddr::new("127.0.0.1".parse().unwrap(), 8081),
        );

        let (sender, receiver) = oneshot::channel();
        let request_id = Uuid::new_v4();

        request_map.add_request(request_id, sender).await;
        assert!(request_map.contains_request(&request_id).await);

        let response = Response::new(ResponseType::Pong, receiver_node, sender_node, request_id);
        request_map.handle_response(response.clone()).await;

        let result = receiver.await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), response);

        assert!(!request_map.contains_request(&request_id).await);
    }
}
