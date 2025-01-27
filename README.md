# How to run

1) Start the initial beacon node:

```cargo run -- --config config_beacon.yaml --skip-join ```

2) Update the config.yaml with the beacon node's address and key


3) Connect as many other nodes as you want:

```cargo run -- --config config.yaml ```

___
___

# Theory 
## Milestone 1 :

### Theory, project description

Before we focus on anything related to files, we need to get the basics down, so thats what the first milestone should
be about.
This means we will have a network where
nodes can join, resolve another nodes IP and port given its Key (key is the id for both nodes and later the files)
and send some default ping+response messages to each other, even without knowing each other's IP.

Each node will first need to contact a beacon node - such that has a public ip and port, and is online 24/7. This
will still use the default implementation though. Once a new node is created, it will contact the beacon node and
get a list of nodes to contact. It will then contact these nodes and get a list of nodes from them, and so on, based on
the freshly generated key of this new node. This is done to populate the routing table of the new node.

Once the new node has a somewhat populated routing table (it will never be full, but it will have *enough* nodes
for startup in it), it will start to listen for incoming connections and requests. It will also start to ping the nodes
in its
routing table, to see if they are still online. If they are not, they will be removed from the routing table. This
should
be enough for the network to be able to function properly in this iteration.

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
