use std::io::{self, Write};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Connecting to radar server on port 8080...");

    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("✅ Connected!");

    // Send SEND_DATA command to start receiving data
    println!("\n📡 Sending SEND_DATA command...");
    stream.write_all(b"SEND_DATA").await?;
    stream.flush().await?;
    println!("✅ SEND_DATA command sent!");

    // Read a few radar sweeps
    println!("\n📊 Reading radar data for 5 seconds...");
    let start_time = std::time::Instant::now();
    let mut sweep_count = 0;

    while start_time.elapsed() < Duration::from_secs(5) {
        match tokio::time::timeout(Duration::from_millis(500), read_radar_sweep(&mut stream)).await
        {
            Ok(Ok(sequence_id)) => {
                sweep_count += 1;
                println!(
                    "📡 Received sweep #{} (total: {})",
                    sequence_id, sweep_count
                );
            }
            Ok(Err(e)) => {
                println!("❌ Error reading sweep: {}", e);
                break;
            }
            Err(_) => {
                // Timeout - continue loop
            }
        }
    }

    println!("\n🛑 Sending STOP command...");
    stream.write_all(b"STOP").await?;
    stream.flush().await?;
    println!("✅ STOP command sent!");

    // Wait a bit and verify no more data is received
    println!("\n⏳ Waiting 3 seconds to verify no more data is received...");
    let stop_time = std::time::Instant::now();
    let mut data_received_after_stop = false;

    while stop_time.elapsed() < Duration::from_secs(3) {
        match tokio::time::timeout(Duration::from_millis(100), read_radar_sweep(&mut stream)).await
        {
            Ok(Ok(sequence_id)) => {
                println!("⚠️  Still received sweep #{} after STOP!", sequence_id);
                data_received_after_stop = true;
            }
            Ok(Err(_)) => break, // Connection closed or error
            Err(_) => {}         // Timeout - good, no data received
        }
    }

    if !data_received_after_stop {
        println!("✅ SUCCESS: No data received after STOP command!");
    } else {
        println!("❌ FAILED: Data was still received after STOP command!");
    }

    // Test resuming with SEND_DATA again
    println!("\n🔄 Testing resume with SEND_DATA command...");
    stream.write_all(b"SEND_DATA").await?;
    stream.flush().await?;
    println!("✅ SEND_DATA command sent again!");

    // Read a few more sweeps to verify resumption
    println!("\n📊 Reading resumed data for 2 seconds...");
    let resume_time = std::time::Instant::now();
    let mut resumed_sweep_count = 0;

    while resume_time.elapsed() < Duration::from_secs(2) {
        match tokio::time::timeout(Duration::from_millis(500), read_radar_sweep(&mut stream)).await
        {
            Ok(Ok(sequence_id)) => {
                resumed_sweep_count += 1;
                println!(
                    "📡 Resumed sweep #{} (count: {})",
                    sequence_id, resumed_sweep_count
                );
            }
            Ok(Err(e)) => {
                println!("❌ Error reading resumed sweep: {}", e);
                break;
            }
            Err(_) => {
                // Timeout - continue loop
            }
        }
    }

    if resumed_sweep_count > 0 {
        println!("✅ SUCCESS: Data streaming resumed after SEND_DATA!");
    } else {
        println!("❌ FAILED: No data received after resume SEND_DATA!");
    }

    println!("\n🏁 Test completed!");
    println!("📊 Total sweeps before STOP: {}", sweep_count);
    println!("📊 Total sweeps after resume: {}", resumed_sweep_count);

    Ok(())
}

async fn read_radar_sweep(stream: &mut TcpStream) -> Result<u64, Box<dyn std::error::Error>> {
    // Read data length first
    let data_len = stream.read_u64().await? as usize;

    // Read the serialized data
    let mut buffer = vec![0u8; data_len];
    stream.read_exact(&mut buffer).await?;

    // Deserialize to get sequence_id
    let radar_sweep: rust_tcp_server::RadarSweep = bincode::deserialize(&buffer)?;

    Ok(radar_sweep.sequence_id)
}
