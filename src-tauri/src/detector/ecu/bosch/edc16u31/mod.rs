// EDC16U31 VAG ECU detector - synchronized copy of the EDC16U34 algorithm
// with U31-specific zone/layout adaptations (VAG 1.9/2.0 TDI PD - MPC563).
// Key layout differences vs U34 (reference: VW_Passat_2_0TDI_1037382052):
//  - all map regions shifted down (turbo 0x1DCxxx vs 0x1E3xxx, SOI 0x1D34xx vs 0x1D64xx)
//  - Driver Wish uses SHARED axes + selector marker [8A 00 00 06] (not per-map headers)
//  - MVS trio variant with zeroed inner bytes, 5 speed switches instead of 3
//  - Torque Limiter uses the PD signature [00 00 00 3C 00 64][rows][cols]
// Architecture: Freescale MPC5xx, FLASH 512KB-2MB, data in BIG-ENDIAN
//
// Detection Strategy (inspired by EDCMasterHP + Grok analysis):
// 1. Use flexible grid sizes per map type (e.g., Boost Limiter: 10x10, 10x11, 11x10)
// 2. Validate axes using RPM detection (values ~500-5000, delta ~250-500)
// 3. Validate Z data using physical ranges AFTER conversion
// 4. Multi-factor confidence scoring system
// 5. Support for tuned files with extended ranges

mod signatures;

use crate::models::{DetectedMap, MapDimensions, DataType, MapCategory};
use std::collections::HashSet;

// ============== MAP TEMPLATES FROM GROK ANALYSIS ==============
// Based on 10+ EDC16U34 mappack files (500+ maps analyzed)

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

/// EDC16U34 ECU specific detector
/// Supports EDC16U1, EDC16U31, EDC16U34 variants (can be extended)
pub struct EDC16U31Detector {
    ecu_variant: EDC16Variant,
    tuned_mode: bool,  // Use extended ranges for tuned files
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EDC16Variant {
    EDC16U1,
    EDC16U31,
    EDC16U34,
    Unknown,
}

impl EDC16U31Detector {
    pub fn new(variant: EDC16Variant) -> Self {
        Self {
            ecu_variant: variant,
            tuned_mode: false,
        }
    }

    /// Create detector in tuned mode (extended value ranges)
    pub fn new_tuned(variant: EDC16Variant) -> Self {
        Self {
            ecu_variant: variant,
            tuned_mode: true,
        }
    }

    /// Main detection function - AXIS-FIRST algorithm (fast & accurate)
    /// Based on EDC16U34_Detection_Guide.md analysis
    pub fn detect(&self, data: &[u8]) -> Vec<DetectedMap> {
        log::debug!("🔧 [EDC16] Starting AXIS-FIRST detection for {:?}, file size: {} bytes, tuned_mode: {}",
                 self.ecu_variant, data.len(), self.tuned_mode);

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
        for map in vmax_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(map);
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
                let mut d06_emitted = false;
                let mut block_end = selector_map.address as usize + selector_map.size;
                for map in duration_maps {
                    let map_range = (map.address, map.address + map.size as u32);
                    if !self.overlaps(&detected_ranges, map_range) {
                        detected_addresses.insert(map.address);
                        detected_ranges.insert(map_range);
                        block_end = block_end.max(map.address as usize + map.size);
                        // Every EDC16 has exactly 6 main Duration maps
                        // (00-05). The dynamic tail maps of the block keep
                        // their range claimed (so no other pass reuses those
                        // bytes) but are NOT reported — hidden per user
                        // request… SAUF la première dynamique 10x10 : Stage X
                        // la liste comme « Duration 06 » (parité U34).
                        if map.name.as_deref() != Some("Duration (Dynamic)") {
                            all_maps.push(map);
                        } else if !d06_emitted
                            && matches!(
                                map.dimensions,
                                MapDimensions::TwoDimensional { rows: 10, cols: 10 }
                            )
                        {
                            let mut m = map;
                            m.name = Some("Duration 06".to_string());
                            all_maps.push(m);
                            d06_emitted = true;
                        }
                    }
                }

                // « Duration min. injection break after main injection » 6x4
                // dans/après le bloc durations (parité U34).
                if let Some(mb) = self.detect_duration_min_break(
                    data,
                    *selector_addr as usize,
                    block_end + 0x400,
                    &detected_addresses,
                ) {
                    let mb_range = (mb.address, mb.address + mb.size as u32);
                    detected_addresses.insert(mb.address);
                    detected_ranges.insert(mb_range);
                    all_maps.push(mb);
                }
            }
        }
        log::debug!("🔧 [EDC16] Duration block detection complete: {} selectors found", duration_selectors.len());

        // ========== PHASE 0.65: extras de parité Stage X (portés de U34) ==========
        // Fuel Volume Correction + Expected Fuel Temp, MAF Linearization,
        // Dpf switch — structures vérifiées présentes sur les U31 réels
        // (016JA : Dpf 0x1D5558, FVC 0x1E60DC, MAF lin 0x1C3B7A).
        let extra_maps = self.detect_stagex_extras(data, &detected_addresses);
        for map in extra_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_addresses.insert(map.address);
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

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

