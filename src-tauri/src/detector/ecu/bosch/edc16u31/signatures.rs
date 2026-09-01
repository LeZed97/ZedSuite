// EDC16U31 Map Signatures Database
// Generated from analysis of 20 mappack files covering various VAG TDI vehicles
// Architecture: Bosch EDC16U31, Freescale MPC5xx, BIG-ENDIAN
// Reference: edc16u31_detection_signatures.json

#![allow(dead_code)]

/// Map signature for EDC16 detection
#[derive(Debug, Clone)]
pub struct EDC16MapSignature {
    pub id_name: &'static str,
    pub display_name: &'static str,
    pub folder: &'static str,
    pub structure_type: StructureType,
    pub cols: DimensionRange,
    pub rows: DimensionRange,
    pub data_size_range: (usize, usize), // (min, max) bytes
    pub factor: f64,
    pub offset: f64,
    pub unit: &'static str,
    pub signed: bool,
    pub x_axis: AxisSignature,
    pub y_axis: AxisSignature,
    pub address_range: Option<(u32, u32)>, // (min, max) addresses where map is typically found
    pub occurrence_rate: f32,              // How often this map appears (0-1)
    pub priority: u8,                      // Detection priority (higher = check first)
}

/// Structure type for EDC16 maps
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StructureType {
    EZweiInv,   // 2D Map with inverted axes (most common)
    EZweidim,   // 2D Map standard
    EEindim,    // 1D Curve
    EEinzel,    // Single value (scalar)
}

/// Dimension range for flexible matching
#[derive(Debug, Clone)]
pub struct DimensionRange {
    pub typical: usize,
    pub min: usize,
    pub max: usize,
}

impl DimensionRange {
    pub const fn exact(val: usize) -> Self {
        Self { typical: val, min: val, max: val }
    }

    pub const fn range(min: usize, typical: usize, max: usize) -> Self {
        Self { typical, min, max }
    }

    pub fn matches(&self, val: usize) -> bool {
        val >= self.min && val <= self.max
    }
}

/// Axis signature for detection
#[derive(Debug, Clone)]
pub struct AxisSignature {
    pub present: bool,
    pub name: &'static str,
    pub unit: &'static str,
    pub factor: f64,
    pub typical_values: Option<&'static [u16]>,
    pub value_range: Option<(u16, u16)>, // (min, max) expected raw values
}

impl AxisSignature {
    pub const fn none() -> Self {
        Self {
            present: false,
            name: "-",
            unit: "-",
            factor: 1.0,
            typical_values: None,
            value_range: None,
        }
    }

    pub const fn rpm() -> Self {
        Self {
            present: true,
            name: "Engine Speed",
            unit: "rpm",
            factor: 1.0,
            typical_values: Some(&[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4500]),
            value_range: Some((400, 6000)),
        }
    }

    pub const fn pedal() -> Self {
        Self {
            present: true,
            name: "Accel Pedal",
            unit: "%",
            factor: 0.01,
            typical_values: Some(&[0, 1000, 2000, 3000, 4500, 6000, 8000, 10000]),
            value_range: Some((0, 10500)),
        }
    }

    pub const fn injection_qty() -> Self {
        Self {
            present: true,
            name: "Injection Qty",
            unit: "mg/stroke",
            factor: 0.01,
            typical_values: Some(&[0, 200, 400, 600, 800, 1000, 1500, 2000, 2500, 3000, 4000, 5000, 6000, 7000]),
            value_range: Some((0, 8000)),
        }
    }

    pub const fn air_mass() -> Self {
        Self {
            present: true,
            name: "Air Mass",
            unit: "mg/stroke",
            factor: 0.1,
            typical_values: None,
            value_range: Some((0, 20000)),
        }
    }

    pub const fn pressure_mbar() -> Self {
        Self {
            present: true,
            name: "Pressure",
            unit: "mbar",
            factor: 1.0,
            typical_values: None,
            value_range: Some((500, 4000)),
        }
    }

    pub const fn temperature() -> Self {
        Self {
            present: true,
            name: "Temperature",
            unit: "deg C",
            factor: 0.1,
            typical_values: None,
            value_range: Some((2000, 4500)), // Raw values: -40°C to 180°C with offset
        }
    }

    pub const fn torque() -> Self {
        Self {
            present: true,
            name: "Torque",
            unit: "Nm",
            factor: 0.1,
            typical_values: None,
            value_range: Some((0, 6000)),
        }
    }
}

