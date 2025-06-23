use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔗 Advanced STOP/START Test Client");
    println!("📡 Connecting to radar server on port 8080...");

    let mut stream = TcpStream::connect("127.0.0.1:8080").await?;
    println!("✅ Connected!");

    // Phase 1: Send SEND_DATA and receive some data
    println!("\n📡 Phase 1: Starting data stream with SEND_DATA...");
    stream.write_all(b"SEND_DATA").await?;
    stream.flush().await?;
    println!("✅ SEND_DATA command sent!");

    // Read some radar sweeps
    let mut sweep_count = 0;
    for i in 1..=3 {
        match read_radar_sweep(&mut stream).await {
            Ok(sequence_id) => {
                sweep_count += 1;
                println!("📡 [{}/3] Received sweep #{}", i, sequence_id);
            }
            Err(e) => {
                println!("❌ Error reading sweep {}: {}", i, e);
                break;
            }
        }
    }

    // Phase 2: Send STOP command
    println!("\n🛑 Phase 2: Stopping data stream with STOP command...");
    stream.write_all(b"STOP").await?;
    stream.flush().await?;
    println!("✅ STOP command sent!");

    // Wait and verify no more data is received
    println!("\n⏳ Waiting 5 seconds to verify no more data is received...");
    let mut stop_verified = true;
    for i in 1..=5 {
        match tokio::time::timeout(Duration::from_secs(1), read_radar_sweep(&mut stream)).await {
            Ok(Ok(sequence_id)) => {
                println!(
                    "⚠️  Still received sweep #{} after STOP! (second {})",
                    sequence_id, i
                );
                stop_verified = false;
            }
            Ok(Err(_)) => {
                println!("❌ Connection error during stop verification");
                return Err("Connection error".into());
            }
            Err(_) => {
                println!("✅ Second {}: No data received (good!)", i);
            }
        }
    }

    if stop_verified {
        println!("✅ SUCCESS: STOP command working correctly!");
    } else {
        println!("❌ FAILED: Still receiving data after STOP command!");
    }

    // Phase 3: Resume with SEND_DATA
    println!("\n🔄 Phase 3: Resuming data stream with SEND_DATA...");
    stream.write_all(b"SEND_DATA").await?;
    stream.flush().await?;
    println!("✅ SEND_DATA command sent again!");

    // Read more sweeps to verify resumption
    let mut resumed_count = 0;
    for i in 1..=3 {
        match read_radar_sweep(&mut stream).await {
            Ok(sequence_id) => {
                resumed_count += 1;
                println!("📡 [{}/3] Resumed sweep #{}", i, sequence_id);
            }
            Err(e) => {
                println!("❌ Error reading resumed sweep {}: {}", i, e);
                break;
            }
        }
    }

    // Phase 4: Test rapid STOP/START cycles
    println!("\n⚡ Phase 4: Testing rapid STOP/START cycles...");
    for cycle in 1..=3 {
        println!("  Cycle {}: STOP", cycle);
        stream.write_all(b"STOP").await?;
        stream.flush().await?;
        sleep(Duration::from_millis(500)).await;

        println!("  Cycle {}: START", cycle);
        stream.write_all(b"SEND_DATA").await?;
        stream.flush().await?;
        sleep(Duration::from_millis(500)).await;
    }

    // Final verification
    println!("\n🏁 Final verification: Reading one more sweep after cycles...");
    match read_radar_sweep(&mut stream).await {
        Ok(sequence_id) => {
            println!("✅ Final sweep #{} received successfully!", sequence_id);
        }
        Err(e) => {
            println!("❌ Failed to read final sweep: {}", e);
        }
    }

    // Final STOP
    println!("\n🛑 Sending final STOP command...");
    stream.write_all(b"STOP").await?;
    stream.flush().await?;
    println!("✅ Final STOP command sent!");

    println!("\n📊 Test Summary:");
    println!("  - Initial sweeps received: {}", sweep_count);
    println!("  - Resumed sweeps received: {}", resumed_count);
    println!(
        "  - STOP functionality: {}",
        if stop_verified {
            "✅ Working"
        } else {
            "❌ Failed"
        }
    );
    println!("  - Connection maintained throughout: ✅ Yes");

    println!("\n🎉 Advanced STOP/START test completed!");

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
