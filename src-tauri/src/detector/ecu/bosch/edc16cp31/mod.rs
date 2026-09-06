// Bosch EDC16CP31 detector (Mercedes-Benz OM642 / OM646 common rail).
//
// STATUS: CALIBRATED. See the header of `signatures.rs` for the corpus and
// for the record layout this module is built on.
//
// Why this detector does not look like the VAG ones
// -------------------------------------------------
// The VAG EDC16 modules scan for a plausible [axis][axis][data] triple and
// guess the grid from a list of candidate sizes. On CP31 that is unnecessary
// and strictly worse: every 2D map uses the Bosch `Kf_Xs16_Ys16_Ws16` record
// layout, which is SELF-DESCRIBING -
//
//   +0x00  u16        nx          number of X points
//   +0x02  u16        ny          number of Y points
//   +0x04  i16[nx]    X axis      strictly increasing
//   +....  i16[ny]    Y axis      strictly increasing
//   +....  i16[nx*ny] Z, column direction (Y is the fast index)
//
// so the grid is read, not guessed. The detector walks the calibration area,
// reads that header, validates both axes against the expected physical
// meaning, and range-checks the data block against the template. A wrong
// grid can no longer produce a plausible-looking map, which removes the
// single worst failure mode of the axis-first engine.
//
// Presentation: because Z is column-major, `rows = nx` (engine speed) and
// `cols = ny` (load) makes the file order and the display order identical -
// no transposition anywhere, and it matches how WinOLS shows these maps. The
// consequence is that `y_axis_address` points at the FIRST axis in the block
// (engine speed) and `x_axis_address` at the second (load).
//
// REUSED from the VAG detectors: big-endian MPC5xx decoding, the scoring
// model (range fit + monotonicity), and the EDC16 checksum path - region
// descriptors are self-describing (magic FA DE CA FE CA FE AF FE at
// region_start + 0x3C, found at 0x19003C on the corpus dump), so
// src/lib/ecu/bosch/checksums/edc16-checksum.ts routes CP31 unchanged.
//
// Safety contract of this module
// ------------------------------
// `detect()` still returns an empty vector while no template is calibrated,
// and never falls back to the VAG detectors. Per CONTRIBUTING.md:
// "A false 'unsupported file' is annoying; a false 'this is an EDC16'
// corrupts someone's ECU."
//
// Known limits of a corpus of one
// -------------------------------
//   * addresses come from ONE software build (1037393817). They are used as
//     search WINDOWS, never as fixed addresses.
//   * the factors and axis semantics come from the manufacturer description
//     of the project and are exact.
//   * OM646 (4-cylinder) has not been checked at all. Its zones will differ.

mod signatures;

pub use signatures::{
    AxisSignature, BlockMarker, DimensionRange, EDC16MapSignature, StructureType, CP31_MARKERS,
    CP31_SIGNATURES,
};

use crate::models::{DataType, DetectedMap, MapCategory, MapDimensions};
use std::collections::HashSet;

// ============================= AXIS TYPES =============================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisType {
    /// Engine speed, 1 rpm/bit. Several CP31 speed axes start at 0.
    Rpm,
    /// Temperature-corrected speed used by the boost limiter, 1 rpm/bit,
    /// runs well past the real rev range (up to 10000).
    RpmCorrected,
    /// Accelerator pedal / any duty-cycle ratio: 1/8192 per bit, so
    /// 8192 == 100 %. The VAG 0.01 %/bit does NOT apply here.
    Percent,
    /// Injected quantity, 0.01 mm3/stroke per bit.
    InjectionQty,
    /// Rail pressure, 0.1 bar/bit (the CP31 `Pres_Rail` conversion).
    RailPressure,
    /// Boost / manifold pressure, 1 hPa absolute per bit.
    BoostPressure,
    /// Air mass, 0.1 mg/stroke per bit.
    AirMass,
    /// Torque set point, 0.1 Nm/bit.
    Torque,
    /// Atmospheric / ambient pressure, 1 hPa/bit.
    AtmosphericPressure,
    /// Coolant / intake temperature. CP31 works in KELVIN: 0.1 K/bit, so
    /// 2731 == 0 degC.
    Temperature,
    /// No axis (scalar).
    None,
}

impl AxisType {
    /// Accepted RAW value window for this axis type, before the factor.
    /// Returns None for `AxisType::None`.
    const fn raw_range(self) -> Option<(u16, u16)> {
        match self {
            AxisType::Rpm => Some((0, 6_400)),
            AxisType::RpmCorrected => Some((100, 10_500)),
            AxisType::Percent => Some((0, 8_192)),
            AxisType::InjectionQty => Some((0, 11_000)),
            AxisType::RailPressure => Some((1_000, 20_000)),
            AxisType::BoostPressure => Some((400, 3_200)),
            AxisType::AirMass => Some((0, 16_000)),
            AxisType::Torque => Some((0, 8_000)),
            AxisType::AtmosphericPressure => Some((400, 1_200)),
            AxisType::Temperature => Some((2_000, 4_400)),
            AxisType::None => None,
        }
    }

    const fn factor(self) -> f64 {
        match self {
            AxisType::Rpm | AxisType::RpmCorrected => 1.0,
            AxisType::Percent => 0.012_207_031_25,
            AxisType::InjectionQty => 0.01,
            AxisType::RailPressure => 0.1,
            AxisType::BoostPressure => 1.0,
            AxisType::AirMass => 0.1,
            AxisType::Torque => 0.1,
            AxisType::AtmosphericPressure => 1.0,
            AxisType::Temperature => 0.1,
            AxisType::None => 1.0,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            AxisType::Rpm | AxisType::RpmCorrected => "rpm",
            AxisType::Percent => "%",
            AxisType::InjectionQty => "mm^3/cyc",
            AxisType::RailPressure => "bar",
            AxisType::BoostPressure => "hPa",
            AxisType::AirMass => "mg",
            AxisType::Torque => "Nm",
            AxisType::AtmosphericPressure => "hPa",
            AxisType::Temperature => "K",
            AxisType::None => "",
        }
    }
}

