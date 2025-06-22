use crate::radar_simulator::{extract_client_portion, RadarSimulator, RadarSweep};
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::time::interval;

// Client connection manager
pub type ClientConnections = Arc<Mutex<HashMap<usize, TcpStream>>>;
pub type ReadyClients = Arc<Mutex<HashMap<usize, bool>>>; // Track which clients are ready for data

pub struct RadarTcpServer {
    pub ports: Vec<u16>,
    pub data_rate_hz: u64,
    pub client_counter: Arc<AtomicUsize>,
    pub clients: ClientConnections,
    pub ready_clients: ReadyClients,
}

impl RadarTcpServer {
    pub fn new(ports: Vec<u16>, data_rate_hz: u64) -> Self {
        Self {
            ports,
            data_rate_hz,
            client_counter: Arc::new(AtomicUsize::new(0)),
            clients: Arc::new(Mutex::new(HashMap::new())),
            ready_clients: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn start(&self) -> io::Result<()> {
        let mut tasks = vec![];

        // Start servers on each port
        for port in &self.ports {
            let counter = Arc::clone(&self.client_counter);
            let clients_clone = Arc::clone(&self.clients);
            let ready_clients_clone = Arc::clone(&self.ready_clients);
            let port = *port;
            let task = spawn(start_server_on_port(
                port,
                counter,
                clients_clone,
                ready_clients_clone,
            ));
            tasks.push(task);
        }

        // Start radar data broadcaster
        let clients_clone = Arc::clone(&self.clients);
        let ready_clients_clone = Arc::clone(&self.ready_clients);
        let data_rate = self.data_rate_hz;
        let _broadcaster_task = spawn(async move {
            radar_data_broadcaster(clients_clone, ready_clients_clone, data_rate).await;
            Ok::<(), io::Error>(())
        });

        println!("All servers started successfully!");
        println!("Connect clients to ports: {:?}", self.ports);
        println!("Radar data will be streamed after clients send 'SEND_DATA' command");

        // Wait for all tasks
        for task in tasks {
            if let Err(e) = task.await {
                eprintln!("Task failed: {}", e);
            }
        }

        Ok(())
    }
}

async fn start_server_on_port(
    port: u16,
    client_counter: Arc<AtomicUsize>,
    clients: ClientConnections,
    ready_clients: ReadyClients,
) -> io::Result<()> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;
    println!("TCP Server listening on port {}", port);

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                let client_id = client_counter.fetch_add(1, Ordering::SeqCst);
                println!(
                    "New connection from {} on port {} (Client ID: {})",
                    addr, port, client_id
                );

                // Initialize client as not ready
                {
                    let mut ready_map = ready_clients.lock().await;
                    ready_map.insert(client_id, false);
                }

                // Spawn a task to handle this client's commands
                let clients_clone = Arc::clone(&clients);
                let ready_clients_clone = Arc::clone(&ready_clients);
                spawn(handle_client_connection(
                    client_id,
                    socket,
                    clients_clone,
                    ready_clients_clone,
                ));

                println!(
                    "Client {} connected. Waiting for 'SEND_DATA' command...",
                    client_id
                );
            }
            Err(e) => {
                eprintln!("Failed to accept connection on port {}: {}", port, e);
            }
        }
    }
}

async fn handle_client_connection(
    client_id: usize,
    mut socket: TcpStream,
    clients: ClientConnections,
    ready_clients: ReadyClients,
) {
    let mut buffer = [0; 1024];

    loop {
        match socket.read(&mut buffer).await {
            Ok(0) => {
                // Connection closed
                println!("Client {} disconnected", client_id);

                // Remove from both maps
                {
                    let mut clients_map = clients.lock().await;
                    clients_map.remove(&client_id);
                }
                {
                    let mut ready_map = ready_clients.lock().await;
                    ready_map.remove(&client_id);
                }
                break;
            }
            Ok(n) => {
                let message = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
                println!("Received from client {}: '{}'", client_id, message);

                if message == "SEND_DATA" {
                    println!("Client {} requested data streaming", client_id);

                    // Mark client as ready and store the socket
                    {
                        let mut ready_map = ready_clients.lock().await;
                        ready_map.insert(client_id, true);
                    }
                    {
                        let mut clients_map = clients.lock().await;
                        clients_map.insert(client_id, socket);
                    }

                    println!("Client {} is now ready for data streaming", client_id);
                    break; // Exit the command reading loop
                } else if message == "STOP" {
                    println!("Client {} requested to stop data streaming", client_id);

                    // Mark client as not ready
                    {
                        let mut ready_map = ready_clients.lock().await;
                        ready_map.insert(client_id, false);
                    }

                    println!("Client {} stopped receiving data streaming", client_id);
                    // Continue listening for more commands
                }
            }
            Err(e) => {
                eprintln!("Error reading from client {}: {}", client_id, e);
                break;
            }
        }
    }
}

