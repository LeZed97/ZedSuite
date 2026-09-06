// Bosch EDC16CP31 detector (Mercedes-Benz OM642 / OM646 common rail).
// PORTING SKELETON - see docs/PORTING-EDC16CP31.md.
//
// What is reusable from the VAG detectors and what is not
// -------------------------------------------------------
// REUSABLE (implemented below, family-generic):
//   - big-endian u16 decoding, MPC5xx layout assumptions
//   - the axis-first scan: find a plausible axis pair, then validate the data
//     block that follows it against a physical range
//   - the scoring model (range fit + monotonicity)
//   - checksum handling: EDC16 region descriptors are self-describing
//     (magic FA DE CA FE CA FE AF FE at region_start + 0x3C), so
//     src/lib/ecu/bosch/checksums/edc16-checksum.ts already routes any type
//     whose name contains "EDC16" - CP31 included - with no change.
//     STILL MUST BE VALIDATED against one stock/modified CP31 pair.
//
// NOT REUSABLE (must come from a CP31 corpus):
//   - every address window. The VAG modules hardcode zones such as
//     turbo 0x1DCxxx (U31) / 0x1E3xxx (U34); those are VAG software layouts.
//   - every byte marker ([8A 00 00 06], [00 00 00 3C 00 64], ...).
//   - the raw->physical factors. A wrong factor does not fail loudly, it
//     silently mis-scales the map, which is the single most dangerous
//     failure mode of this whole engine.
//
// Safety contract of this module
// ------------------------------
// Until MAP_TEMPLATES carries calibrated entries, `detect()` returns an empty
// vector. It never falls back to the VAG detectors. Per CONTRIBUTING.md:
// "A false 'unsupported file' is annoying; a false 'this is an EDC16'
// corrupts someone's ECU."

mod signatures;

pub use signatures::{CP31_MARKERS, CP31_SIGNATURES, EDC16MapSignature, StructureType};

use crate::models::{DataType, DetectedMap, MapCategory, MapDimensions};
use std::collections::HashSet;

// ============================= AXIS TYPES =============================

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisType {
    /// Engine speed, 500-5000 rpm, 1 rpm/bit.
    Rpm,
    /// Accelerator pedal, 0-100 %, 0.01 %/bit.
    Pedal,
    /// Injected quantity, 0-90 mg/stroke, 0.01 mg/bit.
    InjectionQty,
    /// Rail pressure, common rail only, 1 bar/bit.
    RailPressure,
    /// Air mass (MAF), 0-1500 mg/stroke.
    AirMass,
    /// Torque set point, 0.1 Nm/bit.
    Torque,
    /// Atmospheric / ambient pressure, 1 mbar/bit.
    AtmosphericPressure,
    /// Coolant / intake temperature, 0.1 degC/bit with offset.
    Temperature,
    /// No axis (scalar).
    None,
}

impl AxisType {
    /// Accepted RAW value window for this axis type, before the factor.
    /// Returns None for `AxisType::None`.
    const fn raw_range(self) -> Option<(u16, u16)> {
        match self {
            AxisType::Rpm => Some((400, 5600)),
            AxisType::Pedal => Some((0, 10_200)),
            AxisType::InjectionQty => Some((0, 9_500)),
            AxisType::RailPressure => Some((150, 2_000)),
            AxisType::AirMass => Some((0, 16_000)),
            AxisType::Torque => Some((0, 8_000)),
            AxisType::AtmosphericPressure => Some((600, 1_150)),
            AxisType::Temperature => Some((0, 4_000)),
            AxisType::None => None,
        }
    }

    const fn factor(self) -> f64 {
        match self {
            AxisType::Rpm => 1.0,
            AxisType::Pedal => 0.01,
            AxisType::InjectionQty => 0.01,
            AxisType::RailPressure => 1.0,
            AxisType::AirMass => 0.1,
            AxisType::Torque => 0.1,
            AxisType::AtmosphericPressure => 1.0,
            AxisType::Temperature => 0.1,
            AxisType::None => 1.0,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            AxisType::Rpm => "rpm",
            AxisType::Pedal => "%",
            AxisType::InjectionQty => "mg/stroke",
            AxisType::RailPressure => "bar",
            AxisType::AirMass => "mg",
            AxisType::Torque => "Nm",
            AxisType::AtmosphericPressure => "mbar",
            AxisType::Temperature => "degC",
            AxisType::None => "",
        }
    }
}

