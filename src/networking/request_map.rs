use crate::networking::messages::RequestId;
use crate::networking::messages::Response;
use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};

///
/// A map of pending requests and their corresponding oneshot senders.
/// Used to match incoming responses to the corresponding requests via their unique `RequestId`.
/// The map is thread-safe and can be shared across multiple threads.
///
/// Note: The caller is responsible for removing request entries if they reach some
/// timeout and no longer await the response on the channel.
///
#[derive(Clone)]
pub struct RequestMap {
    map: Arc<RwLock<HashMap<RequestId, oneshot::Sender<Response>>>>,
}

impl RequestMap {
    ///
    /// Default constructor.
    ///
    pub fn new() -> Self {
        RequestMap {
            map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    ///
    /// Adds a new request entry.
    ///
    /// A thread has to pass in an oneshot sender to receive the response.
    /// The caller is responsible for ensuring that request IDs are unique.
    /// The caller is responsible for removing the request entry if it reaches some timeout and
    /// no longer awaits the response on the channel
    ///
    pub async fn add_request(&self, request_id: RequestId, sender: oneshot::Sender<Response>) {
        let mut map = self.map.write().await;
        map.insert(request_id, sender);
    }

    ///
    /// Retrieves and removes a request entry by its ID.
    /// Used to match incoming responses to the corresponding request.
    ///
    pub async fn take_request(&self, request_id: &RequestId) -> Option<oneshot::Sender<Response>> {
        let mut map = self.map.write().await;
        map.remove(request_id)
    }

    ///
    /// Checks if a request ID exists in the map.
    ///
    pub async fn contains_request(&self, request_id: &RequestId) -> bool {
        let map = self.map.read().await;
        map.contains_key(request_id)
    }

    ///
    /// Removes a request entry by its ID.
    ///
    pub async fn remove_request(&self, request_id: &RequestId) {
        let mut map = self.map.write().await;
        map.remove(request_id);
    }
}

impl RequestMap {
    ///
    /// Sends the incoming response to the corresponding oneshot channel.
    ///
    /// Checks if the response's `request_id` matches a pending request, and sends it to the corresponding oneshot.
    /// If the request ID is unknown, logs a warning and discards the incoming response.
    ///
    /// This method is thread-safe.
    ///
    pub async fn handle_response(&self, response: Response) {
        info!("Received response: {}", response);
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
    use tokio::sync::oneshot;
    use uuid::Uuid;

    /// Test that adding a request makes it visible in the map.
    #[tokio::test]
    async fn test_add_and_contains_request() {
        let request_map = RequestMap::new();
        let (tx, _rx) = oneshot::channel();
        let request_id = Uuid::new_v4();
        request_map.add_request(request_id, tx).await;
        assert!(request_map.contains_request(&request_id).await);
    }

    /// Test that taking a request removes it from the map and that the sender can be used to send a response.
    #[tokio::test]
    async fn test_take_request() {
        let request_map = RequestMap::new();
        let (tx, rx) = oneshot::channel();
        let request_id = Uuid::new_v4();

        request_map.add_request(request_id, tx).await;
        // Take (and remove) the request from the map.
        let sender_opt = request_map.take_request(&request_id).await;
        assert!(sender_opt.is_some());
        assert!(!request_map.contains_request(&request_id).await);

        // Prepare a dummy response.
        let dummy_sender = NodeInfo::new(
            Key::new_random(),
            "127.0.0.1:9000".parse::<SocketAddr>().unwrap(),
        );
        let dummy_receiver = NodeInfo::new(
            Key::new_random(),
            "127.0.0.1:9001".parse::<SocketAddr>().unwrap(),
        );
        let response = Response::new(ResponseType::Pong, dummy_receiver, dummy_sender, request_id);

        // Use the taken sender to send the response.
        if let Some(sender) = sender_opt {
            let send_result = sender.send(response.clone());
            assert!(send_result.is_ok());
        }
        // The oneshot receiver should now yield the response.
        let received = rx.await.expect("Expected to receive a response");
        assert_eq!(received, response);
    }

    /// Test that remove_request successfully removes an entry.
    #[tokio::test]
    async fn test_remove_request() {
        let request_map = RequestMap::new();
        let (tx, _rx) = oneshot::channel();
        let request_id = Uuid::new_v4();

        request_map.add_request(request_id, tx).await;
        assert!(request_map.contains_request(&request_id).await);

        request_map.remove_request(&request_id).await;
        assert!(!request_map.contains_request(&request_id).await);
    }

    /// Test that handle_response sends the response to the corresponding oneshot channel.
    #[tokio::test]
    async fn test_handle_response_known() {
        let request_map = RequestMap::new();

        let sender_node = NodeInfo::new(
            Key::new_random(),
            "127.0.0.1:8080".parse::<SocketAddr>().unwrap(),
        );
        let receiver_node = NodeInfo::new(
            Key::new_random(),
            "127.0.0.1:8081".parse::<SocketAddr>().unwrap(),
        );
        let (tx, rx) = oneshot::channel();
        let request_id = Uuid::new_v4();

        request_map.add_request(request_id, tx).await;
        let response = Response::new(ResponseType::Pong, receiver_node, sender_node, request_id);
        request_map.handle_response(response.clone()).await;

        // The oneshot channel should receive the response.
        let received = rx.await.expect("Expected to receive a response");
        assert_eq!(received, response);
        // And the request entry is removed.
        assert!(!request_map.contains_request(&request_id).await);
    }

    /// Test that handling a response with an unknown request ID simply discards the response.
    #[tokio::test]
    async fn test_handle_response_unknown() {
        let request_map = RequestMap::new();
        let request_id = Uuid::new_v4(); // This ID is not added to the map.
        let dummy_sender = NodeInfo::new(
            Key::new_random(),
            "127.0.0.1:9002".parse::<SocketAddr>().unwrap(),
        );
        let dummy_receiver = NodeInfo::new(
            Key::new_random(),
            "127.0.0.1:9003".parse::<SocketAddr>().unwrap(),
        );
        let response = Response::new(ResponseType::Pong, dummy_receiver, dummy_sender, request_id);

        // handle_response should log a warning but not panic.
        request_map.handle_response(response).await;
        // Confirm that the unknown request ID is not in the map.
        assert!(!request_map.contains_request(&request_id).await);
    }

    /// Test that if the oneshot receiver has been dropped, handle_response removes the request entry.
    #[tokio::test]
    async fn test_handle_response_sender_dropped() {
        let request_map = RequestMap::new();
        let (tx, rx) = oneshot::channel::<Response>();
        let request_id = Uuid::new_v4();

        request_map.add_request(request_id, tx).await;
        // Drop the receiver so that sending on the channel will fail.
        drop(rx);

        let dummy_sender = NodeInfo::new(
            Key::new_random(),
            "127.0.0.1:9004".parse::<SocketAddr>().unwrap(),
        );
        let dummy_receiver = NodeInfo::new(
            Key::new_random(),
            "127.0.0.1:9005".parse::<SocketAddr>().unwrap(),
        );
        let response = Response::new(ResponseType::Pong, dummy_receiver, dummy_sender, request_id);
        request_map.handle_response(response).await;
        // The request entry should be removed even though sending failed.
        assert!(!request_map.contains_request(&request_id).await);
    }
}
