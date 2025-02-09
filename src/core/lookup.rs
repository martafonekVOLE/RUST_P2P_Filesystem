use crate::constants::{ALPHA, K};
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use serde::{Deserialize, Serialize};

///
/// # The status of a lookup round.
///
/// This is used to determine the next steps in the lookup process.
/// The result of a lookup round can be:
/// ## Results:
/// ### `Fail`: The lookup has failed.
/// ### Success for find_node lookups: `ImprovedNodes`
/// The lookup has improved the list of nodes (found closer nodes). This is used to determine if the lookup should continue.
/// If no closer nodes are found, the lookup should terminate.
/// ### Success for find_value lookups: `FoundValue`
/// The lookup has found the value. This status should terminate the lookup.
///
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum LookupRoundStatus {
    Fail,
    ImprovedNodes,
    FoundValue,
}

///
/// # Extension for NodeInfo that includes lookup-specific information.
///
/// This is used to keep track of the state of a node in a lookup process.
/// - queried: Has the node been queried - lookup request sent
/// - responded: Has the node responded to the lookup request - this means the node is alive.
///
#[derive(Debug)]
pub struct LookupNodeInfo {
    node_info: NodeInfo,
    queried: bool,
    responded: bool,
}

impl LookupNodeInfo {
    ///
    /// Creates a fresh LookupNodeInfo with the given NodeInfo.
    /// Should be constructed from recently discovered nodes.
    ///
    pub fn new(node_info: NodeInfo) -> Self {
        LookupNodeInfo {
            node_info,
            queried: false,
            responded: false,
        }
    }

    ///
    /// Returns true if the node has failed to respond during the lookup.
    /// This means that the node has been queried but has not responded.
    ///
    pub fn has_failed_to_respond(&self) -> bool {
        self.queried && !self.responded
    }
}

impl PartialEq for LookupNodeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.node_info == other.node_info
    }
}

///
/// # The type of success in a single lookup response.
///
/// This is used to determine the type of response in a lookup response. Can be either:
/// - `Value`: The lookup has found the value.
/// - `Nodes`: The lookup node has nod found a value requested in a value lookup, so it responds
///     with the closest nodes it knows of. This implies that the queried node does not store the value.
///
#[derive(Serialize, Deserialize, Clone)]
pub enum LookupSuccessType {
    Value,
    Nodes,
}

///
/// # The response to a lookup request.
///
/// This is used to respond to a lookup request. The response can be either:
/// - `Nodes`: The lookup has found nodes that are closer to the target, OR the node does not store the value
///     during a value lookup, so it responds with the closest nodes it knows of. This is determined by
///     the `success_type` field.
/// - `Value`: The lookup has found the value.
///
#[derive(Serialize, Deserialize, Clone)]
pub struct LookupResponse {
    resolver: NodeInfo,
    success: bool,
    success_type: Option<LookupSuccessType>,
    k_closest: Vec<NodeInfo>,
}

impl LookupResponse {
    ///
    /// Constructor for a successful lookup response with nodes.
    ///
    pub fn new_nodes_successful(resolver: NodeInfo, k_closest: Vec<NodeInfo>) -> Self {
        LookupResponse {
            resolver,
            success: true,
            success_type: Some(LookupSuccessType::Nodes),
            k_closest,
        }
    }

    ///
    /// Constructor for a successful lookup response with a value.
    ///
    pub fn new_value_successful(resolver: NodeInfo) -> Self {
        LookupResponse {
            resolver,
            success: true,
            success_type: Some(LookupSuccessType::Value),
            k_closest: Vec::new(),
        }
    }

    ///
    /// Constructor for a failed lookup response.
    ///
    pub fn new_failed(resolver: NodeInfo) -> Self {
        LookupResponse {
            resolver,
            success: false,
            success_type: None,
            k_closest: Vec::new(),
        }
    }

    ///
    /// Returns the resolver node of this lookup response.
    ///
    pub fn get_resolver(&self) -> NodeInfo {
        self.resolver.clone()
    }

    ///
    /// Returns the K closest nodes the resolver knows of for this lookup response.
    ///
    pub fn get_k_closest(&self) -> Vec<NodeInfo> {
        self.k_closest.clone()
    }