// ============================ MAP TEMPLATES ============================

/// Declarative description of one map family.
///
/// `calibrated` is the gate: a template stays inert until its grids, factor
/// and physical ranges have been confirmed against real CP31 files.
#[derive(Debug, Clone)]
pub struct MapTemplate {
    pub name: &'static str,
    /// Manufacturer label(s) this family carries in the Daimler ASAP2
    /// description. Kept so a report can be traced back to the damos.
    pub damos_label: &'static str,
    pub category: MapCategory,
    /// Accepted grid sizes as (nx, ny) - nx is the engine-speed axis, which
    /// is also the number of displayed ROWS. Read from the block header, so
    /// this list is a filter, not a guess.
    pub grids: &'static [(usize, usize)],
    /// (X axis type, Y axis type) in BLOCK ORDER: X is the first axis in
    /// memory (engine speed on every calibrated family), Y the second.
    pub axes: (AxisType, AxisType),
    /// raw * z_factor + z_offset = physical value.
    pub z_factor: f64,
    pub z_offset: f64,
    /// Physical range accepted on a stock file.
    pub z_range_stock: (f64, f64),
    /// Physical range accepted on a tuned file.
    pub z_range_tuned: (f64, f64),
    pub unit: &'static str,
    pub signed: bool,
    /// Maximum number of maps of this family expected in one file.
    pub max_count: usize,
    /// Higher is scanned first.
    pub priority: u8,
    /// Tight address window this family was found in, widened to a round
    /// boundary. Narrower than the per-category `CP31_ZONES` entry.
    pub zone: Option<(usize, usize)>,
    /// False = hypothesis only, ignored by `detect()`.
    pub calibrated: bool,
}

