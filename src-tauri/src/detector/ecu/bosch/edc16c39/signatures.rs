// Bosch EDC16C39 (Fiat/Alfa Romeo Multijet) map signatures.
//
// STATUS: CALIBRATED for the 2MB passenger-car layout shared by Alfa
// 159/147/GT/Brera and Fiat Bravo/Croma/Doblo/Grande Punto/Punto. See
// `mod.rs`'s header for the record layouts and `docs/PORTING-EDC16C39.md`
// for the full corpus/method writeup.
//
// Corpus for this file: the original families' addresses, grids and first
// axis keys were read byte-identical out of 3 independent real dumps (Alfa
// Romeo 159, two software builds, and Alfa Romeo 147). Every further axis
// key in `signatures_data.rs` was harvested from the 42 real 2MB dumps of
// the corpus -- each one is the exact grid read at that family's slot on one
// real build, never typed in.
//
// Physical meaning, address and grid of every family carrying
// `symbol_confirmed: true` (30 of 49) were cross-checked against an
// independent third-party ECU reference database (ECM Titanium)'s
// `named_maps` variant, which carries the real per-chip Bosch internal
// symbol: 222 EDC16C39 builds, one of them the exact software build of a
// corpus file, where all 32 of its entries decode at their declared
// addresses.
//
// The remaining 19 (`EgrHys1..4`, `IA_FuelAccel1..8`, `I3_InjCrvCorr1..5`,
// `BS_TurboPressureCorr1..2`) were found the same way but confirmed against
// the SAME reference database's OTHER variant, `axis_table` (361 EDC16C39
// builds) -- a categorical English description per driver entry rather than
// a Bosch symbol, so `symbol_confirmed` is false on these and their
// description says so rather than inventing a symbol. Kept to a strict bar
// after a first pass over-collected: only a stride-consistent run with
// independent multi-file support per member was trusted (the same standard
// `InjCrv_Bas1..5` was held to); isolated single addresses that only
// matched via this platform's widely shared RPM/generic breakpoint grid
// were left out, and one initially-accepted 4-member candidate was
// withdrawn after cross-checking the reference database's OWN declared
// address for that exact build showed it was the EXISTING `AccPed_trq*`
// family at its real Grande Punto/Punto address (+0xFC), not a new one.
//
// Every one of this family's 49 templates traces back to a real ECM
// Titanium DRT record; none to a damos or A2L (none reachable covers this
// chip -- see docs/PORTING-EDC16C39.md).

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
            // real breakpoint vector (the smoke-limiter curve) ends at 7000.
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

/// On-disk record kind of one family. Both are self-describing and
/// big-endian; see mod.rs's header.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Shape {
    /// `[u16 nx][u16 ny][i16 X nx][i16 Y ny][i16 Z nx*ny]`
    Map,
    /// `[u16 n][i16 X n][i16 Z n]` -- a curve over one axis. The template's
    /// `nx` is `n`, `ny` is 1 and `axes.1` is ignored. Confirmed against the
    /// reference database's 1D entries (`EngPrt_trqLim`, `TrqMaxGear1..6/R`)
    /// on the exact build it shares with the corpus.
    Curve,
}

/// One saved axis-key vector: the exact header + axis bytes of a real
/// record, read from a real dump -- `[nx][ny] + X[] + Y[]` for a map,
/// `[n] + X[]` for a curve (`ny == 0`, `y` empty). Matched byte for byte at
/// the record head, independently of address, so the family is still
/// identified on a build where the compiler moved every zone -- see mod.rs's
/// `axis_key_hits`. A key is a FILTER, never proof on its own: the record is
/// decoded and range-checked by `try_block` afterwards regardless.
#[derive(Debug, Clone)]
pub struct AxisKey {
    pub nx: usize,
    /// 0 marks a curve key: no second header word, no Y axis.
    pub ny: usize,
    /// First axis in memory -- engine speed on every family here except
    /// `InjVCD_tiET`, which is indexed by rail pressure (see its template).
    pub x: &'static [i16],
    /// Second axis in memory (empty for a curve key).
    pub y: &'static [i16],
}

impl AxisKey {
    /// Encode the key the way it appears in the file, so a match is a plain
    /// byte comparison at the record head.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(4 + 2 * (self.x.len() + self.y.len()));
        out.extend_from_slice(&(self.nx as u16).to_be_bytes());
        if self.ny > 0 {
            out.extend_from_slice(&(self.ny as u16).to_be_bytes());
        }
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
    /// Real Bosch internal symbol when `symbol_confirmed` (cross-referenced
    /// against the reference database, not invented); otherwise an id of
    /// ours, never shown as a Bosch symbol. Unique across templates.
    pub bosch_label: &'static str,
    pub symbol_confirmed: bool,
    pub category: MapCategory,
    pub shape: Shape,
    /// Grid read from the record header on the corpus (`ny` is 1 for a
    /// curve). A record declaring a grid outside `nx`/`ny` and `alt_grids`
    /// is rejected outright -- the check that keeps the address-window phase
    /// from accepting a coincidental neighbour with a differently-shaped
    /// calibration.
    pub nx: usize,
    pub ny: usize,
    /// Further grids the reference database declares for this family on
    /// other real builds (e.g. `TrqStrtBas` exists in nine grids across its
    /// 222 builds). Empty unless that evidence exists.
    pub alt_grids: &'static [(usize, usize)],
    /// (first axis in memory, second axis in memory) -- physical meaning
    /// from the reference database's declared axis units. The second is
    /// ignored for a curve.
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
    /// Accept a record whose Z data is one constant. Off by default (a flat
    /// block is the usual signature of an erased or unrelated zone); on only
    /// where real files show the family flat: limiters left at a single
    /// ceiling on stock files, and EGR thresholds that every real EGR-off
    /// tune in the corpus sets to 0.
    pub allow_flat: bool,
    /// Confirmed real address on the corpus. Center of the outward search
    /// in the address phase, and the tie-breaker when an axis key's byte
    /// pattern repeats elsewhere in the file.
    pub address: usize,
    pub axis_keys: &'static [AxisKey],
    pub calibrated: bool,
}

impl MapTemplate {
    /// Whether a record header declaring this grid can be this family.
    pub fn accepts_grid(&self, nx: usize, ny: usize) -> bool {
        (nx, ny) == (self.nx, self.ny) || self.alt_grids.contains(&(nx, ny))
    }
}

include!("signatures_data.rs");