    ///
    /// Returns the type of success for this lookup response.
    /// This can be either:
    /// `Value`: The lookup has found the value.
    /// `Nodes`:
    /// For a find_value lookup, the resolver node has nod found a value requested in a value lookup, so it responds
    /// with the closest nodes it knows of. This implies that the queried node does not store the value.
    /// For a find_node lookup, these are the closest nodes to the target the resolver knows of.
    ///
    pub fn get_success_type(&self) -> Option<LookupSuccessType> {
        self.success_type.clone()
    }
}

///
/// # A helpers struct for resolving a `find_value` or `find_node` lookup.
///
/// This structure keeps a vector of nodes that are closest to the target key. For each lookup iteration,
/// the method `record_lookup_round_responses` should be called with the responses from the nodes that were queried.
/// This returns a bool signifying whether the lookup should continue.
/// - For `find_value` lookups, the bool means 'was value found'
/// - For `find_node` lookups, this signifies whether the list of closest nodes has been improved, e.g.
///     'did the results improve'.
///
/// These results are necessary to control the Kademlia parallel iterative lookup algorithm.
///
/// When the algorithm is finished, the resulting nodes can be retrieved with `get_resulting_vector()`.
///
/// This struct also keeps the nodes sorted, deduplicated and keeps track of unresponsive nodes.
///
/// Encountered nodes to be used for kbucket updates can be retrieved with `get_responsive_nodes()`.
///
pub struct LookupBuffer {
    nodes: Vec<LookupNodeInfo>,
    target: Key,
    this_node: NodeInfo,
}

impl LookupBuffer {
    ///
    /// Constructor for a new LookupBuffer.
    ///
    /// # Arguments
    ///
    /// * `target` - The target key of the lookup.
    /// * `initial_nodes` - The initial nodes to start the lookup with. You should get them from
    ///  - it should be alpha nodes closest to the target that you can find in this node's routing table.
    /// * `this_node` - The node that is performing the lookup.
    ///
    pub(crate) fn new(target: Key, initial_nodes: Vec<NodeInfo>, this_node: NodeInfo) -> Self {
        let mut nodes = Vec::new();
        for node in initial_nodes {
            nodes.push(LookupNodeInfo::new(node));
        }
        let mut result_buffer = LookupBuffer {
            nodes,
            target,
            this_node,
        };
        result_buffer.sort();
        result_buffer
    }

    ///
    /// Gets the result nodes of the lookup for the current state of the buffer.
    /// This will be maximum `K` closest nodes to the target that are responsive.
    ///
    /// *Hint: Just call this once you're done with the lookup to get the final result.*
    ///
    pub(crate) fn get_resulting_vector(&self) -> Vec<NodeInfo> {
        self.nodes
            .iter()
            .filter(|node| !node.has_failed_to_respond())
            .take(K)
            .map(|node| node.node_info.clone())
            .collect()
    }

    ///
    /// Gets the nodes from the top `K` closest to the target that haven't been queried yet
    ///
    /// This is supposed to be called after the iterative part of the Kademlia lookup protocol
    /// has been completed. The nodes gathered from this method should be used perform the last round
    /// of lookup which can ignore the `ALPHA` parallelism constraint.
    ///
    pub(crate) fn get_unqueried_resulting_nodes(&self) -> Vec<NodeInfo> {
        self.nodes
            .iter()
            .take(K)
            .filter(|node| !node.queried)
            .map(|node| node.node_info.clone())
            .collect()
    }

    ///
    /// Gets the `ALPHA` closest nodes to the target that haven't been queried yet.
    /// This should be used in each of the iterative lookup steps and parallel requests should be
    /// sent to these nodes during the lookup round.
    ///
    pub(crate) fn get_alpha_unqueried_nodes(&mut self) -> Vec<NodeInfo> {
        let candidates: Vec<NodeInfo> = self
            .nodes
            .iter()
            .filter(|node| !node.queried)
            .map(|node| node.node_info.clone())
            .take(ALPHA)
            .collect();

        // Mark candidates as queried
        for node in candidates.iter() {
            self.mark_node_as_queried(node.id);
        }
        candidates
    }

    ///
    /// Get all responsive nodes that have been encountered over the existence of this lookup buffer.
    /// Note: this will always return unspecified number 0f unique nodes.
    ///
    pub(crate) fn get_responsive_nodes(&self) -> Vec<NodeInfo> {
        self.nodes
            .iter()
            .filter(|node| node.queried && node.responded)
            .map(|node| node.node_info.clone())
            .collect()
    }

