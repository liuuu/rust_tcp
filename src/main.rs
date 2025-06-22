mod radar_simulator;
mod tcp_server;

use radar_simulator::{MAX_RANGE_KM, OVERLAP_DEGREES, RANGE_RESOLUTION_M};
use std::io;
use tcp_server::RadarTcpServer;

// Application-specific parameters
const DATA_RATE_HZ: u64 = 1; // 1Hz data rate

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
    let server = RadarTcpServer::new(ports, DATA_RATE_HZ);

    server.start().await
}
