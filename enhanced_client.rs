use colorgrad::Gradient;
use image::{ImageBuffer, Rgb, RgbImage};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::error::Error;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;
use tokio::spawn;
use tokio::time::{sleep, Duration, Instant};

// Import from the server modules
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

// Double buffering structure for efficient data handling
#[derive(Debug)]
struct DoubleBuffer {
    front_buffer: VecDeque<RadarSweep>,
    back_buffer: VecDeque<RadarSweep>,
    current_front: bool,
    max_buffer_size: usize,
}

impl DoubleBuffer {
    fn new(max_size: usize) -> Self {
        Self {
            front_buffer: VecDeque::new(),
            back_buffer: VecDeque::new(),
            current_front: true,
            max_buffer_size: max_size,
        }
    }

    fn add_sweep(&mut self, sweep: RadarSweep) {
        let buffer = if self.current_front {
            &mut self.front_buffer
        } else {
            &mut self.back_buffer
        };

        buffer.push_back(sweep);

        // Keep buffer size manageable
        if buffer.len() > self.max_buffer_size {
            buffer.pop_front();
        }
    }

    fn swap_buffers(&mut self) -> VecDeque<RadarSweep> {
        self.current_front = !self.current_front;

        if self.current_front {
            std::mem::take(&mut self.back_buffer)
        } else {
            std::mem::take(&mut self.front_buffer)
        }
    }

    fn front_buffer_size(&self) -> usize {
        if self.current_front {
            self.front_buffer.len()
        } else {
            self.back_buffer.len()
        }
    }
}

// Sliding window processor for data merging
struct SlidingWindowProcessor {
    window_size: usize,
    client1_data: VecDeque<RadarSweep>,
    client2_data: VecDeque<RadarSweep>,
    processed_frames: u64,
}

impl SlidingWindowProcessor {
    fn new(window_size: usize) -> Self {
        Self {
            window_size,
            client1_data: VecDeque::new(),
            client2_data: VecDeque::new(),
            processed_frames: 0,
        }
    }

    fn add_client_data(&mut self, client_id: usize, sweep: RadarSweep) {
        let buffer = match client_id {
            0 => &mut self.client1_data,
            1 => &mut self.client2_data,
            _ => return,
        };

        buffer.push_back(sweep);

        // Keep sliding window size
        if buffer.len() > self.window_size {
            buffer.pop_front();
        }
    }

    fn try_merge_next_frame(&mut self) -> Option<MergedRadarFrame> {
        // Find synchronized frames with matching sequence IDs
        if let Some((sweep1, sweep2)) = self.find_synchronized_pair() {
            self.processed_frames += 1;
            Some(self.merge_sweeps(sweep1, sweep2))
        } else {
            None
        }
    }

    fn find_synchronized_pair(&mut self) -> Option<(RadarSweep, RadarSweep)> {
        // Look for matching sequence IDs in both buffers
        for (i, sweep1) in self.client1_data.iter().enumerate() {
            for (j, sweep2) in self.client2_data.iter().enumerate() {
                if sweep1.sequence_id == sweep2.sequence_id {
                    // Check timestamp synchronization (within 100ms tolerance for late connections)
                    let time_diff = (sweep1.timestamp as i64 - sweep2.timestamp as i64).abs();
                    if time_diff < 100_000 {
                        // 100ms in microseconds
                        // Remove the synchronized sweeps from buffers safely
                        let (s1, s2) = if i > j {
                            let s1 = self.client1_data.remove(i).unwrap();
                            let s2 = self.client2_data.remove(j).unwrap();
                            (s1, s2)
                        } else {
                            let s2 = self.client2_data.remove(j).unwrap();
                            let s1 = self.client1_data.remove(i).unwrap();
                            (s1, s2)
                        };

                        println!(
                            "🔗 Found synchronized pair: seq {} (time diff: {}μs)",
                            s1.sequence_id, time_diff
                        );
                        return Some((s1, s2));
                    }
                }
            }
        }

        // If no synchronization found and buffers are getting full, warn user
        if self.client1_data.len() > 5 || self.client2_data.len() > 5 {
            println!(
                "⚠️  Large buffer sizes detected (C1: {}, C2: {}). Possible synchronization issue.",
                self.client1_data.len(),
                self.client2_data.len()
            );
        }

        None
    }

    fn merge_sweeps(&self, client1: RadarSweep, client2: RadarSweep) -> MergedRadarFrame {
        let mut complete_data = Vec::new();

        // Client 1: 0-170° (exclude overlap)
        let client1_main = &client1.data[0..170.min(client1.data.len())];
        complete_data.extend_from_slice(client1_main);

        // Overlap region: 170-190° (average both clients)
        let overlap_merged =
            self.merge_overlap_region(&client1.overlap_region, &client2.overlap_region);
        complete_data.extend(overlap_merged);

        // Client 2: 190-360° (skip overlap portion)
        if client2.data.len() > 20 {
            let client2_main = &client2.data[20..];
            complete_data.extend_from_slice(client2_main);
        }

        MergedRadarFrame {
            sequence_id: client1.sequence_id,
            timestamp: client1.timestamp,
            range_bins: client1.range_bins,
            complete_data,
            azimuth_resolution: 1.0, // 1 degree per bin
        }
    }

