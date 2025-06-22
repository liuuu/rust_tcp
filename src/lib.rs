pub mod radar_simulator;
pub mod tcp_server;

// Re-export commonly used types and functions for convenience
pub use radar_simulator::{
    RadarSweep, RadarTarget, RadarSimulator, TargetType,
    extract_client_portion, RANGE_BINS, MAX_RANGE_KM, RANGE_RESOLUTION_M, OVERLAP_DEGREES
};
pub use tcp_server::{
    RadarTcpServer, ClientConnections, radar_data_broadcaster, send_radar_data
};
