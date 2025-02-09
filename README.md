# P2P Filesystem
This project is an implementation of P2P filesystem using Kademlia.

**Contents**:

- [How to configure](#How-to-configure) 
- [How to run](#How-to-run)
- [How to use](#How-to-use-the-network)

- [Theory](#Theory)



## How to configure
This project can be configured using two config files, `config.yaml` and `config_beacon.yaml`. 

### Config_beacon.yaml
This file is a configuration file for a beacon node. Beacon node is a special type of node, which is the first one in the network and upon connectin does not go through joining procedure. User can specify several parameters.

- **node_port**: User can specify a port on which the application is running on. If no port is provided, the applicationdoes use an unused port.
- **cache_file_path**: Default path where cache is stored for a node.
- **storage_path**: Default path where uploaded data is stored.
- **ip_address_type**: Does specify a type of IP address. Possible fields are *[loopback, public, local]*.

### Config.yaml
This file is a configuration file for basic node. User can specify same parameters as in `config_beacon.yaml`, but there are two more parameters, which must be filled.

- **beacon_node_address**: Ip address of the beacon node (node cannot join the network without it) in format *127.0.0.1:8081*.
- **beacon_node_key**: Unique NodeID of the beacon node. This must be K bytes. 


## How to run

### 1.  Start the initial beacon node:
After the project is configured, network should be ready. First of all the **Beacon Node** must join the network. This step is vital in order to allow other nodes to join as well.

```cargo run -- --config config_beacon.yaml --skip-join ```

> NOTE: You can alternatively add `-v` flag in order to see all important messages from the network communication.  

Expected output is: 
```
──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────
Welcome to the network! Your node is 6eb76770415ac55f7ec5ccf52c91047d39ed38f4 @ 127.0.0.1:8081
Available commands:
 - ping <key>: Send a PING request to the specified node
 - find_node <key>: Resolves 20 closest nodes to <key>
 - upload <filepath>: Upload a file to the network
 - download <file_handle> <storage_dir>: Download a file from the network
 - dump_rt: Display the contents of the routing table
 - dump_chunks: Display the chunks owned by this node
 - Note: <key> should be a 40-character hexadecimal string
──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────
```

### 2. Update the config.yaml 
As described in the [How to configure](#How-to-configure) section, user must update the `config.yaml` file with the beacon node's address and key.

```
beacon_node_address: "127.0.0.1:8081"
beacon_node_key: "6eb76770415ac55f7ec5ccf52c91047d39ed38f4"
# node_port: 8081
cache_file_path: "cache.json"
storage_path: "./storage-node01"
ip_address_type: "loopback" # "public" or "loopback" or "local"

```

### 3. Connect other nodes
After the configuration is updated, user can add as many nodes as he wants. Basic nodes can join the network using following command. 

```cargo run -- --config config.yaml ```

> NOTE: You can alternatively add `-v` flag in order to see all important messages from the network communication.  

> If you want to run multiple nodes from one machine please make sure that you update port and the cache and storage path for each of them.

## How to use the network
If the network is running and there are nodes connected to it, user can execute some commands. Commands can be executed from all nodes via CLI.

#### Available commands:
| Command | Parameters | Description |
| ------ | ------ | ------ |
|    **PING**    |    `key`: 40-character hexadecimal string    |    Send a PING request to the specified node    |
|    **FIND_NODE**    |    `key`: 40-character hexadecimal string    |    Resolves 20 closest nodes to the `key`.     |
|    **UPLOAD**    |    `filepath`: valid path to file as string    |    Upload a file to the network    |
|    **DOWNLOAD**    |    `file_handle`: file handle identifier as string, `storage_dir`: valid path to a directory     |    Download a file from the network    |
|    **DUMP_RT**    |   --     |    Display the contents of the routing table    |
|    **DUMP_CHUNKS**    |   --     |    Display the chunks owned by this node    |

#### Usage
Each command should be used like this:
```command [params]```

Response will be displayed directly on the terminal window.

**Example command**:  
 ```upload file.txt```

**Example response**:
```Chunk successfully uploaded.
File uploaded successfully!
File handle: 080000000000000066696c652e74787420000000000000004871bda4d11162045726934d9324c2ab61bf0986fe21d48df2a84993fbc2d54201000000000000002800000000000000346462336534633566393039613965393066623866663434306563353639383263326265313536640c0000000000000097ca18e214ecf47a7f2581a1
──────────────────────────────── ✧ ✧ ✧ ────────────────────────────────
```
___
___

# Theory 
## Milestone 1:

### Theory, project description

Each node that has successfully passed the initial setup will be able to send messages to other nodes. The messages
will be of type `PING`, `STORE`, `FIND_NODE` and `FIND_VALUE`. For this milestone, we will only focus on the `PING`
and `FIND_NODE` messages. The `PING` message is used to check if a node is still online and is issued only to nodes
known by their actual IP and port. The `FIND_NODE` message is used to find a node in the network, given its Key.

Furthermore, we will have to implement a simple CLI for the node. This CLI will be able to start a node, check its
status, kill it and disconnect it from the network.

The hardest part of this milestone will be the actual network setup and the routing table population. We would be using
the tokio for most operations on the networking side of things. An async-first approach must be taken, as we will be
dealing with **a lot** of network operations as the node number increases.

To ensure that the thing we will be building actually works, we will need to develop a sort of testing framework that
will simulate the live network traffic on localhost. This will be used to test the network and the nodes in a controlled
environment, so we can monitor the bottlenecks and failure points of the network before progressing to the next stage,
where
this framework will also be used.

### Goals outline for MS1

- Have a network of nodes that can ping each other
- Be able to add a node to the network
- Nodes manage their own routing tables
- A node can be found only by its Key
- Have a complete and tested communication interface with implement the `PING` and `FIND_NODE` messages.
- Have a CLI that can start, check status, kill and disconnect nodes
- Prepare a sort of testing/simulation framework to run nodes on localhost

### Time plan and organization

A weekly online meeting should be held to discuss the progress and the next steps. The meeting should be held on the
start of the weekend so that we have enough time to get something done with the new information from each other.

Since we only have a little over two months for this project, a 3-week sprint cycle should be adopted. This means that
we will have 3 weeks to complete the tasks outlined above. After that, two more milestones will be created, each with
their own 3-week sprint cycle - in the optimal case :D...

---

---

## Milestone 2 :

### Goals outline for MS2

- Implement TCP data transfer
- Have a complete and tested communication interface with implement the `STORE` and `FIND_VALUE` messages.
- Implement a custom family of `STORE` messages. 
- Setup active and passive data managers.
- Introduce file sharding.
- Introduce file shard encryption.
- Follow the suggested communication outline

### Time plan and organization
An online meeting should be held twice a week to discuss the progress and the next
steps. 

Since we only have a little over two weeks for this project, we have set an internal deadline to **February 7th**.