pub async fn radar_data_broadcaster(
    clients: ClientConnections,
    ready_clients: ReadyClients,
    data_rate_hz: u64,
) {
    let mut radar_sim = RadarSimulator::new();
    let mut interval = interval(Duration::from_millis(1000 / data_rate_hz));
    let mut last_ready_count = 0;

    println!("Starting radar data broadcast at {}Hz", data_rate_hz);
    println!("Real-world approach: ONE radar sweep split between clients");
    println!("Waiting for both clients to connect and send 'SEND_DATA' command...");

    loop {
        interval.tick().await;

        let clients_map = clients.lock().await;
        let ready_map = ready_clients.lock().await;

        // Count ready clients
        let current_ready_count = ready_map.values().filter(|&&ready| ready).count();

        // Check if ready client count changed
        if current_ready_count != last_ready_count {
            println!(
                "Ready client count changed: {} -> {}",
                last_ready_count, current_ready_count
            );

            // Reset sequence counter when both clients are ready for synchronization
            if current_ready_count == 2 && last_ready_count < 2 {
                radar_sim.reset_sequence();
                println!("🔄 Both clients ready! Resetting sequence counter for synchronization.");
            }

            last_ready_count = current_ready_count;
        }

        // Only broadcast when we have both clients ready for proper merging
        if current_ready_count < 2 {
            println!(
                "⏳ Waiting for both clients to be ready... ({}/2 ready)",
                current_ready_count
            );
            continue;
        }

        // Update target positions
        radar_sim.update_targets(1.0 / data_rate_hz as f32);
        radar_sim.current_time += (1000000 / data_rate_hz) as u64; // microseconds

        // Generate ONE complete radar sweep (this is what real radar produces)
        let complete_sweep = radar_sim.generate_complete_sweep();

        let mut disconnected_clients = Vec::new();
        let mut sent_count = 0;

        // Map ready clients to specific ports for consistent assignment
        let mut port_clients: HashMap<usize, usize> = HashMap::new(); // port_index -> client_id

        for (&client_id, &is_ready) in ready_map.iter() {
            if is_ready && sent_count < 2 && clients_map.contains_key(&client_id) {
                port_clients.insert(sent_count, client_id);
                sent_count += 1;
            }
        }

        drop(ready_map); // Release the lock early
        drop(clients_map); // Release the lock early

        // Send data to mapped ready clients
        for (port_index, &client_id) in port_clients.iter() {
            let mut clients_map = clients.lock().await;
            if let Some(stream) = clients_map.get_mut(&client_id) {
                // Extract client's portion from the SAME complete sweep
                let client_data = extract_client_portion(&complete_sweep, *port_index);

                match send_radar_data(stream, &client_data).await {
                    Ok(_) => {
                        println!(
                            "[{}] Sent sweep {} to Client {} (Port {}) (Az: {:.1}°-{:.1}°, {} targets)",
                            format!("{}", chrono::Local::now().format("%H:%M:%S%.3f")),
                            complete_sweep.sequence_id,
                            client_id,
                            if *port_index == 0 { 8080 } else { 8081 },
                            client_data.azimuth_start,
                            client_data.azimuth_end,
                            radar_sim.targets.len()
                        );
                    }
                    Err(e) => {
                        eprintln!("Failed to send data to client {}: {}", client_id, e);
                        disconnected_clients.push(client_id);
                    }
                }
            }
        }

        // Remove disconnected clients from both maps
        if !disconnected_clients.is_empty() {
            let mut clients_map = clients.lock().await;
            let mut ready_map = ready_clients.lock().await;
            for client_id in disconnected_clients {
                clients_map.remove(&client_id);
                ready_map.remove(&client_id);
                println!("Removed disconnected client {}", client_id);
            }
        }
    }
}

pub async fn send_radar_data(
    stream: &mut TcpStream,
    radar_sweep: &RadarSweep,
) -> Result<(), Box<dyn Error>> {
    let encoded_data = bincode::serialize(radar_sweep)?;

    // Send data size first, then the data
    stream.write_u64(encoded_data.len() as u64).await?;
    stream.write_all(&encoded_data).await?;
    stream.flush().await?;

    Ok(())
}