    ///
    /// # Main method for recording lookup iteration result
    ///
    /// This method should be called after each iteration of the lookup process to store the results.
    /// It will only store unique nodes and sort them by distance to the target.
    ///
    /// # Arguments
    /// * `responses` - The responses from the nodes that were queried in the lookup iteration.
    /// * `for_value` - Whether the lookup is find_value or find_node type.
    ///     If this method is in a find_value lookup, it will return true if the value was found.
    ///     If this method is in a find_node lookup or the value was not found in the find_value lookup,
    ///     it will return true if the list of closest nodes has been improved.
    ///
    pub(crate) fn record_lookup_round_responses(
        &mut self,
        responses: Vec<LookupResponse>,
        for_value: bool,
    ) -> bool {
        let previous_k_closest = self.get_resulting_vector();

        for response in responses {
            // For find_value lookup - this will return true if we found the value and should
            // terminate the lookup in such circumstances.
            if for_value && matches!(response.success_type, Some(LookupSuccessType::Value)) {
                return true;
            }

            // If the response was a failure or a different type of success,
            // mark the resolver node as responded
            if !response.success
                || (!for_value && matches!(response.success_type, Some(LookupSuccessType::Value)))
            {
                self.mark_node_as_queried(response.resolver.id);
                continue;
            }

            for node in response.k_closest {
                let lookup_node_info = LookupNodeInfo::new(node.clone());
                if !self.nodes.contains(&lookup_node_info) && self.this_node != node {
                    self.nodes.push(lookup_node_info);
                }
            }
            self.mark_node_as_responded(response.resolver.id);
        }

        self.sort();
        // Returns bool "Did we improve"?
        previous_k_closest != self.get_resulting_vector()
    }

    ///
    /// Sorts the nodes in the buffer by XOR distance to the target.
    ///
    fn sort(&mut self) {
        self.nodes.sort_by(|a, b| {
            let dist_a = a.node_info.id.distance(&self.target);
            let dist_b = b.node_info.id.distance(&self.target);
            dist_a.cmp(&dist_b)
        });
    }

    ///
    /// Marks an input node given by key as queried.
    ///
    fn mark_node_as_queried(&mut self, node_id: Key) {
        for node in self.nodes.iter_mut() {
            if node.node_info.id == node_id {
                node.queried = true;
            }
        }
    }

