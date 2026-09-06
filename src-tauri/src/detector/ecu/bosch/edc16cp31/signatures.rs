// EDC16CP31 map signature database - PORTING SKELETON.
//
// STATUS: EMPTY ON PURPOSE. Nothing here has been derived from real
// Mercedes-Benz dumps yet. The structures below mirror the ones used by the
// VAG detectors (edc16u31/signatures.rs, edc16u34/signatures.rs) so that the
// calibration workflow is identical, but the table itself must be built from
// a corpus before this detector reports anything.
//
// How the VAG tables were produced (see the header of edc16u31/signatures.rs:
// "Generated from analysis of 20 mappack files"):
//   1. Collect N dumps of the SAME ECU family whose map addresses are already
//      known (mappack / damos / A2L, or maps you located by hand and verified
//      on the bench).
//   2. For every known map, record: grid size, address of the map and of both
//      axes, the raw->physical factor, the unit, the byte pattern that
//      precedes the block, and the address window it lives in.
//   3. Aggregate across the corpus: what is constant becomes a signature,
//      what moves becomes an address_range + occurrence_rate.
//
// A signature is only worth adding once it matched every file of the corpus
// and produced ZERO hits on files of other families of the same size.

#![allow(dead_code)]

/// Map signature for EDC16 detection (same shape as the VAG variants).
#[derive(Debug, Clone)]
pub struct EDC16MapSignature {
    pub id_name: &'static str,
    pub display_name: &'static str,
    pub folder: &'static str,
    pub structure_type: StructureType,
    pub cols: DimensionRange,
    pub rows: DimensionRange,
    /// (min, max) size of the data block in bytes.
    pub data_size_range: (usize, usize),
    pub factor: f64,
    pub offset: f64,
    pub unit: &'static str,
    pub signed: bool,
    pub x_axis: AxisSignature,
    pub y_axis: AxisSignature,
    /// Address window where this map is typically found, if stable enough.
    pub address_range: Option<(u32, u32)>,
    /// Share of the corpus in which this map was present (0.0 - 1.0).
    pub occurrence_rate: f32,
    /// Detection priority (higher is checked first).
    pub priority: u8,
}

/// Structure type for EDC16 maps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StructureType {
    /// 2D map with inverted axes (most common on EDC16).
    EZweiInv,
    /// Standard 2D map.
    EZweidim,
    /// 1D curve.
    EEindim,
    /// Single scalar value.
    EEinzel,
}

/// Flexible dimension matching: firmware revisions shift grid sizes by one.
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

/// Axis signature used to confirm a candidate block really is that map.
#[derive(Debug, Clone)]
pub struct AxisSignature {
    pub present: bool,
    pub name: &'static str,
    pub unit: &'static str,
    pub factor: f64,
    pub typical_values: Option<&'static [u16]>,
    /// (min, max) expected RAW values, before applying `factor`.
    pub value_range: Option<(u16, u16)>,
}

/// Byte pattern that marks the start of a known block.
///
/// The VAG detectors use markers such as `[8A 00 00 06]` (U31 driver-wish
/// selector) or `[00 00 00 3C 00 64]` (PD torque limiter). The equivalent
/// markers for CP31 are unknown: they must be extracted from the corpus, not
/// guessed. Copying a VAG marker here would produce confident nonsense.
#[derive(Debug, Clone)]
pub struct BlockMarker {
    pub label: &'static str,
    pub bytes: &'static [u8],
    /// Offset of the marker relative to the start of the block it announces.
    pub offset_in_block: isize,
    pub search_window: Option<(usize, usize)>,
}

/// Signature database for EDC16CP31.
///
/// EMPTY until calibrated. `EDC16CP31Detector::detect` treats an empty table
/// as "not calibrated" and returns no maps rather than falling back to the
/// VAG heuristics, which are tuned to VAG address layouts and would emit
/// plausible-looking but wrong results on a Mercedes file.
pub const CP31_SIGNATURES: &[EDC16MapSignature] = &[
    // TODO(corpus): one entry per confirmed map. Template:
    //
    // EDC16MapSignature {
    //     id_name: "cp31_boost_target",
    //     display_name: "Boost Target",
    //     folder: "Turbo boost pressure",
    //     structure_type: StructureType::EZweiInv,
    //     cols: DimensionRange::range(10, 11, 12),
    //     rows: DimensionRange::range(10, 11, 12),
    //     data_size_range: (200, 288),
    //     factor: 1.0,
    //     offset: 0.0,
    //     unit: "mbar",
    //     signed: false,
    //     x_axis: AxisSignature { present: true, name: "IQ", unit: "mg/stroke",
    //         factor: 0.01, typical_values: None, value_range: Some((0, 9000)) },
    //     y_axis: AxisSignature { present: true, name: "RPM", unit: "rpm",
    //         factor: 1.0, typical_values: None, value_range: Some((500, 5000)) },
    //     address_range: None,
    //     occurrence_rate: 0.0,
    //     priority: 90,
    // },
];

/// Block markers for EDC16CP31. EMPTY until extracted from the corpus.
pub const CP31_MARKERS: &[BlockMarker] = &[];

/// True once the signature database carries at least one confirmed entry.
pub const fn is_signature_db_populated() -> bool {
    !CP31_SIGNATURES.is_empty()
}
