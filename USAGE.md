# Using the Radar Server Library

The radar server logic has been extracted into reusable modules. Here's how another project can use it:

## Module Structure

- `src/radar_simulator.rs` - Core radar simulation logic
- `src/tcp_server.rs` - TCP server and networking logic
- `src/lib.rs` - Library interface (optional)

## Using in Another Project

### Option 1: Copy the modules

Copy the module files to your project:

```
your_project/
├── src/
│   ├── radar_simulator.rs    # From rust_tcp_server
│   ├── tcp_server.rs         # From rust_tcp_server
│   └── main.rs
└── Cargo.toml
```

Then in your `main.rs`:

```rust
mod radar_simulator;
mod tcp_server;

use radar_simulator::{RadarSimulator, extract_client_portion};
use tcp_server::{RadarTcpServer, radar_data_broadcaster};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Custom configuration
    let ports = vec![9090, 9091];
    let data_rate = 5; // 5Hz instead of 1Hz

    let server = RadarTcpServer::new(ports, data_rate);
    server.start().await
}
```

### Option 2: Use as a dependency (if published)

Add to your `Cargo.toml`:

```toml
[dependencies]
rust_tcp_server = { path = "../rust_tcp" }
```

Then use it:

```rust
use rust_tcp_server::{RadarTcpServer, RadarSimulator};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let server = RadarTcpServer::new(vec![7000, 7001], 10);
    server.start().await
}
```

### Option 3: Custom TCP handler

You can use just the radar simulation without the TCP server:

```rust
mod radar_simulator;

use radar_simulator::{RadarSimulator, extract_client_portion};
use std::time::Duration;
use tokio::time::interval;

#[tokio::main]
async fn main() {
    let mut radar_sim = RadarSimulator::new();
    let mut interval = interval(Duration::from_millis(200)); // 5Hz

    loop {
        interval.tick().await;
        radar_sim.update_targets(0.2); // 200ms = 0.2s

        let complete_sweep = radar_sim.generate_complete_sweep();
        let client1_data = extract_client_portion(&complete_sweep, 0);
        let client2_data = extract_client_portion(&complete_sweep, 1);

        // Send via UDP, WebSocket, or any other protocol
        send_via_udp(&client1_data).await;
        send_via_websocket(&client2_data).await;
    }
}
```

## Key Features

- **Modular design**: Use radar simulation independent of TCP logic
- **Configurable data rate**: Easy to change from 1Hz to any frequency
- **Realistic simulation**: Weather targets with noise and range attenuation
- **Client synchronization**: Handles clients connecting at different times
- **Overlap handling**: Built-in support for merging overlapping radar data

## Dependencies Required

Add these to your `Cargo.toml`:

```toml
[dependencies]
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
noise = "0.8"
chrono = { version = "0.4", features = ["serde"] }
bincode = "1.3"
```
