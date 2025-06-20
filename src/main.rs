use noise::{Fbm, NoiseFn, Perlin};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::spawn;
use tokio::sync::Mutex;
use tokio::time::interval;

// Radar system parameters
const RANGE_BINS: usize = 500; // 50km range, 100m resolution
const MAX_RANGE_KM: f32 = 50.0;
const RANGE_RESOLUTION_M: f32 = 100.0;
const DATA_RATE_HZ: u64 = 5; // 5Hz data rate
const OVERLAP_DEGREES: f32 = 20.0; // 20 degree overlap

// Enhanced radar data structure
#[derive(Serialize, Deserialize, Debug, Clone)]
struct RadarSweep {
    timestamp: u64,                // Microsecond timestamp
    sequence_id: u64,              // Frame sequence number
    azimuth_start: f32,            // Starting azimuth (degrees)
    azimuth_end: f32,              // Ending azimuth (degrees)
    range_bins: Vec<f32>,          // Range gate distances (km)
    data: Vec<Vec<f32>>,           // [azimuth][range] intensity values
    overlap_region: Vec<Vec<f32>>, // Overlap data for merging
    client_id: usize,              // Which client this data is for
}

// Simulated radar target
#[derive(Debug, Clone)]
struct RadarTarget {
    azimuth: f32,   // Current azimuth position
    range: f32,     // Range in km
    intensity: f32, // Radar cross section
    velocity: f32,  // Azimuth velocity (degrees/second)
    target_type: TargetType,
}

#[derive(Debug, Clone)]
enum TargetType {
    Aircraft,
    Weather,
    GroundClutter,
}

// Radar simulator that generates realistic data
struct RadarSimulator {
    current_time: u64,
    sequence_counter: u64,
    targets: Vec<RadarTarget>,
    noise_generator: Fbm<Perlin>,
    weather_intensity: f32,
}

impl RadarSimulator {
    fn new() -> Self {
        let mut targets = Vec::new();

        // Add some aircraft targets
        targets.push(RadarTarget {
            azimuth: 45.0,
            range: 15.0,
            intensity: 0.8,
            velocity: 2.0, // 2 degrees/second
            target_type: TargetType::Aircraft,
        });

        targets.push(RadarTarget {
            azimuth: 200.0,
            range: 25.0,
            intensity: 0.9,
            velocity: -1.5,
            target_type: TargetType::Aircraft,
        });

        // Add weather pattern
        targets.push(RadarTarget {
            azimuth: 120.0,
            range: 30.0,
            intensity: 0.6,
            velocity: 0.1,
            target_type: TargetType::Weather,
        });

        // Add ground clutter
        targets.push(RadarTarget {
            azimuth: 300.0,
            range: 3.0,
            intensity: 0.4,
            velocity: 0.0,
            target_type: TargetType::GroundClutter,
        });

        Self {
            current_time: 0,
            sequence_counter: 0,
            targets,
            noise_generator: Fbm::<Perlin>::new(42),
            weather_intensity: 0.3,
        }
    }

    fn update_targets(&mut self, dt: f32) {
        for target in &mut self.targets {
            target.azimuth += target.velocity * dt;
            target.azimuth = target.azimuth % 360.0;
            if target.azimuth < 0.0 {
                target.azimuth += 360.0;
            }
        }
    }