    ///
    /// Marks an input node given by key as responded.
    ///
    fn mark_node_as_responded(&mut self, node_id: Key) {
        for node in self.nodes.iter_mut() {
            if node.node_info.id == node_id {
                node.responded = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    // Helper to create a NodeInfo with a reproducible key
    fn make_node_info(id_seed: &str, port: u16) -> NodeInfo {
        let key = Key::from_input(id_seed.as_bytes());
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        NodeInfo::new(key, addr)
    }

    fn get_this_node_mock() -> NodeInfo {
        make_node_info("this_node", 0)
    }

    #[test]
    fn test_lookup_node_info_equality() {
        let node_a = make_node_info("same", 8000);
        let node_b = make_node_info("same", 8000);
        let lookup_a = LookupNodeInfo::new(node_a.clone());
        let lookup_b = LookupNodeInfo::new(node_b.clone());
        assert_eq!(lookup_a, lookup_b);

        let node_c = make_node_info("diff", 8001);
        let lookup_c = LookupNodeInfo::new(node_c);
        assert_ne!(lookup_a, lookup_c);
    }

    #[test]
    fn test_lookup_response_successful() {
        let resolver = make_node_info("resolver", 7000);
        let neighbors = vec![make_node_info("n1", 7001), make_node_info("n2", 7002)];
        let resp = LookupResponse::new_nodes_successful(resolver.clone(), neighbors.clone());
        assert!(resp.success);
        assert_eq!(resp.get_resolver(), resolver);
        assert_eq!(resp.get_k_closest(), neighbors);
    }

    #[test]
    fn test_lookup_response_failure() {
        let resolver = make_node_info("fail", 7003);
        let resp = LookupResponse::new_failed(resolver.clone());
        assert!(!resp.success);
        assert_eq!(resp.get_resolver(), resolver);
        assert!(resp.get_k_closest().is_empty());
    }

    #[test]
    fn test_lookup_buffer_creation_and_sorting() {
        let target = Key::from_input(b"target");
        let initial_nodes = vec![
            make_node_info("node_a", 8001),
            make_node_info("node_b", 8002),
            make_node_info("node_c", 8003),
        ];
        let buffer = LookupBuffer::new(target, initial_nodes, get_this_node_mock());
        assert_eq!(buffer.nodes.len(), 3);
    }

    #[test]
    fn test_lookup_buffer_get_resulting_vector() {
        let target = Key::from_input(b"resvec");
        let mut buffer = LookupBuffer::new(
            target,
            vec![
                make_node_info("x", 8004),
                make_node_info("y", 8005),
                make_node_info("z", 8006),
                make_node_info("w", 8007),
            ],
            get_this_node_mock(),
        );
        buffer.nodes[2].queried = true;
        buffer.nodes[2].responded = false;
        let result = buffer.get_resulting_vector();
        assert_eq!(result.len(), 3);
        let failing_key = buffer.nodes[2].node_info.id;
        assert!(!result.iter().any(|n| n.id == failing_key));
    }

    #[test]
    fn test_record_lookup_round_responses() {
        let target = Key::from_input(b"lookup_record");
        let mut buffer = LookupBuffer::new(
            target,
            vec![make_node_info("r1", 9001), make_node_info("r2", 9002)],
            get_this_node_mock(),
        );
        let old_vec = buffer.get_resulting_vector();

        let resp_success = LookupResponse::new_nodes_successful(
            make_node_info("r1", 9001),
            vec![make_node_info("new_1", 9003), make_node_info("new_2", 9004)],
        );
        let resp_fail = LookupResponse::new_failed(make_node_info("r2", 9002));
        let changed = buffer.record_lookup_round_responses(vec![resp_success, resp_fail], false);
        assert!(changed);

        let new_vec = buffer.get_resulting_vector();
        assert_ne!(old_vec, new_vec);
        assert!(buffer
            .nodes
            .iter()
            .any(|lni| lni.node_info.id == Key::from_input(b"new_1")));
        assert!(buffer
            .nodes
            .iter()
            .any(|lni| lni.node_info.id == Key::from_input(b"new_2")));
    }

    #[test]
    fn test_get_alpha_unqueried_nodes() {
        let target = Key::from_input(b"alpha");
        let mut buffer = LookupBuffer::new(
            target,
            vec![
                make_node_info("alpha_1", 8081),
                make_node_info("alpha_2", 8082),
                make_node_info("alpha_3", 8083),
                make_node_info("alpha_4", 8084),
            ],
            get_this_node_mock(),
        );
        let alpha_nodes = buffer.get_alpha_unqueried_nodes();
        assert_eq!(alpha_nodes.len(), ALPHA);
        for node in alpha_nodes {
            let found = buffer
                .nodes
                .iter()
                .find(|n| n.node_info.id == node.id)
                .unwrap();
            assert!(found.queried);
        }
    }

    #[test]
    fn test_get_unqueried_resulting_nodes() {
        let target = Key::from_input(b"unqueried");
        let mut buffer = LookupBuffer::new(
            target,
            vec![
                make_node_info("u1", 8101),
                make_node_info("u2", 8102),
                make_node_info("u3", 8103),
                make_node_info("u4", 8104),
            ],
            get_this_node_mock(),
        );
        buffer.nodes[0].queried = true;
        buffer.nodes[1].queried = true;
        let unqueried = buffer.get_unqueried_resulting_nodes();
        assert_eq!(unqueried.len(), 2);
    }

    #[test]
    fn test_cant_add_this_node_to_nodes() {
        let target = Key::from_input(b"this_node");
        let this_node = get_this_node_mock();

        let resolver = make_node_info("resolver", 7000);

        let mut buffer = LookupBuffer::new(target, vec![resolver.clone()], this_node.clone());

        // Result containing this_node
        let lr = LookupResponse::new_nodes_successful(resolver, vec![this_node.clone()]);

        // Record the response
        buffer.record_lookup_round_responses(vec![lr], false);

        // Check that this_node is not in the resulting vector
        let resulting_nodes = buffer.get_resulting_vector();
        assert!(!resulting_nodes.contains(&this_node));
    }
}
