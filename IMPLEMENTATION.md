# Real-World Radar Data Distribution Implementation

## Overview

This implementation mimics how real-world radar systems distribute data for parallel processing. Unlike the initial approach where each client received different independently generated data, this system follows the actual radar processing workflow.

## Real-World Architecture

```
[Radar Antenna] → [Complete 360° Sweep] → [Data Splitter] → [Client 1: 0-190°]
                                                          → [Client 2: 170-360°]
```

## Key Features

### 1. Single Source Truth

- **ONE** radar generates **ONE** complete 360° sweep per frame
- Same timestamp and sequence ID for all clients
- Identical targets and weather patterns across all data portions

### 2. Data Splitting (Not Generation)

```rust
// Generate ONE complete sweep
let complete_sweep = radar_sim.generate_complete_sweep();

// Split the SAME sweep between clients
let client1_data = extract_client_portion(&complete_sweep, 0); // 0-190°
let client2_data = extract_client_portion(&complete_sweep, 1); // 170-360°
```

### 3. Overlap Region for Merging

- Both clients receive the same overlap data (170-190°)
- Critical for seamless merging using sliding window algorithms
- Enables clients to verify data integrity and timing

## Data Structure

```rust
struct RadarSweep {
    timestamp: u64,              // Same for all clients - critical for sync
    sequence_id: u64,            // Same for all clients - frame ordering
    azimuth_start: f32,          // Client-specific coverage start
    azimuth_end: f32,            // Client-specific coverage end
    range_bins: Vec<f32>,        // Same range information
    data: Vec<Vec<f32>>,         // Client's portion of complete sweep
    overlap_region: Vec<Vec<f32>>, // Same overlap data for merging
    client_id: usize,            // Which client this portion is for
}
```

## Client Merging Strategy

### Phase 1: Double Buffering

```rust
struct DoubleBuffer {
    front_buffer: Vec<RadarSweep>,  // Currently receiving data
    back_buffer: Vec<RadarSweep>,   // Ready for processing
}
```

### Phase 2: Synchronization

```rust
// Wait for both clients' data with same sequence_id
fn wait_for_synchronized_data(client1_buffer: &[RadarSweep], client2_buffer: &[RadarSweep]) -> Option<(RadarSweep, RadarSweep)> {
    // Find matching sequence IDs
    // Verify timestamps are within acceptable delta
    // Return synchronized pair for merging
}
```

### Phase 3: Sliding Window Merging

```rust
fn merge_overlapping_data(client1: &RadarSweep, client2: &RadarSweep) -> CompleteRadarImage {
    // Both overlap_region arrays contain IDENTICAL data from 170-190°
    // Use this for seamless stitching

    let mut complete_image = Vec::new();

    // Add client 1 data: 0-170°
    complete_image.extend_from_slice(&client1.data[0..170]);

    // Add averaged overlap: 170-190° (both clients have same data, but average for robustness)
    let averaged_overlap = average_overlap(&client1.overlap_region, &client2.overlap_region);
    complete_image.extend(averaged_overlap);

    // Add client 2 data: 190-360°
    let client2_remaining = &client2.data[20..]; // Skip overlap portion
    complete_image.extend_from_slice(client2_remaining);

    complete_image
}
```

## Server Parameters

- **Data Rate**: 5Hz (200ms intervals)
- **Range**: 50km max, 100m resolution (500 bins)
- **Azimuth**: 1° resolution (360 bins total)
- **Overlap**: 20° (170-190°) for seamless merging
- **Targets**: Aircraft, weather, ground clutter with realistic movement

## Benefits of This Approach

1. **Temporal Consistency**: All clients process data from the same radar sweep moment
2. **Spatial Accuracy**: No artificial discontinuities between client regions
3. **Merge Quality**: Identical overlap regions enable perfect stitching
4. **Real-World Fidelity**: Matches actual radar processing pipeline
5. **Synchronization**: Built-in frame sequencing and timing

## Usage

1. **Start Server**: `cargo run`
2. **Connect Clients**: Port 8080 (Client 0), Port 8081 (Client 1)
3. **Receive Data**: Clients automatically receive 5Hz data stream
4. **Process**: Implement double buffering and sliding window merging on client side

This implementation provides the foundation for realistic radar data processing and PNG image generation with proper spatial and temporal coherence.