/// Map families confirmed on an OM642-class EDC16CP31.
///
/// Every entry below was read out of a real dump and cross-checked against
/// the manufacturer description of the project: the grid comes from the
/// block header, the factor and unit from the ASAP2 COMPU_METHOD, the axis
/// semantics from the AXIS_DESCR input signal. The physical ranges are the
/// observed stock span widened to the value the ECU's own limits allow.
pub const MAP_TEMPLATES: &[MapTemplate] = &[
    MapTemplate {
        name: "Rail Pressure Target",
        damos_label: "Rail_pSetPointBase_MAP",
        category: MapCategory::InjectionSystem,
        grids: &[(16, 16), (16, 12), (16, 8), (12, 12)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.1,
        z_offset: 0.0,
        // observed stock: 250 .. 1632 bar
        z_range_stock: (150.0, 1700.0),
        z_range_tuned: (150.0, 1900.0),
        unit: "bar",
        signed: false,
        max_count: 3,
        priority: 95,
        zone: Some((0x1F1000, 0x1F3200)),
        calibrated: true,
    },
    MapTemplate {
        name: "Rail Pressure Limiter",
        damos_label: "Rail_pSetPointLimN_MAP",
        category: MapCategory::InjectionSystem,
        grids: &[(8, 12), (8, 8), (12, 12), (16, 12)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.1,
        z_offset: 0.0,
        // observed stock: 900 .. 1632 bar
        z_range_stock: (600.0, 1700.0),
        z_range_tuned: (600.0, 1950.0),
        unit: "bar",
        signed: false,
        max_count: 2,
        priority: 93,
        zone: Some((0x1F1000, 0x1F3200)),
        calibrated: true,
    },
    MapTemplate {
        name: "Boost Target",
        damos_label: "PCR_pDesBas_MAP",
        category: MapCategory::TurboBoostPressure,
        grids: &[(16, 12), (16, 16), (12, 12)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 1.0,
        z_offset: 0.0,
        // absolute pressure. observed stock: 1010 .. 2570 hPa
        z_range_stock: (900.0, 2700.0),
        z_range_tuned: (900.0, 3100.0),
        unit: "hPa",
        signed: false,
        max_count: 2,
        priority: 94,
        zone: Some((0x1E4000, 0x1E5000)),
        calibrated: true,
    },
    MapTemplate {
        name: "Boost Limiter",
        damos_label: "PCR_pBDesMaxAPGearHi_MAP / PCR_pBDesMaxAPGearLo_MAP",
        category: MapCategory::TurboBoostPressure,
        grids: &[(16, 12), (16, 16)],
        axes: (AxisType::RpmCorrected, AxisType::AtmosphericPressure),
        z_factor: 1.0,
        z_offset: 0.0,
        // the unused top-right corner of these two maps is zeroed on stock,
        // so the lower bound MUST be 0 or the whole block is rejected.
        z_range_stock: (0.0, 2900.0),
        z_range_tuned: (0.0, 3300.0),
        unit: "hPa",
        signed: false,
        max_count: 2,
        priority: 88,
        zone: Some((0x1E3800, 0x1E4400)),
        calibrated: true,
    },
    MapTemplate {
        name: "VNT Duty Cycle",
        damos_label: "PCR_rCtlBas_MAP",
        category: MapCategory::TurboBoostPressureControl,
        grids: &[(16, 8), (16, 16), (12, 8)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.012_207_031_25,
        z_offset: 0.0,
        // observed stock: 23.5 .. 88.0 %
        z_range_stock: (0.0, 100.0),
        z_range_tuned: (0.0, 100.0),
        unit: "%",
        signed: false,
        max_count: 4,
        priority: 86,
        zone: Some((0x1DB000, 0x1DC000)),
        calibrated: true,
    },
    MapTemplate {
        name: "Smoke Limiter by boost pressure",
        damos_label: "FlMng_qSmk_MAP",
        category: MapCategory::SmokeLimitation,
        grids: &[(16, 16), (12, 12)],
        // NOT by MAF: on CR4 the smoke limit is a function of engine speed
        // and CORRECTED BOOST PRESSURE (PCR_pBPSCor), not of air mass.
        axes: (AxisType::Rpm, AxisType::BoostPressure),
        z_factor: 0.01,
        z_offset: 0.0,
        // observed stock: 24 .. 86 mm3/stroke
        z_range_stock: (0.0, 95.0),
        z_range_tuned: (0.0, 130.0),
        unit: "mm^3/cyc",
        signed: false,
        max_count: 2,
        priority: 92,
        zone: Some((0x1A8000, 0x1A9000)),
        calibrated: true,
    },
    MapTemplate {
        name: "Driver Wish",
        damos_label: "AccPed_trqEng_MAP / AccPed_trqEng2_MAP",
        category: MapCategory::EngineTorqueRequest,
        grids: &[(8, 8), (16, 16), (12, 12)],
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.1,
        z_offset: 0.0,
        // observed stock: 0 .. 500 Nm (trqEng), 0 .. 600 Nm (trqEng2)
        z_range_stock: (0.0, 700.0),
        z_range_tuned: (0.0, 950.0),
        unit: "Nm",
        signed: false,
        max_count: 3,
        priority: 90,
        zone: Some((0x191000, 0x192000)),
        calibrated: true,
    },
    MapTemplate {
        name: "Quantity Limiter by boost pressure",
        damos_label: "FlMng_qLimBstPres_MAP",
        category: MapCategory::EngineTorqueLimiters,
        grids: &[(12, 12), (16, 16)],
        axes: (AxisType::Rpm, AxisType::BoostPressure),
        z_factor: 0.01,
        z_offset: 0.0,
        // observed stock: 0 .. 100 mm3/stroke
        z_range_stock: (0.0, 110.0),
        z_range_tuned: (0.0, 145.0),
        unit: "mm^3/cyc",
        signed: false,
        max_count: 3,
        priority: 87,
        zone: Some((0x1A7C00, 0x1A8400)),
        calibrated: true,
    },
    MapTemplate {
        name: "Torque to IQ Conversion",
        damos_label: "FMTC_trq2qBas_MAP",
        category: MapCategory::TorqueToIqConversion,
        grids: &[(16, 18), (16, 16)],
        axes: (AxisType::Rpm, AxisType::Torque),
        z_factor: 0.01,
        z_offset: 0.0,
        // observed stock: 0 .. 88 mm3/stroke
        z_range_stock: (0.0, 95.0),
        z_range_tuned: (0.0, 130.0),
        unit: "mm^3/cyc",
        signed: false,
        max_count: 2,
        priority: 84,
        zone: Some((0x1A9000, 0x1AA000)),
        calibrated: true,
    },
    MapTemplate {
        name: "Start of injection",
        damos_label: "InjCrv_phiMI1Bas1..3_MAP / InjCrv_phiMI1Max1..2_MAP",
        category: MapCategory::StartOfInjection,
        grids: &[(16, 16), (12, 12)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        // 3/128 degree of crankshaft per bit
        z_factor: 0.023_437_5,
        z_offset: 0.0,
        // observed stock: -10.6 .. +27.6 degCrS across the five members
        z_range_stock: (-20.0, 32.0),
        z_range_tuned: (-25.0, 38.0),
        unit: "degCrS",
        signed: true,
        max_count: 8,
        priority: 83,
        zone: Some((0x1B0000, 0x1B2000)),
        calibrated: true,
    },
    MapTemplate {
        name: "EGR air mass target",
        damos_label: "AirCtl_mDesBas_MAP",
        category: MapCategory::Egr,
        grids: &[(12, 16), (16, 16)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.1,
        z_offset: 0.0,
        // observed stock: 240 .. 1060 mg/stroke
        z_range_stock: (0.0, 1300.0),
        z_range_tuned: (0.0, 1300.0),
        unit: "mg/stroke",
        signed: false,
        max_count: 3,
        priority: 80,
        zone: Some((0x194000, 0x196000)),
        calibrated: true,
    },
];

// ============================== ZONES ==============================

/// Address windows of the calibration area, per map family.
///
/// Coarse, per-CATEGORY windows. They exist so that a family whose template
/// has no tight `zone` still gets a bounded scan; the tight window on the
/// template itself is what makes the scan fast. `None` means "scan the whole
/// calibration area", which is correct but slower and more false-positive
/// prone.
#[derive(Debug, Clone, Copy)]
pub struct ZoneLayout {
    /// Whole calibration area: (start, end). Used when a family has no zone.
    pub calibration: (usize, usize),
    pub boost: Option<(usize, usize)>,
    pub rail_pressure: Option<(usize, usize)>,
    pub injection: Option<(usize, usize)>,
    pub torque: Option<(usize, usize)>,
    pub smoke: Option<(usize, usize)>,
    pub egr: Option<(usize, usize)>,
}

/// Zone layout for a 2 MB CP31 dump.
///
/// CONFIRMED on one real dump (OM642 3.0 CDI 165 kW, SW 1037393817,
/// read with a KESS V2):
///   - dump size is exactly 2 MB (0x200000);
///   - EVERYTHING below 0x190000 is 0xFF. The reader only returns the
///     calibration area and pads the rest, so scanning from 0x180000 (the
///     VAG assumption) wastes 64 KB of padding on every pass;
///   - the single EDC16 checksum region descriptor sits at 0x19003C and
///     declares the region 0x190000..0x1FCFFB inclusive.
///
/// Hence calibration = (0x190000, 0x1FD000). Widen it only if another dump
/// shows real data below 0x190000 (a full-flash read via bench/boot would).
///
/// The per-family windows below come from the same dump. They are generous
/// on purpose - a neighbouring software build moves blocks by a few hundred
/// bytes to a few kilobytes, not across modules.
pub const CP31_ZONES: ZoneLayout = ZoneLayout {
    calibration: (0x190000, 0x1FD000),
    // PCR_* : VNT duty 0x1DB4F0, boost limiter 0x1E3E36, boost target
    // 0x1E4BCE, governor limits 0x1ECEF8.
    boost: Some((0x1DA000, 0x1EE000)),
    // Rail_* : pre-control 0x1F162C .. setpoint corrections 0x1F3040.
    rail_pressure: Some((0x1F0000, 0x1F3400)),
    // InjCrv_phiMI1* : 0x1B07EC .. 0x1B17C8.
    injection: Some((0x1B0000, 0x1B2000)),
    // AccPed_trqEng* 0x1918DE, FlMng_qLim* 0x1A7FE8, FMTC_trq2qBas 0x1A9514.
    torque: Some((0x191000, 0x1AA000)),
    // FlMng_q* smoke block.
    smoke: Some((0x1A7C00, 0x1A9000)),
    // AirCtl_m* air-mass setpoints.
    egr: Some((0x194000, 0x196000)),
};

// ======================== CALIBRATION BLOCK ========================

/// A decoded Bosch `Kf_Xs16_Ys16_Ws16` block.
#[derive(Debug, Clone)]
pub struct CalBlock {
    /// Address of the `nx` word, i.e. of the block itself.
    pub addr: usize,
    pub nx: usize,
    pub ny: usize,
    pub x_axis: Vec<i16>,
    pub y_axis: Vec<i16>,
    /// Address of the first data word.
    pub data_addr: usize,
    /// Data, column direction: `z[x * ny + y]`.
    pub z: Vec<i16>,
}

impl CalBlock {
    /// Byte length of the used part of the block (padding excluded).
    pub const fn len(nx: usize, ny: usize) -> usize {
        CP31_BLOCK_HEADER_LEN + 2 * (nx + ny + nx * ny)
    }

    pub fn end(&self) -> usize {
        self.addr + Self::len(self.nx, self.ny)
    }
}

/// `[nx][ny]`, the two count words every CP31 2D map starts with.
pub const CP31_BLOCK_HEADER_LEN: usize = 4;

/// Largest breakpoint count seen in the project description (a handful of
/// sensor maps declare 32).
const CP31_MAX_AXIS_PTS: usize = 32;

// ============================= DETECTOR =============================

pub struct EDC16CP31Detector {
    tuned_mode: bool,
    /// Runs uncalibrated templates. Corpus-analysis mode: results are
    /// hypotheses to eyeball in the hexdump, never to hand to a user.
    exploratory: bool,
}

impl EDC16CP31Detector {
    pub fn new() -> Self {
        Self { tuned_mode: false, exploratory: false }
    }

    /// Extended physical ranges, for already-modified files.
    pub fn new_tuned() -> Self {
        Self { tuned_mode: true, exploratory: false }
    }

    /// Corpus-analysis mode: ignores the `calibrated` gate so you can see
    /// what the templates would match. Never wire this to the UI.
    pub fn exploratory() -> Self {
        Self { tuned_mode: true, exploratory: true }
    }

    /// True once at least one template has been confirmed on real files.
    pub fn is_calibrated() -> bool {
        MAP_TEMPLATES.iter().any(|t| t.calibrated) || !CP31_SIGNATURES.is_empty()
    }

    /// Main entry point.
    pub fn detect(&self, data: &[u8]) -> Vec<DetectedMap> {
        if !self.exploratory && !Self::is_calibrated() {
            log::warn!(
                "[EDC16CP31] detector is a skeleton: no calibrated template or signature yet, \
                 returning 0 maps. See docs/PORTING-EDC16CP31.md."
            );
            return Vec::new();
        }

        log::debug!(
            "[EDC16CP31] starting detection on {} bytes (tuned: {}, exploratory: {})",
            data.len(),
            self.tuned_mode,
            self.exploratory
        );

        let mut maps: Vec<DetectedMap> = Vec::new();
        let mut claimed: HashSet<(u32, u32)> = HashSet::new();

        // PHASE 1 - marker-anchored detection (highest reliability).
        for map in self.detect_by_signatures(data) {
            self.push_if_free(&mut maps, &mut claimed, map);
        }

        // PHASE 2 - zone-restricted block walk.
        for map in self.detect_by_templates(data) {
            self.push_if_free(&mut maps, &mut claimed, map);
        }

        maps.sort_by_key(|m| m.address);
        log::debug!("[EDC16CP31] detection complete: {} maps", maps.len());
        maps
    }

    fn push_if_free(
        &self,
        maps: &mut Vec<DetectedMap>,
        claimed: &mut HashSet<(u32, u32)>,
        map: DetectedMap,
    ) {
        let range = (map.address, map.address + map.size as u32);
        if claimed.iter().any(|&(s, e)| range.0 < e && s < range.1) {
            return;
        }
        claimed.insert(range);
        maps.push(map);
    }

    // ---------------------- PHASE 1: markers ----------------------

    /// Marker walk. Each `BlockMarker` is the exact head of a confirmed
    /// block, so a hit gives a candidate ADDRESS; the block is then decoded
    /// and validated exactly like in phase 2 before anything is emitted.
    fn detect_by_signatures(&self, data: &[u8]) -> Vec<DetectedMap> {
        let mut out = Vec::new();
        if CP31_MARKERS.is_empty() {
            return out;
        }
        let (scan_start, scan_end) = self.scan_range(data.len());

        for marker in CP31_MARKERS {
            let Some(template) = MAP_TEMPLATES
                .iter()
                .find(|t| Self::marker_belongs_to(marker.label, t))
            else {
                continue;
            };
            if !self.exploratory && !template.calibrated {
                continue;
            }

            let (mut lo, mut hi) = marker.search_window.unwrap_or((scan_start, scan_end));
            lo = lo.max(scan_start);
            hi = hi.min(scan_end).min(data.len());
            if hi <= lo || marker.bytes.is_empty() {
                continue;
            }

            let mut found = 0usize;
            let mut off = lo;
            while off + marker.bytes.len() <= hi {
                if &data[off..off + marker.bytes.len()] == marker.bytes {
                    let block_at = off as isize - marker.offset_in_block;
                    if block_at >= 0 {
                        if let Some(map) = self.try_block(data, block_at as usize, template) {
                            out.push(map);
                            found += 1;
                            if found >= template.max_count {
                                break;
                            }
                        }
                    }
                }
                off += 2;
            }
            log::debug!(
                "[EDC16CP31] marker {} -> {} block(s)",
                marker.label,
                found
            );
        }
        out
    }

    /// Markers are named after the family they announce; the mapping is by
    /// the damos label so that renaming a template cannot silently unhook a
    /// marker.
    fn marker_belongs_to(label: &str, t: &MapTemplate) -> bool {
        match label {
            "cp31_rail_p_setpoint_base" => t.damos_label.starts_with("Rail_pSetPointBase"),
            "cp31_rail_p_setpoint_lim_n" => t.damos_label.starts_with("Rail_pSetPointLimN"),
            "cp31_pcr_p_des_bas" => t.damos_label.starts_with("PCR_pDesBas"),
            "cp31_pcr_p_bdes_max_ap" => t.damos_label.starts_with("PCR_pBDesMaxAP"),
            "cp31_pcr_r_ctl_bas" => t.damos_label.starts_with("PCR_rCtlBas"),
            "cp31_flmng_q_smk" => t.damos_label.starts_with("FlMng_qSmk"),
            "cp31_accped_trq_eng" => t.damos_label.starts_with("AccPed_trqEng"),
            "cp31_flmng_q_lim_bst_pres" => t.damos_label.starts_with("FlMng_qLimBstPres"),
            "cp31_fmtc_trq2q_bas" => t.damos_label.starts_with("FMTC_trq2qBas"),
            "cp31_injcrv_phi_mi1" => t.damos_label.starts_with("InjCrv_phiMI1"),
            "cp31_airctl_m_des_bas" => t.damos_label.starts_with("AirCtl_mDesBas"),
            _ => false,
        }
    }

    // ------------------- PHASE 2: block walk --------------------

    /// Zone-restricted walk over self-describing CP31 blocks.
    fn detect_by_templates(&self, data: &[u8]) -> Vec<DetectedMap> {
        let mut out: Vec<DetectedMap> = Vec::new();
        let (start, end) = self.scan_range(data.len());
        if end <= start || end > data.len() {
            return out;
        }

        let mut templates: Vec<&MapTemplate> = MAP_TEMPLATES
            .iter()
            .filter(|t| self.exploratory || t.calibrated)
            .collect();
        templates.sort_by(|a, b| b.priority.cmp(&a.priority));

        for template in templates {
            let zone = template
                .zone
                .or_else(|| self.zone_for(template))
                .unwrap_or((start, end));
            let (zstart, zend) = (zone.0.max(start), zone.1.min(end));
            if zend <= zstart {
                continue;
            }

            let mut found = 0usize;
            let mut off = zstart;
            while off + CP31_BLOCK_HEADER_LEN < zend && found < template.max_count {
                if let Some(map) = self.try_block(data, off, template) {
                    let step = map.size + CP31_BLOCK_HEADER_LEN;
                    out.push(map);
                    found += 1;
                    off += step;
                    continue;
                }
                off += 2;
            }
        }

        out
    }

    // ------------------------ block decoding ------------------------

    /// Decode the CP31 block at `off` without any interpretation.
    pub fn read_block(data: &[u8], off: usize, max_nx: usize, max_ny: usize) -> Option<CalBlock> {
        let nx = Self::read_u16_be(data, off)? as usize;
        let ny = Self::read_u16_be(data, off + 2)? as usize;
        if nx < 2 || ny < 2 || nx > max_nx || ny > max_ny {
            return None;
        }
        let x_off = off + CP31_BLOCK_HEADER_LEN;
        let y_off = x_off + 2 * nx;
        let data_addr = y_off + 2 * ny;
        let x_axis = Self::read_i16_be_slice(data, x_off, nx)?;
        let y_axis = Self::read_i16_be_slice(data, y_off, ny)?;
        let z = Self::read_i16_be_slice(data, data_addr, nx * ny)?;
        Some(CalBlock { addr: off, nx, ny, x_axis, y_axis, data_addr, z })
    }

    /// Try to read and validate one block at `off` against `template`.
    fn try_block(&self, data: &[u8], off: usize, template: &MapTemplate) -> Option<DetectedMap> {
        let (x_type, y_type) = template.axes;
        let block = Self::read_block(data, off, CP31_MAX_AXIS_PTS, CP31_MAX_AXIS_PTS)?;

        if !template.grids.is_empty()
            && !template.grids.iter().any(|&(nx, ny)| nx == block.nx && ny == block.ny)
        {
            return None;
        }
        if !Self::axis_is_valid(&block.x_axis, x_type)
            || !Self::axis_is_valid(&block.y_axis, y_type)
        {
            return None;
        }

        let values: Vec<f64> = block
            .z
            .iter()
            .map(|&v| {
                let base = if template.signed { v as f64 } else { v as u16 as f64 };
                base * template.z_factor + template.z_offset
            })
            .collect();

        let score = self.score(&values, block.nx, block.ny, template)?;
        Some(self.build_map(&block, template, score))
    }

    fn score(
        &self,
        values: &[f64],
        nx: usize,
        ny: usize,
        template: &MapTemplate,
    ) -> Option<f64> {
        let (lo, hi) = if self.tuned_mode {
            template.z_range_tuned
        } else {
            template.z_range_stock
        };

        // Every cell must be physically plausible: a single wild value means
        // this is not the map, it is code or another structure.
        if values.iter().any(|v| *v < lo || *v > hi) {
            return None;
        }

        // A calibration map is never flat and never noise.
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if (max - min).abs() < f64::EPSILON {
            return None;
        }

        // Rows are laid out along the SECOND axis (Y is the fast index), so
        // one "row" of the file is one engine-speed column of the map.
        let monotonic = Self::row_monotonicity(values, nx, ny);
        Some((0.60 + 0.40 * monotonic).min(0.99))
    }

    /// Share of file rows (one per X breakpoint, `ny` values each) that are
    /// monotonic in either direction.
    fn row_monotonicity(values: &[f64], nx: usize, ny: usize) -> f64 {
        if nx == 0 || ny < 2 || values.len() < nx * ny {
            return 0.0;
        }
        let mut good = 0usize;
        for r in 0..nx {
            let row = &values[r * ny..(r + 1) * ny];
            let up = row.windows(2).all(|w| w[1] >= w[0]);
            let down = row.windows(2).all(|w| w[1] <= w[0]);
            if up || down {
                good += 1;
            }
        }
        good as f64 / nx as f64
    }

    fn build_map(&self, block: &CalBlock, template: &MapTemplate, score: f64) -> DetectedMap {
        let (x_type, y_type) = template.axes;

        // rows = nx (engine speed), cols = ny (load): the file order IS the
        // display order because Z is column-major. The consequence is that
        // the DISPLAY y axis is the block's FIRST axis.
        let mut map = DetectedMap::new(
            block.data_addr as u32,
            block.nx * block.ny * 2,
            MapDimensions::TwoDimensional { rows: block.nx, cols: block.ny },
            if template.signed { DataType::Int16 } else { DataType::UInt16 },
        );

        map.name = Some(template.name.to_string());
        map.category = Some(template.category.display_name().to_string());
        map.unit = Some(template.unit.to_string());
        map.correction_factor = Some(template.z_factor);
        map.offset = Some(template.z_offset);
        map.confidence = score as f32;
        // Display X = block Y (load), display Y = block X (engine speed).
        map.x_axis_address = Some((block.addr + CP31_BLOCK_HEADER_LEN + 2 * block.nx) as u32);
        map.y_axis_address = Some((block.addr + CP31_BLOCK_HEADER_LEN) as u32);
        map.x_axis_correction = Some(y_type.factor());
        map.y_axis_correction = Some(x_type.factor());
        map.x_label = Some(y_type.label().to_string());
        map.y_label = Some(x_type.label().to_string());
        // Both axes are stored ascending and the rows follow the engine-speed
        // axis in the same order, so nothing is inverted on CP31.
        map.y_axis_inverted = Some(false);
        map.description = Some(format!(
            "EDC16CP31 {} - damos family {} - block 0x{:06X}, {}x{} (rows=rpm, cols=load)",
            template.name, template.damos_label, block.addr, block.nx, block.ny
        ));
        map
    }

    // ----------------------- axis validation -----------------------

    fn axis_is_valid(values: &[i16], axis: AxisType) -> bool {
        if axis == AxisType::None {
            return values.is_empty() || values.len() == 1;
        }
        if values.len() < 2 {
            return false;
        }
        let Some((lo, hi)) = axis.raw_range() else {
            return false;
        };
        if values.iter().any(|v| *v < 0) {
            return false;
        }
        if values.iter().any(|v| (*v as u16) < lo || (*v as u16) > hi) {
            return false;
        }
        // Axes are strictly increasing.
        if !values.windows(2).all(|w| w[1] > w[0]) {
            return false;
        }
        // Reject linear ramps that are really counters (0,1,2,3,...).
        let span = values[values.len() - 1] as i32 - values[0] as i32;
        span >= values.len() as i32 * 4
    }

    fn read_u16_be(data: &[u8], off: usize) -> Option<u16> {
        let end = off.checked_add(2)?;
        if end > data.len() {
            return None;
        }
        Some(u16::from_be_bytes([data[off], data[off + 1]]))
    }

    fn read_i16_be_slice(data: &[u8], off: usize, count: usize) -> Option<Vec<i16>> {
        let end = off.checked_add(count.checked_mul(2)?)?;
        if end > data.len() {
            return None;
        }
        Some(
            data[off..end]
                .chunks_exact(2)
                .map(|c| i16::from_be_bytes([c[0], c[1]]))
                .collect(),
        )
    }

    fn zone_for(&self, template: &MapTemplate) -> Option<(usize, usize)> {
        match template.category {
            MapCategory::TurboBoostPressure | MapCategory::TurboBoostPressureControl => {
                CP31_ZONES.boost
            }
            MapCategory::InjectionSystem => CP31_ZONES.rail_pressure,
            MapCategory::StartOfInjection => CP31_ZONES.injection,
            MapCategory::EngineTorqueLimiters
            | MapCategory::EngineTorqueRequest
            | MapCategory::TorqueToIqConversion => CP31_ZONES.torque,
            MapCategory::SmokeLimitation => CP31_ZONES.smoke,
            MapCategory::Egr => CP31_ZONES.egr,
            _ => None,
        }
    }

    /// Calibration area to scan, derived from the dump size.
    fn scan_range(&self, file_size: usize) -> (usize, usize) {
        if file_size >= 0x200000 {
            // Confirmed window, see CP31_ZONES.
            (CP31_ZONES.calibration.0, CP31_ZONES.calibration.1.min(file_size))
        } else if file_size >= 0x100000 {
            (0x80000, file_size)
        } else {
            (0x40000, file_size)
        }
    }
}

impl Default for EDC16CP31Detector {
    fn default() -> Self {
        Self::new()
    }
}

// ================================ TESTS ================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic CP31 block: `[nx][ny][X][Y][Z col-major]`.
    /// Lets the tests exercise the real layout without shipping a dump.
    fn kf_block(x: &[i16], y: &[i16], z: &[i16]) -> Vec<u8> {
        assert_eq!(z.len(), x.len() * y.len());
        let mut v = Vec::new();
        v.extend_from_slice(&(x.len() as u16).to_be_bytes());
        v.extend_from_slice(&(y.len() as u16).to_be_bytes());
        for a in x.iter().chain(y.iter()).chain(z.iter()) {
            v.extend_from_slice(&a.to_be_bytes());
        }
        v
    }

    /// 2 MB image with `block` planted at `addr`, everything else 0xFF like
    /// a real KESS read.
    fn image_with(addr: usize, block: &[u8]) -> Vec<u8> {
        let mut img = vec![0xFFu8; 0x200000];
        img[addr..addr + block.len()].copy_from_slice(block);
        img
    }

    #[test]
    fn zeroed_file_yields_nothing_even_in_exploratory_mode() {
        let data = vec![0u8; 0x200000];
        let maps = EDC16CP31Detector::exploratory().detect(&data);
        assert!(maps.is_empty(), "a zero-filled file has no valid axis");
    }

    #[test]
    fn ff_filled_file_yields_nothing() {
        let data = vec![0xFFu8; 0x200000];
        assert!(EDC16CP31Detector::new().detect(&data).is_empty());
    }

    #[test]
    fn axis_validation_rejects_counters_and_accepts_rpm() {
        let counter: Vec<i16> = (0..12).collect();
        assert!(!EDC16CP31Detector::axis_is_valid(&counter, AxisType::Rpm));

        let rpm: Vec<i16> = vec![750, 1000, 1250, 1500, 1750, 2000, 2500, 3000, 3500, 4000];
        assert!(EDC16CP31Detector::axis_is_valid(&rpm, AxisType::Rpm));

        let out_of_range: Vec<i16> = vec![750, 1000, 9000, 12000];
        assert!(!EDC16CP31Detector::axis_is_valid(&out_of_range, AxisType::Rpm));
    }

    #[test]
    fn big_endian_reader_matches_mpc5xx_layout() {
        let data = [0x0B, 0xB8, 0x0F, 0xA0];
        let vals = EDC16CP31Detector::read_i16_be_slice(&data, 0, 2).unwrap();
        assert_eq!(vals, vec![3000, 4000]);
    }

    #[test]
    fn block_len_accounts_for_header_both_axes_and_data() {
        // 4 header bytes + both axes + the data block.
        assert_eq!(CalBlock::len(10, 10), 4 + (10 + 10 + 100) * 2);
        assert_eq!(CalBlock::len(16, 12), 4 + (16 + 12 + 192) * 2);
    }

    /// The record layout really is self-describing, and Z really is
    /// column-major.
    #[test]
    fn reads_a_synthetic_kf_block() {
        let x: Vec<i16> = vec![800, 1200, 1600];
        let y: Vec<i16> = vec![0, 2000, 4000, 6000];
        // z[x * ny + y]
        let z: Vec<i16> = (0..12).map(|i| 1000 + i as i16 * 10).collect();
        let blk = kf_block(&x, &y, &z);
        assert_eq!(blk.len(), CalBlock::len(3, 4));

        let b = EDC16CP31Detector::read_block(&blk, 0, 32, 32).unwrap();
        assert_eq!((b.nx, b.ny), (3, 4));
        assert_eq!(b.x_axis, x);
        assert_eq!(b.y_axis, y);
        assert_eq!(b.data_addr, 4 + 2 * (3 + 4));
        assert_eq!(b.z, z);
        assert_eq!(b.end(), blk.len());
    }

    /// Layout facts confirmed on a real OM642 dump. If a future dump
    /// contradicts these, this test is where you find out.
    #[test]
    fn confirmed_cp31_layout_constants() {
        // Calibration area starts at 0x190000, not the VAG 0x180000.
        assert_eq!(CP31_ZONES.calibration.0, 0x190000);
        // And stops just past the checksummed region end (0x1FCFFB).
        assert!(CP31_ZONES.calibration.1 > 0x1FCFFB);
        assert!(CP31_ZONES.calibration.1 <= 0x200000);

        // A 2MB dump must scan exactly that window.
        let (start, end) = EDC16CP31Detector::new().scan_range(0x200000);
        assert_eq!(start, 0x190000);
        assert_eq!(end, 0x1FD000);
    }

    /// Every per-family window must sit inside the calibration area, and
    /// every template zone inside its category window.
    #[test]
    fn zones_are_consistent() {
        let (c0, c1) = CP31_ZONES.calibration;
        for z in [
            CP31_ZONES.boost,
            CP31_ZONES.rail_pressure,
            CP31_ZONES.injection,
            CP31_ZONES.torque,
            CP31_ZONES.smoke,
            CP31_ZONES.egr,
        ]
        .into_iter()
        .flatten()
        {
            assert!(z.0 >= c0 && z.1 <= c1 && z.0 < z.1, "{:X?}", z);
        }
        for t in MAP_TEMPLATES {
            if let Some(z) = t.zone {
                assert!(z.0 >= c0 && z.1 <= c1 && z.0 < z.1, "{} {:X?}", t.name, z);
            }
        }
    }

    /// Guard against a template being flipped to calibrated without ranges.
    #[test]
    fn calibrated_templates_have_sane_ranges() {
        for t in MAP_TEMPLATES.iter().filter(|t| t.calibrated) {
            assert!(t.z_range_stock.0 < t.z_range_stock.1, "{}", t.name);
            assert!(t.z_range_tuned.0 <= t.z_range_stock.0, "{}", t.name);
            assert!(t.z_range_tuned.1 >= t.z_range_stock.1, "{}", t.name);
            assert!(!t.grids.is_empty(), "{}", t.name);
            assert!(t.z_factor != 0.0, "{}", t.name);
            assert!(!t.damos_label.is_empty(), "{}", t.name);
        }
    }

    /// Every marker must be wired to a template, and its window must be
    /// inside the calibration area.
    #[test]
    fn markers_are_wired_and_in_range() {
        assert!(!CP31_MARKERS.is_empty());
        for m in CP31_MARKERS {
            assert!(
                MAP_TEMPLATES
                    .iter()
                    .any(|t| EDC16CP31Detector::marker_belongs_to(m.label, t)),
                "marker {} has no template",
                m.label
            );
            assert!(m.bytes.len() >= 6, "marker {} is too short to be safe", m.label);
            let (lo, hi) = m.search_window.expect("every CP31 marker is windowed");
            assert!(lo >= CP31_ZONES.calibration.0 && hi <= CP31_ZONES.calibration.1);
            // The marker starts with the [nx][ny] header of the block it
            // announces, so its first four bytes must be a plausible grid.
            let nx = u16::from_be_bytes([m.bytes[0], m.bytes[1]]) as usize;
            let ny = u16::from_be_bytes([m.bytes[2], m.bytes[3]]) as usize;
            assert!((2..=32).contains(&nx) && (2..=32).contains(&ny), "{}", m.label);
        }
    }

    /// Every signature must name a folder that an actual MapCategory can
    /// produce, so the UI never gets an orphan folder.
    #[test]
    fn signature_folders_match_template_categories() {
        for s in CP31_SIGNATURES {
            assert!(
                MAP_TEMPLATES
                    .iter()
                    .any(|t| t.category.display_name() == s.folder),
                "signature {} has folder {:?} that no template produces",
                s.id_name,
                s.folder
            );
        }
    }

    /// End to end on a synthetic boost-target block planted at the confirmed
    /// address: the detector must find it, scale it, and hand back the two
    /// axis addresses in the CP31 order (y = engine speed = first axis).
    #[test]
    fn detects_a_planted_boost_target() {
        let x: Vec<i16> = vec![751, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2800,
                               3200, 3600, 4000, 4200, 4400, 4600];
        let y: Vec<i16> = vec![0, 500, 1000, 1500, 2000, 2500, 3000, 4000, 5000, 6000,
                               7000, 8000];
        // Rising with load at every engine speed, 1000..2500 hPa.
        let mut z = Vec::new();
        for xi in 0..x.len() {
            for yi in 0..y.len() {
                z.push(1000 + (yi as i16) * 120 + (xi as i16) * 5);
            }
        }
        let addr = 0x1E4BCE;
        let img = image_with(addr, &kf_block(&x, &y, &z));

        let maps = EDC16CP31Detector::new().detect(&img);
        let boost = maps
            .iter()
            .find(|m| m.name.as_deref() == Some("Boost Target"))
            .expect("boost target not detected");

        assert_eq!(boost.address, (addr + 4 + 2 * (16 + 12)) as u32);
        assert_eq!(boost.y_axis_address, Some((addr + 4) as u32));
        assert_eq!(boost.x_axis_address, Some((addr + 4 + 32) as u32));
        assert_eq!(boost.size, 16 * 12 * 2);
        assert!(matches!(
            boost.dimensions,
            MapDimensions::TwoDimensional { rows: 16, cols: 12 }
        ));
        assert_eq!(boost.correction_factor, Some(1.0));
        assert_eq!(boost.unit.as_deref(), Some("hPa"));
        // rows follow the engine-speed axis, cols the load axis
        assert_eq!(boost.y_label.as_deref(), Some("rpm"));
        assert_eq!(boost.x_label.as_deref(), Some("mm^3/cyc"));
        assert!(boost.confidence > 0.9, "confidence {}", boost.confidence);
    }

    /// The same block one byte out of physical range must be rejected: a
    /// wrong factor or a wrong address must fail loudly, not scale silently.
    #[test]
    fn rejects_a_block_outside_the_physical_range() {
        let x: Vec<i16> = vec![751, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2800,
                               3200, 3600, 4000, 4200, 4400, 4600];
        let y: Vec<i16> = vec![0, 500, 1000, 1500, 2000, 2500, 3000, 4000, 5000, 6000,
                               7000, 8000];
        let mut z = Vec::new();
        for xi in 0..x.len() {
            for yi in 0..y.len() {
                z.push(1000 + (yi as i16) * 120 + (xi as i16) * 5);
            }
        }
        // 5000 hPa is not a boost pressure any OM642 makes.
        z[100] = 5000;
        let img = image_with(0x1E4BCE, &kf_block(&x, &y, &z));
        assert!(EDC16CP31Detector::new()
            .detect(&img)
            .iter()
            .all(|m| m.name.as_deref() != Some("Boost Target")));
    }

    /// A block planted outside its family window must not be reported by
    /// that family: zones are a safety net, not only a speed-up.
    #[test]
    fn ignores_a_boost_target_planted_outside_its_zone() {
        let x: Vec<i16> = vec![751, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2800,
                               3200, 3600, 4000, 4200, 4400, 4600];
        let y: Vec<i16> = vec![0, 500, 1000, 1500, 2000, 2500, 3000, 4000, 5000, 6000,
                               7000, 8000];
        let mut z = Vec::new();
        for xi in 0..x.len() {
            for yi in 0..y.len() {
                z.push(1000 + (yi as i16) * 120 + (xi as i16) * 5);
            }
        }
        // 0x1C0000 is inside the calibration area but far from the PCR zone.
        let img = image_with(0x1C0000, &kf_block(&x, &y, &z));
        assert!(EDC16CP31Detector::new()
            .detect(&img)
            .iter()
            .all(|m| m.name.as_deref() != Some("Boost Target")));
    }
}