/// Typical RPM axis values for EDC16
pub const TYPICAL_RPM_16: &[u16] = &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4500];
pub const TYPICAL_RPM_15: &[u16] = &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000];
pub const TYPICAL_RPM_14: &[u16] = &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500];

/// Typical pedal position axis values
pub const TYPICAL_PEDAL_8: &[u16] = &[0, 1000, 2000, 3000, 4500, 6000, 8000, 10000];

/// Typical injection quantity axis values
pub const TYPICAL_IQ_14: &[u16] = &[0, 200, 400, 600, 800, 1000, 1500, 2000, 2500, 3000, 4000, 5000, 6000, 7000];
pub const TYPICAL_IQ_10: &[u16] = &[0, 300, 600, 1000, 1500, 2000, 3000, 4000, 5000, 6000];

/// Known offset between consecutive maps of same type
pub const OFFSET_DRIVER_WISH: usize = 342;
pub const OFFSET_EGR_HYSTERESIS: usize = 82;
pub const OFFSET_SOI: usize = 448;

/// Characteristic factors that identify map types
pub const FACTOR_SOI: f64 = 0.023438;       // Start of Injection maps
pub const FACTOR_PILOT: f64 = 0.023427;     // Pilot injection maps
pub const FACTOR_IQ: f64 = 0.01;            // Injection quantity maps
pub const FACTOR_TORQUE: f64 = 0.1;         // Torque maps
pub const FACTOR_LAMBDA: f64 = 0.001;       // Lambda ratio maps
pub const FACTOR_SELECTOR: f64 = 0.003906;  // Map selector values

