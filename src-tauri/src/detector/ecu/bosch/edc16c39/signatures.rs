// Bosch EDC16C39 (Fiat/Alfa Romeo Multijet) map signatures.
//
// STATUS: CALIBRATED for the Alfa 159/147-class passenger-car layout only.
// See `mod.rs`'s header for the record layout and `docs/PORTING-EDC16C39.md`
// for the full corpus/method writeup.
//
// Corpus for this file: 3 independent real dumps pulled from a large
// third-party tuning archive (not committed, per CONTRIBUTING.md) --
// Alfa Romeo 159 1.9 JTDm (two different software builds) and Alfa Romeo 147
// 1.9 JTD, all genuinely different vehicles/ECU flashes, not re-saves of one
// file. Every address, dimension pair AND axis-key vector below was found
// BYTE-IDENTICAL across all three before being accepted here -- a template
// with axis values that differed between builds was left out rather than
// guessed at (see mod.rs's "not yet calibrated" list).

use crate::models::MapCategory;

/// Physical meaning of one axis. Determines both the accepted raw range (a
/// structural sanity check, independent of any specific file) and the
/// display factor/label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisType {
    /// Engine speed, 1 rpm/bit.
    Rpm,
    /// Accelerator pedal / any duty-cycle ratio: 1/8192 per bit (8192 == 100%).
    Percent,
    /// Injected quantity, matches the Trq2qBas/Rail_Point family convention.
    InjectionQty,
    /// Rail pressure, 0.1 bar/bit.
    RailPressure,
    /// Boost / manifold pressure, 1 hPa absolute per bit.
    BoostPressure,
    /// Air mass, 0.1 mg/Hub per bit.
    AirMass,
    /// Torque set point, 0.1 Nm/bit.
    Torque,
    /// Duration, 2 us/bit (the InjVCD_tiET conversion).
    Microseconds,
}

impl AxisType {
    /// Accepted RAW value window for this axis type, before the factor. A
    /// structural filter, not a guess at any one file's actual breakpoints
    /// -- bounds below are the real observed min/max across all 21
    /// confirmed families' axis arrays (see signatures.rs's module doc),
    /// with a small margin, not copied from another family's (CP31's)
    /// assumptions. `Rpm` in particular is wider than a literal engine-speed
    /// axis needs, because it is reused here for a couple of non-RPM index
    /// axes this pass did not fully re-derive the physical meaning of (see
    /// docs/PORTING-EDC16C39.md's Known Limits) -- a real simplification,
    /// not a guess: every bound is still a real ceiling seen in the corpus.
    pub const fn raw_range(self) -> (i16, i16) {
        match self {
            AxisType::Rpm => (0, 18_100),
            AxisType::Percent => (0, 8_400),
            AxisType::InjectionQty => (0, 7_200),
            AxisType::RailPressure => (0, 8_200),
            AxisType::BoostPressure => (0, 1_200),
            AxisType::AirMass => (0, 5_200),
            AxisType::Torque => (0, 8_400),
            AxisType::Microseconds => (0, 18_100),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            AxisType::Rpm => "rpm",
            AxisType::Percent => "%",
            AxisType::InjectionQty => "mm3/cyc",
            AxisType::RailPressure => "bar",
            AxisType::BoostPressure => "hPa",
            AxisType::AirMass => "mg/Hub",
            AxisType::Torque => "Nm",
            AxisType::Microseconds => "us",
        }
    }

    pub const fn factor(self) -> f64 {
        match self {
            AxisType::Rpm => 1.0,
            AxisType::Percent => 1.0 / 81.92,
            AxisType::InjectionQty => 0.01,
            AxisType::RailPressure => 0.1,
            AxisType::BoostPressure => 1.0,
            AxisType::AirMass => 0.1,
            AxisType::Torque => 0.1,
            AxisType::Microseconds => 2.0,
        }
    }
}

/// One saved axis-key vector: the exact `[nx][ny] + X[] + Y[]` bytes of a
/// real block, read from a real dump. Matched byte for byte at the block
/// head, independently of address, so the family is still identified on a
/// build where the compiler moved every zone -- see mod.rs's
/// `detect_by_axis_keys`. A key is a FILTER, never proof on its own: the
/// block is decoded and range-checked by `try_block` afterwards regardless.
#[derive(Debug, Clone)]
pub struct AxisKey {
    pub nx: usize,
    pub ny: usize,
    /// First axis in memory (engine speed on every calibrated family here).
    pub x: &'static [i16],
    /// Second axis in memory.
    pub y: &'static [i16],
}

impl AxisKey {
    /// Encode the key the way it appears in the file, so a match is a plain
    /// byte comparison at the block head.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + 2 * (self.x.len() + self.y.len()));
        out.extend_from_slice(&(self.nx as u16).to_be_bytes());
        out.extend_from_slice(&(self.ny as u16).to_be_bytes());
        for v in self.x.iter().chain(self.y.iter()) {
            out.extend_from_slice(&v.to_be_bytes());
        }
        out
    }
}

/// Declarative description of one map family. `calibrated` is the gate: a
/// template stays inert (never emitted) until its address, grid, factor and
/// physical range have been confirmed against real EDC16C39 files -- see the
/// corpus note at the top of this file.
#[derive(Debug, Clone)]
pub struct MapTemplate {
    pub name: &'static str,
    /// Real Bosch internal symbol name this family carries (cross-referenced
    /// against an independent third-party ECU database, not invented).
    pub bosch_label: &'static str,
    pub category: MapCategory,
    /// Expected grid, read from the block header and confirmed identical on
    /// all 3 corpus files. A block whose header declares a different (nx,
    /// ny) is rejected outright -- this is the check that keeps the
    /// address-window phase from accepting a coincidental neighbour block
    /// with a differently-shaped calibration.
    pub nx: usize,
    pub ny: usize,
    pub axes: (AxisType, AxisType),
    /// raw * z_factor + z_offset = physical value.
    pub z_factor: f64,
    pub z_offset: f64,
    pub z_range_stock: (f64, f64),
    pub z_range_tuned: (f64, f64),
    pub unit: &'static str,
    /// Whether the raw Z cells are a signed i16 (torque/timing families that
    /// legitimately go negative) or an unsigned u16 (percentages, pressures,
    /// masses -- everything else in this corpus). Determined from the real
    /// ECU database's own declared physical range for each family, not
    /// guessed.
    pub signed: bool,
    /// Confirmed real address (identical across all 3 corpus files). Used as
    /// the center of a small search window in the zone-walk phase, and as a
    /// tie-breaker when an axis key's byte pattern coincidentally repeats
    /// elsewhere in the file.
    pub address: usize,
    pub axis_keys: &'static [AxisKey],
    pub calibrated: bool,
}

include!("signatures_data.rs");
