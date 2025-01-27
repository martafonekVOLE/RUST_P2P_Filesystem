# How to run

1) Start the initial beacon node:

```cargo run -- --config config_beacon.yaml --skip-join ```

2) Update the config.yaml with the beacon node's address and key


3) Connect as many other nodes as you want:

```cargo run -- --config config.yaml ```