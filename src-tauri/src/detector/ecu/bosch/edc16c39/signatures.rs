// Bosch EDC16C39 (Fiat/Alfa Romeo Multijet) map signatures.
//
// STATUS: CALIBRATED for the 2MB passenger-car layout shared by Alfa
// 159/147/GT/Brera and Fiat Bravo/Croma/Doblo/Grande Punto/Punto. See
// `mod.rs`'s header for the record layout and `docs/PORTING-EDC16C39.md`
// for the full corpus/method writeup.
//
// Corpus for this file: the 21 families' addresses, grids and first axis
// keys were read byte-identical out of 3 independent real dumps (Alfa
// Romeo 159, two software builds, and Alfa Romeo 147). The additional axis
// keys in `signatures_data.rs` were then harvested from a further 39 real
// 2MB dumps across the models above -- each one is the exact grid read at
// that family's confirmed slot on one real build, never typed in. A family
// whose grid differed between builds in SHAPE was left out rather than
// guessed at (see mod.rs's "Known limits").
//
// Physical meaning of every axis was cross-checked against an independent
// third-party ECU reference database's declared axis units for this chip,
// not inferred from the numbers alone.

use crate::models::MapCategory;

/// Physical meaning of one axis. Determines both the accepted raw range (a
/// structural sanity check, independent of any specific file) and the
/// display factor/offset/label.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AxisType {
    /// Engine speed, 1 rpm/bit.
    Rpm,
    /// Accelerator pedal / any duty-cycle ratio: 1/8192 per bit (8192 == 100%).
    Percent,
    /// Injected quantity, 0.01 mm3/stroke per bit.
    InjectionQty,
    /// Rail pressure, 0.1 bar/bit.
    RailPressure,
    /// Boost / ambient / manifold pressure, 1 hPa absolute per bit.
    BoostPressure,
    /// Air mass, 0.1 mg/stroke per bit.
    AirMass,
    /// Torque, 0.1 Nm/bit.
    Torque,
    /// Temperature, 0.1 K per bit, shown in degrees C.
    Temperature,
}

impl AxisType {
    /// Accepted RAW value window for this axis type, before the factor. A
    /// structural filter that a real breakpoint vector must pass, so it is
    /// set from what is physically possible for that quantity on this
    /// engine class, not from any one file -- and every axis key harvested
    /// from a real dump must pass it (test
    /// `every_axis_key_passes_its_templates_axis_gates`; an earlier
    /// revision's tighter "observed" bounds silently rejected four real
    /// Brera/Doblo grids and lost those builds' maps).
    pub const fn raw_range(self) -> (i16, i16) {
        match self {
            // 8000 rpm: no Multijet diesel here revs past ~5500; the widest
            // real breakpoint vector (driver wish) ends at 6200.
            AxisType::Rpm => (0, 8_000),
            // 8192 == 100 %; real vectors reach 8356 (a few % of overshoot).
            AxisType::Percent => (0, 8_400),
            // 120 mm3/stroke; real vectors reach 100 (injector-duration map).
            AxisType::InjectionQty => (0, 12_000),
            // 2200 bar; real vectors reach 2000.
            AxisType::RailPressure => (0, 22_000),
            // 4000 hPa absolute; real vectors (ambient pressure) reach 1086.
            AxisType::BoostPressure => (0, 4_000),
            // 1500 mg/stroke; real vectors (smoke limiter) reach 1050.
            AxisType::AirMass => (0, 15_000),
            // 800 Nm; real vectors reach 500.
            AxisType::Torque => (0, 8_000),
            // -50 .. 150 degC in tenths of kelvin; real vectors span 30..110.
            AxisType::Temperature => (2_231, 4_231),
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
            AxisType::Temperature => "degC",
        }
    }

    /// displayed = raw * factor() + offset()
    pub const fn factor(self) -> f64 {
        match self {
            AxisType::Rpm => 1.0,
            AxisType::Percent => 1.0 / 81.92,
            AxisType::InjectionQty => 0.01,
            AxisType::RailPressure => 0.1,
            AxisType::BoostPressure => 1.0,
            AxisType::AirMass => 0.1,
            AxisType::Torque => 0.1,
            AxisType::Temperature => 0.1,
        }
    }

    pub const fn offset(self) -> f64 {
        match self {
            AxisType::Temperature => -273.1,
            _ => 0.0,
        }
    }
}

/// One saved axis-key vector: the exact `[nx][ny] + X[] + Y[]` bytes of a
/// real block, read from a real dump. Matched byte for byte at the block
/// head, independently of address, so the family is still identified on a
/// build where the compiler moved every zone -- see mod.rs's
/// `axis_key_hits`. A key is a FILTER, never proof on its own: the block is
/// decoded and range-checked by `try_block` afterwards regardless.
#[derive(Debug, Clone)]
pub struct AxisKey {
    pub nx: usize,
    pub ny: usize,
    /// First axis in memory -- engine speed on every family here except
    /// `InjVCD_tiET`, which is indexed by rail pressure (see its template).
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
    /// every corpus file where the family was found. A block whose header
    /// declares a different (nx, ny) is rejected outright -- this is the
    /// check that keeps the address-window phase from accepting a
    /// coincidental neighbour block with a differently-shaped calibration.
    pub nx: usize,
    pub ny: usize,
    /// (first axis in memory, second axis in memory) -- physical meaning
    /// from the reference database's declared axis units.
    pub axes: (AxisType, AxisType),
    /// raw * z_factor + z_offset = physical value.
    pub z_factor: f64,
    pub z_offset: f64,
    pub z_range_stock: (f64, f64),
    pub z_range_tuned: (f64, f64),
    pub unit: &'static str,
    /// Whether the raw Z cells are a signed i16 (torque/timing families that
    /// legitimately go negative) or an unsigned u16 (percentages, pressures,
    /// masses). Determined from the reference database's declared physical
    /// range AND checked against real cells: `Trq2qBas` carries raw values
    /// above 0x7FFF on 4 real files that decode to a plausible -0.85 mm3
    /// signed and an impossible 655 mm3 unsigned, so it is signed although
    /// the database lists its minimum as 0.
    pub signed: bool,
    /// Confirmed real address on the corpus. Center of the outward search
    /// in the address phase, and the tie-breaker when an axis key's byte
    /// pattern repeats elsewhere in the file.
    pub address: usize,
    pub axis_keys: &'static [AxisKey],
    pub calibrated: bool,
}

include!("signatures_data.rs");
