
File/shard hash has same length as node KEY.


key - hash, value - file/shard data

CONSTANTS:
```
K = 20
KEY_LENGHT = 160 (bit)
CACHE_ENTRY_DEFAULT_EXPIRATION_TIME = 24 hours
K_BUCKET_REFRESH_TIME = 1 hour # send FIND_NODE to random ID from k-bucket
CACHE_ENTRY_REPUBLISH_TIME = 1 hour # send STORE to K closest node for this cache entry
CHUNK_BYTE_LENGTH = ?
MAX_AVAILABLE_STORAGE = 4 GB?
MAX_FILES_STORED = ?
STORAGE_PATH = ?


```


Key, Value pair expiration time is EXP = 24 hours
```
Every node needs to have a thread (or task) to advance the time for each stored value.
	After it expires - remove from storage
```

```
Store <Key, Value> pair:
	Send STORE RPC to K nodes closest to Key:
		Nodes receive and store the pair into map
			Also store metadata of file (see file metadata section):
				Start timer to know when to re-publish
				(?)
		Nodes re-publish their pairs every hour (time can be adjusted) (?):
			This means send STORE to K closest nodes again
	Start timer for 24 hours (?) to re-publish

Additionally:

FIND_NODE:
	...
	When received FIND_NODE:
		If sending node is closer to any of stored pairs than you, replicate the pair to it (send STORE), do not remove from own storage
		// This 
	...
	If found node closer to any of stored pairs than you, replicate the pair to it (send STORE), do not remove  from own storage
	...
			
FIND_VALUE:
	Same as FIND_NODE but stop at the first node that contains the value
	...
	When found the value, send STORE the value at closest known node, that did not return the value
	...

```


## Caching

> Finally, in order to sustain consistency in the
> publishing-searching life-cycle of a ⟨key,value⟩ pair,
> we require that whenever a node w observes a new
> node u which is closer to some of w’s ⟨key,value⟩
> pairs, w replicates these pairs to u without removing
> them from its own database.

> To avoid “over-caching,” we make the expiration time
> of a ⟨key,value⟩ pair in any node’s database exponen-
> tially inversely proportional to the number of nodes
> between the current node and the node whose ID is
> closest to the key ID.3 While simple LRU eviction
> would result in a similar lifetime distribution, there
> is no natural way of choosing the cache size, since
> nodes have no a priori knowledge of how many val-
> ues the system will store.


## K-bucket refresh

> To avoid pathological cases when no traffic exists,
> each node refreshes a bucket in whose range it has
> not performed a node lookup within an hour. Re-
> freshing means picking a random ID in the bucket’s
> range and performing a node search for that ID.


## Join network

> To join the network, a node u must have a contact
> to an already participating node w. u inserts w into
> the appropriate k-bucket. u then performs a node
> lookup for its own node ID. Finally, u refreshes all k-
> buckets further away than its closest neighbor. Dur-
> ing the refreshes, u both populates its own k-buckets
> and inserts itself into other nodes’ k-buckets as nec-
> essary.

> This property is not violated
> by the insertion of new nodes that are close to the
> key, because as soon as such nodes are inserted, they
> contact their closest nodes in order to fill their buck-
> ets and thereby receive any nearby ⟨key,value⟩ pairs
> they should store.



# Retrieve information about available files in the network

It is not part of Kademlia, user needs to know the file hash and then query it hoping that it is stored somewhere in the network.

### CLI

```
CLI:

list - list all filenames. 
	Send FIND_FILES request to all known nodes (or take N nodes from random k-buckets) to get all files known to them.
	
	
get <filename> - download the file. Store to output directory (passed in config file or by flag)

```

### Centralized approach

```
Beacon just needs to keep a list of all files (shards) and its hashes
Any node would get a list of files from beacon node and use file(shard) key to look for file in the network:
```
Key: file hash, Value: map or shard hashes to node IDs that store them.

Every shard with go with hash(key) of the file for the beacon node to know which file does shard belong to.
When node wants to store a shard it also contacts the beacon node so that it stores the file metadata.


* How to know which node is beacon, what if it goes down, need a new beacon node.

### Decentralized approach

```
STORE:
	Sender:
		Payload:
			chunk data
			file metadata (see section)
	Receiver:
		store chunk data to local file
		add file metadata to files map
		
		

Introduce new command:
FIND_FILES:
	Send request to all known nodes (or take N nodes from random k-buckets) to get all files known to them
	
	Downsides:
		Heavy on traffic, prone to DOS (can put a cooldown on `list` command).

	Upsides:
		Guarrantees to find all files known in the network

	Alternative:
		Gossip propagation:
			When node finds out about new file (from shared metadata) it propagates the knowledge to K nearest neightbours. Could also make it recursive (heavy traffic). 
			
			Downsides:
				Heavy traffic, prone to DOS, does not guarrantee that nodes will know about all files.
				Also, what if new node joins, it can only wait to get info about available files.
```

# File chunking

#### File metadata

```
File metadata:
	file name
	file size
	file hash
	chunks metadata [
		{
			chunk hash,
			chunk index, (to not rely on JSON array order, may be changed)
			node ID, that has the chunk (questionable, maybe better not to use at all),
		},
		...
	]
```


```
Upload a file:
	Read file
	Compute file hash
	Write to metadata
	Split into chunks
	For each chunk:
		Compute hash
		Find K closest nodes to own the chunk
		Populate the metadata
	
	Add file metadata to local map of known files

Download a file:
	Look into map of known files
	If file size too big:
		-> return
	For each chunk in metadata:
		(Optional): {
			Contact the mentioned node to donwload the chunk
			If does not respond:
				proceed to FIND_VALUE
		} else {
			Do FIND_VALUE on chunk hash
		}
		
		Store chunk to local file in a dedicated storage dir
	Combine all chunks to get the file
	Store file in user-specified output dir (not node local dir for storing chunks)
	
		
```














# Misc


Each node keeps table of `<key, value>` of file chunks (or whole files)


### Suggestions for Your Implementation

1. **Key Assignment:** Use a consistent hash function to generate node IDs based on their IP/port or another stable attribute.
2. **Routing Table:** Maintain a structured routing table (e.g., k-buckets in Kademlia) to organize nodes by XOR distance.
3. **Redundancy:** Query multiple nodes per iteration to account for node failures or delays.
4. **Iterative Narrowing:** Prioritize nodes with smaller XOR distances in each iteration.


### Generate ID based on hash(ip+port)

What if port changes? (i.e. is occupied on machine)