    fn merge_overlap_region(&self, overlap1: &[Vec<f32>], overlap2: &[Vec<f32>]) -> Vec<Vec<f32>> {
        let mut merged = Vec::new();
        let max_len = overlap1.len().max(overlap2.len());

        for i in 0..max_len {
            let row1 = overlap1.get(i);
            let row2 = overlap2.get(i);

            match (row1, row2) {
                (Some(r1), Some(r2)) => {
                    // Average the overlapping data
                    let averaged: Vec<f32> = r1
                        .iter()
                        .zip(r2.iter())
                        .map(|(a, b)| (a + b) / 2.0)
                        .collect();
                    merged.push(averaged);
                }
                (Some(r1), None) => merged.push(r1.clone()),
                (None, Some(r2)) => merged.push(r2.clone()),
                (None, None) => break,
            }
        }

        merged
    }
}

// Complete merged radar frame
#[derive(Debug, Clone)]
struct MergedRadarFrame {
    sequence_id: u64,
    timestamp: u64,
    range_bins: Vec<f32>,
    complete_data: Vec<Vec<f32>>, // [azimuth][range]
    azimuth_resolution: f32,
}

// Image processor for PNG generation
struct RadarImageProcessor {
    gradient: Gradient,
    value_range: (f32, f32),
    apply_log_scale: bool,
}

impl RadarImageProcessor {
    fn new() -> Self {
        // Create a radar-like color gradient (blue -> green -> yellow -> red)
        let gradient = colorgrad::turbo();

        Self {
            gradient,
            value_range: (0.0, 1.0),
            apply_log_scale: true,
        }
    }

    fn process_and_save(
        &self,
        frame: &MergedRadarFrame,
        filename: &str,
    ) -> Result<(), Box<dyn Error>> {
        let width = frame.complete_data.len() as u32;
        let height = frame.complete_data.get(0).map_or(0, |row| row.len()) as u32;

        if width == 0 || height == 0 {
            return Err("Invalid frame dimensions".into());
        }

        println!("Generating {}x{} radar image: {}", width, height, filename);

        // Create image buffer
        let mut img: RgbImage = ImageBuffer::new(width, height);

        // Find value range for normalization
        let (min_val, max_val) = self.find_value_range(frame);

        // Process each pixel
        for (x, azimuth_data) in frame.complete_data.iter().enumerate() {
            for (y, &intensity) in azimuth_data.iter().enumerate() {
                // Apply mathematical processing
                let processed_value = if self.apply_log_scale {
                    if intensity > 0.0 {
                        (intensity.abs().ln() + 1.0).max(0.0)
                    } else {
                        0.0
                    }
                } else {
                    intensity.abs()
                };

                // Normalize to 0-1 range
                let normalized = if max_val > min_val {
                    ((processed_value - min_val) / (max_val - min_val)).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                // Map to color
                let color = self.gradient.at(normalized as f64);
                let rgb = color.to_rgba8();

                // Set pixel (note: image coordinates vs array coordinates)
                if x < width as usize && y < height as usize {
                    img.put_pixel(x as u32, y as u32, Rgb([rgb[0], rgb[1], rgb[2]]));
                }
            }
        }

        // Save image
        img.save(filename)?;
        println!(
            "Saved radar image: {} (range: {:.6} - {:.6})",
            filename, min_val, max_val
        );

        Ok(())
    }

    fn find_value_range(&self, frame: &MergedRadarFrame) -> (f32, f32) {
        let mut min_val = f32::INFINITY;
        let mut max_val = f32::NEG_INFINITY;

        for azimuth_data in &frame.complete_data {
            for &intensity in azimuth_data {
                let processed = if self.apply_log_scale {
                    if intensity > 0.0 {
                        (intensity.abs().ln() + 1.0).max(0.0)
                    } else {
                        0.0
                    }
                } else {
                    intensity.abs()
                };

                min_val = min_val.min(processed);
                max_val = max_val.max(processed);
            }
        }

        (min_val, max_val)
    }
}

async fn receive_radar_data(
    port: u16,
    buffer: Arc<Mutex<DoubleBuffer>>,
) -> Result<(), Box<dyn Error>> {
    use tokio::io::AsyncWriteExt;

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port)).await?;
    println!("Connected to radar server on port {}", port);

    // Determine send delay based on port (0s for 8080, 10s for 8081)
    let send_delay = if port == 8080 {
        Duration::from_secs(0)
    } else {
        Duration::from_secs(5)
    };

    println!(
        "Will send 'SEND_DATA' command in {}s on port {}",
        send_delay.as_secs(),
        port
    );

    // Wait for the specified delay
    sleep(send_delay).await;

    // Send SEND_DATA command
    stream.write_all(b"SEND_DATA").await?;
    stream.flush().await?;
    println!("✅ Sent 'SEND_DATA' command to server on port {}", port);

    loop {
        // Read the size of the incoming data
        let data_size = stream.read_u64().await?;

        // Read the serialized data
        let mut data_buffer = vec![0u8; data_size as usize];
        stream.read_exact(&mut data_buffer).await?;

        // Deserialize the radar sweep
        let radar_sweep: RadarSweep = bincode::deserialize(&data_buffer)?;

        println!(
            "[Port {}] Received sweep {} (Client {}): Az {:.1}°-{:.1}°, {} azimuth bins, {} range bins",
            port,
            radar_sweep.sequence_id,
            radar_sweep.client_id,
            radar_sweep.azimuth_start,
            radar_sweep.azimuth_end,
            radar_sweep.data.len(),
            radar_sweep.data.get(0).map_or(0, |row| row.len())
        );

        // Add to double buffer
        {
            let mut buffer_guard = buffer.lock().unwrap();
            buffer_guard.add_sweep(radar_sweep);
        }
    }
}

