// Bosch ECU detector modules
// Each folder contains detection logic for a specific Bosch ECU variant

pub mod launch_control; // Détection du Launch Control (EDC15P/EDC15VM)
pub mod egr_temperature; // Détection de l'EGR temperature map (EDC15P/EDC15VM)

pub mod edc15p;     // Bosch EDC15P (VAG 1.9 TDI PD)
pub mod edc15vm;    // Bosch EDC15VM (VM variant)
pub mod edc16u1;    // Bosch EDC16U1 (VAG TDI)
pub mod edc16u31;   // Bosch EDC16U31 (VAG 1.9/2.0/2.5 TDI PD - MPC563)
pub mod edc16u34;   // Bosch EDC16U34 (VAG 2.0 TDI CR)

// Re-export detectors with clear naming
pub use edc15p::EDC15PDetector;
pub use edc15vm::EDC15VMDetector;
pub use edc16u1::EDC16U1Detector;
pub use edc16u31::EDC16U31Detector;
// Note: EDC16U31 has its own EDC16Variant enum, accessed via edc16u31::EDC16Variant
pub use edc16u34::EDC16U34Detector;
