use std::io;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::spawn;
use tokio::time::{sleep, Duration};

async fn handle_client(mut socket: TcpStream, port: u16) -> io::Result<()> {
    let mut buffer = [0; 1024];

    loop {
        match socket.read(&mut buffer).await {
            Ok(0) => {
                println!("Client disconnected from port {}", port);
                break;
            }
            Ok(n) => {
                let message = String::from_utf8_lossy(&buffer[..n]);
                let trimmed_message = message.trim();
                println!("Port {}: Received: {}", port, trimmed_message);

                // Check if this is ports 8082 or 8083 and if the message is 'SEND_DATA'
                if (port == 8082 || port == 8083) && trimmed_message == "SEND_DATA" {
                    println!("Port {}: Initiating data stream...", port);

                    // Send acknowledgment first
                    let ack_message = format!("Port {}: Starting data stream...\n", port);
                    if let Err(e) = socket.write_all(ack_message.as_bytes()).await {
                        eprintln!("Failed to send acknowledgment on port {}: {}", port, e);
                        break;
                    }

                    // Read and stream the data file
                    match fs::read_to_string("data.txt").await {
                        Ok(file_content) => {
                            for line in file_content.lines() {
                                let data_line = format!("Port {}: {}\n", port, line);
                                if let Err(e) = socket.write_all(data_line.as_bytes()).await {
                                    eprintln!("Failed to send data on port {}: {}", port, e);
                                    break;
                                }
                                // Add a small delay to simulate streaming
                                sleep(Duration::from_millis(100)).await;
                            }

                            // Send completion message
                            let completion_message =
                                format!("Port {}: Data stream completed.\n", port);
                            if let Err(e) = socket.write_all(completion_message.as_bytes()).await {
                                eprintln!(
                                    "Failed to send completion message on port {}: {}",
                                    port, e
                                );
                            }
                        }
                        Err(e) => {
                            let error_message =
                                format!("Port {}: Error reading data file: {}\n", port, e);
                            if let Err(e) = socket.write_all(error_message.as_bytes()).await {
                                eprintln!("Failed to send error message on port {}: {}", port, e);
                            }
                        }
                    }
                } else {
                    // Default echo behavior for other ports or messages
                    let response = format!("Echo from port {}: {}\n", port, trimmed_message);
                    if let Err(e) = socket.write_all(response.as_bytes()).await {
                        eprintln!("Failed to write to socket on port {}: {}", port, e);
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to read from socket on port {}: {}", port, e);
                break;
            }
        }
    }

    Ok(())
}

async fn start_server_on_port(port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    println!("TCP Server listening on port {}", port);

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                println!("New connection from {} on port {}", addr, port);

                // Spawn a new task for each client connection
                spawn(async move {
                    if let Err(e) = handle_client(socket, port).await {
                        eprintln!("Error handling client on port {}: {}", port, e);
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection on port {}: {}", port, e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("Starting multi-port TCP server...");

    // Define the ports you want to listen on
    let ports = vec![8080, 8081, 8082, 8083];

    // Create a vector to hold all the server tasks
    let mut tasks = vec![];

    // Start a server on each port
    for port in ports {
        let task = spawn(start_server_on_port(port));
        tasks.push(task);
    }

    println!("All servers started successfully!");
    println!("You can connect to any of the following ports: 8080, 8081, 8082, 8083");
    println!("Use telnet or nc to test: telnet 127.0.0.1 8080");
    println!("Special feature: Send 'SEND_DATA' to ports 8082 or 8083 to receive data stream!");

    // Wait for all tasks to complete (they run indefinitely)
    for task in tasks {
        if let Err(e) = task.await {
            eprintln!("Server task failed: {}", e);
        }
    }

    Ok(())
}