    // Generate ONE complete 360° radar sweep (real-world approach)
    fn generate_complete_sweep(&mut self) -> RadarSweep {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_micros() as u64;

        self.sequence_counter += 1;

        // Complete 360° sweep - this is what ONE radar antenna produces
        let azimuth_range = 360;
        let mut data = vec![vec![0.0; RANGE_BINS]; azimuth_range];

        // Generate range bins
        let range_bins: Vec<f32> = (0..RANGE_BINS)
            .map(|i| (i as f32) * RANGE_RESOLUTION_M / 1000.0)
            .collect();

        // Fill complete sweep with base noise level
        for az_idx in 0..azimuth_range {
            let azimuth = az_idx as f32;

            for range_idx in 0..RANGE_BINS {
                let range_km = range_bins[range_idx];

                // Base noise level with range attenuation
                let range_attenuation = 1.0 / (1.0 + range_km * 0.1);
                let noise_value = self.noise_generator.get([
                    azimuth as f64 * 0.1,
                    range_km as f64 * 0.2,
                    self.current_time as f64 * 0.001,
                ]);
                let base_intensity = (noise_value.abs() as f32) * 0.1 * range_attenuation;

                data[az_idx][range_idx] = base_intensity;
            }
        }

        // Add all targets to the complete sweep
        for target in &self.targets {
            let az_idx = (target.azimuth as usize) % 360;
            let range_idx =
                ((target.range / (RANGE_RESOLUTION_M / 1000.0)) as usize).min(RANGE_BINS - 1);

            // Add target with some spread
            for az_offset in -2..=2 {
                for range_offset in -3..=3 {
                    let target_az = ((az_idx as i32 + az_offset + 360) % 360) as usize;
                    let target_range = (range_idx as i32 + range_offset).max(0) as usize;

                    if target_range < RANGE_BINS {
                        let distance =
                            ((az_offset * az_offset + range_offset * range_offset) as f32).sqrt();
                        let intensity_factor = (-distance * 0.5).exp();

                        match target.target_type {
                            TargetType::Aircraft => {
                                data[target_az][target_range] +=
                                    target.intensity * intensity_factor;
                            }
                            TargetType::Weather => {
                                data[target_az][target_range] +=
                                    target.intensity * intensity_factor * self.weather_intensity;
                            }
                            TargetType::GroundClutter => {
                                if target.range < 5.0 {
                                    data[target_az][target_range] +=
                                        target.intensity * intensity_factor * 0.3;
                                }
                            }
                        }
                    }
                }
            }
        }

        RadarSweep {
            timestamp,
            sequence_id: self.sequence_counter,
            azimuth_start: 0.0,
            azimuth_end: 360.0,
            range_bins,
            data,
            overlap_region: vec![], // Will be filled when extracting client portions
            client_id: 999,         // Indicates complete sweep
        }
    }
}

// Extract portion of complete sweep for specific client (real-world data splitting)
fn extract_client_portion(complete_sweep: &RadarSweep, client_id: usize) -> RadarSweep {
    let (azimuth_start, azimuth_end) = match client_id {
        0 => (0.0, 190.0),   // Client 1: 0-190° with overlap
        1 => (170.0, 360.0), // Client 2: 170-360° with overlap
        _ => (0.0, 360.0),   // Fallback
    };

    let start_idx = azimuth_start as usize;
    let end_idx = azimuth_end as usize;

    // Extract data portion from complete sweep
    let mut client_data = Vec::new();
    if client_id == 0 {
        // Client 1: 0-190° (simple slice)
        client_data = complete_sweep.data[start_idx..end_idx].to_vec();
    } else if client_id == 1 {
        // Client 2: 170-360° (wrap around case)
        client_data.extend_from_slice(&complete_sweep.data[start_idx..360]);
        // Note: end_idx would be 360, so we don't need to add anything from the beginning
    }

    // Extract overlap region (170-190° for both clients) - same data for seamless merging
    let overlap_data = complete_sweep.data[170..190].to_vec();

    RadarSweep {
        timestamp: complete_sweep.timestamp, // Same timestamp - critical for merging
        sequence_id: complete_sweep.sequence_id, // Same sequence - critical for merging
        azimuth_start,
        azimuth_end,
        range_bins: complete_sweep.range_bins.clone(),
        data: client_data,
        overlap_region: overlap_data, // Same overlap data for both clients
        client_id,
    }
}

// Client connection manager
type ClientConnections = Arc<Mutex<HashMap<usize, TcpStream>>>;

