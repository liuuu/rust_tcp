use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{self, Write};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Serialize, Deserialize, Debug, Clone)]
struct RadarSweep {
    timestamp: u64,
    sequence_id: u64,
    azimuth_start: f32,
    azimuth_end: f32,
    range_bins: Vec<f32>,
    data: Vec<Vec<f32>>,
    overlap_region: Vec<Vec<f32>>,
    client_id: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Demo Client - SEND_DATA Command Test");

    // Get port from user
    print!("Enter port (8080 or 8081): ");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let port: u16 = input.trim().parse().unwrap_or(8080);

    // Connect to server
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
    println!("Connected to server on port {}", port);

    // Wait for user input to send SEND_DATA command
    print!("Press Enter to send 'SEND_DATA' command...");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();

    // Send SEND_DATA command
    stream.write_all(b"SEND_DATA").await?;
    stream.flush().await?;
    println!("Sent 'SEND_DATA' command to server");

    // Now start receiving radar data
    println!("Starting to receive radar data...");
    let mut sweep_count = 0;

    loop {
        // Read data size
        let data_size = stream.read_u64().await?;

        // Read the serialized data
        let mut buffer = vec![0u8; data_size as usize];
        stream.read_exact(&mut buffer).await?;

        // Deserialize the radar sweep
        let sweep: RadarSweep = bincode::deserialize(&buffer)?;

        sweep_count += 1;
        println!(
            "[{}] Received sweep {} (Az: {:.1}°-{:.1}°, {} range bins, {} data points)",
            sweep_count,
            sweep.sequence_id,
            sweep.azimuth_start,
            sweep.azimuth_end,
            sweep.range_bins.len(),
            sweep.data.len()
        );

        // Stop after receiving 10 sweeps for demo
        if sweep_count >= 10 {
            println!("Demo complete - received {} sweeps", sweep_count);
            break;
        }
    }

    Ok(())
}
