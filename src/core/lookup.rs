use crate::constants::{ALPHA, K};
use crate::core::key::Key;
use crate::networking::node_info::NodeInfo;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub enum LookupRoundStatus {
    Fail,
    ImprovedNodes,
    FoundValue,
}

#[derive(Debug)]
pub struct LookupNodeInfo {
    node_info: NodeInfo,
    queried: bool,
    responded: bool,
}

impl LookupNodeInfo {
    pub fn new(node_info: NodeInfo) -> Self {
        LookupNodeInfo {
            node_info,
            queried: false,
            responded: false,
        }
    }

    pub fn has_failed_to_respond(&self) -> bool {
        self.queried && !self.responded
    }

    pub fn has_responded(&self) -> bool {
        self.queried && self.responded
    }

    pub fn has_been_queried(&self) -> bool {
        self.queried
    }
}

impl PartialEq for LookupNodeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.node_info == other.node_info
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum LookupSuccessType {
    Value,
    Nodes,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LookupResponse {
    resolver: NodeInfo,
    success: bool,
    success_type: Option<LookupSuccessType>,
    k_closest: Vec<NodeInfo>,
}

impl LookupResponse {
    pub fn new_nodes_successful(resolver: NodeInfo, k_closest: Vec<NodeInfo>) -> Self {
        LookupResponse {
            resolver,
            success: true,
            success_type: Some(LookupSuccessType::Nodes),
            k_closest,
        }
    }

    pub fn new_value_successful(resolver: NodeInfo) -> Self {
        LookupResponse {
            resolver,
            success: true,
            success_type: Some(LookupSuccessType::Value),
            k_closest: Vec::new(),
        }
    }

    pub fn new_failed(resolver: NodeInfo) -> Self {
        LookupResponse {
            resolver,
            success: false,
            success_type: None,
            k_closest: Vec::new(),
        }
    }

    pub fn get_resolver(&self) -> NodeInfo {
        self.resolver.clone()
    }

    pub fn get_k_closest(&self) -> Vec<NodeInfo> {
        self.k_closest.clone()
    }

    pub fn was_successful(&self) -> bool {
        self.success
    }

    pub fn get_success_type(&self) -> Option<LookupSuccessType> {
        self.success_type.clone()
    }
}

pub struct LookupBuffer {
    nodes: Vec<LookupNodeInfo>,
    target: Key,
    this_node: NodeInfo,
}

impl LookupBuffer {
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

    pub(crate) fn get_resulting_vector(&self) -> Vec<NodeInfo> {
        self.nodes
            .iter()
            .filter(|node| !node.has_failed_to_respond())
            .take(K)
            .map(|node| node.node_info.clone())
            .collect()
    }

    fn sort(&mut self) {
        self.nodes.sort_by(|a, b| {
            let dist_a = a.node_info.id.distance(&self.target);
            let dist_b = b.node_info.id.distance(&self.target);
            dist_a.cmp(&dist_b)
        });
    }

    pub(crate) fn record_lookup_round_responses(
        &mut self,
        responses: Vec<LookupResponse>,
        for_value: bool,
    ) -> bool {
        let previous_k_closest = self.get_resulting_vector();

        for response in responses {
            // If value is being looked up, this will return true
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

    fn check_node_key_uniqueness(&self, node: NodeInfo) -> bool {
        self.nodes
            .iter()
            .filter(|n| (n.node_info.id == node.id) && (n.node_info.address != node.address))
            .count()
            == 0
    }

    fn mark_node_as_queried(&mut self, node_id: Key) {
        for node in self.nodes.iter_mut() {
            if node.node_info.id == node_id {
                node.queried = true;
            }
        }
    }

    fn mark_node_as_responded(&mut self, node_id: Key) {
        for node in self.nodes.iter_mut() {
            if node.node_info.id == node_id {
                node.responded = true;
            }
        }
    }

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

    fn get_unresponsive_nodes(&self) -> Vec<NodeInfo> {
        self.nodes
            .iter()
            .filter(|node| node.queried && !node.responded)
            .map(|node| node.node_info.clone())
            .collect()
    }

    pub(crate) fn get_responsive_nodes(&self) -> Vec<NodeInfo> {
        self.nodes
            .iter()
            .filter(|node| node.queried && node.responded)
            .map(|node| node.node_info.clone())
            .collect()
    }

    pub(crate) fn get_unqueried_resulting_nodes(&self) -> Vec<NodeInfo> {
        self.nodes
            .iter()
            .take(K)
            .filter(|node| !node.queried)
            .map(|node| node.node_info.clone())
            .collect()
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
    fn test_lookup_node_info_flags() {
        let mut lni = LookupNodeInfo::new(make_node_info("test_flags", 9000));
        assert!(!lni.has_been_queried());
        assert!(!lni.has_responded());
        assert!(!lni.has_failed_to_respond());

        lni.queried = true;
        assert!(lni.has_been_queried());
        assert!(!lni.has_responded());
        assert!(lni.has_failed_to_respond());

        lni.responded = true;
        assert!(lni.has_responded());
        assert!(!lni.has_failed_to_respond());
    }

    #[test]
    fn test_lookup_response_successful() {
        let resolver = make_node_info("resolver", 7000);
        let neighbors = vec![make_node_info("n1", 7001), make_node_info("n2", 7002)];
        let resp = LookupResponse::new_nodes_successful(resolver.clone(), neighbors.clone());
        assert!(resp.was_successful());
        assert_eq!(resp.get_resolver(), resolver);
        assert_eq!(resp.get_k_closest(), neighbors);
    }

    #[test]
    fn test_lookup_response_failure() {
        let resolver = make_node_info("fail", 7003);
        let resp = LookupResponse::new_failed(resolver.clone());
        assert!(!resp.was_successful());
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