/// Load all EDC16U31 map signatures
pub fn load_edc16u31_signatures() -> Vec<EDC16MapSignature> {
    vec![
        // ===== DRIVER'S WISH MAPS (8x16) =====
        EDC16MapSignature {
            id_name: "AccPed_trqEng0Cold_MAP",
            display_name: "Driver Wish 01 (cold)",
            folder: "1-Drivers Wish",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(8),
            rows: DimensionRange::exact(16),
            data_size_range: (256, 256),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::pedal(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x186000, 0x1C3000)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "AccPed_trqEng0_MAP",
            display_name: "Driver Wish 02",
            folder: "1-Drivers Wish",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(8),
            rows: DimensionRange::exact(16),
            data_size_range: (256, 256),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::pedal(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x186000, 0x1C3200)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "AccPed_trqEng1_MAP",
            display_name: "Driver Wish 03",
            folder: "1-Drivers Wish",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(8),
            rows: DimensionRange::exact(16),
            data_size_range: (256, 256),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::pedal(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x186000, 0x1C3400)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "AccPed_trqEng2_MAP",
            display_name: "Driver Wish 04",
            folder: "1-Drivers Wish",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(8),
            rows: DimensionRange::exact(16),
            data_size_range: (256, 256),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::pedal(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x186000, 0x1C3600)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "AccPed_trqEng3_MAP",
            display_name: "Driver Wish 05",
            folder: "1-Drivers Wish",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(8),
            rows: DimensionRange::exact(16),
            data_size_range: (256, 256),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::pedal(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x186000, 0x1C3800)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "AccPed_trqEng4_MAP",
            display_name: "Driver Wish 06",
            folder: "1-Drivers Wish",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(8),
            rows: DimensionRange::exact(16),
            data_size_range: (256, 256),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::pedal(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x186000, 0x1C3A00)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "AccPed_trqEng5_MAP",
            display_name: "Driver Wish 07",
            folder: "1-Drivers Wish",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(8),
            rows: DimensionRange::exact(16),
            data_size_range: (256, 256),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::pedal(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x186000, 0x1C3C00)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "AccPed_trqEng6_MAP",
            display_name: "Driver Wish 08",
            folder: "1-Drivers Wish",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(8),
            rows: DimensionRange::exact(16),
            data_size_range: (256, 256),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::pedal(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x186000, 0x1C3E00)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "AccPed_trqEngLow_MAP",
            display_name: "Driver Wish AWD",
            folder: "1-Drivers Wish",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(8),
            rows: DimensionRange::exact(16),
            data_size_range: (256, 256),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::pedal(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x186000, 0x1C4000)),
            occurrence_rate: 1.0,
            priority: 90,
        },

        // ===== EGR MAPS (13x16 or 16x16) =====
        EDC16MapSignature {
            id_name: "AirCtl_mDesBase_MAP",
            display_name: "EGR",
            folder: "4-EGR",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::range(13, 13, 16),
            rows: DimensionRange::exact(16),
            data_size_range: (416, 512),
            factor: 0.1,
            offset: 0.0,
            unit: "mg/stroke air",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x187000, 0x1C7000)),
            occurrence_rate: 0.7,
            priority: 85,
        },
        EDC16MapSignature {
            id_name: "AirCtl_mDesBase0_MAP",
            display_name: "EGR (Gear 1,2,R)",
            folder: "4-EGR",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::range(13, 16, 16),
            rows: DimensionRange::exact(16),
            data_size_range: (416, 512),
            factor: 0.1,
            offset: 0.0,
            unit: "mg/stroke air",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x187000, 0x1C7000)),
            occurrence_rate: 1.0,
            priority: 85,
        },
        EDC16MapSignature {
            id_name: "AirCtl_mDesBase1_MAP",
            display_name: "EGR (Gear 3,4)",
            folder: "4-EGR",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::range(13, 16, 16),
            rows: DimensionRange::exact(16),
            data_size_range: (416, 512),
            factor: 0.1,
            offset: 0.0,
            unit: "mg/stroke air",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x187000, 0x1C7000)),
            occurrence_rate: 1.0,
            priority: 85,
        },
        EDC16MapSignature {
            id_name: "AirCtl_mDesBase2_MAP",
            display_name: "EGR (Gear 5,6)",
            folder: "4-EGR",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::range(13, 16, 16),
            rows: DimensionRange::exact(16),
            data_size_range: (416, 512),
            factor: 0.1,
            offset: 0.0,
            unit: "mg/stroke air",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x187000, 0x1C7000)),
            occurrence_rate: 1.0,
            priority: 85,
        },

        EDC16MapSignature {
            id_name: "FlMng_rLmbdSmkLim0_MAP",
            display_name: "Smoke Limiter by Lambda 1",
            folder: "3-Limiters",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(13),
            rows: DimensionRange::exact(16),
            data_size_range: (416, 416),
            factor: 0.001,
            offset: 0.0,
            unit: "-",
            signed: true,
            x_axis: AxisSignature::air_mass(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x18AA00, 0x1D6A00)),
            occurrence_rate: 1.0,
            priority: 85,
        },
        EDC16MapSignature {
            id_name: "FlMng_rLmbdSmkLim1_MAP",
            display_name: "Smoke Limiter by Lambda 2",
            folder: "3-Limiters",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(13),
            rows: DimensionRange::exact(16),
            data_size_range: (416, 416),
            factor: 0.001,
            offset: 0.0,
            unit: "-",
            signed: true,
            x_axis: AxisSignature::air_mass(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x18AC00, 0x1D6C00)),
            occurrence_rate: 1.0,
            priority: 85,
        },
        EDC16MapSignature {
            id_name: "FlMng_rLmbdSmkLim2_MAP",
            display_name: "Smoke Limiter by Lambda 3",
            folder: "3-Limiters",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(13),
            rows: DimensionRange::exact(16),
            data_size_range: (416, 416),
            factor: 0.001,
            offset: 0.0,
            unit: "-",
            signed: true,
            x_axis: AxisSignature::air_mass(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x18AE00, 0x1D6E00)),
            occurrence_rate: 1.0,
            priority: 85,
        },

        // ===== TORQUE LIMITER (20-22 x 3-4) =====
        EDC16MapSignature {
            id_name: "EngPrt_trqLimP_MAP",
            display_name: "Torque Limiter",
            folder: "3-Limiters",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::range(20, 21, 22),
            rows: DimensionRange::range(3, 4, 4),
            data_size_range: (120, 176),
            factor: 0.1,
            offset: 0.0,
            unit: "Nm",
            signed: true,
            x_axis: AxisSignature::rpm(),
            y_axis: AxisSignature::pressure_mbar(),
            address_range: Some((0x18A600, 0x1D4800)),
            occurrence_rate: 1.0,
            priority: 92,
        },

        // ===== TORQUE TO IQ CONVERSION (16-18 x 15-16) =====
        EDC16MapSignature {
            id_name: "FMTC_trq2qBas_MAP",
            display_name: "Torque to IQ Conversion",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::range(16, 18, 18),
            rows: DimensionRange::range(15, 16, 16),
            data_size_range: (480, 576),
            factor: 0.01,
            offset: 0.0,
            unit: "mg/stroke",
            signed: true,
            x_axis: AxisSignature::torque(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x1D3F00, 0x1D7300)),
            occurrence_rate: 1.0,
            priority: 95,
        },

        // ===== START OF INJECTION MAPS (14x16) =====
        EDC16MapSignature {
            id_name: "InjCrv_phiBas0_GMAP",
            display_name: "SOI 00",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(14),
            rows: DimensionRange::exact(16),
            data_size_range: (448, 448),
            factor: FACTOR_SOI,
            offset: 0.0,
            unit: "deg CrS",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x18AD00, 0x1DA000)),
            occurrence_rate: 1.0,
            priority: 95,
        },
        EDC16MapSignature {
            id_name: "InjCrv_phiBas1_GMAP",
            display_name: "SOI 01",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(14),
            rows: DimensionRange::exact(16),
            data_size_range: (448, 448),
            factor: FACTOR_SOI,
            offset: 0.0,
            unit: "deg CrS",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x18AE00, 0x1DA200)),
            occurrence_rate: 1.0,
            priority: 95,
        },
        // ... Additional SOI maps (phiBas2-9) follow same pattern

        // ===== BOOST PRESSURE MAPS (10x16) =====
        EDC16MapSignature {
            id_name: "PCR_pBDesBas_MAP",
            display_name: "Boost Target",
            folder: "2-Turbo",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(10),
            rows: DimensionRange::exact(16),
            data_size_range: (320, 320),
            factor: 1.0,
            offset: 0.0,
            unit: "mbar",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x18F600, 0x1EB200)),
            occurrence_rate: 1.0,
            priority: 92,
        },
        EDC16MapSignature {
            id_name: "PCR_pBDesBas2_MAP",
            display_name: "Boost Target 2",
            folder: "2-Turbo",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(10),
            rows: DimensionRange::range(15, 16, 16),
            data_size_range: (300, 320),
            factor: 1.0,
            offset: 0.0,
            unit: "mbar",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x18F400, 0x1EB000)),
            occurrence_rate: 1.0,
            priority: 92,
        },
        EDC16MapSignature {
            id_name: "PCR_pBDesMaxAP_MAP",
            display_name: "Boost Limiter (Altitude)",
            folder: "2-Turbo",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(10),
            rows: DimensionRange::range(10, 11, 11),
            data_size_range: (200, 220),
            factor: 1.0,
            offset: 0.0,
            unit: "mbar",
            signed: true,
            x_axis: AxisSignature::pressure_mbar(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x18F700, 0x1EB400)),
            occurrence_rate: 1.0,
            priority: 90,
        },

        // ===== BOOST LIMITER SCALAR =====
        EDC16MapSignature {
            id_name: "PCR_pBDesMax_C",
            display_name: "Max Boost Pressure",
            folder: "3-Limiters",
            structure_type: StructureType::EEinzel,
            cols: DimensionRange::exact(1),
            rows: DimensionRange::exact(1),
            data_size_range: (2, 2),
            factor: 1.0,
            offset: 0.0,
            unit: "mbar",
            signed: true,
            x_axis: AxisSignature::none(),
            y_axis: AxisSignature::none(),
            address_range: Some((0x18F800, 0x1EB400)),
            occurrence_rate: 1.0,
            priority: 95,
        },

        // ===== PILOT INJECTION DURATION MAPS =====
        EDC16MapSignature {
            id_name: "INJVLV_PHIINJMI1_MAP0",
            display_name: "Duration 00",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(10),
            rows: DimensionRange::exact(10),
            data_size_range: (200, 200),
            factor: FACTOR_PILOT,
            offset: 0.0,
            unit: "deg CrS",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x1DF000, 0x1E5000)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "INJVLV_PHIINJMI1_MAP1",
            display_name: "Duration 01",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(15),
            rows: DimensionRange::range(17, 19, 19),
            data_size_range: (510, 570),
            factor: FACTOR_PILOT,
            offset: 0.0,
            unit: "deg CrS",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x1DF100, 0x1E5100)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "INJVLV_PHIINJMI1_MAP2",
            display_name: "Duration 02",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(15),
            rows: DimensionRange::range(16, 19, 19),
            data_size_range: (480, 570),
            factor: FACTOR_PILOT,
            offset: 0.0,
            unit: "deg CrS",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x1DF400, 0x1E5300)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "INJVLV_PHIINJMI1_MAP3",
            display_name: "Duration 03",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(15),
            rows: DimensionRange::range(16, 19, 19),
            data_size_range: (480, 570),
            factor: FACTOR_PILOT,
            offset: 0.0,
            unit: "deg CrS",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x1DF600, 0x1E5600)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "INJVLV_PHIINJMI1_MAP4",
            display_name: "Duration 04",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(15),
            rows: DimensionRange::range(16, 19, 19),
            data_size_range: (480, 570),
            factor: FACTOR_PILOT,
            offset: 0.0,
            unit: "deg CrS",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x1DF900, 0x1E5900)),
            occurrence_rate: 1.0,
            priority: 90,
        },
        EDC16MapSignature {
            id_name: "INJVLV_PHIINJMI1_MAP5",
            display_name: "Duration 05",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::range(9, 10, 10),
            rows: DimensionRange::range(9, 10, 10),
            data_size_range: (162, 200),
            factor: FACTOR_PILOT,
            offset: 0.0,
            unit: "deg CrS",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x1DFB00, 0x1E5B00)),
            occurrence_rate: 1.0,
            priority: 90,
        },

        // ===== EGR HYSTERESIS CURVES (1D - 20x1) =====
        EDC16MapSignature {
            id_name: "AirCtl_qHigh1_CUR",
            display_name: "EGR Hysteresis 1",
            folder: "4-EGR",
            structure_type: StructureType::EEindim,
            cols: DimensionRange::exact(20),
            rows: DimensionRange::exact(1),
            data_size_range: (40, 40),
            factor: 0.01,
            offset: 0.0,
            unit: "mg/stroke",
            signed: true,
            x_axis: AxisSignature::rpm(),
            y_axis: AxisSignature::none(),
            address_range: Some((0x188500, 0x1CA100)),
            occurrence_rate: 1.0,
            priority: 75,
        },
        EDC16MapSignature {
            id_name: "AirCtl_qHigh_CUR",
            display_name: "EGR Hysteresis 2",
            folder: "4-EGR",
            structure_type: StructureType::EEindim,
            cols: DimensionRange::exact(20),
            rows: DimensionRange::exact(1),
            data_size_range: (40, 40),
            factor: 0.01,
            offset: 0.0,
            unit: "mg/stroke",
            signed: true,
            x_axis: AxisSignature::rpm(),
            y_axis: AxisSignature::none(),
            address_range: Some((0x188600, 0x1CA100)),
            occurrence_rate: 0.85,
            priority: 75,
        },
        EDC16MapSignature {
            id_name: "AirCtl_qLow_CUR",
            display_name: "EGR Hysteresis 3",
            folder: "4-EGR",
            structure_type: StructureType::EEindim,
            cols: DimensionRange::exact(20),
            rows: DimensionRange::exact(1),
            data_size_range: (40, 40),
            factor: 0.01,
            offset: 0.0,
            unit: "mg/stroke",
            signed: true,
            x_axis: AxisSignature::rpm(),
            y_axis: AxisSignature::none(),
            address_range: Some((0x188600, 0x1CA200)),
            occurrence_rate: 0.85,
            priority: 75,
        },

        // ===== DPF SWITCH (Scalar - 1 byte) =====
        EDC16MapSignature {
            id_name: "EGT_swtEGTActv_C",
            display_name: "DPF Switch",
            folder: "6-Switches",
            structure_type: StructureType::EEinzel,
            cols: DimensionRange::exact(1),
            rows: DimensionRange::exact(1),
            data_size_range: (1, 1),
            factor: 1.0,
            offset: 0.0,
            unit: "-",
            signed: false,
            x_axis: AxisSignature::none(),
            y_axis: AxisSignature::none(),
            address_range: Some((0x1D1200, 0x1D4300)),
            occurrence_rate: 0.95,
            priority: 80,
        },

        // ===== MAF LINEARIZATION (1D - 32x1) =====
        EDC16MapSignature {
            id_name: "AFSCD_dmLin_CUR",
            display_name: "MAF Linearization",
            folder: "7-Calibrations",
            structure_type: StructureType::EEindim,
            cols: DimensionRange::exact(32),
            rows: DimensionRange::exact(1),
            data_size_range: (64, 64),
            factor: 0.1,
            offset: 0.0,
            unit: "kg/h",
            signed: true,
            x_axis: AxisSignature {
                present: true,
                name: "MAF Voltage",
                unit: "mV",
                factor: 4.8876, // Raw value to mV conversion
                typical_values: None,
                value_range: Some((0, 1024)),
            },
            y_axis: AxisSignature::none(),
            address_range: Some((0x1C3200, 0x1C4000)),
            occurrence_rate: 0.5,
            priority: 70,
        },

        // ===== UNIVERSAL MAPS (100% occurrence) =====
        EDC16MapSignature {
            id_name: "InjVlv_numMI1_CUR",
            display_name: "Duration Selector",
            folder: "5-Injection",
            structure_type: StructureType::EEindim,
            cols: DimensionRange::exact(6),
            rows: DimensionRange::exact(1),
            data_size_range: (12, 12),
            factor: FACTOR_SELECTOR,
            offset: 0.0,
            unit: "-",
            signed: false,
            x_axis: AxisSignature {
                present: true,
                name: "Index",
                unit: "-",
                factor: FACTOR_PILOT,
                typical_values: None,
                value_range: Some((0, 10000)),
            },
            y_axis: AxisSignature::none(),
            address_range: Some((0x1DF000, 0x1E5000)),
            occurrence_rate: 1.0,
            priority: 85,
        },
        EDC16MapSignature {
            id_name: "InjVlv_tiMI1BreMin_MAP",
            display_name: "Min Injection Break",
            folder: "5-Injection",
            structure_type: StructureType::EZweiInv,
            cols: DimensionRange::exact(4),
            rows: DimensionRange::exact(6),
            data_size_range: (48, 48),
            factor: FACTOR_PILOT,
            offset: 0.0,
            unit: "deg CrS",
            signed: true,
            x_axis: AxisSignature::injection_qty(),
            y_axis: AxisSignature::rpm(),
            address_range: Some((0x1DFC00, 0x1E5E00)),
            occurrence_rate: 1.0,
            priority: 85,
        },
    ]
}

