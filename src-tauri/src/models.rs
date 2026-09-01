use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a detected map in the ECU file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedMap {
    pub id: String,
    pub name: Option<String>,
    pub address: u32,
    pub size: usize,
    pub dimensions: MapDimensions,
    pub data_type: DataType,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub confidence: f32,
    // New fields for axis addresses and correction factors
    pub x_axis_address: Option<u32>,
    pub y_axis_address: Option<u32>,
    pub correction_factor: Option<f64>,
    pub offset: Option<f64>,
    pub x_axis_correction: Option<f64>,
    pub y_axis_correction: Option<f64>,
    pub x_axis_offset: Option<f64>,
    pub y_axis_offset: Option<f64>,
    // Axis labels (units) for display
    pub x_label: Option<String>,
    pub y_label: Option<String>,
    // Display inversion flags
    pub y_axis_inverted: Option<bool>,
    // Endianness flag (default is Big-Endian, set to true for Little-Endian maps)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_little_endian: Option<bool>,
    // Codeblock information for EDC15
    pub codeblock_id: Option<u32>,
    pub codeblock_start_address: Option<u32>,
    pub codeblock_end_address: Option<u32>,
    // Classification fields (from zededc15pfile.cs NameKnownMaps)
    pub category: Option<String>,
    pub subcategory: Option<String>,
    // MapSelector for maps with multiple variations (SOI maps, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map_selector: Option<MapSelectorInfo>,
}

/// Information about a MapSelector attached to a map
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapSelectorInfo {
    pub num_repeats: usize,
    pub selector_address: u32,
    pub map_data: Vec<u16>,
    pub map_indexes: Vec<u16>,
}

/// Map dimensions (1D, 2D, or 3D)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MapDimensions {
    OneDimensional { length: usize },
    TwoDimensional { rows: usize, cols: usize },
    ThreeDimensional { x: usize, y: usize, z: usize },
}

/// Data type of the map values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    UInt8,
    UInt16,
    UInt32,
    Int8,
    Int16,
    Int32,
    Float32,
}

/// Standard map categories for EDC16/EDC15 ECUs
/// Based on WinOLS/EcuSuite standard classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MapCategory {
    /// Air control (MAF, EGR, swirl flaps)
    AirControl,
    /// Engine torque limiters
    EngineTorqueLimiters,
    /// Engine torque request (driver wish, pedal maps)
    EngineTorqueRequest,
    /// Exhaust gas temperature protection
    ExhaustGasTemperature,
    /// Gearbox/transmission torque limiter
    GearboxTorqueLimiter,
    /// Idle speed control
    IdleSpeedRpm,
    /// Injection system (SOI, duration, rail pressure)
    InjectionSystem,
    /// Maximum RPM limiter
    MaximumRpmLimiter,
    /// Smoke limitation (opacity limiters)
    SmokeLimitation,
    /// Start of injection timing
    StartOfInjection,
    /// Turbo boost pressure (target, limiter)
    TurboBoostPressure,
    /// Turbo boost pressure control (N75 duty cycle)
    TurboBoostPressureControl,
    /// Vehicle speed limiters
    VehicleSpeedLimiters,
    /// Fuel quantity/IQ maps
    FuelQuantity,
    /// EGR (Exhaust Gas Recirculation)
    Egr,
    /// Torque to IQ conversion
    TorqueToIqConversion,
    /// Calibration/sensor maps
    Calibrations,
    /// Uncategorized maps
    Other,
}

impl MapCategory {
    /// Returns the display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            MapCategory::AirControl => "Air control",
            MapCategory::EngineTorqueLimiters => "Engine torque limiters",
            MapCategory::EngineTorqueRequest => "Engine torque request",
            MapCategory::ExhaustGasTemperature => "Exhaust gas temperature EGT",
            MapCategory::GearboxTorqueLimiter => "Gearbox torque limiter",
            MapCategory::IdleSpeedRpm => "Idle speed RPM",
            MapCategory::InjectionSystem => "Injection system",
            MapCategory::MaximumRpmLimiter => "Maximum RPM limiter",
            MapCategory::SmokeLimitation => "Smoke limitation",
            MapCategory::StartOfInjection => "Start of injection SOI",
            MapCategory::TurboBoostPressure => "Turbo boost pressure",
            MapCategory::TurboBoostPressureControl => "Turbo boost pressure control",
            MapCategory::VehicleSpeedLimiters => "Vehicle speed limiters",
            MapCategory::FuelQuantity => "Fuel quantity",
            MapCategory::Egr => "EGR",
            MapCategory::TorqueToIqConversion => "Torque to IQ Conversion",
            MapCategory::Calibrations => "Calibrations",
            MapCategory::Other => "Other",
        }
    }
}

/// Statut d'une famille de maps ATTENDUE dans le fichier (rapport de
/// complétude EDC16) : « Torque Limiter : 1 attendu, 1 trouvé ». Sert au
/// pourcentage de confiance du mappack affiché dans l'éditeur.
#[derive(Debug, Serialize)]
pub struct ExpectedMapStatus {
    pub label: String,
    pub expected: usize,
    pub found: usize,
}

/// Response containing detected maps (shape kept identical to the old
/// map-detector HTTP service so the frontend needs no changes)
#[derive(Debug, Serialize)]
pub struct DetectMapsResponse {
    pub success: bool,
    pub maps: Vec<DetectedMap>,
    pub total_maps: usize,
    pub processing_time_ms: u128,
    pub file_size: usize,
    /// Version du moteur de détection ayant produit ce résultat. Stockée avec
    /// le projet : à l'ouverture, un projet détecté par une version plus
    /// ancienne est re-scanné automatiquement (voir DETECTOR_VERSION).
    pub detector_version: u32,
    /// Rapport de complétude (EDC16 uniquement) : familles de maps qui
    /// doivent toujours exister et leur compte trouvé. None = famille ECU
    /// sans invariants définis (EDC15, MJD6…).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_maps: Option<Vec<ExpectedMapStatus>>,
}

impl DetectedMap {
    pub fn new(address: u32, size: usize, dimensions: MapDimensions, data_type: DataType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: None,
            address,
            size,
            dimensions,
            data_type,
            unit: None,
            description: None,
            confidence: 0.0,
            x_axis_address: None,
            y_axis_address: None,
            correction_factor: None,
            offset: None,
            x_axis_correction: None,
            y_axis_correction: None,
            x_axis_offset: None,
            y_axis_offset: None,
            x_label: None,
            y_label: None,
            y_axis_inverted: None,
            is_little_endian: None,
            codeblock_id: None,
            codeblock_start_address: None,
            codeblock_end_address: None,
            category: None,
            subcategory: None,
            map_selector: None,
        }
    }
}
