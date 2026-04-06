# WhatSpace

WhatSpace is a minimal Delay-Tolerant Networking (DTN) middleware implemented in Rust.  
It simulates distributed DTN nodes based on the Store–Carry–Forward model.

The project focuses on modular system design, persistent storage, asynchronous networking, and distributed coordination between independent nodes.

---

## Overview

Each instance of the program represents an independent DTN node capable of:

- Generating and receiving bundles
- Persisting bundles locally
- Forwarding bundles opportunistically
- Handling TTL expiration
- Recovering from process restarts
- Operating under intermittent connectivity

Multiple nodes can be executed simultaneously in separate terminals or containers.

---

## Architecture

### Single Node Architecture

<img width="527" height="604" alt="Blank diagram (1)" src="https://github.com/user-attachments/assets/8ed35558-f036-4156-b2c5-db4fc6fe4f16" />

Each node is structured into the following modules:

- **CLI** : Handles user interaction and command execution.

- **Routing Engine** : Implements the Store–Carry–Forward logic and epidemic routing strategy.

- **Bundle Manager** : Manages bundle lifecycle and coordination between routing and storage.

- **Network Layer** : Handles TCP communication with peer nodes using asynchronous I/O.

- **Storage Layer** : Provides persistent local storage for bundles.

The routing engine interacts with:
- The network layer to send bundles.
- The storage layer to persist and retrieve bundles.
- The CLI to process user-initiated actions.

---

### Distributed Architecture

<img width="810" height="300" alt="Blank diagram (2)" src="https://github.com/user-attachments/assets/a27de688-217d-46df-b56f-47515d6e6101" />

Each node maintains:

- Its own process
- Its own local storage
- Its own routing logic

Nodes communicate exclusively via TCP connections.  
There is no shared database between nodes, preserving the distributed nature of the system.

---

## Features

### Bundle Management

Each bundle contains:

- Unique identifier
- Source node
- Destination node
- Timestamp
- TTL (Time To Live)
- Payload

Bundles are serialized using Serde and stored locally.

---

### Persistent Storage

- Local structured storage
- Duplicate detection
- Automatic removal of expired bundles
- State recovery after node restart

Each node maintains independent persistent storage.

---

### Network Communication

- TCP-based communication
- Static peer configuration at startup
- Asynchronous message handling using Tokio
- Periodic connection attempts
- Failure handling and retry logic

TCP is chosen to simplify reliability at the transport layer.

---

### Routing Logic

- Store–Carry–Forward mechanism
- Epidemic routing (simplified)
- Peer inventory synchronization
- Duplicate forwarding prevention
- Delivery confirmation handling

Bundles are forwarded opportunistically when peers become available.

---

### Command Line Interface

Available commands:

- `send` – create and send a bundle
- `list` – list locally stored bundles
- `peers` – display configured peers
- `status` – display node state

---

## Project Structure

The project follows a modular architecture where each feature is isolated in its own module.

```
Whatspace/
├── src/
│   ├── main.rs
│   ├── network/
│   │   ├── mod.rs
│   │   ├── bundle.rs
│   │   ├── server.rs
│   │   ├── client.rs
│   │   ├── bundle.proto
│   │   └── protobuf.rs
|   |
│   ├── storage/
│   │   ├── mod.rs
│   │   └── storage.rs
│   ├── routing/
│   │   ├── mod.rs
│   │   ├── ack.rs
│   │   ├── bundleManager.rs
│   │   ├── epidemic.rs
│   │   ├── model.rs
│   │   ├── scf.rs
│   │   └── engine.rs
|   |
│   └── cli/
│       ├── mod.rs
│       ├── handlers.rs
│       └── cli.rs
├── scripts/
│   └── test_ack_flow.sh
├── docs/
│   └── architecture.png
├── tests/
├── Cargo.toml
└── README.md
```
---

## Running the Project

### 1. Build

```bash
cargo build
```

### 2. Run the registry server

Start the registry server in a dedicated terminal:

```bash
cargo run -- serve
```

You can also use:

```bash
cargo run -- server
```

The registry server listens on `127.0.0.1:8080`.

### 3. Run the application in interactive mode

Open another terminal and start the application:

```bash
cargo run
```

This opens interactive mode with the predefined demo nodes:

- `alice` on `127.0.0.1:9001`
- `bob` on `127.0.0.1:9002`
- `carol` on `127.0.0.1:9003`

### 4. Run a node

Inside interactive mode, start a node and register it with the registry server:

```text
start alice --server 127.0.0.1:8080
```

You can start additional demo nodes the same way:

```text
start bob --server 127.0.0.1:8080
start carol --server 127.0.0.1:8080
```

Each started node opens its peer listener on its configured local port.

### 5. Available interactive commands

```text
all
start <name> --server 127.0.0.1:8080
stop <name>
status <name>
peers <name> list-peers
peers <name> get-connected-peers <uuid> [<uuid> ...]
peers <name> add <peer-name>
peers <name> remove <peer-name>
send --from <name> --to <name> --message "<message>" --ttl <seconds>
help
exit
```

### 6. Send a bundle

Example:

```text
send --from alice --to carol --message "hello from alice" --ttl 60
```
---

### 7. Test Script

Start the registry server in a dedicated terminal:

```bash
cargo run -- serve
```
Run the script:
```bash
./scripts/test_ack_flow.sh
```



## Future Work

- Temporal contact plan
- Bundle encryption
- Priority-based forwarding
- Containerized deployment
- Monitoring interface
- Performance optimization

---

## License

This project is licensed under the MIT License.

See the [LICENSE](LICENSE) file for details.