async fn start_server_on_port(
    port: u16,
    client_counter: Arc<AtomicUsize>,
    clients: ClientConnections,
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

                // Store client connection
                {
                    let mut clients_map = clients.lock().await;
                    clients_map.insert(client_id, socket);
                }

                println!(
                    "Client {} connected and ready for data streaming",
                    client_id
                );
            }
            Err(e) => {
                eprintln!("Failed to accept connection on port {}: {}", port, e);
            }
        }
    }
}

async fn radar_data_broadcaster(clients: ClientConnections) {
    let mut radar_sim = RadarSimulator::new();
    let mut interval = interval(Duration::from_millis(1000 / DATA_RATE_HZ));

    println!("Starting radar data broadcast at {}Hz", DATA_RATE_HZ);
    println!("Real-world approach: ONE radar sweep split between clients");

    loop {
        interval.tick().await;

        // Update target positions
        radar_sim.update_targets(1.0 / DATA_RATE_HZ as f32);
        radar_sim.current_time += (1000000 / DATA_RATE_HZ) as u64; // microseconds

        // Generate ONE complete radar sweep (this is what real radar produces)
        let complete_sweep = radar_sim.generate_complete_sweep();

        let mut clients_map = clients.lock().await;
        let mut disconnected_clients = Vec::new();

        for (&client_id, stream) in clients_map.iter_mut() {
            // Only support first 2 clients for radar data
            if client_id >= 2 {
                continue;
            }

            // Extract client's portion from the SAME complete sweep
            let client_data = extract_client_portion(&complete_sweep, client_id);

            match send_radar_data(stream, &client_data).await {
                Ok(_) => {
                    println!(
                        "[{}] Sent sweep {} portion to Client {} (Az: {:.1}°-{:.1}°, {} total targets)",
                        chrono::Local::now().format("%H:%M:%S%.3f"),
                        complete_sweep.sequence_id,
                        client_id,
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

        // Remove disconnected clients
        for client_id in disconnected_clients {
            clients_map.remove(&client_id);
            println!("Removed disconnected client {}", client_id);
        }
    }
}

async fn send_radar_data(
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

#[tokio::main]
async fn main() -> io::Result<()> {
    println!("Starting Enhanced Radar Data Server...");
    println!("Parameters:");
    println!("  - Data Rate: {}Hz", DATA_RATE_HZ);
    println!("  - Azimuth Resolution: 1°");
    println!(
        "  - Range: {} km, {} m resolution",
        MAX_RANGE_KM, RANGE_RESOLUTION_M
    );
    println!("  - Client 1: 0-190° (overlap: 170-190°)");
    println!("  - Client 2: 170-360° (overlap: 170-190°)");
    println!("  - Overlap Region: {} degrees", OVERLAP_DEGREES);

    let ports = vec![8080, 8081];
    let mut tasks = vec![];
    let client_counter = Arc::new(AtomicUsize::new(0));
    let clients: ClientConnections = Arc::new(Mutex::new(HashMap::new()));

    // Start servers on each port
    for port in ports {
        let counter = Arc::clone(&client_counter);
        let clients_clone = Arc::clone(&clients);
        let task = spawn(start_server_on_port(port, counter, clients_clone));
        tasks.push(task);
    }

    // Start radar data broadcaster
    let clients_clone = Arc::clone(&clients);
    let _broadcaster_task = spawn(async move {
        radar_data_broadcaster(clients_clone).await;
        Ok::<(), io::Error>(())
    });

    println!("All servers started successfully!");
    println!("Connect clients to ports 8080 and 8081");
    println!(
        "Radar data will be continuously streamed at {}Hz",
        DATA_RATE_HZ
    );

    // Wait for all tasks
    for task in tasks {
        if let Err(e) = task.await {
            eprintln!("Task failed: {}", e);
        }
    }

    Ok(())
}
