#!/usr/bin/env python3
import subprocess
import threading
import time
import re
import networkx as nx
import matplotlib.pyplot as plt
import sys
import os
import shutil

class NodeProcess:
    def __init__(self, name, command):
        """
        Launch a node process and start a thread to capture its output.
        :param name: A friendly name for the node (e.g. "Beacon", "Node2")
        :param command: List of command-line arguments to launch the node.
        """
        self.name = name
        self.command = command
        self.process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stdin=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            universal_newlines=True,
            bufsize=1  # line-buffered
        )
        self.output_lines = []     # Keep all output lines
        self.node_id = None        # Will be parsed from output
        self.routing_table = set() # Set of node IDs parsed from "dump_rt" output

        # Start a background thread to continuously read stdout.
        self.thread = threading.Thread(target=self.read_output, daemon=True)
        self.thread.start()

    def read_output(self):
        """Continuously read output lines from the process."""
        routing_section = False  # Flag for when we are inside a dump_rt output block.
        for line in self.process.stdout:
            line = line.strip()
            self.output_lines.append(line)
            # Echo the output (prefixed with the node name) for debugging.
            print(f"[{self.name}] {line}")
            sys.stdout.flush()

            # Look for the node's startup message.
            m = re.search(r"Your node (?:is )?([0-9a-f]+)", line)
            if m and not self.node_id:
                self.node_id = m.group(1)

            # Detect the start of the routing table output.
            if "Routing table content:" in line:
                routing_section = True
                continue

            # If inside a routing table section, parse node entries.
            if routing_section:
                m = re.search(r"-\s*([0-9a-f]+)\s+@", line)
                if m:
                    rid = m.group(1)
                    self.routing_table.add(rid)
                else:
                    # End routing section when a line is encountered that does not match.
                    if line == "" or not line.startswith("-"):
                        routing_section = False

    def send_command(self, command_str):
        """Sends a command (with a newline) to the node's stdin."""
        if self.process.poll() is None:  # process is still running
            try:
                self.process.stdin.write(command_str + "\n")
                self.process.stdin.flush()
            except Exception as e:
                print(f"Error sending command to {self.name}: {e}")

    def terminate(self):
        """Terminate the process."""
        if self.process.poll() is None:
            print(f"Terminating {self.name}...")
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()

# --- Functions to generate configuration files in a temporary folder ---

def generate_beacon_config(tmp_dir):
    """Generate the configuration file for the beacon node."""
    os.makedirs(tmp_dir, exist_ok=True)
    config_beacon = f"""\
node_port: 8081
cache_file_path: cache.json
storage_path: ./storage-beacon
ip_address_type: "loopback"  # "public" or "loopback" or "local"
"""
    beacon_config_path = os.path.join(tmp_dir, "config_beacon.yaml")
    with open(beacon_config_path, "w") as f:
        f.write(config_beacon)
    print(f"Generated beacon configuration file in '{tmp_dir}': {beacon_config_path}")
    return beacon_config_path

def generate_node_config(tmp_dir, beacon_node_key):
    """Generate the configuration file for simple nodes with the beacon node key set."""
    os.makedirs(tmp_dir, exist_ok=True)
    config_node = f"""\
beacon_node_address: "127.0.0.1:8081"
cache_file_path: cache.json
storage_path: ./storage-beacon
ip_address_type: "local"
beacon_node_key: "{beacon_node_key}"
"""
    node_config_path = os.path.join(tmp_dir, "config.yaml")
    with open(node_config_path, "w") as f:
        f.write(config_node)
    print(f"Generated non-beacon node configuration file in '{tmp_dir}': {node_config_path}")
    return node_config_path

# --- Main function ---
def main():
    nodes = []
    tmp_dir = "playground-tmp"
    try:
        # Step 1: Generate beacon configuration file.
        beacon_config_path = generate_beacon_config(tmp_dir)

        # Step 2: Launch the beacon node.
        beacon_cmd = ["./target/debug/p2p", "--skip-join", "--config", f"./{beacon_config_path}"]
        beacon_node = NodeProcess("Beacon", beacon_cmd)
        nodes.append(beacon_node)
        print("Launched Beacon node.")
        
        # Wait until the beacon node produces its hash.
        timeout = 10  # seconds
        elapsed = 0
        while beacon_node.node_id is None and elapsed < timeout:
            time.sleep(0.5)
            elapsed += 0.5
        if beacon_node.node_id is None:
            print("Beacon node did not produce a node id within the timeout period. Exiting.")
            return
        beacon_hash = beacon_node.node_id
        print(f"Beacon node hash obtained: {beacon_hash}")

        # Step 3: Generate non-beacon node configuration file with beacon_node_key set.
        node_config_path = generate_node_config(tmp_dir, beacon_hash)

        # Step 4: Launch non-beacon node(s).
        node2_cmd = ["./target/debug/p2p", "--config", f"./{node_config_path}", "--port", "8082"]
        node2 = NodeProcess("Node2", node2_cmd)
        nodes.append(node2)
        print("Launched Node2 on port 8082.")
        time.sleep(2)

        node3_cmd = ["./target/debug/p2p", "--config", f"./{node_config_path}", "--port", "8083"]
        node3 = NodeProcess("Node3", node3_cmd)
        nodes.append(node3)
        print("Launched Node3 on port 8083.")
        time.sleep(2)
        
        time.sleep(5)

        # Step 5: Request routing tables from each node.
        print("\nRequesting routing tables from each node...\n")
        for node in nodes:
            node.send_command("dump_rt")
        time.sleep(3)  # wait for the output to be captured

        G = nx.Graph()
        for node in nodes:
            label = node.node_id if node.node_id else node.name
            G.add_node(label, node_name=node.name)
        for node in nodes:
            src = node.node_id if node.node_id else node.name
            for target in node.routing_table:
                G.add_node(target)
                G.add_edge(src, target)

        # Draw the graph.
        pos = nx.spring_layout(G)
        plt.figure(figsize=(8, 6))
        nx.draw_networkx_nodes(G, pos, node_color="lightblue", node_size=1500)
        nx.draw_networkx_edges(G, pos, edge_color="gray")
        nx.draw_networkx_labels(G, pos, font_size=10)
        plt.title("P2P Network Topology")
        plt.axis("off")
        plt.show()

    finally:
        print("\nShutting down all node processes...")
        for node in nodes:
            node.terminate()

        if os.path.exists(tmp_dir):
            try:
                shutil.rmtree(tmp_dir)
                print(f"Removed temporary folder '{tmp_dir}' and all its contents.")
            except Exception as e:
                print(f"Error removing temporary folder '{tmp_dir}': {e}")

if __name__ == "__main__":
    main()
