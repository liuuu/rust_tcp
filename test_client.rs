use serde::{Deserialize, Serialize};
use std::error::Error;
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
    // Connect to the server
    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("Connected to radar server on port 8080");

    // Receive radar data continuously
    loop {
        // Read the size of the incoming data
        let data_size = stream.read_u64().await?;

        // Read the serialized data
        let mut buffer = vec![0u8; data_size as usize];
        stream.read_exact(&mut buffer).await?;

        // Deserialize the radar sweep
        let radar_sweep: RadarSweep = bincode::deserialize(&buffer)?;

        println!(
            "Received sweep {} from server: Az {:.1}°-{:.1}°, {} azimuth bins, {} range bins, overlap: {} bins",
            radar_sweep.sequence_id,
            radar_sweep.azimuth_start,
            radar_sweep.azimuth_end,
            radar_sweep.data.len(),
            radar_sweep.data.get(0).map_or(0, |row| row.len()),
            radar_sweep.overlap_region.len()
        );

        // Print some sample data values
        if !radar_sweep.data.is_empty() && !radar_sweep.data[0].is_empty() {
            let sample_intensity = radar_sweep.data[0][0];
            println!("  Sample intensity at [0,0]: {:.6}", sample_intensity);
        }
    }
}
