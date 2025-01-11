
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use p2p::core::key::Key;
use p2p::routing::routing_table::RoutingTable;
use p2p::networking::node_info::NodeInfo;

#[test]
fn test_routing_table_initialization() {
    let id = Key::new_random();
    let routing_table = RoutingTable::new(id);

    // Verify the number of buckets
    assert_eq!(routing_table.get_buckets().len(), RoutingTable::NUM_BUCKETS);
    // for bucket in &routing_table.get_buckets() {
    //     assert!(bucket.read()); // Ensure each bucket is initialized
    // }
}

#[tokio::test]
async fn test_store_nodeinfo_success() {
    let id = Key::from_input(b"local_node");
        let routing_table = RoutingTable::new(id);

        let remote_id = Key::from_input(b"remote_node");
        let remote_address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 8080);
        let node_info = NodeInfo::new(remote_id, remote_address);

        // Store the node and verify success
        let result = routing_table.store_nodeinfo(node_info.clone()).await;
        assert!(result);

        // Verify the node is added to the correct bucket
        // let bucket_index = id.leading_zeros(&remote_id) as usize;
        // let bucket = routing_table.get_buckets().get(bucket_index).unwrap();
        // let bucket = bucket.read().await;

        // Assuming KBucket has a method to check if a node exists
        // For now, mock this method as true to simulate functionality
        // assert!(bucket.contains(&node_info)); // Replace with actual KBucket method if implemented
}

// #[tokio::test]
// async fn test_store_nodeinfo_failure_out_of_bounds() {
//     let id = Key::new_random();
//     let routing_table = RoutingTable::new(id);

//     // Create a fake node ID with a leading zeros count that exceeds bucket range
//     let out_of_bounds_key = Key::new_random();
//     let out_of_bounds_info = NodeInfo::new(out_of_bounds_key);

//     // Force an invalid leading zeros value
//     let bucket_index = RoutingTable::NUM_BUCKETS + 1;
//     assert!(routing_table.buckets.get(bucket_index).is_none());

//     // Attempting to store should fail gracefully
//     let result = routing_table.store_nodeinfo(out_of_bounds_info).await;
//     assert!(!result);
// }