// ============================ MAP TEMPLATES ============================

/// Declarative description of one map family.
///
/// `calibrated` is the gate: a template stays inert until its grids, factor
/// and physical ranges have been confirmed against real CP31 files. Flip it
/// to `true` only with a dump in front of you.
#[derive(Debug, Clone)]
pub struct MapTemplate {
    pub name: &'static str,
    pub category: MapCategory,
    /// Accepted grid sizes as (rows, cols).
    pub grids: &'static [(usize, usize)],
    /// (Y axis type, X axis type) - Y is the row axis, X the column axis.
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
    /// False = hypothesis only, ignored by `detect()`.
    pub calibrated: bool,
}

/// Map families expected on an OM642-class EDC16CP31.
///
/// EVERY ENTRY IS AN UNCALIBRATED HYPOTHESIS. The grids, factors and ranges
/// below are starting points for corpus analysis, NOT verified CP31 values -
/// they are extrapolated from the shape of the VAG EDC16 templates and from
/// what a common-rail V6 diesel plausibly runs. Confirm each one against a
/// real dump, then set `calibrated: true` on that entry alone.
pub const MAP_TEMPLATES: &[MapTemplate] = &[
    MapTemplate {
        name: "Rail Pressure Target",
        category: MapCategory::InjectionSystem,
        grids: &[(12, 12), (11, 12), (12, 16), (16, 16)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 1.0,
        z_offset: 0.0,
        z_range_stock: (200.0, 1650.0),
        z_range_tuned: (200.0, 1900.0),
        unit: "bar",
        signed: false,
        max_count: 3,
        priority: 95,
        calibrated: false,
    },
    MapTemplate {
        name: "Rail Pressure Limiter",
        category: MapCategory::InjectionSystem,
        grids: &[(1, 12), (1, 16)],
        axes: (AxisType::None, AxisType::Rpm),
        z_factor: 1.0,
        z_offset: 0.0,
        z_range_stock: (200.0, 1700.0),
        z_range_tuned: (200.0, 1950.0),
        unit: "bar",
        signed: false,
        max_count: 2,
        priority: 90,
        calibrated: false,
    },
    MapTemplate {
        name: "Boost Target",
        category: MapCategory::TurboBoostPressure,
        grids: &[(11, 10), (10, 10), (12, 12), (16, 16)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 1.0,
        z_offset: 0.0,
        z_range_stock: (900.0, 2500.0),
        z_range_tuned: (900.0, 2900.0),
        unit: "mbar",
        signed: false,
        max_count: 3,
        priority: 92,
        calibrated: false,
    },
    MapTemplate {
        name: "Boost Limiter",
        category: MapCategory::TurboBoostPressure,
        grids: &[(11, 10), (10, 10), (10, 11)],
        axes: (AxisType::Rpm, AxisType::AtmosphericPressure),
        z_factor: 1.0,
        z_offset: 0.0,
        z_range_stock: (1100.0, 2800.0),
        z_range_tuned: (1100.0, 3200.0),
        unit: "mbar",
        signed: false,
        max_count: 2,
        priority: 88,
        calibrated: false,
    },
    MapTemplate {
        name: "VNT Duty Cycle",
        category: MapCategory::TurboBoostPressureControl,
        grids: &[(16, 16), (12, 12), (11, 10)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.01,
        z_offset: 0.0,
        z_range_stock: (0.0, 100.0),
        z_range_tuned: (0.0, 100.0),
        unit: "%",
        signed: false,
        max_count: 4,
        priority: 80,
        calibrated: false,
    },
    MapTemplate {
        name: "Smoke Limiter by MAF",
        category: MapCategory::SmokeLimitation,
        grids: &[(12, 12), (11, 12), (16, 16)],
        axes: (AxisType::Rpm, AxisType::AirMass),
        z_factor: 0.01,
        z_offset: 0.0,
        z_range_stock: (0.0, 90.0),
        z_range_tuned: (0.0, 120.0),
        unit: "mg/stroke",
        signed: false,
        max_count: 3,
        priority: 90,
        calibrated: false,
    },
    MapTemplate {
        name: "Driver Wish",
        category: MapCategory::EngineTorqueRequest,
        grids: &[(16, 16), (12, 16), (12, 12)],
        axes: (AxisType::Rpm, AxisType::Pedal),
        z_factor: 0.1,
        z_offset: 0.0,
        z_range_stock: (0.0, 700.0),
        z_range_tuned: (0.0, 900.0),
        unit: "Nm",
        signed: false,
        max_count: 4,
        priority: 85,
        calibrated: false,
    },
    MapTemplate {
        name: "Torque Limiter",
        category: MapCategory::EngineTorqueLimiters,
        grids: &[(12, 12), (11, 12), (16, 16)],
        axes: (AxisType::Rpm, AxisType::AtmosphericPressure),
        z_factor: 0.1,
        z_offset: 0.0,
        z_range_stock: (0.0, 700.0),
        z_range_tuned: (0.0, 950.0),
        unit: "Nm",
        signed: false,
        max_count: 4,
        priority: 87,
        calibrated: false,
    },
    MapTemplate {
        name: "Torque to IQ Conversion",
        category: MapCategory::TorqueToIqConversion,
        grids: &[(16, 16), (12, 12)],
        axes: (AxisType::Rpm, AxisType::Torque),
        z_factor: 0.01,
        z_offset: 0.0,
        z_range_stock: (0.0, 90.0),
        z_range_tuned: (0.0, 120.0),
        unit: "mg/stroke",
        signed: false,
        max_count: 2,
        priority: 82,
        calibrated: false,
    },
    MapTemplate {
        name: "Start of injection",
        category: MapCategory::StartOfInjection,
        grids: &[(12, 12), (16, 16), (11, 12)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.01,
        z_offset: 0.0,
        z_range_stock: (-15.0, 40.0),
        z_range_tuned: (-20.0, 45.0),
        unit: "degCA",
        signed: true,
        max_count: 8,
        priority: 84,
        calibrated: false,
    },
    MapTemplate {
        name: "EGR Target",
        category: MapCategory::Egr,
        grids: &[(12, 12), (16, 16)],
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.1,
        z_offset: 0.0,
        z_range_stock: (0.0, 1200.0),
        z_range_tuned: (0.0, 1200.0),
        unit: "mg/stroke",
        signed: false,
        max_count: 4,
        priority: 78,
        calibrated: false,
    },
];

// ============================== ZONES ==============================

/// Address windows of the calibration area, per map family.
///
/// The VAG modules hardcode these (84 distinct addresses in edc16u31/mod.rs
/// alone). None of them transfer. Fill these in once you can locate the
/// families on a real CP31 dump; `None` means "scan the whole calibration
/// area", which is correct but slower and more false-positive prone.
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
pub const CP31_ZONES: ZoneLayout = ZoneLayout {
    calibration: (0x190000, 0x1FD000),
    boost: None,
    rail_pressure: None,
    injection: None,
    torque: None,
    smoke: None,
    egr: None,
};

/// Order in which the two axes are laid out ahead of the data block.
/// Both orders exist across Bosch EDC16 software; the detector tries both
/// until the corpus tells you which one CP31 uses.
#[derive(Debug, Clone, Copy, PartialEq)]
enum LayoutOrder {
    /// [X axis][Y axis][data]
    XThenY,
    /// [Y axis][X axis][data]
    YThenX,
}

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

    /// Main entry point. Returns an empty vector while the family is
    /// uncalibrated - by design, see the safety contract at the top.
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

        // PHASE 1 - signature-based detection (highest reliability).
        for map in self.detect_by_signatures(data) {
            self.push_if_free(&mut maps, &mut claimed, map);
        }

        // PHASE 2 - axis-first template scan.
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

    // ---------------------- PHASE 1: signatures ----------------------

    /// Signature pass. Inert while `CP31_SIGNATURES` is empty.
    ///
    /// TODO(corpus): implement the marker walk once `CP31_MARKERS` carries
    /// real byte patterns. See `detect_n75_by_signature` in edc16u34/mod.rs
    /// for the reference implementation.
    fn detect_by_signatures(&self, _data: &[u8]) -> Vec<DetectedMap> {
        if CP31_SIGNATURES.is_empty() && CP31_MARKERS.is_empty() {
            return Vec::new();
        }
        log::warn!("[EDC16CP31] signature table populated but the walk is not implemented yet");
        Vec::new()
    }

    // ------------------- PHASE 2: axis-first scan --------------------

    /// Generic axis-first scan: this part IS family-agnostic and works as
    /// soon as the templates carry correct grids, factors and ranges.
    ///
    /// For each candidate offset it reads, in both layout orders,
    /// [axis][axis][data] and keeps the combination whose axes are
    /// monotonic and in range AND whose data block falls inside the
    /// template's physical range.
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
            let mut found = 0usize;
            let zone = self.zone_for(template).unwrap_or((start, end));
            let (zstart, zend) = (zone.0.max(start), zone.1.min(end));

            for &(rows, cols) in template.grids {
                if found >= template.max_count {
                    break;
                }
                for order in [LayoutOrder::YThenX, LayoutOrder::XThenY] {
                    let mut off = zstart;
                    while off + Self::block_len(rows, cols) <= zend {
                        if let Some(map) =
                            self.try_block(data, off, rows, cols, template, order)
                        {
                            out.push(map);
                            found += 1;
                            off += Self::block_len(rows, cols);
                            if found >= template.max_count {
                                break;
                            }
                            continue;
                        }
                        off += 2;
                    }
                }
            }
        }

        out
    }

    const fn block_len(rows: usize, cols: usize) -> usize {
        (rows + cols + rows * cols) * 2
    }

    /// Try to read one [axis][axis][data] block at `off`.
    fn try_block(
        &self,
        data: &[u8],
        off: usize,
        rows: usize,
        cols: usize,
        template: &MapTemplate,
        order: LayoutOrder,
    ) -> Option<DetectedMap> {
        let (y_type, x_type) = template.axes;

        let (y_off, x_off) = match order {
            LayoutOrder::YThenX => (off, off + rows * 2),
            LayoutOrder::XThenY => (off + cols * 2, off),
        };
        let data_off = off + (rows + cols) * 2;

        let y_axis = Self::read_u16_be_slice(data, y_off, rows)?;
        let x_axis = Self::read_u16_be_slice(data, x_off, cols)?;

        if !Self::axis_is_valid(&y_axis, y_type) || !Self::axis_is_valid(&x_axis, x_type) {
            return None;
        }

        let raw = Self::read_u16_be_slice(data, data_off, rows * cols)?;
        let values: Vec<f64> = raw
            .iter()
            .map(|&v| {
                let base = if template.signed { v as i16 as f64 } else { v as f64 };
                base * template.z_factor + template.z_offset
            })
            .collect();

        let score = self.score(&values, rows, cols, template)?;

        Some(self.build_map(
            data_off, y_off, x_off, rows, cols, template, score,
        ))
    }

    fn score(
        &self,
        values: &[f64],
        rows: usize,
        cols: usize,
        template: &MapTemplate,
    ) -> Option<f64> {
        let (lo, hi) = if self.tuned_mode {
            template.z_range_tuned
        } else {
            template.z_range_stock
        };

        let in_range = values.iter().filter(|v| **v >= lo && **v <= hi).count();
        let ratio = in_range as f64 / values.len() as f64;
        // Every cell must be physically plausible: a single wild value means
        // this is not the map, it is code or another structure.
        if ratio < 1.0 {
            return None;
        }

        // A calibration map is never flat and never noise.
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if (max - min).abs() < f64::EPSILON {
            return None;
        }

        let monotonic = Self::row_monotonicity(values, rows, cols);
        Some((0.60 + 0.40 * monotonic).min(0.99))
    }

    /// Share of rows that are monotonic (either direction).
    fn row_monotonicity(values: &[f64], rows: usize, cols: usize) -> f64 {
        if rows == 0 || cols < 2 {
            return 0.0;
        }
        let mut good = 0usize;
        for r in 0..rows {
            let row = &values[r * cols..(r + 1) * cols];
            let up = row.windows(2).all(|w| w[1] >= w[0]);
            let down = row.windows(2).all(|w| w[1] <= w[0]);
            if up || down {
                good += 1;
            }
        }
        good as f64 / rows as f64
    }

    fn build_map(
        &self,
        data_off: usize,
        y_off: usize,
        x_off: usize,
        rows: usize,
        cols: usize,
        template: &MapTemplate,
        score: f64,
    ) -> DetectedMap {
        let (y_type, x_type) = template.axes;

        let mut map = DetectedMap::new(
            data_off as u32,
            rows * cols * 2,
            MapDimensions::TwoDimensional { rows, cols },
            if template.signed { DataType::Int16 } else { DataType::UInt16 },
        );

        map.name = Some(template.name.to_string());
        map.category = Some(template.category.display_name().to_string());
        map.unit = Some(template.unit.to_string());
        map.correction_factor = Some(template.z_factor);
        map.offset = Some(template.z_offset);
        map.confidence = score as f32;
        map.x_axis_address = Some(x_off as u32);
        map.y_axis_address = Some(y_off as u32);
        map.x_axis_correction = Some(x_type.factor());
        map.y_axis_correction = Some(y_type.factor());
        map.x_label = Some(x_type.label().to_string());
        map.y_label = Some(y_type.label().to_string());
        // TODO(corpus): confirm per family. On the VAG EDC16 modules most
        // rpm-on-Y maps are displayed inverted; do not assume it holds here.
        map.y_axis_inverted = Some(false);
        map.description = Some(
            "EDC16CP31 skeleton detection - verify address and factor before editing".to_string(),
        );
        map
    }

    // ----------------------- axis validation -----------------------

    fn axis_is_valid(values: &[u16], axis: AxisType) -> bool {
        if axis == AxisType::None {
            return values.is_empty() || values.len() == 1;
        }
        if values.len() < 2 {
            return false;
        }
        let Some((lo, hi)) = axis.raw_range() else {
            return false;
        };
        if values.iter().any(|v| *v < lo || *v > hi) {
            return false;
        }
        // Axes are strictly increasing, with no repeated plateau at the head.
        if !values.windows(2).all(|w| w[1] > w[0]) {
            return false;
        }
        // Reject linear ramps that are really counters (0,1,2,3,...).
        let span = values[values.len() - 1] as i32 - values[0] as i32;
        span >= values.len() as i32 * 4
    }

    fn read_u16_be_slice(data: &[u8], off: usize, count: usize) -> Option<Vec<u16>> {
        let end = off.checked_add(count * 2)?;
        if end > data.len() {
            return None;
        }
        Some(
            data[off..end]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
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

    /// The safety contract: an uncalibrated detector reports nothing.
    /// Delete this test the day you calibrate the first template - and
    /// replace it with a fixture test on a real dump.
    #[test]
    fn uncalibrated_detector_reports_no_maps() {
        let data = vec![0u8; 0x200000];
        let maps = EDC16CP31Detector::new().detect(&data);
        assert!(
            maps.is_empty(),
            "an uncalibrated CP31 detector must never emit maps"
        );
    }

    #[test]
    fn zeroed_file_yields_nothing_even_in_exploratory_mode() {
        let data = vec![0u8; 0x200000];
        let maps = EDC16CP31Detector::exploratory().detect(&data);
        assert!(maps.is_empty(), "a zero-filled file has no valid axis");
    }

    #[test]
    fn axis_validation_rejects_counters_and_accepts_rpm() {
        let counter: Vec<u16> = (0..12).collect();
        assert!(!EDC16CP31Detector::axis_is_valid(&counter, AxisType::Rpm));

        let rpm: Vec<u16> = vec![750, 1000, 1250, 1500, 1750, 2000, 2500, 3000, 3500, 4000];
        assert!(EDC16CP31Detector::axis_is_valid(&rpm, AxisType::Rpm));

        let out_of_range: Vec<u16> = vec![750, 1000, 9000, 12000];
        assert!(!EDC16CP31Detector::axis_is_valid(&out_of_range, AxisType::Rpm));
    }

    #[test]
    fn big_endian_reader_matches_mpc5xx_layout() {
        let data = [0x0B, 0xB8, 0x0F, 0xA0];
        let vals = EDC16CP31Detector::read_u16_be_slice(&data, 0, 2).unwrap();
        assert_eq!(vals, vec![3000, 4000]);
    }

    #[test]
    fn block_len_accounts_for_both_axes_and_data() {
        assert_eq!(EDC16CP31Detector::block_len(10, 10), (10 + 10 + 100) * 2);
    }

    /// Locks the layout facts confirmed on a real OM642 dump. If a future
    /// dump contradicts these, this test is where you find out.
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

    /// Guard against a template being flipped to calibrated without ranges.
    #[test]
    fn calibrated_templates_have_sane_ranges() {
        for t in MAP_TEMPLATES.iter().filter(|t| t.calibrated) {
            assert!(t.z_range_stock.0 < t.z_range_stock.1, "{}", t.name);
            assert!(t.z_range_tuned.0 <= t.z_range_stock.0, "{}", t.name);
            assert!(t.z_range_tuned.1 >= t.z_range_stock.1, "{}", t.name);
            assert!(!t.grids.is_empty(), "{}", t.name);
            assert!(t.z_factor != 0.0, "{}", t.name);
        }
    }
}
