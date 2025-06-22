use noise::{Fbm, NoiseFn, Perlin};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// Radar system parameters
pub const RANGE_BINS: usize = 500; // 50km range, 100m resolution
pub const MAX_RANGE_KM: f32 = 50.0;
pub const RANGE_RESOLUTION_M: f32 = 100.0;
pub const OVERLAP_DEGREES: f32 = 20.0; // 20 degree overlap

// Enhanced radar data structure
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RadarSweep {
    pub timestamp: u64,                // Microsecond timestamp
    pub sequence_id: u64,              // Frame sequence number
    pub azimuth_start: f32,            // Starting azimuth (degrees)
    pub azimuth_end: f32,              // Ending azimuth (degrees)
    pub range_bins: Vec<f32>,          // Range gate distances (km)
    pub data: Vec<Vec<f32>>,           // [azimuth][range] intensity values
    pub overlap_region: Vec<Vec<f32>>, // Overlap data for merging
    pub client_id: usize,              // Which client this data is for
}

// Simulated radar target
#[derive(Debug, Clone)]
pub struct RadarTarget {
    pub azimuth: f32,   // Current azimuth position
    pub range: f32,     // Range in km
    pub intensity: f32, // Radar cross section
    pub velocity: f32,  // Azimuth velocity (degrees/second)
    pub target_type: TargetType,
}

#[derive(Debug, Clone)]
pub enum TargetType {
    Aircraft,
    Weather,
    GroundClutter,
}

// Radar simulator that generates realistic data
pub struct RadarSimulator {
    pub current_time: u64,
    pub sequence_counter: u64,
    pub targets: Vec<RadarTarget>,
    noise_generator: Fbm<Perlin>,
    weather_intensity: f32,
}

impl RadarSimulator {
    pub fn new() -> Self {
        let mut targets = Vec::new();

        // Only weather patterns - remove aircraft and ground clutter
        targets.push(RadarTarget {
            azimuth: 45.0,
            range: 15.0,
            intensity: 0.6,
            velocity: 0.5, // Slow moving weather system
            target_type: TargetType::Weather,
        });

        targets.push(RadarTarget {
            azimuth: 120.0,
            range: 30.0,
            intensity: 0.8,
            velocity: 0.2,
            target_type: TargetType::Weather,
        });

        targets.push(RadarTarget {
            azimuth: 200.0,
            range: 25.0,
            intensity: 0.7,
            velocity: -0.3,
            target_type: TargetType::Weather,
        });

        // Add a larger weather system spanning multiple ranges
        targets.push(RadarTarget {
            azimuth: 280.0,
            range: 20.0,
            intensity: 0.9,
            velocity: 0.1,
            target_type: TargetType::Weather,
        });

        Self {
            current_time: 0,
            sequence_counter: 0,
            targets,
            noise_generator: Fbm::<Perlin>::new(42),
            weather_intensity: 0.4, // Increase weather intensity
        }
    }

    pub fn update_targets(&mut self, dt: f32) {
        for target in &mut self.targets {
            target.azimuth += target.velocity * dt;
            target.azimuth = target.azimuth % 360.0;
            if target.azimuth < 0.0 {
                target.azimuth += 360.0;
            }
        }
    }

    // Generate ONE complete 360° radar sweep (real-world approach)
    pub fn generate_complete_sweep(&mut self) -> RadarSweep {
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
                            TargetType::Weather => {
                                data[target_az][target_range] +=
                                    target.intensity * intensity_factor * self.weather_intensity;
                            }
                            _ => {} // Only process weather targets
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

    pub fn reset_sequence(&mut self) {
        self.sequence_counter = 0;
    }
}

// Extract portion of complete sweep for specific client (real-world data splitting)
pub fn extract_client_portion(complete_sweep: &RadarSweep, client_id: usize) -> RadarSweep {
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