async fn process_radar_data(
    client1_buffer: Arc<Mutex<DoubleBuffer>>,
    client2_buffer: Arc<Mutex<DoubleBuffer>>,
) -> Result<(), Box<dyn Error>> {
    let mut processor = SlidingWindowProcessor::new(10); // 10-frame sliding window
    let image_processor = RadarImageProcessor::new();
    let mut last_process_time = Instant::now();

    println!("Starting radar data processing with sliding window merging...");

    loop {
        sleep(Duration::from_millis(100)).await; // Check every 100ms

        // Swap buffers and get data for processing
        let client1_data = {
            let mut buffer = client1_buffer.lock().unwrap();
            if buffer.front_buffer_size() > 0 {
                buffer.swap_buffers()
            } else {
                VecDeque::new()
            }
        };

        let client2_data = {
            let mut buffer = client2_buffer.lock().unwrap();
            if buffer.front_buffer_size() > 0 {
                buffer.swap_buffers()
            } else {
                VecDeque::new()
            }
        };

        // Add data to sliding window processor
        for sweep in client1_data {
            processor.add_client_data(0, sweep);
        }

        for sweep in client2_data {
            processor.add_client_data(1, sweep);
        }

        // Try to merge and process frames
        while let Some(merged_frame) = processor.try_merge_next_frame() {
            println!(
                "Merged frame {} at timestamp {} (360° complete, {} range bins)",
                merged_frame.sequence_id,
                merged_frame.timestamp,
                merged_frame.range_bins.len()
            );

            // Generate PNG every frame since server runs at 1Hz now
            if merged_frame.sequence_id % 1 == 0 {
                let filename = format!("radar_frame_{:06}.png", merged_frame.sequence_id);

                let current_dir = std::env::current_dir();
                let save_path = current_dir
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
                    .join("radar_images")
                    .join(&filename);
                std::fs::create_dir_all(save_path.parent().unwrap())
                    .unwrap_or_else(|_| panic!("Failed to create directory for images"));

                if let Err(e) =
                    image_processor.process_and_save(&merged_frame, &save_path.to_string_lossy())
                {
                    eprintln!("Failed to save image {}: {}", filename, e);
                } else {
                    let elapsed = last_process_time.elapsed();
                    println!("✅ Generated {} (processing time: {:?})", filename, elapsed);
                    last_process_time = Instant::now();
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🎯 Enhanced Radar Client with Double Buffering & Sliding Window Merging");
    println!("📡 Connecting to radar data streams...");
    println!("⏰ Timing: Client 1 (8080) sends SEND_DATA at 0s, Client 2 (8081) at 10s");

    // Create double buffers for each client
    let client1_buffer = Arc::new(Mutex::new(DoubleBuffer::new(20)));
    let client2_buffer = Arc::new(Mutex::new(DoubleBuffer::new(20)));

    // Start data receivers for both clients
    let client1_buffer_clone = Arc::clone(&client1_buffer);
    let receiver1 = spawn(async move {
        if let Err(e) = receive_radar_data(8080, client1_buffer_clone).await {
            eprintln!("Client 1 receiver error: {}", e);
        }
    });

    let client2_buffer_clone = Arc::clone(&client2_buffer);
    let receiver2 = spawn(async move {
        if let Err(e) = receive_radar_data(8081, client2_buffer_clone).await {
            eprintln!("Client 2 receiver error: {}", e);
        }
    });

    // Start data processor
    let processor = spawn(async move {
        if let Err(e) = process_radar_data(client1_buffer, client2_buffer).await {
            eprintln!("Data processor error: {}", e);
        }
    });

    println!("🚀 All systems started!");
    println!("📊 Receiving radar data from ports 8080 and 8081");
    println!("🔄 Processing with sliding window algorithm");
    println!("🖼️  Generating PNG images every frame (1Hz rate)");
    println!("Press Ctrl+C to stop...");

    // Wait for all tasks
    tokio::try_join!(receiver1, receiver2, processor)?;

    Ok(())
}
