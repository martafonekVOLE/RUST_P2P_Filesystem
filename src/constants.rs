// This module contains all the constants that affect the behavior of the network.

/// # Main Kademlia constants.
/// This determines the size of Keys (in bytes) and K-Bucket entry count, which significantly
/// impacts the properties of the network.
pub const K: usize = 20;

/// # Parallelism factor for the network.
/// * Determines how many nodes are contacted in parallel lookups.
/// * Also determines the shard replication factor.
pub const ALPHA: usize = 3;

/// Maximum size of file stored in the network in MB.
pub const MAX_FILE_SIZE_MB: usize = 4096; // 4 GB

/// The size of single file data chunk stored in this network.
/// This strictly applies to all chunks (padding is used to fill the chunk if the data is smaller).
pub const CHUNK_DATA_SIZE_KB: usize = 1024; // 1 KB

/// Maximum amount of shards stored on a single node.
pub const MAX_SHARDS_STORED: usize = 1024;

/// Maximum amount of chunk data stored on a single node in MB.
pub const MAX_DATA_STORED_MB: usize = 4096; // 4 GB

/// The time after a node republishes chunks that its keeping
pub const DEFAULT_CHUNK_REUPLOAD_INTERVAL_S: u64 = 3600; // 1 h

/// The inactivity time after a chunk is considered expired and can be removed from the network.
pub const DEFAULT_CHUNK_EXPIRE_TIME_S: u64 = 86400; // 24 h

pub const LOOKUP_TIMEOUT_MILLISECONDS: u64 = 2500;
pub const BASIC_REQUEST_TIMEOUT_MILLISECONDS: u64 = 1000;
pub const TCP_TIMEOUT_MILLISECONDS: u64 = 5000;

/// Number of worker tasks for handling incoming requests.
pub const RECEIVE_WORKER_TASK_COUNT: u32 = 4;
