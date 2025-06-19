# Multi-Port TCP Server in Rust

A simple TCP server implementation in Rust that can listen on multiple ports simultaneously using async/await with Tokio.

## Features

- Listens on multiple ports concurrently (default: 8080, 8081, 8082, 8083)
- Handles multiple client connections per port
- Echo server functionality - echoes back received messages
- **Data streaming feature**: Ports 8082 and 8083 respond to 'SEND_DATA' command with streaming data
- Async/await architecture using Tokio

## Prerequisites

- Rust (latest stable version)
- Cargo

## Installation & Running

1. Clone or navigate to this directory
2. Install dependencies and run:

```bash
cargo run
```

## Testing the Server

You can test the server using telnet or netcat (nc):

### Using telnet:

```bash
telnet 127.0.0.1 8080
telnet 127.0.0.1 8081
telnet 127.0.0.1 8082
telnet 127.0.0.1 8083
```

### Using netcat:

```bash
nc 127.0.0.1 8080
nc 127.0.0.1 8081
nc 127.0.0.1 8082
nc 127.0.0.1 8083
```

### Using curl for quick testing:

```bash
curl telnet://127.0.0.1:8080
```

## Special Commands

### Data Streaming (Ports 8082 and 8083 only)

Send the command `SEND_DATA` to ports 8082 or 8083 to receive a stream of sensor data:

```bash
# Connect to port 8082 or 8083
telnet 127.0.0.1 8082

# Then type:
SEND_DATA
```

This will trigger the server to stream data from the `data.txt` file line by line with a small delay between each line, simulating real-time data streaming.

## Customization

To change the ports the server listens on, modify the `ports` vector in `src/main.rs`:

```rust
let ports = vec![8080, 8081, 8082, 8083]; // Change these to your desired ports
```

## How it Works

1. The server creates multiple async tasks, one for each port
2. Each task runs an independent TCP listener
3. When a client connects, a new task is spawned to handle that specific connection
4. **Ports 8080 and 8081**: Echo back any message received from clients
5. **Ports 8082 and 8083**: Echo messages normally, but respond to 'SEND_DATA' with streaming data from `data.txt`
6. Multiple clients can connect to each port simultaneously

## Example Output

```
Starting multi-port TCP server...
TCP Server listening on port 8080
TCP Server listening on port 8081
TCP Server listening on port 8082
TCP Server listening on port 8083
All servers started successfully!
You can connect to any of the following ports: 8080, 8081, 8082, 8083
Use telnet or nc to test: telnet 127.0.0.1 8080
New connection from 127.0.0.1:54321 on port 8080
Port 8080: Received: Hello from client!
```
# rust_tcp