        // ========== PHASE 0.12: Injector BIP correction block ==========
        // Chained triple: fuel-temp correction (1D×6), basic characteristic
        // (1D×10) and multiple correction (10x10)
        let bip_maps = self.detect_bip_maps(data, &detected_addresses);
        for map in bip_maps {
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

        // ========== PHASE 1.5: Turbo-region 2D maps (header+axes only) ==========
        // Adds VGT/Wastegate, Turbo boost request and Inversed drivers wish that
        // the template scorer misses (works on modified files). Runs AFTER the
        // generic axis detection so already-found N75/Boost Target keep priority.
        let detected_addr_turbo: HashSet<u32> =
            detected_ranges.iter().map(|(start, _)| *start).collect();
        let turbo_maps = self.detect_turbo_region_maps(data, &detected_addr_turbo);
        for map in turbo_maps {
            let range = (map.address, map.address + map.size as u32);
            if !self.overlaps(&detected_ranges, range) {
                detected_ranges.insert(range);
                all_maps.push(map);
            }
        }

        // ========== PHASE 1.6: Region 0x1D header maps (header+axes only) ==========
        // Friction torque, Fuel correction 3x3, Torque limiter 2x2, extra SOI 13x14.
        // Runs after generic detection so overlap protects already-found maps.
        let detected_addr_r1d: HashSet<u32> =
            detected_ranges.iter().map(|(start, _)| *start).collect();
        let r1d_maps = self.detect_region1d_header_maps(data, &detected_addr_r1d);
        for map in r1d_maps {
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

                if has_selector && has_limiter {
                    log::debug!("✅ [EDC16] Valid SOI block at 0x{:X} (has Selector and Limiter)", addr);
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
                // Only the numbered main-block maps ("Start of injection 00".."09")
                // are subject to the valid-block filter. The two extra 13x14 SOI
                // maps detected by detect_region1d_header_maps live OUTSIDE any
                // block and must be kept — their name ("Start of injection (extra N)")
                // must therefore NOT trigger this filter.
                let is_numbered_soi = name.starts_with("Start of injection ")
                    && name.chars().nth(19).map_or(false, |c| c.is_ascii_digit());
                if is_numbered_soi {
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
                        // Dynamic maps WITHIN the block span (interleaved with the
                        // numbered maps — e.g. abcABC has one at 0x1D75FE, between
                        // block_start 0x1D6414 and block_end)
                        let is_within = map.address >= block_start && map.address <= block_end;
                        // Dynamic maps AFTER main block + metadata
                        let is_after = map.address > block_end
                                      && map.address < block_end + 106 + 5 * SOI_MAP_SIZE as u32;
                        is_before || is_within || is_after
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

        Self::normalize_boost_names(&mut all_maps);

        all_maps
    }

    /// Align the boost-family display names on the EDC15P convention
    /// (user request: every ECU family shows the same names).
    fn normalize_boost_names(maps: &mut [DetectedMap]) {
        for m in maps.iter_mut() {
            let new = match m.name.as_deref() {
                Some("Boost Target") | Some("Boost Target Map") => Some("Boost target map"),
                Some("Boost Target 2") => Some("Boost target map 2"),
                Some("Boost Limiter") | Some("Boost Limiter (Altitude)") => Some("Boost limit map"),
                Some("N75 Duty Cycle") => Some("N75 duty cycle"),
                Some("SVBL - Max Boost Pressure") | Some("Max Boost Pressure") => {
                    Some("SVBL (Single value boost limiter)")
                }
                Some("Turbo Boost Pressure Correction") => Some("Boost correction by temperature"),
                _ => None,
            };
            if let Some(n) = new {
                m.name = Some(n.to_string());
            }
        }
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

        // ========== PHASE 2b: Driver Wish maps (U31 shared-axes layout) ==========
        // One RPM axis + one pedal axis + selector marker + contiguous maps
        let dw_shared_maps = self.detect_driver_wish_shared_axes(data, start, end, &detected_addresses);
        for map in dw_shared_maps {
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

    /// Detect Driver Wish maps using signature-based detection
    ///
    /// Per-map structure: [00 R 00 P] header + axes + data, where
    /// - R = RPM point count (12-16: 16 on 2.0 TDI, 12-14 on T5 R5 engines)
    /// - P = pedal point count (8, or 9 when a kickdown point sits above 100%)
    /// - RPM axis: R values Big-Endian, strictly increasing to the redline
    ///   (3500-6500). Usually starts at 0, but reduced driving programs start
    ///   as high as 550 (Crafter).
    /// - Pedal axis: P values Big-Endian, starts <= 500, strictly increasing.
    ///   Usually ends at exactly 10000 (100%, index 7; 9-point axes append a
    ///   kickdown point 10001-10200), but reduced programs top out at
    ///   8000/9000 (Crafter eco/limp programs capped at 80-90% pedal).
    /// - Map data: R*P i16 torque values (never validated beyond sign/zero so
    ///   tuned files still match — axes and layout survive a stage remap)
    ///
    /// Driver Wish maps come in groups of 7-9 (one per driving program).
    /// STRICT candidates (RPM starts at 0 AND pedal[7] == 10000) are accepted
    /// on their own; RELAXED ones (reduced programs) only count when they sit
    /// in a chain of >= 3 consecutive blocks (headers within 96 bytes of the
    /// previous data end) — isolated lookalikes never qualify.
    fn detect_driver_wish_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let read = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);

        struct DwCandidate {
            header: usize,
            r: usize,
            p: usize,
            rpm_start: usize,
            pedal_start: usize,
            map_start: usize,
            map_size: usize,
            strict: bool,
            empty: bool,
        }

        // ---- Phase A: collect structural candidates ----
        let scan_end = end.min(data.len());
        let mut candidates: Vec<DwCandidate> = Vec::new();
        let mut offset = start;
        while offset + 4 < scan_end {
            let r = read(offset) as usize;     // RPM points
            let p = read(offset + 2) as usize; // Pedal points
            if !(12..=16).contains(&r) || !(8..=9).contains(&p) {
                offset += 2;
                continue;
            }

            let rpm_axis_start = offset + 4;
            let pedal_axis_start = rpm_axis_start + r * 2;
            let map_start = pedal_axis_start + p * 2;
            let map_size = r * p * 2;
            if map_start + map_size > scan_end {
                offset += 2;
                continue;
            }

            // RPM axis: strictly increasing to redline; start 0 (or <= 600
            // for reduced programs)
            let rpm: Vec<u16> = (0..r).map(|i| read(rpm_axis_start + i * 2)).collect();
            if rpm[0] > 600
                || !(3500..=6500).contains(&rpm[r - 1])
                || !rpm.windows(2).all(|w| w[0] < w[1])
            {
                offset += 2;
                continue;
            }

            // Pedal axis: strictly increasing, last point 7500-10200
            // (10000 = 100%; reduced programs cap at 8000/9000)
            let pedal: Vec<u16> = (0..p).map(|i| read(pedal_axis_start + i * 2)).collect();
            if pedal[0] > 500
                || !(7500..=10200).contains(&pedal[p - 1])
                || !pedal.windows(2).all(|w| w[0] < w[1])
            {
                offset += 2;
                continue;
            }

            // First torque cell must be non-negative (0 at idle is fine)
            let first_value = i16::from_be_bytes([data[map_start], data[map_start + 1]]);
            if first_value < 0 {
                offset += 2;
                continue;
            }

            // All-zero placeholder programs keep the chain alive but are
            // never emitted
            let empty = data[map_start..map_start + map_size].iter().all(|&b| b == 0);
            let strict = rpm[0] == 0
                && pedal[7] == 10000
                && (p == 8 || pedal[8] <= 10200);

            candidates.push(DwCandidate {
                header: offset,
                r,
                p,
                rpm_start: rpm_axis_start,
                pedal_start: pedal_axis_start,
                map_start,
                map_size,
                strict,
                empty,
            });
            offset = map_start + map_size;
        }

        // ---- Phase B: keep chains of >= 3, plus isolated strict blocks ----
        let Some(template) = MAP_TEMPLATES.iter().find(|t| t.name == "Driver Wish") else {
            return maps;
        };
        // MAXENCE has 70 bytes between blocks; the last program of each
        // Crafter/JA cluster sits 148 bytes after the previous data end
        const CHAIN_MAX_GAP: usize = 160;
        let mut i = 0;
        while i < candidates.len() {
            let mut j = i;
            while j + 1 < candidates.len()
                && candidates[j + 1].header
                    <= candidates[j].map_start + candidates[j].map_size + CHAIN_MAX_GAP
            {
                j += 1;
            }
            let chain = &candidates[i..=j];
            let keep_all = chain.len() >= 3;
            for c in chain {
                if c.empty || !(keep_all || c.strict) {
                    continue;
                }
                if detected.contains(&(c.map_start as u32)) {
                    continue;
                }
                log::debug!("🎯 [EDC16] Found Driver Wish ({}x{}) at 0x{:X} (chain of {})",
                    c.r, c.p, c.map_start, chain.len());
                let map = self.create_detected_map(
                    "Driver Wish",
                    c.map_start,
                    c.r,             // rows (RPM)
                    c.p,             // cols (Pedal)
                    c.rpm_start,     // Y axis (RPM)
                    c.pedal_start,   // X axis (Pedal)
                    0.95,
                    template,
                );
                maps.push(map);
            }

            // NOTE: the Driver Wish program selector (marker 8A 00, right
            // before the cluster) is intentionally NOT reported — hidden per
            // user request (not a tuning map).
            i = j + 1;
        }

        log::debug!("🔧 [EDC16] Driver Wish detection: found {} maps", maps.len());
        maps
    }

    /// Validate a Driver Wish program selector at marker position `m`.
    ///
    /// Layout: [8A 00][count u16 BE][count RPM thresholds BE, increasing]
    /// [count program entries as u16 with a small low-endian id: XX 00].
    /// Crafter: marker 0x1C2A3A, count 6, entries 01 00 ×6 at 0x1C2A4A;
    /// Passat (shared-axes layout): marker 0x1C1AC8, entries 00 00..05 00.
    /// The emitted address is the entries block (matches external listings).
    /// Kept unused: the selector is no longer reported (hidden per user
    /// request) but the layout knowledge stays available here.
    #[allow(dead_code)]
    fn dw_selector_at(&self, data: &[u8], m: usize) -> Option<DetectedMap> {
        let read = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
        if m + 4 > data.len() || data[m] != 0x8A || data[m + 1] != 0x00 {
            return None;
        }
        let count = read(m + 2) as usize;
        if !(2..=8).contains(&count) {
            return None;
        }
        let axis_start = m + 4;
        let entries_start = axis_start + count * 2;
        if entries_start + count * 2 > data.len() {
            return None;
        }
        let axis: Vec<u16> = (0..count).map(|i| read(axis_start + i * 2)).collect();
        if axis[0] > 2000 || axis[count - 1] > 8000 || !axis.windows(2).all(|w| w[0] < w[1]) {
            return None;
        }
        let entries_ok = (0..count).all(|i| {
            data[entries_start + i * 2] <= 10 && data[entries_start + i * 2 + 1] == 0
        });
        if !entries_ok {
            return None;
        }

        let mut map = DetectedMap::new(
            entries_start as u32,
            count * 2,
            MapDimensions::OneDimensional { length: count },
            DataType::Int16,
        );
        map.name = Some("Driver Wish Selector".to_string());
        map.category = Some(MapCategory::EngineTorqueRequest.display_name().to_string());
        map.unit = Some("[-]".to_string());
        map.correction_factor = Some(1.0);
        map.confidence = 0.9;
        map.is_little_endian = Some(true); // entries are XX 00 (program id)
        map.x_axis_address = Some(axis_start as u32);
        map.x_label = Some("rpm".to_string());
        map.x_axis_correction = Some(1.0);
        log::debug!("🎯 [EDC16] Found Driver Wish Selector at 0x{:X} ({} entries)", entries_start, count);
        Some(map)
    }

    /// Scan backwards (up to 250 bytes) from a Driver Wish cluster header for
    /// the program selector marker.
    #[allow(dead_code)]
    fn detect_dw_selector_before(&self, data: &[u8], cluster_header: usize) -> Option<DetectedMap> {
        let lo = cluster_header.saturating_sub(250);
        let mut m = cluster_header.saturating_sub(4);
        while m > lo {
            if let Some(map) = self.dw_selector_at(data, m) {
                return Some(map);
            }
            m -= 1;
        }
        None
    }

    /// Detect Driver Wish maps with the U31 SHARED-AXES layout.
    ///
    /// Unlike the per-map layout above (each map preceded by its own
    /// [00 10 00 08] header + axes), U31 files store ONE RPM axis (16 values,
    /// starting at 0), ONE pedal axis (8 values, ending at 10000), then a
    /// selector marker [8A 00][count u16] with `count` program entries
    /// (bytes [i, 00] for i in 0..count), and finally `count + 1` contiguous
    /// 16x8 maps of 256 bytes each.
    /// Passat reference: RPM axis at 0x1C1A82, pedal axis at 0x1C1AB4,
    /// marker at 0x1C1AC8 (count = 6), 7 maps from 0x1C1B04.
    /// Structure-only matching — map data is never validated, so tuned files
    /// (flattened/zeroed programs are skipped individually) still detect.
    fn detect_driver_wish_shared_axes(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        const COLS: usize = 16;
        const ROWS: usize = 8;
        const MAP_SIZE: usize = COLS * ROWS * 2;
        let read = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);

        let scan_end = end.min(data.len());
        let mut offset = start;
        while offset + 2 + 32 + 96 + MAP_SIZE <= scan_end {
            // [00 10] + 16 RPM values, first == 0, strictly increasing, last 3500-6500
            if read(offset) != 16 {
                offset += 2;
                continue;
            }
            let rpm_start = offset + 2;
            let rpm: Vec<u16> = (0..16).map(|i| read(rpm_start + i * 2)).collect();
            if rpm[0] != 0
                || !(3500..=6500).contains(&rpm[15])
                || !rpm.windows(2).all(|w| w[0] < w[1])
            {
                offset += 2;
                continue;
            }

            // Pedal axis: [00 08] + 8 values ending exactly at 10000 (100%),
            // located within 64 bytes after the RPM axis
            let mut pedal_start = None;
            let mut q = rpm_start + 32;
            while q + 18 <= scan_end && q < rpm_start + 32 + 64 {
                if read(q) == 8 {
                    let pedal: Vec<u16> = (0..8).map(|i| read(q + 2 + i * 2)).collect();
                    if pedal[0] <= 500
                        && pedal[7] == 10000
                        && pedal.windows(2).all(|w| w[0] < w[1])
                    {
                        pedal_start = Some(q + 2);
                        break;
                    }
                }
                q += 2;
            }
            let Some(pedal_start) = pedal_start else { offset += 2; continue; };

            // Selector marker: [8A 00][count u16], then `count` axis values,
            // then `count` program indices as byte pairs [i, 00]
            // (Passat: 8A 00 00 06 | axis 0,830,1631,2451,3271,4211 | 00 00 01 00 ... 05 00)
            let mut marker = None;
            let mut m = pedal_start + 16;
            while m + 4 <= scan_end && m < pedal_start + 16 + 64 {
                if data[m] == 0x8A && data[m + 1] == 0x00 {
                    let count = read(m + 2) as usize;
                    let entries_at = m + 4 + count * 2; // indices come after the axis
                    if (2..=8).contains(&count) && entries_at + count * 2 <= scan_end {
                        let entries_ok = (0..count).all(|i| {
                            data[entries_at + i * 2] == i as u8 && data[entries_at + i * 2 + 1] == 0
                        });
                        if entries_ok {
                            marker = Some((m, count));
                            break;
                        }
                    }
                }
                m += 1; // the marker byte can sit at odd offsets
            }
            let Some((marker_pos, count)) = marker else { offset += 2; continue; };

            // Maps: count + 1 contiguous 256-byte blocks after the selector
            // structure: marker(4) + axis(count*2) + indices(count*2) + 32
            // config bytes (Passat count=6: marker + 60 = 0x1C1B04)
            let first_map = marker_pos + 36 + count * 4;
            let n_maps = count + 1;
            if first_map + n_maps * MAP_SIZE > scan_end {
                offset += 2;
                continue;
            }

            if let Some(template) = MAP_TEMPLATES.iter().find(|t| t.name == "Driver Wish") {
                let mut added = 0;
                for i in 0..n_maps {
                    let map_start = first_map + i * MAP_SIZE;
                    if detected.contains(&(map_start as u32)) {
                        continue;
                    }
                    // Skip all-zero placeholder programs
                    if data[map_start..map_start + MAP_SIZE].iter().all(|&b| b == 0) {
                        continue;
                    }
                    let map = self.create_detected_map(
                        "Driver Wish",
                        map_start,
                        COLS,
                        ROWS,
                        rpm_start,    // Y axis (RPM, shared)
                        pedal_start,  // X axis (Pedal, shared)
                        0.95,
                        template,
                    );
                    maps.push(map);
                    added += 1;
                }
                if added > 0 {
                    log::debug!(
                        "🎯 [EDC16U31] Driver Wish shared-axes block: {} maps at 0x{:X} (axes 0x{:X}/0x{:X})",
                        added, first_map, rpm_start, pedal_start
                    );
                    // The selector marker anchoring this layout is also a map
                    if let Some(selector) = self.dw_selector_at(data, marker_pos) {
                        if !detected.contains(&selector.address) {
                            maps.push(selector);
                        }
                    }
                    offset = first_map + n_maps * MAP_SIZE;
                    continue;
                }
            }
            offset += 2;
        }

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

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] < values[i-1] {
                return false;
            }
        }

        true
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

        // Type C: 16x10 avec en-tête [00 10 00 0A] et RPM paddé (KN)
        let type_c_maps = self.detect_boost_limiter_type_c(data, start, end, &local_detected);
        for map in type_c_maps {
            local_detected.insert(map.address);
            maps.push(map);
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

    /// Detect Boost Limiter Type C: en-tête [00 10][00 0A] + RPM 16 valeurs
    /// (paddées en marches de 1 en fin, style Crafter) + atmo 10 + map 16×10
    /// (lignes du bas répétées). Référence : KN @0x1EC5D2, la fin de la map
    /// tombe exactement sur le SVBL.
    fn detect_boost_limiter_type_c(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        const RPM_LEN: usize = 16;
        const ATMO_LEN: usize = 10;
        const MAP_SIZE: usize = RPM_LEN * ATMO_LEN * 2; // 320 octets

        let mut offset = start;
        while offset + 4 + RPM_LEN * 2 + ATMO_LEN * 2 + MAP_SIZE <= end.min(data.len()) {
            // En-tête [00 10][00 0A]
            if !(data[offset] == 0x00
                && data[offset + 1] == 0x10
                && data[offset + 2] == 0x00
                && data[offset + 3] == 0x0A)
            {
                offset += 2;
                continue;
            }
            let rpm_start = offset + 4;
            let rpm_axis = self.read_values_be_unsigned(data, rpm_start, RPM_LEN);
            // 16 RPM strictement croissants, plage plausible (le padding en
            // marches de 1 reste strictement croissant)
            let rpm_ok = rpm_axis.windows(2).all(|w| w[0] < w[1])
                && (1000..=2200).contains(&rpm_axis[0])
                && rpm_axis[RPM_LEN - 1] <= 6000
                && rpm_axis[RPM_LEN - 1] >= 4000;
            if !rpm_ok {
                offset += 2;
                continue;
            }
            let atmo_start = rpm_start + RPM_LEN * 2;
            let atmo_axis = self.read_values_be_unsigned(data, atmo_start, ATMO_LEN);
            if !self.is_valid_atmospheric_axis(&atmo_axis) {
                offset += 2;
                continue;
            }
            let map_start = atmo_start + ATMO_LEN * 2;
            if detected.contains(&(map_start as u32)) {
                offset += 2;
                continue;
            }
            if self.validate_boost_limiter_data(data, map_start, RPM_LEN, ATMO_LEN) {
                if let Some(template) = MAP_TEMPLATES.iter().find(|t| t.name == "Boost Limiter") {
                    log::debug!("🔧 [Boost Limiter Type C] Found 16x10 @ 0x{:X}", map_start);
                    let map = self.create_detected_map(
                        "Boost Limiter",
                        map_start,
                        RPM_LEN,
                        ATMO_LEN,
                        rpm_start,
                        atmo_start,
                        0.85,
                        template,
                    );
                    maps.push(map);
                    offset = map_start + MAP_SIZE - 2;
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

    // ========== N75 DUTY CYCLE / VGT DETECTION (SIGNATURE-BASED) ==========
    // N75/VGT maps have signature pattern: [00][rows][00][cols] before axes
    // Valid configurations (from StageX analysis):
    //   - 16x10, 16x11, 16x12, 16x13 (16 RPM rows, 10-13 IQ cols)
    //   - 13x16 (13 RPM rows, 16 IQ cols)
    //   - 10x16 (10 RPM rows, 16 IQ cols)
    // NOT valid: 12x12 (false positives)
    // Structure: Signature + RPM axis (rows values) + IQ axis (cols values) + Map data

    /// Detect N75 Duty Cycle / Variable Geometry maps by signature pattern
    /// Signature: [00 XX 00 YY] where XX=rows, YY=cols
    fn detect_n75_by_signature(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let (start, end) = self.get_data_section_range(data.len());

        // N75/VGT valid configurations (explicit list to avoid false positives like 12x12):
        // (rows, cols) pairs
        let valid_configs: &[(u8, u8)] = &[
            (16, 10), (16, 11), (16, 12), (16, 13),  // 16 RPM rows
            (14, 10),                                 // 14 RPM rows (U34-style 10x14 blocks)
            (12, 10),                                 // 12 RPM rows (U31: 10x12 block at 0x1DCCxx)
            (13, 16),                                 // 13 RPM rows
            (10, 16),                                 // 10 RPM rows
        ];

        let mut offset = start;

        while offset + 100 <= end.min(data.len()) {
            // Check for N75/VGT signature pattern: [00 XX 00 YY]
            if offset + 4 <= data.len() &&
               data[offset] == 0x00 && data[offset + 2] == 0x00 {
                let rows = data[offset + 1] as usize;
                let cols = data[offset + 3] as usize;

                // Valid N75/VGT configurations (explicit check)
                if valid_configs.contains(&(rows as u8, cols as u8)) {
                    let sig_offset = offset;

                    // After signature: RPM axis (rows values)
                    let rpm_axis_start = sig_offset + 4;
                    if rpm_axis_start + rows * 2 + cols * 2 > data.len() {
                        offset += 2;
                        continue;
                    }

                    // Read and validate RPM axis
                    let rpm_axis = self.read_values_be_unsigned(data, rpm_axis_start, rows);
                    if !self.is_valid_n75_rpm_axis(&rpm_axis) {
                        log::debug!("🔧 [N75] Rejected at 0x{:X} ({}x{}): invalid RPM axis {:?}", sig_offset, rows, cols, &rpm_axis[..rpm_axis.len().min(4)]);
                        offset += 2;
                        continue;
                    }

                    // After RPM axis: IQ axis immediately follows (no padding)
                    // Note: First IQ value is often 0 (0x0000), not padding
                    let iq_axis_start = rpm_axis_start + rows * 2;

                    // Read and validate IQ axis
                    let iq_axis = self.read_values_be_unsigned(data, iq_axis_start, cols);
                    if !self.is_valid_n75_iq_axis(&iq_axis) {
                        log::debug!("🔧 [N75] Rejected at 0x{:X} ({}x{}): invalid IQ axis {:?}", sig_offset, rows, cols, &iq_axis[..iq_axis.len().min(4)]);
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
                        log::debug!("🔧 [N75] Skipped at 0x{:X}: already detected", map_start);
                        offset = map_start + map_size;
                        continue;
                    }

                    // Validate map data (N75/VGT duty cycle values)
                    if !self.validate_n75_data(&data[map_start..map_start + map_size]) {
                        log::debug!("🔧 [N75] Rejected at 0x{:X} ({}x{}): invalid map data", map_start, rows, cols);
                        offset += 2;
                        continue;
                    }

                    log::debug!("✅ [EDC16] Found N75/VGT Duty Cycle ({}x{}) at 0x{:X}", rows, cols, map_start);

                    let mut map = DetectedMap::new(
                            map_start as u32,
                            map_size,
                            MapDimensions::TwoDimensional { rows, cols },
                            DataType::Int16,
                        );

                        map.name = Some("N75 Duty Cycle".to_string());
                        map.category = Some(MapCategory::TurboBoostPressureControl.display_name().to_string());
                        map.unit = Some("%".to_string());
                        map.correction_factor = Some(0.012207);
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
            offset += 2;
        }

        log::debug!("🔧 [EDC16] N75/VGT signature detection: found {} maps", maps.len());
        maps
    }

    /// Validate N75/VGT RPM axis (10, 12, 13, 14 or 16 values, 600-5500 RPM range)
    fn is_valid_n75_rpm_axis(&self, values: &[u16]) -> bool {
        // Valid RPM axis lengths: 10, 12, 13, 14, 16 (matching valid configs)
        let valid_lengths = [10, 12, 13, 14, 16];
        if !valid_lengths.contains(&values.len()) {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // N75 RPM: starts 0-1500, ends 3500-5500
        // First value can be 0 (idle) up to 1500
        if first > 1500 {
            return false;
        }
        if last < 3500 || last > 5500 {
            return false;
        }

        // Must be monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i - 1] {
                return false;
            }
        }

        true
    }

    /// Validate N75/VGT IQ axis (10-13 or 16 values, 0-7000 mg/stroke range)
    fn is_valid_n75_iq_axis(&self, values: &[u16]) -> bool {
        // Valid IQ axis lengths: 10, 11, 12, 13, 16 (matching valid configs)
        let valid_lengths = [10, 11, 12, 13, 16];
        if !valid_lengths.contains(&values.len()) {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // N75 IQ: starts 0-700, ends >= 3500 (35 mg/stroke with factor 0.01)
        // Real N75 maps need IQ axis ending at least 35 mg/stroke
        if first > 700 {
            return false;
        }
        if last < 3500 || last > 8500 {
            return false;
        }

        // Must be monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i - 1] {
                return false;
            }
        }

        true
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

        // N75 duty cycle: factor 0.012207 (raw 0-8192 = 0-100%)
        let factor = 0.012207;

        let min_val = values.iter().cloned().min().unwrap_or(0) as f64 * factor;
        let max_val = values.iter().cloned().max().unwrap_or(0) as f64 * factor;

        // Valid range: 0% to 120%
        if min_val < -5.0 || max_val > 125.0 {
            return false;
        }

        // Average should be reasonable for N75 (30-115%)
        // Relaxed lower bound from 40% to 30% for edge cases
        let avg: f64 = values.iter().map(|&v| v as f64 * factor).sum::<f64>() / values.len() as f64;
        if avg < 30.0 || avg > 115.0 {
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
        while offset + SOI_BLOCK_SIZE + 100 <= end.min(data.len()) {
            // Look for metadata pattern: value 16 followed by valid RPM axis
            let potential_rows = self.read_u16_be(data, offset);

            if potential_rows == SOI_ROWS as u16 {
                _found_16_count += 1;
                // Read potential RPM axis (16 values after the "16")
                let rpm_axis = self.read_values_be_unsigned(data, offset + 2, SOI_ROWS);

                if self.is_valid_soi_rpm_axis(&rpm_axis) {
                    // Check for cols count after RPM axis
                    let potential_cols = self.read_u16_be(data, offset + 2 + SOI_ROWS * 2);

                    if potential_cols == SOI_COLS as u16 {
                        // Read potential IQ axis (14 values)
                        let iq_axis = self.read_values_be_unsigned(data, offset + 2 + SOI_ROWS * 2 + 2, SOI_COLS);

                        if self.is_valid_soi_iq_axis(&iq_axis) {
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
                                // SOI maps have very specific characteristics:
                                // - Values typically range from 0x0100 (256) to 0x0300 (768) = ~6° to ~18° after factor
                                // - Very consistent values - lots of repetition (same value appears many times)
                                // - First few values in each row are usually very similar
                                // - No extreme values (>1200 or <50)
                                let validate_soi_data = |map_data: &[u8]| -> bool {
                                    if map_data.iter().all(|&b| b == 0) {
                                        return false;
                                    }

                                    let mut values: Vec<i16> = Vec::with_capacity(SOI_MAP_SIZE / 2);
                                    let mut valid_soi_count = 0;
                                    let mut bad_value_count = 0;
                                    let mut typical_soi_count = 0;
                                    let total_values = SOI_MAP_SIZE / 2;

                                    for i in (0..SOI_MAP_SIZE).step_by(2) {
                                        let val = ((map_data[i] as i16) << 8) | (map_data[i + 1] as i16);
                                        values.push(val);

                                        // Typical SOI values: 200-700 raw (4.7° to 16.4°)
                                        if val >= 200 && val <= 700 {
                                            typical_soi_count += 1;
                                        }

                                        // Acceptable SOI values: 50-1000 raw
                                        if val >= 50 && val <= 1000 {
                                            valid_soi_count += 1;
                                        }

                                        // Bad values: significantly out of SOI range
                                        if val > 1200 || val < 0 {
                                            bad_value_count += 1;
                                        }
                                    }

                                    // Check the first 16 values (first row) - they should be in typical SOI range
                                    // Real SOI maps start with values like 0x01AB (427), 0x01D5 (469)
                                    // Not random values like 0x0069, 0x0ADD, etc.
                                    let first_row_typical_count = values.iter()
                                        .take(16)
                                        .filter(|&&v| v >= 200 && v <= 700)
                                        .count();

                                    // Also check that there are no extreme values in first row
                                    let first_row_bad_count = values.iter()
                                        .take(16)
                                        .filter(|&&v| v > 1200 || v < 50)
                                        .count();

                                    // Require:
                                    // - At least 50% in typical SOI range (200-700)
                                    // - At least 80% in acceptable range (50-1000)
                                    // - Less than 3% bad values overall
                                    // - First row: at least 12 of 16 values in typical range
                                    // - First row: no bad values
                                    typical_soi_count >= (total_values * 50 / 100)
                                        && valid_soi_count >= (total_values * 80 / 100)
                                        && bad_value_count <= (total_values * 3 / 100)
                                        && first_row_typical_count >= 12
                                        && first_row_bad_count == 0
                                };

                                // The dynamic maps normally sit CONTIGUOUS right before the
                                // main block, on the 448-byte grid anchored at block_start
                                // (crafter: 3 maps at block_start - k*448 exactly). Walk that
                                // grid backwards FIRST: the +2 creeping scan below can lock
                                // on 4 bytes too early when stray calibration words (500/400)
                                // pass the SOI value filter, shifting every dynamic map
                                // (crafter: 0x18E018 instead of the real 0x18E01C).
                                let mut grid_addrs: Vec<usize> = Vec::new();
                                for k in 1..=max_pre_dynamic_maps {
                                    let Some(p) = block_start.checked_sub(k * SOI_MAP_SIZE) else { break; };
                                    if p + SOI_MAP_SIZE > data.len()
                                        || !validate_soi_data(&data[p..p + SOI_MAP_SIZE]) {
                                        break;
                                    }
                                    grid_addrs.push(p);
                                }
                                for &p in grid_addrs.iter().rev() {
                                    if detected.contains(&(p as u32)) {
                                        continue;
                                    }
                                    let mut dyn_map = DetectedMap::new(
                                        p as u32,
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

                                // Fallback: no grid-aligned dynamics — some layouts keep a
                                // gap between the dynamics and the block (EDCMasterHP);
                                // use the original creeping scan for those.
                                if pre_dynamic_maps.is_empty() {
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
                                } // end fallback creeping scan

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

                                for dyn_idx in 0..max_dynamic_maps {
                                    let dyn_map_addr = dynamic_start + dyn_idx * SOI_MAP_SIZE;

                                    if dyn_map_addr + SOI_MAP_SIZE > data.len() {
                                        break;
                                    }

                                    // Check if this map is all zeros (empty placeholder)
                                    let map_data = &data[dyn_map_addr..dyn_map_addr + SOI_MAP_SIZE];
                                    let all_zeros = map_data.iter().all(|&b| b == 0);

                                    if all_zeros {
                                        break;  // Stop at first empty map
                                    }

                                    // Validate this looks like SOI data (values in reasonable range)
                                    // SOI values typically range from ~0x0080 to ~0x0300 raw (2-12° after factor)
                                    // We need:
                                    // 1. At least 60% of values in valid non-zero range (100-1000)
                                    // 2. Very few "bad" values (above 1500 or below -500)
                                    let mut valid_soi_count = 0;
                                    let mut bad_value_count = 0;
                                    let total_values = SOI_MAP_SIZE / 2;  // 224 values
                                    for i in (0..SOI_MAP_SIZE).step_by(2) {
                                        let val = ((map_data[i] as i16) << 8) | (map_data[i + 1] as i16);
                                        // Valid SOI values: 100-1000 raw (2.3° to 23°)
                                        if val >= 100 && val <= 1000 {
                                            valid_soi_count += 1;
                                        }
                                        // Bad values: significantly out of SOI range
                                        if val > 1500 || val < -500 {
                                            bad_value_count += 1;
                                        }
                                    }
                                    // Require at least 60% valid SOI values and less than 5% bad values
                                    let has_valid_soi_values = valid_soi_count >= (total_values * 60 / 100)
                                                            && bad_value_count <= (total_values * 5 / 100);

                                    if !has_valid_soi_values {
                                        break;
                                    }

                                    if detected.contains(&(dyn_map_addr as u32)) {
                                        continue;
                                    }

                                    let mut dyn_map = DetectedMap::new(
                                        dyn_map_addr as u32,
                                        SOI_MAP_SIZE,
                                        MapDimensions::TwoDimensional { rows: SOI_ROWS, cols: SOI_COLS },
                                        DataType::Int16,
                                    );

                                    // Dynamic SOI maps are not numbered, just called "SOI Dynamic"
                                    dyn_map.name = Some("Start of injection Dynamic".to_string());
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
                                    log::debug!("🎯 [EDC16] Found {} dynamic SOI maps after block at 0x{:X}", dynamic_count, block_start);
                                }

                                // Skip past this block including dynamic maps
                                offset = dynamic_start + dynamic_count * SOI_MAP_SIZE + SOI_MAP_SIZE;  // Skip past last dynamic + one empty
                                continue;
                            }
                        }
                    }
                }
            }
            offset += 2;
        }

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

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] < values[i-1] {
                return false;
            }
        }

        true
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

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i-1] {
                return false;
            }
        }

        true
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
                            // X axis: SOI timing in 0.1 degree units with Kelvin-style offset
                            // Formula: degree = (rawValue * 0.1) - 273.1
                            // Example: 2531 * 0.1 - 273.1 = 253.1 - 273.1 = -20.0°
                            map.x_axis_correction = Some(0.1);
                            map.x_axis_offset = Some(-273.1);
                            map.y_axis_correction = Some(1.0);       // RPM
                            map.x_label = Some("deg CrS".to_string());
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

        // SOI Limiter RPM: starts 300-500, ends 4500-5500
        if first < 300 || first > 600 {
            return false;
        }
        if last < 4500 || last > 5500 {
            return false;
        }

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] < values[i-1] {
                return false;
            }
        }

        true
    }

    /// Validate SOI Limiter X axis (11 values, range 2500-3600)
    fn is_valid_soi_limiter_x_axis(&self, values: &[u16]) -> bool {
        if values.len() != 11 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // X axis range: 2400-2700 first, 3400-3700 last
        if first < 2400 || first > 2700 {
            return false;
        }
        if last < 3400 || last > 3700 {
            return false;
        }

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i-1] {
                return false;
            }
        }

        true
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

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i-1] {
                return false;
            }
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

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i-1] {
                return false;
            }
        }

        true
    }

    /// Check Boost Limiter RPM axis Type B (10 values, 1500-4500 range)
    /// Pattern from passat: 1500, 1750, 1900, 2250, 2500, 3000, 3500, 4000, 4250, 4500
    fn is_valid_boost_limiter_rpm_axis_10(&self, values: &[u16]) -> bool {
        if values.len() != 10 {
            return false;
        }

        let first = values[0];
        let last = values[values.len() - 1];

        // Type B RPM: 1400-1800 first / 4300-4700 last on passenger cars,
        // 700..5000 on the Crafter (10x10 at 0x1ECD52) — bounds widened,
        // the atmospheric axis + data validation remain the discriminator
        if first < 650 || first > 1800 {
            return false;
        }
        if last < 4300 || last > 5100 {
            return false;
        }

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i-1] {
                return false;
            }
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

        // Atmospheric pressure: 600-700 first, 1000-1150 last
        if first < 550 || first > 750 {
            return false;
        }
        if last < 950 || last > 1200 {
            return false;
        }

        // Check monotonically increasing
        for i in 1..values.len() {
            if values[i] <= values[i-1] {
                return false;
            }
        }

        true
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
        // Based on EDC16U34_Detection_Guide.md signatures:
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
        // EDC16U34: typically 8 values from 0 to 10000
        if len >= 6 && len <= 10 && first <= 500 && last >= 7500 && last <= 11000 {
            return XAxisType::Pedal;
        }

        // Torque: 0-4000 (0-400 Nm with factor 0.1), 14-18 values
        // EDC16U34: 16 values typically, 0 to ~3360 (336 Nm)
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

        // Last value should be 2000-9000 (200-900 Nm - wider range for tuned files)
        // Stock files: ~350-450 Nm, Tuned files: up to 720+ Nm
        let last = values[values.len() - 1];
        if last < 2000 || last > 9000 {
            return false;  // Invalid range for Torque axis
        }

        // Check monotonic increase
        let mut prev = values[0];
        for i in 1..values.len() {
            if values[i] <= prev {
                return false;  // Not monotonically increasing
            }
            if values[i] > 10000 {
                return false;  // Out of Torque range (1000 Nm max for heavily tuned)
            }
            prev = values[i];
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

        let mut prev = 0u16;
        for (i, &val) in values.iter().enumerate() {
            if val > 6000 {
                return false;  // RPM too high
            }
            if i > 0 && val <= prev {
                return false;  // Not strictly increasing
            }
            prev = val;
        }

        true
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

        // Variant 2: 16x16-18 (VWPassat2l and similar 2.0L engines; the
        // Crafter R5 uses 16 RPM rows × 18 torque columns at 0x1D8AA0)
        if let Some(map) = self.detect_torque_to_iq_variant_single(data, start, end, 0x10, 16, 18, detected) {
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

        // Filter candidates to valid address range: 0x1D0000 - 0x1E0000
        // Real Torque to IQ maps are ALWAYS in this memory region based on JSON definitions
        let valid_candidates: Vec<DetectedMap> = candidates.into_iter()
            .filter(|m| m.address >= 0x1D0000 && m.address < 0x1E0000)
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

        // The real Torque to IQ map always lives in 0x1D0000-0x1E0000 (the
        // post-filter already enforces it). Clamping the SCAN window matters
        // too: this function returns the FIRST match, and a valid-looking
        // structure earlier in the file (KN: 0x1C68F2) used to win, get
        // filtered out, and leave the real map to the turbo-region pass
        // (which then misnamed it "Turbo boost pressure request").
        let start = start.max(0x1D0000);
        let end = end.min(0x1E0000);

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

        // Variant 3: 9x8 maps (Crafter R5 "Start IQ": RPM 100-1400 × temp
        // 2431-3531, pair at 0x1F214E/0x1F222C)
        let maps_9x8 = self.detect_cranking_variant(data, start, end, 9, 8, &local_detected);
        for map in &maps_9x8 {
            local_detected.insert(map.address);
        }
        maps.extend(maps_9x8);

        // Variant 4: 8x9 maps (u31full : RPM 200-1400 sans point 0 × temp
        // 2431-3631, paire à 0x1E24D8/0x1E25B6)
        let maps_8x9 = self.detect_cranking_variant(data, start, end, 8, 9, &local_detected);
        maps.extend(maps_8x9);

        log::debug!("🔧 [EDC16] Cranking Torque Map detection: found {} maps (9x9 + 9x10 + 9x8 + 8x9 variants)", maps.len());
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
        // Typical last value: 1400-1500 RPM
        let last = *values.last().unwrap();
        if last < 400 || last > 2000 {
            return false;  // Last value should be 400-2000 RPM
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
        // Typical last value: ~3700 (about 100°C)
        let last = *values.last().unwrap();
        if last < 3000 || last > 4500 {
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

        // Torque values: factor 0.1, range 0-336 Nm (raw 0-3360)
        let max_val = values.iter().cloned().max().unwrap_or(0);

        // Should have reasonable torque values (not all zeros, not too high)
        if max_val == 0 || max_val > 5000 {
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
        const COLS: usize = 10;

        let mut offset = start;
        while offset + 400 <= end.min(data.len()) {
            // Signature [00 rows][00 0A] : rows = 16 (axe paddé, JA) ou 11
            // (axe réel sans padding, KN @0x1D73DA)
            if offset + 4 <= data.len() &&
               data[offset] == 0x00 && (data[offset + 1] == 0x10 || data[offset + 1] == 0x0B) &&
               data[offset + 2] == 0x00 && data[offset + 3] == 0x0A {
                #[allow(non_snake_case)]
                let ROWS = data[offset + 1] as usize;

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
        // 16 valeurs (axe paddé, ex. JA) ou 11 valeurs réelles (ex. KN)
        if values.len() != 16 && values.len() != 11 {
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
        // 16x10 (320 octets) ou 11x10 (220 octets, variante KN)
        if map_data.len() != 16 * 10 * 2 && map_data.len() != 11 * 10 * 2 {
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

        if values.len() != 160 && values.len() != 110 {
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

        // Signature: [00 8B][mv_high] = 139 + upper mV calibration bound.
        // The second word VARIES by sensor calibration: 0x03E7 (999) on most
        // firmwares but 0x03E0 (992) on Passat U31 etc. — match [00 8B] plus
        // a plausible mV range instead of an exact value. The cascade of
        // checks below (count == 2, mV/pressure ranges, axis coherence)
        // keeps the match unambiguous.
        let mut offset = start;
        while offset + 20 <= end.min(data.len()) {
            // Check for signature
            if offset + 14 <= data.len() &&
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

                // Coherence: the signature's upper bound must sit close to the
                // X axis upper mV value (|diff| <= 100 on the whole corpus)
                let sig_hi = ((data[offset + 2] as u16) << 8) | (data[offset + 3] as u16);
                let mv_hi = ((data[offset + 8] as u16) << 8) | (data[offset + 9] as u16);
                if (sig_hi as i32 - mv_hi as i32).abs() > 100 {
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
    /// Multiple dimension variants based on engine displacement:
    /// - 20x4: 1.9L standard (g5runtrophy, VWGolf1.9l, etc.)
    /// - 21x3: 1.9L variant (AudiA31.9l, SeatLeon1.9l, etc.)
    /// - 22x3: 2.0L standard (AudiA32l, passat2lbkd, etc.)
    /// - 22x4: 2.0L variant (SeatAltea2l, VWEos2l, etc.)
    ///
    /// Structure: [VV VV][VV VV][VV VV][00 rows][00 cols][Y axis][X axis][map data]
    /// (placeholders are usually 0x7FFF on stock files, but tuned dumps may use
    /// a soft-cap value like 0x0BB8 / 3000 Nm.)
    /// X axis: RPM (0-5500), Y axis: Atmospheric pressure (600-1100 mbar)
    /// Values: Torque in Nm * 10 (factor 0.1)
    ///
    /// Returns at most one Torque Limiter map (the lowest-address valid match
    /// across all variants). The relaxed placeholder signature can produce
    /// secondary candidates at higher addresses that all describe the same
    /// underlying map; we keep only the canonical (first) one.
    fn detect_torque_limiter_maps(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        detected: &HashSet<u32>,
    ) -> Vec<DetectedMap> {
        // Prefer the PD-style signature first (U31/U1 layout, e.g. Passat 22x4
        // at 0x1CF2CA behind [00 00 00 3C 00 64][00 04][00 16]).
        for (rows, cols) in [(3usize, 20usize), (3, 21), (4, 21), (4, 22)] {
            let found = self.detect_torque_limiter_u31_sig(data, start, end, rows, cols, detected);
            if !found.is_empty() {
                log::debug!("🔧 [EDC16U31] Torque Limiter detected ({}x{}) at 0x{:X}", cols, rows, found[0].address);
                return found.into_iter().take(1).collect();
            }
        }

        // Fallback to legacy U34-style signature variants.
        // (4, 23): Crafter EngPrt_trqLimP at 0x1D5ED8 — 7F FF placeholders +
        // [00 04][00 17], 4 pressure rows × 23 RPM columns.
        // (4, 25) / (3, 24): T5 R5 layout (MAXENCE at 0x1D5960/0x1D5A90).
        // The axis validators (pressure 600-1100 mbar + monotonic RPM) gate
        // false positives, so the accepted grid can stay wide.
        let mut all_candidates: Vec<DetectedMap> = Vec::new();
        for (rows, cols) in [
            (4usize, 20usize), (3, 21), (3, 22), (4, 22), (4, 23),
            (3, 23), (3, 24), (3, 25), (4, 24), (4, 25),
        ] {
            all_candidates.extend(self.detect_torque_limiter_variant(data, start, end, rows, cols, detected));
        }

        let mut result = Vec::new();
        if let Some(first) = all_candidates.into_iter().min_by_key(|m| m.address) {
            log::debug!("🔧 [EDC16U31] Torque Limiter (legacy sig) selected at 0x{:X}", first.address);
            result.push(first);
        }

        // The 10x3 secondary limiter variant is intentionally NOT included here:
        // the user wants exactly one Torque Limiter entry, and the principal
        // variants above always cover the canonical EngPrt_trqLimP_MAP.

        log::debug!("🔧 [EDC16U31] Torque Limiter detection: returning {} map(s)", result.len());
        result
    }

    /// EDC16U31-specific Torque Limiter detector (same layout as U1).
    ///
    /// Signature: `[00 00 00 3C 00 64][00 rows][00 cols]` followed by the
    /// pressure axis, the RPM axis, then signed-i16 map data (big-endian).
    fn detect_torque_limiter_u31_sig(
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
                return maps;
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

        // Three signature variants exist:
        // Variant 1 (U34): [00 05][00 00][00 32][00 00][00 0A][00 0A] - g5runtrophy, lectureG5, etc.
        // Variant 2 (U34): [00 05][00 00][00 32][75 30][00 0A][00 0A] - VWPassat2l, VWJetta1.9l, etc.
        //   Common prefix: [00 05][00 00][00 32] then 2 bytes variable then [00 0A][00 0A]
        // Variant 3 (U31): [00 64][01 90][00 00][00 32][00 32][SVGL u16][00 0A][00 0A]
        //   (Passat U31 at 0x1D2612 — the 2 variable bytes at +10 hold the
        //    Single Value Gearbox Limiter that Stage X lists at 0x1D261C)
        let signature_prefix: [u8; 6] = [0x00, 0x05, 0x00, 0x00, 0x00, 0x32];
        let signature_suffix: [u8; 4] = [0x00, 0x0A, 0x00, 0x0A];
        let sig31_head: [u8; 4] = [0x00, 0x64, 0x01, 0x90];
        let sig31_mid: [u8; 6] = [0x00, 0x00, 0x00, 0x32, 0x00, 0x32];

        let y_axis_size = rows * 2;  // RPM axis (10 × 2 = 20 bytes)
        let x_axis_size = cols * 2;  // Vehicle speed axis (10 × 2 = 20 bytes)
        let map_size = rows * cols * 2;  // Map data (10 × 10 × 2 = 200 bytes)
        // Structure: signature(12 or 16) + Y axis(20) + X axis(20) + data(200)
        let total_structure_size = 16 + y_axis_size + x_axis_size + map_size;

        let mut offset = start;
        while offset + total_structure_size <= end.min(data.len()) {
            // Variant 1/2 (U34 layout): sig at offset, axes at offset+12
            let is_u34_sig = offset + 12 <= data.len()
                && data[offset..offset + 6] == signature_prefix
                && data[offset + 8..offset + 12] == signature_suffix;
            // Variant 3 (U31 layout): sig at offset, SVGL at +10, axes at offset+16
            let is_u31_sig = !is_u34_sig
                && offset + 16 <= data.len()
                && data[offset..offset + 4] == sig31_head
                && data[offset + 4..offset + 10] == sig31_mid
                && data[offset + 12..offset + 16] == signature_suffix;

            if is_u34_sig || is_u31_sig {
                // Structure: signature -> Y axis -> X axis -> Map data
                let y_axis_start = if is_u31_sig { offset + 16 } else { offset + 12 };
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

                // The Launch Control map itself is intentionally NOT reported
                // (hidden per user request). The block is still located and
                // validated because the U31 signature variant embeds the
                // Single Value Gearbox Limiter, which IS reported below.
                let _ = (switch_address, switch_value, is_enabled);

                // U31 layout: the two variable bytes inside the signature hold
                // the Single Value Gearbox Limiter (Stage X lists it, e.g.
                // Passat 0x1D261C = 0x7530 = 3000.0 Nm) — same fields as the
                // SVGL created by detect_gearbox_torque_limiter.
                if is_u31_sig {
                    let svgl_addr = offset + 10;
                    if !detected.contains(&(svgl_addr as u32)) {
                        let mut single_map = DetectedMap::new(
                            svgl_addr as u32,
                            2,
                            MapDimensions::OneDimensional { length: 1 },
                            DataType::Int16,
                        );
                        single_map.name = Some("Single Value Gearbox Limiter".to_string());
                        single_map.category = Some("Gearbox".to_string());
                        single_map.unit = Some("Nm".to_string());
                        single_map.correction_factor = Some(0.1);
                        single_map.offset = Some(0.0);
                        single_map.confidence = 0.93;
                        maps.push(single_map);
                    }
                }

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
    /// followed by `00 rows 00 cols`. Stock files use `7F FF` (32767, the i16
    /// max), but tuned files often rewrite the placeholders with a soft cap
    /// (e.g. `0B B8` = 3000 Nm) so we accept any three matching non-zero
    /// values and rely on the axis/data validators to reject false positives.
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
            // Three identical 16-bit BE placeholders + 00 rows + 00 cols
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

        // RPM axis: starts at 0 or very low, ends at 4500-5500
        if first > 100 {
            return false;  // First value should be 0 or very low
        }
        if last < 4500 || last > 6000 {
            return false;  // Last value should be ~5000-5500 RPM
        }

        // Check monotonically increasing (mostly)
        for i in 1..values.len() {
            if values[i] < values[i - 1] {
                return false;
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
            // Check for exact byte pattern
            if data[offset..offset + 12] == selector_pattern {
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

        // The U31 Duration block holds up to 10 maps (Passat: 10x10, 15x17,
        // 3× 15x16, 3× 10x10, 9x12, 8x12 — the 9x12/8x12 tail maps are part
        // of the same contiguous block). Capping prevents the scan from
        // spilling into the maps that follow the block.
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
            // - Cols: 8-15 (U31 tail maps go down to 8 IQ columns, e.g. 8x12)
            let valid_signature = sig_rows >= 9 && sig_rows <= 20 &&
                                  sig_cols >= 8 && sig_cols <= 15;

            if !valid_signature {
                log::debug!("[EDC16] Duration block ended at idx {} (invalid signature {}x{} at 0x{:X})",
                         duration_idx, sig_rows, sig_cols, current_offset);
                break;
            }

            let (rows, cols) = (sig_rows, sig_cols);
            // Only the first 6 maps are the selector-driven main Durations
            // (Duration 00-05 on every EDC16, info ZedPerf); the tail maps of
            // the block are dynamic corrections, named like the SOI
            // equivalents ("Start of injection (Dynamic)")
            let name = if duration_idx <= 5 {
                format!("Duration {:02}", duration_idx)
            } else {
                "Duration (Dynamic)".to_string()
            };

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

                // Valid Duration signature: 9-20 rows, 8-15 cols
                if sig_r >= 9 && sig_r <= 20 && sig_c >= 8 && sig_c <= 15 {
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

    /// « Duration min. injection break after main injection » 6x4 (parité U34)
    fn detect_duration_min_break(
        &self,
        data: &[u8],
        win_lo: usize,
        win_hi: usize,
        detected: &HashSet<u32>,
    ) -> Option<DetectedMap> {
        let rd = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
        let mut i = win_lo;
        let hi = win_hi.min(data.len().saturating_sub(4 + 12 + 8 + 48));
        while i < hi {
            if !(data[i] == 0x00 && data[i + 1] == 0x06 && data[i + 2] == 0x00 && data[i + 3] == 0x04)
            {
                i += 2;
                continue;
            }
            let y0 = i + 4;
            let x0 = y0 + 12;
            let d0 = x0 + 8;
            let y: Vec<u16> = (0..6).map(|k| rd(y0 + k * 2)).collect();
            if y[0] < 500
                || y[0] > 2000
                || y.windows(2).any(|w| w[0] >= w[1])
                || !(2500..=4500).contains(&y[5])
            {
                i += 2;
                continue;
            }
            let x: Vec<u16> = (0..4).map(|k| rd(x0 + k * 2)).collect();
            if x[0] > 600 || x.windows(2).any(|w| w[0] >= w[1]) || x[3] > 1500 {
                i += 2;
                continue;
            }
            let vals: Vec<u16> = (0..24).map(|k| rd(d0 + k * 2)).collect();
            if vals.iter().any(|&v| !(200..=3500).contains(&v)) {
                i += 2;
                continue;
            }
            if detected.contains(&(d0 as u32)) {
                i += 2;
                continue;
            }
            let mut map = DetectedMap::new(
                d0 as u32,
                48,
                MapDimensions::TwoDimensional { rows: 6, cols: 4 },
                DataType::Int16,
            );
            map.name = Some("Duration min. injection break after main injection".to_string());
            map.category = Some(MapCategory::InjectionSystem.display_name().to_string());
            map.unit = Some("deg CrS".to_string());
            map.correction_factor = Some(0.023427);
            map.y_axis_address = Some(y0 as u32);
            map.y_label = Some("rpm".to_string());
            map.y_axis_correction = Some(1.0);
            map.x_axis_address = Some(x0 as u32);
            map.x_label = Some("mg/stroke".to_string());
            map.x_axis_correction = Some(0.01);
            map.confidence = 0.9;
            log::debug!("✅ [EDC16U31] Found Duration min. injection break (6x4) at 0x{:X}", d0);
            return Some(map);
        }
        None
    }

    /// Extras de parité Stage X (portés de U34) : FVC 9x8 + EFT 8x8 en
    /// paire, MAF Linearization 1D 32, Dpf switch scalaire — structures
    /// vérifiées présentes sur les U31 réels.
    fn detect_stagex_extras(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let rd = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
        let (scan_lo, scan_hi) = if data.len() >= 0x200000 {
            (0x180000usize, data.len().saturating_sub(0x400))
        } else {
            (0x80000usize, data.len().saturating_sub(0x400))
        };

        // ---- Fuel Volume Correction (9x8) + Expected Fuel Temp (8x8) ----
        let mut i = scan_lo;
        while i < scan_hi {
            if !(data[i] == 0 && data[i + 1] == 0x09 && data[i + 2] == 0 && data[i + 3] == 0x08) {
                i += 2;
                continue;
            }
            let y0 = i + 4;
            let x0 = y0 + 18;
            let d0 = x0 + 16;
            let y: Vec<u16> = (0..9).map(|k| rd(y0 + k * 2)).collect();
            let x: Vec<u16> = (0..8).map(|k| rd(x0 + k * 2)).collect();
            let axes_ok = y[0] <= 1500
                && y.windows(2).all(|w| w[0] < w[1])
                && (2000..=6000).contains(&y[8])
                && x[0] <= 600
                && x.windows(2).all(|w| w[0] < w[1])
                && (2000..=8000).contains(&x[7]);
            if !axes_ok {
                i += 2;
                continue;
            }
            let vals: Vec<u16> = (0..72).map(|k| rd(d0 + k * 2)).collect();
            let rows_start_ok = (0..9).all(|r| vals[r * 8] <= 200);
            let rows_high = (0..9).filter(|r| vals[r * 8 + 7] >= 1000).count();
            let rows_ok =
                rows_start_ok && vals.iter().all(|&v| v <= 12000) && rows_high >= 6;
            if !rows_ok || detected.contains(&(d0 as u32)) {
                i += 2;
                continue;
            }
            let fvc_end = d0 + 144;
            let mut eft: Option<(usize, usize, usize)> = None;
            let mut j = fvc_end;
            while j < (fvc_end + 0x200).min(data.len().saturating_sub(200)) {
                if data[j] == 0 && data[j + 1] == 0x08 && data[j + 2] == 0 && data[j + 3] == 0x08 {
                    let ey0 = j + 4;
                    let ex0 = ey0 + 16;
                    let ed0 = ex0 + 16;
                    let ey: Vec<u16> = (0..8).map(|k| rd(ey0 + k * 2)).collect();
                    let ex: Vec<u16> = (0..8).map(|k| rd(ex0 + k * 2)).collect();
                    let evals: Vec<u16> = (0..64).map(|k| rd(ed0 + k * 2)).collect();
                    if ey[0] <= 1500
                        && ey.windows(2).all(|w| w[0] < w[1])
                        && ex[0] <= 600
                        && ex.windows(2).all(|w| w[0] < w[1])
                        && evals.iter().all(|&v| (2500..=4800).contains(&v))
                        && !detected.contains(&(ed0 as u32))
                    {
                        eft = Some((ey0, ex0, ed0));
                        break;
                    }
                }
                j += 2;
            }
            if let Some((ey0, ex0, ed0)) = eft {
                let mut m = DetectedMap::new(
                    d0 as u32,
                    144,
                    MapDimensions::TwoDimensional { rows: 9, cols: 8 },
                    DataType::Int16,
                );
                m.name = Some("Fuel Volume Correction".to_string());
                m.category = Some("Fuel Correction".to_string());
                m.unit = Some("(mg/stroke)/100°C".to_string());
                m.correction_factor = Some(0.002441);
                m.y_axis_address = Some(y0 as u32);
                m.y_label = Some("rpm".to_string());
                m.y_axis_correction = Some(1.0);
                m.x_axis_address = Some(x0 as u32);
                m.x_label = Some("mg/stroke".to_string());
                m.x_axis_correction = Some(0.01);
                m.confidence = 0.9;
                maps.push(m);

                let mut m2 = DetectedMap::new(
                    ed0 as u32,
                    128,
                    MapDimensions::TwoDimensional { rows: 8, cols: 8 },
                    DataType::Int16,
                );
                m2.name = Some("Expected Fuel Temp".to_string());
                m2.category = Some("Fuel Correction".to_string());
                m2.unit = Some("°C".to_string());
                m2.correction_factor = Some(0.1);
                m2.y_axis_address = Some(ey0 as u32);
                m2.y_label = Some("rpm".to_string());
                m2.y_axis_correction = Some(1.0);
                m2.x_axis_address = Some(ex0 as u32);
                m2.x_label = Some("mg/stroke".to_string());
                m2.x_axis_correction = Some(0.01);
                m2.confidence = 0.9;
                maps.push(m2);
                log::debug!("✅ [EDC16U31] Found Fuel Volume Correction 0x{:X} + Expected Fuel Temp 0x{:X}", d0, ed0);
                i = ed0 + 128;
                continue;
            }
            i += 2;
        }

        // ---- MAF Linearization ----
        let mut maf_count = 0;
        let mut i = scan_lo;
        while i < scan_hi {
            if !(data[i] == 0 && data[i + 1] == 0x20) {
                i += 2;
                continue;
            }
            let x0 = i + 2;
            let d0 = x0 + 64;
            if d0 + 64 > data.len() {
                break;
            }
            let ax: Vec<u16> = (0..32).map(|k| rd(x0 + k * 2)).collect();
            if ax[0] > 400 || ax[31] > 1100 || ax.windows(2).any(|w| w[0] >= w[1]) {
                i += 2;
                continue;
            }
            let vals: Vec<i16> = (0..32).map(|k| rd(d0 + k * 2) as i16).collect();
            if !((-2500..=0).contains(&vals[0])
                && vals.windows(2).all(|w| w[0] < w[1])
                && vals[31] >= 5000)
            {
                i += 2;
                continue;
            }
            if detected.contains(&(d0 as u32)) {
                i += 2;
                continue;
            }
            maf_count += 1;
            let mut m = DetectedMap::new(
                d0 as u32,
                64,
                MapDimensions::OneDimensional { length: 32 },
                DataType::Int16,
            );
            m.name = Some(if maf_count == 1 {
                "MAF Linearization".to_string()
            } else {
                format!("MAF Linearization {}", maf_count)
            });
            m.category = Some("Airflow".to_string());
            m.unit = Some("kg/h".to_string());
            m.correction_factor = Some(0.1);
            m.x_axis_address = Some(x0 as u32);
            m.x_label = Some("mV".to_string());
            m.x_axis_correction = Some(4.887586);
            m.confidence = 0.9;
            log::debug!("✅ [EDC16U31] Found MAF Linearization at 0x{:X}", d0);
            maps.push(m);
            i = d0 + 64;
        }

        // ---- Dpf switch ----
        let mut i = scan_lo;
        while i + 20 < scan_hi {
            if rd(i) == 0x7FFF
                && rd(i + 2) == 0x8000
                && rd(i + 4) == 0x0000
                && rd(i + 6) == 0x028F
                && rd(i + 10) == 0
                && rd(i + 12) == 0
                && rd(i + 14) == 0x001E
                && rd(i + 16) == 0xFFFF
            {
                let addr = (i + 8) as u32;
                if !detected.contains(&addr) && !maps.iter().any(|m| m.address == addr) {
                    let mut m = DetectedMap::new(
                        addr,
                        2,
                        MapDimensions::OneDimensional { length: 1 },
                        DataType::UInt16,
                    );
                    m.name = Some("Dpf switch".to_string());
                    m.category = Some("DPF".to_string());
                    m.unit = Some("-".to_string());
                    m.correction_factor = Some(1.0);
                    m.confidence = 0.9;
                    log::debug!("✅ [EDC16U31] Found Dpf switch at 0x{:X}", addr);
                    maps.push(m);
                    break;
                }
            }
            i += 2;
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

        // Duration RPM: starts from 0-2000 (dynamic Duration maps of the tail
        // block start their RPM axis at exactly 0, e.g. 03G906016JA at 0x1E6DA6)
        if first > 2000 {
            return false;
        }

        // End should be at least 2000 and at most 8000
        // Some variants have low RPM range (2500), others go up to 6500
        if last < 2000 || last > 8000 {
            return false;
        }

        // Should be monotonically increasing
        for i in 1..values.len() {
            if values[i] < values[i - 1] {
                return false;
            }
        }

        true
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
        // Must end in range 2000-10000 (20-100 mg/stroke). The lower bound is
        // 2000 (not 3000) because the last two low-IQ Duration maps of some
        // firmwares (e.g. abcABC at 0x1DFCD8/0x1DFDCC) top out at exactly 20
        // mg/stroke. Safe because these maps are only accepted inside the
        // contiguous Duration block anchored by the Duration Selector.
        if first > 2000 {
            return false;
        }
        if last < 2000 || last > 10000 {
            return false;
        }

        // Should be monotonically increasing
        for i in 1..values.len() {
            if values[i] < values[i - 1] {
                return false;
            }
        }

        true
    }

    // ============== SIGNATURE-BASED DETECTION ==============
    // These methods detect maps using byte sequences that are IDENTICAL across all files
    // This approach is much more reliable than heuristic detection for specific map types

    /// Detect SVBL (Boost Limiter Scalar) using combined signature
    /// SVBL is a single 16-bit Big-Endian value representing max boost pressure in mbar
    /// Typical values: 2350 mbar (1.9L) or 2620 mbar (2.0L), up to 3200+ mbar for tuned files
    ///
    /// Structure (values are CALIBRATION-dependent, only the low-boost curve
    /// core is constant across firmwares):
    /// - core: 06 06 06 40 06 61 06 B8 07 3A (5 low-boost points, constant)
    /// - then 4-8 repeats of the max-boost value (08 CA on 1.9/2.0L,
    ///   09 5A on the Crafter — hard-coding these broke Crafter detection)
    /// - then the SVBL scalar (may EQUAL the repeated value, Crafter does)
    /// - then 00 00 00
    pub fn detect_svbl_by_signature(&self, data: &[u8]) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        let read = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);

        let (scan_start, scan_end) = self.get_data_section_range(data.len());
        let scan_end = scan_end.min(data.len());

        // Signature STRUCTURELLE (indépendante de la calibration) : le SVBL
        // termine la table de limitation de boost, composée de lignes de
        // 20 octets répétées à l'identique (courbe par palier). Motif :
        //   [ligne 20 o] ×N (N ≥ 3, identiques)
        //   [début de ligne 12 o identique][SVBL u16][00 00][00 0A][00 0A]
        // Vérifié sur Passat (2620 @0x1DDBC8), JA (2350) et KN (2620) —
        // l'ancien noyau bas-boost codé en dur (06 06 06 40 …) ne couvrait
        // que certaines calibrations 1.9/2.0L et le Crafter.
        for offset in scan_start..scan_end.saturating_sub(8) {
            let value = read(offset);
            if !(1500..=6200).contains(&value) {
                continue;
            }
            // Queue : 00 00 | 00 0A | 00 0A
            if read(offset + 2) != 0 || read(offset + 4) != 10 || read(offset + 6) != 10 {
                continue;
            }
            // La ligne de 20 octets qui précède le SVBL doit se répéter au
            // moins 3 fois à l'identique (motif de table très discriminant)
            if offset < 80 {
                continue;
            }
            let line = &data[offset - 20..offset];
            if &data[offset - 40..offset - 20] != line || &data[offset - 60..offset - 40] != line {
                continue;
            }
            let svbl_offset = offset;
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

        // Second passage : signature HISTORIQUE à noyau bas-boost codé en dur
        // — certaines calibrations (Suberb, u31full) n'ont pas la queue
        // 00/0A/0A de la signature structurelle, mais partagent ce noyau.
        let core: [u8; 10] = [0x06, 0x06, 0x06, 0x40, 0x06, 0x61, 0x06, 0xB8, 0x07, 0x3A];
        for offset in scan_start..scan_end.saturating_sub(core.len() + 24) {
            if data[offset..offset + core.len()] != core {
                continue;
            }
            let run_start = offset + core.len();
            let v = read(run_start);
            if !(1500..=6200).contains(&v) {
                continue;
            }
            let mut t = 1usize;
            while t < 10 && run_start + (t + 1) * 2 <= data.len() && read(run_start + t * 2) == v {
                t += 1;
            }
            if !(4..=9).contains(&t) {
                continue;
            }
            let after = run_start + t * 2;
            if after + 5 > data.len() {
                continue;
            }
            let candidate = read(after);
            let svbl_offset = if candidate > 0 && candidate <= 6200
                && data[after + 2..after + 5] == [0, 0, 0]
            {
                after
            } else if candidate == 0 && t >= 5 && data[after..after + 3] == [0, 0, 0] {
                run_start + (t - 1) * 2
            } else {
                continue;
            };
            if maps.iter().any(|m| m.address == svbl_offset as u32) {
                continue;
            }
            let value = read(svbl_offset);
            log::debug!("🎯 [EDC16] Found SVBL by legacy core at 0x{:X}, value: {} mbar", svbl_offset, value);

            let mut map = DetectedMap::new(
                svbl_offset as u32,
                2,
                MapDimensions::OneDimensional { length: 1 },
                DataType::Int16,
            );
            map.name = Some("SVBL - Max Boost Pressure".to_string());
            map.category = Some(MapCategory::TurboBoostPressure.display_name().to_string());
            map.unit = Some("mbar".to_string());
            map.correction_factor = Some(1.0);
            map.offset = Some(0.0);
            map.confidence = 1.0;
            maps.push(map);
        }

        log::debug!("🔧 [EDC16] SVBL detection: found {} maps", maps.len());
        maps
    }

    /// Detect the injector BIP correction block — three consecutive
    /// structures (crafter reference: data at 0x1CD48A / 0x1CD4AC / 0x1CD5BA):
    ///  1. [00 06][6 fuel temps K×10, increasing][6 factors ~4096]
    ///     → "BIP Fuel Temp Correction" (1D×6)
    ///  2. [00 0A][10 rail pressures bar, increasing][10 BIP times, overall decreasing]
    ///     → "BIP Basic Characteristic" (1D×10)
    ///  3. [00 0A][10 raw axis, increasing][10 signed axis, increasing][100 factors ~4096]
    ///     → "BIP Multiple Correction" (10x10)
    /// Nothing is emitted unless all three parts line up (within 0x40 /
    /// 0x200 windows) — the chained triple keeps false positives out.
    pub fn detect_bip_maps(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let read = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
        let read_i = |o: usize| i16::from_be_bytes([data[o], data[o + 1]]);
        let (scan_start, scan_end) = self.get_data_section_range(data.len());
        let scan_end = scan_end.min(data.len());

        let mut offset = scan_start;
        while offset + 26 < scan_end {
            // ---- Part 1: [00 06] + 6 fuel temps + 6 factors ----
            if !(data[offset] == 0x00 && data[offset + 1] == 0x06) {
                offset += 2;
                continue;
            }
            let temps: Vec<u16> = (0..6).map(|i| read(offset + 2 + i * 2)).collect();
            if !(2000..=3000).contains(&temps[0])
                || !(2500..=4000).contains(&temps[5])
                || !temps.windows(2).all(|w| w[0] < w[1])
            {
                offset += 2;
                continue;
            }
            let d1 = offset + 14;
            if d1 + 12 > scan_end {
                offset += 2;
                continue;
            }
            let f1: Vec<u16> = (0..6).map(|i| read(d1 + i * 2)).collect();
            if !f1.iter().all(|&v| (3000..=5500).contains(&v)) {
                offset += 2;
                continue;
            }
            let p1_end = d1 + 12;

            // ---- Part 2 within 0x40: [00 0A] + 10 pressures + 10 times ----
            let mut part2 = None;
            let mut p = p1_end;
            while p + 42 <= scan_end && p < p1_end + 0x40 {
                if data[p] == 0x00 && data[p + 1] == 0x0A {
                    let press: Vec<u16> = (0..10).map(|i| read(p + 2 + i * 2)).collect();
                    let times_start = p + 22;
                    let times: Vec<u16> = (0..10).map(|i| read(times_start + i * 2)).collect();
                    if (100..=800).contains(&press[0])
                        && (600..=2500).contains(&press[9])
                        && press.windows(2).all(|w| w[0] < w[1])
                        && times.iter().all(|&v| (200..=4000).contains(&v))
                        && times[0] > times[9]
                    {
                        part2 = Some((p + 2, times_start));
                        break;
                    }
                }
                p += 2;
            }
            let Some((press_axis, d2)) = part2 else { offset += 2; continue; };
            let p2_end = d2 + 20;

            // ---- Part 3 within 0x200: [00 0A] + axis + signed axis + 100 factors ----
            let mut part3 = None;
            let mut q = p2_end;
            while q + 242 <= scan_end && q < p2_end + 0x200 {
                if data[q] == 0x00 && data[q + 1] == 0x0A {
                    let ax1: Vec<u16> = (0..10).map(|i| read(q + 2 + i * 2)).collect();
                    let ax2: Vec<i16> = (0..10).map(|i| read_i(q + 22 + i * 2)).collect();
                    let d3 = q + 42;
                    if ax1[0] <= 1000
                        && ax1.windows(2).all(|w| w[0] < w[1])
                        && ax2.windows(2).all(|w| w[0] < w[1])
                        && (0..100).all(|i| (2000..=6500).contains(&read(d3 + i * 2)))
                    {
                        part3 = Some((q + 2, q + 22, d3));
                        break;
                    }
                }
                q += 2;
            }
            let Some((ax1_start, ax2_start, d3)) = part3 else { offset += 2; continue; };

            log::debug!("🎯 [EDC16] Found BIP block: temp corr 0x{:X}, basic 0x{:X}, multiple 0x{:X}",
                d1, d2, d3);

            if !detected.contains(&(d1 as u32)) {
                let mut map = DetectedMap::new(
                    d1 as u32,
                    12,
                    MapDimensions::OneDimensional { length: 6 },
                    DataType::UInt16,
                );
                map.name = Some("BIP Fuel Temp Correction".to_string());
                map.category = Some(MapCategory::InjectionSystem.display_name().to_string());
                map.unit = Some("-".to_string());
                map.correction_factor = Some(0.000244140625); // 4096 = 1.0
                map.confidence = 0.88;
                map.x_axis_address = Some((offset + 2) as u32);
                map.x_label = Some("°C".to_string());
                map.x_axis_correction = Some(0.1);
                map.x_axis_offset = Some(-273.14);
                maps.push(map);
            }
            if !detected.contains(&(d2 as u32)) {
                let mut map = DetectedMap::new(
                    d2 as u32,
                    20,
                    MapDimensions::OneDimensional { length: 10 },
                    DataType::UInt16,
                );
                map.name = Some("BIP Basic Characteristic".to_string());
                map.category = Some(MapCategory::InjectionSystem.display_name().to_string());
                map.unit = Some("µs".to_string());
                map.correction_factor = Some(1.0);
                map.confidence = 0.88;
                map.x_axis_address = Some(press_axis as u32);
                map.x_label = Some("bar".to_string());
                map.x_axis_correction = Some(1.0);
                maps.push(map);
            }
            if !detected.contains(&(d3 as u32)) {
                let mut map = DetectedMap::new(
                    d3 as u32,
                    200,
                    MapDimensions::TwoDimensional { rows: 10, cols: 10 },
                    DataType::UInt16,
                );
                map.name = Some("BIP Multiple Correction".to_string());
                map.category = Some(MapCategory::InjectionSystem.display_name().to_string());
                map.unit = Some("-".to_string());
                map.correction_factor = Some(0.000244140625); // 4096 = 1.0
                map.confidence = 0.88;
                map.y_axis_address = Some(ax1_start as u32);
                map.y_label = Some("raw".to_string());
                map.y_axis_correction = Some(1.0);
                map.x_axis_address = Some(ax2_start as u32);
                map.x_label = Some("raw".to_string());
                map.x_axis_correction = Some(1.0);
                maps.push(map);
            }
            offset = d3 + 200;
        }

        log::debug!("🔧 [EDC16] BIP detection: found {} maps", maps.len());
        maps
    }

    /// Detect SVRL (RPM Limiter Scalar) using signature
    /// SVRL is a single 16-bit Big-Endian value representing max engine RPM
    /// Typical values: 4500-5500 RPM (validation range: 0-6500 RPM)
    ///
    /// Signature: Pattern "05 00 04 00 03 00 02 00 01 00 00 00" appears at SVRL + 16 bytes
    /// This pattern is a descending sequence (5,4,3,2,1,0) that appears in ALL EDC16U34 files
    pub fn detect_svrl_by_signature(&self, data: &[u8]) -> Vec<DetectedMap> {
        let mut maps = Vec::new();

        // The signature pattern that appears AFTER SVRL (+16 bytes)
        // This is a descending value sequence: 05 00 04 00 03 00 02 00 01 00 00 00
        let signature: [u8; 12] = [
            0x05, 0x00, 0x04, 0x00, 0x03, 0x00,
            0x02, 0x00, 0x01, 0x00, 0x00, 0x00
        ];

        let (scan_start, scan_end) = self.get_data_section_range(data.len());

        // Search for the signature
        for offset in scan_start..scan_end.saturating_sub(signature.len()) {
            if data[offset..offset + signature.len()] == signature {
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
    /// EDC16U34: Based on StageX patterns analysis (lectureG5.bin)
    /// StageX addresses: 1EFFB4, 1EFFBE, 1EFFC0, 1F33CA, 1F33D2, 1F3410
    /// Factor 0.01: raw value 25000 = 250 km/h displayed
    /// Detect small/isolated calibration maps in the 0x1D region by
    /// HEADER + AXES only (structure never validated against data, so modified
    /// files still match). Each shape is unique within its zone on the whole
    /// test corpus:
    ///  - Friction torque (8x9)                     header [00 09 00 08]
    ///  - Fuel correction factor by temp & speed (3x3) header [00 03 00 03]
    ///  - Torque limiter (2x2)                      header [00 02 00 02]
    ///  - Start of Injection (13x14) — the two extra SOI maps that live just
    ///    after the main SOI block                  header [00 0E 00 0D]
    pub fn detect_region1d_header_maps(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let read = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
        let strictly_inc = |v: &[u16]| v.windows(2).all(|w| w[0] < w[1]);
        let non_dec = |v: &[u16]| v.windows(2).all(|w| w[0] <= w[1]);

        // U31: this family sits lower than on U34 (friction/fuelcorr/torquelim
        // around 0x1CFxxx on the Passat reference) — start the scan at 0x1C8000.
        let mut pos = 0x1C8000usize;
        while pos + 4 < data.len().min(0x1E0000) {
            let rows = read(pos) as usize;
            let cols = read(pos + 2) as usize;

            // (rows, cols, kind) — only the exact shapes we want
            // Fuel correction factor comes as 3x3 (U34) or 4x4 (U31)
            let kind = match (rows, cols) {
                (9, 8) => Some("friction"),
                (3, 3) | (4, 4) => Some("fuelcorr"),
                (2, 2) => Some("torquelim"),
                (14, 13) => Some("soi1314"),
                _ => None,
            };
            let Some(kind) = kind else { pos += 2; continue; };

            let y_start = pos + 4;
            let x_start = y_start + rows * 2;
            let data_start = x_start + cols * 2;
            let data_end = data_start + rows * cols * 2;
            if data_end > data.len() { pos += 2; continue; }

            let y: Vec<u16> = (0..rows).map(|i| read(y_start + i * 2)).collect();
            let x: Vec<u16> = (0..cols).map(|i| read(x_start + i * 2)).collect();

            // Per-shape axis validation (structure only), then per-shape fields.
            // Each family replicates EXACTLY the fields of its existing siblings
            // (same name, category, unit, corrections) so it lands in the same
            // folder with the same display characteristics.
            let mut map = DetectedMap::new(
                data_start as u32,
                rows * cols * 2,
                MapDimensions::TwoDimensional { rows, cols },
                DataType::Int16,
            );
            map.confidence = 0.9;
            map.y_axis_address = Some(y_start as u32);
            map.x_axis_address = Some(x_start as u32);

            match kind {
                "friction" => {
                    // Y RPM 600-1200 .. 4000-5000 strictly inc, X inc starting >= 2000
                    if !(600..=1200).contains(&y[0]) || !(4000..=5000).contains(&y[rows - 1])
                        || !strictly_inc(&y) || x[0] < 2000 || !strictly_inc(&x) {
                        pos += 2; continue;
                    }
                    // Data is signed negative (friction losses), Nm × 10
                    map.name = Some("Friction torque".to_string());
                    map.category = Some(MapCategory::EngineTorqueRequest.display_name().to_string());
                    map.unit = Some("Nm".to_string());
                    map.correction_factor = Some(0.1);
                    map.y_label = Some("rpm".to_string());
                    map.y_axis_correction = Some(1.0);
                    // X axis is coolant/oil temperature (raw 2531-3731 = -20..100 °C)
                    map.x_label = Some("°C".to_string());
                    map.x_axis_correction = Some(0.1);
                    map.x_axis_offset = Some(-273.14);
                }
                "fuelcorr" => {
                    // Y 3800-4100 inc, X strictly inc (temperature/speed factors)
                    if !(3800..=4100).contains(&y[0]) || !strictly_inc(&y) || !strictly_inc(&x) {
                        pos += 2; continue;
                    }
                    // Same family as the other fuel corrections: factor centered
                    // on 8192 = 1.0 (× 1/8192)
                    map.data_type = DataType::UInt16;
                    map.name = Some("Fuel correction factor by engine temp and vehicle speed".to_string());
                    map.category = Some(MapCategory::InjectionSystem.display_name().to_string());
                    map.unit = Some("-".to_string());
                    map.correction_factor = Some(0.000122070313);
                    map.y_label = Some("°C".to_string());
                    map.y_axis_correction = Some(0.1);
                    map.y_axis_offset = Some(-273.14);
                    // X axis is vehicle speed (raw 4000/6000/8000 = 40/60/80 km/h)
                    map.x_label = Some("km/h".to_string());
                    map.x_axis_correction = Some(0.01);
                }
                "torquelim" => {
                    // Very specific 2x2 torque limiter axes: Y ~[500, 10000],
                    // X ~[0, 6000] RPM. Y[0] must NOT be 0 (that rules out the many
                    // zeroed 2x2 structures scattered in the 0x1D region).
                    if !(200..=800).contains(&y[0]) || !(8000..=12000).contains(&y[1])
                        || x[0] != 0 || !(5000..=7000).contains(&x[1]) {
                        pos += 2; continue;
                    }
                    // Same fields as the existing "Torque Limiter" family (Nm × 10)
                    map.name = Some("Torque Limiter".to_string());
                    map.category = Some(MapCategory::EngineTorqueLimiters.display_name().to_string());
                    map.unit = Some("Nm".to_string());
                    map.correction_factor = Some(0.1);
                    map.y_label = Some("mbar".to_string());
                    map.y_axis_correction = Some(1.0);
                    map.x_label = Some("rpm".to_string());
                    map.x_axis_correction = Some(1.0);
                }
                "soi1314" => {
                    // Y RPM, X IQ — same validation as the main SOI maps
                    if y[0] > 1400 || !(2800..=6000).contains(&y[rows - 1]) || !strictly_inc(&y)
                        || x[0] > 700 || !(2500..=5500).contains(&x[cols - 1]) || !non_dec(&x) {
                        pos += 2; continue;
                    }
                    // Same type as the SOI Dynamic maps (verified: same 0.023437
                    // factor, same 0-27 deg range) — replicate their exact fields.
                    // Named "(Dynamic)" directly (final display name): the numbered
                    // "Start of injection NN" retain-filter must not drop them, and
                    // the "Start of injection Dynamic" block-proximity filter only
                    // applies to the in-block spelling without parentheses.
                    map.name = Some("Start of injection (Dynamic)".to_string());
                    map.category = Some(MapCategory::StartOfInjection.display_name().to_string());
                    map.unit = Some("deg CrS".to_string());
                    map.correction_factor = Some(0.023437);
                    map.confidence = 0.88;
                    map.y_label = Some("rpm".to_string());
                    map.y_axis_correction = Some(1.0);
                    map.y_axis_inverted = Some(true);
                    map.x_label = Some("mg/stroke".to_string());
                    map.x_axis_correction = Some(0.01);
                }
                _ => { pos += 2; continue; }
            }

            if detected.contains(&(data_start as u32)) { pos = data_end; continue; }

            log::debug!("🎯 [EDC16] {} at 0x{:X} ({}x{})",
                map.name.as_deref().unwrap_or("?"), data_start, cols, rows);
            maps.push(map);
            pos = data_end;
        }

        maps
    }

    /// Detect the remaining turbo-region 2D maps by HEADER + AXES only.
    ///
    /// U31 layout (reference VW_Passat_2_0TDI_1037382052): the turbo region
    /// sits LOWER than on U34 — 0x1DCxxx-0x1E0xxx instead of 0x1E3xxx-0x1E6xxx:
    ///  - Variable geometry / Wastegate duty cycle control (10x12) — 0x1DCCxx-0x1DD1xx
    ///  - Turbo boost pressure request (11x12)                     — 0x1DDDxx-0x1DDFxx
    ///  - Inversed drivers wish (16x16, RPM starts at 0, X=torque) — 0x1DFExx
    ///
    /// All share the layout [rows u16 BE][cols u16 BE][Y=rows RPM][X=cols IQ][data].
    /// Validation is STRUCTURE-ONLY (axes strictly increasing, plausible RPM/IQ
    /// ranges) — the data magnitude is never checked, so tuned files still match.
    /// The RPM lower bound (400) rejects the padded conversion table at 0x1E009E
    /// (Y starts at 70 with a 5355..5359 padding tail) without touching real maps.
    pub fn detect_turbo_region_maps(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut vgt_count = 0;
        let mut boost_count = 0;

        let rpm_ok = |y: &[u16]| -> bool {
            y.len() >= 10
                && (400..=1400).contains(&y[0])
                && (2800..=6000).contains(&y[y.len() - 1])
                && y.windows(2).all(|w| w[0] < w[1])
        };
        // Inversed drivers wish: RPM axis starts at exactly 0 and reaches high rev
        let rpm_ok_zero = |y: &[u16]| -> bool {
            y.len() >= 10
                && y[0] == 0
                && (4000..=6000).contains(&y[y.len() - 1])
                && y.windows(2).all(|w| w[0] < w[1])
        };
        let iq_ok = |x: &[u16]| -> bool {
            x.len() >= 10
                && x[0] <= 700
                && (2500..=6000).contains(&x[x.len() - 1])
                && x.windows(2).all(|w| w[0] <= w[1])
                && x.windows(2).any(|w| w[0] < w[1])
        };
        // Inversed drivers wish X axis is engine torque (Nm × 10), ends lower
        // than an IQ axis (Passat U31: 0..1800, U34-style files: 0..3600)
        let torque_ok = |x: &[u16]| -> bool {
            x.len() >= 10
                && x[0] == 0
                && (1500..=3800).contains(&x[x.len() - 1])
                && x.windows(2).all(|w| w[0] < w[1])
        };

        // Window extended down to 0x1D8000: U31 layouts park part of the
        // boost-request family (10x10 / 12x11 / 12x12) around 0x1D8Cxx-0x1D98xx
        // (Passat), matching the U34's 0x1E4Bxx-0x1E4Fxx block. 0x1D7xxx stays
        // excluded: the 12x12 family there is not a boost request (the U34
        // does not detect it either).
        let mut pos = 0x1D8000;
        while pos + 4 < data.len().min(0x1E8000) {
            let rows = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;
            let cols = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            if !(10..=16).contains(&rows) || !(10..=16).contains(&cols) {
                pos += 2;
                continue;
            }
            // 14x13 is the Start-of-injection (Dynamic) shape — handled by
            // detect_region1d_header_maps with proper SOI fields. On hybrid
            // layouts (e.g. 03G906016JA) those SOI maps sit inside this turbo
            // window and must not be claimed as boost requests here.
            if rows == 14 && cols == 13 {
                pos += 2;
                continue;
            }

            let y_start = pos + 4;
            let x_start = y_start + rows * 2;
            let data_start = x_start + cols * 2;
            let data_end = data_start + rows * cols * 2;
            if data_end > data.len() {
                pos += 2;
                continue;
            }

            let y: Vec<u16> = (0..rows)
                .map(|i| u16::from_be_bytes([data[y_start + i * 2], data[y_start + i * 2 + 1]]))
                .collect();
            let x: Vec<u16> = (0..cols)
                .map(|i| u16::from_be_bytes([data[x_start + i * 2], data[x_start + i * 2 + 1]]))
                .collect();

            // Two accepted profiles:
            //  - VGT/Boost: RPM axis 400-1400 -> 2800-6000, X = IQ axis
            //  - Inversed DW: RPM axis 0 -> 4000-6000, X = torque axis, 16 cols
            let starts_zero = rpm_ok_zero(&y);
            let is_inv_dw = starts_zero
                && cols == 16
                && (rows == 12 || rows == 16)
                && torque_ok(&x);
            let is_turbo = rpm_ok(&y) && iq_ok(&x);
            if !(is_inv_dw || is_turbo) {
                pos += 2;
                continue;
            }

            // Loose anti-garbage bound only
            let mut garbage = false;
            for i in (data_start..data_end).step_by(2) {
                if u16::from_be_bytes([data[i], data[i + 1]]) > 40000 {
                    garbage = true;
                    break;
                }
            }
            if garbage {
                pos += 2;
                continue;
            }

            if detected.contains(&(data_start as u32)) {
                pos = data_end;
                continue;
            }

            // Inversed drivers wish is intentionally NOT reported (not a
            // tuning map — hidden per user request). It is still recognized
            // here so the whole region is skipped and never misclassified
            // as a boost request.
            if is_inv_dw {
                pos = data_end;
                continue;
            }

            // Classify by shape + position, using EXACTLY the same names,
            // categories and characteristics as the already-existing families
            // so they land in the same folders with consistent display.
            let mut map = DetectedMap::new(
                data_start as u32,
                rows * cols * 2,
                MapDimensions::TwoDimensional { rows, cols },
                DataType::UInt16,
            );
            map.y_axis_address = Some(y_start as u32);
            map.y_label = Some("rpm".to_string());
            map.y_axis_correction = Some(1.0);
            map.x_axis_address = Some(x_start as u32);
            map.confidence = 0.9;

            // Only the bench-validated VGT/Wastegate family is reported from
            // this net (10 columns x 12/14 rows, Passat 0x1DCCxx). The
            // "Turbo boost pressure request" tables it used to emit have no
            // Stage X / EDCMasterHP counterpart (false positives).
            if !(cols == 10 && (rows == 12 || rows == 14)) {
                let _ = boost_count;
                pos = data_end;
                continue;
            }
            vgt_count += 1;
            let _ = vgt_count;
            map.data_type = DataType::Int16;
            map.name = Some("N75 Duty Cycle".to_string());
            map.category = Some(MapCategory::TurboBoostPressureControl.display_name().to_string());
            map.unit = Some("%".to_string());
            map.correction_factor = Some(0.012207);
            map.x_label = Some("mg/stroke".to_string());
            map.x_axis_correction = Some(0.01);

            log::debug!("🎯 [EDC16] {} at 0x{:X} ({}x{})",
                map.name.as_deref().unwrap_or("?"), data_start, cols, rows);
            maps.push(map);

            pos = data_end;
        }

        maps
    }

    pub fn detect_maximum_vehicle_speed(&self, data: &[u8], detected_addresses: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let mut found: HashSet<u32> = HashSet::new();
        let (_scan_start, scan_end) = self.get_data_section_range(data.len());

        // Helper to create vmax map
        let create_vmax_map = |addr: u32, confidence: f32| -> DetectedMap {
            let mut m = DetectedMap::new(addr, 2, MapDimensions::OneDimensional { length: 1 }, DataType::Int16);
            m.name = Some("Maximum Vehicle Speed".to_string());
            m.category = Some("Maximum Vehicle Speed".to_string());
            m.unit = Some("km/h".to_string());
            m.correction_factor = Some(0.01);
            m.confidence = confidence;
            m
        };

        // ===== ZONE 1: 0x1EFxxx - Pattern [01 F4 1B 58] =====
        // Structure: ... 01 F4 1B 58 | [VMAX1] 01 F4 01 F4 00 0A 01 F4 | [VMAX2] [VMAX3] ...
        // StageX: 1EFFB4 (61A8=250), 1EFFBE (61A8=250), 1EFFC0 (7530=300)
        // Pattern: [01 F4 1B 58] at offset-4 before first vmax
        let pattern1: [u8; 4] = [0x01, 0xF4, 0x1B, 0x58];
        for offset in 0x1E0000..scan_end.min(0x200000).saturating_sub(30) {
            if offset + 4 <= data.len() && data[offset..offset + 4] == pattern1 {
                // First vmax at pattern + 4
                let addr1 = offset + 4;
                if addr1 + 2 <= data.len() {
                    let v = ((data[addr1] as u16) << 8) | (data[addr1 + 1] as u16);
                    if v >= 15000 && v <= 35000 {
                        let a32 = addr1 as u32;
                        if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                            found.insert(a32);
                            maps.push(create_vmax_map(a32, 0.95));
                        }
                    }
                }
                // Second vmax at pattern + 14 (after 01 F4 01 F4 00 0A 01 F4)
                let addr2 = offset + 14;
                if addr2 + 2 <= data.len() {
                    let v = ((data[addr2] as u16) << 8) | (data[addr2 + 1] as u16);
                    if v >= 15000 && v <= 35000 {
                        let a32 = addr2 as u32;
                        if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                            found.insert(a32);
                            maps.push(create_vmax_map(a32, 0.95));
                        }
                    }
                }
                // Third vmax at pattern + 16
                let addr3 = offset + 16;
                if addr3 + 2 <= data.len() {
                    let v = ((data[addr3] as u16) << 8) | (data[addr3 + 1] as u16);
                    if v >= 15000 && v <= 35000 {
                        let a32 = addr3 as u32;
                        if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                            found.insert(a32);
                            maps.push(create_vmax_map(a32, 0.95));
                        }
                    }
                }
            }
        }

        // ===== ZONE 2: Pattern [03 E8 00 00 00 00] + strict inner check =====
        // Structure: ... 03 E8 00 00 00 00 | [VMAX1] 00 00 2F FD 01 F4 | [VMAX2] ...
        // Found in two distinct address ranges depending on the firmware
        // variant: late variants put it around 0x1F33xx (StageX 1F33CA, 1F33D2),
        // earlier variants (e.g. ZZZori, VWJetta1.9, VWPassat2l) put it around
        // 0x1EA7xx.
        //
        // IMPORTANT: the inner bytes [00 00 2F FD 01 F4] between VMAX1 and VMAX2
        // MUST be verified — without this check the 6-byte prefix alone matched
        // random data (e.g. false positives at 0x1E23A6/0x1E2478 on abcABC,
        // which Stage X does not list). With the strict check, exactly ONE match
        // per file on the whole 33-file test corpus, always the real location.
        // VMAX values themselves are NOT range-checked (modified files can hold
        // any value after a vmax-off), only a loose >= 5000 sanity bound.
        let pattern2: [u8; 6] = [0x03, 0xE8, 0x00, 0x00, 0x00, 0x00];
        let pattern2_inner: [u8; 6] = [0x00, 0x00, 0x2F, 0xFD, 0x01, 0xF4];
        for offset in 0x1E0000..scan_end.min(0x200000).saturating_sub(20) {
            if offset + 16 <= data.len()
                && data[offset..offset + 6] == pattern2
                && data[offset + 8..offset + 14] == pattern2_inner
            {
                // First vmax at pattern + 6
                let addr1 = offset + 6;
                let v1 = ((data[addr1] as u16) << 8) | (data[addr1 + 1] as u16);
                if v1 >= 5000 {
                    let a32 = addr1 as u32;
                    if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                        found.insert(a32);
                        maps.push(create_vmax_map(a32, 0.97));  // ZONE2 MARKER
                    }
                }
                // Second vmax at pattern + 14 (after 00 00 2F FD 01 F4)
                let addr2 = offset + 14;
                let v2 = ((data[addr2] as u16) << 8) | (data[addr2 + 1] as u16);
                if v2 >= 5000 {
                    let a32 = addr2 as u32;
                    if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                        found.insert(a32);
                        maps.push(create_vmax_map(a32, 0.97));  // ZONE2 MARKER
                    }
                }
            }
        }

        // ===== ZONE 4: 0x1E74xx trio - Pattern [01 F4 7F FF] ... [00 00 FF FE FF FF] =====
        // Structure: [01 F4][7F FF][VMAX1][VMAX2][00 00][FF FE][FF FF][VMAX3]
        // StageX (abcABC/ZZZori firmware): 1E7414, 1E7416, 1E741E
        //         (VWPassat2l variant):     1E74C4, 1E74C6, 1E74CE
        // The 10 fixed bytes around the values make this signature unique
        // (exactly one match per file where present on the 33-file corpus, zero
        // false positives elsewhere). Values are not range-checked so vmax-off
        // modified files are still detected.
        let trio_prefix: [u8; 4] = [0x01, 0xF4, 0x7F, 0xFF];
        let trio_inner: [u8; 6] = [0x00, 0x00, 0xFF, 0xFE, 0xFF, 0xFF];
        for offset in 0x1E0000..scan_end.min(0x200000).saturating_sub(20) {
            if offset + 16 <= data.len()
                && data[offset..offset + 4] == trio_prefix
                && data[offset + 8..offset + 14] == trio_inner
            {
                for &addr in &[offset + 4, offset + 6, offset + 14] {
                    let v = ((data[addr] as u16) << 8) | (data[addr + 1] as u16);
                    if v >= 5000 {
                        let a32 = addr as u32;
                        if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                            found.insert(a32);
                            maps.push(create_vmax_map(a32, 0.95));
                        }
                    }
                }
            }
        }

        // ===== ZONE 4b (U31): duo - [7F FF][01 F4][7F FF][VMAX1][7F FF][00 x6][VMAX2] =====
        // U31 variant of the trio: the inner bytes between the two values are
        // all zero and the middle slot holds 7F FF (disabled placeholder, not a
        // speed — Stage X does not list it). Passat reference: VMAX at 1E0432
        // (0x7530 = 300 km/h) and 1E043C. The 12 fixed bytes make the signature
        // unique; values only get the loose >= 5000 sanity bound so vmax-off
        // modified files still match.
        let duo_sig_a: [u8; 6] = [0x7F, 0xFF, 0x01, 0xF4, 0x7F, 0xFF];
        let duo_mid: [u8; 2] = [0x7F, 0xFF];
        let duo_zeros: [u8; 6] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        for offset in 0x1C0000..scan_end.min(0x200000).saturating_sub(20) {
            if offset + 18 <= data.len()
                && data[offset..offset + 6] == duo_sig_a
                && data[offset + 8..offset + 10] == duo_mid
                && data[offset + 10..offset + 16] == duo_zeros
            {
                for &addr in &[offset + 6, offset + 16] {
                    let v = ((data[addr] as u16) << 8) | (data[addr + 1] as u16);
                    if v >= 5000 && v != 0x7FFF {
                        let a32 = addr as u32;
                        if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                            found.insert(a32);
                            maps.push(create_vmax_map(a32, 0.95));
                        }
                    }
                }
            }
        }

        // ===== ZONE 5 (U31): single value before fixed suffix =====
        // Passat reference 0x1CA0B8 (0x61A8 = 250 km/h) followed by the 14-byte
        // sequence [7F FF 00 FA 00 FA 03 84 01 F4 01 F4 01 F4]. The suffix is
        // pure calibration structure (placeholders + hysteresis constants), so
        // it survives tuning; the value itself is never range-restricted beyond
        // the >= 5000 sanity bound.
        let single_suffix: [u8; 14] = [
            0x7F, 0xFF, 0x00, 0xFA, 0x00, 0xFA, 0x03, 0x84,
            0x01, 0xF4, 0x01, 0xF4, 0x01, 0xF4,
        ];
        for offset in 0x1C0000..scan_end.min(0x200000).saturating_sub(16) {
            if offset >= 2 && data[offset..offset + 14] == single_suffix {
                let addr = offset - 2;
                let v = ((data[addr] as u16) << 8) | (data[addr + 1] as u16);
                if v >= 5000 && v != 0x7FFF {
                    let a32 = addr as u32;
                    if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                        found.insert(a32);
                        maps.push(create_vmax_map(a32, 0.93));
                    }
                }
            }
        }

        // ===== ZONE 3: 0x1F34xx - Pattern with 0x5682 marker =====
        // Structure: ... 56 82 00 00 56 82 01 00 00 00 00 00 FF FF | [VMAX] 00 00 00 00 00 00 FF FF ...
        // StageX: 1F3410 (639C=255)
        for offset in 0x1F3000..scan_end.min(0x1F5000).saturating_sub(20) {
            if offset >= 14 && offset + 10 <= data.len() {
                // Check for 5682 marker in the 14 bytes before
                let m1 = ((data[offset - 10] as u16) << 8) | data[offset - 9] as u16;
                let m2 = ((data[offset - 6] as u16) << 8) | data[offset - 5] as u16;

                if m1 == 0x5682 || m2 == 0x5682 {
                    let v = ((data[offset] as u16) << 8) | (data[offset + 1] as u16);
                    // Check pattern after: 00 00 00 00 00 00 FF FF
                    if v >= 15000 && v <= 35000 && offset + 10 <= data.len() {
                        let a1 = ((data[offset + 2] as u16) << 8) | data[offset + 3] as u16;
                        let a2 = ((data[offset + 4] as u16) << 8) | data[offset + 5] as u16;
                        let a3 = ((data[offset + 6] as u16) << 8) | data[offset + 7] as u16;
                        let a4 = ((data[offset + 8] as u16) << 8) | data[offset + 9] as u16;

                        if a1 == 0x0000 && a2 == 0x0000 && a3 == 0x0000 && a4 == 0xFFFF {
                            let a32 = offset as u32;
                            if !found.contains(&a32) && !detected_addresses.contains(&a32) {
                                found.insert(a32);
                                maps.push(create_vmax_map(a32, 0.90));
                            }
                        }
                    }
                }
            }
        }

        // Sort by address and renumber
        maps.sort_by_key(|m| m.address);
        for (i, m) in maps.iter_mut().enumerate() {
            m.name = Some(format!("Maximum Vehicle Speed {}", i + 1));
        }

        // NOTE: the 1-byte enable switches after the [22 11 D3 00 00 00]
        // signature are intentionally NOT reported — hidden per user request
        // (only the speed values belong in the map list).

        log::debug!("Maximum Vehicle Speed detection (EDC16U34): found {} maps", maps.len());
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

    /// True when a Smoke Limiter by MAP slot starts at `off`: either the
    /// explicit [00 10][00 0B..0D] header, or the implicit anchor curve
    /// [00 02 00 02][00 00][u16][boost-min][boost-max] that precedes the
    /// header-less 8x13 by-MAP block on some firmwares (U31 Passat, Crafter).
    /// The boost bounds (500-1500 -> up to 3200 mbar) tell the by-MAP anchor
    /// apart from the identical-looking RPM-bound curves (3000/5000) that
    /// follow the MAF data.
    fn has_smoke_map_slot(data: &[u8], off: usize) -> bool {
        if off + 12 > data.len() {
            return false;
        }
        if data[off] == 0x00
            && data[off + 1] == 0x10
            && data[off + 2] == 0x00
            && (0x0B..=0x0D).contains(&data[off + 3])
        {
            return true;
        }
        if data[off..off + 6] == [0x00, 0x02, 0x00, 0x02, 0x00, 0x00] {
            let mn = u16::from_be_bytes([data[off + 8], data[off + 9]]);
            let mx = u16::from_be_bytes([data[off + 10], data[off + 11]]);
            return (500..=1500).contains(&mn) && mn < mx && mx <= 3200;
        }
        false
    }

    /// Parse the implicit (header-less) 8x13 Smoke Limiter by MAP block that
    /// follows the [00 02 00 02] anchor: [anchor 20 bytes][8 RPM values]
    /// [13 boost values][8x13 data]. Axes only are validated — the data is
    /// routinely all zero (slot present, ECU running on the MAF map), so it
    /// must never gate the detection.
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
            || y.windows(2).any(|w| w[0] >= w[1])
        {
            return None;
        }
        let x: Vec<u16> = (0..ICOLS)
            .map(|i| u16::from_be_bytes([data[x0 + i * 2], data[x0 + i * 2 + 1]]))
            .collect();
        if x[0] > 1100 || x[ICOLS - 1] > 3200 || x.windows(2).any(|w| w[0] >= w[1]) {
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
            // 16x11: Smoke Limiter by MAP variant on JA/MAXENCE/u31full
            // (boost axis 800..2000 mbar at 0x1D78xx / 0x1D105A)
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

            // Check Y axis is mostly increasing (allow small tolerance for last values)
            let mut y_increasing = true;
            for i in 1..ROWS.saturating_sub(2) {
                if y_axis_values[i] < y_axis_values[i - 1] {
                    y_increasing = false;
                    break;
                }
            }

            if !y_increasing {
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

            // Check X axis is mostly increasing
            let mut x_increasing = true;
            for i in 1..cols {
                if x_axis_values[i] < x_axis_values[i - 1] {
                    x_increasing = false;
                    break;
                }
            }

            if !x_increasing {
                continue;
            }

            // Determine map type based on X axis values
            let first_x = x_axis_values[0];
            let last_x = x_axis_values[cols - 1];

            let (map_name, _x_label, x_unit, x_factor): (&str, &str, &str, f64);

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

                // Structural pairing: in every EDC16 codeblock the Smoke
                // Limiter by MAP slot (explicit 16xN header or implicit 8x13
                // anchor) sits exactly 0xE0 bytes after the end of the MAF
                // data. An air-mass table with that companion is ALWAYS the
                // Smoke Limiter by MAF, no matter what a tuner wrote in it
                // (zeroed-out MAF data is common on modified files). This
                // also fixes the second codeblock of dual-codeblock files,
                // whose MAF used to be misnamed Lambda.
                let has_map_pair = Self::has_smoke_map_slot(data, data_addr + data_size + 0xE0);

                // Determine map type:
                // - If a by-MAP slot follows at +0xE0 => MAF (structural)
                // - If max value > 2500 OR significant values > 2000 => MAF (IQ limiter)
                // - If all values are in lambda range (800-1600) => Lambda
                // Classification logic:
                // - First air mass map with X axis 3000-11000 is MAF (IQ limiter)
                // - Subsequent maps with same axis type are Lambda (if values are in 800-1600 range)
                // - Maps with very high values (>2500) are always MAF regardless of order

                if has_map_pair || max_val > 2500 || high_iq_count > total_count * 10 / 100 {
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

            // At least 70% of data should be valid (relaxed for mixed maps)
            if valid_data_count < (total_data_count * 70 / 100) {
                continue;
            }

            // NOTE: previously we skipped MAF/MAP maps whose data was all below
            // 3500 (treated as "inactive/zeroed"). This was WRONG for modified
            // files: tuners routinely flatten the Smoke Limiter by MAF to a low
            // constant (e.g. all 1000 = 10 mg/stroke) to lift smoke limiting.
            // Stage X still lists such a map because it exists structurally.
            // Detection must therefore rely on the header + axes only, never on
            // the data magnitude. Filter removed.

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

        // Emit the implicit Smoke Limiter by MAP variant: some firmwares
        // (U31 Passat, Crafter) store the by-MAP slot as a header-less 8x13
        // block behind the [00 02 00 02] anchor curve, always 0xE0 bytes
        // after the end of the MAF data. The slot exists in EVERY file even
        // when its data is all zero (management running on the MAF map).
        let mut implicit_maps: Vec<DetectedMap> = Vec::new();
        for m in &maps {
            if m.name.as_deref() != Some("Smoke Limiter by MAF") {
                continue;
            }
            let anchor = m.address as usize + m.size + 0xE0;
            if let Some(imap) = self.parse_implicit_smoke_map(data, anchor, detected) {
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

    /// Detect Gearbox Torque Limiter maps (1D, one per gear)
    /// These maps limit torque output per gear to protect the drivetrain
    ///
    /// Structure per map:
    /// - Header: [00][N] with N = RPM point count (11-15 depending on firmware:
    ///   15 on 2.0 TDI passenger cars with axis 635..4000, 12 on the Crafter
    ///   with axis 500..5300)
    /// - X-axis: N × 2 bytes (strictly increasing RPM values)
    /// - Data: N × 2 bytes (torque values in Nm × 10, typically 30000 = limiter disabled)
    ///
    /// The maps appear consecutively in groups of 6-7 (one per gear + neutral),
    /// ALL sharing the identical RPM axis. Detection therefore requires a run
    /// of at least 4 consecutive units with the same axis, and at least one
    /// torque value >= 10000 somewhere in the run — isolated 1D curves that
    /// happen to share the profile never qualify.
    pub fn detect_gearbox_torque_limiter(&self, data: &[u8], detected: &HashSet<u32>) -> Vec<DetectedMap> {
        let mut maps = Vec::new();
        let read = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);

        let (scan_start, scan_end) = self.get_data_section_range(data.len());
        let scan_end = scan_end.min(data.len());

        // Validate one unit at `o`: [00 N] header + N-value RPM axis + N data
        // values. Returns the axis length when structurally valid.
        let unit_len = |o: usize| -> Option<usize> {
            if o + 2 > scan_end {
                return None;
            }
            let n = read(o) as usize;
            if !(11..=15).contains(&n) || o + 2 + n * 4 > scan_end {
                return None;
            }
            let axis: Vec<u16> = (0..n).map(|i| read(o + 2 + i * 2)).collect();
            if !(300..=1500).contains(&axis[0])
                || !(2500..=6000).contains(&axis[n - 1])
                || !axis.windows(2).all(|w| w[0] < w[1])
            {
                return None;
            }
            // Torque data: plausible values (0-6000 Nm), not all zero
            let mut all_zero = true;
            for i in 0..n {
                let v = read(o + 2 + n * 2 + i * 2);
                if v > 60000 {
                    return None;
                }
                if v != 0 {
                    all_zero = false;
                }
            }
            if all_zero {
                return None;
            }
            Some(n)
        };

        let mut offset = scan_start;
        let mut found_count = 0;
        let mut single_value_detected = false;

        while offset + 2 <= scan_end {
            let Some(n) = unit_len(offset) else {
                offset += 2;
                continue;
            };
            let unit_size = 2 + n * 4;
            let axis_bytes = &data[offset + 2..offset + 2 + n * 2];

            // Collect the run of units sharing the identical RPM axis. Units
            // may be separated by a few bytes of per-gear side data (Crafter:
            // 12 bytes of extra values between each 12-point unit)
            let mut unit_starts = vec![offset];
            let mut cursor = offset + unit_size;
            'run: loop {
                for pad in (0..=16usize).step_by(2) {
                    let cand = cursor + pad;
                    if unit_len(cand) == Some(n)
                        && &data[cand + 2..cand + 2 + n * 2] == axis_bytes
                    {
                        unit_starts.push(cand);
                        cursor = cand + unit_size;
                        continue 'run;
                    }
                }
                break;
            }
            let count = unit_starts.len();
            if count < 4 {
                offset += 2;
                continue;
            }

            // At least one real torque limit (>= 1000 Nm×10) somewhere in the
            // run — rejects runs of small-magnitude curves (duty cycles, temps)
            let has_torque_value = unit_starts.iter().any(|&s| {
                let data_addr = s + 2 + n * 2;
                (0..n).any(|i| read(data_addr + i * 2) >= 10000)
            });
            if !has_torque_value {
                offset += 2;
                continue;
            }

            // ===== SINGLE VALUE GEARBOX LIMITER =====
            // Single value (1x1) that appears 2 bytes BEFORE the first header
            // This is a global gearbox torque limiter scalar
            if !single_value_detected && offset >= 2 {
                let single_val_addr = offset - 2;
                let single_val = read(single_val_addr);

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

            for &unit_start in &unit_starts {
                let axis_start = unit_start + 2;
                let data_addr = axis_start + n * 2;

                // Skip if already detected
                if detected.contains(&(data_addr as u32)) {
                    continue;
                }

                found_count += 1;
                log::debug!("🎯 [EDC16] Found Gearbox Torque Limiter #{} at data 0x{:X} (header at 0x{:X}, {} points)",
                          found_count, data_addr, unit_start, n);

                let mut map = DetectedMap::new(
                    data_addr as u32,
                    n * 2,
                    MapDimensions::OneDimensional { length: n },
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
            }

            offset = unit_starts[count - 1] + unit_size;
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
        // démarre ≥ 2000) ou la variante PADDÉE de l'ambient pressure (KN
        // 0x1D5E1C : axe 100..1500 + marches de 1, comme la 5x5 des autres)
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

        // 5x5 - check Y axis to distinguish ambient pressure vs oil temp vs IAT
        if rows == 5 && cols == 5 {
            // Ambient pressure typically starts lower (100-800 mbar)
            if y_first < 1000 {
                return "Fuel correction by ambient pressure";
            }
            // High-temperature-only axis (starts >= ~70°C = 3430 K×10):
            // intake air temperature protection map (Passat U31 at 0x1CF5A4
            // has 80..130°C). Oil temperature maps start cold (-30..0°C,
            // raw 2431..2731).
            if y_first >= 3400 {
                return "Fuel correction factor by intake air temperature";
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

        // ===== EGR "Desired air quantity" Maps Detection (AirCtl_mDesBase) =====
        //
        // Structure: [rows u16 BE][cols u16 BE][Y axis: rows RPM values][X axis: cols IQ values][data rows*cols]
        // Sizes vary by firmware: 10x10, 12x12, 13x14, 13x15, 16x13... (rows/cols in 10..=16)
        //
        // These tables form a contiguous block located just BEFORE the EGR hysteresis
        // cluster in the calibration area — this positional anchor plus STRICT axis
        // validation is the discriminator. Data values are NOT validated (beyond a
        // loose sanity bound) so detection works identically on MODIFIED files
        // (stage 1/2/3 tunes change the data, never the axes or the layout).
        //
        // Verified on abcABC_Version 2 (Stage X reference): 10/10 maps at exact
        // addresses (0x1C4302..0x1C524E), identical results on ORI vs modified.

        // 1) Find hysteresis cluster anchors: [00 14] header + valid RPM axis.
        //    Only the FIRST hysteresis of each cluster is kept as anchor.
        let mut hyst_anchors: Vec<usize> = Vec::new();
        let mut i = 0x180000;
        while i < data.len().saturating_sub(100) {
            if data[i] == 0x00 && data[i + 1] == 0x14 {
                let mut x_values = Vec::with_capacity(20);
                let mut ok = true;
                for j in 0..20 {
                    let o = i + 2 + j * 2;
                    if o + 1 >= data.len() { ok = false; break; }
                    x_values.push(u16::from_be_bytes([data[o], data[o + 1]]));
                }
                if ok && self.is_valid_egr_hyst_rpm_axis_strict(&x_values) {
                    if hyst_anchors.last().map_or(true, |&a| i > a + 0x1000) {
                        hyst_anchors.push(i);
                    }
                    i += 80; // skip this hysteresis block
                    continue;
                }
            }
            i += 2;
        }

        // 2) For each anchor, scan the window BEFORE the hysteresis cluster
        const DAQ_WINDOW: usize = 0x8000;
        for &anchor in &hyst_anchors {
            let lo = anchor.saturating_sub(DAQ_WINDOW);
            let mut pos = lo;
            'scan: while pos + 4 < anchor {
                let rows = u16::from_be_bytes([data[pos], data[pos + 1]]) as usize;     // Y axis len (RPM)
                let cols = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize; // X axis len (IQ)
                // Only the canonical AirCtl_mDesBase family is a real EGR
                // map: 16 RPM rows x 13-16 IQ cols (13 on most firmwares —
                // Stage X/tool listings; 14-16 on the JA/KN/Superb gear
                // trios at the same locations). The other sizes this scan
                // used to accept (10x10, 12x12, 13x14...) are neighbouring
                // tables (false positives flagged by the user).
                if rows != 16 || !(13..=16).contains(&cols) {
                    pos += 2;
                    continue;
                }

                let y_axis_start = pos + 4;
                let x_axis_start = y_axis_start + rows * 2;
                let data_start = x_axis_start + cols * 2;
                let data_end = data_start + rows * cols * 2;
                if data_end > data.len() {
                    pos += 2;
                    continue;
                }

                // Y axis = RPM: strictly increasing, starts low, ends 2800-6000
                let mut y_values = Vec::with_capacity(rows);
                for j in 0..rows {
                    let o = y_axis_start + j * 2;
                    y_values.push(u16::from_be_bytes([data[o], data[o + 1]]));
                }
                if !self.is_valid_daq_rpm_axis(&y_values) {
                    pos += 2;
                    continue;
                }

                // X axis = IQ: strictly increasing, starts near 0, ends 2500-5500
                let mut x_values = Vec::with_capacity(cols);
                for j in 0..cols {
                    let o = x_axis_start + j * 2;
                    x_values.push(u16::from_be_bytes([data[o], data[o + 1]]));
                }
                if !self.is_valid_daq_iq_axis(&x_values) {
                    pos += 2;
                    continue;
                }

                // Loose anti-garbage bound only — NO tight data validation, so
                // modified files (higher requested air mass) are still detected.
                // 32767 (0x7FFF) is a legit "EGR off" sentinel in high-load
                // cells (Superb MK1 DAQ maps at 0x1C3774..0x1C431A), so only
                // values that would be negative as i16 are rejected.
                let mut max_v: u16 = 0;
                for j in (data_start..data_end).step_by(2) {
                    let v = u16::from_be_bytes([data[j], data[j + 1]]);
                    if v > 32767 {
                        pos += 2;
                        continue 'scan;
                    }
                    if v > max_v {
                        max_v = v;
                    }
                }
                // A desired-air-quantity map always requests hundreds of
                // mg/stroke somewhere. Tables whose ceiling stays below
                // 100 mg/stroke (raw 1000) are unrelated calibration data —
                // e.g. the constant-12.8 10x10 false positives on JA/KN/
                // Passat/Superb, absent from every reference list.
                if max_v < 1000 {
                    pos = data_end;
                    continue;
                }

                if detected.contains(&(data_start as u32)) {
                    pos = data_end;
                    continue;
                }

                egr_2d_count += 1;
                let map_name = format!("EGR (Desired air quantity) {}", egr_2d_count);
                log::debug!("🎯 [EDC16] Found {} at 0x{:X} ({}x{})", map_name, data_start, cols, rows);

                // Same conventions as the previous EGR 2D detection:
                // first axis in binary = RPM (displayed as X), second = IQ (displayed as Y)
                let mut map = DetectedMap::new(
                    data_start as u32,
                    rows * cols * 2,
                    MapDimensions::TwoDimensional { rows: cols, cols: rows },
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
                pos = data_end; // jump over the whole map
            }
        }

        // Supplementary zone scan: some layouts park an extra canonical
        // 16x13 DAQ block outside any hysteresis-anchored window (g5run
        // codeblock 2 at 0x190B88). With the strict 16x13-only rule plus
        // the axis/data validators, a plain sweep of the calibration zone
        // is safe.
        let zone_end = data.len().min(0x1D0000);
        let mut pos = 0x180000usize;
        'zone: while pos + 4 + 32 + 32 + 512 <= zone_end {
            if !(data[pos] == 0x00
                && data[pos + 1] == 0x10
                && data[pos + 2] == 0x00
                && (0x0D..=0x10).contains(&data[pos + 3]))
            {
                pos += 2;
                continue;
            }
            let cols = data[pos + 3] as usize;
            let y_axis_start = pos + 4;
            let x_axis_start = y_axis_start + 32;
            let data_start = x_axis_start + cols * 2;
            let data_end = data_start + 16 * cols * 2;
            let y_values: Vec<u16> = (0..16)
                .map(|j| u16::from_be_bytes([data[y_axis_start + j * 2], data[y_axis_start + j * 2 + 1]]))
                .collect();
            let x_values: Vec<u16> = (0..cols)
                .map(|j| u16::from_be_bytes([data[x_axis_start + j * 2], data[x_axis_start + j * 2 + 1]]))
                .collect();
            if !self.is_valid_daq_rpm_axis(&y_values) || !self.is_valid_daq_iq_axis(&x_values) {
                pos += 2;
                continue;
            }
            let mut max_v: u16 = 0;
            for j in (data_start..data_end).step_by(2) {
                let v = u16::from_be_bytes([data[j], data[j + 1]]);
                if v > 32767 {
                    pos += 2;
                    continue 'zone;
                }
                if v > max_v {
                    max_v = v;
                }
            }
            if max_v < 1000
                || detected.contains(&(data_start as u32))
                || maps.iter().any(|m| m.address == data_start as u32)
            {
                pos = data_end;
                continue;
            }
            egr_2d_count += 1;
            let map_name = format!("EGR (Desired air quantity) {}", egr_2d_count);
            log::debug!("🎯 [EDC16] Found {} (zone scan) at 0x{:X}", map_name, data_start);
            let mut map = DetectedMap::new(
                data_start as u32,
                16 * cols * 2,
                MapDimensions::TwoDimensional { rows: cols, cols: 16 },
                DataType::UInt16,
            );
            map.name = Some(map_name);
            map.category = Some("EGR".to_string());
            map.unit = Some("mg/stroke".to_string());
            map.correction_factor = Some(0.1);
            map.offset = Some(0.0);
            map.confidence = 0.88;
            map.y_axis_address = Some(x_axis_start as u32);
            map.y_label = Some("mg/stroke".to_string());
            map.y_axis_correction = Some(0.01);
            map.y_axis_offset = Some(0.0);
            map.x_axis_address = Some(y_axis_start as u32);
            map.x_label = Some("RPM".to_string());
            map.x_axis_correction = Some(1.0);
            map.x_axis_offset = Some(0.0);
            maps.push(map);
            pos = data_end;
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

        // Search in zones 0x180000+ and 0x1C0000+ for [00 14] header pattern
        for i in 0x180000..data.len().saturating_sub(100) {
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

            // Must have at least 2 neighbors (maps come in groups of 5).
            // A lone candidate is still kept when a real cluster follows
            // within the same EGR region (crafter: isolated hysteresis at
            // 0x1C42F6, 0x5BD0 before the 5-map cluster at 0x1C9EC6).
            if nearby_count >= 2 {
                valid_candidates.push(egr_hyst_candidates[i].clone());
            } else {
                let near_cluster_after = egr_hyst_candidates.iter().any(|(other_addr, _)| {
                    *other_addr > *addr
                        && other_addr - addr < 0x8000
                        && egr_hyst_candidates.iter().filter(|(a2, _)| {
                            let d = if a2 > other_addr { a2 - other_addr } else { other_addr - a2 };
                            d > 0 && d < 500
                        }).count() >= 2
                });
                if near_cluster_after {
                    valid_candidates.push(egr_hyst_candidates[i].clone());
                }
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
                DataType::UInt16,
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
                DataType::UInt16,
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

        log::debug!("🔧 [EDC16] EGR detection: {} 2D maps, {} Hysteresis maps", egr_2d_count, egr_hyst_count);
        maps
    }

    /// Validates RPM axis for EGR 2D maps (strict - 16 values only)
    /// Based on JSON reference: first value ~0, last value ~3423
    /// Validates RPM axis for "Desired air quantity" (EGR classic) maps.
    /// Structure-only validation (works on modified files): strictly increasing,
    /// starts <= 1400 rpm, ends between 2800 and 6000 rpm.
    fn is_valid_daq_rpm_axis(&self, values: &[u16]) -> bool {
        if values.len() < 10 {
            return false;
        }
        if values[0] > 1400 {
            return false;
        }
        let last = values[values.len() - 1];
        if last < 2800 || last > 6000 {
            return false;
        }
        values.windows(2).all(|w| w[0] < w[1])
    }

    /// Validates IQ axis for "Desired air quantity" (EGR classic) maps.
    /// Structure-only validation: strictly increasing, starts <= 700
    /// (U31 10x10 DAQ maps start their IQ axis at 500), ends between
    /// 2500 and 5500 (25-55 mg/stroke).
    fn is_valid_daq_iq_axis(&self, values: &[u16]) -> bool {
        if values.len() < 10 {
            return false;
        }
        if values[0] > 700 {
            return false;
        }
        let last = values[values.len() - 1];
        if last < 2500 || last > 5500 {
            return false;
        }
        values.windows(2).all(|w| w[0] < w[1])
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
            // Header [00 r 00 04] : r lignes de température × 4 colonnes.
            // r observé selon les calibrations : 2 (MAXENCE), 3 (Jetta 1.9),
            // 4 et 6 (KN), 5 (Passat 0x1DB614) — on accepte 2..=6.
            if i + 3 >= data.len()
                || data[i] != 0x00
                || !(2..=6).contains(&data[i + 1])
                || data[i + 2] != 0x00
                || data[i + 3] != 0x04
            {
                continue;
            }

            let (rows, cols) = (data[i + 1] as usize, 4usize);

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
                    // Idle RPM values typically 750-1500
                    if val >= 700 && val <= 1600 {
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

            // Check next value is 012C (300)
            if i + 3 >= data.len() {
                continue;
            }
            if data[i + 2] != 0x01 || data[i + 3] != 0x2C {
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
            let is_pattern1 = prev2 == 0x03E8 && prev1 == 0x0011;

            // Pattern 2: Something then [0000] then RPM
            let is_pattern2 = prev1 == 0x0000 && prev2 > 0x0100 && prev2 < 0x0400;

            // Pattern 3: [.. FFFF][0011] then RPM then [012C] — abcABC/Jetta1.9
            // firmware variant. The [0011] marker immediately before the idle
            // value, together with the mandatory [012C] after it (already
            // checked), is specific enough on the whole test corpus.
            let is_pattern3 = prev1 == 0x0011 && prev2 == 0xFFFF;

            // Pattern 4: [0064][0011] then RPM then [012C] — U31 variant
            // (Passat: 0x1E0564, value 672 rpm preceded by [00 64][00 11])
            let is_pattern4 = prev1 == 0x0011 && prev2 == 0x0064;

            // Pattern 5: [FFFF][0000] then RPM then [012C] — Crafter
            // (0x1F0A36) et bloc paramètre par codeblock sur d'autres
            // calibrations (1 seul hit par fichier sur tout le corpus)
            let is_pattern5 = prev1 == 0x0000 && prev2 == 0xFFFF;

            if !is_pattern1 && !is_pattern2 && !is_pattern3 && !is_pattern4 && !is_pattern5 {
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

        // Should be strictly increasing (temperature axis)
        for i in 1..values.len() {
            if values[i] <= values[i - 1] {
                return false;
            }
        }

        // First value should be cold temperature (-40 to 0°C = 2331 to 2731)
        if values[0] < 2300 || values[0] > 2900 {
            return false;
        }

        // Last value: certains axes s'arrêtent tièdes ou plafonnent en
        // marches de 1 (KN : [2531..2911] et [2530,2831..2835]) — exiger
        // « chaud » (≥3000) rejetait des maps réelles. Seuil ramené à 2800.
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

        for i in 0x1C8000..data.len().saturating_sub(100) {
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

        for i in 0x1C8000..data.len().saturating_sub(100) {
            if i + 81 >= data.len() {
                continue;
            }

            let val1 = u16::from_be_bytes([data[i], data[i + 1]]);
            let val2 = u16::from_be_bytes([data[i + 2], data[i + 3]]);

            // First value should be around 1731 (0x06C3) ± tolerance
            if val1 < 0x0600 || val1 > 0x0800 {
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

            // Verify it's a valid increasing sequence of ~40 values ending in padding
            let mut values = Vec::new();
            let mut found_padding = false;

            for j in 0..50 {
                let addr = i + j * 2;
                if addr + 1 >= data.len() {
                    break;
                }
                let val = u16::from_be_bytes([data[addr], data[addr + 1]]);

                // Check for padding (0x0030)
                if val == 0x0030 {
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

        // ===== EGT Base Map Detection (11x14, 12x13, 11x12) =====
        // These are larger 2D maps with RPM on one axis
        // Header patterns: [00 0D 00 0C] (13x12), [00 0E 00 0B] (14x11),
        // [00 0C 00 0B] (12x11 — abcABC/ZZZori firmware at 0x1DD74E/0x1DD8A2)
        // NOTE: scan starts at 0x1D0000 (not 0x1E0000): the 12x11 variants
        // live around 0x1DD7xx.

        for i in 0x1D0000..data.len().saturating_sub(400) {
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
                           (rows == 11 && cols == 14) ||  // 11x14
                           (rows == 12 && cols == 11) ||  // 12x11
                           (rows == 11 && cols == 12) ||  // 11x12
                           (rows == 14 && cols == 14);    // 14x14 (Crafter pair at 0x1E4DFE/0x1E4FC2)

            if !valid_dims {
                continue;
            }

            // The 14x14 shape overlaps the turbo-request profile — require the
            // hot EGT data range (>= 350 K×10... i.e. 3500) so a boost map
            // (values <= ~3500 mbar) can never be claimed as EGT base.
            let strict_hot_data = rows == 14 && cols == 14;

            // The 11x12 / 12x11 variants share their shape with Boost-request
            // maps. On U31 the turbo region starts at 0x1DC000 (Passat boost
            // requests at 0x1DDD52/0x1DDFFA were previously mis-detected as
            // EGT base maps shifted by +2) — restrict these two shapes to
            // below 0x1DC000 where real EGT base maps reside.
            if (rows == 12 && cols == 11 || rows == 11 && cols == 12) && i >= 0x1DC000 {
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
            let (lo, hi) = if strict_hot_data { (3500u16, 12000u16) } else { (1000, 15000) };
            let mut valid_count = 0;
            for j in (data_start..data_start + data_size).step_by(2) {
                let val = u16::from_be_bytes([data[j], data[j + 1]]);
                // EGT temps: 200°C to 900°C = 4731 to 11731 in Kelvin×10
                // But raw values might be different scaling
                if val >= lo && val <= hi {
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