/// Map groups for validation - consecutive maps should be at known offsets
pub fn get_map_groups() -> std::collections::HashMap<&'static str, Vec<&'static str>> {
    let mut groups = std::collections::HashMap::new();

    groups.insert("drivers_wish", vec![
        "AccPed_trqEng0_MAP",
        "AccPed_trqEng1_MAP",
        "AccPed_trqEng2_MAP",
        "AccPed_trqEng3_MAP",
        "AccPed_trqEng4_MAP",
        "AccPed_trqEng5_MAP",
        "AccPed_trqEng6_MAP",
        "AccPed_trqEngLow_MAP",
        "AccPed_trqEng0Cold_MAP",
    ]);

    groups.insert("injection_timing", vec![
        "InjCrv_phiBas0_GMAP",
        "InjCrv_phiBas1_GMAP",
        "InjCrv_phiBas2_GMAP",
        "InjCrv_phiBas3_GMAP",
        "InjCrv_phiBas4_GMAP",
        "InjCrv_phiBas5_GMAP",
        "InjCrv_phiBas6_GMAP",
        "InjCrv_phiBas7_GMAP",
        "InjCrv_phiBas8_GMAP",
        "InjCrv_phiBas9_GMAP",
    ]);

    groups.insert("pilot_injection", vec![
        "INJVLV_PHIINJMI1_MAP0",
        "INJVLV_PHIINJMI1_MAP1",
        "INJVLV_PHIINJMI1_MAP2",
        "INJVLV_PHIINJMI1_MAP3",
        "INJVLV_PHIINJMI1_MAP4",
        "INJVLV_PHIINJMI1_MAP5",
        "INJVLV_PHIINJMI1_MAP6",
    ]);

    groups.insert("boost_pressure", vec![
        "PCR_pBDesBas_MAP",
        "PCR_pBDesBas2_MAP",
        "PCR_pBDesMaxAP_MAP",
        "PCR_pBDesMax_C",
    ]);

    groups.insert("boost_duty", vec![
        "PCR_rBPCtlBas_MAP",
        "PCR_rBPCtlBas2_MAP",
        "PCR_rBPCtlGear_MAP",
    ]);

    groups.insert("egr", vec![
        "AirCtl_mDesBase_MAP",
        "AirCtl_mDesBase0_MAP",
        "AirCtl_mDesBase1_MAP",
        "AirCtl_mDesBase2_MAP",
    ]);

    groups.insert("torque", vec![
        "EngPrt_trqLimP_MAP",
        "FMTC_trq2qBas_MAP",
    ]);

    groups
}
