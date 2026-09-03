// EDC16U1 VAG ECU detector - Based on EDC16U34 algorithm
// Supports EDC16U1 variant only
// Based on Bosch EDC16 series (VAG TDI PD engines)
// Architecture: Freescale MPC5xx, FLASH 512KB-2MB, data in BIG-ENDIAN
//
// NOTE: This is a copy of EDC16U34 detector, will need adjustments for U1 specifics
// Detection Strategy:
// 1. Use flexible grid sizes per map type (e.g., Boost Limiter: 10x10, 10x11, 11x10)
// 2. Validate axes using RPM detection (values ~500-5000, delta ~250-500)
// 3. Validate Z data using physical ranges AFTER conversion
// 4. Multi-factor confidence scoring system
// 5. Support for tuned files with extended ranges

mod signatures;

use crate::models::{DetectedMap, MapDimensions, DataType, MapCategory};
use std::collections::HashSet;

// ============== MAP TEMPLATES FROM GROK ANALYSIS ==============
// Based on 10+ EDC16U1 mappack files (500+ maps analyzed)

/// Map type definition with flexible grid sizes and physical value ranges
#[derive(Debug, Clone)]
pub struct MapTemplate {
    pub name: &'static str,
    pub category: MapCategory,
    /// Accepted grid sizes as (rows, cols) tuples
    pub grids: &'static [(usize, usize)],
    /// Expected axes: (X axis type, Y axis type)
    pub axes: (AxisType, AxisType),
    /// Z factor for conversion (raw * factor = physical)
    pub z_factor: f64,
    /// Z value range AFTER conversion (min, max) for stock files
    pub z_range_stock: (f64, f64),
    /// Z value range AFTER conversion (min, max) for tuned files (+20-30%)
    pub z_range_tuned: (f64, f64),
    /// Expected axis range for "other" axis (not RPM)
    pub other_axis_range: (f64, f64),
    /// Unit for display
    pub unit: &'static str,
    /// Is signed data
    pub signed: bool,
    /// Maximum expected maps of this type
    pub max_count: usize,
    /// Priority (higher = detect first)
    pub priority: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisType {
    Rpm,                // Engine speed: 500-5000 rpm
    Pedal,              // Pedal position: 0-100%
    InjectionQty,       // IQ: 0-80 mg/stroke
    AtmosphericPressure, // Atmospheric: 800-1100 mbar
    BoostError,         // Boost error for N75
    Torque,             // Torque setpoint: 0-500 Nm
    AirMass,            // MAF: 200-1500 mg air
    RailPressure,       // Rail pressure: 300-1800 bar
    Temperature,        // Temperature
    None,               // No axis (scalar)
}

/// X axis type for detection (internal use)
#[derive(Debug, Clone, Copy, PartialEq)]
enum XAxisType {
    Pedal,              // 0-10000 (0-100%)
    InjectionQty,       // 0-7000 (0-70 mg)
    InjectionQtyHigh,   // 0-6500 (EGR range)
    Torque,             // 0-4000 (0-400 Nm)
    AirMass,            // 2000-15000 (200-1500 mg air)
    AtmosphericPressure, // 700-1100 mbar
    Unknown,
}

/// All EDC16 map templates based on Grok's analysis
pub const MAP_TEMPLATES: &[MapTemplate] = &[
    // ===== BOOST LIMITER (PCR_pBDesMaxAP_MAP) =====
    // Grilles: (10,10), (10,11), (11,10) - ~80% are 11x10
    MapTemplate {
        name: "Boost Limiter",
        category: MapCategory::TurboBoostPressure,
        grids: &[(11, 10), (10, 10), (10, 11)],
        axes: (AxisType::Rpm, AxisType::AtmosphericPressure),
        z_factor: 1.0,
        z_range_stock: (1100.0, 3500.0),   // mbar
        z_range_tuned: (1100.0, 4000.0),   // Extended for stage files
        other_axis_range: (800.0, 1100.0), // Atmospheric pressure mbar
        unit: "mbar",
        signed: true,
        max_count: 2,
        priority: 95,
    },
    // ===== BOOST MAP / TARGET (PCR_pBDesBas_MAP, PCR_pBDesBas2_MAP) =====
    // Grilles: (10,16), (16,10) - flexible rows/cols
    MapTemplate {
        name: "Boost Target",
        category: MapCategory::TurboBoostPressure,
        grids: &[(16, 10), (15, 10), (10, 16), (10, 15)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 1.0,
        z_range_stock: (1000.0, 2800.0),   // mbar
        z_range_tuned: (1000.0, 3500.0),   // Extended
        other_axis_range: (0.0, 80.0),     // IQ mg/stroke
        unit: "mbar",
        signed: true,
        max_count: 4,
        priority: 92,
    },
    // ===== TORQUE TO IQ CONVERSION (FMTC_trq2qBas_MAP) =====
    // Grilles: (15,16), (15,18), (16,15), (16,16) - ~60% are 15x16
    MapTemplate {
        name: "Torque to IQ Conversion",
        category: MapCategory::InjectionSystem,
        grids: &[(16, 16), (16, 18), (15, 16), (15, 18), (18, 16)],
        axes: (AxisType::Rpm, AxisType::Torque),
        z_factor: 0.01,
        z_range_stock: (0.0, 80.0),        // mg/stroke
        z_range_tuned: (0.0, 100.0),       // Extended for stage
        other_axis_range: (0.0, 500.0),    // Nm
        unit: "mg/stroke",
        signed: true,
        max_count: 1,  // Unique map
        priority: 98,
    },
    // ===== DRIVER'S WISH (AccPed_trqEng_MAP) =====
    // Grilles: (8,16), (16,8) - systematic
    MapTemplate {
        name: "Driver Wish",
        category: MapCategory::EngineTorqueRequest,
        grids: &[(16, 8), (8, 16)],
        axes: (AxisType::Rpm, AxisType::Pedal),
        z_factor: 0.1,
        z_range_stock: (0.0, 500.0),       // Nm (converted)
        z_range_tuned: (0.0, 600.0),       // Extended
        other_axis_range: (0.0, 100.0),    // Pedal %
        unit: "Nm",
        signed: true,
        max_count: 30,
        priority: 90,
    },
    // ===== START OF INJECTION (InjCrv_phiBas_GMAP) =====
    // Grilles: (14,16), (16,14)
    MapTemplate {
        name: "Start of Injection",
        category: MapCategory::StartOfInjection,
        grids: &[(16, 14), (14, 16)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.023437,
        z_range_stock: (-15.0, 35.0),      // deg crank (after conversion)
        z_range_tuned: (-20.0, 40.0),      // Extended
        other_axis_range: (0.0, 80.0),     // IQ mg
        unit: "deg CrS",
        signed: true,
        max_count: 35,
        priority: 94,
    },
    // ===== INJECTION DURATION (INJVLV_PHIINJMI1_MAP) =====
    // Grilles: (15,19), (19,15), (10,10)
    MapTemplate {
        name: "Injection Duration",
        category: MapCategory::InjectionSystem,
        grids: &[(10, 10), (19, 15), (15, 19), (17, 15), (15, 17), (16, 15), (15, 16)],
        axes: (AxisType::InjectionQty, AxisType::RailPressure),
        z_factor: 0.75,  // ~0.75 µs per raw unit
        z_range_stock: (300.0, 2200.0),    // µs
        z_range_tuned: (300.0, 2500.0),
        other_axis_range: (300.0, 1800.0), // bar rail pressure
        unit: "us",
        signed: true,
        max_count: 10,
        priority: 85,
    },
    // ===== EGR (AirCtl_mDesBase_MAP) =====
    // Grilles: (13,16), (16,13)
    MapTemplate {
        name: "EGR",
        category: MapCategory::Egr,
        grids: &[(16, 16), (16, 13), (13, 16)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.1,
        z_range_stock: (0.0, 100.0),       // % or mg/stroke air
        z_range_tuned: (0.0, 100.0),
        other_axis_range: (10.0, 60.0),    // IQ mg
        unit: "%",
        signed: false,
        max_count: 12,
        priority: 85,
    },
    // ===== N75 DUTY CYCLE (PCR_rBPCtlBas_MAP, PCR_rBPCtlBas2_MAP) =====
    // Based on actual analysis: 12x16, 13x16, 11x16, 10x16 grids
    // Stock values typically 45-95%, tuned files can go up to 110%
    // Note: "No gear" maps are often flat (all values = 75%), this is valid!
    MapTemplate {
        name: "N75 Duty Cycle",
        category: MapCategory::TurboBoostPressure,
        grids: &[(16, 12), (12, 16), (16, 13), (13, 16), (16, 11), (11, 16), (16, 10), (10, 16)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),  // IQ axis, not boost error!
        z_factor: 0.01,
        z_range_stock: (0.0, 96.0),        // % - Allow 0% for first column (low IQ), max 96%
        z_range_tuned: (0.0, 115.0),       // Extended for aggressive tuned files (up to 110%+)
        other_axis_range: (0.0, 70.0),     // IQ mg/stroke (0-70 mg typical)
        unit: "%",
        signed: true,  // Data is signed!
        max_count: 15, // EDC16 can have up to 12+ N75 maps (3 variants x 3-4 blocks)
        priority: 82,
    },
    // ===== TORQUE LIMITER (EngPrt_trqLimP_MAP) =====
    // Grilles: (22,4), (21,4), (20,4), (4,22)
    MapTemplate {
        name: "Torque Limiter",
        category: MapCategory::EngineTorqueLimiters,
        grids: &[(4, 22), (4, 21), (4, 20), (22, 4), (21, 4), (20, 4), (3, 22), (3, 21)],
        axes: (AxisType::Rpm, AxisType::AtmosphericPressure),
        z_factor: 0.1,
        z_range_stock: (150.0, 450.0),     // Nm (converted)
        z_range_tuned: (150.0, 550.0),     // Extended
        other_axis_range: (700.0, 1100.0), // Atmospheric mbar
        unit: "Nm",
        signed: true,
        max_count: 1,  // Unique
        priority: 92,
    },
    // ===== DURATION / INJECTION TIMING (10x10, 19x15) =====
    // Duration 00 is 10x10, Duration 01+ are 19x15 or similar
    MapTemplate {
        name: "Duration",
        category: MapCategory::InjectionSystem,
        grids: &[(10, 10), (15, 19), (19, 15), (15, 17), (17, 15), (15, 16), (16, 15)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.023427,  // deg CrS
        z_range_stock: (-20.0, 60.0),      // deg CrS typical
        z_range_tuned: (-30.0, 70.0),
        other_axis_range: (0.0, 80.0),     // IQ mg/stroke
        unit: "deg CrS",
        signed: true,
        max_count: 8,
        priority: 80,
    },
    // ===== LAMBDA CALCULATIONS (16x13) =====
    // Air/fuel ratio calculations based on air mass and RPM
    MapTemplate {
        name: "Lambda Calculations",
        category: MapCategory::Calibrations,
        grids: &[(16, 13), (13, 16)],
        axes: (AxisType::Rpm, AxisType::AirMass),
        z_factor: 0.001,  // Lambda ratio
        z_range_stock: (0.5, 2.0),         // Lambda typical range
        z_range_tuned: (0.4, 2.5),
        other_axis_range: (200.0, 1500.0), // mg air/stroke
        unit: "[-]",
        signed: true,
        max_count: 6,
        priority: 78,
    },
    // ===== FUEL VOLUME CORRECTION (9x8) =====
    MapTemplate {
        name: "Fuel Volume Correction",
        category: MapCategory::FuelQuantity,
        grids: &[(9, 8), (8, 9)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.01,
        z_range_stock: (0.8, 1.2),         // Correction factor
        z_range_tuned: (0.7, 1.3),
        other_axis_range: (0.0, 80.0),     // IQ
        unit: "[-]",
        signed: true,
        max_count: 2,
        priority: 70,
    },
    // ===== EXPECTED FUEL TEMP (8x8) =====
    MapTemplate {
        name: "Expected Fuel Temp",
        category: MapCategory::Calibrations,
        grids: &[(8, 8)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.1,
        z_range_stock: (-20.0, 100.0),     // Temperature degC
        z_range_tuned: (-30.0, 120.0),
        other_axis_range: (0.0, 80.0),     // IQ
        unit: "degC",
        signed: true,
        max_count: 2,
        priority: 68,
    },
];

// ============== HELPER CONSTANTS ==============
const MIN_DATA_SECTION: usize = 0x30000;  // Start scanning after this offset

/// Validates whether a 256-byte block at `addr` looks like a Driver Wish
/// 8×16 torque table: each row mostly non-decreasing (more pedal → more
/// torque), values within the raw 0-7000 range that maps to 0-700 Nm, and
/// the first row peaking higher than the last row (peak torque at low RPM,
/// reduced demand at high RPM). Helper for [`detect_driver_wish_maps`].
fn is_driver_wish_block(data: &[u8], addr: usize) -> bool {
    const ROWS: usize = 16;
    const COLS: usize = 8;
    if addr + ROWS * COLS * 2 > data.len() { return false; }

    let read = |r: usize, c: usize| -> i32 {
        let off = addr + (r * COLS + c) * 2;
        let raw = ((data[off] as u16) << 8) | data[off + 1] as u16;
        raw as i16 as i32
    };

    let mut max_row: [i32; ROWS] = [0; ROWS];
    let mut sum_row: [i64; ROWS] = [0; ROWS];
    let mut first_col: [i32; ROWS] = [0; ROWS];
    let mut last_col:  [i32; ROWS] = [0; ROWS];
    let mut mono_rows = 0;
    for r in 0..ROWS {
        let mut row_max = i32::MIN;
        let mut row_sum: i64 = 0;
        let mut bad = 0;
        let mut prev = read(r, 0);
        for c in 0..COLS {
            let v = read(r, c);
            if v < -500 || v > 8000 { return false; }
            if c > 0 && v < prev - 20 { bad += 1; }
            if v > row_max { row_max = v; }
            row_sum += v as i64;
            prev = v;
        }
        if bad <= 1 { mono_rows += 1; }
        max_row[r] = row_max;
        sum_row[r] = row_sum;
        first_col[r] = read(r, 0);
        last_col[r]  = read(r, COLS - 1);
    }

    // Need a clear majority of rows monotone-along-pedal.
    if mono_rows < 14 { return false; }

    // Peak (max) of the table should land in a plausible torque range.
    if max_row[0] < 1500 || max_row[0] > 7000 { return false; }

    // Driver Wish drops off with RPM: the average torque on row 0 (low RPM,
    // demand peaks) is meaningfully higher than on the last row (high RPM,
    // demand collapses). We compare sums instead of max because tuners often
    // clamp the rightmost column to a soft cap on every row, which leaves
    // max_row[0] == max_row[15] even on a legit Driver Wish.
    if sum_row[0] <= sum_row[ROWS - 1] + 2000 { return false; }

    // Anti-shift guard: on a real Driver Wish, each new row restarts from a
    // lower torque than the previous row ended at (the new row is at higher
    // RPM, so initial demand is lower). A block that "matches" because of a
    // byte misalignment tends to have first_col[r+1] >= last_col[r] instead.
    // Require this drop on a clear majority of row transitions.
    let mut drops = 0;
    for r in 1..ROWS {
        if first_col[r] < last_col[r - 1] { drops += 1; }
    }
    if drops < ROWS - 4 { return false; }

    true
}

/// Best-effort search for the shared RPM (16 values) and pedal (8 values)
/// axes in the 256 bytes preceding `map_addr`. Returns `(rpm_addr, pedal_addr)`.
/// Both halves are optional — callers should treat None as "axis unknown".
fn find_driver_wish_axes(data: &[u8], map_addr: usize) -> (Option<usize>, Option<usize>) {
    let search_start = map_addr.saturating_sub(256);
    let read_be_u16 = |i: usize| -> u16 {
        ((data[i] as u16) << 8) | data[i + 1] as u16
    };

    let mut rpm_addr: Option<usize> = None;
    let mut pedal_addr: Option<usize> = None;

    // RPM axis: 16 monotonically-non-decreasing values, starts ≤ 200, ends 4500-6500.
    let mut a = search_start;
    while a + 32 <= map_addr {
        let first = read_be_u16(a);
        let last = read_be_u16(a + 30);
        if first <= 200 && (4500..=6500).contains(&last) {
            let mut mono = true;
            let mut prev = first;
            for i in 1..16 {
                let v = read_be_u16(a + i * 2);
                if v < prev { mono = false; break; }
                prev = v;
            }
            if mono { rpm_addr = Some(a); break; }
        }
        a += 2;
    }

    // Pedal axis: 8 monotonically-non-decreasing values, ends at 0x2710 (10000).
    let mut a = search_start;
    while a + 16 <= map_addr {
        if read_be_u16(a + 14) == 0x2710 {
            let mut mono = true;
            let mut prev = read_be_u16(a);
            if prev <= 200 {
                for i in 1..8 {
                    let v = read_be_u16(a + i * 2);
                    if v < prev { mono = false; break; }
                    prev = v;
                }
                if mono { pedal_addr = Some(a); break; }
            }
        }
        a += 2;
    }

    (rpm_addr, pedal_addr)
}

/// EDC16U1 ECU specific detector
/// EDC16U1 specialized detector
pub struct EDC16U1Detector {
    tuned_mode: bool,  // Use extended ranges for tuned files
}

impl EDC16U1Detector {
    pub fn new() -> Self {
        Self {
            tuned_mode: false,
        }
    }

    /// Create detector in tuned mode (extended value ranges)
    pub fn new_tuned() -> Self {
        Self {
            tuned_mode: true,
        }
    }

    /// Main detection function - AXIS-FIRST algorithm (fast & accurate)
    /// Based on EDC16U1 analysis
    pub fn detect(&self, data: &[u8]) -> Vec<DetectedMap> {
        log::debug!("🔧 [EDC16U1] Starting AXIS-FIRST detection, file size: {} bytes, tuned_mode: {}",
                 data.len(), self.tuned_mode);

        let mut all_maps = Vec::new();
        let mut detected_ranges: HashSet<(u32, u32)> = HashSet::new();

        let (scan_start, scan_end) = self.get_data_section_range(data.len());
        log::debug!("🔧 [EDC16] Scanning range 0x{:X} - 0x{:X}", scan_start, scan_end);

        // ========== PHASE 0: Signature-based detection (highest reliability) ==========
        // These maps have IDENTICAL byte sequences across all files

        // Detect SVBL (Max Boost Pressure scalar)
        let svbl_maps = self.detect_svbl_by_signature(data);
        for map in svbl_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // Detect SVRL (RPM Limiter scalar)
        let svrl_maps = self.detect_svrl_by_signature(data);
        for map in svrl_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // Detect Maximum Vehicle Speed maps
        let detected_addresses_for_vmax: HashSet<u32> = detected_ranges.iter().map(|(start, _)| *start).collect();
        let vmax_maps = self.detect_maximum_vehicle_speed(data, &detected_addresses_for_vmax);
        log::debug!("Vmax detect returned {} maps", vmax_maps.len());
        for map in &vmax_maps {
            log::debug!("  Vmax candidate: 0x{:X}", map.address);
        }
        for map in vmax_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(map);
            } else {
                log::debug!("Vmax 0x{:X} REJECTED due to overlap", map.address);
            }
        }

        // Detect SOI Limiters BEFORE axis-based detection
        // This ensures they get priority over false positive SOI maps that might overlap
        let soi_limiter_maps = self.detect_soi_limiter_maps(data, scan_start, scan_end, &HashSet::new());
        log::debug!("🔧 [EDC16] Pre-detected {} SOI Limiters in PHASE 0", soi_limiter_maps.len());
        for map in soi_limiter_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // NOTE: SOI Selectors are detected AFTER axis-based detection
        // because they need the SOI block addresses to avoid false positives

        // ========== PHASE 0.5: IQ Limit Maps (Smoke Limiter by MAP/MAF) ==========
        // Detect Smoke Limiter by MAP and Smoke Limiter by MAF by signature [00 10][00 0D]
        let detected_addresses_set: HashSet<u32> = detected_ranges.iter().map(|(start, _)| *start).collect();
        let iq_limit_maps = self.detect_iq_limit_maps_by_signature(data, &detected_addresses_set);
        for map in iq_limit_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // Detect small Lambda maps (2x2, 2x3, 3x2)
        let detected_addresses_set2: HashSet<u32> = detected_ranges.iter().map(|(start, _)| *start).collect();
        let small_lambda_maps = self.detect_small_lambda_maps(data, &detected_addresses_set2);
        for map in small_lambda_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // ========== PHASE 0.6: Duration Block Detection ==========
        // Detect Duration Selector first, then Duration maps 00-05
        let mut detected_addresses: HashSet<u32> = detected_ranges.iter().map(|(start, _)| *start).collect();

        let duration_selectors = self.detect_duration_selector(data);
        for (selector_addr, selector_map) in &duration_selectors {
            let range = (selector_map.address, selector_map.address + selector_map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(selector_map.clone());

                // Detect Duration maps for this block
                let duration_maps = self.detect_duration_block_maps(data, *selector_addr, &detected_addresses);
                for map in duration_maps {
                    let map_range = (map.address, map.address + map.size as u32);
                    if !self.overlaps(&detected_ranges, map_range) {
                        detected_addresses.insert(map.address);
                        detected_ranges.insert(map_range);
                        all_maps.push(map);
                    }
                }
            }
        }
        log::debug!("🔧 [EDC16] Duration block detection complete: {} selectors found", duration_selectors.len());

        // ========== PHASE 0.7: Gearbox Torque Limiter Detection ==========
        // Detect 15x1 1D maps that limit torque per gear (typically all 30000 = OFF on VAG)
        let gearbox_maps = self.detect_gearbox_torque_limiter(data, &detected_addresses);
        for map in gearbox_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_addresses.insert(map.address);
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // ========== PHASE 0.8: Fuel Correction Maps Detection ==========
        // Detect correction factor maps (fuel temp, oil temp, EGT, ambient pressure, etc.)
        let fuel_correction_maps = self.detect_fuel_correction_maps(data, &detected_addresses);
        for map in fuel_correction_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_addresses.insert(map.address);
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // ========== PHASE 0.9: EGR Maps Detection ==========
        // Detect EGR 2D maps (16x13) and EGR Hysteresis 1D maps (20x1)
        let egr_maps = self.detect_egr_maps(data, &detected_addresses);
        for map in egr_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_addresses.insert(map.address);
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // ========== PHASE 0.10: Idle Speed Maps Detection ==========
        // Detect Single Value Idle Speed (1x1) and Idle Speed Correction by Engine Temp (2x4, 4x4)
        let idle_speed_maps = self.detect_idle_speed_maps(data, &detected_addresses);
        for map in idle_speed_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_addresses.insert(map.address);
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // ========== PHASE 0.11: EGT (Exhaust Gas Temperature) Maps Detection ==========
        // Detect EGT sensor linearisation and EGT base maps
        let egt_maps = self.detect_egt_maps(data, &detected_addresses);
        for map in egt_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_addresses.insert(map.address);
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // ========== PHASE 1: Axis-based detection ==========
        // This finds RPM axes first, then validates adjacent data structures
        let axis_maps = self.detect_all_maps_by_axes(data, scan_start, scan_end, &detected_ranges);

        for map in axis_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // ========== PHASE 2: SOI block validation ==========
        // A valid SOI block must have: 10 SOI maps + SOI Selector + SOI Limiter
        // We filter out blocks that don't have all components

        // Collect SOI map addresses (maps named "Start of injection XX")
        let soi_addresses: Vec<u32> = all_maps.iter()
            .filter(|m| m.name.as_ref().map_or(false, |n| n.starts_with("Start of injection")))
            .map(|m| m.address)
            .collect();

        // Collect SOI Limiter addresses
        let soi_limiter_addresses: Vec<u32> = all_maps.iter()
            .filter(|m| m.name.as_ref().map_or(false, |n| n == "SOI Limiter"))
            .map(|m| m.address)
            .collect();

        log::debug!("🔍 [EDC16] Found {} SOI maps and {} SOI Limiters for validation",
                  soi_addresses.len(), soi_limiter_addresses.len());

        // Debug: check if expected block starts are in the list
        let expected_block_starts = [0x18DA48u32, 0x196640, 0x19B7A0, 0x1D9D78];
        for &expected in &expected_block_starts {
            if soi_addresses.contains(&expected) {
                log::debug!("✅ [EDC16] Block start 0x{:X} is in soi_addresses", expected);
            } else {
                log::debug!("❌ [EDC16] Block start 0x{:X} is NOT in soi_addresses", expected);
            }
        }

        // Detect SOI Selectors
        let soi_selector_maps = if !soi_addresses.is_empty() {
            self.detect_soi_selector_by_signature(data, &soi_addresses)
        } else {
            Vec::new()
        };

        let soi_selector_addresses: Vec<u32> = soi_selector_maps.iter()
            .map(|m| m.address)
            .collect();

        // Identify valid SOI blocks:
        // A block starts at address X and ends ~4700 bytes later
        // It must have:
        // - A SOI Selector within 300 bytes after the 10th map
        // - A SOI Limiter within 3000 bytes after the 10th map
        const SOI_MAP_SIZE: usize = 16 * 14 * 2;  // 448 bytes per map
        const SOI_BLOCK_SIZE: u32 = (SOI_MAP_SIZE * 10) as u32;  // 4480 bytes

        // Find block start addresses (first map of each block of 10)
        // Note: Some files have interleaved metadata for the last N maps
        // In that case, the spacing changes from 448 to 512 bytes for those maps
        let mut valid_block_starts: Vec<u32> = Vec::new();
        const INTERLEAVED_METADATA_SIZE: u32 = 64;

        for &addr in &soi_addresses {
            // Check if this could be "Start of injection 00" (first map of a block)
            // Try both: standard format (all 448-byte spacing) and interleaved format
            let is_block_start_standard = (0..10).all(|i| {
                let expected_addr = addr + (i * SOI_MAP_SIZE as u32);
                soi_addresses.contains(&expected_addr)
            });

            // Check for interleaved format: first N maps at 448-byte spacing,
            // then remaining maps at 512-byte spacing (448 + 64 metadata)
            let is_block_start_interleaved = (1..=5).any(|interleaved_count| {
                let contiguous_maps = 10 - interleaved_count;
                (0..10).all(|i| {
                    let expected_addr = if i < contiguous_maps {
                        addr + (i * SOI_MAP_SIZE as u32)
                    } else {
                        let interleaved_idx = i - contiguous_maps;
                        addr + (contiguous_maps as u32 * SOI_MAP_SIZE as u32)
                            + (interleaved_idx as u32 * (SOI_MAP_SIZE as u32 + INTERLEAVED_METADATA_SIZE))
                            + INTERLEAVED_METADATA_SIZE
                    };
                    soi_addresses.contains(&expected_addr)
                })
            });

            let is_block_start = is_block_start_standard || is_block_start_interleaved;

            if is_block_start {
                // Calculate block_end accounting for potential interleaved metadata
                // For interleaved format, block is larger: 10*448 + N*64 where N is interleaved count
                // We check with max interleaved (5*64 = 320 extra bytes)
                let block_end_max = addr + SOI_BLOCK_SIZE + 5 * INTERLEAVED_METADATA_SIZE;

                // Check for SOI Selector within 300 bytes after block
                // Use the standard block_end for selector check (it's right after main metadata)
                let block_end_standard = addr + SOI_BLOCK_SIZE;
                let has_selector = soi_selector_addresses.iter().any(|&sel_addr| {
                    sel_addr > block_end_standard && sel_addr < block_end_max + 300
                });

                // Check for SOI Limiter within 3000 bytes after block
                let has_limiter = soi_limiter_addresses.iter().any(|&lim_addr| {
                    lim_addr > block_end_standard && lim_addr < block_end_max + 3000
                });

                // Sélecteur OPTIONNEL : les SDI atmosphériques (Golf 75ch
                // 016M/GJ, Caddy 016HN) ont le bloc de 10 + limiteur mais
                // AUCUN sélecteur — le limiteur suffit à valider le bloc.
                if has_limiter {
                    log::debug!("✅ [EDC16] Valid SOI block at 0x{:X} (Limiter ok, Selector: {})",
                              addr, has_selector);
                    valid_block_starts.push(addr);
                } else {
                    log::debug!("❌ [EDC16] Invalid SOI block at 0x{:X} (Selector: {}, Limiter: {})",
                              addr, has_selector, has_limiter);
                }
            }
        }

        // Filter SOI maps and SOI Dynamic maps to keep only those from valid blocks
        all_maps.retain(|map| {
            if let Some(ref name) = map.name {
                if name.starts_with("Start of injection") {
                    // Check if this map belongs to a valid block
                    // Account for interleaved metadata: block can be up to 4480 + 320 = 4800 bytes
                    let block_size_with_interleaved = SOI_BLOCK_SIZE + 5 * INTERLEAVED_METADATA_SIZE;
                    let belongs_to_valid_block = valid_block_starts.iter().any(|&block_start| {
                        map.address >= block_start && map.address < block_start + block_size_with_interleaved
                    });
                    return belongs_to_valid_block;
                }
                if name == "Start of injection Dynamic" {
                    // SOI Dynamic maps can be:
                    // 1. BEFORE the main 10 SOI maps (up to 5 maps = 2240 bytes before block_start)
                    // 2. AFTER the main 10 SOI maps + metadata (at block_end + 106 + up to 5*448 bytes)
                    let belongs_to_valid_block = valid_block_starts.iter().any(|&block_start| {
                        let block_end = block_start + SOI_BLOCK_SIZE + 5 * INTERLEAVED_METADATA_SIZE;
                        // Dynamic maps BEFORE main block
                        let is_before = map.address >= block_start.saturating_sub(5 * SOI_MAP_SIZE as u32)
                                       && map.address < block_start;
                        // Dynamic maps AFTER main block + metadata
                        let is_after = map.address > block_end
                                      && map.address < block_end + 106 + 5 * SOI_MAP_SIZE as u32;
                        is_before || is_after
                    });
                    return belongs_to_valid_block;
                }
            }
            true  // Keep non-SOI maps
        });

        // Add SOI Selectors from valid blocks only
        for selector in soi_selector_maps {
            // Check if this selector is associated with a valid block
            let belongs_to_valid_block = valid_block_starts.iter().any(|&block_start| {
                let block_end = block_start + SOI_BLOCK_SIZE;
                selector.address > block_end && selector.address < block_end + 300
            });

            if belongs_to_valid_block {
                let range = (selector.address, selector.address + selector.size as u32);
                if !self.overlaps(&detected_ranges, range) {
                    detected_ranges.insert(range);
                    all_maps.push(selector);
                }
            }
        }

        log::debug!("🔧 [EDC16] After SOI validation: {} total maps ({} valid SOI blocks)",
                  all_maps.len(), valid_block_starts.len());

        // Rename SOI Dynamic maps to "Start of injection (Dynamic)" for display
        // These maps are now included in the output for users to modify
        let mut dynamic_count = 0;
        for map in all_maps.iter_mut() {
            if map.name.as_deref() == Some("Start of injection Dynamic") {
                map.name = Some("Start of injection (Dynamic)".to_string());
                dynamic_count += 1;
            }
        }
        if dynamic_count > 0 {
            log::debug!("🎯 [EDC16] Renamed {} SOI Dynamic maps to 'Start of injection (Dynamic)'", dynamic_count);
        }

        // Generate descriptions for all maps that don't have one yet
        // Format: "Unit | X: X_label | Y: Y_label"
        for map in all_maps.iter_mut() {
            if map.description.is_none() {
                if let (Some(unit), Some(x_label), Some(y_label)) = (map.unit.as_ref(), map.x_label.as_ref(), map.y_label.as_ref()) {
                    map.description = Some(format!("{} | X: {} | Y: {}", unit, x_label, y_label));
                }
            }
        }

        // Normalisation des noms (portée depuis U31/U34) : mêmes noms que
        // l'EDC15P/les autres familles pour le boost, et « EGR (Desired air
        // quantity) » au lieu de « EGR N » — l'U1 gardait les anciens noms,
        // ce qui faussait la vérification de complétude.
        for m in all_maps.iter_mut() {
            let new_name: Option<String> = match m.name.as_deref() {
                Some("Boost Target") => Some("Boost target map".to_string()),
                Some("Boost Target 2") => Some("Boost target map 2".to_string()),
                Some("Boost Limiter") | Some("Boost Limiter (Altitude)") => {
                    Some("Boost limit map".to_string())
                }
                Some("N75 Duty Cycle") => Some("N75 duty cycle".to_string()),
                Some("SVBL - Max Boost Pressure") | Some("Max Boost Pressure") => {
                    Some("SVBL (Single value boost limiter)".to_string())
                }
                Some("Turbo Boost Pressure Correction") => {
                    Some("Boost correction by temperature".to_string())
                }
                Some(n) if n.starts_with("EGR ") && n[4..].chars().all(|c| c.is_ascii_digit()) => {
                    Some(format!("EGR (Desired air quantity) {}", &n[4..]))
                }
                _ => None,
            };
            if let Some(n) = new_name {
                m.name = Some(n);
            }
        }

        all_maps
    }

    /// Fast detection of all maps by finding axes first
    /// Layout in EDC16: [Y axis][X axis][Map Data]
    fn detect_all_maps_by_axes(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        existing: &HashSet<(u32, u32)>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut detected_addresses: HashSet<u32> = HashSet::new();

        // ========== PHASE 0.5: SOI (Start of Injection) blocks - FIRST ==========
        // SOI maps MUST be detected BEFORE generic axis detection because:
        // 1. They have a unique layout with shared axes AFTER the map data
        // 2. Generic detection might misidentify SOI map data as axes
        // 3. We need all 10 maps per block to be detected together
        let soi_maps = self.detect_soi_blocks(data, start, end, &detected_addresses);
        for map in soi_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // Fallback SDI (Caddy 70 / Golf 75 atmosphériques) : pas de bloc de
        // 10 — un petit groupe de 2-6 maps SOI headerées [0010][000E|000F]
        // quasi contiguës (gap ≤0x120), données en degrés signés.
        // Référence : 03G906016N, 4 maps (2×16x14 + 2×16x15) à 0xE89DC.
        let has_numbered_soi = maps.iter().any(|m| {
            m.name.as_deref().map_or(false, |n| {
                n.starts_with("Start of injection ")
                    && n["Start of injection ".len()..]
                        .chars()
                        .next()
                        .map_or(false, |c| c.is_ascii_digit())
            })
        });
        if !has_numbered_soi {
            let rd = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
            let mut found: Vec<(usize, usize, usize, usize)> = Vec::new(); // (d0, y0, x0, cols)
            let mut i = start;
            let scan_end2 = end.min(data.len().saturating_sub(600));
            while i < scan_end2 {
                if !(data[i] == 0
                    && data[i + 1] == 0x10
                    && data[i + 2] == 0
                    && (data[i + 3] == 0x0E || data[i + 3] == 0x0F))
                {
                    i += 2;
                    continue;
                }
                let cols = data[i + 3] as usize;
                let y0 = i + 4;
                let x0 = y0 + 32;
                let d0 = x0 + cols * 2;
                if d0 + 16 * cols * 2 > data.len() {
                    i += 2;
                    continue;
                }
                let rpm: Vec<u16> = (0..16).map(|k| rd(y0 + k * 2)).collect();
                if rpm[0] > 300
                    || rpm.windows(2).any(|w| w[0] >= w[1])
                    || !(4000..=5500).contains(&rpm[15])
                {
                    i += 2;
                    continue;
                }
                let x: Vec<u16> = (0..cols).map(|k| rd(x0 + k * 2)).collect();
                if x[0] > 300
                    || x.windows(2).any(|w| w[0] >= w[1])
                    || !(3000..=5000).contains(&x[cols - 1])
                {
                    i += 2;
                    continue;
                }
                let vals: Vec<i16> = (0..16 * cols).map(|k| rd(d0 + k * 2) as i16).collect();
                // Degrés SOI : bornés ±3000, mélange négatif/positif
                if vals.iter().any(|&v| v.unsigned_abs() > 3000)
                    || !vals.iter().any(|&v| v < 0)
                    || !vals.iter().any(|&v| v > 200)
                {
                    i += 2;
                    continue;
                }
                if detected_addresses.contains(&(d0 as u32)) {
                    i += 2;
                    continue;
                }
                // Contiguïté du run (gap ≤0x120 entre fin de map et header)
                if let Some(&(pd0, _, _, pcols)) = found.last() {
                    let prev_end = pd0 + 16 * pcols * 2;
                    if i < prev_end || i - prev_end > 0x120 {
                        if found.len() >= 2 {
                            break;
                        }
                        found.clear();
                    }
                }
                found.push((d0, y0, x0, cols));
                i = d0 + 16 * cols * 2;
            }
            if found.len() >= 2 {
                log::debug!(
                    "🎯 [EDC16U1] SOI fallback SDI: {} maps à partir de 0x{:X}",
                    found.len(),
                    found[0].0
                );
                for (idx, &(d0, y0, x0, cols)) in found.iter().enumerate() {
                    let mut map = DetectedMap::new(
                        d0 as u32,
                        16 * cols * 2,
                        MapDimensions::TwoDimensional { rows: 16, cols },
                        DataType::Int16,
                    );
                    map.name = Some(format!("Start of injection {:02}", idx));
                    map.category =
                        Some(MapCategory::StartOfInjection.display_name().to_string());
                    map.unit = Some("deg CrS".to_string());
                    map.correction_factor = Some(0.023437);
                    map.confidence = 0.88;
                    map.y_axis_address = Some(y0 as u32);
                    map.y_label = Some("rpm".to_string());
                    map.y_axis_correction = Some(1.0);
                    map.x_axis_address = Some(x0 as u32);
                    map.x_label = Some("mg/stroke".to_string());
                    map.x_axis_correction = Some(0.01);
                    detected_addresses.insert(d0 as u32);
                    maps.push(map);
                }
            }
        }

        // ========== PHASE 1: Standard maps (RPM axis starting 400-1600) ==========
        let mut offset = start;
        while offset + 64 <= end.min(data.len()) {
            // Try different RPM axis lengths (15, 16 are most common)
            for rpm_len in [16usize, 15, 14, 13, 11, 10] {
                if offset + rpm_len * 2 > end.min(data.len()) {
                    continue;
                }

                let rpm_axis = self.read_values_be_unsigned(data, offset, rpm_len);

                // Check if this looks like an RPM axis
                if self.is_valid_rpm_axis(&rpm_axis) {
                    // Found a potential RPM (Y) axis - now look for X axis after it
                    let x_axis_start = offset + rpm_len * 2;

                    // Try different X axis lengths based on map type signatures
                    for x_len in [16usize, 14, 13, 12, 11, 10, 8, 6] {
                        if x_axis_start + x_len * 2 > end.min(data.len()) {
                            continue;
                        }

                        let x_axis = self.read_values_be_unsigned(data, x_axis_start, x_len);

                        // Identify map type based on dimensions and axis patterns
                        if let Some((map_type, rows, cols)) = self.identify_map_type_by_axes(&rpm_axis, &x_axis, rpm_len, x_len) {
                            let map_start = x_axis_start + x_len * 2;
                            let map_size = rows * cols * 2;

                            if map_start + map_size > end.min(data.len()) {
                                continue;
                            }

                            // Check if already detected at this address
                            if detected_addresses.contains(&(map_start as u32)) {
                                continue;
                            }

                            // Check overlap with existing maps
                            if self.overlaps(existing, (map_start as u32, (map_start + map_size) as u32)) {
                                continue;
                            }

                            // Validate map data
                            let values: Vec<f64> = self.read_values_be_signed(data, map_start, rows * cols)
                                .iter()
                                .map(|&v| v as f64)
                                .collect();

                            if let Some(template) = MAP_TEMPLATES.iter().find(|t| t.name == map_type) {
                                if let Some(score) = self.score_map(&values, rows, cols, template) {
                                    if score >= 0.65 {
                                        let map = self.create_detected_map(
                                            map_type,
                                            map_start,
                                            rows,
                                            cols,
                                            offset,        // Y axis address
                                            x_axis_start,  // X axis address
                                            score,
                                            template,
                                        );

                                        detected_addresses.insert(map_start as u32);
                                        maps.push(map);

                                        // Skip past this map
                                        offset = map_start + map_size - 2;
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            offset += 2;
        }

        // ========== PHASE 2: Driver Wish maps (RPM axis starting 0) ==========
        // Layout: [Y axis (16 RPM)][X axis (8 Pedal)][Map Data (16*8)]
        let driver_wish_maps = self.detect_driver_wish_maps(data, start, end, &detected_addresses);
        for map in driver_wish_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // ========== PHASE 3: Boost Target maps (RPM starts 0, IQ axis 0-4500) ==========
        // Layout: [Y axis (16 RPM starting 0)][X axis (10 IQ)][Map Data (16*10)]
        let boost_target_maps = self.detect_boost_target_maps(data, start, end, &detected_addresses);
        for map in boost_target_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // ========== PHASE 4: Boost Limiter maps (RPM 1400-4500, Atmo 600-1100) ==========
        let boost_limiter_maps = self.detect_boost_limiter_maps(data, start, end, &detected_addresses);
        for map in boost_limiter_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // ========== PHASE 5: N75 Duty Cycle maps (SIGNATURE-BASED) ==========
        // N75 maps use signature pattern: [00][rows][00][cols] before axes
        // Common variants: 16x10, 16x12, 16x11
        let n75_maps = self.detect_n75_by_signature(data, &detected_addresses);
        for map in n75_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // ========== PHASE 6: Torque to IQ Conversion (SIGNATURE-BASED) ==========
        // Signature: [00 0F 00 XX] where 0F = 15 rows, XX = cols (10-18)
        let torque_iq_maps = self.detect_torque_to_iq_by_signature(data, &detected_addresses);
        for map in torque_iq_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // ========== PHASE 7: Cranking Torque Maps (Start IQ) ==========
        // Signature: [00 09][00 09] - 9x9 maps
        // Structure: Signature -> Temp axis (9 vals) -> RPM axis (9 vals) -> Map data
        let cranking_maps = self.detect_cranking_torque_maps(data, start, end, &detected_addresses);
        for map in cranking_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // ========== PHASE 8: Turbo Boost Pressure Correction ==========
        // Signature: [00 10][00 0A] = 16x10 maps
        // Structure: Signature -> Boost axis (16 vals) -> Temp axis (10 vals) -> Map data
        let boost_correction_maps = self.detect_turbo_boost_correction_maps(data, start, end, &detected_addresses);
        for map in boost_correction_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // ========== PHASE 9: MAP Linearization (Map Sensor) ==========
        // Signature: [00 8B][03 E7][00 02] - unique signature for MAP sensor linearization
        // Structure: Signature -> Count (2) -> X axis (2 mV) -> Y values (2 pressure)
        let map_linearization = self.detect_map_linearization(data, start, end, &detected_addresses);
        for map in map_linearization {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // ========== PHASE 10: Torque Limiter ==========
        // Multiple dimension variants: 20x4, 21x3, 22x3, 22x4
        // Signature: [7F FF][7F FF][7F FF][00 rows][00 cols] followed by pressure axis, RPM axis, map data
        // X axis: RPM, Y axis: Atmospheric pressure (mbar), Values: Torque (Nm * 10)
        let torque_limiter_maps = self.detect_torque_limiter_maps(data, start, end, &detected_addresses);
        for map in torque_limiter_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // ========== PHASE 11: Torque Limiter by Vehicle Speed (Launch Control) ==========
        // Signature: [00 05][00 00][00 32][00 00][00 0A][00 0A] (ID=5, 50, 10x10)
        // Y axis: RPM (800-5000), X axis: Vehicle speed (0-180 km/h)
        // Values: Torque limit in Nm * 10 (typically 3000 Nm = disabled)
        let launch_control_maps = self.detect_launch_control_maps(data, start, end, &detected_addresses);
        for map in launch_control_maps {
            if !detected_addresses.contains(&map.address) {
                detected_addresses.insert(map.address);
                maps.push(map);
            }
        }

        // NOTE: SOI blocks are now detected in PHASE 0.5 (at the start of this function)
        // to ensure they get priority before generic axis detection

        // NOTE: SOI Limiters are detected in PHASE 0 (before this function, in detect())
        // to ensure they get priority over false positive SOI maps that might overlap

        // Log results by category
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for map in &maps {
            if let Some(name) = &map.name {
                *counts.entry(name.clone()).or_insert(0) += 1;
            }
        }
        for (name, count) in counts {
            log::debug!("🔧 [EDC16] Found {} {} maps", count, name);
        }

        maps
    }

    /// Detect Driver Wish maps for EDC16U1 (PD 1.9L / 2.0L TDI).
    ///
    /// Real-world layout differs from U34: maps are 8 cols (pedal) × 16 rows
    /// (RPM), stored as a contiguous block of 7 consecutive maps spaced by
    /// 0x100 bytes. The RPM and pedal axes are shared and stored once in the
    /// header region preceding the first map — not repeated per map — so we
    /// can't rely on a per-map structural signature like U34.
    ///
    /// Strategy: shape-based detection. For each candidate position, check
    /// whether the 256-byte block parses as a plausible Driver Wish table
    /// (torque values in raw 0-7000 / 0-700 Nm, each row non-decreasing along
    /// pedal axis, first row peaks higher than last row), AND that the six
    /// following blocks at +0x100 increments also pass. The cluster pattern
    /// gives the strict discrimination that a single-block check can't.
    fn detect_driver_wish_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        const COLS: usize = 8;
        const ROWS: usize = 16;
        const MAP_SIZE: usize = COLS * ROWS * 2; // 256 bytes
        const GROUP_COUNT: usize = 7;
        const GROUP_STRIDE: usize = 0x100;
        const GROUP_TOTAL: usize = GROUP_COUNT * GROUP_STRIDE;

        let mut offset = start;
        while offset + GROUP_TOTAL <= end.min(data.len()) {
            if detected.contains(&(offset as u32)) {
                offset += 2;
                continue;
            }

            // First block must look like a Driver Wish table on its own.
            if !is_driver_wish_block(data, offset) {
                offset += 2;
                continue;
            }

            // All 7 consecutive blocks must pass the same shape check.
            let mut all_ok = true;
            for i in 1..GROUP_COUNT {
                if !is_driver_wish_block(data, offset + i * GROUP_STRIDE) {
                    all_ok = false;
                    break;
                }
            }
            if !all_ok {
                offset += 2;
                continue;
            }

            log::debug!("🎯 [EDC16U1] Found Driver Wish cluster at 0x{:X} ({} maps)", offset, GROUP_COUNT);

            // Best-effort axis location: look in the 96 bytes immediately
            // preceding the first map for an RPM-like 16-value axis (starts
            // ≤100, ends 4500-6500, monotone) and a pedal-like 8-value axis
            // (ends at 10000 / 0x2710). These are advisory hints; if not
            // found we still emit the maps without axis addresses.
            let (rpm_axis_addr, pedal_axis_addr) = find_driver_wish_axes(data, offset);

            let template = MAP_TEMPLATES.iter().find(|t| t.name == "Driver Wish");
            for i in 0..GROUP_COUNT {
                let map_addr = offset + i * GROUP_STRIDE;
                if detected.contains(&(map_addr as u32)) { continue; }

                let mut map = if let Some(tmpl) = template {
                    let mut m = self.create_detected_map(
                        "Driver Wish",
                        map_addr,
                        ROWS,
                        COLS,
                        rpm_axis_addr.unwrap_or(0),
                        pedal_axis_addr.unwrap_or(0),
                        0.90,
                        tmpl,
                    );
                    // Override axis addresses to None if we didn't find them.
                    if rpm_axis_addr.is_none() { m.y_axis_address = None; }
                    if pedal_axis_addr.is_none() { m.x_axis_address = None; }
                    m
                } else {
                    let mut m = DetectedMap::new(
                        map_addr as u32,
                        MAP_SIZE,
                        MapDimensions::TwoDimensional { rows: ROWS, cols: COLS },
                        DataType::Int16,
                    );
                    m.category = Some(MapCategory::EngineTorqueRequest.display_name().to_string());
                    m.correction_factor = Some(0.1);
                    m.unit = Some("Nm".to_string());
                    m.confidence = 0.90;
                    m
                };

                map.name = Some("Driver Wish".to_string());
                maps.push(map);
            }

            offset += GROUP_TOTAL;
        }

        // Variante R5/V10 et vieux SW 1.9 : chaque map Driver Wish porte
        // son PROPRE en-tête [00 0C..10][00 08|09] + axes inline (RPM 12-16
        // partant de 0, pédale 8-9 finissant à 7000-10200) — pas de cluster
        // contigu stride 0x100. Références : 070906016A (7×16x8 à 0xE094C),
        // 016J/038906016A (7×12x8 aux MÊMES adresses).
        if maps.is_empty() {
            let mut offset2 = start;
            while offset2 + 4 + 32 + 18 + MAP_SIZE <= end.min(data.len()) {
                if !(data[offset2] == 0x00
                    && (0x0C..=0x10).contains(&data[offset2 + 1])
                    && data[offset2 + 2] == 0x00
                    && (data[offset2 + 3] == 0x08 || data[offset2 + 3] == 0x09))
                {
                    offset2 += 2;
                    continue;
                }
                let rows2 = data[offset2 + 1] as usize;
                let cols2 = data[offset2 + 3] as usize;
                let rpm_start = offset2 + 4;
                let rpm = self.read_values_be_unsigned(data, rpm_start, rows2);
                // rpm[0] ≤ 600 : les programmes bridés démarrent à 550
                // (Touareg 016F, même convention que les chaînes Crafter U31)
                let rpm_ok = rpm[0] <= 600
                    && rpm.windows(2).all(|w| w[0] < w[1])
                    && (3500..=6500).contains(&rpm[rows2 - 1]);
                if !rpm_ok {
                    offset2 += 2;
                    continue;
                }
                let ped_start = rpm_start + rows2 * 2;
                let ped = self.read_values_be_unsigned(data, ped_start, cols2);
                // Fin de pédale : ~10000 sur R5 (échelle %×100), mais
                // 7373-8192 sur V10 (échelle 0x2000) — accepter 7000+.
                let ped_ok = ped[0] <= 700
                    && ped.windows(2).all(|w| w[0] < w[1])
                    && (7000..=10200).contains(&ped[cols2 - 1]);
                if !ped_ok {
                    offset2 += 2;
                    continue;
                }
                let map_addr = ped_start + cols2 * 2;
                let map_size2 = rows2 * cols2 * 2;
                if map_addr + map_size2 > data.len()
                    || detected.contains(&(map_addr as u32))
                    || maps.iter().any(|m| m.address == map_addr as u32)
                {
                    offset2 += 2;
                    continue;
                }
                // Données : couple SIGNÉ ×0.1 plausible (−250..2000 Nm) — le
                // Driver Wish porte des valeurs négatives (frein moteur) en
                // bas de pédale, ex. 65516 = −20.
                let vals = self.read_values_be_unsigned(data, map_addr, rows2 * cols2);
                let signed_ok = vals.iter().all(|&v| {
                    let s = v as i16;
                    (-2500..=20000).contains(&s)
                });
                if !signed_ok || !vals.iter().any(|&v| (v as i16) > 100) {
                    offset2 += 2;
                    continue;
                }

                let mut map = DetectedMap::new(
                    map_addr as u32,
                    map_size2,
                    MapDimensions::TwoDimensional { rows: rows2, cols: cols2 },
                    DataType::Int16,
                );
                map.name = Some("Driver Wish".to_string());
                map.category = Some(MapCategory::EngineTorqueRequest.display_name().to_string());
                map.unit = Some("Nm".to_string());
                map.correction_factor = Some(0.1);
                map.confidence = 0.88;
                map.y_axis_address = Some(rpm_start as u32);
                map.y_label = Some("rpm".to_string());
                map.y_axis_correction = Some(1.0);
                map.x_axis_address = Some(ped_start as u32);
                map.x_label = Some("%".to_string());
                map.x_axis_correction = Some(0.01);
                map.y_axis_inverted = Some(true);
                maps.push(map);
                offset2 = map_addr + map_size2;
            }
            if !maps.is_empty() {
                log::debug!("🎯 [EDC16U1] Found {} headered Driver Wish maps (R5/V10 layout)", maps.len());
            }
        }

        log::debug!("🔧 [EDC16U1] Driver Wish detection: returning {} map(s)", maps.len());
        maps
    }

    /// Detect Boost Target maps (PCR_pBDesBas_MAP, PCR_pBDesBas2_MAP)
    /// SIGNATURE-BASED detection for reliability
    /// Two variants:
    /// - 16 rows × 10 cols (PCR_pBDesBas_MAP) - signature [00 10 00 0A]
    /// - 15 rows × 10 cols (PCR_pBDesBas2_MAP) - signature [00 0F 00 0A]
    fn detect_boost_target_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut local_detected = detected.clone();

        // Detect 16x10 variant (PCR_pBDesBas_MAP)
        let maps_16x10 = self.detect_boost_target_variant(data, start, end, 16, 10, &local_detected);
        for map in maps_16x10 {
            local_detected.insert(map.address);
            maps.push(map);
        }

        // Detect 15x10 variant (PCR_pBDesBas2_MAP)
        let maps_15x10 = self.detect_boost_target_variant(data, start, end, 15, 10, &local_detected);
        for map in maps_15x10 {
            local_detected.insert(map.address);
            maps.push(map);
        }

        log::debug!("🔧 [EDC16] Boost Target detection: found {} maps (16x10: {}, 15x10: {})",
                  maps.len(),
                  maps.iter().filter(|m| matches!(&m.dimensions, MapDimensions::TwoDimensional { rows, .. } if *rows == 16)).count(),
                  maps.iter().filter(|m| matches!(&m.dimensions, MapDimensions::TwoDimensional { rows, .. } if *rows == 15)).count());

        maps
    }

    /// Detect a specific Boost Target variant by signature
    /// Signature format: [00][rows][00][cols] (Big-Endian)
    fn detect_boost_target_variant(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        rows: usize,
        cols: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        // Signature: [00][rows][00][cols]
        let signature: [u8; 4] = [0x00, rows as u8, 0x00, cols as u8];
        let map_size = rows * cols * 2;
        let y_axis_size = rows * 2;  // RPM axis
        let x_axis_size = cols * 2;  // IQ axis

        let mut offset = start;
        'outer: while offset + 4 + 4 + y_axis_size + x_axis_size + map_size <= end.min(data.len()) {
            // Look for signature
            if data[offset..offset + 4] == signature {
                // Found potential signature!
                // Try two structure variants:
                // Variant A: [header 4][Y axis][X axis][data] (no padding)
                // Variant B: [header 4][padding 4][Y axis][X axis][data] (with 4-byte padding)

                for padding in [0usize, 4usize] {
                    let y_axis_start = offset + 4 + padding;
                    let x_axis_start = y_axis_start + y_axis_size;
                    let map_start = x_axis_start + x_axis_size;

                    // Check bounds
                    if map_start + map_size > end.min(data.len()) {
                        continue;
                    }

                    // Skip if already detected
                    if detected.contains(&(map_start as u32)) {
                        continue;
                    }

                    // Read Y axis (RPM) - should start at 0 or low value, end at 4000-5500
                    let rpm_axis = self.read_values_be_unsigned(data, y_axis_start, rows);
                    if !self.is_valid_boost_target_rpm_axis_generic(&rpm_axis) {
                        continue;
                    }

                    // Read X axis (IQ) - should be 0-5000 in mg/stroke * 100
                    let iq_axis = self.read_values_be_unsigned(data, x_axis_start, cols);
                    if !self.is_valid_boost_target_iq_axis(&iq_axis) {
                        continue;
                    }

                    // Validate map data (Boost pressure in mbar)
                    let values: Vec<f64> = self.read_values_be_signed(data, map_start, rows * cols)
                        .iter()
                        .map(|&v| v as f64)
                        .collect();

                    let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

                    // Boost pressure in mbar: typical range 100-3500 (stock), up to 5500 (tuned)
                    // Allow up to 5500 for all files since tuned files can have very high boost values
                    let max_limit = if self.tuned_mode { 6000.0 } else { 5500.0 };
                    if min_val >= 0.0 && max_val <= max_limit && max_val >= 1000.0 {
                        if let Some(template) = MAP_TEMPLATES.iter().find(|t| t.name == "Boost Target") {
                            let mut map = self.create_detected_map(
                                "Boost Target",
                                map_start,
                                rows,
                                cols,
                                y_axis_start,  // Y axis (RPM)
                                x_axis_start,  // X axis (IQ)
                                0.90,  // High confidence for signature match
                                template,
                            );

                            // Add axis metadata
                            map.x_axis_correction = Some(0.01);  // IQ in 0.01 mg/stroke
                            map.y_axis_correction = Some(1.0);   // RPM direct
                            map.x_label = Some("mg/stroke".to_string());
                            map.y_label = Some("rpm".to_string());

                            log::debug!("✅ [EDC16] Found Boost Target {}x{} at 0x{:X} (RPM: {}-{}, IQ: {}-{}, Boost: {:.0}-{:.0} mbar, padding={})",
                                      rows, cols, map_start,
                                      rpm_axis[0], rpm_axis[rows-1],
                                      iq_axis[0], iq_axis[cols-1],
                                      min_val, max_val, padding);

                            maps.push(map);

                            // Skip past this map and continue searching
                            offset = map_start + map_size;
                            continue 'outer;  // Continue outer while loop without incrementing
                        }
                    }
                }
            }
            offset += 2;
        }

        maps
    }

    /// Validate RPM axis for Boost Target maps (generic for any length)
    fn is_valid_boost_target_rpm_axis_generic(&self, values: &[u16]) -> bool {
        if values.is_empty() {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // Boost Target RPM: starts 0-1500 (some files start at 1000), ends 4000-5500
        if first > 1500 {
            return false;
        }
        if last < 4000 || last > 5500 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Detect Boost Limiter maps (PCR_pBDesMaxAP_MAP)
    /// Two variants:
    /// - Type A: 11 rows (RPM 1400-4500), 10 cols (Atmospheric pressure 600-1100)
    /// - Type B: 10 rows (RPM 1500-4500), 10 cols (Atmospheric pressure 550-1100)
    fn detect_boost_limiter_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut local_detected: HashSet<u32> = detected.clone();

        // Type A: 11x10 Boost Limiter
        let type_a_maps = self.detect_boost_limiter_type_a(data, start, end, &local_detected);
        for map in type_a_maps {
            local_detected.insert(map.address);
            maps.push(map);
        }

        // Type B: 10x10 Boost Limiter
        let type_b_maps = self.detect_boost_limiter_type_b(data, start, end, &local_detected);
        for map in type_b_maps {
            local_detected.insert(map.address);
            maps.push(map);
        }

        // Type C: 12x10 AVEC header [00 0C][00 0A] (Transporter R5 : 0xF8628,
        // juste après le boost target — RPM 700..5000, atmo 550..1013)
        let type_c_maps = self.detect_boost_limiter_type_c(data, start, end, &local_detected);
        for map in type_c_maps {
            local_detected.insert(map.address);
            maps.push(map);
        }

        maps
    }

    /// Detect Boost Limiter Type C: header [00 r][00 c] r=10-12, c=8-10 +
    /// RPM + Atmo (variantes R5/V10 headerées, juste après le boost target :
    /// T5 12x10 @0xF8658 et 10x10 @0xF8628, V10 11x8 @0xF8E84, BL 10x10 —
    /// les types A/B sans header ne les voient pas car leur RPM part de 700)
    fn detect_boost_limiter_type_c(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let rd = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);

        let mut offset = start;
        while offset + 4 + 24 * 2 + 240 <= end.min(data.len()) {
            if !(data[offset] == 0x00
                && (0x0A..=0x0C).contains(&data[offset + 1])
                && data[offset + 2] == 0x00
                && (0x08..=0x0A).contains(&data[offset + 3]))
            {
                offset += 2;
                continue;
            }
            let rpm_len = data[offset + 1] as usize;
            let atmo_len = data[offset + 3] as usize;
            let map_size = rpm_len * atmo_len * 2;
            let rpm_start = offset + 4;
            let rpm: Vec<u16> = (0..rpm_len).map(|k| rd(rpm_start + k * 2)).collect();
            let rpm_ok = (500..=1500).contains(&rpm[0])
                && rpm.windows(2).all(|w| w[0] < w[1])
                && (4000..=5500).contains(&rpm[rpm_len - 1]);
            if !rpm_ok {
                offset += 2;
                continue;
            }
            let atmo_start = rpm_start + rpm_len * 2;
            let atmo: Vec<u16> = (0..atmo_len).map(|k| rd(atmo_start + k * 2)).collect();
            let atmo_ok = (450..=750).contains(&atmo[0])
                && atmo.windows(2).all(|w| w[0] < w[1])
                && (950..=1150).contains(&atmo[atmo_len - 1]);
            if !atmo_ok {
                offset += 2;
                continue;
            }
            let map_start = atmo_start + atmo_len * 2;
            if map_start + map_size > data.len() || detected.contains(&(map_start as u32)) {
                offset += 2;
                continue;
            }
            // Données : pression de suralimentation absolue 800-3200 mbar
            let vals: Vec<u16> = (0..rpm_len * atmo_len).map(|k| rd(map_start + k * 2)).collect();
            if vals.iter().any(|&v| v < 700 || v > 3500)
                || !vals.iter().any(|&v| v > 1500)
            {
                offset += 2;
                continue;
            }
            if let Some(template) = MAP_TEMPLATES.iter().find(|t| t.name == "Boost Limiter") {
                log::debug!("🔧 [Boost Limiter Type C] Found {}x{} @ 0x{:X}", rpm_len, atmo_len, map_start);
                let map = self.create_detected_map(
                    "Boost Limiter",
                    map_start,
                    rpm_len,
                    atmo_len,
                    rpm_start,
                    atmo_start,
                    0.85,
                    template,
                );
                maps.push(map);
                offset = map_start + map_size;
                continue;
            }
            offset += 2;
        }

        maps
    }

    /// Detect Boost Limiter Type A: 11x10 (RPM 1400-4500)
    fn detect_boost_limiter_type_a(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        const RPM_LEN: usize = 11;
        const ATMO_LEN: usize = 10;
        const MAP_SIZE: usize = RPM_LEN * ATMO_LEN * 2;  // 220 bytes

        let mut offset = start;
        while offset + 22 + 20 + MAP_SIZE <= end.min(data.len()) {
            let rpm_axis = self.read_values_be_unsigned(data, offset, RPM_LEN);

            if self.is_valid_boost_limiter_rpm_axis_11(&rpm_axis) {
                let atmo_start = offset + RPM_LEN * 2;
                let atmo_axis = self.read_values_be_unsigned(data, atmo_start, ATMO_LEN);

                if self.is_valid_atmospheric_axis(&atmo_axis) {
                    let map_start = atmo_start + ATMO_LEN * 2;

                    if detected.contains(&(map_start as u32)) {
                        offset += 2;
                        continue;
                    }

                    if self.validate_boost_limiter_data(data, map_start, RPM_LEN, ATMO_LEN) {
                        if let Some(template) = MAP_TEMPLATES.iter().find(|t| t.name == "Boost Limiter") {
                            let map = self.create_detected_map(
                                "Boost Limiter",
                                map_start,
                                RPM_LEN,
                                ATMO_LEN,
                                offset,
                                atmo_start,
                                0.85,
                                template,
                            );
                            maps.push(map);
                            offset = map_start + MAP_SIZE - 2;
                        }
                    }
                }
            }
            offset += 2;
        }

        maps
    }

    /// Detect Boost Limiter Type B: 10x10 (RPM 1500-4500)
    fn detect_boost_limiter_type_b(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        const RPM_LEN: usize = 10;
        const ATMO_LEN: usize = 10;
        const MAP_SIZE: usize = RPM_LEN * ATMO_LEN * 2;  // 200 bytes

        let mut offset = start;
        while offset + 20 + 20 + MAP_SIZE <= end.min(data.len()) {
            let rpm_axis = self.read_values_be_unsigned(data, offset, RPM_LEN);

            if self.is_valid_boost_limiter_rpm_axis_10(&rpm_axis) {
                let atmo_start = offset + RPM_LEN * 2;
                let atmo_axis = self.read_values_be_unsigned(data, atmo_start, ATMO_LEN);

                if self.is_valid_atmospheric_axis(&atmo_axis) {
                    let map_start = atmo_start + ATMO_LEN * 2;

                    if detected.contains(&(map_start as u32)) {
                        offset += 2;
                        continue;
                    }

                    if self.validate_boost_limiter_data(data, map_start, RPM_LEN, ATMO_LEN) {
                        if let Some(template) = MAP_TEMPLATES.iter().find(|t| t.name == "Boost Limiter") {
                            log::debug!("🔧 [Boost Limiter Type B] Found 10x10 @ 0x{:X}", map_start);
                            let map = self.create_detected_map(
                                "Boost Limiter",
                                map_start,
                                RPM_LEN,
                                ATMO_LEN,
                                offset,
                                atmo_start,
                                0.85,
                                template,
                            );
                            maps.push(map);
                            offset = map_start + MAP_SIZE - 2;
                        }
                    }
                }
            }
            offset += 2;
        }

        maps
    }

    /// Validate Boost Limiter map data
    fn validate_boost_limiter_data(&self, data: &[u8], map_start: usize, rows: usize, cols: usize) -> bool {
        let values: Vec<f64> = self.read_values_be_signed(data, map_start, rows * cols)
            .iter()
            .map(|&v| v as f64)
            .collect();

        let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // Boost limiter values in mbar, typical 1100-4000
        min_val >= 800.0 && max_val <= 5000.0 && max_val >= 1800.0
    }

    // ========== N75 DUTY CYCLE DETECTION (SIGNATURE-BASED) ==========
    // N75 maps have signature pattern: [00][rows][00][cols][00 00] before axes
    // Common variants: 16x10, 16x12, 16x11
    // Structure: Signature + RPM axis (rows values) + [00 00] + IQ axis (cols values) + Map data

    /// Detect N75 Duty Cycle maps by signature pattern
    /// Signature: [00 10 00 XX] where XX is the column count.
    /// Real EDC16U1 dumps use 13 cols on 1.9L PD and 12 cols on 2.0L PD;
    /// older 16xN variants (10, 11) are kept for compatibility with other
    /// EDC16 derivatives that share this code path.
    fn detect_n75_by_signature(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let (start, end) = self.get_data_section_range(data.len());

        let mut offset = start;

        while offset + 100 <= end.min(data.len()) {
            // Check for N75 signature pattern: [00 10 00 XX]
            if offset + 4 <= data.len() &&
               data[offset] == 0x00 && data[offset + 1] == 0x10 &&
               data[offset + 2] == 0x00 {
                let cols = data[offset + 3] as usize;

                // Valid N75 cols: 10-13 (covers 16x10, 16x11, 16x12 and 16x13).
                if cols >= 10 && cols <= 13 {
                    let rows = 16;
                    let sig_offset = offset;

                    // After signature: RPM axis (16 values = 32 bytes)
                    let rpm_axis_start = sig_offset + 4;
                    if rpm_axis_start + rows * 2 + 2 + cols * 2 > data.len() {
                        offset += 2;
                        continue;
                    }

                    // Read and validate RPM axis. Le V10 016B a un N75 dont
                    // le RPM démarre à 0 (0xF95B6) — accepté SEULEMENT si
                    // les données montent en vrai duty (max ≥ 3000), pour ne
                    // pas gober les boost targets (rpm 0.., data ≤ 2400).
                    let rpm_axis = self.read_values_be_unsigned(data, rpm_axis_start, rows);
                    let rpm_zero_start = rpm_axis[0] == 0;
                    let rpm_ok = if rpm_zero_start {
                        let mut shifted = rpm_axis.clone();
                        shifted[0] = 1; // contourne le rejet first<1
                        self.is_valid_n75_rpm_axis(&shifted)
                    } else {
                        self.is_valid_n75_rpm_axis(&rpm_axis)
                    };
                    if !rpm_ok {
                        offset += 2;
                        continue;
                    }

                    // After RPM axis: IQ axis immediately follows (no padding)
                    // Note: First IQ value is often 0 (0x0000), not padding
                    let iq_axis_start = rpm_axis_start + rows * 2;

                    // Read and validate IQ axis
                    let iq_axis = self.read_values_be_unsigned(data, iq_axis_start, cols);
                    if !self.is_valid_n75_iq_axis(&iq_axis) {
                        offset += 2;
                        continue;
                    }

                    // Map data starts after IQ axis
                    let map_start = iq_axis_start + cols * 2;
                    let map_size = rows * cols * 2;

                    if map_start + map_size > data.len() {
                        offset += 2;
                        continue;
                    }

                    // Skip if already detected
                    if detected.contains(&(map_start as u32)) {
                        offset = map_start + map_size;
                        continue;
                    }

                    // Validate map data (N75 duty cycle values). Si le RPM
                    // démarre à 0, exiger un vrai plein-duty (≥ 3000).
                    let data_ok = self.validate_n75_data(&data[map_start..map_start + map_size])
                        && (!rpm_zero_start || {
                            let vals = self.read_values_be_unsigned(data, map_start, rows * cols);
                            vals.iter().any(|&v| v >= 3000)
                        });
                    if data_ok {
                        log::debug!("✅ [EDC16] Found N75 Duty Cycle ({}x{}) at 0x{:X}", rows, cols, map_start);

                        let mut map = DetectedMap::new(
                            map_start as u32,
                            map_size,
                            MapDimensions::TwoDimensional { rows, cols },
                            DataType::Int16,
                        );

                        map.name = Some("N75 Duty Cycle".to_string());
                        map.category = Some(MapCategory::TurboBoostPressureControl.display_name().to_string());
                        map.unit = Some("%".to_string());
                        map.correction_factor = Some(0.01); // 0.01 (pas 100/8192) : vérifié au banc — « no gear » plates à 7500 brut = 75 %, plateaux à 8000 = 80 % ; l'U34 utilisait déjà 0.01
                        map.y_axis_address = Some(rpm_axis_start as u32);
                        map.x_axis_address = Some(iq_axis_start as u32);
                        map.y_axis_correction = Some(1.0);
                        map.x_axis_correction = Some(0.01);
                        map.y_label = Some("rpm".to_string());
                        map.x_label = Some("mg/stroke".to_string());
                        map.confidence = 0.92;

                        maps.push(map);
                        offset = map_start + map_size;
                        continue;
                    }
                }
            }
            offset += 2;
        }

        log::debug!("🔧 [EDC16] N75 signature detection: found {} maps", maps.len());
        maps
    }

    /// Axe « croissant avec encoches » : tolère jusqu'à `max_dips` marches
    /// descendantes (fichiers TUNÉS : points d'axe édités à la baisse, ex.
    /// 016FF 371366 : 1000→994/894/844 dans SOI/N75/boost/durations —
    /// WinOLS accepte ces axes, nos validateurs stricts rejetaient tout le
    /// milieu de calibration de ces fichiers).
    fn axis_ascending_tolerant(values: &[u16], max_dips: usize) -> bool {
        if values.len() < 2 {
            return true;
        }
        let mut dips = 0;
        for i in 1..values.len() {
            if values[i] < values[i - 1] {
                dips += 1;
                if dips > max_dips {
                    return false;
                }
            }
        }
        // net croissant malgré les encoches
        values[values.len() - 1] > values[0]
    }

    /// Validate N75 RPM axis (16 values, 600-5500 RPM range)
    fn is_valid_n75_rpm_axis(&self, values: &[u16]) -> bool {
        if values.len() != 16 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // N75 RPM: starts 1-1500, ends 4000-5500
        // First value must be at least 1 (reject maps starting at 0)
        if first < 1 || first > 1500 {
            return false;
        }
        if last < 3500 || last > 5500 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Validate N75 IQ axis (10-12 values, 0-7000 mg/stroke range)
    fn is_valid_n75_iq_axis(&self, values: &[u16]) -> bool {
        if values.len() < 10 || values.len() > 13 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // N75 IQ: starts 0-500, ends >= 3500 (35 mg/stroke with factor 0.01)
        // Real N75 maps need IQ axis ending at least 35 mg/stroke
        if first > 700 {
            return false;
        }
        if last < 3500 || last > 8500 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Validate N75 map data (duty cycle 0-120%)
    fn validate_n75_data(&self, map_data: &[u8]) -> bool {
        if map_data.len() < 4 {
            return false;
        }

        // Read values as signed 16-bit BE
        let mut values: Vec<i16> = Vec::new();
        for i in (0..map_data.len()).step_by(2) {
            if i + 1 < map_data.len() {
                let val = ((map_data[i] as i16) << 8) | (map_data[i + 1] as i16);
                values.push(val);
            }
        }

        if values.is_empty() {
            return false;
        }

        // Échelle de VALIDATION uniquement (seuils historiquement calés sur 0.012207) — l'affichage utilise 0.01
        let factor = 0.012207;

        let min_val = values.iter().cloned().min().unwrap_or(0) as f64 * factor;
        let max_val = values.iter().cloned().max().unwrap_or(0) as f64 * factor;

        // Valid range: 0% to 120%
        if min_val < -5.0 || max_val > 125.0 {
            return false;
        }

        // Average should be reasonable for N75 (50-110%)
        let avg: f64 = values.iter().map(|&v| v as f64 * factor).sum::<f64>() / values.len() as f64;
        if avg < 40.0 || avg > 115.0 {
            return false;
        }

        true
    }

    /// Check if axis is linear (constant delta between values)
    /// Linear axes are typically false positives (like 0, 400, 800, 1200, ...)
    fn is_linear_axis(&self, values: &[u16]) -> bool {
        if values.len() < 3 {
            return false;
        }

        // Calculate first delta
        let first_delta = values[1] as i32 - values[0] as i32;
        if first_delta <= 0 {
            return false;
        }

        // Check if all deltas are within 10% of the first delta
        let tolerance = (first_delta as f64 * 0.1) as i32;
        for i in 2..values.len() {
            let delta = values[i] as i32 - values[i-1] as i32;
            if (delta - first_delta).abs() > tolerance.max(20) {
                return false;  // Not linear - has irregular spacing
            }
        }

        true  // All deltas are similar - linear axis
    }

    /// Detect SOI (Start of Injection) blocks
    /// SOI maps have a unique layout with shared axes AFTER the map data:
    /// [10 consecutive 14x16 maps (448 bytes each)][metadata][shared Y axis (16 RPM)][metadata][shared X axis (14 IQ)]
    ///
    /// IMPORTANT: Only COMPLETE blocks of 10 maps are valid SOI blocks.
    /// We track detected block ranges to avoid false positives from overlapping detections.
    fn detect_soi_blocks(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        const SOI_ROWS: usize = 16;
        const SOI_COLS: usize = 14;
        const SOI_MAP_SIZE: usize = SOI_ROWS * SOI_COLS * 2;  // 448 bytes
        const SOI_MAPS_PER_BLOCK: usize = 10;
        const SOI_BLOCK_SIZE: usize = SOI_MAP_SIZE * SOI_MAPS_PER_BLOCK;  // 4480 bytes

        // Track detected SOI block ranges to avoid overlapping detections
        let mut detected_ranges: Vec<(usize, usize)> = Vec::new();

        // Search for the metadata pattern after SOI blocks:
        // [16][RPM axis 16 values][14][IQ axis 14 values][10]
        let mut offset = start;
        let mut _found_16_count = 0;
        log::debug!("🔍 [EDC16U1] Starting SOI block detection from 0x{:X} to 0x{:X}", start, end);

        while offset + SOI_BLOCK_SIZE + 100 <= end.min(data.len()) {
            // Look for metadata pattern: value 16 followed by valid RPM axis
            let potential_rows = self.read_u16_be(data, offset);

            if potential_rows == SOI_ROWS as u16 {
                _found_16_count += 1;
                // Read potential RPM axis (16 values after the "16")
                let rpm_axis = self.read_values_be_unsigned(data, offset + 2, SOI_ROWS);

                if self.is_valid_soi_rpm_axis(&rpm_axis) {
                    log::debug!("🔧 [EDC16U1] Found valid RPM axis at 0x{:X}, first={}, last={}", offset + 2, rpm_axis[0], rpm_axis[15]);
                    // Check for cols count after RPM axis
                    let potential_cols = self.read_u16_be(data, offset + 2 + SOI_ROWS * 2);

                    if potential_cols == SOI_COLS as u16 {
                        // Read potential IQ axis (14 values)
                        let iq_axis = self.read_values_be_unsigned(data, offset + 2 + SOI_ROWS * 2 + 2, SOI_COLS);

                        if self.is_valid_soi_iq_axis(&iq_axis) {
                            log::debug!("🎯 [EDC16U1] Found SOI metadata at 0x{:X}: RPM axis valid, IQ axis valid", offset);
                            // Found valid SOI metadata! The maps are BEFORE this point
                            let metadata_offset = offset;

                            // Check for interleaved metadata pattern (lectureG5.bin style)
                            // Some files have individual [0010 000E][axes] before each of the last few maps
                            // Pattern: [0010][000E][RPM axis 32 bytes][IQ axis 28 bytes] = 64 bytes per interleaved metadata
                            const INTERLEAVED_METADATA_SIZE: usize = 64; // 2 + 32 + 2 + 28

                            // Count interleaved metadata blocks before the main metadata
                            // They appear at intervals of (448 + 64) = 512 bytes before main metadata
                            let mut interleaved_count = 0;
                            let mut check_offset = metadata_offset;

                            // Look backwards for interleaved metadata pattern [00 10][00 0E]
                            while check_offset > SOI_MAP_SIZE + INTERLEAVED_METADATA_SIZE {
                                let potential_interleaved = check_offset - SOI_MAP_SIZE - INTERLEAVED_METADATA_SIZE;
                                if potential_interleaved + 4 <= data.len() {
                                    let val1 = self.read_u16_be(data, potential_interleaved);
                                    let val2 = self.read_u16_be(data, potential_interleaved + 2);
                                    // Check for [0010][000E] pattern (16 rows, 14 cols)
                                    if val1 == 0x0010 && val2 == 0x000E {
                                        interleaved_count += 1;
                                        check_offset = potential_interleaved;
                                        if interleaved_count >= 5 {
                                            break; // Max 5 interleaved metadata
                                        }
                                    } else {
                                        break;
                                    }
                                } else {
                                    break;
                                }
                            }

                            // Calculate true block start accounting for interleaved metadata
                            // Total size = (10 - interleaved_count) * map_size + interleaved_count * (map_size + metadata_size)
                            // = 10 * map_size + interleaved_count * metadata_size
                            let total_block_size = SOI_BLOCK_SIZE + interleaved_count * INTERLEAVED_METADATA_SIZE;
                            let block_start = metadata_offset.saturating_sub(total_block_size);

                            if interleaved_count > 0 {
                                log::debug!("🔧 [EDC16] Found {} interleaved metadata blocks before SOI block", interleaved_count);
                            }
                            // Block end extends past metadata to cover:
                            // - Metadata itself (~70 bytes: [16][RPM 32][14][IQ 28])
                            // - SOI Selector structure (~50 bytes: [10][00 0A][temp 20][selector 20])
                            // - SOI Limiter that may follow
                            // Total protected zone = ~4700 bytes from block_start
                            let block_end = block_start + 4700;

                            // Check if this block overlaps with already detected blocks
                            // A new block is invalid if:
                            // 1. Its start is within an existing block's range
                            // 2. Its end overlaps with an existing block's start
                            let overlaps = detected_ranges.iter().any(|&(existing_start, existing_end)| {
                                // New block starts within existing block
                                (block_start >= existing_start && block_start < existing_end) ||
                                // New block ends within existing block
                                (block_end > existing_start && block_end <= existing_end) ||
                                // New block completely contains existing block
                                (block_start <= existing_start && block_end >= existing_end)
                            });

                            if overlaps {
                                log::debug!("🔧 [EDC16] Skipping SOI block at 0x{:X} - overlaps with existing block", block_start);
                                offset += 2;
                                continue;
                            }

                            // Verify we have 10 valid SOI maps before the metadata
                            let valid_block = self.validate_soi_block(data, block_start, SOI_MAPS_PER_BLOCK, SOI_ROWS, SOI_COLS);

                            if valid_block {
                                // Mark this block range as detected (extends to prevent overlaps)
                                detected_ranges.push((block_start, block_end));
                                log::debug!("🎯 [EDC16] Valid SOI block: 0x{:X} - 0x{:X} (10 maps sharing axes at 0x{:X})",
                                          block_start, block_start + SOI_BLOCK_SIZE, metadata_offset);
                                // Extract shared axis addresses
                                let y_axis_addr = offset + 2;
                                let x_axis_addr = offset + 2 + SOI_ROWS * 2 + 2;

                                // ========== DETECT DYNAMIC SOI MAPS BEFORE MAIN BLOCK ==========
                                // EDCMasterHP shows dynamic maps BEFORE the 10 main SOI maps
                                // These are marked with '*' in EDCMasterHP
                                // IMPORTANT: The dynamic maps may not be directly adjacent to the main block
                                // EDCMasterHP shows 4 dynamic maps at addresses like:
                                //   0x1D9778, 0x1D9938, 0x1D9AF8, 0x1D9CB8 (before block at 0x1D9D78)
                                // Distance from first dynamic to block_start = 0x600 = 1536 bytes
                                // So we need to scan far back: at least 5 maps (2240) + gap (256) = 2496 bytes

                                let max_pre_dynamic_maps = 5;
                                let mut pre_dynamic_maps: Vec<DetectedMap> = Vec::new();

                                // Start scanning from far before block_start to find dynamic maps
                                // Max distance: 5 maps × 448 bytes + 512 bytes margin = 2752 bytes
                                let scan_range = max_pre_dynamic_maps * SOI_MAP_SIZE + 512;
                                let scan_start = if block_start > scan_range { block_start - scan_range } else { 0 };
                                // Helper closure to validate SOI data
                                // SOI maps have specific characteristics:
                                // - Main SOI values: 200-700 raw (4.7° to 16.4°)
                                // - Dynamic SOI can have lower values: 50-700 raw
                                // - Max values can reach 1300 raw (30°)
                                // - No extreme values (>2000 or < -200)
                                let validate_soi_data = |map_data: &[u8]| -> bool {
                                    if map_data.iter().all(|&b| b == 0) {
                                        return false;
                                    }

                                    let mut values: Vec<i16> = Vec::with_capacity(SOI_MAP_SIZE / 2);
                                    let mut valid_soi_count = 0;
                                    let mut bad_value_count = 0;
                                    let total_values = SOI_MAP_SIZE / 2;

                                    for i in (0..SOI_MAP_SIZE).step_by(2) {
                                        let val = ((map_data[i] as i16) << 8) | (map_data[i + 1] as i16);
                                        values.push(val);

                                        // Acceptable SOI values: -200 to 1500 raw (wider range, max 35°)
                                        if val >= -200 && val <= 1500 {
                                            valid_soi_count += 1;
                                        }

                                        // Bad values: significantly out of SOI range
                                        if val > 2000 || val < -500 {
                                            bad_value_count += 1;
                                        }
                                    }

                                    // Check first row has no extreme values
                                    let first_row_bad_count = values.iter()
                                        .take(16)
                                        .filter(|&&v| v > 2000 || v < -500)
                                        .count();

                                    // Relaxed requirements for dynamic SOI:
                                    // - At least 50% in acceptable range (-200 to 1500)
                                    // - Less than 5% bad values overall
                                    // - First row: no extreme values
                                    valid_soi_count >= (total_values * 50 / 100)
                                        && bad_value_count <= (total_values * 5 / 100)
                                        && first_row_bad_count == 0
                                };

                                // Scan FORWARD from scan_start, looking for the first valid SOI map
                                // Then continue forward adding consecutive valid maps until we reach block_start
                                let mut first_dynamic_found = false;
                                let mut scan_pos = scan_start;

                                // Allow maps that start before block_start, even if they overlap slightly
                                // EDCMasterHP shows dynamic maps that may overlap with the main block by up to 256 bytes
                                while scan_pos < block_start {
                                    if scan_pos + SOI_MAP_SIZE > data.len() {
                                        break;
                                    }

                                    let map_data = &data[scan_pos..scan_pos + SOI_MAP_SIZE];
                                    let is_valid = validate_soi_data(map_data);

                                    if is_valid {
                                        if !first_dynamic_found {
                                            first_dynamic_found = true;
                                            log::debug!("🔍 [EDC16] First pre-dynamic SOI found at 0x{:X}", scan_pos);
                                        }

                                        // Don't add dynamic maps that overlap with the main block at all
                                        // The main SOI maps have priority for validation
                                        // A map at scan_pos would end at scan_pos + SOI_MAP_SIZE
                                        let overlaps_main_block = scan_pos + SOI_MAP_SIZE > block_start;

                                        if !detected.contains(&(scan_pos as u32)) && pre_dynamic_maps.len() < max_pre_dynamic_maps && !overlaps_main_block {
                                            let mut dyn_map = DetectedMap::new(
                                                scan_pos as u32,
                                                SOI_MAP_SIZE,
                                                MapDimensions::TwoDimensional { rows: SOI_ROWS, cols: SOI_COLS },
                                                DataType::Int16,
                                            );

                                            dyn_map.name = Some("Start of injection Dynamic".to_string());
                                            dyn_map.category = Some(MapCategory::StartOfInjection.display_name().to_string());
                                            dyn_map.unit = Some("deg CrS".to_string());
                                            dyn_map.correction_factor = Some(0.023437);
                                            dyn_map.confidence = 0.88;
                                            dyn_map.x_axis_address = Some(x_axis_addr as u32);
                                            dyn_map.y_axis_address = Some(y_axis_addr as u32);
                                            dyn_map.x_axis_correction = Some(0.01);
                                            dyn_map.y_axis_correction = Some(1.0);
                                            dyn_map.x_label = Some("mg/stroke".to_string());
                                            dyn_map.y_label = Some("rpm".to_string());
                                            dyn_map.y_axis_inverted = Some(true);

                                            pre_dynamic_maps.push(dyn_map);
                                        }
                                        // Move to next potential map position
                                        scan_pos += SOI_MAP_SIZE;
                                    } else if first_dynamic_found {
                                        // We found valid maps before but now hit invalid data
                                        // This could be a gap - try to skip it and continue
                                        // But limit the gap to 512 bytes
                                        let gap_start = scan_pos;
                                        let mut found_more = false;
                                        let mut gap_scan = scan_pos + 2;

                                        while gap_scan + SOI_MAP_SIZE <= block_start && gap_scan < gap_start + 512 {
                                            if gap_scan + SOI_MAP_SIZE <= data.len() {
                                                let gap_data = &data[gap_scan..gap_scan + SOI_MAP_SIZE];
                                                if validate_soi_data(gap_data) {
                                                    scan_pos = gap_scan;
                                                    found_more = true;
                                                    break;
                                                }
                                            }
                                            gap_scan += 2;
                                        }

                                        if !found_more {
                                            // No more valid maps found, stop searching
                                            break;
                                        }
                                    } else {
                                        // Haven't found first map yet, continue scanning
                                        scan_pos += 2;
                                    }
                                }

                                let pre_dynamic_count = pre_dynamic_maps.len();
                                for dyn_map in pre_dynamic_maps {
                                    maps.push(dyn_map);
                                }

                                if pre_dynamic_count > 0 {
                                    log::debug!("🎯 [EDC16] Found {} dynamic SOI maps BEFORE block at 0x{:X}", pre_dynamic_count, block_start);
                                }

                                // Create DetectedMap for each SOI map in the block
                                // For files with interleaved metadata, the last N maps have 64-byte headers
                                // Maps 0 to (9 - interleaved_count) are contiguous
                                // Maps (10 - interleaved_count) to 9 each have a 64-byte header before them
                                let contiguous_maps = SOI_MAPS_PER_BLOCK - interleaved_count;

                                for i in 0..SOI_MAPS_PER_BLOCK {
                                    let map_addr = if i < contiguous_maps {
                                        // Contiguous maps: simple offset from block_start
                                        block_start + i * SOI_MAP_SIZE
                                    } else {
                                        // Maps with interleaved metadata:
                                        // After all contiguous maps, then (map + metadata) pairs
                                        let interleaved_index = i - contiguous_maps;
                                        block_start + contiguous_maps * SOI_MAP_SIZE
                                            + interleaved_index * (INTERLEAVED_METADATA_SIZE + SOI_MAP_SIZE)
                                            + INTERLEAVED_METADATA_SIZE  // Skip the metadata header
                                    };

                                    if detected.contains(&(map_addr as u32)) {
                                        continue;
                                    }

                                    // For maps with interleaved metadata, use their individual axis addresses
                                    let (map_x_axis_addr, map_y_axis_addr) = if i >= contiguous_maps {
                                        // This map has its own metadata header right before it
                                        let header_offset = map_addr - INTERLEAVED_METADATA_SIZE;
                                        // Header structure: [0010][000E][RPM 32 bytes][IQ 28 bytes]
                                        // Y axis (RPM) starts at header_offset + 4
                                        // X axis (IQ) starts at header_offset + 4 + 32 = header_offset + 36
                                        (header_offset + 36, header_offset + 4)
                                    } else {
                                        (x_axis_addr, y_axis_addr)
                                    };

                                    let mut map = DetectedMap::new(
                                        map_addr as u32,
                                        SOI_MAP_SIZE,
                                        MapDimensions::TwoDimensional { rows: SOI_ROWS, cols: SOI_COLS },
                                        DataType::Int16,
                                    );

                                    // Name format: "Start of injection 00", "Start of injection 01", etc. (WinOLS style)
                                    map.name = Some(format!("Start of injection {:02}", i));
                                    map.category = Some(MapCategory::StartOfInjection.display_name().to_string());
                                    map.unit = Some("deg CrS".to_string());
                                    map.correction_factor = Some(0.023437);  // SOI factor
                                    map.confidence = 0.92;
                                    map.x_axis_address = Some(map_x_axis_addr as u32);
                                    map.y_axis_address = Some(map_y_axis_addr as u32);
                                    map.x_axis_correction = Some(0.01);  // IQ factor
                                    map.y_axis_correction = Some(1.0);   // RPM
                                    map.x_label = Some("mg/stroke".to_string());
                                    map.y_label = Some("rpm".to_string());
                                    map.y_axis_inverted = Some(true);

                                    maps.push(map);
                                }

                                // NOTE: SOI Selectors are now detected separately by detect_soi_selector_by_signature()
                                // which uses the 00 0A + temperature axis signature pattern

                                // ========== DETECT DYNAMIC SOI MAPS ==========
                                // After the 10 main SOI maps and metadata, there are typically 3-5 dynamic SOI maps
                                // These are "situational" maps used for specific conditions (air-limited, high torque demand, etc.)
                                // Structure after metadata:
                                //   +0: 00 10 (Y axis header)
                                //   +2: Y axis (16*2 = 32 bytes)
                                //   +34: 00 0E (X axis header)
                                //   +36: X axis (14*2 = 28 bytes)
                                //   +64: 00 0A (Temp axis header)
                                //   +66: Temp axis (10*2 = 20 bytes)
                                //   +86: Selector (10*2 = 20 bytes)
                                //   +106: Dynamic SOI maps start

                                let dynamic_start = metadata_offset + 2 + SOI_ROWS * 2 + 2 + SOI_COLS * 2 + 2 + 10 * 2 + 10 * 2;
                                // = metadata_offset + 2 + 32 + 2 + 28 + 2 + 20 + 20 = metadata_offset + 106

                                let max_dynamic_maps = 5;  // Maximum number of dynamic maps to look for
                                let mut dynamic_count = 0;
                                let mut found_first_valid = false;
                                let mut consecutive_zeros = 0;

                                for dyn_idx in 0..max_dynamic_maps {
                                    let dyn_map_addr = dynamic_start + dyn_idx * SOI_MAP_SIZE;

                                    if dyn_map_addr + SOI_MAP_SIZE > data.len() {
                                        break;
                                    }

                                    // Check if this map is all zeros (empty placeholder)
                                    let map_data = &data[dyn_map_addr..dyn_map_addr + SOI_MAP_SIZE];
                                    let all_zeros = map_data.iter().all(|&b| b == 0);

                                    if all_zeros {
                                        consecutive_zeros += 1;
                                        // Stop if we've seen 3+ consecutive zero maps after finding valid ones
                                        if found_first_valid && consecutive_zeros >= 3 {
                                            break;
                                        }
                                        continue;  // Skip zero maps but keep looking
                                    }

                                    consecutive_zeros = 0;  // Reset counter when we find non-zero data

                                    // Validate this looks like SOI data (values in reasonable range)
                                    // Dynamic SOI can have low values (50+) or zeros in parts of the map
                                    // Max values can reach 1300 raw (30°) based on StageX detection
                                    // We need:
                                    // 1. At least 40% of values in valid range (-200 to 1500)
                                    // 2. Very few "bad" values (above 2000 or below -500)
                                    let mut valid_soi_count = 0;
                                    let mut bad_value_count = 0;
                                    let total_values = SOI_MAP_SIZE / 2;  // 224 values
                                    for i in (0..SOI_MAP_SIZE).step_by(2) {
                                        let val = ((map_data[i] as i16) << 8) | (map_data[i + 1] as i16);
                                        // Valid SOI values: -200 to 1500 raw (wider range, max 35°)
                                        if val >= -200 && val <= 1500 {
                                            valid_soi_count += 1;
                                        }
                                        // Bad values: significantly out of SOI range
                                        if val > 2000 || val < -500 {
                                            bad_value_count += 1;
                                        }
                                    }
                                    // Relaxed: at least 40% valid and less than 5% bad
                                    let has_valid_soi_values = valid_soi_count >= (total_values * 40 / 100)
                                                            && bad_value_count <= (total_values * 5 / 100);

                                    if !has_valid_soi_values {
                                        // If we haven't found any valid yet, keep looking
                                        // If we have, then this might be the end of the sequence
                                        if found_first_valid {
                                            break;
                                        }
                                        continue;
                                    }

                                    found_first_valid = true;

                                    if detected.contains(&(dyn_map_addr as u32)) {
                                        continue;
                                    }

                                    let mut dyn_map = DetectedMap::new(
                                        dyn_map_addr as u32,
                                        SOI_MAP_SIZE,
                                        MapDimensions::TwoDimensional { rows: SOI_ROWS, cols: SOI_COLS },
                                        DataType::Int16,
                                    );

                                    // Dynamic SOI maps are numbered 01-05 (like in WinOLS)
                                    dyn_map.name = Some(format!("Start of injection (dynamic) {:02}", dyn_idx + 1));
                                    dyn_map.category = Some(MapCategory::StartOfInjection.display_name().to_string());
                                    dyn_map.unit = Some("deg CrS".to_string());
                                    dyn_map.correction_factor = Some(0.023437);  // Same SOI factor
                                    dyn_map.confidence = 0.88;  // Slightly lower confidence than main maps
                                    dyn_map.x_axis_address = Some(x_axis_addr as u32);
                                    dyn_map.y_axis_address = Some(y_axis_addr as u32);
                                    dyn_map.x_axis_correction = Some(0.01);  // IQ factor
                                    dyn_map.y_axis_correction = Some(1.0);   // RPM
                                    dyn_map.x_label = Some("mg/stroke".to_string());
                                    dyn_map.y_label = Some("rpm".to_string());
                                    dyn_map.y_axis_inverted = Some(true);

                                    maps.push(dyn_map);
                                    dynamic_count += 1;
                                }

                                if dynamic_count > 0 {
                                    log::debug!("🎯 [EDC16U1] Found {} dynamic SOI maps after block at 0x{:X}", dynamic_count, block_start);

                                    // ========== DETECT SOI DYNAMIC SELECTOR (EDC16U1 specific) ==========
                                    // After the 5 SOI Dynamic maps, there's a metadata structure and Dynamic Selector
                                    // Structure: [00 10][RPM 32][00 0E][IQ 28][00 05][Temp 10][Selector 10]
                                    // Total metadata size: 2 + 32 + 2 + 28 + 2 + 10 = 76 bytes
                                    // Dynamic Selector is 5 values (10 bytes)
                                    let dynamic_metadata_start = dynamic_start + dynamic_count * SOI_MAP_SIZE;
                                    let dynamic_selector_addr = dynamic_metadata_start + 76; // 2+32+2+28+2+10 = 76

                                    if dynamic_selector_addr + 10 <= data.len() {
                                        // Check for 00 05 header (5 values)
                                        let header_offset = dynamic_metadata_start + 64; // 2+32+2+28 = 64
                                        if data[header_offset] == 0x00 && data[header_offset + 1] == 0x05 {
                                            // Read temperature axis (5 values, Big-Endian)
                                            let temp_axis_start = header_offset + 2;
                                            let mut temp_valid = true;
                                            let mut temp_values: Vec<u16> = Vec::with_capacity(5);

                                            for i in 0..5 {
                                                let val = ((data[temp_axis_start + i * 2] as u16) << 8) |
                                                         (data[temp_axis_start + i * 2 + 1] as u16);
                                                temp_values.push(val);
                                                // Temperature range: 2300-3800 (Kelvin * 10)
                                                if val < 2300 || val > 3800 {
                                                    temp_valid = false;
                                                    break;
                                                }
                                            }

                                            if temp_valid {
                                                // Read selector values (5 values, Little-Endian)
                                                let mut selector_values: Vec<u16> = Vec::with_capacity(5);
                                                for i in 0..5 {
                                                    let val = ((data[dynamic_selector_addr + i * 2 + 1] as u16) << 8) |
                                                             (data[dynamic_selector_addr + i * 2] as u16);
                                                    selector_values.push(val);
                                                }

                                                // Validate selector pattern: all values 0-4, non-decreasing
                                                let all_in_range = selector_values.iter().all(|&v| v <= 4);
                                                let monotonic = selector_values.windows(2).all(|w| w[1] >= w[0]);

                                                if all_in_range && monotonic {
                                                    log::debug!("🎯 [EDC16U1] Found SOI Dynamic Selector at 0x{:X}, values: {:?}",
                                                              dynamic_selector_addr, selector_values);

                                                    let mut dyn_sel_map = DetectedMap::new(
                                                        dynamic_selector_addr as u32,
                                                        10, // 5 * 2 bytes
                                                        MapDimensions::OneDimensional { length: 5 },
                                                        DataType::UInt16,
                                                    );
                                                    dyn_sel_map.name = Some("SOI Dynamic Selector".to_string());
                                                    dyn_sel_map.category = Some(MapCategory::StartOfInjection.display_name().to_string());
                                                    dyn_sel_map.unit = Some("[-]".to_string());
                                                    dyn_sel_map.correction_factor = Some(1.0);
                                                    dyn_sel_map.offset = Some(0.0);
                                                    dyn_sel_map.confidence = 0.92;
                                                    dyn_sel_map.x_axis_address = Some(temp_axis_start as u32);
                                                    dyn_sel_map.x_label = Some("Water temp [°C]".to_string());
                                                    dyn_sel_map.x_axis_correction = Some(0.1);
                                                    dyn_sel_map.x_axis_offset = Some(-273.14);
                                                    dyn_sel_map.is_little_endian = Some(true);

                                                    maps.push(dyn_sel_map);
                                                }
                                            }
                                        }
                                    }
                                }

                                // Skip past this block including dynamic maps and metadata
                                let skip_to = dynamic_start + dynamic_count * SOI_MAP_SIZE + 86; // 76 (metadata) + 10 (selector)
                                offset = skip_to.max(offset + SOI_MAP_SIZE);
                                continue;
                            }
                        }
                    }
                }
            }
            offset += 2;
        }

        log::debug!("🔧 [EDC16U1] SOI block detection complete: found {} maps, scanned {} potential '16' headers", maps.len(), _found_16_count);
        maps
    }

    /// Read a single u16 Big-Endian value
    fn read_u16_be(&self, data: &[u8], offset: usize) -> u16 {
        if offset + 1 < data.len() {
            ((data[offset] as u16) << 8) | (data[offset + 1] as u16)
        } else {
            0
        }
    }

    /// Validate SOI RPM axis (100-5000 range, 16 values)
    fn is_valid_soi_rpm_axis(&self, values: &[u16]) -> bool {
        if values.len() != 16 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // SOI RPM: starts 50-1200, ends 4200-5600 (relaxed for more variants)
        if first < 50 || first > 1200 {
            return false;
        }
        if last < 4200 || last > 5600 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Validate SOI IQ axis (0-5500 range, 14 values)
    fn is_valid_soi_iq_axis(&self, values: &[u16]) -> bool {
        if values.len() != 14 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // SOI IQ: starts 0-300, ends 4000-7000 (relaxed for more variants)
        if first > 300 {
            return false;
        }
        if last < 4000 || last > 7000 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Validate a block of consecutive SOI maps
    fn validate_soi_block(&self, data: &[u8], block_start: usize, count: usize, rows: usize, cols: usize) -> bool {
        let map_size = rows * cols * 2;

        // Check we have enough data
        if block_start + count * map_size > data.len() {
            return false;
        }

        // Check block_start is valid (not negative wrap)
        if block_start < 0x30000 {
            return false;
        }

        // Validate each SOI map in the block
        let mut valid_maps = 0;
        for i in 0..count {
            let map_offset = block_start + i * map_size;
            let values: Vec<i16> = self.read_values_be_signed(data, map_offset, rows * cols);

            // SOI values: factor 0.023437, typical range -20 to +40 degrees
            // Raw values: -853 to +1706 (for -20 to +40 deg)
            let min_val = values.iter().cloned().min().unwrap_or(0);
            let max_val = values.iter().cloned().max().unwrap_or(0);

            // Valid SOI raw range: -2000 to +2500 (extended for tuned files)
            if min_val >= -2000 && max_val <= 2500 && max_val > -500 {
                valid_maps += 1;
            }
        }

        // At least 6 of 10 maps should be valid (relaxed)
        valid_maps >= count * 6 / 10
    }

    /// Detect SOI Limiter maps by signature pattern
    /// Structure: [14][11][RPM axis 14 vals][Timing axis 11 vals][Map 14x11]
    /// The signature [14][11] (0x000E 0x000B in Big-Endian) marks the start
    fn detect_soi_limiter_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        const RPM_LEN: usize = 14;  // Y axis (rows)
        const X_LEN: usize = 11;    // X axis (cols)
        const MAP_SIZE: usize = RPM_LEN * X_LEN * 2;  // 308 bytes

        // Total structure: 2 (14) + 2 (11) + 28 (RPM) + 22 (timing) + 308 (map) = 362 bytes
        let struct_size = 4 + RPM_LEN * 2 + X_LEN * 2 + MAP_SIZE;

        let mut offset = start;
        while offset + struct_size <= end.min(data.len()) {
            // Look for signature [14][11] (Big-Endian)
            let marker1 = self.read_u16_be(data, offset);
            let marker2 = self.read_u16_be(data, offset + 2);

            if marker1 == 14 && marker2 == 11 {
                // Found potential SOI Limiter signature
                let rpm_axis_start = offset + 4;
                let rpm_axis = self.read_values_be_unsigned(data, rpm_axis_start, RPM_LEN);

                // SOI Limiter RPM: starts 300-500, ends 4500-5500
                if self.is_valid_soi_limiter_rpm_axis(&rpm_axis) {
                    let x_axis_start = rpm_axis_start + RPM_LEN * 2;
                    let x_axis = self.read_values_be_unsigned(data, x_axis_start, X_LEN);

                    // SOI Limiter X axis: 11 timing values, range 2400-3600
                    if self.is_valid_soi_limiter_x_axis(&x_axis) {
                        let map_start = x_axis_start + X_LEN * 2;

                        if detected.contains(&(map_start as u32)) {
                            offset += 2;
                            continue;
                        }

                        // Validate map data
                        if self.validate_soi_limiter_data(data, map_start, RPM_LEN, X_LEN) {
                            log::debug!("🎯 [EDC16] Found SOI Limiter at 0x{:X} (signature at 0x{:X})",
                                      map_start, offset);

                            let mut map = DetectedMap::new(
                                map_start as u32,
                                MAP_SIZE,
                                MapDimensions::TwoDimensional { rows: RPM_LEN, cols: X_LEN },
                                DataType::UInt16,
                            );

                            map.name = Some("SOI Limiter".to_string());
                            map.category = Some(MapCategory::StartOfInjection.display_name().to_string());
                            map.unit = Some("deg CrS".to_string());
                            map.correction_factor = Some(0.023437);
                            map.confidence = 0.92;
                            map.x_axis_address = Some(x_axis_start as u32);
                            map.y_axis_address = Some(rpm_axis_start as u32);
                            // X axis: Temperature in Kelvin*10
                            // Formula: °C = (rawValue * 0.1) - 273.14
                            // Example: 2731 * 0.1 - 273.14 = 0°C
                            map.x_axis_correction = Some(0.1);
                            map.x_axis_offset = Some(-273.14);
                            map.y_axis_correction = Some(1.0);       // RPM
                            map.x_label = Some("Water temp [°C]".to_string());
                            map.y_label = Some("rpm".to_string());

                            maps.push(map);
                            offset = map_start + MAP_SIZE;
                            continue;
                        }
                    }
                }
            }
            offset += 2;
        }

        log::debug!("🔧 [EDC16] SOI Limiter detection: found {} maps", maps.len());
        maps
    }

    /// Validate SOI Limiter RPM axis (400-5000, 14 values)
    fn is_valid_soi_limiter_rpm_axis(&self, values: &[u16]) -> bool {
        if values.len() != 14 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // SOI Limiter RPM: starts 300-500, ends 4500-5500 — le Caddy SDI
        // (03G906016N, 0xE3C3A) démarre à 800 et finit à 4000
        if first < 300 || first > 1000 {
            return false;
        }
        if last < 3800 || last > 5500 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Validate SOI Limiter X axis (11 values, Temperature in Kelvin*10)
    /// Temperature range: typically 2231 (-50°C) to 3731 (100°C)
    fn is_valid_soi_limiter_x_axis(&self, values: &[u16]) -> bool {
        if values.len() != 11 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // X axis range: Temperature in Kelvin*10
        // First value: typically 2231-2600 (about -50°C to +13°C)
        // Last value: typically 3400-3800 (about 67°C to 107°C)
        if first < 2200 || first > 2700 {
            return false;
        }
        if last < 3300 || last > 3900 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Validate SOI Limiter map data
    fn validate_soi_limiter_data(&self, data: &[u8], map_start: usize, rows: usize, cols: usize) -> bool {
        let values = self.read_values_be_unsigned(data, map_start, rows * cols);

        // SOI Limiter values: typical range 500-1500 (raw, unsigned)
        let min_val = values.iter().cloned().min().unwrap_or(0);
        let max_val = values.iter().cloned().max().unwrap_or(0);

        // Valid range
        min_val >= 400 && max_val <= 2000 && max_val > 600
    }

    /// Check Boost Target IQ axis (0-7500 in steps of 500-800)
    fn is_valid_boost_target_iq_axis(&self, values: &[u16]) -> bool {
        if values.len() != 10 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // IQ axis: starts 0-1500 (some files start at 1000), ends 3500-8000
        if first > 1500 {
            return false;
        }
        if last < 3500 || last > 8000 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        if !Self::axis_ascending_tolerant(values, 2) {
            return false;
        }

        // Check roughly equal steps (should be ~500-800)
        let step = (last - first) / (values.len() as u16 - 1);
        if step < 300 || step > 900 {
            return false;
        }

        true
    }

    /// Check Boost Limiter RPM axis Type A (11 values, 1400-4500 range)
    fn is_valid_boost_limiter_rpm_axis_11(&self, values: &[u16]) -> bool {
        if values.len() != 11 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // Boost Limiter RPM: 1400-1800 first, 4200-4800 last
        if first < 1200 || first > 2000 {
            return false;
        }
        if last < 4000 || last > 5000 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Check Boost Limiter RPM axis Type B (10 values, 1500-4500 range)
    /// Pattern from passat: 1500, 1750, 1900, 2250, 2500, 3000, 3500, 4000, 4250, 4500
    fn is_valid_boost_limiter_rpm_axis_10(&self, values: &[u16]) -> bool {
        if values.len() != 10 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // Type B RPM: 1400-1700 first, 4300-4700 last
        if first < 1400 || first > 1800 {
            return false;
        }
        if last < 4300 || last > 4700 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        if !Self::axis_ascending_tolerant(values, 2) {
            return false;
        }

        // Must NOT be linear (this is a key distinguishing feature)
        if self.is_linear_axis(values) {
            return false;
        }

        true
    }

    /// Check Atmospheric pressure axis (600-1100 mbar)
    fn is_valid_atmospheric_axis(&self, values: &[u16]) -> bool {
        if values.len() != 10 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // Atmospheric pressure: 600-700 first, 1000-1150 last — borne sur
        // le MAX de l'axe : les fichiers tunés peuvent éditer le dernier
        // point à la baisse (016FF : 1024→844)
        if first < 550 || first > 750 {
            return false;
        }
        let hi = *values.iter().max().unwrap();
        if hi < 950 || hi > 1200 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Check if values form a valid RPM axis (standard type: starts 400-1600, ends 3800-6200)
    fn is_valid_rpm_axis(&self, values: &[u16]) -> bool {
        if values.len() < 10 || values.len() > 20 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // Standard RPM axis: first value 400-1600, last value 3800-6200
        if first < 400 || first > 1600 {
            return false;
        }
        if last < 3800 || last > 6200 {
            return false;
        }

        // Check monotonically increasing with reasonable deltas
        let mut prev = values[0];
        for i in 1..values.len() {
            if values[i] <= prev {
                return false;
            }
            let delta = values[i] - prev;
            if delta < 50 || delta > 750 {
                return false;
            }
            prev = values[i];
        }

        true
    }

    /// Identify map type based on axis patterns and dimensions
    fn identify_map_type_by_axes(&self, _rpm_axis: &[u16], x_axis: &[u16], rpm_len: usize, x_len: usize) -> Option<(&'static str, usize, usize)> {
        // Based on EDC16U1_Detection_Guide.md signatures:
        // Dimensions -> Map Type (rows = rpm_len, cols = x_len)

        let rows = rpm_len;
        let cols = x_len;

        // Check X axis type to narrow down map identification
        let x_type = self.identify_x_axis_type(x_axis);

        match (rows, cols, x_type) {
            // Driver Wish: 16x8, X axis is Pedal (0-100%)
            (16, 8, XAxisType::Pedal) => Some(("Driver Wish", rows, cols)),
            (8, 16, XAxisType::Pedal) => Some(("Driver Wish", cols, rows)),  // Swapped

            // N75 Duty Cycle: Now detected in Phase 5 with dedicated function
            // Skip here to avoid duplicates and false positives
            (16, 11, XAxisType::InjectionQty) |
            (16, 12, XAxisType::InjectionQty) |
            (16, 13, XAxisType::InjectionQty) => {
                // These could be N75 or other maps - skip, N75 detected in Phase 5
                None
            },

            // Start of Injection: 16x14, X axis is IQ
            // Now detected in Phase 6 with shared axes - skip to avoid duplicates
            (16, 14, XAxisType::InjectionQty) |
            (14, 16, XAxisType::InjectionQty) => {
                None  // SOI detected in Phase 6 with block pattern
            },

            // Boost Target: 15x10, X axis is IQ (16x10 N75 detected in Phase 5)
            (15, 10, XAxisType::InjectionQty) => {
                Some(("Boost Target", rows, cols))
            },
            // 16x10 with IQ axis could be N75 - skip, detected in Phase 5
            (16, 10, XAxisType::InjectionQty) => {
                None  // N75 Type B detected in Phase 5
            },

            // EGR: 16x13, 13x16, X axis is IQ (higher range)
            (16, 13, XAxisType::InjectionQtyHigh) |
            (13, 16, XAxisType::InjectionQtyHigh) => Some(("EGR", rows, cols)),

            // Smoke Limiter: 16x13, X axis is AirMass
            (16, 13, XAxisType::AirMass) => Some(("Smoke Limiter", rows, cols)),

            // Note: Torque to IQ maps are detected by signature-based detection
            // (detect_torque_to_iq_by_signature in PHASE 6) - not here to avoid duplicates

            // Boost Limiter: 10x10, 11x10, X axis is Atmospheric Pressure
            (10, 10, XAxisType::AtmosphericPressure) |
            (11, 10, XAxisType::AtmosphericPressure) |
            (10, 11, XAxisType::AtmosphericPressure) => Some(("Boost Limiter", rows, cols)),

            // Note: Injection Duration maps are detected by signature-based detection
            // (detect_duration_by_signature) - not here to avoid duplicates

            _ => None  // Unknown combination
        }
    }

    /// Identify X axis type based on value patterns
    fn identify_x_axis_type(&self, values: &[u16]) -> XAxisType {
        if values.is_empty() {
            return XAxisType::Unknown;
        }

        let first = values[0];
        let last = values[values.len() - 1];
        let len = values.len();

        // Check if monotonically increasing
        let mut is_monotonic = true;
        for i in 1..values.len() {
            if values[i] <= values[i-1] {
                is_monotonic = false;
                break;
            }
        }

        if !is_monotonic {
            return XAxisType::Unknown;
        }

        // Pedal: 0-10000 (0-100% with factor 0.01), 6-10 values
        // EDC16U1: typically 8 values from 0 to 10000
        if len >= 6 && len <= 10 && first <= 500 && last >= 7500 && last <= 11000 {
            return XAxisType::Pedal;
        }

        // Torque: 0-4000 (0-400 Nm with factor 0.1), 14-18 values
        // EDC16U1: 16 values typically, 0 to ~3360 (336 Nm)
        if len >= 14 && len <= 18 && first <= 150 && last >= 2000 && last <= 5000 {
            return XAxisType::Torque;
        }

        // Injection Qty (N75/SOI range): 0-7000 (0-70 mg with factor 0.01), 10-16 values
        // Covers N75, SOI, Boost Target
        if len >= 10 && len <= 16 && first <= 400 && last >= 1500 && last <= 8000 {
            return XAxisType::InjectionQty;
        }

        // Injection Qty (higher range for EGR): 0-6500, 13+ values
        if len >= 12 && first <= 500 && last >= 4000 && last <= 7000 {
            return XAxisType::InjectionQtyHigh;
        }

        // Air Mass: 2000-15000 (200-1500 mg air), 12-14 values typically
        if len >= 11 && len <= 15 && first >= 1200 && first <= 3500 && last >= 9000 && last <= 16000 {
            return XAxisType::AirMass;
        }

        // Atmospheric Pressure: 700-1100 mbar, 9-12 values
        if len >= 9 && len <= 13 && first >= 650 && first <= 950 && last >= 980 && last <= 1200 {
            return XAxisType::AtmosphericPressure;
        }

        XAxisType::Unknown
    }

    /// Create a DetectedMap with all metadata
    fn create_detected_map(
        &self,
        map_type: &str,
        map_address: usize,
        rows: usize,
        cols: usize,
        y_axis_address: usize,
        x_axis_address: usize,
        score: f64,
        template: &MapTemplate,
    ) -> DetectedMap {
        let map_size = rows * cols * 2;

        let mut map = DetectedMap::new(
            map_address as u32,
            map_size,
            MapDimensions::TwoDimensional { rows, cols },
            if template.signed { DataType::Int16 } else { DataType::UInt16 },
        );

        map.name = Some(template.name.to_string());
        map.category = Some(template.category.display_name().to_string());
        map.unit = Some(template.unit.to_string());
        map.correction_factor = Some(template.z_factor);
        map.confidence = (score + 0.15) as f32;  // Axis validation bonus
        map.x_axis_address = Some(x_axis_address as u32);
        map.y_axis_address = Some(y_axis_address as u32);

        // Set axis correction factors based on map type
        match map_type {
            "N75 Duty Cycle" => {
                map.x_axis_correction = Some(0.01);  // IQ: raw/100 = mg/stroke
                map.y_axis_correction = Some(1.0);   // RPM
                map.x_label = Some("mg/stroke".to_string());
                map.y_label = Some("rpm".to_string());
                map.y_axis_inverted = Some(true);
            },
            "Driver Wish" => {
                map.x_axis_correction = Some(0.01);  // Pedal: raw/100 = %
                map.y_axis_correction = Some(1.0);
                map.x_label = Some("%".to_string());
                map.y_label = Some("rpm".to_string());
                map.y_axis_inverted = Some(true);
            },
            "Torque to IQ Conversion" => {
                map.x_axis_correction = Some(0.1);   // Torque: raw/10 = Nm
                map.y_axis_correction = Some(1.0);
                map.x_label = Some("Nm".to_string());
                map.y_label = Some("rpm".to_string());
                map.y_axis_inverted = Some(false);
            },
            "Boost Target" => {
                map.x_axis_correction = Some(0.01);
                map.y_axis_correction = Some(1.0);
                map.x_label = Some("mg/stroke".to_string());
                map.y_label = Some("rpm".to_string());
                map.y_axis_inverted = Some(true);
            },
            // Boost Limiter : l'axe X est la pression ATMOSPHÉRIQUE en mbar
            // bruts (700-1100), pas une quantité injectée — le 0.01/mg/stroke
            // hérité de la Boost Target affichait 10.50 au lieu de 1050.
            "Boost Limiter" => {
                map.x_axis_correction = Some(1.0);
                map.y_axis_correction = Some(1.0);
                map.x_label = Some("mbar".to_string());
                map.y_label = Some("rpm".to_string());
                map.y_axis_inverted = Some(true);
            },
            "Start of Injection" => {
                map.x_axis_correction = Some(0.01);
                map.y_axis_correction = Some(1.0);
                map.x_label = Some("mg/stroke".to_string());
                map.y_label = Some("rpm".to_string());
                map.y_axis_inverted = Some(true);
            },
            "EGR" => {
                map.x_axis_correction = Some(0.01);
                map.y_axis_correction = Some(1.0);
                map.x_label = Some("mg/stroke".to_string());
                map.y_label = Some("rpm".to_string());
                map.y_axis_inverted = Some(true);
            },
            _ => {
                map.x_axis_correction = Some(1.0);
                map.y_axis_correction = Some(1.0);
            }
        }

        // Generate description from unit and axis labels
        // Format: "Unit | X: X_label | Y: Y_label"
        if let (Some(unit), Some(x_label), Some(y_label)) = (map.unit.as_ref(), map.x_label.as_ref(), map.y_label.as_ref()) {
            map.description = Some(format!("{} | X: {} | Y: {}", unit, x_label, y_label));
        }

        map
    }

    /// Validate Torque axis for Torque to IQ maps
    /// Real Torque axes: 0-4000 raw (0-400 Nm with factor 0.1), 14-18 values, monotonic
    fn validate_torque_axis(&self, values: &[u16]) -> bool {
        // Support multiple variants:
        // - 1.9L: 14-18 values
        // - 2.0L: 12-13 values
        if values.len() < 10 || values.len() > 20 {
            return false;  // Torque to IQ X axes have 10-20 values
        }

        // Torque axis should:
        // 1. Start at 0 (or near 0)
        // 2. End in reasonable range (2000-6000 raw = 200-600 Nm for 2.0L higher torque)
        // 3. Be monotonically increasing
        // 4. Have all values < 7000 (700 Nm max for 2.0L engines)

        // First value MUST be 0 (Torque starts at 0 Nm)
        // This is a strict requirement for Torque to IQ maps
        if values[0] != 0 {
            return false;  // Torque axis MUST start at 0
        }

        // Last value should be 2000-10500 (200-1050 Nm — le Touareg V10 a
        // un axe couple jusqu'à 10200)
        let last = values[values.len() - 1];
        if last < 2000 || last > 10500 {
            return false;  // Invalid range for Torque axis
        }

        // Croissant avec tolérance d'encoches (axes tunés) + borne haute
        if values.iter().any(|&v| v > 10500) {
            return false; // Out of Torque range (1050 Nm max, V10 tuned)
        }
        if !Self::axis_ascending_tolerant(values, 2) {
            return false;
        }

        true
    }

    /// Validate RPM axis for Torque to IQ maps (1000-5000 RPM)
    fn validate_rpm_axis_torque_iq(&self, values: &[u16]) -> bool {
        // Support multiple variants:
        // - 1.9L: 13-18 values
        // - 2.0L: 12-13 values
        if values.len() < 10 || values.len() > 20 {
            return false;  // Torque to IQ Y axis has 10-20 values
        }

        // RPM axis should:
        // 1. Start between 500-1500 RPM
        // 2. End between 4000-5500 RPM (wider range for variants)
        // 3. Be monotonically increasing

        if values[0] < 500 || values[0] > 1500 {
            return false;  // First value should be 500-1500 RPM
        }

        if values[values.len() - 1] < 4000 || values[values.len() - 1] > 5500 {
            return false;  // Last value should be 4000-5500 RPM
        }

        if values.iter().any(|&v| v > 6000) {
            return false; // RPM too high
        }
        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Torque to IQ detection by signature pattern
    /// Multiple variants:
    /// - 1.9L variant: Signature [00 0F 00 XX] where 0F = 15 rows, XX = cols (10-18)
    /// - 2.0L variant A: Signature [00 0C 00 0C] where 0C = 12 rows x 12 cols
    /// - 2.0L variant B: Signature [00 10 00 10] where 10 = 16 rows x 16 cols
    /// Structure: Signature -> RPM axis -> Torque axis -> Map data
    /// IMPORTANT: Only ONE Torque to IQ map exists per file
    /// Strategy: Find all candidates, return the one with LOWEST address (real map is in specific region)
    fn detect_torque_to_iq_by_signature(
        &self,
        data: &[u8],
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let (start, end) = self.get_data_section_range(data.len());
        let mut candidates: Vec<DetectedMap> = Vec::new();

        // Collect all valid candidates from all variants
        // Variant 1: 15xN (1.9L engines typically)
        if let Some(map) = self.detect_torque_to_iq_variant_single(data, start, end, 0x0F, 10, 18, detected) {
            candidates.push(map);
        }

        // Variant 2: 16x12-18 (VWPassat2l 16x16 ; Transporter R5 16x18 à
        // 0xEDCE8 — validateurs d'axes stricts + fenêtre d'adresses en aval)
        if let Some(map) = self.detect_torque_to_iq_variant_single(data, start, end, 0x10, 12, 18, detected) {
            candidates.push(map);
        }

        // Variant 3: 12x12 (2.0L engines typically)
        if let Some(map) = self.detect_torque_to_iq_variant_single(data, start, end, 0x0C, 12, 12, detected) {
            candidates.push(map);
        }

        // Variant 4: 13x13 (some 2.0L variants)
        if let Some(map) = self.detect_torque_to_iq_variant_single(data, start, end, 0x0D, 13, 13, detected) {
            candidates.push(map);
        }

        // Filter candidates to valid address range (zone calibration haute) :
        // 0x1D0000-0x1E0000 sur les fichiers 2 Mo, 0xD0000-0xE0000 sur les
        // U1 en 1 Mo (même layout décalé de -0x100000). La borne absolue
        // 2 Mo rejetait TOUT sur les fichiers 1 Mo.
        let (win_lo, win_hi): (u32, u32) = if data.len() >= 0x200000 {
            (0x1D0000, 0x1E0000)
        } else {
            // 1 Mo : fenêtre plus large — le T2IQ observé est à 0xE05C8
            // (016CC), soit au-delà du simple décalage -0x100000
            (0xD0000, 0xF0000)
        };
        let valid_candidates: Vec<DetectedMap> = candidates.into_iter()
            .filter(|m| m.address >= win_lo && m.address < win_hi)
            .collect();

        // Return the candidate with the LOWEST address within the valid range
        if let Some(best) = valid_candidates.into_iter().min_by_key(|m| m.address) {
            log::debug!("🔧 [EDC16] Torque to IQ signature detection: found 1 map at 0x{:X}", best.address);
            return vec![best];
        }

        log::debug!("🔧 [EDC16] Torque to IQ signature detection: found 0 maps");
        Vec::new()
    }

    /// Detect a single Torque to IQ map of a specific variant
    /// Signature: [00][rows][00][cols] followed by RPM axis, Torque axis, then map data
    /// Returns the first valid map found (only ONE exists per file)
    fn detect_torque_to_iq_variant_single(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        rows_byte: u8,
        min_cols: usize,
        max_cols: usize,
        detected: &HashSet<u32>,
    ) -> Option<DetectedMap> {
        let rows = rows_byte as usize;

        let mut offset = start;
        while offset + 100 <= end.min(data.len()) {
            // Check for signature: [00][rows][00][cols]
            if offset + 4 <= data.len() &&
               data[offset] == 0x00 && data[offset + 1] == rows_byte &&
               data[offset + 2] == 0x00 {
                let cols = data[offset + 3] as usize;

                // Valid cols within range
                if cols >= min_cols && cols <= max_cols {
                    // After signature: RPM axis (rows values)
                    let rpm_axis_start = offset + 4;
                    if rpm_axis_start + rows * 2 + cols * 2 > data.len() {
                        offset += 2;
                        continue;
                    }

                    // Read and validate RPM axis
                    let rpm_axis = self.read_values_be_unsigned(data, rpm_axis_start, rows);
                    if !self.validate_rpm_axis_torque_iq(&rpm_axis) {
                        offset += 2;
                        continue;
                    }

                    // After RPM axis: Torque axis (cols values)
                    let torque_axis_start = rpm_axis_start + rows * 2;
                    let torque_axis = self.read_values_be_unsigned(data, torque_axis_start, cols);
                    if !self.validate_torque_axis(&torque_axis) {
                        offset += 2;
                        continue;
                    }

                    // Map data starts after Torque axis
                    let map_start = torque_axis_start + cols * 2;
                    let map_size = rows * cols * 2;

                    if map_start + map_size > data.len() {
                        offset += 2;
                        continue;
                    }

                    // Skip if already detected
                    if detected.contains(&(map_start as u32)) {
                        offset = map_start + map_size;
                        continue;
                    }

                    // Validate map data (IQ values: 0-120 mg/stroke with factor 0.01)
                    if self.validate_torque_to_iq_data(&data[map_start..map_start + map_size]) {
                        log::debug!("✅ [EDC16] Found Torque to IQ Conversion ({}x{}) at 0x{:X}", rows, cols, map_start);

                        let mut map = DetectedMap::new(
                            map_start as u32,
                            map_size,
                            MapDimensions::TwoDimensional { rows, cols },
                            DataType::Int16,
                        );

                        map.name = Some("Torque to IQ Conversion".to_string());
                        map.category = Some(MapCategory::InjectionSystem.display_name().to_string());
                        map.unit = Some("mg/stroke".to_string());
                        map.correction_factor = Some(0.01);
                        map.y_axis_address = Some(rpm_axis_start as u32);
                        map.x_axis_address = Some(torque_axis_start as u32);
                        map.y_axis_correction = Some(1.0);   // RPM
                        map.x_axis_correction = Some(0.1);   // Torque Nm
                        map.y_label = Some("rpm".to_string());
                        map.x_label = Some("Nm".to_string());
                        map.confidence = 0.95;

                        // Return immediately - only ONE Torque to IQ map per file
                        return Some(map);
                    }
                }
            }
            offset += 2;
        }

        None
    }

    /// Validate Torque to IQ map data (IQ values 0-12000 raw = 0-120 mg/stroke)
    fn validate_torque_to_iq_data(&self, map_data: &[u8]) -> bool {
        if map_data.len() < 4 {
            return false;
        }

        // Read values as signed 16-bit BE
        let mut values: Vec<i16> = Vec::new();
        for i in (0..map_data.len()).step_by(2) {
            if i + 1 < map_data.len() {
                let val = ((map_data[i] as i16) << 8) | (map_data[i + 1] as i16);
                values.push(val);
            }
        }

        if values.is_empty() {
            return false;
        }

        // IQ values: factor 0.01, range 0-120 mg/stroke (raw 0-12000)
        let min_val = values.iter().cloned().min().unwrap_or(0);
        let max_val = values.iter().cloned().max().unwrap_or(0);

        // Values should be non-negative and in reasonable range
        if min_val < -500 || max_val > 15000 {
            return false;
        }

        // Should have some variation (not all zeros)
        if max_val - min_val < 100 {
            return false;
        }

        // Most values should be positive
        let positive_count = values.iter().filter(|&&v| v > 0).count();
        if positive_count < values.len() / 2 {
            return false;
        }

        true
    }

    /// Detect Cranking Torque Maps (also known as Start IQ maps)
    /// These are 9x9 maps used during engine cranking
    /// Signature: [00 09][00 09] followed by temperature axis, RPM axis, then map data
    /// There are typically 4 maps per file (2 per codeblock)
    fn detect_cranking_torque_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut local_detected = detected.clone();

        // Variant 1: 9x9 maps (most ECU files)
        let maps_9x9 = self.detect_cranking_variant(data, start, end, 9, 9, &local_detected);
        for map in &maps_9x9 {
            local_detected.insert(map.address);
        }
        maps.extend(maps_9x9);

        // Variant 2: 9x10 maps (2.0L engines like VWPassat2l)
        let maps_9x10 = self.detect_cranking_variant(data, start, end, 9, 10, &local_detected);
        for map in &maps_9x10 {
            local_detected.insert(map.address);
        }
        maps.extend(maps_9x10);

        // Variant 3: 8x9 (2.0L U1, Touran 016EH 0xEEDF4 : RPM 200-1400 sans
        // point 0) — même variante que sur U31/U34
        let maps_8x9 = self.detect_cranking_variant(data, start, end, 8, 9, &local_detected);
        for m in &maps_8x9 {
            local_detected.insert(m.address);
        }
        maps.extend(maps_8x9);

        // Variante 10x6 (Touareg R5 BL/016F : rpm 400..2500 × temp
        // 2531..2951, une seule map par fichier sur cette famille)
        let maps_10x6 = self.detect_cranking_variant(data, start, end, 10, 6, &local_detected);
        maps.extend(maps_10x6);

        log::debug!("🔧 [EDC16] Cranking Torque Map detection: found {} maps (9x9 + 9x10 + 8x9 + 10x6 variants)", maps.len());
        maps
    }

    /// Detect Cranking Torque Maps of a specific dimension
    fn detect_cranking_variant(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        rows: usize,
        cols: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let signature_rows = rows as u8;
        let signature_cols = cols as u8;

        let mut offset = start;
        while offset + 200 <= end.min(data.len()) {
            // Check for signature: [00][rows][00][cols]
            if offset + 4 <= data.len() &&
               data[offset] == 0x00 && data[offset + 1] == signature_rows &&
               data[offset + 2] == 0x00 && data[offset + 3] == signature_cols {

                // After signature: RPM axis (rows values) - cranking RPM values like 0, 200, 250, 280...
                let rpm_axis_start = offset + 4;
                if rpm_axis_start + rows * 2 + cols * 2 + rows * cols * 2 > data.len() {
                    offset += 2;
                    continue;
                }

                // Read and validate RPM axis (Y axis = rows)
                let rpm_axis = self.read_values_be_unsigned(data, rpm_axis_start, rows);
                if !self.validate_cranking_rpm_axis_generic(&rpm_axis) {
                    offset += 2;
                    continue;
                }

                // After RPM axis: Temperature axis (cols values) - Kelvin*10 values like 2431, 2530...
                let temp_axis_start = rpm_axis_start + rows * 2;
                let temp_axis = self.read_values_be_unsigned(data, temp_axis_start, cols);
                if !self.validate_cranking_temp_axis_generic(&temp_axis) {
                    offset += 2;
                    continue;
                }

                // Map data starts after Temperature axis
                let map_start = temp_axis_start + cols * 2;
                let map_size = rows * cols * 2;

                if map_start + map_size > data.len() {
                    offset += 2;
                    continue;
                }

                // Skip if already detected
                if detected.contains(&(map_start as u32)) {
                    offset = map_start + map_size;
                    continue;
                }

                // Validate map data (torque values: 0-3360 raw = 0-336 Nm)
                if self.validate_cranking_torque_data_generic(&data[map_start..map_start + map_size], rows * cols) {
                    log::debug!("✅ [EDC16] Found Cranking Torque Map ({}x{}) at 0x{:X}", rows, cols, map_start);

                    let mut map = DetectedMap::new(
                        map_start as u32,
                        map_size,
                        MapDimensions::TwoDimensional { rows, cols },
                        DataType::UInt16,
                    );

                    map.name = Some("Cranking Torque Map".to_string());
                    map.category = Some(MapCategory::EngineTorqueRequest.display_name().to_string());
                    map.unit = Some("Nm".to_string());
                    map.correction_factor = Some(0.1);  // Raw value * 0.1 = Nm
                    // Y axis = RPM (first axis), X axis = Temperature (second axis)
                    map.y_axis_address = Some(rpm_axis_start as u32);
                    map.x_axis_address = Some(temp_axis_start as u32);
                    map.y_axis_correction = Some(1.0);   // RPM (direct values)
                    map.x_axis_correction = Some(0.1);   // Temperature in Kelvin*10
                    map.x_axis_offset = Some(-273.14);   // Kelvin to Celsius
                    map.y_label = Some("rpm".to_string());
                    map.x_label = Some("°C".to_string());
                    map.confidence = 0.92;

                    maps.push(map);
                    offset = map_start + map_size;
                    continue;
                }
            }
            offset += 2;
        }

        maps
    }

    /// Validate RPM axis for Cranking Torque maps (generic version)
    /// RPM values during cranking: 0-1500 RPM (low cranking speeds)
    /// Typical observed: 0, 200, 250, 280, 450, 600, 1008, 1440, 1500
    fn validate_cranking_rpm_axis_generic(&self, values: &[u16]) -> bool {
        if values.is_empty() || values.len() > 20 {
            return false;
        }

        // Cranking RPM axis can start at 0 or near 0
        // First value should be 0 or low (engine just starting)
        if values[0] > 500 {
            return false;  // First RPM value should be low (0-500)
        }

        // Last value should be higher (engine reaching idle)
        // Typical last value: 1400-1500 RPM (2500 sur la variante 10x6
        // des Touareg R5)
        let last = *values.last().unwrap();
        if last < 400 || last > 2600 {
            return false;  // Last value should be 400-2600 RPM
        }

        // Values should be in reasonable cranking RPM range
        for &val in values {
            if val > 2500 {
                return false;
            }
        }

        // Should be monotonically increasing (or stable at some points)
        let mut prev = values[0];
        for &val in values.iter().skip(1) {
            if val < prev {
                return false;
            }
            prev = val;
        }

        true
    }

    /// Validate temperature axis for Cranking Torque maps (generic version)
    /// Temperature values in Kelvin*10: ~2400 to ~3700 (= -30°C to 100°C after conversion)
    /// Conversion: value * 0.1 - 273.14 = °C
    fn validate_cranking_temp_axis_generic(&self, values: &[u16]) -> bool {
        if values.is_empty() || values.len() > 20 {
            return false;
        }

        // Temperature axis in Kelvin*10
        // First value should be around 2400-2500 (about -30°C to -25°C)
        if values[0] < 2000 || values[0] > 3000 {
            return false;
        }

        // Last value should be higher (warm temperature)
        // Typical last value: ~3700 (2951 sur la variante 10x6 Touareg R5)
        let last = *values.last().unwrap();
        if last < 2900 || last > 4500 {
            return false;
        }

        // Should be monotonically increasing
        let mut prev = values[0];
        for &val in values.iter().skip(1) {
            if val <= prev {
                return false;
            }
            prev = val;
        }

        true
    }

    /// Validate Cranking Torque map data (generic version)
    /// Torque values: 0-3360 raw = 0-336 Nm
    fn validate_cranking_torque_data_generic(&self, map_data: &[u8], expected_values: usize) -> bool {
        let expected_bytes = expected_values * 2;
        if map_data.len() != expected_bytes {
            return false;
        }

        // Read values as unsigned 16-bit BE
        let mut values: Vec<u16> = Vec::new();
        for i in (0..map_data.len()).step_by(2) {
            if i + 1 < map_data.len() {
                let val = ((map_data[i] as u16) << 8) | (map_data[i + 1] as u16);
                values.push(val);
            }
        }

        if values.len() != expected_values {
            return false;
        }

        // Torque values: factor 0.1, range 0-336 Nm (raw 0-3360) — le
        // Touareg V10 monte à 8750-10200 (016B) en couple de démarrage
        let max_val = values.iter().cloned().max().unwrap_or(0);

        // Should have reasonable torque values (not all zeros, not too high)
        if max_val == 0 || max_val > 10500 {
            return false;
        }

        // Should have some variation (not constant)
        let min_val = values.iter().cloned().min().unwrap_or(0);
        if max_val == min_val {
            return false;
        }

        // Many values should be non-zero
        let non_zero = values.iter().filter(|&&v| v > 0).count();
        if non_zero < values.len() / 3 {
            return false;
        }

        true
    }

    /// Detect Turbo Boost Pressure Correction by Intake Air Temp maps
    /// Signature: [00 10][00 0A] = 16 rows x 10 cols
    /// Structure: Signature -> Boost axis (16 vals) -> Temp axis (10 vals) -> Map data
    /// Y axis: Turbo boost pressure (mbar), values ~500-3500
    /// X axis: Intake air temp (Kelvin*10), factor 0.1, offset -273.1 = °C
    fn detect_turbo_boost_correction_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        const ROWS: usize = 16;
        const COLS: usize = 10;

        let mut offset = start;
        while offset + 400 <= end.min(data.len()) {
            // Check for signature: [00][10][00][0A] = 16 rows x 10 cols
            if offset + 4 <= data.len() &&
               data[offset] == 0x00 && data[offset + 1] == 0x10 &&
               data[offset + 2] == 0x00 && data[offset + 3] == 0x0A {

                // After signature: Boost pressure axis (Y axis, 16 values)
                let boost_axis_start = offset + 4;
                if boost_axis_start + ROWS * 2 + COLS * 2 + ROWS * COLS * 2 > data.len() {
                    offset += 2;
                    continue;
                }

                // Read and validate Boost pressure axis
                let boost_axis = self.read_values_be_unsigned(data, boost_axis_start, ROWS);
                if !self.validate_boost_pressure_axis(&boost_axis) {
                    offset += 2;
                    continue;
                }

                // After Boost axis: Temperature axis (X axis, 10 values in Kelvin*10)
                let temp_axis_start = boost_axis_start + ROWS * 2;
                let temp_axis = self.read_values_be_unsigned(data, temp_axis_start, COLS);
                if !self.validate_intake_air_temp_axis(&temp_axis) {
                    offset += 2;
                    continue;
                }

                // Map data starts after Temperature axis
                let map_start = temp_axis_start + COLS * 2;
                let map_size = ROWS * COLS * 2;  // 16x10 = 160 values x 2 bytes = 320 bytes

                if map_start + map_size > data.len() {
                    offset += 2;
                    continue;
                }

                // Skip if already detected
                if detected.contains(&(map_start as u32)) {
                    offset = map_start + map_size;
                    continue;
                }

                // Validate map data (boost pressure correction values)
                if self.validate_boost_correction_data(&data[map_start..map_start + map_size]) {
                    log::debug!("✅ [EDC16] Found Turbo Boost Pressure Correction (16x10) at 0x{:X}", map_start);

                    let mut map = DetectedMap::new(
                        map_start as u32,
                        map_size,
                        MapDimensions::TwoDimensional { rows: ROWS, cols: COLS },
                        DataType::UInt16,
                    );

                    map.name = Some("Turbo Boost Pressure Correction".to_string());
                    map.category = Some(MapCategory::TurboBoostPressure.display_name().to_string());
                    map.unit = Some("mbar".to_string());
                    map.correction_factor = Some(1.0);  // Direct mbar values
                    // Y axis = Boost pressure (first axis), X axis = Intake air temp (second axis)
                    map.y_axis_address = Some(boost_axis_start as u32);
                    map.x_axis_address = Some(temp_axis_start as u32);
                    map.y_axis_correction = Some(1.0);   // Boost pressure in mbar
                    map.x_axis_correction = Some(0.1);   // Temperature in Kelvin*10
                    map.x_axis_offset = Some(-273.1);    // Kelvin to Celsius
                    map.y_label = Some("mbar".to_string());
                    map.x_label = Some("°C".to_string());
                    map.confidence = 0.90;

                    maps.push(map);
                    offset = map_start + map_size;
                    continue;
                }
            }
            offset += 2;
        }

        log::debug!("🔧 [EDC16] Turbo Boost Pressure Correction detection: found {} maps", maps.len());
        maps
    }

    /// Validate boost pressure axis for Turbo Boost Correction maps
    /// Boost pressure values: ~500-3500 mbar, monotonically increasing
    fn validate_boost_pressure_axis(&self, values: &[u16]) -> bool {
        if values.len() != 16 {
            return false;
        }

        // First value should be reasonable boost pressure (around 500-1000 mbar)
        if values[0] < 200 || values[0] > 1500 {
            return false;
        }

        // Last value should be higher (high boost, around 2000-3500 mbar)
        let last = *values.last().unwrap();
        if last < 1500 || last > 4000 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Validate intake air temperature axis
    /// Temperature values in Kelvin*10: ~2400-3400 (= -33°C to 67°C after conversion)
    fn validate_intake_air_temp_axis(&self, values: &[u16]) -> bool {
        if values.len() != 10 {
            return false;
        }

        // First value: cold air temp around 2400-2600 Kelvin*10 (= -33°C to -13°C)
        if values[0] < 2200 || values[0] > 2800 {
            return false;
        }

        // Last value: warm air temp around 3200-3500 Kelvin*10 (= 47°C to 77°C)
        let last = *values.last().unwrap();
        if last < 3000 || last > 3600 {
            return false;
        }

        // Values should be monotonically increasing
        let mut prev = values[0];
        for &val in values.iter().skip(1) {
            if val <= prev {
                return false;
            }
            prev = val;
        }

        true
    }

    /// Validate boost pressure correction map data
    /// Values should be in boost pressure range (550-3500 mbar typically)
    fn validate_boost_correction_data(&self, map_data: &[u8]) -> bool {
        const EXPECTED_BYTES: usize = 16 * 10 * 2;  // 320 bytes
        if map_data.len() != EXPECTED_BYTES {
            return false;
        }

        // Read values as unsigned 16-bit BE
        let mut values: Vec<u16> = Vec::new();
        for i in (0..map_data.len()).step_by(2) {
            if i + 1 < map_data.len() {
                let val = ((map_data[i] as u16) << 8) | (map_data[i + 1] as u16);
                values.push(val);
            }
        }

        if values.len() != 160 {
            return false;
        }

        // Boost correction values typically in range 550-3500 mbar
        let max_val = values.iter().cloned().max().unwrap_or(0);
        let min_val = values.iter().cloned().min().unwrap_or(0);

        // Should have reasonable boost values (not all zeros, not too high)
        if max_val < 500 || max_val > 4000 {
            return false;
        }

        // Minimum should also be reasonable
        if min_val < 100 || min_val > 2000 {
            return false;
        }

        // Should have some variation (not constant)
        if max_val == min_val {
            return false;
        }

        // Most values should be non-zero
        let non_zero = values.iter().filter(|&&v| v > 0).count();
        if non_zero < values.len() * 3 / 4 {
            return false;
        }

        true
    }

    /// Detect MAP Linearization (Map Sensor) - 1D map with 2 points
    /// Signature: [00 8B][03 E7] = 139, 999 followed by [00 02] count
    /// Structure: Signature -> Count (2) -> X axis (2 mV values) -> Y values (2 pressure values)
    /// X axis: mV values (factor 5), Y values: pressure in mbar
    fn detect_map_linearization(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        // Signature : [00 8B][borne mV haute]. La 2e valeur VARIE selon la
        // calibration du capteur (0x03E7=999 courant, 0x03E0=992 sur les U1
        // type 016CC) : on matche [00 8B] + une plage plausible, comme sur
        // l'EDC16U31 — la signature exacte ratait ~85 % des U1.
        let mut offset = start;
        while offset + 20 <= end.min(data.len()) {
            // Check for signature
            if offset + 12 <= data.len() &&
               data[offset] == 0x00 && data[offset + 1] == 0x8B &&
               {
                   let sig_hi = ((data[offset + 2] as u16) << 8) | (data[offset + 3] as u16);
                   (900..=1100).contains(&sig_hi)
               } {

                // After signature: Count should be [00 02]
                let count_offset = offset + 4;
                if data[count_offset] != 0x00 || data[count_offset + 1] != 0x02 {
                    offset += 2;
                    continue;
                }

                // X axis (2 mV values) starts at count_offset + 2
                let x_axis_start = count_offset + 2;
                let mv1 = ((data[x_axis_start] as u16) << 8) | (data[x_axis_start + 1] as u16);
                let mv2 = ((data[x_axis_start + 2] as u16) << 8) | (data[x_axis_start + 3] as u16);

                // Validate mV values: typically 50-200 for low, 800-1100 for high (raw / 5 = mV)
                // Raw values: ~250-1000 for 1250-5000mV range
                if mv1 > 500 || mv2 < 300 || mv2 > 1500 {
                    offset += 2;
                    continue;
                }

                // Y values (2 pressure values) start after X axis
                let y_values_start = x_axis_start + 4;
                let p1 = ((data[y_values_start] as u16) << 8) | (data[y_values_start + 1] as u16);
                let p2 = ((data[y_values_start + 2] as u16) << 8) | (data[y_values_start + 3] as u16);

                // Validate pressure values: typically 100-500 for low, 2000-4000 for high (mbar)
                if p1 > 1000 || p2 < 1500 || p2 > 5000 {
                    offset += 2;
                    continue;
                }

                // Skip if already detected
                if detected.contains(&(y_values_start as u32)) {
                    offset += 10;
                    continue;
                }

                log::debug!("✅ [EDC16] Found MAP Linearization at 0x{:X} (mV: {}, {} -> Pressure: {}, {})",
                    y_values_start, mv1 * 5, mv2 * 5, p1, p2);

                let mut map = DetectedMap::new(
                    y_values_start as u32,
                    4,  // 2 values x 2 bytes
                    MapDimensions::OneDimensional { length: 2 },
                    DataType::UInt16,
                );

                map.name = Some("MAP Linearization".to_string());
                map.category = Some("MAP Sensor".to_string());
                map.unit = Some("mbar".to_string());
                map.correction_factor = Some(1.0);  // Direct pressure values
                map.x_axis_address = Some(x_axis_start as u32);
                map.x_axis_correction = Some(5.0);  // mV factor
                map.x_label = Some("mV".to_string());
                map.confidence = 0.95;

                maps.push(map);
                offset += 12;  // Skip past this map
                continue;
            }
            offset += 2;
        }

        log::debug!("🔧 [EDC16] MAP Linearization detection: found {} maps", maps.len());
        maps
    }

    /// Detect Torque Limiter maps (EngPrt_trqLimP_MAP)
    ///
    /// EDC16U1 uses a distinctive signature `[00 00 00 3C 00 64][00 rows][00 cols]`
    /// (the 6 leading bytes are scaling/default constants, NOT the U34-style
    /// `[7F FF][7F FF][7F FF]` placeholders). Confirmed dimensions on real
    /// EDC16U1 dumps:
    /// - 20x3 for 1.9L PD variants
    /// - 21x3 for 2.0L PD variants
    ///
    /// Structure: [00 00 00 3C 00 64][00 rows][00 cols][Y axis][X axis][map data]
    /// X axis: RPM (0-5500), Y axis: Atmospheric pressure (600-1100 mbar)
    /// Values: Torque in Nm * 10 (factor 0.1, signed i16)
    ///
    /// The user expects exactly ONE Torque Limiter map per file, so we stop
    /// at the first valid match instead of stacking candidates.
    fn detect_torque_limiter_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        // Prefer the U1-specific signature first (20x3 / 21x3 ; 23x3-24x3 et
        // 23x4 sur R5/V10/Caddy — même signature [003C][0064], axe RPM plus
        // long, 4 lignes de pression sur le Transporter AJ).
        for (rows, cols) in [
            (3usize, 20usize), (3, 21), (3, 22), (3, 23), (3, 24),
            (4, 22), (4, 23), (4, 24),
        ] {
            let found = self.detect_torque_limiter_u1_sig(data, start, end, rows, cols, detected);
            if !found.is_empty() {
                log::debug!("🔧 [EDC16U1] Torque Limiter detected ({}x{}) at 0x{:X}", cols, rows, found[0].address);
                return found.into_iter().take(1).collect();
            }
        }

        // Fallback to legacy U34-style signature variants in case a file uses
        // the older layout. Take only the first valid match.
        for (rows, cols) in [(4usize, 20usize), (3, 21), (3, 22), (4, 22)] {
            let found = self.detect_torque_limiter_variant(data, start, end, rows, cols, detected);
            if !found.is_empty() {
                log::debug!("🔧 [EDC16U1] Torque Limiter (legacy sig) detected ({}x{}) at 0x{:X}", cols, rows, found[0].address);
                return found.into_iter().take(1).collect();
            }
        }

        log::debug!("🔧 [EDC16U1] Torque Limiter detection: no map found");
        Vec::new()
    }

    /// EDC16U1-specific Torque Limiter detector.
    /// Signature: `00 00 00 3C 00 64 00 <rows> 00 <cols>` followed by the
    /// pressure axis (rows*2 bytes), the RPM axis (cols*2 bytes), then the
    /// signed-i16 map data (rows*cols*2 bytes). All big-endian.
    fn detect_torque_limiter_u1_sig(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        rows: usize,
        cols: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let rows_byte = rows as u8;
        let cols_byte = cols as u8;
        let y_axis_size = rows * 2;
        let x_axis_size = cols * 2;
        let map_size = rows * cols * 2;
        let total_size = 10 + y_axis_size + x_axis_size + map_size;

        let mut offset = start;
        while offset + total_size <= end.min(data.len()) {
            if data[offset]     == 0x00 && data[offset + 1] == 0x00
                && data[offset + 2] == 0x00 && data[offset + 3] == 0x3C
                && data[offset + 4] == 0x00 && data[offset + 5] == 0x64
                && data[offset + 6] == 0x00 && data[offset + 7] == rows_byte
                && data[offset + 8] == 0x00 && data[offset + 9] == cols_byte
            {
                let y_axis_start = offset + 10;
                let x_axis_start = y_axis_start + y_axis_size;
                let map_start = x_axis_start + x_axis_size;

                if detected.contains(&(map_start as u32)) {
                    offset += 2;
                    continue;
                }

                let y_axis = self.read_values_be_unsigned(data, y_axis_start, rows);
                if !self.validate_torque_limiter_pressure_axis(&y_axis) {
                    offset += 2;
                    continue;
                }

                let x_axis = self.read_values_be_unsigned(data, x_axis_start, cols);
                if !self.validate_torque_limiter_rpm_axis(&x_axis) {
                    offset += 2;
                    continue;
                }

                let map_values = self.read_values_be_signed(data, map_start, rows * cols);
                if !self.validate_torque_limiter_data(&map_values) {
                    offset += 2;
                    continue;
                }

                let mut map = DetectedMap::new(
                    map_start as u32,
                    map_size,
                    MapDimensions::TwoDimensional { rows, cols },
                    DataType::Int16,
                );
                map.name = Some("Torque Limiter".to_string());
                map.category = Some(MapCategory::EngineTorqueLimiters.display_name().to_string());
                map.x_axis_address = Some(x_axis_start as u32);
                map.y_axis_address = Some(y_axis_start as u32);
                map.correction_factor = Some(0.1);
                map.unit = Some("Nm".to_string());
                map.description = Some(format!(
                    "Engine torque limiter by atmospheric pressure and RPM. Pressure: {}-{} mbar, RPM: {}-{}",
                    y_axis[0], y_axis[rows - 1], x_axis[0], x_axis[cols - 1]
                ));
                maps.push(map);
                return maps; // Only need the first match
            }
            offset += 2;
        }

        maps
    }

    /// Detect Launch Control / Torque Limiter by Vehicle Speed maps (10x10)
    /// Also detects the enable switch if present
    ///
    /// Signature: [00 05][00 00][00 32][00 00][00 0A][00 0A][00 00][01 90]
    /// - 0005 = Type ID (constant 5)
    /// - 0000 0032 = 50 decimal (parameter)
    /// - 000a 000a = 10x10 dimensions
    /// - 0000 0190 = 400 (max speed parameter)
    /// - Y axis (10 values): RPM (800-5000)
    /// - X axis (10 values): Vehicle speed (0-180 km/h, stored as value*100)
    /// - Data (100 values): Torque limit in Nm * 10
    ///
    /// Enable switch: Located at offset -10 from signature start
    /// Value 0x0064 (100) or 0x0000 indicates state
    fn detect_launch_control_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        // Dimensions: 10x10 as used by StageX and tuning software
        // Both axes have 10 values each
        let rows = 10;  // Y axis: 10 RPM rows (0, 400, 800, 1200, 1600, 2000, 2400, 2700, 3000, 5000)
        let cols = 10;  // X axis: 10 vehicle speed values (0-180 km/h)

        // Two signature variants exist:
        // Variant 1: [00 05][00 00][00 32][00 00][00 0A][00 0A] - g5runtrophy, lectureG5, etc.
        // Variant 2: [00 05][00 00][00 32][75 30][00 0A][00 0A] - VWPassat2l, VWJetta1.9l, etc.
        // Common prefix: [00 05][00 00][00 32] then 2 bytes variable then [00 0A][00 0A]
        let signature_prefix: [u8; 6] = [0x00, 0x05, 0x00, 0x00, 0x00, 0x32];
        let signature_suffix: [u8; 4] = [0x00, 0x0A, 0x00, 0x0A];

        let y_axis_size = rows * 2;  // RPM axis (10 × 2 = 20 bytes)
        let x_axis_size = cols * 2;  // Vehicle speed axis (10 × 2 = 20 bytes)
        let map_size = rows * cols * 2;  // Map data (10 × 10 × 2 = 200 bytes)
        // Structure: signature(12) + Y axis(20) + X axis(20) + data(200)
        let total_structure_size = 12 + y_axis_size + x_axis_size + map_size;

        let mut offset = start;
        while offset + total_structure_size <= end.min(data.len()) {
            // Look for signature pattern (prefix + 2 variable bytes + suffix)
            if offset + 12 <= data.len() &&
               data[offset..offset + 6] == signature_prefix &&
               data[offset + 8..offset + 12] == signature_suffix {

                // Structure: signature(12) -> Y axis -> X axis -> Map data
                let y_axis_start = offset + 12;  // RPM axis starts right after signature
                let x_axis_start = y_axis_start + y_axis_size;  // Vehicle speed axis
                let map_start = x_axis_start + x_axis_size;  // Map data

                // Skip if already detected
                if detected.contains(&(map_start as u32)) {
                    offset += 2;
                    continue;
                }

                // Validate Y axis (RPM: 0-5500, monotonic increasing) - 10 values
                let y_axis = self.read_values_be_unsigned(data, y_axis_start, rows);
                if !self.validate_launch_control_rpm_axis(&y_axis) {
                    offset += 2;
                    continue;
                }

                // Validate X axis (Vehicle speed: 0-20000 raw = 0-200 km/h)
                let x_axis = self.read_values_be_unsigned(data, x_axis_start, cols);
                if !self.validate_launch_control_speed_axis(&x_axis) {
                    offset += 2;
                    continue;
                }

                // Validate map data (Torque: 0-50000 raw = 0-5000 Nm) - 10x10 = 100 values
                let map_values = self.read_values_be_unsigned(data, map_start, rows * cols);
                if !self.validate_launch_control_data(&map_values) {
                    offset += 2;
                    continue;
                }

                // Check for enable switch in the zone before signature
                // The switch value 0x0064 (100) appears at variable offset before signature:
                // - damos: offset -12 (0x1D9D5C for signature at 0x1D9D68)
                // - g5runtrophy: offset -10 (0x1D4F26 for signature at 0x1D4F30)
                // Search in range -14 to -8 from signature for value 0x0064
                let mut switch_address = 0usize;
                let mut switch_value = 0u16;
                let mut is_enabled = false;

                for check_offset in [12, 10, 14, 8].iter() {
                    if offset >= *check_offset {
                        let addr = offset - check_offset;
                        if addr + 2 <= data.len() {
                            let val = ((data[addr] as u16) << 8) | (data[addr + 1] as u16);
                            if val == 0x0064 {
                                switch_address = addr;
                                switch_value = val;
                                is_enabled = true;
                                break;
                            }
                        }
                    }
                }

                // If not found, default to offset -12 and report the value there
                if switch_address == 0 && offset >= 12 {
                    switch_address = offset - 12;
                    if switch_address + 2 <= data.len() {
                        switch_value = ((data[switch_address] as u16) << 8) | (data[switch_address + 1] as u16);
                    }
                }

                log::debug!("✅ [EDC16] Found Launch Control / Torque Limiter by Vehicle Speed ({}x{}) at 0x{:X}", rows, cols, map_start);
                log::debug!("   Y axis (RPM) at 0x{:X}: {:?}", y_axis_start, y_axis);
                log::debug!("   X axis (km/h) at 0x{:X}: {:?}", x_axis_start, x_axis);
                log::debug!("   Switch at 0x{:X} = {} ({})",
                    switch_address, switch_value,
                    if is_enabled { "ENABLED" } else { "DISABLED" });

                let mut map = DetectedMap::new(
                    map_start as u32,
                    map_size,
                    MapDimensions::TwoDimensional { rows, cols },
                    DataType::Int16,
                );

                map.name = Some("Launch Control".to_string());
                map.category = Some(MapCategory::EngineTorqueLimiters.display_name().to_string());
                map.unit = Some("Nm".to_string());
                map.correction_factor = Some(0.1);  // Values are Nm * 10
                map.x_axis_address = Some(x_axis_start as u32);
                map.y_axis_address = Some(y_axis_start as u32);
                map.x_axis_correction = Some(0.01);  // Vehicle speed: value/100 = km/h
                map.y_axis_correction = Some(1.0);  // RPM direct
                map.x_label = Some("km/h".to_string());
                map.y_label = Some("rpm".to_string());
                map.confidence = 0.95;

                // Store switch info in description
                map.description = Some(format!(
                    "Torque Limiter by Vehicle Speed. Enable switch at 0x{:X} = {} ({})",
                    switch_address, switch_value,
                    if is_enabled { "ENABLED" } else { "DISABLED" }
                ));

                maps.push(map);

                // Skip past this map
                offset = map_start + map_size;
                continue;
            }
            offset += 2;
        }

        if !maps.is_empty() {
            log::debug!("🔧 [EDC16] Launch Control detection: found {} maps", maps.len());
        }

        maps
    }

    /// Validate Launch Control RPM axis (Y axis)
    /// Axis has 10 values: 0, 400, 800, 1200, 1600, 2000, 2400, 2700, 3000, 5000
    fn validate_launch_control_rpm_axis(&self, values: &[u16]) -> bool {
        // Must have exactly 10 values
        if values.len() != 10 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // RPM axis: starts at 0 (exactly), ends at 4000-6000 (typically 5000)
        if first != 0 {
            return false;
        }
        if last < 3500 || last > 7000 {
            return false;
        }

        // Check monotonically increasing (after first value which is 0)
        for i in 1..values.len() {
            if values[i] <= values[i - 1] {
                return false;
            }
        }

        true
    }

    /// Validate Launch Control vehicle speed axis (X axis)
    fn validate_launch_control_speed_axis(&self, values: &[u16]) -> bool {
        if values.len() != 10 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // Speed axis: 0-20000 raw = 0-200 km/h (factor 0.01)
        // First value should be 0 or very low
        if first > 500 {
            return false;
        }
        // Last value should be 15000-20000 (150-200 km/h)
        if last < 10000 || last > 25000 {
            return false;
        }

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i - 1] {
                return false;
            }
        }

        true
    }

    /// Validate Launch Control torque data
    fn validate_launch_control_data(&self, values: &[u16]) -> bool {
        if values.is_empty() {
            return false;
        }

        let max_val = values.iter().cloned().max().unwrap_or(0);

        // Torque values: 0-50000 raw = 0-5000 Nm (factor 0.1)
        // Typically set to 30000 (3000 Nm) when disabled/maxed out
        // Valid range: 0-50000
        if max_val > 50000 {
            return false;
        }

        // Should have at least some non-zero values
        let non_zero = values.iter().filter(|&&v| v > 0).count();
        if non_zero < values.len() / 2 {
            return false;
        }

        true
    }

    /// Detect Torque Limiter maps of a specific dimension variant
    ///
    /// Signature: three identical big-endian 16-bit placeholder values
    /// followed by `00 rows 00 cols`. Stock files use `7F FF`, but tuned
    /// files often rewrite the placeholders with a soft cap value (e.g.
    /// 3000 Nm). We accept any three matching non-zero values and rely on
    /// the axis/data validators to reject false positives.
    fn detect_torque_limiter_variant(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        rows: usize,
        cols: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        let rows_byte = rows as u8;
        let cols_byte = cols as u8;

        let y_axis_size = rows * 2;  // Atmospheric pressure axis
        let x_axis_size = cols * 2;  // RPM axis
        let map_size = rows * cols * 2;  // Map data
        let total_structure_size = 10 + y_axis_size + x_axis_size + map_size;  // signature + axes + data

        let mut offset = start;
        while offset + total_structure_size <= end.min(data.len()) {
            let placeholder_match = offset + 10 <= data.len()
                && data[offset]     == data[offset + 2]
                && data[offset]     == data[offset + 4]
                && data[offset + 1] == data[offset + 3]
                && data[offset + 1] == data[offset + 5]
                && !(data[offset] == 0 && data[offset + 1] == 0)
                && data[offset + 6] == 0x00 && data[offset + 7] == rows_byte
                && data[offset + 8] == 0x00 && data[offset + 9] == cols_byte;

            if placeholder_match {

                // Found potential signature!
                let y_axis_start = offset + 10;  // Atmospheric pressure axis
                let x_axis_start = y_axis_start + y_axis_size;  // RPM axis
                let map_start = x_axis_start + x_axis_size;  // Map data

                // Skip if already detected
                if detected.contains(&(map_start as u32)) {
                    offset += 2;
                    continue;
                }

                // Validate Y axis (Atmospheric pressure: 600-1100 mbar, monotonic)
                let y_axis = self.read_values_be_unsigned(data, y_axis_start, rows);
                if !self.validate_torque_limiter_pressure_axis(&y_axis) {
                    offset += 2;
                    continue;
                }

                // Validate X axis (RPM: 0-5500, monotonic increasing)
                let x_axis = self.read_values_be_unsigned(data, x_axis_start, cols);
                if !self.validate_torque_limiter_rpm_axis(&x_axis) {
                    offset += 2;
                    continue;
                }

                // Validate map data (Torque: 0-6000 raw = 0-600 Nm)
                let map_values = self.read_values_be_signed(data, map_start, rows * cols);
                if !self.validate_torque_limiter_data(&map_values) {
                    offset += 2;
                    continue;
                }

                log::debug!("✅ [EDC16] Found Torque Limiter ({}x{}) at 0x{:X} (Pressure: {}-{} mbar, RPM: {}-{})",
                    cols, rows, map_start,
                    y_axis[0], y_axis[rows - 1],
                    x_axis[0], x_axis[cols - 1]);

                let mut map = DetectedMap::new(
                    map_start as u32,
                    map_size,
                    MapDimensions::TwoDimensional { rows, cols },
                    DataType::Int16,
                );

                map.name = Some("Torque Limiter".to_string());
                map.category = Some(MapCategory::EngineTorqueLimiters.display_name().to_string());
                map.unit = Some("Nm".to_string());
                map.correction_factor = Some(0.1);  // Values are Nm * 10
                map.x_axis_address = Some(x_axis_start as u32);
                map.y_axis_address = Some(y_axis_start as u32);
                map.x_axis_correction = Some(1.0);  // RPM direct
                map.y_axis_correction = Some(1.0);  // Pressure in mbar direct
                map.x_label = Some("rpm".to_string());
                map.y_label = Some("mbar".to_string());
                map.confidence = 0.95;

                maps.push(map);

                // Skip past this map
                offset = map_start + map_size;
                continue;
            }
            offset += 2;
        }

        maps
    }

    /// Validate Torque Limiter pressure axis (Atmospheric pressure in mbar)
    fn validate_torque_limiter_pressure_axis(&self, values: &[u16]) -> bool {
        if values.is_empty() || values.len() > 5 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // Atmospheric pressure: typically 600-1100 mbar range
        if first < 500 || first > 800 {
            return false;  // First value should be ~600-700 mbar
        }
        if last < 800 || last > 1200 {
            return false;  // Last value should be ~900-1100 mbar
        }

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i - 1] {
                return false;
            }
        }

        true
    }

    /// Validate Torque Limiter RPM axis
    fn validate_torque_limiter_rpm_axis(&self, values: &[u16]) -> bool {
        if values.len() < 15 || values.len() > 25 {
            return false;  // Should be 20-22 values
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // RPM axis: starts at 0 or low (Caddy 016GP : 549, Touareg V10 :
        // 250 — pas de point 0), ends at 4500-5500
        if first > 600 {
            return false;  // First value should be low
        }
        if last < 4500 || last > 6000 {
            return false;  // Last value should be ~5000-5500 RPM
        }

        // Check monotonically increasing — tolère jusqu'à 2 inversions
        // (fichiers tunés : 016FF a un point d'axe édité 1000→994)
        let mut dips = 0;
        for i in 1..values.len() {
            if values[i] < values[i - 1] {
                dips += 1;
                if dips > 2 {
                    return false;
                }
            }
        }

        true
    }

    /// Validate Torque Limiter map data (Torque values in Nm * 10)
    fn validate_torque_limiter_data(&self, values: &[i16]) -> bool {
        if values.is_empty() {
            return false;
        }

        let min_val = values.iter().cloned().min().unwrap_or(0);
        let max_val = values.iter().cloned().max().unwrap_or(0);

        // Torque values: 0-6000 raw = 0-600 Nm (factor 0.1)
        // Stock values typically 0-4000, tuned can go higher
        if min_val < -500 || max_val > 10000 {
            return false;  // Out of reasonable range
        }

        // Should have some variation (not all zeros or constant)
        if max_val - min_val < 500 {
            return false;  // Not enough variation
        }

        // Most values should be positive (torque limits are positive)
        let positive_count = values.iter().filter(|&&v| v > 0).count();
        if positive_count < values.len() / 2 {
            return false;
        }

        true
    }

    /// Score a potential map based on multiple factors (EDCMasterHP style)
    fn score_map(&self, values: &[f64], rows: usize, cols: usize, template: &MapTemplate) -> Option<f64> {
        let count = values.len();
        if count != rows * cols {
            return None;
        }

        // Convert values using z_factor
        let converted: Vec<f64> = values.iter().map(|&v| v * template.z_factor).collect();

        // Get appropriate range based on mode
        let (z_min, z_max) = if self.tuned_mode {
            template.z_range_tuned
        } else {
            template.z_range_stock
        };

        let mut score = 0.0;

        // Factor 1: Grid size match (+0.4 for most frequent size)
        let is_frequent_grid = template.grids.first() == Some(&(rows, cols));
        score += if is_frequent_grid { 0.40 } else { 0.30 };

        // Factor 2: Z values within physical range (+0.30)
        let in_range_count = converted.iter()
            .filter(|&&v| v >= z_min && v <= z_max)
            .count();
        let range_ratio = in_range_count as f64 / count as f64;

        if range_ratio < 0.85 {
            return None;  // Too many out-of-range values
        }
        score += range_ratio * 0.30;

        // Factor 3: Monotonicity check based on map type (+0.20)
        let monotonicity_score = self.check_monotonicity(&converted, rows, cols, template);
        score += monotonicity_score * 0.20;

        // Factor 4: Value distribution (+0.10)
        let distribution_score = self.check_distribution(&converted, z_min, z_max);
        score += distribution_score * 0.10;

        // Apply map-type specific validation
        if !self.validate_map_specific(values, &converted, rows, cols, template) {
            return None;
        }

        Some(score.min(0.98))
    }

    /// Check monotonicity patterns based on map type
    fn check_monotonicity(&self, values: &[f64], rows: usize, cols: usize, template: &MapTemplate) -> f64 {
        match template.name {
            "Driver Wish" => {
                // Torque should increase with pedal position (columns)
                self.check_column_increasing(values, rows, cols)
            },
            "N75 Duty Cycle" => {
                // N75 maps can be flat (all same value) or have slight variations
                // "No gear" maps are typically all 75% - this is valid
                0.8  // Neutral/good score for N75
            },
            "Boost Target" | "Boost Limiter" => {
                // Boost should generally increase with load
                self.check_column_increasing(values, rows, cols)
            },
            "Torque to IQ Conversion" => {
                // IQ should increase with torque (columns)
                self.check_row_increasing(values, rows, cols)
            },
            "EGR" => {
                // EGR typically decreases at high load (zeros at high RPM/load)
                self.check_egr_pattern(values, rows, cols)
            },
            "Start of Injection" => {
                // SOI maps should be smooth (low roughness)
                self.check_smoothness(values, rows, cols)
            },
            _ => 0.5 // Neutral score for unknown patterns
        }
    }

    /// Check if columns show increasing trend (for pedal-based maps)
    fn check_column_increasing(&self, values: &[f64], rows: usize, cols: usize) -> f64 {
        let mut col_avgs: Vec<f64> = Vec::with_capacity(cols);

        for col in 0..cols {
            let sum: f64 = (0..rows).map(|row| values[row * cols + col]).sum();
            col_avgs.push(sum / rows as f64);
        }

        let mut increasing = 0;
        for i in 1..cols {
            if col_avgs[i] > col_avgs[i - 1] {
                increasing += 1;
            }
        }

        increasing as f64 / (cols - 1).max(1) as f64
    }

    /// Check if rows show increasing trend
    fn check_row_increasing(&self, values: &[f64], rows: usize, cols: usize) -> f64 {
        let mut rows_increasing = 0;

        for row in 0..rows {
            let mut is_increasing = true;
            for col in 1..cols {
                if values[row * cols + col] < values[row * cols + col - 1] {
                    is_increasing = false;
                    break;
                }
            }
            if is_increasing {
                rows_increasing += 1;
            }
        }

        rows_increasing as f64 / rows as f64
    }

    /// Check EGR-specific pattern (zeros at high load)
    fn check_egr_pattern(&self, values: &[f64], rows: usize, cols: usize) -> f64 {
        // Count zeros in high-load corner (last quarter)
        let mut high_load_zeros = 0;
        let start_row = rows * 3 / 4;
        let start_col = cols * 3 / 4;

        for row in start_row..rows {
            for col in start_col..cols {
                if values[row * cols + col].abs() < 1.0 {
                    high_load_zeros += 1;
                }
            }
        }

        let corner_size = ((rows - start_row) * (cols - start_col)) as f64;
        let zero_ratio = high_load_zeros as f64 / corner_size;

        // EGR maps should have some zeros at high load (but not all)
        if zero_ratio > 0.1 && zero_ratio < 0.8 {
            0.8
        } else if zero_ratio >= 0.05 {
            0.5
        } else {
            0.3
        }
    }

    /// Check smoothness (for SOI maps)
    fn check_smoothness(&self, values: &[f64], rows: usize, cols: usize) -> f64 {
        let mut rough_transitions = 0;
        let threshold = 150.0 * 0.023437; // ~3.5 degrees

        for row in 0..rows {
            for col in 1..cols {
                let diff = (values[row * cols + col] - values[row * cols + col - 1]).abs();
                if diff > threshold {
                    rough_transitions += 1;
                }
            }
        }

        let total_transitions = rows * (cols - 1);
        let roughness_ratio = rough_transitions as f64 / total_transitions as f64;

        (1.0 - roughness_ratio).max(0.0)
    }

    /// Check value distribution quality
    fn check_distribution(&self, values: &[f64], z_min: f64, z_max: f64) -> f64 {
        let min_val = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_val = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let range = max_val - min_val;
        let expected_range = z_max - z_min;

        // Good maps have reasonable spread (not flat, not too extreme)
        let range_ratio = range / expected_range;

        if range_ratio < 0.1 {
            0.2  // Too flat
        } else if range_ratio > 1.5 {
            0.5  // Too extreme
        } else {
            0.8  // Good distribution
        }
    }

    /// Map-type specific validation rules
    fn validate_map_specific(&self, raw_values: &[f64], converted: &[f64], rows: usize, cols: usize, template: &MapTemplate) -> bool {
        match template.name {
            "Driver Wish" => {
                // Full throttle average should be substantial
                let last_col_avg: f64 = (0..rows)
                    .map(|row| converted[row * cols + cols - 1])
                    .sum::<f64>() / rows as f64;

                let first_col_avg: f64 = (0..rows)
                    .map(|row| converted[row * cols])
                    .sum::<f64>() / rows as f64;

                // Full throttle should be > idle + 80 Nm
                last_col_avg > first_col_avg + 80.0 && last_col_avg > 150.0
            },

            "Torque to IQ Conversion" => {
                // Must have good value spread
                let max = converted.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let min = converted.iter().cloned().fold(f64::INFINITY, f64::min);
                max > 20.0 && max - min > 15.0
            },

            "Start of Injection" => {
                // SOI should have diverse values
                let unique: HashSet<i64> = raw_values.iter().map(|&v| v as i64).collect();
                unique.len() >= 40
            },

            "EGR" => {
                // Should have mix of zeros and non-zeros
                let zeros = converted.iter().filter(|&&v| v.abs() < 1.0).count();
                zeros > 20 && zeros < converted.len() - 20
            },

            "Torque Limiter" => {
                // Values should be relatively uniform (it's a limit)
                let max = raw_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let min = raw_values.iter().cloned().fold(f64::INFINITY, f64::min);
                (max - min) < 2000.0 * 10.0  // Max 200 Nm variation
            },

            "N75 Duty Cycle" => {
                // N75 validation based on actual map analysis:
                // - Stock: values typically 20-96%
                // - Tuned: values can go up to 110%+ (aggressive boost control)
                // - EXCEPTION: First column can be 0% (low IQ = no boost needed)
                // - Average varies: stock 65-80%, tuned can be 90-100%

                let min_val = converted.iter().cloned().fold(f64::INFINITY, f64::min);
                let max_val = converted.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let avg: f64 = converted.iter().sum::<f64>() / converted.len() as f64;

                // N75 maps often have:
                // - First column (low IQ) at 0%
                // - Rest of values in working range
                // - High duty at low RPM, decreasing with RPM

                // Count values outside valid range
                // Allow up to 115% for tuned files (raw ~9400 with factor 0.012207 = ~115%)
                let out_of_range_high = converted.iter().filter(|&&v| v > 115.0).count();
                let out_of_range_low = converted.iter().filter(|&&v| v < -1.0).count();

                // Count how many values are in a reasonable N75 working range (20-115%)
                let in_working_range = converted.iter().filter(|&&v| v >= 20.0 && v <= 115.0).count();
                let working_ratio = in_working_range as f64 / converted.len() as f64;

                // Validation criteria:
                // 1. All values must be >= 0% (or very close to it)
                // 2. Maximum <= 115% (allows tuned files)
                // 3. Average between 50-110% (tuned files can have very high averages)
                // 4. At least 70% of values in working range (20-115%)
                //    This allows some values to be 0% (e.g., first column of 16 rows)
                min_val >= -1.0 &&           // Allow -0.something for rounding
                max_val <= 115.0 &&
                avg >= 50.0 && avg <= 110.0 &&
                working_ratio >= 0.70 &&
                out_of_range_high == 0 &&
                out_of_range_low == 0
            },

            _ => true  // No specific validation
        }
    }

    // ============== HELPER FUNCTIONS ==============

    /// Read Big-Endian signed values
    fn read_values_be_signed(&self, data: &[u8], offset: usize, count: usize) -> Vec<i16> {
        let mut values = Vec::with_capacity(count);
        for i in 0..count {
            let pos = offset + i * 2;
            if pos + 1 < data.len() {
                let val = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
                values.push(val as i16);
            }
        }
        values
    }

    /// Read Big-Endian unsigned values
    fn read_values_be_unsigned(&self, data: &[u8], offset: usize, count: usize) -> Vec<u16> {
        let mut values = Vec::with_capacity(count);
        for i in 0..count {
            let pos = offset + i * 2;
            if pos + 1 < data.len() {
                let val = ((data[pos] as u16) << 8) | (data[pos + 1] as u16);
                values.push(val);
            }
        }
        values
    }

    /// Check for overlapping ranges
    fn overlaps(&self, existing: &HashSet<(u32, u32)>, new_range: (u32, u32)) -> bool {
        for &(start, end) in existing {
            if new_range.0 < end && new_range.1 > start {
                return true;
            }
        }
        false
    }

    /// Get data section range for scanning
    fn get_data_section_range(&self, file_size: usize) -> (usize, usize) {
        // For EDC16, data is typically in the upper half of the ROM
        // Skip first 0x30000 (code/vectors)
        if file_size >= 0x200000 {
            // 2MB file
            (0x180000, 0x200000)
        } else if file_size >= 0x100000 {
            // 1MB file
            (0x80000, 0x100000)
        } else if file_size >= 0x80000 {
            // 512KB file
            (0x40000, file_size)
        } else {
            // Smaller file - scan from 0x30000
            (MIN_DATA_SECTION.min(file_size / 2), file_size)
        }
    }

    /// Detect RPM axis values (helper for axis validation)
    #[allow(dead_code)]
    fn detect_rpm_axis(&self, values: &[u16]) -> bool {
        if values.len() < 8 || values.len() > 20 {
            return false;
        }

        // RPM values should be:
        // 1. In range 400-6000
        // 2. Monotonically increasing
        // 3. With typical delta ~250-500 between points

        let mut prev = 0u16;
        for (i, &val) in values.iter().enumerate() {
            if val < 400 || val > 6000 {
                return false;
            }
            if i > 0 {
                if val <= prev {
                    return false;  // Not increasing
                }
                let delta = val - prev;
                if delta < 100 || delta > 1000 {
                    return false;  // Unusual delta
                }
            }
            prev = val;
        }

        true
    }

    /// Detect pedal axis values
    #[allow(dead_code)]
    fn detect_pedal_axis(&self, values: &[u16]) -> bool {
        if values.len() < 6 || values.len() > 12 {
            return false;
        }

        // Pedal values should be in 0-10500 range (0-105% in 0.01% units)
        // Starting near 0, ending near 10000

        if values[0] > 500 {
            return false;  // Should start near 0
        }

        if values[values.len() - 1] < 8000 {
            return false;  // Should end near 100%
        }

        // Check monotonic increase
        for i in 1..values.len() {
            if values[i] <= values[i - 1] {
                return false;
            }
        }

        true
    }

    // ========== PHASE 11: Duration Block Detection (SIGNATURE-BASED) ==========
    // Duration block structure (similar to SOI):
    // - Duration Selector (1x6): values 5,4,3,2,1,0 (decreasing)
    // - Duration 00: 10x10 (signature 00 0A 00 0A)
    // - Duration 01-04: 19x15 (signature 00 13 00 0F)
    // - Duration 05: 9x9 or 10x10 (signature 00 09 00 09 or 00 0A 00 0A)

    /// Detect Duration Selector by its characteristic pattern
    /// Pattern: 6 decreasing values (5, 4, 3, 2, 1, 0) in Little-Endian format
    /// Hex: 05 00, 04 00, 03 00, 02 00, 01 00, 00 00
    fn detect_duration_selector(&self, data: &[u8]) -> Vec<(u32, DetectedMap)> {
        let mut results = Vec::new();

        // Duration Selector pattern in Little-Endian:
        // 05 00 04 00 03 00 02 00 01 00 00 00 = values 5,4,3,2,1,0
        let selector_pattern: [u8; 12] = [0x05, 0x00, 0x04, 0x00, 0x03, 0x00, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00];

        let (start, end) = self.get_data_section_range(data.len());
        let mut offset = start;

        while offset + 20 <= end.min(data.len()) {
            // Sélecteur = 6 mots LITTLE-ENDIAN ≤7, non croissants, premier
            // ≥2, dernier 0. Le motif exact 5,4,3,2,1,0 était mis en échec
            // par les fichiers TUNÉS (016FF 371366 : sélecteur édité en
            // 5,3,3,2,1,0) — l'ancrage sur la signature Duration 00
            // [000A][000A] juste après reste le garde-fou anti-FP.
            let sel_ok = data[offset..offset + 12] == selector_pattern || {
                let vals: Vec<u16> = (0..6)
                    .map(|k| {
                        u16::from_le_bytes([data[offset + k * 2], data[offset + k * 2 + 1]])
                    })
                    .collect();
                vals.iter().all(|&v| v <= 7)
                    && vals[0] >= 2
                    && vals[5] == 0
                    && vals.windows(2).all(|w| w[0] >= w[1])
            };
            if sel_ok {
                // Found Duration Selector!
                // Check for Duration 00 signature at offset+12 (variant 1) or offset+16 (variant 2)
                // Variant 1: Selector immediately followed by Duration 00 signature
                // Variant 2: 4 extra bytes between selector and Duration 00 signature (e.g., 01 00 00 00)

                let sig_offset_v1 = offset + 12;
                let sig_offset_v2 = offset + 16;

                let gap_bytes = if sig_offset_v2 + 4 <= data.len() {
                    (sig_offset_v2 - sig_offset_v1) as i32
                } else {
                    0
                };

                let (confirmed, gap) = if sig_offset_v1 + 4 <= data.len() &&
                    data[sig_offset_v1..sig_offset_v1 + 4] == [0x00, 0x0A, 0x00, 0x0A] {
                    (true, 0)  // Variant 1: no gap
                } else if sig_offset_v2 + 4 <= data.len() &&
                    data[sig_offset_v2..sig_offset_v2 + 4] == [0x00, 0x0A, 0x00, 0x0A] {
                    (true, gap_bytes)  // Variant 2: 4-byte gap
                } else {
                    (false, 0)
                };

                if confirmed {
                    log::debug!("✅ [EDC16] Found Duration Selector at 0x{:X} (gap: {} bytes)", offset, gap);

                    let mut map = DetectedMap::new(
                        offset as u32,
                        12,  // 6 values × 2 bytes
                        MapDimensions::OneDimensional { length: 6 },
                        DataType::UInt16,
                    );
                    map.name = Some("Duration Selector".to_string());
                    map.category = Some(MapCategory::InjectionSystem.display_name().to_string());
                    map.confidence = 0.95;
                    map.unit = Some("[-]".to_string());
                    map.correction_factor = Some(1.0);  // Selector values are 5,4,3,2,1,0 directly
                    map.is_little_endian = Some(true);  // Duration Selector is Little-Endian

                    results.push((offset as u32, map));

                    offset += 16 + gap as usize;  // Skip selector + gap + signature
                    continue;
                }
            }

            offset += 2;
        }

        log::debug!("🔧 [EDC16] Duration Selector detection: found {} selectors", results.len());
        results
    }

    /// Detect Duration block maps starting from a Duration Selector address
    /// Structure: Selector + [optional gap] + Duration 00 (10x10) + Duration 01-04 (19x15 each) + Duration 05 (9x9 or 10x10)
    /// Some ECU variants have a 4-byte gap between selector and Duration 00
    fn detect_duration_block_maps(
        &self,
        data: &[u8],
        selector_addr: u32,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        // After selector (12 bytes), find Duration 00 signature
        // It can be at offset+12 (no gap) or offset+16 (4-byte gap)
        let base_offset = selector_addr as usize + 12;
        let mut current_offset = base_offset;

        // Check if Duration 00 signature is at base_offset or base_offset+4
        if base_offset + 8 <= data.len() {
            let sig_at_base = &data[base_offset..base_offset + 4];
            let sig_at_gap = &data[base_offset + 4..base_offset + 8];

            if sig_at_base == [0x00, 0x0A, 0x00, 0x0A] {
                current_offset = base_offset;  // No gap
            } else if sig_at_gap == [0x00, 0x0A, 0x00, 0x0A] {
                current_offset = base_offset + 4;  // 4-byte gap
                log::debug!("[EDC16] Duration block has 4-byte gap after selector");
            } else {
                log::warn!("⚠️ [EDC16] Duration 00 signature not found after selector at 0x{:X}", selector_addr);
                return maps;
            }
        } else {
            return maps;
        }

        // Flexible Duration block detection:
        // Duration maps follow a pattern of decreasing complexity:
        // - Duration 00: typically 10x10
        // - Duration 01-04: typically 17-19 rows × 15 cols (varies by ECU variant)
        // - Duration 05+: 9-10 rows × 9-10 cols (final small maps)
        // We detect up to 10 Duration maps dynamically based on valid signatures

        let max_duration_maps = 10;
        let mut duration_idx = 0;

        while duration_idx < max_duration_maps {
            if current_offset + 4 > data.len() {
                break;
            }

            // Read signature
            let sig_rows = ((data[current_offset] as u16) << 8 | data[current_offset + 1] as u16) as usize;
            let sig_cols = ((data[current_offset + 2] as u16) << 8 | data[current_offset + 3] as u16) as usize;

            // Validate signature: Duration maps have reasonable dimensions
            // - Rows: 9-20 (typically 9-10 for small maps, 16-19 for large maps)
            // - Cols: 9-15 (typically 9-10 for small maps, 15 for large maps)
            let valid_signature = sig_rows >= 9 && sig_rows <= 20 &&
                                  sig_cols >= 9 && sig_cols <= 15;

            if !valid_signature {
                log::debug!("[EDC16] Duration block ended at idx {} (invalid signature {}x{} at 0x{:X})",
                         duration_idx, sig_rows, sig_cols, current_offset);
                break;
            }

            let (rows, cols) = (sig_rows, sig_cols);
            let name = format!("Duration {:02}", duration_idx);

            // Calculate addresses
            let sig_offset = current_offset;
            let y_axis_start = sig_offset + 4;  // After signature
            let x_axis_start = y_axis_start + rows * 2;
            let map_start = x_axis_start + cols * 2;
            let map_size = rows * cols * 2;

            if map_start + map_size > data.len() {
                break;
            }

            // Skip if already detected
            if detected.contains(&(map_start as u32)) {
                current_offset = map_start + map_size;
                duration_idx += 1;
                continue;
            }

            // Validate axes
            let rpm_axis = self.read_values_be_unsigned(data, y_axis_start, rows);
            let iq_axis = self.read_values_be_unsigned(data, x_axis_start, cols);

            if !self.is_valid_duration_rpm_axis_v2(&rpm_axis) {
                log::warn!("⚠️ [EDC16] {} invalid RPM axis at 0x{:X}", name, y_axis_start);
                break;
            }

            if !self.is_valid_duration_iq_axis_v2(&iq_axis) {
                log::warn!("⚠️ [EDC16] {} invalid IQ axis at 0x{:X}", name, x_axis_start);
                break;
            }

            // Create detected map
            let mut map = DetectedMap::new(
                map_start as u32,
                map_size,
                MapDimensions::TwoDimensional { rows, cols },
                DataType::Int16,
            );

            map.name = Some(name.clone());
            map.category = Some(MapCategory::InjectionSystem.display_name().to_string());
            map.unit = Some("deg CrS".to_string());
            map.correction_factor = Some(0.023427);
            map.y_axis_address = Some(y_axis_start as u32);
            map.x_axis_address = Some(x_axis_start as u32);
            map.y_axis_correction = Some(1.0);
            map.x_axis_correction = Some(0.01);
            map.y_label = Some("rpm".to_string());
            map.x_label = Some("mg/stroke".to_string());
            map.confidence = 0.92;

            log::debug!("✅ [EDC16] Found {} ({}x{}) at 0x{:X}", name, rows, cols, map_start);
            maps.push(map);

            // Move to next map - scan for next valid signature
            // Duration maps may have variable padding between them
            current_offset = map_start + map_size;
            duration_idx += 1;

            // Scan for next valid signature within reasonable distance (up to 512 bytes)
            // Some ECU variants have large gaps (400+ bytes) between Duration maps
            let scan_limit = (current_offset + 512).min(data.len() - 4);
            let mut found_next = false;
            while current_offset < scan_limit {
                let sig_r = ((data[current_offset] as u16) << 8 | data[current_offset + 1] as u16) as usize;
                let sig_c = ((data[current_offset + 2] as u16) << 8 | data[current_offset + 3] as u16) as usize;

                // Valid Duration signature: 9-20 rows, 9-15 cols
                if sig_r >= 9 && sig_r <= 20 && sig_c >= 9 && sig_c <= 15 {
                    found_next = true;
                    break;
                }
                current_offset += 2;
            }

            if !found_next {
                log::debug!("[EDC16] Duration block ended after {} maps (no valid signature found within scan range)", duration_idx);
                break;
            }
        }

        maps
    }

    /// Validate Duration RPM axis (improved version)
    /// Duration RPM axes vary widely:
    /// - Some maps have RPM from 100-6500
    /// - Some maps have RPM from 600-2500 (very low max!)
    /// - Most maps have RPM from 800-5200
    fn is_valid_duration_rpm_axis_v2(&self, values: &[u16]) -> bool {
        if values.is_empty() || values.len() > 25 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // Duration RPM: starts from 0-2000 (l'axe peut démarrer à 0 —
        // Touran 016A Duration 05 : [0, 150, 550…] ; même fix que sur U31)
        if first > 2000 {
            return false;
        }

        // End should be at least 2000 and at most 8000
        // Some variants have low RPM range (2500), others go up to 6500
        if last < 2000 || last > 8000 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    /// Validate Duration IQ axis (improved version)
    /// IQ axis for Duration maps: typically 15-60 mg/stroke (raw values 1500-6000)
    /// or 0-55 mg/stroke (raw values 0-5500) depending on the variant
    fn is_valid_duration_iq_axis_v2(&self, values: &[u16]) -> bool {
        if values.is_empty() || values.len() > 20 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // IQ axis can start anywhere from 0 to 2000 (0-20 mg/stroke)
        // Must end in range 3000-10000 (30-100 mg/stroke)
        if first > 2000 {
            return false;
        }
        if last < 3000 || last > 10000 {
            return false;
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        Self::axis_ascending_tolerant(values, 2)
    }

    // ============== SIGNATURE-BASED DETECTION ==============
    // These methods detect maps using byte sequences that are IDENTICAL across all files
    // This approach is much more reliable than heuristic detection for specific map types

    /// Detect SVBL (Boost Limiter Scalar) using combined signature
    /// SVBL is a single 16-bit Big-Endian value representing max boost pressure in mbar
    /// Typical values: 2350 mbar (1.9L) or 2620 mbar (2.0L), up to 3200+ mbar for tuned files
    ///
    /// Combined Signature:
    /// - 32 bytes BEFORE SVBL: 07 3A 08 CA 08 CA 08 CA 08 CA 08 CA 06 06 06 40 06 61 06 B8 07 3A 08 CA 08 CA 08 CA 08 CA 08 CA
    /// - 3 bytes AFTER SVBL: 00 00 00
    ///
    /// There is always exactly ONE SVBL per file.
    pub fn detect_svbl_by_signature(&self, data: &[u8]) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        // The signature pattern that precedes SVBL (32 bytes before)
        let signature_before: [u8; 32] = [
            0x07, 0x3A, 0x08, 0xCA, 0x08, 0xCA, 0x08, 0xCA,
            0x08, 0xCA, 0x08, 0xCA, 0x06, 0x06, 0x06, 0x40,
            0x06, 0x61, 0x06, 0xB8, 0x07, 0x3A, 0x08, 0xCA,
            0x08, 0xCA, 0x08, 0xCA, 0x08, 0xCA, 0x08, 0xCA
        ];

        // The signature pattern after SVBL (3 bytes: 00 00 00)
        let signature_after: [u8; 3] = [0x00, 0x00, 0x00];

        let (scan_start, scan_end) = self.get_data_section_range(data.len());

        // Search for the combined signature
        for offset in scan_start..scan_end.saturating_sub(signature_before.len() + 5) {
            // Check signature BEFORE
            if data[offset..offset + signature_before.len()] == signature_before {
                // SVBL value is immediately after the signature
                let svbl_offset = offset + signature_before.len();

                if svbl_offset + 5 <= data.len() {
                    // Check signature AFTER (at svbl_offset + 2)
                    let after_offset = svbl_offset + 2;
                    if data[after_offset..after_offset + 3] == signature_after {
                        // Read Big-Endian 16-bit value
                        let value = ((data[svbl_offset] as u16) << 8) | (data[svbl_offset + 1] as u16);

                        // Validate: SVBL should be between 0-6200 mbar
                        if value <= 6200 {
                            log::debug!("🎯 [EDC16] Found SVBL by signature at 0x{:X}, value: {} mbar", svbl_offset, value);

                            let mut map = DetectedMap::new(
                                svbl_offset as u32,
                                2, // 2 bytes
                                MapDimensions::OneDimensional { length: 1 },
                                DataType::Int16,
                            );
                            map.name = Some("SVBL - Max Boost Pressure".to_string());
                            map.category = Some(MapCategory::TurboBoostPressure.display_name().to_string());
                            map.unit = Some("mbar".to_string());
                            map.correction_factor = Some(1.0);
                            map.offset = Some(0.0);
                            map.confidence = 1.0; // Very high confidence with combined signature match

                            maps.push(map);
                            // Continue searching for more SVBL maps in multi-codeblock files
                        }
                    }
                }
            }
        }

        // Variante structurelle (Touareg V10 016D, 0xF8E10) : le noyau
        // historique 32 octets est absent — le SVBL est suivi d'un stub 2x2
        // désactivé : v [0000][0002][0002][0000][y 2000-3200][0000][x 4500-5800]
        if maps.is_empty() {
            let rd = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
            for offset in (scan_start..scan_end.saturating_sub(20)).step_by(2) {
                let v = rd(offset);
                if !(1500..=6200).contains(&v) {
                    continue;
                }
                if rd(offset + 2) == 0
                    && rd(offset + 4) == 2
                    && rd(offset + 6) == 2
                    && rd(offset + 8) == 0
                    && (2000..=3200).contains(&rd(offset + 10))
                    && rd(offset + 12) == 0
                    && (4500..=5800).contains(&rd(offset + 14))
                {
                    log::debug!("🎯 [EDC16U1] Found SVBL (variante stub 2x2) at 0x{:X}, value: {} mbar", offset, v);
                    let mut map = DetectedMap::new(
                        offset as u32,
                        2,
                        MapDimensions::OneDimensional { length: 1 },
                        DataType::Int16,
                    );
                    map.name = Some("SVBL - Max Boost Pressure".to_string());
                    map.category = Some(MapCategory::TurboBoostPressure.display_name().to_string());
                    map.unit = Some("mbar".to_string());
                    map.correction_factor = Some(1.0);
                    map.offset = Some(0.0);
                    map.confidence = 0.9;
                    maps.push(map);
                    break;
                }
            }
        }

        log::debug!("🔧 [EDC16] SVBL detection: found {} maps", maps.len());
        maps
    }

    /// Detect SVRL (RPM Limiter Scalar) using signature
    /// SVRL is a single 16-bit Big-Endian value representing max engine RPM
    /// Typical values: 4500-5500 RPM (validation range: 0-6500 RPM)
    ///
    /// Signature: Pattern "05 00 04 00 03 00 02 00 01 00 00 00" appears at SVRL + 16 bytes
    /// This pattern is a descending sequence (5,4,3,2,1,0) that appears in ALL EDC16U1 files
    pub fn detect_svrl_by_signature(&self, data: &[u8]) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        // The signature pattern that appears AFTER SVRL (+16 bytes)
        // This is a descending value sequence: 05 00 04 00 03 00 02 00 01 00 00 00
        let signature: [u8; 12] = [
            0x05, 0x00, 0x04, 0x00, 0x03, 0x00,
            0x02, 0x00, 0x01, 0x00, 0x00, 0x00
        ];

        let (scan_start, scan_end) = self.get_data_section_range(data.len());

        // Search for the signature. Le motif exact 5,4,3,2,1,0 est mis en
        // échec par les fichiers TUNÉS (016FF : sélecteur édité 5,3,3,2,1,0)
        // — la forme générale (6 mots LE ≤7 non croissants finissant à 0)
        // n'est acceptée QUE si la signature Duration 00 [000A][000A] suit
        // à +12 ou +16 (même garde-fou que detect_duration_selector).
        for offset in scan_start..scan_end.saturating_sub(20) {
            let exact = data[offset..offset + signature.len()] == signature;
            let general = !exact && {
                let vals: Vec<u16> = (0..6)
                    .map(|k| {
                        u16::from_le_bytes([data[offset + k * 2], data[offset + k * 2 + 1]])
                    })
                    .collect();
                vals.iter().all(|&v| v <= 7)
                    && vals[0] >= 2
                    && vals[5] == 0
                    && vals.windows(2).all(|w| w[0] >= w[1])
                    && (data[offset + 12..offset + 16] == [0x00, 0x0A, 0x00, 0x0A]
                        || (offset + 20 <= data.len()
                            && data[offset + 16..offset + 20] == [0x00, 0x0A, 0x00, 0x0A]))
            };
            if exact || general {
                // SVRL is 16 bytes BEFORE this signature
                let svrl_offset = offset.saturating_sub(16);

                if svrl_offset >= scan_start && svrl_offset + 2 <= data.len() {
                    // Read Big-Endian 16-bit value
                    let value = ((data[svrl_offset] as u16) << 8) | (data[svrl_offset + 1] as u16);

                    // Validate RPM range: 0-6500 RPM
                    if value <= 6500 {
                        log::debug!("🎯 [EDC16] Found SVRL by signature at 0x{:X}, value: {} RPM", svrl_offset, value);

                        let mut map = DetectedMap::new(
                            svrl_offset as u32,
                            2, // 2 bytes
                            MapDimensions::OneDimensional { length: 1 },
                            DataType::Int16,
                        );
                        map.name = Some("SVRL - RPM Limiter".to_string());
                        map.category = Some(MapCategory::MaximumRpmLimiter.display_name().to_string());
                        map.unit = Some("RPM".to_string());
                        map.correction_factor = Some(1.0);
                        map.offset = Some(0.0);
                        map.confidence = 1.0; // Very high confidence with signature match

                        maps.push(map);
                        // Only one SVRL per file
                        break;
                    }
                }
            }
        }

        log::debug!("🔧 [EDC16] SVRL detection: found {} maps", maps.len());
        maps
    }


    /// Detect Maximum Vehicle Speed maps (Single Value 1x1 maps)
    /// EDC16U1: Based on StageX patterns analysis with CORRECT Big-Endian interpretation
    /// Addresses vary but follow patterns:
    /// - Zone 1: D8D9C-D8E10 range (1 map)
    /// - Zone 2: EEB2A-EECC0 range (2 maps, 10 bytes apart)
    /// - Zone 3: F05F6-F0600 range (2 maps for default files only)
    /// - Zone 4: F126A-F2D36 range (1 map)
    pub fn detect_maximum_vehicle_speed(&self, data: &[u8], detected_addresses: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut found: HashSet<u32> = HashSet::new();
        let (_scan_start, scan_end) = self.get_data_section_range(data.len());

        // Universal Vmax detection for EDC16U1
        // Analysis of 3 files (A32, MB2, Altea) with CORRECT Big-Endian shows:
        //
        // MAP 1 (Zone D8xxx): [0005 0005 0005 0005 0005 0005] before + [7FFF 00FA 00FA] after
        // MAPS 2 & 3 (Zone EExxx): [7FFD 7FFD 0000 7FFF 01F4 7FFF] before first map, maps 10 bytes apart
        // MAP 4 (Zone F1xxx): [0000 0000 0863 0000 2FFD 01F4] before + [012C 0064 0258] after
        //
        // Default file (A32): Uses fixed addresses in homogeneous zones (0005, 0000, FFFF)

        // Helper to create map
        // Factor 0.01: raw value 25000 = 250 km/h displayed (same as StageX)
        let create_vmax_map = |addr: u32, confidence: f32| -> DetectedMap {
            let mut m = DetectedMap::new(addr, 2, MapDimensions::OneDimensional { length: 1 }, DataType::Int16);
            m.name = Some("Maximum Vehicle Speed".to_string());
            m.category = Some("Maximum Vehicle Speed".to_string());
            m.unit = Some("km/h".to_string());
            m.correction_factor = Some(0.01);
            m.confidence = confidence;
            m
        };

        // ===== MAP 1: Zone D8xxx =====
        // Pattern: [0005 0005 0005 0005 0005 0005] before + [7FFF 00FA 00FA] after
        // Value is typically 0x61A8 (25000 = 250 km/h)
        let d8_range_end = scan_end.min(0xD9000).saturating_sub(10);
        log::debug!("Vmax D8 scan range: 0x{:X} to 0x{:X}", 0xD8000usize, d8_range_end);

        for offset in 0xD8000..d8_range_end {
            if offset >= 12 && offset + 8 <= data.len() {
                // Read 6 words before
                let b1 = ((data[offset - 2] as u16) << 8) | data[offset - 1] as u16;
                let b2 = ((data[offset - 4] as u16) << 8) | data[offset - 3] as u16;
                let b3 = ((data[offset - 6] as u16) << 8) | data[offset - 5] as u16;
                let b4 = ((data[offset - 8] as u16) << 8) | data[offset - 7] as u16;
                let b5 = ((data[offset - 10] as u16) << 8) | data[offset - 9] as u16;
                let b6 = ((data[offset - 12] as u16) << 8) | data[offset - 11] as u16;

                // Read 3 words after
                let a1 = ((data[offset + 2] as u16) << 8) | data[offset + 3] as u16;
                let a2 = ((data[offset + 4] as u16) << 8) | data[offset + 5] as u16;
                let a3 = ((data[offset + 6] as u16) << 8) | data[offset + 7] as u16;

                let val = ((data[offset] as u16) << 8) | data[offset + 1] as u16;

                // Debug at specific address
                if offset == 0xD8E06 {
                    log::debug!("Vmax D8 check at 0x{:X}: b1=0x{:04X} b2=0x{:04X} b3=0x{:04X} b4=0x{:04X} b5=0x{:04X} b6=0x{:04X} val=0x{:04X} a1=0x{:04X} a2=0x{:04X} a3=0x{:04X}",
                        offset, b1, b2, b3, b4, b5, b6, val, a1, a2, a3);
                }

                // Pattern for tuned files: [0005 0005 0005 0005 0005 0005] before + [7FFF 00FA 00FA] after
                if b1 == 0x0005 && b2 == 0x0005 && b3 == 0x0005 &&
                   b4 == 0x0005 && b5 == 0x0005 && b6 == 0x0005 &&
                   a1 == 0x7FFF && a2 == 0x00FA && a3 == 0x00FA && val != 0x0005 {
                    let a32 = offset as u32;
                    log::debug!("Vmax D8 MATCH at 0x{:X}", offset);
                    if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                        found.insert(a32);
                        maps.push(create_vmax_map(a32, 0.95));
                    }
                }
            }
        }

        // Pattern B for D8xxx: Fixed address D8DB4 for default files (in 0005 zone)
        // ONLY apply if no D8xxx maps were found from Pattern A
        let has_d8_maps = found.iter().any(|&a| (a as usize) >= 0xD8000 && (a as usize) < 0xD9000);
        if !has_d8_maps {
            let offset = 0xD8DB4usize;
            if offset >= 6 && offset + 8 <= data.len() {
                let b1 = ((data[offset - 2] as u16) << 8) | data[offset - 1] as u16;
                let val = ((data[offset] as u16) << 8) | data[offset + 1] as u16;
                let a1 = ((data[offset + 2] as u16) << 8) | data[offset + 3] as u16;

                // Must be in 0005 zone (default untuned file)
                if b1 == 0x0005 && val == 0x0005 && a1 == 0x0005 {
                    let a32 = offset as u32;
                    if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                        found.insert(a32);
                        maps.push(create_vmax_map(a32, 0.80));
                    }
                }
            }
        }

        // ===== MAPS 2 & 3: Zone EExxx OR F0xxx =====
        // Pattern: [7FFD 7FFD 0000 7FFF 01F4 7FFF] before first map
        // Maps are 10 bytes apart, value is typically 0x7530 (30000 = 300 km/h)
        // Some files have these maps in EExxx zone, others in F0xxx zone

        // Scan both zones: EE000-EF000 and F0000-F1000
        let zones_2_3 = [(0xEE000usize, 0xEF000usize), (0xF0000usize, 0xF1000usize)];

        for (zone_start, zone_end) in zones_2_3.iter() {
            let range_end = scan_end.min(*zone_end).saturating_sub(20);

            for offset in *zone_start..range_end {
                if offset >= 12 && offset + 12 <= data.len() {
                    let b1 = ((data[offset - 12] as u16) << 8) | data[offset - 11] as u16;
                    let b2 = ((data[offset - 10] as u16) << 8) | data[offset - 9] as u16;
                    let b3 = ((data[offset - 8] as u16) << 8) | data[offset - 7] as u16;
                    let b4 = ((data[offset - 6] as u16) << 8) | data[offset - 5] as u16;
                    let b5 = ((data[offset - 4] as u16) << 8) | data[offset - 3] as u16;
                    let b6 = ((data[offset - 2] as u16) << 8) | data[offset - 1] as u16;

                    // Pattern: [7FFD 7FFD 0000 7FFF 01F4 7FFF] before (CORRECT Big-Endian)
                    if b1 == 0x7FFD && b2 == 0x7FFD && b3 == 0x0000 &&
                       b4 == 0x7FFF && b5 == 0x01F4 && b6 == 0x7FFF {
                        let a32 = offset as u32;
                        if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                            found.insert(a32);
                            maps.push(create_vmax_map(a32, 0.95));
                        }

                        // Second map at +10 bytes
                        let offset2 = offset + 10;
                        let a32_2 = offset2 as u32;
                        if !found.contains(&a32_2) && !detected_addresses.contains(&a32_2) {
                            found.insert(a32_2);
                            maps.push(create_vmax_map(a32_2, 0.95));
                        }
                    }
                }
            }
        }

        // ===== MAP 4: Zone F1xxx-F2xxx =====
        // Pattern A: [0000 0000 0863 0000 2FFD 01F4] before + [012C 0064 0258] after (MB2, Altea)
        for offset in 0xF1000..scan_end.min(0xF3000).saturating_sub(10) {
            if offset >= 12 && offset + 8 <= data.len() {
                let b1 = ((data[offset - 12] as u16) << 8) | data[offset - 11] as u16;
                let b2 = ((data[offset - 10] as u16) << 8) | data[offset - 9] as u16;
                let b3 = ((data[offset - 8] as u16) << 8) | data[offset - 7] as u16;
                let b4 = ((data[offset - 6] as u16) << 8) | data[offset - 5] as u16;
                let b5 = ((data[offset - 4] as u16) << 8) | data[offset - 3] as u16;
                let b6 = ((data[offset - 2] as u16) << 8) | data[offset - 1] as u16;

                let a1 = ((data[offset + 2] as u16) << 8) | data[offset + 3] as u16;
                let a2 = ((data[offset + 4] as u16) << 8) | data[offset + 5] as u16;
                let a3 = ((data[offset + 6] as u16) << 8) | data[offset + 7] as u16;

                // Pattern: [0000 0000 0863 0000 2FFD 01F4] before + [012C 0064 0258] after
                if b1 == 0x0000 && b2 == 0x0000 && b3 == 0x0863 &&
                   b4 == 0x0000 && b5 == 0x2FFD && b6 == 0x01F4 &&
                   a1 == 0x012C && a2 == 0x0064 && a3 == 0x0258 {
                    let a32 = offset as u32;
                    if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                        found.insert(a32);
                        maps.push(create_vmax_map(a32, 0.95));
                    }
                }
            }
        }

        // Pattern B: Fixed address F2D36 for default files (in FFFF zone)
        // ONLY apply if no F1xxx maps were found (indicating a default/untuned file)
        let has_f1_maps = found.iter().any(|&a| (a as usize) >= 0xF1000 && (a as usize) < 0xF2000);
        if !has_f1_maps {
            let offset = 0xF2D36usize;
            if offset >= 10 && offset + 4 <= data.len() {
                let b1 = ((data[offset - 2] as u16) << 8) | data[offset - 1] as u16;
                let b2 = ((data[offset - 4] as u16) << 8) | data[offset - 3] as u16;
                let val = ((data[offset] as u16) << 8) | data[offset + 1] as u16;
                let a1 = ((data[offset + 2] as u16) << 8) | data[offset + 3] as u16;

                // Must be in FFFF zone (default untuned file)
                if b1 == 0xFFFF && b2 == 0xFFFF && val == 0xFFFF && a1 == 0xFFFF {
                    let a32 = offset as u32;
                    if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                        found.insert(a32);
                        maps.push(create_vmax_map(a32, 0.80));
                    }
                }
            }
        }

        // ===== Motifs GÉNÉRALISÉS (indépendants des zones figées) =====
        // Validés sur 116 fichiers : vieux SW (016J/016G/016AL → 0xE6DC2 +
        // 0xFA95C), Transporter R5/1.9 (19000 = 190 km/h), Caddy (zones
        // décalées D9068/F08AA). Les zones figées ci-dessus restent pour la
        // compat : dédup par adresse via `found`.
        {
            let rd = |o: usize| ((data[o] as u16) << 8) | data[o + 1] as u16;
            let gstart = 0x80000usize.min(scan_end);
            let gend = scan_end.min(data.len().saturating_sub(60));
            let mut i = gstart;
            while i + 2 < gend {
                let v = rd(i);
                if !(8000..=30001).contains(&v) {
                    i += 2;
                    continue;
                }
                let after: Vec<u16> = (1..9).map(|k| rd(i + k * 2)).collect();
                // G2 « rangée paramètre » : v [any, 250, 250, 800-1100, 500,
                // 500, 500, 150] — le vmax ouvre une rangée de paramètres
                // fixe (hystérésis 250/250, temporisation ~900, 500×3, 150).
                let g2 = after[1] == 250
                    && after[2] == 250
                    && (800..=1100).contains(&after[3])
                    && after[4] == 500
                    && after[5] == 500
                    && after[6] == 500
                    && after[7] == 150;
                // G1 « bloc 7FFF » : fenêtre de 5 mots avant contenant au
                // moins deux 7FFF et un 0, et marqueur [100 48 48 48] dans
                // les 20 mots suivants.
                let mut g1 = false;
                if !g2 && i >= gstart + 10 {
                    let w: Vec<u16> = (1..=5).map(|k| rd(i - k * 2)).collect();
                    if w.iter().filter(|&&x| x == 0x7FFF).count() >= 2
                        && w.iter().any(|&x| x == 0)
                    {
                        let tail: Vec<u16> = (1..21).map(|k| rd(i + k * 2)).collect();
                        g1 = tail.windows(4).any(|t| t == [100, 48, 48, 48]);
                    }
                }
                // G3 « rangée paramètre V10 » : [100][6000][100] v [100][100]
                // (Touareg V10 : 25000 à 0xECFB8/0xEC724, identique sur les
                // deux fichiers de la famille)
                let mut g3 = false;
                if !g2 && !g1 && i >= gstart + 6 {
                    g3 = rd(i - 6) == 100
                        && rd(i - 4) == 6000
                        && rd(i - 2) == 100
                        && after[0] == 100
                        && after[1] == 100;
                }
                // G4 « trim high speed V10 » : [2400][0] v [50][3071]
                let mut g4 = false;
                if !g2 && !g1 && !g3 && i >= gstart + 4 {
                    g4 = rd(i - 4) == 9216
                        && rd(i - 2) == 0
                        && after[0] == 50
                        && after[1] == 3071;
                }
                if g2 || g1 || g3 || g4 {
                    let a32 = i as u32;
                    if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                        found.insert(a32);
                        maps.push(create_vmax_map(a32, 0.9));
                    }
                    // Paire G1 : v [7FFF 0 0 0] v2 (30000/29999 des vieux SW)
                    if g1
                        && after[0] == 0x7FFF
                        && after[1] == 0
                        && after[2] == 0
                        && after[3] == 0
                        && (8000..=30001).contains(&after[4])
                    {
                        let a2 = (i + 10) as u32;
                        if !found.contains(&a2) && !detected_addresses.contains(&a2) {
                            found.insert(a2);
                            maps.push(create_vmax_map(a2, 0.9));
                        }
                    }
                }
                i += 2;
            }
        }

        // Sort by address and renumber
        maps.sort_by_key(|m| m.address);
        for (i, m) in maps.iter_mut().enumerate() {
            m.name = Some(format!("Maximum Vehicle Speed {}", i + 1));
        }

        log::debug!("Maximum Vehicle Speed detection (EDC16U1): found {} maps", maps.len());
        for m in &maps {
            log::debug!("  Vmax at 0x{:X}", m.address);
        }

        maps
    }
    /// Detect SOI Selector maps by signature pattern
    /// SOI Selector MUST follow a valid SOI block (10 x 16x14 maps = 4480 bytes)
    /// Structure after SOI block: [16][RPM axis 32 bytes][14][IQ axis 28 bytes][10][00 0A][temp axis 20 bytes][selector 20 bytes]
    /// The selector values are in Little-Endian format (unlike most other maps in EDC16)
    pub fn detect_soi_selector_by_signature(&self, data: &[u8], detected_soi_addresses: &[u32]) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        // SOI Selector must be associated with a valid SOI block
        // Each SOI block has 10 maps of 448 bytes each = 4480 bytes
        // After the block: [16][RPM 32 bytes][14][IQ 28 bytes][10][00 0A + temp + selector]
        // Note: Some files have interleaved metadata - last N maps have 64-byte headers
        const SOI_MAP_SIZE: usize = 16 * 14 * 2;  // 448 bytes per map
        const SOI_BLOCK_SIZE: usize = SOI_MAP_SIZE * 10;  // 4480 bytes for 10 maps
        const INTERLEAVED_METADATA_SIZE: u32 = 64;

        // For each detected SOI block (look at first map addresses)
        let mut soi_block_starts: Vec<u32> = Vec::new();
        for addr in detected_soi_addresses {
            // Check if this is the first map of a block (Start of injection 00)
            // Try standard format first (all maps at 448-byte intervals)
            let is_block_start_standard = (0..10).all(|i| {
                let expected_addr = addr + (i * SOI_MAP_SIZE as u32);
                detected_soi_addresses.contains(&expected_addr)
            });

            // Try interleaved format (first N maps at 448-byte intervals, then maps at 512-byte intervals)
            let is_block_start_interleaved = (1..=5).any(|interleaved_count| {
                let contiguous_maps = 10 - interleaved_count;
                (0..10).all(|i| {
                    let expected_addr = if i < contiguous_maps {
                        addr + (i * SOI_MAP_SIZE as u32)
                    } else {
                        let interleaved_idx = i - contiguous_maps;
                        addr + (contiguous_maps as u32 * SOI_MAP_SIZE as u32)
                            + (interleaved_idx as u32 * (SOI_MAP_SIZE as u32 + INTERLEAVED_METADATA_SIZE))
                            + INTERLEAVED_METADATA_SIZE
                    };
                    detected_soi_addresses.contains(&expected_addr)
                })
            });

            if (is_block_start_standard || is_block_start_interleaved) && !soi_block_starts.contains(addr) {
                soi_block_starts.push(*addr);
            }
        }

        log::debug!("🔍 [EDC16] Found {} SOI blocks for Selector detection", soi_block_starts.len());

        for block_start in &soi_block_starts {
            // After the 10 SOI maps, there's metadata:
            // Offset from block_start: 4480 (block) + 2 (16) + 32 (RPM) + 2 (14) + 28 (IQ) + 2 (10) = 4546
            // For interleaved format, add up to 5*64 = 320 extra bytes
            // Then: 00 0A + temp axis + selector
            let metadata_start = *block_start as usize + SOI_BLOCK_SIZE;

            // Search for 00 0A pattern in a reasonable range after the SOI block
            // The exact position can vary, so search within 520 bytes after metadata
            // (200 standard + 320 for max interleaved metadata = 520)
            let search_start = metadata_start;
            let search_end = (metadata_start + 520).min(data.len().saturating_sub(42));

            for offset in (search_start..search_end).step_by(2) {
                if offset + 42 >= data.len() {
                    break;
                }

                // Check for 00 0A (10 columns header)
                if data[offset] == 0x00 && data[offset + 1] == 0x0A {
                    // Read 10 temperature values (Big-Endian, Kelvin * 10)
                    let temp_start = offset + 2;
                    let mut temp_values = Vec::with_capacity(10);
                    let mut temp_valid = true;

                    for i in 0..10 {
                        let val = ((data[temp_start + i * 2] as u16) << 8) | (data[temp_start + i * 2 + 1] as u16);
                        temp_values.push(val);
                        // Temperature range: 2300-3800 (Kelvin * 10, -43°C to +107°C)
                        if val < 2300 || val > 3800 {
                            temp_valid = false;
                            break;
                        }
                    }

                    if !temp_valid {
                        continue;
                    }

                    // Check temperature axis is increasing
                    let mut increasing = true;
                    for i in 1..10 {
                        if temp_values[i] <= temp_values[i - 1] {
                            increasing = false;
                            break;
                        }
                    }

                    if !increasing {
                        continue;
                    }

                    // Read selector values (Little-Endian!)
                    let selector_addr = offset + 22;
                    let mut selector_values = Vec::with_capacity(10);

                    for i in 0..10 {
                        // LITTLE-ENDIAN: low byte first
                        let val = ((data[selector_addr + i * 2 + 1] as u16) << 8) | (data[selector_addr + i * 2] as u16);
                        selector_values.push(val);
                    }

                    // Validate selector pattern:
                    // - All values 0-9
                    // - Values must be non-decreasing (monotonic)
                    // - First value is 0
                    // - At least 5 unique values (variety)
                    let all_in_range = selector_values.iter().all(|&v| v <= 9);
                    let first_is_zero = selector_values[0] == 0;
                    let mut monotonic = true;
                    for i in 1..10 {
                        if selector_values[i] < selector_values[i - 1] {
                            monotonic = false;
                            break;
                        }
                    }
                    let unique_count = {
                        let mut unique: Vec<u16> = selector_values.clone();
                        unique.sort();
                        unique.dedup();
                        unique.len()
                    };

                    if all_in_range && monotonic && first_is_zero && unique_count >= 5 {
                        log::debug!("🎯 [EDC16] Found SOI Selector at 0x{:X} (after SOI block at 0x{:X}), values: {:?}",
                                  selector_addr, block_start, selector_values);

                        let mut map = DetectedMap::new(
                            selector_addr as u32,
                            20, // 10 * 2 bytes
                            MapDimensions::OneDimensional { length: 10 },
                            DataType::UInt16, // Unsigned since values are 0-9
                        );
                        map.name = Some("SOI Selector".to_string());
                        map.category = Some(MapCategory::StartOfInjection.display_name().to_string());
                        map.unit = Some("[-]".to_string());
                        map.correction_factor = Some(1.0); // No correction, values are 0-9 directly
                        map.offset = Some(0.0);
                        map.confidence = 0.95;
                        map.x_axis_address = Some((offset + 2) as u32); // Temperature axis (Big-Endian)
                        map.x_label = Some("Water temp [°C]".to_string());
                        map.x_axis_correction = Some(0.1);
                        map.x_axis_offset = Some(-273.14);
                        // SOI Selector data is in LITTLE-ENDIAN format (unlike most EDC16 maps)
                        map.is_little_endian = Some(true);

                        maps.push(map);
                        break; // Only one selector per SOI block
                    }
                }
            }
        }

        log::debug!("🔧 [EDC16] SOI Selector detection: found {} maps (associated with {} SOI blocks)", maps.len(), soi_block_starts.len());
        maps
    }

    /// Parse the implicit (header-less) 8x13 Smoke Limiter by MAP block that
    /// follows the [00 02 00 02] anchor: [anchor 20 bytes][8 RPM values]
    /// [13 boost values][8x13 data]. Axes only are validated — the data is
    /// routinely all zero (slot present, ECU running on the MAF map), so it
    /// must never gate the detection. (Porté depuis l'EDC16U34.)
    fn parse_implicit_smoke_map(
        &self,
        data: &[u8],
        anchor: usize,
        detected: &HashSet<u32>,
    ) -> Option<DetectedMap> {
        const IROWS: usize = 8;
        const ICOLS: usize = 13;
        let y0 = anchor + 20;
        let x0 = y0 + IROWS * 2;
        let d0 = x0 + ICOLS * 2;
        let dend = d0 + IROWS * ICOLS * 2;
        if anchor + 12 > data.len() || dend > data.len() {
            return None;
        }
        if data[anchor..anchor + 6] != [0x00, 0x02, 0x00, 0x02, 0x00, 0x00] {
            return None;
        }
        let mn = u16::from_be_bytes([data[anchor + 8], data[anchor + 9]]);
        let mx = u16::from_be_bytes([data[anchor + 10], data[anchor + 11]]);
        if !(500..=1500).contains(&mn) || mn >= mx || mx > 3200 {
            return None;
        }
        let y: Vec<u16> = (0..IROWS)
            .map(|i| u16::from_be_bytes([data[y0 + i * 2], data[y0 + i * 2 + 1]]))
            .collect();
        if !(1500..=3500).contains(&y[0])
            || !(4000..=6300).contains(&y[IROWS - 1])
            || !Self::axis_ascending_tolerant(&y, 2)
        {
            return None;
        }
        let x: Vec<u16> = (0..ICOLS)
            .map(|i| u16::from_be_bytes([data[x0 + i * 2], data[x0 + i * 2 + 1]]))
            .collect();
        if x[0] > 1100 || x[ICOLS - 1] > 3200 || !Self::axis_ascending_tolerant(&x, 2) {
            return None;
        }
        if detected.contains(&(d0 as u32)) {
            return None;
        }

        let mut map = DetectedMap::new(
            d0 as u32,
            IROWS * ICOLS * 2,
            MapDimensions::TwoDimensional { rows: IROWS, cols: ICOLS },
            DataType::Int16,
        );
        map.name = Some("Smoke Limiter by MAP".to_string());
        map.category = Some("Smoke Limiter".to_string());
        map.unit = Some("mg/stroke".to_string());
        map.correction_factor = Some(0.01);
        map.offset = Some(0.0);
        map.confidence = 0.9;
        map.y_axis_address = Some(y0 as u32);
        map.y_label = Some("rpm".to_string());
        map.y_axis_correction = Some(1.0);
        map.y_axis_offset = Some(0.0);
        map.x_axis_address = Some(x0 as u32);
        map.x_label = Some("mbar".to_string());
        map.x_axis_correction = Some(1.0);
        map.x_axis_offset = Some(0.0);
        log::debug!("🎯 [EDC16] Found Smoke Limiter by MAP (implicit 8x13) at 0x{:X}", d0);
        Some(map)
    }

    /// Variante explicite 11x11 du slot Smoke Limiter by MAP à MAF_end+0xE0
    /// (famille 2.0L 140 016AT/FF/G/FJ/FH/F) : [000B][000B][RPM 11][MAP 11]
    /// [data 11x11]. Sur certaines calibrations (016FJ), le header et les 8
    /// premières valeurs RPM sont écrasés par un stub 2x2 désactivé — la
    /// géométrie et les données restent identiques : la map est alors émise
    /// sans axe Y (accepté seulement si `allow_stub`).
    fn parse_smoke_map_11x11(
        &self,
        data: &[u8],
        start: usize,
        allow_stub: bool,
        detected: &HashSet<u32>,
    ) -> Option<DetectedMap> {
        const N: usize = 11;
        let y0 = start + 4;
        let x0 = y0 + N * 2;
        let d0 = x0 + N * 2;
        if d0 + N * N * 2 > data.len() {
            return None;
        }
        let rd = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
        let explicit = data[start] == 0x00
            && data[start + 1] == 0x0B
            && data[start + 2] == 0x00
            && data[start + 3] == 0x0B;
        let stub = allow_stub && rd(start) == 2 && rd(start + 2) == 2;
        if !explicit && !stub {
            return None;
        }
        // Axe X (pression MAP, mbar) : 11 valeurs strictement croissantes
        let x: Vec<u16> = (0..N).map(|i| rd(x0 + i * 2)).collect();
        if !(500..=1500).contains(&x[0]) || x[N - 1] > 3500 || x.windows(2).any(|w| w[0] >= w[1]) {
            return None;
        }
        if explicit {
            let y: Vec<u16> = (0..N).map(|i| rd(y0 + i * 2)).collect();
            if y[0] < 500 || y[N - 1] > 6000 || y.windows(2).any(|w| w[0] >= w[1]) {
                return None;
            }
        }
        // Données IQ (0.01 mg/str) plausibles : bornées, au moins un plein-charge
        let vals: Vec<u16> = (0..N * N).map(|i| rd(d0 + i * 2)).collect();
        if vals.iter().any(|&v| v > 9000) || !vals.iter().any(|&v| v >= 2500) {
            return None;
        }
        if detected.contains(&(d0 as u32)) {
            return None;
        }

        let mut map = DetectedMap::new(
            d0 as u32,
            N * N * 2,
            MapDimensions::TwoDimensional { rows: N, cols: N },
            DataType::Int16,
        );
        map.name = Some("Smoke Limiter by MAP".to_string());
        map.category = Some("Smoke Limiter".to_string());
        map.unit = Some("mg/stroke".to_string());
        map.correction_factor = Some(0.01);
        map.offset = Some(0.0);
        map.confidence = if explicit { 0.92 } else { 0.85 };
        if explicit {
            map.y_axis_address = Some(y0 as u32);
            map.y_label = Some("rpm".to_string());
            map.y_axis_correction = Some(1.0);
            map.y_axis_offset = Some(0.0);
        }
        map.x_axis_address = Some(x0 as u32);
        map.x_label = Some("mbar".to_string());
        map.x_axis_correction = Some(1.0);
        map.x_axis_offset = Some(0.0);
        log::debug!(
            "🎯 [EDC16U1] Found Smoke Limiter by MAP (11x11 {}) at 0x{:X}",
            if explicit { "explicit" } else { "stub" },
            d0
        );
        Some(map)
    }

    /// Variante V10 du slot Smoke Limiter by MAP : header EXPLICITE
    /// [00 10][00 0A..0D] avec axe X pression (mbar, ≤3200) dans la fenêtre
    /// qui suit la MAF (016AA : 16x10 à MAF_end+0x3C, données constantes
    /// 6000). La passe principale ne descend pas à 10 colonnes (trop de
    /// faux positifs hors fenêtre) — ici l'ancrage positionnel sécurise.
    fn parse_explicit_smoke_map_window(
        &self,
        data: &[u8],
        start: usize,
        detected: &HashSet<u32>,
    ) -> Option<DetectedMap> {
        const ROWS: usize = 16;
        if start + 4 > data.len() {
            return None;
        }
        let rd = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
        if data[start] != 0x00 || data[start + 1] != 0x10 || data[start + 2] != 0x00 {
            return None;
        }
        let cols = data[start + 3] as usize;
        if !(10..=13).contains(&cols) {
            return None;
        }
        let y0 = start + 4;
        let x0 = y0 + ROWS * 2;
        let d0 = x0 + cols * 2;
        if d0 + ROWS * cols * 2 > data.len() {
            return None;
        }
        let y: Vec<u16> = (0..ROWS).map(|i| rd(y0 + i * 2)).collect();
        if y[0] < 300
            || y[ROWS - 1] > 6000
            || y.windows(2).any(|w| w[0] >= w[1])
            || y[ROWS - 1].saturating_sub(y[0]) < 3000
        {
            return None;
        }
        let x: Vec<u16> = (0..cols).map(|i| rd(x0 + i * 2)).collect();
        if x[0] > 1100 || x[cols - 1] > 3200 || x.windows(2).any(|w| w[0] >= w[1]) {
            return None;
        }
        if detected.contains(&(d0 as u32)) {
            return None;
        }
        let vals: Vec<u16> = (0..ROWS * cols).map(|i| rd(d0 + i * 2)).collect();
        if vals.iter().any(|&v| v > 9000) {
            return None;
        }

        let mut map = DetectedMap::new(
            d0 as u32,
            ROWS * cols * 2,
            MapDimensions::TwoDimensional { rows: ROWS, cols },
            DataType::Int16,
        );
        map.name = Some("Smoke Limiter by MAP".to_string());
        map.category = Some("Smoke Limiter".to_string());
        map.unit = Some("mg/stroke".to_string());
        map.correction_factor = Some(0.01);
        map.offset = Some(0.0);
        map.confidence = 0.9;
        map.y_axis_address = Some(y0 as u32);
        map.y_label = Some("rpm".to_string());
        map.y_axis_correction = Some(1.0);
        map.y_axis_offset = Some(0.0);
        map.x_axis_address = Some(x0 as u32);
        map.x_label = Some("mbar".to_string());
        map.x_axis_correction = Some(1.0);
        map.x_axis_offset = Some(0.0);
        log::debug!("🎯 [EDC16U1] Found Smoke Limiter by MAP (explicit 16x{} window) at 0x{:X}", cols, d0);
        Some(map)
    }

    /// Detect Smoke Limiter by MAP and Smoke Limiter by MAF maps by signature pattern
    /// These maps have dimensions 16x13 or 16x12 with headers [00 10][00 0D] or [00 10][00 0C]
    /// Structure: [00 10][cols][Y axis RPM 32 bytes][X axis][Map data]
    /// Distinction:
    /// - Smoke Limiter by MAP: X axis values 300-2500 (boost pressure mbar)
    /// - Smoke Limiter by MAF: X axis values 3000-15000 (air mass mg/stroke)
    pub fn detect_iq_limit_maps_by_signature(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut maf_map_found = false;  // Track if we've found the first MAF map

        const ROWS: usize = 16;
        const HEADER_SIZE: usize = 4;

        let (scan_start, scan_end) = self.get_data_section_range(data.len());

        // Search for signatures [00 10][00 0D] (16x13) or [00 10][00 0C] (16x12)
        for offset in (scan_start..scan_end.saturating_sub(600)).step_by(2) {
            // Check header signature: [00 10][00 0C] or [00 10][00 0D]
            if data[offset] != 0x00 || data[offset + 1] != 0x10 || data[offset + 2] != 0x00 {
                continue;
            }

            let cols = data[offset + 3] as usize;
            // 16x11 : variante Smoke Limiter by MAP (016CC @0xDF9DA — même
            // layout que VWJetta1.9 côté U34 et JA/MAXENCE côté U31)
            if cols != 11 && cols != 12 && cols != 13 {
                continue;
            }

            let y_axis_size = ROWS * 2;  // 32 bytes (16 RPM values)
            let x_axis_size = cols * 2;  // 24 or 26 bytes
            let data_size = ROWS * cols * 2;  // 384 or 416 bytes

            // Calculate addresses
            let y_axis_addr = offset + HEADER_SIZE;
            let x_axis_addr = y_axis_addr + y_axis_size;
            let data_addr = x_axis_addr + x_axis_size;

            // Skip if already detected
            if detected.contains(&(data_addr as u32)) {
                continue;
            }

            // Read Y axis (RPM) - should be 16 increasing values in range 300-6000
            let mut y_axis_values = Vec::with_capacity(ROWS);
            let mut y_axis_valid = true;
            for i in 0..ROWS {
                let val = ((data[y_axis_addr + i * 2] as u16) << 8) | (data[y_axis_addr + i * 2 + 1] as u16);
                y_axis_values.push(val);
                // RPM range check: 300-6000 rpm
                if val < 300 || val > 6000 {
                    y_axis_valid = false;
                    break;
                }
            }

            if !y_axis_valid {
                continue;
            }

            // Y croissant avec tolérance d'encoches (axes tunés : 016FF a
            // un point RPM édité 1000→844 sur sa smoke MAF)
            if !Self::axis_ascending_tolerant(&y_axis_values, 2) {
                continue;
            }

            // Additional validation: RPM axis should span a reasonable range (at least 3000 rpm)
            // This filters out false positives with narrow RPM ranges
            let rpm_span = y_axis_values[ROWS - 1].saturating_sub(y_axis_values[0]);
            if rpm_span < 3000 {
                continue;
            }

            // Read X axis values (12 or 13 values)
            let mut x_axis_values = Vec::with_capacity(cols);
            for i in 0..cols {
                let val = ((data[x_axis_addr + i * 2] as u16) << 8) | (data[x_axis_addr + i * 2 + 1] as u16);
                x_axis_values.push(val);
            }

            // X croissant avec tolérance d'encoches (axes tunés)
            if !Self::axis_ascending_tolerant(&x_axis_values, 2) {
                continue;
            }

            // Determine map type based on X axis values
            let first_x = x_axis_values[0];
            let last_x = x_axis_values[cols - 1];

            let (mut map_name, _x_label, mut x_unit, mut x_factor): (&str, &str, &str, f64);

            if first_x <= 1000 && last_x <= 3000 {
                // X axis is boost pressure (MAP): 300-2500 mbar
                map_name = "Smoke Limiter by MAP";
                _x_label = "Boost pressure";
                x_unit = "mbar";
                x_factor = 1.0;
            } else if first_x >= 2000 && last_x >= 5000 {
                // X axis is air mass - need to distinguish between MAF and Lambda
                // by analyzing the DATA values (not just axis range)
                // Smoke Limiter by MAF: Data values 200-8000 (IQ in 0.01 mg/stroke)
                // Lambda Calculations: Data values 800-1500 (lambda ratio x 1000)
                if last_x < 8500 || last_x > 12500 {
                    // X axis outside reasonable air mass range
                    log::debug!("🔧 [EDC16] Skipping IQ Limit map at 0x{:X} (X axis ends at {} - outside range)",
                              data_addr, last_x);
                    continue;
                }

                // Pre-read data to determine map type by value range
                // Key distinction:
                // - MAF maps: IQ values typically 0-8000 with many values > 2000 (high IQ at high load)
                // - Lambda maps: Lambda ratios typically 800-1500 with very few values > 2000
                let total_count = ROWS * cols;
                let mut max_val: u16 = 0;
                let mut high_iq_count = 0;  // Values > 2000 (definitely IQ, not lambda)
                let mut lambda_range_count = 0;  // Values in 800-1600 range

                for i in 0..total_count {
                    let val = ((data[data_addr + i * 2] as u16) << 8) | (data[data_addr + i * 2 + 1] as u16);
                    if val > max_val {
                        max_val = val;
                    }
                    if val > 2000 && val <= 8000 {
                        high_iq_count += 1;
                    }
                    if val >= 800 && val <= 1600 {
                        lambda_range_count += 1;
                    }
                }

                // Classification logic:
                // - First air mass map with X axis 3000-11000 is MAF (IQ limiter)
                // - Subsequent maps with same axis type are Lambda (if values are in 800-1600 range)
                // - Maps with very high values (>2500) are always MAF regardless of order

                if max_val > 2500 || high_iq_count > total_count * 10 / 100 {
                    // Definitely MAF - has high IQ values
                    map_name = "Smoke Limiter by MAF";
                    _x_label = "Air mass";
                    x_unit = "mg/stroke";
                    x_factor = 1.0;
                    maf_map_found = true;
                } else if !maf_map_found && last_x <= 11000 {
                    // First air mass map - classify as MAF
                    map_name = "Smoke Limiter by MAF";
                    _x_label = "Air mass";
                    x_unit = "mg/stroke";
                    x_factor = 1.0;
                    maf_map_found = true;
                } else if lambda_range_count > total_count * 50 / 100 {
                    // Subsequent maps with lambda-like values
                    map_name = "Smoke Limiter by Lambda";
                    _x_label = "Air mass";
                    x_unit = "mg/stroke";
                    x_factor = 0.1;
                } else {
                    // Default to Lambda for subsequent ambiguous cases
                    map_name = "Smoke Limiter by Lambda";
                    _x_label = "Air mass";
                    x_unit = "mg/stroke";
                    x_factor = 0.1;
                }
            } else {
                // Unknown X axis type, skip
                continue;
            }

            // Validate map data based on type
            let mut valid_data_count = 0;
            let total_data_count = ROWS * cols;
            for i in 0..total_data_count {
                let val = ((data[data_addr + i * 2] as u16) << 8) | (data[data_addr + i * 2 + 1] as u16);
                if map_name == "Smoke Limiter by Lambda" {
                    // Lambda values: 700-1500 (lambda 0.7-1.5)
                    if val >= 500 && val <= 2000 {
                        valid_data_count += 1;
                    }
                } else {
                    // IQ values: 200-8000
                    if val <= 8000 || val >= 6000 {
                        valid_data_count += 1;
                    }
                }
            }

            // At least 70% of data should be valid (relaxed for mixed maps).
            // Exception : un slot Lambda ENTIÈREMENT à zéro est légitime
            // (SDI 016N : 3 slots 16x13 x=3000..11500 tout à zéro — slot
            // présent, moteur atmosphérique sans régulation lambda active).
            if valid_data_count < (total_data_count * 70 / 100) {
                let all_zero = (0..total_data_count).all(|i| {
                    data[data_addr + i * 2] == 0 && data[data_addr + i * 2 + 1] == 0
                });
                if !(map_name == "Smoke Limiter by Lambda" && all_zero) {
                    continue;
                }
            }

            // Filter out maps that are empty or have all values below threshold
            // This prevents detecting zeroed-out or inactive smoke limiter maps
            // Threshold: 3500 raw = 35 mg/stroke displayed (with 0.01 correction factor)
            const MIN_SIGNIFICANT_VALUE: u16 = 3500;
            let mut has_significant_value = false;

            // Only apply this filter for MAF maps : un by-MAP entièrement à
            // zéro est LÉGITIME (slot présent, gestion sur la map MAF — même
            // règle que U31/U34), et les Lambda ont leurs propres plages.
            if map_name == "Smoke Limiter by MAF" {
                let mut all_zero = true;
                for i in 0..total_data_count {
                    let val = ((data[data_addr + i * 2] as u16) << 8) | (data[data_addr + i * 2 + 1] as u16);
                    if val > MIN_SIGNIFICANT_VALUE {
                        has_significant_value = true;
                        break;
                    }
                    if val != 0 {
                        all_zero = false;
                    }
                }

                // Un slot MAF ENTIÈREMENT à zéro est légitime (Transporter
                // R5 : slot présent à 0xED538, gestion sur la map lambda) —
                // même règle que le by-MAP. On ne rejette que les données
                // non nulles mais toutes faibles (bruit)… sauf si elles ont
                // un profil LAMBDA (V10 : 16x13 à 0xECFC2, valeurs
                // 1120-1270, classé MAF car premier de la zone air-mass).
                if !has_significant_value && !all_zero {
                    let lambda_like = (0..total_data_count)
                        .filter(|&i| {
                            let v = ((data[data_addr + i * 2] as u16) << 8)
                                | (data[data_addr + i * 2 + 1] as u16);
                            (800..=1600).contains(&v)
                        })
                        .count();
                    if lambda_like * 2 > total_data_count {
                        map_name = "Smoke Limiter by Lambda";
                        x_unit = "mg/stroke";
                        x_factor = 0.1;
                    } else {
                        log::debug!("🔧 [EDC16] Skipping {} at 0x{:X} - all values are zero or below {} (inactive/zeroed map)",
                                  map_name, data_addr, MIN_SIGNIFICANT_VALUE);
                        continue;
                    }
                }
            }

            log::debug!("🎯 [EDC16] Found {} at 0x{:X} (Y axis: {}-{} RPM, X axis: {}-{} {})",
                      map_name, data_addr, y_axis_values[0], y_axis_values[ROWS - 1],
                      first_x, last_x, x_unit);

            // Create the map
            let mut map = DetectedMap::new(
                data_addr as u32,
                data_size,
                MapDimensions::TwoDimensional { rows: ROWS, cols },
                DataType::Int16,
            );
            map.name = Some(map_name.to_string());
            map.category = Some("Smoke Limiter".to_string());

            // Lambda maps have different unit and factor than IQ maps
            if map_name == "Smoke Limiter by Lambda" {
                map.unit = Some("-".to_string()); // Lambda is a ratio, no unit
                map.correction_factor = Some(0.001); // Raw 1250 → 1.25 lambda
            } else {
                map.unit = Some("mg/stroke".to_string());
                map.correction_factor = Some(0.01);
            }
            map.offset = Some(0.0);
            map.confidence = 0.92;

            // Y axis (RPM)
            map.y_axis_address = Some(y_axis_addr as u32);
            map.y_label = Some("rpm".to_string());
            map.y_axis_correction = Some(1.0);
            map.y_axis_offset = Some(0.0);

            // X axis (MAP or MAF)
            map.x_axis_address = Some(x_axis_addr as u32);
            map.x_label = Some(x_unit.to_string());
            map.x_axis_correction = Some(x_factor);
            map.x_axis_offset = Some(0.0);

            maps.push(map);
        }

        // Émission de la variante IMPLICITE « Smoke Limiter by MAP » (règle
        // d'or portée depuis U34/U31) : le slot by-MAP est TOUJOURS à
        // MAF_data_end + 0xE0, parfois sans header (bloc 8x13 derrière
        // l'ancre [00 02 00 02]), avec des données souvent à zéro (gestion
        // sur la map MAF). C'est ce qui manquait sur ~80 % des U1.
        let mut implicit_maps: Vec<DetectedMap> = Vec::new();
        for m in &maps {
            if m.name.as_deref() != Some("Smoke Limiter by MAF") {
                continue;
            }
            let map_end = m.address as usize + m.size;
            // Ordre des variantes (du plus standard au plus permissif) :
            // 1. 8x13 implicite à +0xE0 (comportement historique) ;
            // 2. 11x11 explicite [000B][000B] à +0xE0 (2.0L 140 016AT…) ;
            // 3. ancre 8x13 dans la fenêtre +0x00..+0x140 (016J : +0x3C,
            //    axe MAP paddé en pas de 1) ;
            // 4. 11x11 sans header (stub 2x2 écrasant le début, 016FJ).
            let mut found = self.parse_implicit_smoke_map(data, map_end + 0xE0, detected);
            if found.is_none() {
                found = self.parse_smoke_map_11x11(data, map_end + 0xE0, false, detected);
            }
            if found.is_none() {
                for off in (0..=0x140usize).step_by(2) {
                    if off == 0xE0 {
                        continue;
                    }
                    if let Some(im) = self.parse_implicit_smoke_map(data, map_end + off, detected) {
                        found = Some(im);
                        break;
                    }
                }
            }
            if found.is_none() {
                // Variante V10 : header explicite 16x10-13 à axe pression
                // dans la même fenêtre (016AA : +0x3C)
                for off in (0..=0x140usize).step_by(2) {
                    if let Some(im) =
                        self.parse_explicit_smoke_map_window(data, map_end + off, detected)
                    {
                        found = Some(im);
                        break;
                    }
                }
            }
            if found.is_none() {
                found = self.parse_smoke_map_11x11(data, map_end + 0xE0, true, detected);
            }
            if let Some(imap) = found {
                if !maps.iter().any(|e| e.address == imap.address)
                    && !implicit_maps.iter().any(|e| e.address == imap.address)
                {
                    implicit_maps.push(imap);
                }
            }
        }
        maps.extend(implicit_maps);
        maps.sort_by_key(|m| m.address);

        // Filter out consecutive duplicate maps that share the same axis values
        // Only filter if maps are close together (within 0x2000 bytes) - these are variants
        // Maps in different codeblocks (far apart) should be kept even with same axes
        let mut filtered_maps: Vec<DetectedMap> = Vec::new();
        let total_candidates = maps.len();

        for map in maps {
            let map_type = map.name.clone().unwrap_or_default();

            // Get dimensions to know how many axis values
            let (rows, cols) = match map.dimensions {
                MapDimensions::TwoDimensional { rows, cols } => (rows, cols),
                _ => {
                    filtered_maps.push(map);
                    continue;
                }
            };

            // Read axis values from data
            let y_axis_addr = map.y_axis_address.unwrap_or(0) as usize;
            let x_axis_addr = map.x_axis_address.unwrap_or(0) as usize;

            let first_y = if y_axis_addr + 1 < data.len() {
                ((data[y_axis_addr] as u16) << 8) | (data[y_axis_addr + 1] as u16)
            } else { 0 };
            let last_y_offset = y_axis_addr + (rows - 1) * 2;
            let last_y = if last_y_offset + 1 < data.len() {
                ((data[last_y_offset] as u16) << 8) | (data[last_y_offset + 1] as u16)
            } else { 0 };

            let first_x = if x_axis_addr + 1 < data.len() {
                ((data[x_axis_addr] as u16) << 8) | (data[x_axis_addr + 1] as u16)
            } else { 0 };
            let last_x_offset = x_axis_addr + (cols - 1) * 2;
            let last_x = if last_x_offset + 1 < data.len() {
                ((data[last_x_offset] as u16) << 8) | (data[last_x_offset + 1] as u16)
            } else { 0 };

            // Check if this is a duplicate of a recent map (within 0x2000 bytes)
            // Skip duplicate check for Lambda maps - they intentionally have multiple maps with same axes
            let mut is_duplicate = false;
            if map_type == "Smoke Limiter by Lambda" {
                // Lambda maps are allowed to have multiple instances with same axes
                // They represent different operating conditions (FlMng_rLmbdSmkLim0, 1, 2, etc.)
                filtered_maps.push(map);
                continue;
            }
            for existing in filtered_maps.iter().rev().take(5) {
                // Only compare maps of the same type
                if existing.name.as_deref() != Some(&map_type) {
                    continue;
                }

                // Only filter if maps are close together (within 0x2000 bytes = 8KB)
                let distance = if map.address > existing.address {
                    map.address - existing.address
                } else {
                    existing.address - map.address
                };

                if distance > 0x2000 {
                    continue;  // Maps are too far apart, likely different codeblocks
                }

                // Get existing map's axis values
                let (ex_rows, ex_cols) = match existing.dimensions {
                    MapDimensions::TwoDimensional { rows, cols } => (rows, cols),
                    _ => continue,
                };

                let ex_y_addr = existing.y_axis_address.unwrap_or(0) as usize;
                let ex_x_addr = existing.x_axis_address.unwrap_or(0) as usize;

                let ex_first_y = if ex_y_addr + 1 < data.len() {
                    ((data[ex_y_addr] as u16) << 8) | (data[ex_y_addr + 1] as u16)
                } else { 0 };
                let ex_last_y_offset = ex_y_addr + (ex_rows - 1) * 2;
                let ex_last_y = if ex_last_y_offset + 1 < data.len() {
                    ((data[ex_last_y_offset] as u16) << 8) | (data[ex_last_y_offset + 1] as u16)
                } else { 0 };

                let ex_first_x = if ex_x_addr + 1 < data.len() {
                    ((data[ex_x_addr] as u16) << 8) | (data[ex_x_addr + 1] as u16)
                } else { 0 };
                let ex_last_x_offset = ex_x_addr + (ex_cols - 1) * 2;
                let ex_last_x = if ex_last_x_offset + 1 < data.len() {
                    ((data[ex_last_x_offset] as u16) << 8) | (data[ex_last_x_offset + 1] as u16)
                } else { 0 };

                // If axes are identical, this is a duplicate
                if first_y == ex_first_y && last_y == ex_last_y &&
                   first_x == ex_first_x && last_x == ex_last_x {
                    is_duplicate = true;
                    log::debug!("🔧 [EDC16] Filtering duplicate {} at 0x{:X} (same axes as 0x{:X}, distance: 0x{:X})",
                              map_type, map.address, existing.address, distance);
                    break;
                }
            }

            if !is_duplicate {
                filtered_maps.push(map);
            }
        }

        log::debug!("🔧 [EDC16] IQ Limit detection: found {} maps (filtered {} duplicates from {} candidates)",
                  filtered_maps.len(), total_candidates - filtered_maps.len(), total_candidates);
        filtered_maps
    }


    /// Detect small Lambda Smoke Limit maps (2x2, 2x3, 3x2)
    /// These are smaller calibration maps for Lambda calculations at specific operating points
    ///
    /// Structure:
    /// - Header: [00 rows][00 cols] (e.g., [00 02][00 02] for 2x2 or [00 03][00 02] for 3x2)
    /// - Y-axis: rows × 2 bytes (RPM values: 1000-4000)
    /// - X-axis: cols × 2 bytes (Air mass values: 3000-10000 mg/stroke)
    /// - Data: rows × cols × 2 bytes (Lambda calculation values)
    ///
    /// Distinction from other small maps:
    /// - X-axis values in air mass range (3000-10000)
    /// - Y-axis values in RPM range (1000-4000)
    /// - Located near the larger Lambda Calculation maps
    pub fn detect_small_lambda_maps(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        
        let (scan_start, scan_end) = self.get_data_section_range(data.len());
        
        // Search for small 2D map headers: [00 02][00 02], [00 03][00 02], [00 02][00 03]
        for offset in (scan_start..scan_end.saturating_sub(100)).step_by(2) {
            // Check header signature
            if data[offset] != 0x00 || data[offset + 2] != 0x00 {
                continue;
            }
            
            let rows = data[offset + 1] as usize;
            let cols = data[offset + 3] as usize;
            
            // Only look for small maps (2-3 rows/cols)
            if rows < 2 || rows > 3 || cols < 2 || cols > 3 {
                continue;
            }
            
            let y_axis_size = rows * 2;
            let x_axis_size = cols * 2;
            let data_size = rows * cols * 2;
            
            let y_axis_addr = offset + 4;
            let x_axis_addr = y_axis_addr + y_axis_size;
            let data_addr = x_axis_addr + x_axis_size;
            
            // Check bounds
            if data_addr + data_size > data.len() {
                continue;
            }
            
            // Skip if already detected
            if detected.contains(&(data_addr as u32)) {
                continue;
            }
            
            // Read Y axis (RPM)
            let mut y_values = Vec::with_capacity(rows);
            for i in 0..rows {
                let val = ((data[y_axis_addr + i * 2] as u16) << 8) | (data[y_axis_addr + i * 2 + 1] as u16);
                y_values.push(val);
            }
            
            // Validate Y axis: should be RPM values (500-5000 range, increasing)
            // Also require reasonable span (at least 500 RPM for 2 values, 1000 for 3 values)
            let y_span = y_values.last().unwrap_or(&0).saturating_sub(*y_values.first().unwrap_or(&0));
            let min_y_span = if rows == 2 { 500 } else { 1000 };
            let y_valid = y_values.iter().all(|&v| v >= 500 && v <= 5000)
                && y_values.windows(2).all(|w| w[1] >= w[0])
                && y_span >= min_y_span;

            if !y_valid {
                continue;
            }

            // Read X axis (Air mass)
            let mut x_values = Vec::with_capacity(cols);
            for i in 0..cols {
                let val = ((data[x_axis_addr + i * 2] as u16) << 8) | (data[x_axis_addr + i * 2 + 1] as u16);
                x_values.push(val);
            }

            // Validate X axis: should be air mass values (2000-15000 range, increasing)
            // with last value >= 5000 (distinguishes from pressure-based maps)
            // Also require reasonable span (at least 2000 for air mass axis)
            let x_span = x_values.last().unwrap_or(&0).saturating_sub(*x_values.first().unwrap_or(&0));
            let x_valid = x_values.iter().all(|&v| v >= 2000 && v <= 15000)
                && x_values.windows(2).all(|w| w[1] >= w[0])
                && x_values.last().copied().unwrap_or(0) >= 5000
                && x_span >= 2000;

            if !x_valid {
                continue;
            }
            
            // Validate data values: Lambda calculation values are typically 800-1500 (0.8-1.5 lambda)
            // or IQ-like values 500-2000
            let mut valid_count = 0;
            for i in 0..(rows * cols) {
                let val = ((data[data_addr + i * 2] as u16) << 8) | (data[data_addr + i * 2 + 1] as u16);
                if val >= 500 && val <= 5000 {
                    valid_count += 1;
                }
            }
            
            if valid_count < (rows * cols * 80 / 100) {
                continue;
            }
            
            log::debug!("🎯 [EDC16] Found small Smoke Limiter by Lambda at 0x{:X} ({}x{}, Y: {}-{} RPM, X: {}-{} mg/stroke)",
                      data_addr, rows, cols, y_values[0], y_values[rows-1],
                      x_values[0], x_values[cols-1]);

            let mut map = DetectedMap::new(
                data_addr as u32,
                data_size,
                MapDimensions::TwoDimensional { rows, cols },
                DataType::Int16,
            );
            map.name = Some("Smoke Limiter by Lambda".to_string());
            map.category = Some("Smoke Limiter".to_string());
            map.unit = Some("-".to_string()); // Lambda is a ratio, no unit
            map.correction_factor = Some(0.001); // Raw 1250 → 1.25 lambda
            map.offset = Some(0.0);
            map.confidence = 0.85;

            map.y_axis_address = Some(y_axis_addr as u32);
            map.y_label = Some("rpm".to_string());
            map.y_axis_correction = Some(1.0);
            map.y_axis_offset = Some(0.0);

            map.x_axis_address = Some(x_axis_addr as u32);
            map.x_label = Some("mg/stroke".to_string());
            map.x_axis_correction = Some(0.1);
            map.x_axis_offset = Some(0.0);
            
            maps.push(map);
        }
        
        log::debug!("🔧 [EDC16] Small Lambda map detection: found {} maps", maps.len());
        maps
    }

    /// Detect Gearbox Torque Limiter maps (15x1 1D maps)
    /// These maps limit torque output per gear to protect the drivetrain
    ///
    /// Structure per map:
    /// - Header: [00][0F] (15 values, 1D map)
    /// - X-axis: 15 × 2 bytes (RPM values: 635, 680, 800, 1000... 4000)
    /// - Data: 15 × 2 bytes (torque values in Nm × 100, typically 30000 = limiter disabled)
    ///
    /// Total per map: 2 (header) + 30 (axis) + 30 (data) = 62 bytes
    /// Maps appear consecutively in groups of 6-7 (one per gear + neutral)
    ///
    /// Detection uses the RPM axis signature: 027b 02a8 0320 03e8 04e2 05dc 06d6 07d0 08ca 09c4 0abe 0bb8 0dac 0ea6 0fa0
    /// (635, 680, 800, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 3750, 4000 RPM)
    pub fn detect_gearbox_torque_limiter(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        // Signature: Header [00][0F] followed by RPM axis starting with 027b (635 RPM)
        // The first 4 bytes of the header+axis pattern: 00 0F 02 7b
        let header_signature: [u8; 4] = [0x00, 0x0F, 0x02, 0x7B];

        // Full RPM axis pattern (30 bytes) - all EDC16U1 gearbox limiters use this axis
        let rpm_axis_signature: [u8; 30] = [
            0x02, 0x7B, // 635 RPM
            0x02, 0xA8, // 680 RPM
            0x03, 0x20, // 800 RPM
            0x03, 0xE8, // 1000 RPM
            0x04, 0xE2, // 1250 RPM
            0x05, 0xDC, // 1500 RPM
            0x06, 0xD6, // 1750 RPM
            0x07, 0xD0, // 2000 RPM
            0x08, 0xCA, // 2250 RPM
            0x09, 0xC4, // 2500 RPM
            0x0A, 0xBE, // 2750 RPM
            0x0B, 0xB8, // 3000 RPM
            0x0D, 0xAC, // 3500 RPM
            0x0E, 0xA6, // 3750 RPM
            0x0F, 0xA0, // 4000 RPM
        ];

        let (scan_start, scan_end) = self.get_data_section_range(data.len());

        const HEADER_SIZE: usize = 2;  // [00][0F]
        const AXIS_SIZE: usize = 30;   // 15 values × 2 bytes
        const DATA_SIZE: usize = 30;   // 15 values × 2 bytes
        const MAP_TOTAL_SIZE: usize = HEADER_SIZE + AXIS_SIZE + DATA_SIZE; // 62 bytes

        let mut offset = scan_start;
        let mut found_count = 0;
        let mut single_value_detected = false;

        while offset + MAP_TOTAL_SIZE <= scan_end.min(data.len()) {
            // Check for header signature [00 0F 02 7b]
            if data[offset..offset + 4] == header_signature {
                // Verify full RPM axis matches
                let axis_start = offset + HEADER_SIZE;
                if axis_start + AXIS_SIZE <= data.len() &&
                   data[axis_start..axis_start + AXIS_SIZE] == rpm_axis_signature {

                    // ===== SINGLE VALUE GEARBOX LIMITER =====
                    // Detect single value (1x1) that appears 2 bytes BEFORE the first header
                    // This is a global gearbox torque limiter scalar
                    if !single_value_detected && offset >= 2 {
                        let single_val_addr = offset - 2;
                        let single_val = ((data[single_val_addr] as u16) << 8) | (data[single_val_addr + 1] as u16);

                        // Validate: should be in torque limiter range (typically 30000 = OFF)
                        if single_val > 0 && single_val <= 60000 && !detected.contains(&(single_val_addr as u32)) {
                            log::debug!("🎯 [EDC16] Found Single Value Gearbox Limiter at 0x{:X}, value: {}",
                                      single_val_addr, single_val);

                            let mut single_map = DetectedMap::new(
                                single_val_addr as u32,
                                2,
                                MapDimensions::OneDimensional { length: 1 },
                                DataType::Int16,
                            );
                            single_map.name = Some("Single Value Gearbox Limiter".to_string());
                            single_map.category = Some("Gearbox".to_string());
                            single_map.unit = Some("Nm".to_string());
                            single_map.correction_factor = Some(0.1); // 30000 → 3000.0
                            single_map.offset = Some(0.0);
                            single_map.confidence = 0.93;

                            maps.push(single_map);
                            single_value_detected = true;
                        }
                    }

                    // Data starts right after axis
                    let data_addr = axis_start + AXIS_SIZE;

                    // Skip if already detected
                    if detected.contains(&(data_addr as u32)) {
                        offset += 2;
                        continue;
                    }

                    // Validate data values (should be torque limiters: 0-60000 range)
                    // Typical values: 30000 (0x7530) when limiter is disabled/off
                    // Active limiters: usually in range 10000-40000
                    let mut valid_data = true;
                    let mut all_zero = true;
                    let mut sample_value = 0u16;

                    for i in 0..15 {
                        let val_offset = data_addr + i * 2;
                        if val_offset + 2 > data.len() {
                            valid_data = false;
                            break;
                        }
                        let val = ((data[val_offset] as u16) << 8) | (data[val_offset + 1] as u16);
                        if i == 0 {
                            sample_value = val;
                        }
                        if val != 0 {
                            all_zero = false;
                        }
                        // Torque limiter values should be in reasonable range
                        // 0-60000 (0-600 Nm when divided by 100)
                        if val > 60000 {
                            valid_data = false;
                            break;
                        }
                    }

                    // Skip empty maps (all zeros)
                    if all_zero {
                        offset += 2;
                        continue;
                    }

                    if valid_data {
                        found_count += 1;
                        log::debug!("🎯 [EDC16] Found Gearbox Torque Limiter #{} at data 0x{:X} (header at 0x{:X}), sample value: {}",
                                  found_count, data_addr, offset, sample_value);

                        let mut map = DetectedMap::new(
                            data_addr as u32,
                            DATA_SIZE,
                            MapDimensions::OneDimensional { length: 15 },
                            DataType::Int16,
                        );
                        map.name = Some(format!("Gearbox Torque Limiter {}", found_count));
                        map.category = Some("Gearbox".to_string());
                        map.unit = Some("Nm".to_string());
                        map.correction_factor = Some(0.1); // Raw value / 10 = Nm (30000 → 3000.0)
                        map.offset = Some(0.0);
                        map.confidence = 0.95;

                        // X axis is RPM
                        map.x_axis_address = Some(axis_start as u32);
                        map.x_label = Some("rpm".to_string());
                        map.x_axis_correction = Some(1.0);
                        map.x_axis_offset = Some(0.0);

                        maps.push(map);

                        // Jump to next potential map (they are consecutive)
                        offset = offset + MAP_TOTAL_SIZE;
                        continue;
                    }
                }
            }
            offset += 2;
        }

        // ===== Fallback générique (R5/V10 Transporter/Touareg) =====
        // L'axe RPM n'est PAS le 635..4000 figé : runs d'unités 1D
        // [00 N][axe N croissant][N valeurs] partageant le MÊME axe
        // (070906016A : 3+ runs N=12 axe 500..5300, stride 2+4N),
        // même généralisation que le Crafter côté U31.
        if maps.is_empty() {
            let read = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
            let unit_at = |o: usize| -> Option<(usize, Vec<u16>)> {
                if o + 4 > data.len() || data[o] != 0 {
                    return None;
                }
                let n = data[o + 1] as usize;
                if !(11..=15).contains(&n) || o + 2 + 4 * n > data.len() {
                    return None;
                }
                let axis: Vec<u16> = (0..n).map(|k| read(o + 2 + k * 2)).collect();
                if axis[0] < 300
                    || axis[0] > 1500
                    || !Self::axis_ascending_tolerant(&axis, 1)
                    || axis[n - 1] < 2500
                    || axis[n - 1] > 6000
                {
                    return None;
                }
                Some((n, axis))
            };

            let mut offset = scan_start;
            let mut found_count = 0;
            let mut single_value_detected = false;
            while offset + 60 <= scan_end.min(data.len()) {
                let Some((n, _axis)) = unit_at(offset) else {
                    offset += 2;
                    continue;
                };
                let unit_size = 2 + 4 * n;
                let mut run: Vec<usize> = vec![offset];
                let mut pos = offset + unit_size;
                'grow: loop {
                    for pad in 0..=16usize {
                        if let Some((n2, _a2)) = unit_at(pos + pad) {
                            // Même N suffit : l'axe peut varier d'un point
                            // entre rapports (Transporter R5 : 4550 vs 4800
                            // sur l'unité 3) — chaque unité a déjà passé la
                            // validation d'axe individuellement.
                            if n2 == n {
                                run.push(pos + pad);
                                pos = pos + pad + unit_size;
                                continue 'grow;
                            }
                        }
                    }
                    break;
                }
                if run.len() < 4 {
                    offset += 2;
                    continue;
                }
                // Au moins une unité doit contenir une vraie limite (≥ 900 Nm)
                let has_high = run
                    .iter()
                    .any(|&u| (0..n).any(|k| read(u + 2 + 2 * n + k * 2) >= 9000));
                if !has_high {
                    offset = pos;
                    continue;
                }

                // Single value = u16 juste avant le premier header
                if !single_value_detected && run[0] >= 2 {
                    let sv_addr = run[0] - 2;
                    let sv = read(sv_addr);
                    if sv > 0 && sv <= 60000 && !detected.contains(&(sv_addr as u32)) {
                        let mut single_map = DetectedMap::new(
                            sv_addr as u32,
                            2,
                            MapDimensions::OneDimensional { length: 1 },
                            DataType::Int16,
                        );
                        single_map.name = Some("Single Value Gearbox Limiter".to_string());
                        single_map.category = Some("Gearbox".to_string());
                        single_map.unit = Some("Nm".to_string());
                        single_map.correction_factor = Some(0.1);
                        single_map.offset = Some(0.0);
                        single_map.confidence = 0.9;
                        maps.push(single_map);
                        single_value_detected = true;
                    }
                }

                for &u in &run {
                    let data_addr = u + 2 + 2 * n;
                    if detected.contains(&(data_addr as u32)) {
                        continue;
                    }
                    let vals: Vec<u16> = (0..n).map(|k| read(data_addr + k * 2)).collect();
                    if vals.iter().all(|&v| v == 0) || vals.iter().any(|&v| v > 60000) {
                        continue;
                    }
                    found_count += 1;
                    let mut map = DetectedMap::new(
                        data_addr as u32,
                        n * 2,
                        MapDimensions::OneDimensional { length: n },
                        DataType::Int16,
                    );
                    map.name = Some(format!("Gearbox Torque Limiter {}", found_count));
                    map.category = Some("Gearbox".to_string());
                    map.unit = Some("Nm".to_string());
                    map.correction_factor = Some(0.1);
                    map.offset = Some(0.0);
                    map.confidence = 0.9;
                    map.x_axis_address = Some((u + 2) as u32);
                    map.x_label = Some("rpm".to_string());
                    map.x_axis_correction = Some(1.0);
                    map.x_axis_offset = Some(0.0);
                    maps.push(map);
                }
                log::debug!(
                    "🎯 [EDC16U1] Gearbox fallback: run of {} units (N={}) at 0x{:X}",
                    run.len(),
                    n,
                    run[0]
                );
                offset = pos;
            }
        }

        log::debug!("🔧 [EDC16] Gearbox Torque Limiter detection: found {} maps", maps.len());
        maps
    }

    /// Detect Fuel Correction maps
    /// These are 2D maps that apply correction factors based on various temperatures
    ///
    /// Types detected:
    /// - Fuel correction by fuel temperature (10x10, 3x3)
    /// - Fuel correction by oil temperature (5x5)
    /// - Fuel correction by ambient pressure (5x5)
    /// - Fuel correction by exhaust gas temp EGT (2x2, 8x8)
    /// - Fuel correction factor by intake air temperature (3x4, 5x5)
    /// - Fuel correction factor by engine temp and vehicle speed (4x4)
    ///
    /// Structure:
    /// - Header: [00][rows][00][cols] (4 bytes)
    /// - Y axis: rows × 2 bytes (temperature values, typically 0x0900-0x1100 range = ~230K-430K)
    /// - X axis: cols × 2 bytes (RPM values, typically 0x0000-0x1388 = 0-5000 RPM)
    /// - Data: rows × cols × 2 bytes (correction factors, typically around 0x2000 = 8192 = factor 1.0)
    ///
    /// Factor formula: raw_value × 0.000122070313 = correction factor (0x2000 = 1.0)
    pub fn detect_fuel_correction_maps(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        // Valid grid sizes for fuel correction maps
        let valid_grids: &[(usize, usize)] = &[
            (2, 2),   // EGT small
            (3, 3),   // Fuel temp small
            (3, 4),   // Intake air temp
            (4, 3),   // Intake air temp (inverted)
            (4, 4),   // Engine temp + vehicle speed
            (5, 5),   // Oil temp, ambient pressure
            (8, 8),   // EGT large
            (10, 10), // Fuel temp large
        ];

        let (scan_start, scan_end) = self.get_data_section_range(data.len());

        let mut offset = scan_start;

        while offset + 8 <= scan_end.min(data.len()) {
            // Check for header pattern [00][rows][00][cols]
            if data[offset] == 0x00 && data[offset + 2] == 0x00 {
                let rows = data[offset + 1] as usize;
                let cols = data[offset + 3] as usize;

                // Check if this is a valid grid size
                if !valid_grids.contains(&(rows, cols)) {
                    offset += 2;
                    continue;
                }

                let header_size = 4;
                let y_axis_size = rows * 2;
                let x_axis_size = cols * 2;
                let data_size = rows * cols * 2;
                let total_size = header_size + y_axis_size + x_axis_size + data_size;

                if offset + total_size > data.len() {
                    offset += 2;
                    continue;
                }

                let y_axis_start = offset + header_size;
                let x_axis_start = y_axis_start + y_axis_size;
                let data_start = x_axis_start + x_axis_size;

                // Skip if already detected
                if detected.contains(&(data_start as u32)) {
                    offset += 2;
                    continue;
                }

                // Read Y axis values (typically temperature)
                let mut y_axis_values = Vec::with_capacity(rows);
                for i in 0..rows {
                    let val = ((data[y_axis_start + i * 2] as u16) << 8) | (data[y_axis_start + i * 2 + 1] as u16);
                    y_axis_values.push(val);
                }

                // Read X axis values (typically RPM)
                let mut x_axis_values = Vec::with_capacity(cols);
                for i in 0..cols {
                    let val = ((data[x_axis_start + i * 2] as u16) << 8) | (data[x_axis_start + i * 2 + 1] as u16);
                    x_axis_values.push(val);
                }

                // Validate Y axis - should be increasing or temperature-like values
                // Temperature values typically in range 0x0800-0x1200 (Kelvin × 10)
                // Or atmospheric pressure 0x0064-0x05DC (100-1500 mbar)
                let y_valid = self.is_valid_fuel_correction_y_axis(&y_axis_values);
                if !y_valid {
                    offset += 2;
                    continue;
                }

                // Validate X axis - should be RPM (0-5500 range) and increasing
                let x_valid = self.is_valid_fuel_correction_x_axis(&x_axis_values);
                if !x_valid {
                    offset += 2;
                    continue;
                }

                // Validate data - correction factors should be around 0x2000 (8192)
                // Typical range: 0x1000-0x3000 (4096-12288) = factors 0.5-1.5
                let mut valid_data_count = 0;
                let mut sum_values: u64 = 0;
                let total_values = rows * cols;

                for i in 0..total_values {
                    let val_offset = data_start + i * 2;
                    let val = ((data[val_offset] as u16) << 8) | (data[val_offset + 1] as u16);
                    sum_values += val as u64;

                    // Valid correction factor range: 0x1000-0x4000 (0.5-2.0)
                    // Also allow 0 for disabled entries
                    if val == 0 || (val >= 0x1000 && val <= 0x4000) {
                        valid_data_count += 1;
                    }
                }

                // At least 70% of data should be in valid range
                if valid_data_count < (total_values * 70 / 100) {
                    offset += 2;
                    continue;
                }

                // Average value should be around 0x2000 (factor 1.0)
                let avg_value = sum_values / total_values as u64;
                if avg_value < 0x1800 || avg_value > 0x2800 {
                    // Not centered around 1.0, likely not a correction factor map
                    offset += 2;
                    continue;
                }

                // Determine map type based on Y axis characteristics
                let map_name = self.identify_fuel_correction_type(&y_axis_values, &x_axis_values, rows, cols);

                log::debug!("🎯 [EDC16] Found {} at 0x{:X} ({}x{}, Y: {}-{}, X: {}-{})",
                          map_name, data_start, rows, cols,
                          y_axis_values.first().unwrap_or(&0), y_axis_values.last().unwrap_or(&0),
                          x_axis_values.first().unwrap_or(&0), x_axis_values.last().unwrap_or(&0));

                let mut map = DetectedMap::new(
                    data_start as u32,
                    data_size,
                    MapDimensions::TwoDimensional { rows, cols },
                    DataType::UInt16,
                );
                map.name = Some(map_name.to_string());
                map.category = Some("Injection system".to_string());
                map.unit = Some("-".to_string());
                map.correction_factor = Some(0.000122070313); // 1/8192 = factor for raw -> decimal
                map.offset = Some(0.0);
                map.confidence = 0.88;

                // Y axis - configure based on map type
                map.y_axis_address = Some(y_axis_start as u32);
                if map_name.contains("ambient pressure") {
                    // Y axis is pressure in mbar (direct values)
                    map.y_label = Some("mbar".to_string());
                    map.y_axis_correction = Some(1.0);
                    map.y_axis_offset = Some(0.0);
                } else {
                    // Y axis is temperature in Kelvin × 10 -> convert to Celsius
                    map.y_label = Some("Temperature".to_string());
                    map.y_axis_correction = Some(0.1);
                    map.y_axis_offset = Some(-273.14);
                }

                // X axis (RPM)
                map.x_axis_address = Some(x_axis_start as u32);
                map.x_label = Some("rpm".to_string());
                map.x_axis_correction = Some(1.0);
                map.x_axis_offset = Some(0.0);

                maps.push(map);

                // Move past this map
                offset = offset + total_size;
                continue;
            }
            offset += 2;
        }

        log::debug!("🔧 [EDC16] Fuel Correction detection: found {} maps", maps.len());
        maps
    }

    /// Validate Y axis for fuel correction maps (temperature or pressure)
    fn is_valid_fuel_correction_y_axis(&self, values: &[u16]) -> bool {
        if values.is_empty() {
            return false;
        }

        // Check if values are mostly increasing (allow some tolerance)
        let mut increasing_count = 0;
        for i in 1..values.len() {
            if values[i] >= values[i - 1] {
                increasing_count += 1;
            }
        }
        // For small arrays (2-3 values), be more lenient
        let min_increasing = if values.len() <= 3 { 1 } else { (values.len() - 1) * 70 / 100 };
        if increasing_count < min_increasing {
            return false;
        }

        let first = values[0];
        let last = *values.last().unwrap_or(&0);

        // Temperature range (Kelvin × 10): 2000-5500 (200K-550K = -73°C to 277°C)
        // This covers fuel temp, oil temp, intake air temp
        let is_temperature = first >= 2000 && first <= 5000 && last >= 2500 && last <= 5500;

        // EGT temperature range (Kelvin × 10): 5000-12000 (500K-1200K = 227°C to 927°C)
        // Exhaust gas temperatures are much higher
        let is_egt_temperature = first >= 5000 && first <= 10000 && last >= 6000 && last <= 12000;

        // Atmospheric pressure range: 50-1500 mbar (borne haute 1600 : les
        // axes paddés en marches de 1 finissent à 1501..1505, ex. KN 10x10)
        let is_pressure = first >= 50 && first <= 800 && last >= 100 && last <= 1600;

        // Vehicle speed range: 0-300 km/h (for engine temp + speed maps)
        let is_speed = first <= 50 && last >= 50 && last <= 500;

        is_temperature || is_egt_temperature || is_pressure || is_speed
    }

    /// Validate X axis for fuel correction maps (typically RPM)
    fn is_valid_fuel_correction_x_axis(&self, values: &[u16]) -> bool {
        if values.is_empty() {
            return false;
        }

        // Check if values are increasing
        for i in 1..values.len() {
            if values[i] < values[i - 1] {
                return false;
            }
        }

        let first = values[0];
        let last = *values.last().unwrap_or(&0);

        // RPM range: 0-5500 (typical EDC16 RPM range)
        // First value can be 0 or higher (some maps start at 1000-3000 RPM)
        // Last value should be at least 1000 RPM (some maps only go to 1388 = 1388 RPM)
        first <= 3500 && last >= 1000 && last <= 5500
    }

    /// Identify fuel correction map type based on axis values
    fn identify_fuel_correction_type(&self, y_values: &[u16], _x_values: &[u16], rows: usize, cols: usize) -> &'static str {
        let y_first = *y_values.first().unwrap_or(&0);

        // Check if Y axis is high temperature (EGT range: > 5000 = 500K = 227°C)
        let is_egt_temp = y_first >= 5000;

        // 2x2 or 8x8 - EGT correction (check Y values to confirm)
        if (rows == 2 && cols == 2) || (rows == 8 && cols == 8) {
            if is_egt_temp {
                return "Fuel correction by exhaust gas temp EGT";
            }
        }

        // 10x10 : deux familles — fuel temperature (axe Y température K×10,
        // démarre >= 2000) ou la variante PADDÉE de l'ambient pressure
        // (axe 100..1500 + marches de 1, comme la 5x5 — cf. KN côté U31)
        if rows == 10 && cols == 10 {
            if y_first < 1000 {
                return "Fuel correction by ambient pressure";
            }
            return "Fuel correction by fuel temperature";
        }

        // 3x3 = Fuel correction by fuel temperature (small variant)
        if rows == 3 && cols == 3 {
            return "Fuel correction by fuel temperature";
        }

        // 5x5 - check Y axis to distinguish oil temp vs ambient pressure
        if rows == 5 && cols == 5 {
            // Ambient pressure typically starts lower (100-800 mbar)
            if y_first < 1000 {
                return "Fuel correction by ambient pressure";
            }
            // Temperature range - this is oil temperature (5x5 with temp axis)
            // Oil temp range: ~2400-4000 K×10 (-30°C to 127°C)
            return "Fuel correction by oil temperature";
        }

        // 3x4 or 4x3 = Intake air temperature
        if (rows == 3 && cols == 4) || (rows == 4 && cols == 3) {
            return "Fuel correction factor by intake air temperature";
        }

        // 4x4 = Engine temp and vehicle speed
        if rows == 4 && cols == 4 {
            return "Fuel correction factor by engine temp and vehicle speed";
        }

        // Default
        "Fuel correction"
    }

    /// Detects EGR maps and EGR Hysteresis maps
    ///
    /// ## EGR 2D Maps (16x13):
    /// - Header: [00 10 00 0D] = 16 rows, 13 columns
    /// - Y axis: 16 RPM values (32 bytes)
    /// - X axis: 13 IQ values (26 bytes)
    /// - Data: 16×13 = 208 values (416 bytes)
    /// - Data values: air mass per cylinder in mg/stroke × 10 (factor 0.1)
    /// - Typical range: 2000-8000 (200-800 mg/stroke)
    ///
    /// ## EGR Hysteresis 1D Maps (20x1):
    /// - Header: [00 00 00 14] or similar (0 rows, 20 columns)
    /// - X axis: 20 RPM values (40 bytes)
    /// - Data: 20 IQ values (40 bytes)
    /// - Data values: injection quantity in mg/stroke × 100 (factor 0.01)
    /// - Typical range: 5000-15000 (50-150 mg/stroke)
    pub fn detect_egr_maps(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut egr_2d_count = 0;
        let mut egr_hyst_count = 0;

        // ===== EGR 2D Maps Detection =====
        // Only detect 16x13 maps with header [00 10 00 0D]
        // These are the "AirCtl_mDesBase_MAP" EGR maps
        const EGR_ROWS: usize = 16;
        const EGR_COLS: usize = 13;

        for i in 0x10000..data.len().saturating_sub(500) {
            // Check for EGR header pattern [00 10 00 0D] (16 rows × 13 cols)
            if i + 3 >= data.len() {
                continue;
            }
            if data[i] != 0x00 || data[i + 1] != 0x10 || data[i + 2] != 0x00 || data[i + 3] != 0x0D {
                continue;
            }

            // Y axis (RPM) starts right after header
            let y_axis_start = i + 4;
            let y_axis_end = y_axis_start + EGR_ROWS * 2; // 16 values × 2 bytes = 32 bytes

            if y_axis_end >= data.len() {
                continue;
            }

            // Read Y axis (RPM values)
            let mut y_values = Vec::new();
            for j in (y_axis_start..y_axis_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    y_values.push(val);
                }
            }

            // Validate Y axis as RPM values for EGR (starts near 0, ends around 3000-4000)
            if !self.is_valid_egr_2d_rpm_axis_strict(&y_values) {
                continue;
            }

            // X axis (IQ) starts after Y axis
            let x_axis_start = y_axis_end;
            let x_axis_end = x_axis_start + EGR_COLS * 2; // 13 values × 2 bytes = 26 bytes

            if x_axis_end >= data.len() {
                continue;
            }

            // Read X axis (IQ values)
            let mut x_values = Vec::new();
            for j in (x_axis_start..x_axis_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    x_values.push(val);
                }
            }

            // Validate X axis as IQ values (starts near 0, ends around 3800)
            if !self.is_valid_egr_2d_iq_axis_strict(&x_values) {
                continue;
            }

            // Data starts after X axis
            let data_start = x_axis_end;
            let data_size = EGR_ROWS * EGR_COLS * 2; // 16 × 13 × 2 = 416 bytes
            let data_end = data_start + data_size;

            if data_end >= data.len() {
                continue;
            }

            // Check if already detected
            if detected.contains(&(data_start as u32)) {
                continue;
            }

            // Validate data values (air mass: typically 2000-10000 range = 200-1000 mg/stroke)
            let mut valid_data = true;
            let mut data_sum: u64 = 0;
            let mut data_count = 0;
            let mut high_values = 0;
            for j in (data_start..data_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    data_sum += val as u64;
                    data_count += 1;
                    // Count values in typical air mass range (200-1000 mg/stroke = 2000-10000 raw)
                    if val >= 2000 && val <= 10000 {
                        high_values += 1;
                    }
                    // Air mass should be reasonable (max ~2000 mg/stroke = 20000 raw)
                    if val > 25000 {
                        valid_data = false;
                        break;
                    }
                }
            }

            if !valid_data || data_count == 0 {
                continue;
            }

            // At least 50% of values should be in typical air mass range
            if high_values < data_count / 2 {
                continue;
            }

            let avg_data = data_sum / data_count as u64;
            // Average should be in reasonable range for air mass (3000-9000)
            if avg_data < 2500 || avg_data > 12000 {
                continue;
            }

            egr_2d_count += 1;
            let map_name = format!("EGR {}", egr_2d_count);

            // EGR maps in binary: first axis = RPM (16 values), second axis = IQ (13 values)
            // Display with IQ on Y axis (vertical) and RPM on X axis (horizontal)
            let mut map = DetectedMap::new(
                data_start as u32,
                data_size,
                MapDimensions::TwoDimensional { rows: EGR_COLS, cols: EGR_ROWS },
                DataType::UInt16,
            );
            map.name = Some(map_name);
            map.category = Some("EGR".to_string());
            map.unit = Some("mg/stroke".to_string());
            map.correction_factor = Some(0.1);
            map.offset = Some(0.0);
            map.confidence = 0.90;

            // Y axis = IQ (second axis in binary)
            map.y_axis_address = Some(x_axis_start as u32);
            map.y_label = Some("mg/stroke".to_string());
            map.y_axis_correction = Some(0.01);
            map.y_axis_offset = Some(0.0);

            // X axis = RPM (first axis in binary)
            map.x_axis_address = Some(y_axis_start as u32);
            map.x_label = Some("RPM".to_string());
            map.x_axis_correction = Some(1.0);
            map.x_axis_offset = Some(0.0);

            maps.push(map);
        }

        // ===== EGR Hysteresis 1D Maps Detection (20x1) =====
        // Structure based on StageX analysis:
        // [00 14] [X axis 40 bytes] [Data 40 bytes] [00 14] [X axis] [Data] ...
        //
        // StageX reports the address of the DATA (after the X axis), not the header itself.
        //
        // Known addresses from StageX:
        // - passat: 18B5FE, 18B650, 18B6A2, 18B6F4, 18B746, 1C9D82, 1C9DD4, 1C9E26, 1C9E78, 1C9ECA
        // - g5runtrophy: 1885A4, 1885F6, 188648, 18869A, 1886EC, 1C6EC0, 1C6F12, 1C6F64, 1C6FB6, 1C7008

        let mut egr_hyst_candidates: Vec<(usize, Vec<u16>)> = Vec::new(); // (data_address, x_values)

        // Search in the high calibration zone for [00 14] header pattern
        // (0x180000+ sur 2 Mo, 0x80000+ sur les U1 en 1 Mo)
        let hyst_start = if data.len() >= 0x200000 { 0x180000 } else { 0x80000 };
        for i in hyst_start..data.len().saturating_sub(100) {
            // Check for 1D map header [00 14] = 20 columns
            if i + 1 >= data.len() || data[i] != 0x00 || data[i + 1] != 0x14 {
                continue;
            }

            // X axis (RPM) starts after 2-byte header
            let x_axis_start = i + 2;
            let x_axis_end = x_axis_start + 40; // 20 values × 2 bytes

            // Data starts after X axis
            let data_start = x_axis_end;
            let data_end = data_start + 40;

            if data_end >= data.len() {
                continue;
            }

            // Read X axis (RPM values, AFTER the header)
            let mut x_values = Vec::new();
            for j in (x_axis_start..x_axis_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    x_values.push(val);
                }
            }

            // Check if X axis looks like RPM values
            let has_valid_rpm = self.is_valid_egr_hyst_rpm_axis_strict(&x_values);
            if !has_valid_rpm {
                continue;
            }

            // Check if already detected
            if detected.contains(&(data_start as u32)) {
                continue;
            }

            // Read data values (AFTER the X axis)
            let mut data_values = Vec::new();
            for j in (data_start..data_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    data_values.push(val);
                }
            }

            if data_values.len() != 20 {
                continue;
            }

            // Validate data values - EGR Hysteresis typically has values like:
            // - 10000 (100%)
            // - IQ-related values (100-25000 range)
            // - 0 (disabled EGR)
            // - 0xFFFF (no limit / inactive)
            let valid_data_count = data_values.iter().filter(|&&v| {
                v == 0 || v == 0xFFFF || (v >= 100 && v <= 25000)
            }).count();

            // At least 60% should be valid (more permissive for maps with mixed values)
            if valid_data_count < 12 {
                continue;
            }

            egr_hyst_candidates.push((data_start, x_values));
        }

        // Filter to keep only clustered candidates (maps should be in groups)
        let mut valid_candidates: Vec<(usize, Vec<u16>)> = Vec::new();

        for i in 0..egr_hyst_candidates.len() {
            let (addr, _) = &egr_hyst_candidates[i];

            // Count how many other candidates are within 500 bytes
            let nearby_count = egr_hyst_candidates.iter().filter(|(other_addr, _)| {
                let distance = if *other_addr > *addr {
                    other_addr - addr
                } else {
                    addr - other_addr
                };
                distance > 0 && distance < 500
            }).count();

            // Must have at least 2 neighbors (maps come in groups of 5)
            if nearby_count >= 2 {
                valid_candidates.push(egr_hyst_candidates[i].clone());
            }
        }

        // Create maps from valid candidates
        for (data_addr, _x_values) in valid_candidates.iter() {
            egr_hyst_count += 1;
            let map_name = format!("EGR hysteresis {}", egr_hyst_count);

            log::debug!("🎯 [EDC16] Found {} at 0x{:X} (20x1)", map_name, data_addr);

            let mut map = DetectedMap::new(
                *data_addr as u32,
                40, // Data size only
                MapDimensions::OneDimensional { length: 20 },
                DataType::Int16,
            );
            map.name = Some(map_name);
            map.category = Some("EGR".to_string());
            map.unit = Some("%".to_string());
            map.correction_factor = Some(0.01); // 10000 -> 100%
            map.offset = Some(0.0);
            map.confidence = 0.85;

            // X axis (RPM) - address is 42 bytes BEFORE data start (40 X axis + 2 header)
            map.x_axis_address = Some((*data_addr - 40) as u32);
            map.x_label = Some("RPM".to_string());
            map.x_axis_correction = Some(1.0);
            map.x_axis_offset = Some(0.0);

            maps.push(map);
        }

        // Legacy detection for other structure types (kept for backward compatibility)
        // This handles maps in lower address ranges (0x10000-0x180000)
        let mut legacy_candidates: Vec<(usize, usize, Vec<u16>, bool)> = Vec::new();

        for i in 0x10000..0x180000.min(data.len().saturating_sub(100)) {
            // Check for 1D map header [00 14] = 20 columns
            if i + 1 >= data.len() || data[i] != 0x00 || data[i + 1] != 0x14 {
                continue;
            }

            // X axis (RPM) starts after 2-byte header
            let x_axis_start = i + 2;
            let x_axis_end = x_axis_start + 40;

            if x_axis_end >= data.len() {
                continue;
            }

            let mut x_values = Vec::new();
            for j in (x_axis_start..x_axis_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    x_values.push(val);
                }
            }

            let has_valid_rpm = self.is_valid_egr_hyst_rpm_axis_strict(&x_values);

            // Data starts after X axis
            let data_start = x_axis_end;
            let data_end = data_start + 40;

            if data_end >= data.len() {
                continue;
            }

            if detected.contains(&(data_start as u32)) {
                continue;
            }

            let mut data_values = Vec::new();
            for j in (data_start..data_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    data_values.push(val);
                }
            }

            if data_values.len() != 20 {
                continue;
            }

            let non_ffff_values: Vec<u16> = data_values.iter()
                .filter(|&&v| v != 0xFFFF && v != 0)
                .copied()
                .collect();

            if non_ffff_values.len() < 5 {
                continue;
            }

            let in_iq_range = non_ffff_values.iter().filter(|&&v| {
                v >= 100 && v <= 25000
            }).count();

            if in_iq_range < (non_ffff_values.len() * 2) / 5 {
                continue;
            }

            let mut value_counts: std::collections::HashMap<u16, usize> = std::collections::HashMap::new();
            for &v in &non_ffff_values {
                *value_counts.entry(v).or_insert(0) += 1;
            }
            let max_repeat = *value_counts.values().max().unwrap_or(&0);

            if !has_valid_rpm && max_repeat < 2 {
                continue;
            }

            legacy_candidates.push((i, data_start, x_values, has_valid_rpm));
        }

        // Process legacy candidates with clustering
        let mut has_nearby_valid: Vec<bool> = vec![false; legacy_candidates.len()];

        for i in 0..legacy_candidates.len() {
            if legacy_candidates[i].3 {
                has_nearby_valid[i] = true;
                continue;
            }

            // Check if any candidate with valid RPM is exactly 82-90 bytes before this one
            for j in 0..legacy_candidates.len() {
                if i != j && legacy_candidates[j].3 {
                    let dist = if legacy_candidates[i].0 > legacy_candidates[j].0 {
                        legacy_candidates[i].0 - legacy_candidates[j].0
                    } else {
                        legacy_candidates[j].0 - legacy_candidates[i].0
                    };
                    // Maps are typically 82 bytes apart, allow some tolerance
                    if dist >= 78 && dist <= 90 {
                        has_nearby_valid[i] = true;
                        break;
                    }
                }
            }
        }

        // Third pass: Filter to only keep clustered candidates (either valid RPM or near valid RPM)
        // EGR Hysteresis maps should have at least 3 neighbors within 500 bytes
        // This prevents false positive pairs from passing
        let mut is_clustered: Vec<bool> = vec![false; legacy_candidates.len()];
        const CLUSTER_DISTANCE: usize = 500;
        const MIN_NEIGHBORS: usize = 3; // Require at least 3 neighbors

        for i in 0..legacy_candidates.len() {
            let mut neighbor_count = 0;
            for j in 0..legacy_candidates.len() {
                if i != j {
                    let dist = if legacy_candidates[i].1 > legacy_candidates[j].1 {
                        legacy_candidates[i].1 - legacy_candidates[j].1
                    } else {
                        legacy_candidates[j].1 - legacy_candidates[i].1
                    };
                    if dist <= CLUSTER_DISTANCE {
                        neighbor_count += 1;
                    }
                }
            }
            // Must have at least MIN_NEIGHBORS neighbors to be considered clustered
            if neighbor_count >= MIN_NEIGHBORS {
                is_clustered[i] = true;
            }
        }

        // Create maps for clustered legacy candidates that are either valid or near valid
        for (idx, (header_pos, data_start, _x_values, has_valid_rpm)) in legacy_candidates.iter().enumerate() {
            // Accept if: (clustered AND (valid RPM OR near valid RPM)) OR (isolated but has valid RPM)
            let is_valid = if is_clustered[idx] {
                // For clustered maps, accept if valid RPM or near valid RPM
                has_nearby_valid[idx]
            } else {
                // For isolated maps, only accept if they have a valid RPM axis
                *has_valid_rpm
            };

            if !is_valid {
                continue;
            }

            let x_axis_start = header_pos + 2;
            let data_size = 40;

            egr_hyst_count += 1;
            let map_name = format!("EGR hysteresis {}", egr_hyst_count);

            let mut map = DetectedMap::new(
                *data_start as u32,
                data_size,
                MapDimensions::OneDimensional { length: 20 },
                DataType::Int16,
            );
            map.name = Some(map_name);
            map.category = Some("EGR".to_string());
            map.unit = Some("mg/stroke".to_string());
            map.correction_factor = Some(0.01);
            map.offset = Some(0.0);
            map.confidence = 0.85;

            // X axis (RPM)
            map.x_axis_address = Some(x_axis_start as u32);
            map.x_label = Some("RPM".to_string());
            map.x_axis_correction = Some(1.0);
            map.x_axis_offset = Some(0.0);

            maps.push(map);
        }

        // ===== Fallback EGR 2D ancré sur le bloc hysteresis =====
        // Le slot EGR 2D est à ~0x1078 avant la première hysteresis sur
        // toutes les familles observées (016J, 038A, Transporter R5 :
        // header à h0-0x1078), mais le profil d'axe varie : 038A a un RPM
        // 0..5355 (la borne stricte 2500-4500 le rejette), le R5 un 16x12
        // dont le RPM démarre à 700 (pas de point 0). Si la passe stricte
        // n'a rien trouvé, on scanne la fenêtre [h0-0x1800, h0-0x800] avec
        // des bornes larges — l'ancrage positionnel remplace la rigueur
        // d'axe pour éviter les N75/smoke.
        if egr_2d_count == 0 {
            // Ancre = première hysteresis en zone CALIBRATION (les copies
            // de flash programme à 0x88-89xxx sur Transporter K/Touareg BL
            // faussaient le min)
            let h0 = valid_candidates
                .iter()
                .map(|(a, _)| *a)
                .filter(|&a| a >= 0xC0000)
                .min();
            if let Some(h0) = h0 {
                if h0 >= 0xC0000 + 0x1800 {
                    let rd = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
                    let win_lo = h0 - 0x1800;
                    let win_hi = h0 - 0x800;
                    let mut i = win_lo;
                    while i + 4 < win_hi.min(data.len()) {
                        if data[i] != 0 || data[i + 1] != 0x10 || data[i + 2] != 0 {
                            i += 2;
                            continue;
                        }
                        let cols = data[i + 3] as usize;
                        if !(11..=16).contains(&cols) {
                            i += 2;
                            continue;
                        }
                        let y0 = i + 4;
                        let x0 = y0 + 32;
                        let d0 = x0 + cols * 2;
                        let dsz = 16 * cols * 2;
                        if d0 + dsz > data.len() {
                            i += 2;
                            continue;
                        }
                        let rpm: Vec<u16> = (0..16).map(|k| rd(y0 + k * 2)).collect();
                        // rpm[0] ≤ 950 : le Caddy HN démarre à 900
                        if rpm[0] > 950
                            || rpm.windows(2).any(|w| w[0] >= w[1])
                            || !(3000..=6000).contains(&rpm[15])
                        {
                            i += 2;
                            continue;
                        }
                        let iq: Vec<u16> = (0..cols).map(|k| rd(x0 + k * 2)).collect();
                        if iq[0] > 600
                            || iq.windows(2).any(|w| w[0] >= w[1])
                            || !(3000..=6000).contains(&iq[cols - 1])
                        {
                            i += 2;
                            continue;
                        }
                        if detected.contains(&(d0 as u32)) {
                            i += 2;
                            continue;
                        }
                        // Tolère quelques cellules sentinelles hors plage
                        // (V10 : cellules à -10000 signé « EGR off ») —
                        // moyenne calculée sur les cellules en plage.
                        let vals: Vec<u16> = (0..16 * cols).map(|k| rd(d0 + k * 2)).collect();
                        let in_range: Vec<u64> = vals
                            .iter()
                            .filter(|&&v| v <= 12000)
                            .map(|&v| v as u64)
                            .collect();
                        let outliers = vals.len() - in_range.len();
                        if outliers > vals.len() / 10 || in_range.is_empty() {
                            i += 2;
                            continue;
                        }
                        let avg = in_range.iter().sum::<u64>() / in_range.len() as u64;
                        if !(2000..=12000).contains(&avg) {
                            i += 2;
                            continue;
                        }

                        egr_2d_count += 1;
                        let mut map = DetectedMap::new(
                            d0 as u32,
                            dsz,
                            MapDimensions::TwoDimensional { rows: cols, cols: 16 },
                            DataType::UInt16,
                        );
                        map.name = Some(format!("EGR {}", egr_2d_count));
                        map.category = Some("EGR".to_string());
                        map.unit = Some("mg/stroke".to_string());
                        map.correction_factor = Some(0.1);
                        map.offset = Some(0.0);
                        map.confidence = 0.85;
                        map.y_axis_address = Some(x0 as u32);
                        map.y_label = Some("mg/stroke".to_string());
                        map.y_axis_correction = Some(0.01);
                        map.y_axis_offset = Some(0.0);
                        map.x_axis_address = Some(y0 as u32);
                        map.x_label = Some("RPM".to_string());
                        map.x_axis_correction = Some(1.0);
                        map.x_axis_offset = Some(0.0);
                        log::debug!(
                            "🎯 [EDC16U1] EGR fallback (ancre hysteresis) : 16x{} à 0x{:X}",
                            cols,
                            d0
                        );
                        maps.push(map);
                        i = d0 + dsz;
                    }
                }
            }
        }

        log::debug!("🔧 [EDC16] EGR detection: {} 2D maps, {} Hysteresis maps", egr_2d_count, egr_hyst_count);
        maps
    }

    /// Validates RPM axis for EGR 2D maps (strict - 16 values only)
    /// Based on JSON reference: first value ~0, last value ~3423
    fn is_valid_egr_2d_rpm_axis_strict(&self, values: &[u16]) -> bool {
        if values.len() != 16 {
            return false;
        }

        // Profil classique uniquement (0, ~700, …, 2500-4500). NE PAS
        // élargir à rpm0>100 : les N75 (rpm 760..4250) et les smoke lambda
        // partagent le format 16x13 et se feraient classer EGR — le manque
        // apparent d'EGR sur U1 était un problème de NOM (« EGR N »),
        // corrigé par la normalisation en fin de detect().
        if values[0] > 100 {
            return false;
        }
        if values[1] < 500 || values[1] > 1000 {
            return false;
        }

        if values[15] < 2500 || values[15] > 4500 {
            return false;
        }

        // All values should be valid RPM (0-5000)
        for &v in values {
            if v > 5000 {
                return false;
            }
        }

        // Should be strictly increasing
        let mut increasing_count = 0;
        for i in 1..values.len() {
            if values[i] >= values[i - 1] {
                increasing_count += 1;
            }
        }

        // All values should be non-decreasing for EGR RPM axis
        increasing_count >= 14
    }

    /// Validates IQ axis for EGR 2D maps (strict - 13 values only)
    /// Based on JSON reference: first value ~0, last value ~3800
    fn is_valid_egr_2d_iq_axis_strict(&self, values: &[u16]) -> bool {
        if values.len() != 13 {
            return false;
        }

        // First value should be 0 or very small
        if values[0] > 100 {
            return false;
        }

        // Second value should be around 300-400 (3-4 mg/stroke)
        if values[1] < 200 || values[1] > 600 {
            return false;
        }

        // Last value should be around 3500-4000 (35-40 mg/stroke)
        if values[12] < 3000 || values[12] > 4500 {
            return false;
        }

        // All values should be in IQ range (0-5000 = 0-50 mg/stroke)
        for &v in values {
            if v > 6000 {
                return false;
            }
        }

        // Should be strictly increasing
        let mut increasing_count = 0;
        for i in 1..values.len() {
            if values[i] >= values[i - 1] {
                increasing_count += 1;
            }
        }

        // All values should be non-decreasing for EGR IQ axis
        increasing_count >= 11
    }

    /// Validates RPM axis for EGR Hysteresis 1D maps (strict)
    /// Based on JSON reference: starts low (0-500), increases to high RPM
    /// Some Hysteresis maps have unusual patterns (Hysteresis 5)
    fn is_valid_egr_hyst_rpm_axis_strict(&self, values: &[u16]) -> bool {
        if values.len() != 20 {
            return false;
        }

        // First value should be in low-mid RPM range (0-1000)
        // Some EGR Hysteresis X axes start at 0, others at 800 RPM
        if values[0] > 1000 {
            return false;
        }

        // Look at the range of values
        let min_val = *values.iter().min().unwrap_or(&0);
        let max_val = *values.iter().max().unwrap_or(&0);

        // Should have reasonable range (at least 1500 RPM spread)
        // Some EGR Hysteresis maps have smaller ranges (like 0-2700 RPM)
        if max_val < min_val + 1500 {
            return false;
        }

        // Max should be in mid-high RPM range (2500-5500)
        // Relaxed from 3500 to catch maps like 0x1C33A0 with max ~2700
        if max_val < 2500 || max_val > 5500 {
            return false;
        }

        // All values should be in valid range (0-5500)
        for &v in values {
            if v > 5500 {
                return false;
            }
        }

        // Check for large drops anywhere in the axis
        // A valid RPM axis should be monotonically increasing
        for i in 1..values.len() {
            if values[i - 1] > values[i] + 300 {
                // Large drop (>300 RPM) - this is not a valid RPM axis
                return false;
            }
        }

        // Should be STRICTLY increasing (no repeated values) in the first 10 values
        let mut strictly_increasing = 0;
        for i in 1..10.min(values.len()) {
            if values[i] > values[i - 1] {
                strictly_increasing += 1;
            }
        }

        // At least 7 of the first 9 transitions should be strictly increasing
        if strictly_increasing < 7 {
            return false;
        }

        // Should be generally increasing overall (at least 80%)
        let mut increasing_count = 0;
        for i in 1..values.len() {
            if values[i] >= values[i - 1] {
                increasing_count += 1;
            }
        }

        // At least 80% should be non-decreasing
        increasing_count >= 15
    }

    // ========== IDLE SPEED MAPS DETECTION ==========
    /// Detects Idle Speed maps:
    /// - Single Value Idle Speed (1x1): A single RPM value (600-900 rpm typically)
    /// - Idle Speed Correction by Engine Temp (2x4 or 4x4): RPM correction values indexed by temperature
    ///
    /// Structure for Single Value Idle Speed:
    /// - 1x1 map (2 bytes)
    /// - Value: typical idle RPM (600-900, raw value ~0x0258-0x0384)
    ///
    /// Structure for Idle Speed Correction by Engine Temp:
    /// - Header: [00 02 00 04] for 2x4 or [00 04 00 04] for 4x4
    /// - Signature before header: [09 E3] (temp axis marker)
    /// - X axis: 4 temperature values (engine temp in Kelvin*10, e.g., 2531 = -20°C, 3181 = 60°C)
    /// - Y axis: 2 or 4 values (correction factors or RPM offsets)
    /// - Data: 2×4 or 4×4 RPM values (800-1500 rpm range)
    pub fn detect_idle_speed_maps(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut single_value_count = 0;
        let mut correction_count = 0;

        // ===== Idle Speed Correction by Engine Temp Detection =====
        // Structure: [Header 4 bytes][X axis temp][Y axis RPM][Data]
        // Header: [00 02][00 04] for 2x4 or [00 04][00 04] for 4x4
        // X axis starts with [09 E3] (temperature marker ~2531 = -20°C in Kelvin*10)

        for i in 0x10000..data.len().saturating_sub(100) {
            // Header [00 r][00 c] avec r = 2..6 et c = 3..4 (généralisation
            // portée de l'U31 : Touareg V10 5x4 à 0xF715C, BL 5x4 ; le SW
            // 016FF/BQ 3713xx a des 2x3 [0002][0003])
            let header_ok = i + 3 < data.len()
                && data[i] == 0x00
                && (2..=6).contains(&data[i + 1])
                && data[i + 2] == 0x00
                && (3..=4).contains(&data[i + 3]);

            if !header_ok {
                continue;
            }

            let (rows, cols) = (data[i + 1] as usize, data[i + 3] as usize);

            // Structure for 2D maps in EDC16:
            // Header: [00 rows][00 cols]
            // First axis (X): `rows` values (temperature)
            // Second axis (Y): `cols` values (RPM correction)
            // Data: rows × cols values

            // X axis (temperature) starts after header - has `rows` values
            let x_axis_start = i + 4;
            let x_axis_size = rows * 2; // rows values × 2 bytes
            let x_axis_end = x_axis_start + x_axis_size;

            // Check signature: first temperature value should be around 0x09E3 (-20°C)
            // This validates that this is a temperature-based map
            if x_axis_start + 1 >= data.len() {
                continue;
            }
            let first_temp = u16::from_be_bytes([data[x_axis_start], data[x_axis_start + 1]]);
            // First temp should be cold: -40°C to 20°C = 2331 to 2931 in Kelvin*10
            if first_temp < 2300 || first_temp > 2950 {
                continue;
            }

            if x_axis_end >= data.len() {
                continue;
            }

            // Read X axis (temperature values in Kelvin*10)
            let mut x_values = Vec::new();
            for j in (x_axis_start..x_axis_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    x_values.push(val);
                }
            }

            // Validate X axis as temperature values (only if 4 values, for 2-value axis skip strict validation)
            if x_values.len() >= 4 && !self.is_valid_idle_temp_axis(&x_values) {
                continue;
            }
            // For 2-value axis, just check both are in temperature range and increasing
            if x_values.len() == 2 {
                if x_values[0] < 2300 || x_values[0] > 2950 || x_values[1] < 2800 || x_values[1] > 3400 {
                    continue;
                }
                if x_values[1] <= x_values[0] {
                    continue;
                }
            }

            // Y axis starts after X axis - has `cols` values
            let y_axis_start = x_axis_end;
            let y_axis_size = cols * 2; // cols values × 2 bytes
            let y_axis_end = y_axis_start + y_axis_size;

            if y_axis_end >= data.len() {
                continue;
            }

            // Read Y axis (RPM correction values)
            let mut y_values = Vec::new();
            for j in (y_axis_start..y_axis_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    y_values.push(val);
                }
            }

            // Validate Y axis (should be reasonable RPM-like values: 600-1500)
            if !self.is_valid_idle_rpm_correction_axis(&y_values) {
                continue;
            }

            // Data starts after Y axis
            let data_start = y_axis_end;
            let data_size = rows * cols * 2;
            let data_end = data_start + data_size;

            if data_end >= data.len() {
                continue;
            }

            // Check if already detected
            if detected.contains(&(data_start as u32)) {
                continue;
            }

            // Validate data values (RPM: typically 800-1500)
            let mut valid_data = true;
            let mut rpm_count = 0;
            let mut data_values = Vec::new();
            for j in (data_start..data_end).step_by(2) {
                if j + 1 < data.len() {
                    let val = u16::from_be_bytes([data[j], data[j + 1]]);
                    data_values.push(val);
                    // Idle RPM values typically 750-1500 (610 à chaud sur
                    // le Touareg V10)
                    if val >= 550 && val <= 1600 {
                        rpm_count += 1;
                    }
                    // Reject if values are way outside RPM range
                    if val > 3000 || val < 300 {
                        valid_data = false;
                        break;
                    }
                }
            }

            if !valid_data {
                continue;
            }

            // At least 70% should be in typical idle RPM range
            let total_values = rows * cols;
            if rpm_count < (total_values * 7 / 10) {
                continue;
            }

            correction_count += 1;
            let map_name = format!("Idle speed correction by engine temp {}", correction_count);

            // Header format: [00 rows][00 cols], StageX displays as rows×cols
            log::debug!("🎯 [EDC16] Found {} at 0x{:X} ({}x{})", map_name, data_start, rows, cols);

            let mut map = DetectedMap::new(
                data_start as u32,
                data_size,
                MapDimensions::TwoDimensional { rows, cols },
                DataType::UInt16,
            );
            map.name = Some(map_name);
            map.category = Some("Idle speed RPM".to_string());
            map.unit = Some("rpm".to_string());
            map.correction_factor = Some(1.0);
            map.offset = Some(0.0);
            map.confidence = 0.88;

            // X axis (temperature)
            map.x_axis_address = Some(x_axis_start as u32);
            map.x_label = Some("Engine temp".to_string());
            map.x_axis_correction = Some(0.1);  // Kelvin*10 to Kelvin
            map.x_axis_offset = Some(-273.15); // Kelvin to Celsius

            // Y axis
            map.y_axis_address = Some(y_axis_start as u32);
            map.y_label = Some("rpm".to_string());
            map.y_axis_correction = Some(1.0);
            map.y_axis_offset = Some(0.0);

            maps.push(map);
        }

        // ===== Single Value Idle Speed Detection =====
        // Look for single RPM values (1x1) that represent base idle speed
        // Based on StageX analysis, these follow a specific pattern:
        // - Pattern: [03E8 0011] then [RPM value] [012C]
        // - The pattern [03E8 0011] followed by idle RPM (600-900) then 012C (300)
        // - Found at addresses like 1908DC, 1F0704, etc.
        //
        // Also collect axis ranges from correction maps to exclude them

        // Collect all axis address ranges to exclude
        let mut axis_ranges: Vec<(usize, usize)> = Vec::new();
        for m in &maps {
            // X axis range - has `rows` values (temperature axis)
            if let Some(x_addr) = m.x_axis_address {
                if let MapDimensions::TwoDimensional { rows, cols: _ } = m.dimensions {
                    let x_size = rows * 2; // rows values × 2 bytes
                    axis_ranges.push((x_addr as usize, x_addr as usize + x_size));
                }
            }
            // Y axis range - has `cols` values (RPM correction axis)
            if let Some(y_addr) = m.y_axis_address {
                if let MapDimensions::TwoDimensional { rows: _, cols } = m.dimensions {
                    let y_size = cols * 2; // cols values × 2 bytes
                    axis_ranges.push((y_addr as usize, y_addr as usize + y_size));
                }
            }
        }

        // Search for single value idle speed patterns
        // Two patterns observed:
        // 1. [03E8 0011] [RPM 600-900] [012C] - after correction map data ends
        // 2. [XXXX 0000] [RPM 600-900] [012C] - isolated parameter block
        //
        // Common signature: [RPM value 600-850] followed by [012C] (300)
        // and preceded by [0000] or [0011]

        let mut detected_single_addrs: HashSet<u32> = HashSet::new();

        for i in 0x10000..data.len().saturating_sub(8) {
            // Check if this could be an idle RPM value
            let rpm_val = u16::from_be_bytes([data[i], data[i + 1]]);

            // Check if this is a valid idle RPM (typically 650-850)
            if rpm_val < 620 || rpm_val > 850 {
                continue;
            }

            // Next value: [012C] (300) pour les motifs historiques ; le
            // Transporter R5 a 375 → motif 6 dédié avec plage 250-500.
            if i + 3 >= data.len() {
                continue;
            }
            let next = u16::from_be_bytes([data[i + 2], data[i + 3]]);
            let next_strict = next == 0x012C;
            let next_relaxed = (250..=700).contains(&next);
            if !next_strict && !next_relaxed {
                continue;
            }

            // Check previous values for context:
            // Pattern 1: [03E8 0011] before RPM
            // Pattern 2: [XXXX 0000] before RPM
            if i < 4 {
                continue;
            }
            let prev1 = u16::from_be_bytes([data[i - 2], data[i - 1]]);
            let prev2 = u16::from_be_bytes([data[i - 4], data[i - 3]]);

            // Pattern 1: [03E8] [0011] then RPM
            let is_pattern1 = next_strict && prev2 == 0x03E8 && prev1 == 0x0011;

            // Pattern 2: Something then [0000] then RPM
            let is_pattern2 = next_strict && prev1 == 0x0000 && prev2 > 0x0100 && prev2 < 0x0400;

            // Pattern 3: [FFFF][0011] then RPM then [012C] (2.0L U1 0xEEDEC,
            // meme variante que U34) ; Pattern 5: [FFFF][0000] then RPM
            let is_pattern3 = next_strict && prev1 == 0x0011 && prev2 == 0xFFFF;
            let is_pattern5 = next_strict && prev1 == 0x0000 && prev2 == 0xFFFF;

            // Pattern 6 (Transporter R5/1.9) : [0011] juste avant suffit —
            // prev2 varie (queue idle corr 600-1200, 100, FFFF…) et next
            // va de 300 à 600 selon les familles
            let is_pattern6 = next_relaxed && prev1 == 0x0011;

            if !is_pattern1 && !is_pattern2 && !is_pattern3 && !is_pattern5 && !is_pattern6 {
                continue;
            }

            let rpm_addr = i;

            // Check if already detected
            if detected.contains(&(rpm_addr as u32)) || detected_single_addrs.contains(&(rpm_addr as u32)) {
                continue;
            }

            // Skip if this is part of an already detected map or axis
            let in_map_or_axis = maps.iter().any(|m| {
                let m_end = m.address + m.size as u32;
                (rpm_addr as u32) >= m.address && (rpm_addr as u32) < m_end
            }) || axis_ranges.iter().any(|(start, end)| {
                rpm_addr >= *start && rpm_addr < *end
            });

            if in_map_or_axis {
                continue;
            }

            detected_single_addrs.insert(rpm_addr as u32);
            single_value_count += 1;
            let map_name = format!("Single value idle speed {}", single_value_count);

            log::debug!("🎯 [EDC16] Found {} at 0x{:X} (value: {} rpm)", map_name, rpm_addr, rpm_val);

            let mut map = DetectedMap::new(
                rpm_addr as u32,
                2, // Single 16-bit value
                MapDimensions::OneDimensional { length: 1 },
                DataType::UInt16,
            );
            map.name = Some(map_name);
            map.category = Some("Idle speed RPM".to_string());
            map.unit = Some("rpm".to_string());
            map.correction_factor = Some(1.0);
            map.offset = Some(0.0);
            map.confidence = 0.85;

            maps.push(map);
        }

        log::debug!("🔧 [EDC16] Idle Speed detection: {} single value, {} correction maps",
                  single_value_count, correction_count);
        maps
    }

    /// Validates temperature axis for Idle Speed Correction maps
    /// Temperature values are in Kelvin × 10 (e.g., 2531 = 253.1K = -20°C)
    /// Typical range: 2531 (-20°C) to 3431 (70°C)
    fn is_valid_idle_temp_axis(&self, values: &[u16]) -> bool {
        if values.len() < 3 {
            return false;
        }

        // All values should be in temperature range (Kelvin × 10)
        // -40°C to 100°C = 2331 to 3731
        for &v in values {
            if v < 2300 || v > 3800 {
                return false;
            }
        }

        // Croissant avec tolérance d'encoches (axes tunés)
        if !Self::axis_ascending_tolerant(values, 1) {
            return false;
        }

        // First value should be cold temperature (-40 to 0°C = 2331 to 2731)
        if values[0] < 2300 || values[0] > 2900 {
            return false;
        }

        // Last value should be hot temperature — seuil 2800 comme sur U31
        // (le 4x4 du Touareg BL finit à 2931)
        if values.last().copied().unwrap_or(0) < 2800 || values.last().copied().unwrap_or(0) > 3800 {
            return false;
        }

        true
    }

    /// Validates RPM/correction axis for Idle Speed Correction maps
    /// Values should be in reasonable idle RPM range (600-1500)
    fn is_valid_idle_rpm_correction_axis(&self, values: &[u16]) -> bool {
        if values.len() < 2 {
            return false;
        }

        // All values should be in idle RPM range
        for &v in values {
            if v < 500 || v > 1800 {
                return false;
            }
        }

        // Should be generally increasing or stable
        let mut decreasing_count = 0;
        for i in 1..values.len() {
            if values[i] < values[i - 1].saturating_sub(100) {
                decreasing_count += 1;
            }
        }

        // Allow at most one significant decrease
        decreasing_count <= 1
    }

    // ========== PHASE 0.11: EGT (Exhaust Gas Temperature) Maps Detection ==========
    /// Detect EGT sensor linearisation maps and EGT base maps
    ///
    /// Types of EGT maps:
    /// 1. EGT Sensor Linearisation (2x1 or 40x1):
    ///    - Signature: [03FF] [0AAB 0AAB] followed by increasing values
    ///    - Or: [7FFF 0002 0000 03FF] [0AAB 0AAB] followed by values
    ///    - Used for sensor voltage to temperature conversion
    ///
    /// 2. EGT Base Map (11x14 or 12x13):
    ///    - Header: [00 rows][00 cols] or [01 00][00 rows][00 cols]
    ///    - Y axis: RPM values (1000-4500)
    ///    - X axis: IQ or other parameter
    ///    - Data: Temperature values
    pub fn detect_egt_maps(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut sensor_count = 0;
        let mut base_map_count = 0;

        // ===== EGT Sensor Linearisation with 0AAB 0AAB header =====
        // Signature: [03FF][0AAB 0AAB] followed by increasing values
        // Used in files like passat, lectureG5, g5runtrophy
        // (zone haute : 0x1D0000 sur 2 Mo, 0xD0000 sur les U1 1 Mo)
        let egt_lin_start = if data.len() >= 0x200000 { 0x1D0000 } else { 0xD0000 };

        for i in egt_lin_start..data.len().saturating_sub(100) {
            if i + 3 >= data.len() || i < 2 {
                continue;
            }

            let val1 = u16::from_be_bytes([data[i], data[i + 1]]);
            let val2 = u16::from_be_bytes([data[i + 2], data[i + 3]]);

            // Look for 0AAB 0AAB signature
            if val1 != 0x0AAB || val2 != 0x0AAB {
                continue;
            }

            // Must be preceded by 03FF
            let prev = u16::from_be_bytes([data[i - 2], data[i - 1]]);
            if prev != 0x03FF {
                continue;
            }

            let map_address = i;
            let data_start = i + 4;

            // Check if already detected
            if detected.contains(&(map_address as u32)) {
                continue;
            }

            // Check if this overlaps with existing map
            let already_in_map = maps.iter().any(|m: &DetectedMap| {
                let m_end = m.address + m.size as u32;
                (map_address as u32) >= m.address && (map_address as u32) < m_end
            });
            if already_in_map {
                continue;
            }

            // Read values until we hit a big jump, padding, or invalid data
            let mut values = Vec::new();
            for j in 0..50 {
                let addr = data_start + j * 2;
                if addr + 1 >= data.len() {
                    break;
                }
                let val = u16::from_be_bytes([data[addr], data[addr + 1]]);

                // Stop at padding or zeros
                if val == 0 || val == 0x0030 || val > 0x4000 {
                    break;
                }

                // Stop if big jump (>1024) - indicates next section
                if !values.is_empty() {
                    let last: u16 = *values.last().unwrap();
                    if val > last + 0x400 || val < last.saturating_sub(0x100) {
                        break;
                    }
                }

                values.push(val);
            }

            // Need reasonable number of values
            if values.len() >= 30 && values.len() <= 50 {
                let num_values = values.len();
                let data_size = 4 + num_values * 2;

                sensor_count += 1;
                let map_name = format!("Exhaust gas temperature sensor linearisation EGT {}", sensor_count);
                log::debug!("🎯 [EDC16] Found {} at 0x{:X} ({}x{})", map_name, map_address, num_values, 1);

                let mut map = DetectedMap::new(
                    map_address as u32,
                    data_size,
                    MapDimensions::TwoDimensional { rows: num_values, cols: 1 },
                    DataType::UInt16,
                );
                map.name = Some(map_name);
                map.category = Some("Exhaust gas temperature EGT".to_string());
                map.unit = Some("°C".to_string());
                map.correction_factor = Some(0.1);
                map.offset = Some(-273.15);
                map.confidence = 0.85;
                maps.push(map);
            }
        }

        // ===== EGT Sensor Linearisation without 0AAB header (40x1 high temperature) =====
        // These maps start with values around 0x06C3 (1731) and increase to ~0x398B (14731)
        // Used in files like VWGolf2l, AudiA3

        for i in egt_lin_start..data.len().saturating_sub(100) {
            if i + 81 >= data.len() {
                continue;
            }

            let val1 = u16::from_be_bytes([data[i], data[i + 1]]);
            let val2 = u16::from_be_bytes([data[i + 2], data[i + 3]]);

            // First value should be around 1731 (0x06C3) ± tolerance —
            // le Touareg V10 démarre à 2231 (0x8B7, −50°C en K×10)
            if val1 < 0x0600 || val1 > 0x0900 {
                continue;
            }

            // Second value should be increasing
            if val2 <= val1 || val2 > val1 + 0x200 {
                continue;
            }

            // Check if already detected
            if detected.contains(&(i as u32)) {
                continue;
            }

            // Check if this overlaps with existing map
            let already_in_map = maps.iter().any(|m: &DetectedMap| {
                let m_end = m.address + m.size as u32;
                (i as u32) >= m.address && (i as u32) < m_end
            });
            if already_in_map {
                continue;
            }

            // Verify it's a valid increasing sequence of ~40 values ending in
            // padding (0x0030) ou en sentinelle FFFF/0000 (Touareg V10)
            let mut values = Vec::new();
            let mut found_padding = false;

            for j in 0..50 {
                let addr = i + j * 2;
                if addr + 1 >= data.len() {
                    break;
                }
                let val = u16::from_be_bytes([data[addr], data[addr + 1]]);

                // Check for padding (0x0030) or sentinel terminator
                if val == 0x0030 || val == 0xFFFF || val == 0x0000 {
                    found_padding = true;
                    break;
                }

                // Stop if value drops or goes too high
                if !values.is_empty() {
                    let last: u16 = *values.last().unwrap();
                    if val < last.saturating_sub(0x100) || val > 0x4000 {
                        break;
                    }
                }

                values.push(val);
            }

            // Valid if we have ~40 values and found padding
            if values.len() >= 35 && values.len() <= 45 && found_padding {
                let num_values = values.len();
                let data_size = num_values * 2;

                sensor_count += 1;
                let map_name = format!("Exhaust gas temperature sensor linearisation EGT {}", sensor_count);
                log::debug!("🎯 [EDC16] Found {} at 0x{:X} ({}x{})", map_name, i, num_values, 1);

                let mut map = DetectedMap::new(
                    i as u32,
                    data_size,
                    MapDimensions::TwoDimensional { rows: num_values, cols: 1 },
                    DataType::UInt16,
                );
                map.name = Some(map_name);
                map.category = Some("Exhaust gas temperature EGT".to_string());
                map.unit = Some("°C".to_string());
                map.correction_factor = Some(0.1);
                map.offset = Some(-273.15);
                map.confidence = 0.80;
                maps.push(map);
            }
        }

        // ===== EGT Base Map Detection (11x14, 12x13) =====
        // These are larger 2D maps with RPM on one axis
        // Header patterns: [00 0D 00 0C] (13x12), [00 0E 00 0B] (14x11)
        // (zone haute : 0x1E0000 sur 2 Mo, 0xE0000 sur les U1 1 Mo)
        let egt_base_start = if data.len() >= 0x200000 { 0x1E0000 } else { 0xE0000 };

        for i in egt_base_start..data.len().saturating_sub(400) {
            // Check for header patterns
            if i + 3 >= data.len() {
                continue;
            }

            let byte0 = data[i];
            let byte1 = data[i + 1];
            let byte2 = data[i + 2];
            let byte3 = data[i + 3];

            // Header format: [00 rows][00 cols]
            if byte0 != 0x00 || byte2 != 0x00 {
                continue;
            }

            let rows = byte1 as usize;
            let cols = byte3 as usize;

            // Valid EGT base map dimensions
            let valid_dims = (rows == 13 && cols == 12) ||  // 13x12
                           (rows == 14 && cols == 11) ||  // 14x11
                           (rows == 12 && cols == 13) ||  // 12x13
                           (rows == 11 && cols == 14);    // 11x14

            if !valid_dims {
                continue;
            }

            // Y axis (RPM) starts after header
            let y_axis_start = i + 4;
            let y_axis_size = rows * 2;
            let y_axis_end = y_axis_start + y_axis_size;

            if y_axis_end + 2 >= data.len() {
                continue;
            }

            // Read Y axis values (should be RPM: 1000-5000)
            let mut y_values = Vec::new();
            for j in (y_axis_start..y_axis_end).step_by(2) {
                let val = u16::from_be_bytes([data[j], data[j + 1]]);
                y_values.push(val);
            }

            // Validate Y axis as RPM values
            if !self.is_valid_egt_rpm_axis(&y_values) {
                continue;
            }

            // Check for separator (often 0000)
            let separator = u16::from_be_bytes([data[y_axis_end], data[y_axis_end + 1]]);

            // X axis starts after Y axis (possibly after separator)
            let x_axis_start = if separator == 0x0000 {
                y_axis_end + 2
            } else {
                y_axis_end
            };

            let x_axis_size = cols * 2;
            let x_axis_end = x_axis_start + x_axis_size;

            if x_axis_end >= data.len() {
                continue;
            }

            // Read X axis values
            let mut x_values = Vec::new();
            for j in (x_axis_start..x_axis_end).step_by(2) {
                let val = u16::from_be_bytes([data[j], data[j + 1]]);
                x_values.push(val);
            }

            // X axis should be injection quantity or similar (generally increasing)
            if !self.is_valid_egt_x_axis(&x_values) {
                continue;
            }

            // Data starts after X axis
            let data_start = x_axis_end;
            let data_size = rows * cols * 2;

            if data_start + data_size > data.len() {
                continue;
            }

            // Check if already detected
            if detected.contains(&(data_start as u32)) {
                continue;
            }

            // Check if this overlaps with existing map
            let already_in_map = maps.iter().any(|m: &DetectedMap| {
                let m_end = m.address + m.size as u32;
                (data_start as u32) >= m.address && (data_start as u32) < m_end
            });
            if already_in_map {
                continue;
            }

            // Validate data values (should be temperature in Kelvin×10 range: 2000-9000)
            let mut valid_count = 0;
            for j in (data_start..data_start + data_size).step_by(2) {
                let val = u16::from_be_bytes([data[j], data[j + 1]]);
                // EGT temps: 200°C to 900°C = 4731 to 11731 in Kelvin×10
                // But raw values might be different scaling
                if val >= 1000 && val <= 15000 {
                    valid_count += 1;
                }
            }

            let total_values = rows * cols;
            if valid_count < (total_values * 60 / 100) {
                continue;
            }

            base_map_count += 1;
            let map_name = format!("Exhaust gas temperature base map EGT {}", base_map_count);

            log::debug!("🎯 [EDC16] Found {} at 0x{:X} ({}x{}, Y: {}-{} RPM)",
                      map_name, data_start, rows, cols, y_values[0], y_values[rows - 1]);

            let mut map = DetectedMap::new(
                data_start as u32,
                data_size,
                MapDimensions::TwoDimensional { rows, cols },
                DataType::UInt16,
            );
            map.name = Some(map_name);
            map.category = Some("Exhaust gas temperature EGT".to_string());
            map.unit = Some("°C".to_string());
            map.correction_factor = Some(0.1);
            map.offset = Some(-273.15);  // Kelvin to Celsius
            map.confidence = 0.85;

            // Axis info
            map.y_axis_address = Some(y_axis_start as u32);
            map.y_label = Some("rpm".to_string());
            map.y_axis_correction = Some(1.0);
            map.y_axis_offset = Some(0.0);

            map.x_axis_address = Some(x_axis_start as u32);
            map.x_label = Some("mg/stroke".to_string());
            map.x_axis_correction = Some(0.01);
            map.x_axis_offset = Some(0.0);

            maps.push(map);
        }

        log::debug!("🔧 [EDC16] EGT detection: {} sensor linearisation, {} base maps",
                  sensor_count, base_map_count);
        maps
    }

    /// Validates RPM axis for EGT base maps
    fn is_valid_egt_rpm_axis(&self, values: &[u16]) -> bool {
        if values.len() < 5 {
            return false;
        }

        // RPM values should be in range 500-5500
        for &v in values {
            if v < 500 || v > 5500 {
                return false;
            }
        }

        // Should be increasing
        for i in 1..values.len() {
            if values[i] <= values[i - 1] {
                return false;
            }
        }

        // First value should be low RPM (800-1500)
        if values[0] < 800 || values[0] > 1500 {
            return false;
        }

        true
    }

    /// Validates X axis for EGT base maps (injection quantity or similar)
    fn is_valid_egt_x_axis(&self, values: &[u16]) -> bool {
        if values.len() < 5 {
            return false;
        }

        // X values should generally increase
        let mut increasing_count = 0;
        for i in 1..values.len() {
            if values[i] >= values[i - 1] {
                increasing_count += 1;
            }
        }

        // At least 60% should be increasing
        increasing_count >= (values.len() - 1) * 60 / 100
    }
}

// ============== TESTS ==============
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_templates_configured() {
        assert!(MAP_TEMPLATES.len() >= 9, "Should have at least 9 map templates");

        // Check all templates have valid configurations
        for template in MAP_TEMPLATES {
            assert!(!template.grids.is_empty(), "Template {} should have at least one grid", template.name);
            assert!(template.max_count > 0, "Template {} should have max_count > 0", template.name);
            assert!(template.z_range_stock.0 < template.z_range_stock.1,
                    "Template {} stock range invalid", template.name);
        }
    }

    #[test]
    fn test_boost_limiter_grids() {
        let template = MAP_TEMPLATES.iter().find(|t| t.name == "Boost Limiter").unwrap();
        assert!(template.grids.contains(&(11, 10)));
        assert!(template.grids.contains(&(10, 10)));
        assert!(template.grids.contains(&(10, 11)));
    }

    #[test]
    fn test_tuned_mode_extends_ranges() {
        let template = MAP_TEMPLATES.iter().find(|t| t.name == "Boost Limiter").unwrap();
        assert!(template.z_range_tuned.1 > template.z_range_stock.1,
                "Tuned mode should have extended range");
    }
}
