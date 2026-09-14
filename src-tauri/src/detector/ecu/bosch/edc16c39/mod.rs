// Bosch EDC16C39 detector (Fiat/Alfa Romeo Multijet common rail).
//
// STATUS: CALIBRATED for the 2MB passenger-car layout shared by Alfa
// 159/147/GT/Brera and Fiat Bravo/Croma/Doblo/Grande Punto/Punto. See
// `signatures.rs`'s module doc for the corpus and this module's "Known
// limits" section below for what is NOT covered.
//
// Why this detector does not look like the VAG ones
// ---------------------------------------------------
// The VAG EDC16 modules scan for a plausible [axis][axis][data] triple and
// guess the grid from a list of candidate sizes. EDC16C39 does not need
// that: every calibration here uses one of two self-describing records --
//
//   map:   [u16 nx][u16 ny][i16 X nx][i16 Y ny][i16 Z nx*ny]   (Y fast index)
//   curve: [u16 n][i16 X n][i16 Z n]
//
// so the grid is read, not guessed. The map record is the same shape
// independently confirmed on the (unrelated OEM) EDC16CP31 family, reused
// here as the same generic mechanism, not shared code. The curve record
// was confirmed against the reference database's own 1D entries, which all
// decode at their declared addresses on the build it shares with the corpus.
//
// Layout facts this detector leans on (measured on the corpus, re-checked
// by the tests below):
//   1. On most real builds a family's record sits at, or within a few bytes
//      of, the same address (`MapTemplate::address`) -- the primary signal
//      (phase 0). A minority of real builds carry several families tens of
//      KB away; the axis-key phase (1) finds them within
//      `C39_KEY_AMBIGUITY_WINDOW` and they are left UNDETECTED beyond it.
//   2. Some families are stored as a RUN of back-to-back records: the five
//      `InjCrv_Bas1..5` timing maps, `LmbdSmkHigh/Low`, `AccPed_trqEngA/B`
//      (0x244 apart, sharing a grid), the seven `TrqMaxGear` curves (0x3E
//      apart) and the four EGR hysteresis curves (0x66 apart, in two
//      interleaved pairs). Searching for one member's grid legitimately
//      matches its siblings, so members are never labelled by "nearest hit"
//      alone:
//        - phase 1 resolves a run as a whole: only a COMPLETE run, each slot
//          matching that member's own axis keys (or a sibling's it shares a
//          grid with), nearest the run's confirmed start;
//        - a TIGHT run -- records shorter than two address windows -- is
//          resolved as a whole in phase 0 too. For such a stride any shift
//          of the build puts some sibling inside a member's address window
//          (a real +0xC0 gear-run shift would put gear 1 six bytes from
//          gear 4's address; the real Grande Punto +0xFC shift puts
//          hysteresis 1 0x30 from hysteresis 3's), so per-member address
//          search is ambiguous by construction. Wide runs keep per-member
//          phase 0: their members are genuinely absent on some real builds
//          (`InjCrv_Bas4/5` exist on 166 of the reference database's 222).
//   3. `PCR_DesMax` (single-value boost limiter) is a bare i16 with no
//      header, 0x244 after the `PCR_DesMaxAP` record head on 220 of 222
//      reference builds and on all 18 corpus files carrying the pair. It is
//      only ever read at that offset from a record found by this detector.
//
// Safety contract of this module
// -------------------------------
// `detect()` returns an empty vector for any file that is not the
// calibrated 2MB layout, and never falls back to the VAG detectors. Per
// CONTRIBUTING.md: "A false 'unsupported file' is annoying; a false 'this
// is an EDC16' corrupts someone's ECU." The same contract extends to
// labels: a template is emitted AT MOST ONCE per file, across both phases
// (`found` in `detect`), and never by per-member proximity inside a tight
// run (regression tests `a_template_is_never_reported_twice_across_phases`,
// `a_tight_curve_run_is_never_labelled_member_by_member`,
// `interleaved_egr_hysteresis_pairs_resolve_on_a_shifted_build`).
//
// Known limits (corpus: 42 real 2MB dumps + 12 partial 256KB extracts)
// -----------------------------------------------------------------------
//   * Fiat Ducato (commercial van, same "EDC16C39" WinOLS label): the 34
//     passenger-tune families (torque/injection curves specific to the
//     1.9/2.4 JTD calibration) do not match on any of the 10 real Ducato
//     files checked -- a different physical calibration, NOT covered. Two
//     of the newer, platform-wide families (`IA_FuelAccel*`,
//     `BS_TurboPressureCorr*` -- see signatures.rs's module doc) ARE
//     present at their confirmed addresses on 6 of the 10, the ones with a
//     newer HW number (0281015xxx-0281016xxx); absent on the other 4
//     (0281012xxx-0281014xxx). A real split, not a false positive: the
//     safety contract is about never claiming a match that isn't there,
//     not about a foreign file matching nothing at all.
//   * 19 of the 49 families (the ones sourced from ECM Titanium's
//     `axis_table` DRT variant -- see signatures.rs) have no confirmed
//     Bosch symbol, only a categorical description; `symbol_confirmed`
//     is false on those and their description says so honestly instead
//     of inventing one.
//   * Real tunes in the corpus still edit calibrations this module does not
//     report, because no reference names them: an 8x8 engine speed x
//     temperature map (edited on 16 files), an 8x8 engine speed x rail
//     pressure map (13), a 10x10 temperature x quantity map (9), further
//     isolated maps on a widely-shared RPM breakpoint grid this pass could
//     not safely attribute to one real table, and on Grande Punto/Punto a
//     fixed-capacity pedal table at ~0x1C2040 (13) that does not use either
//     record above. See docs/PORTING-EDC16C39.md.
//   * `SpdLimMax` (vehicle speed limit) is a bare scalar whose position is
//     not stable relative to any family found here -- not read.
//   * `z_range_stock`/`z_range_tuned` are derived from this corpus's observed
//     spans, generously widened -- still not the "20 files, know what's
//     always there" bar CONTRIBUTING.md asks for on every family.
//   * No completeness/invariant report yet (needs a real multi-file bench,
//     see docs/PORTING-EDC16C39.md).

mod signatures;

pub use signatures::{AxisKey, AxisType, MapTemplate, Shape, MAP_TEMPLATES};

use crate::models::{DataType, DetectedMap, MapCategory, MapDimensions};
use std::collections::{HashMap, HashSet};

const C39_MAP_HEADER_LEN: usize = 4;
const C39_CURVE_HEADER_LEN: usize = 2;
const C39_MAX_AXIS_PTS: usize = 32;
const C39_AXIS_KEY_BONUS: f32 = 0.15;
/// A key matching more places than this describes a grid shared by half the
/// file, not a family -- discarded rather than trusted.
const C39_MAX_KEY_HITS: usize = 64;
/// Half-width of the search window placed around `MapTemplate::address`.
/// Searched outward from the exact address (see `detect_by_address`), so a
/// structurally valid neighbour inside the window can never win over the
/// record at the confirmed address itself.
const C39_ADDRESS_WINDOW: usize = 64;
/// An axis-key hit is trusted only within this many bytes of the confirmed
/// address -- see `resolve_sequence`. Wider than `C39_ADDRESS_WINDOW` on
/// purpose: this is the fallback for a build where the record moved further
/// than a few bytes, while still being far short of "anywhere in the file".
const C39_KEY_AMBIGUITY_WINDOW: usize = 0x4000;
/// `PCR_DesMax` -- see this module's header, fact 3.
const C39_SVBL_ANCHOR: &str = "PCR_DesMaxAP";
const C39_SVBL_OFFSET: usize = 0x244;
const C39_SVBL_NAME: &str = "SVBL (Single value boost limiter)";
const C39_SVBL_FACTOR: f64 = 2.0;
/// hPa. Stock: the corpus spans 4800-5600; tuned: generous room either way.
const C39_SVBL_RANGE_STOCK: (f64, f64) = (3_000.0, 7_000.0);
const C39_SVBL_RANGE_TUNED: (f64, f64) = (1_000.0, 12_000.0);

/// Decoded self-describing record, before any template is applied. For a
/// curve `ny` is 1 and `y_axis` is empty.
#[derive(Debug)]
pub struct CalBlock {
    pub addr: usize,
    pub nx: usize,
    pub ny: usize,
    pub x_axis: Vec<i16>,
    pub y_axis: Vec<i16>,
    pub data_addr: usize,
    pub z: Vec<i16>,
}

pub struct EDC16C39Detector {
    tuned_mode: bool,
}

/// Record head of every template emitted so far, by template label.
type Found = HashMap<&'static str, usize>;

impl EDC16C39Detector {
    pub fn new() -> Self {
        Self { tuned_mode: false }
    }

    pub fn new_tuned() -> Self {
        Self { tuned_mode: true }
    }

    /// Main entry point. `data` must already be known to be a 2MB EDC16C39
    /// dump (the identifier gate handles that) -- this only refuses to run
    /// on a size it has never been calibrated against.
    pub fn detect(&self, data: &[u8]) -> Vec<DetectedMap> {
        if data.len() != 2_097_152 {
            log::debug!(
                "[EDC16C39] {} bytes: only the 2MB passenger-car layout is calibrated, returning 0 maps",
                data.len()
            );
            return Vec::new();
        }

        let templates = Self::calibrated_templates_by_address();
        let sequences = Self::sequences(&templates);
        let mut maps: Vec<DetectedMap> = Vec::new();
        let mut claimed: HashSet<(u32, u32)> = HashSet::new();
        let mut found: Found = HashMap::new();

        // PHASE 0: confirmed address, searched outward. The primary signal
        // for this family -- see this module's header.
        for sequence in &sequences {
            if Self::is_tight(sequence) {
                self.detect_run_by_address(data, sequence, &mut maps, &mut claimed, &mut found);
                continue;
            }
            for template in sequence {
                if let Some((map, head)) = self.detect_by_address(data, template) {
                    if Self::push_if_free(&mut maps, &mut claimed, map) {
                        found.insert(template.bosch_label, head);
                    }
                }
            }
        }

        // PHASE 1: axis-key walk, address-independent, for whatever phase 0
        // did not find.
        if templates.iter().any(|t| !found.contains_key(t.bosch_label)) {
            let hits = Self::axis_key_hits(data, &templates);
            for sequence in &sequences {
                if sequence.iter().all(|t| found.contains_key(t.bosch_label)) {
                    continue;
                }
                self.resolve_sequence(data, sequence, &hits, &mut maps, &mut claimed, &mut found);
            }
        }

        // Single values: only ever read relative to a record found above.
        self.detect_svbl(data, &found, &mut maps, &mut claimed);

        maps.sort_by_key(|m| m.address);
        maps
    }

    /// For the identifier: does `bosch_label`'s confirmed record decode at
    /// its real address on this file, under the same checks the detector
    /// applies (grid, axis ranges, Z range, flatness)? Uses the tuned Z
    /// range on purpose: a tuned Fiat/Alfa file is still an EDC16C39.
    pub fn confirmed_block_present(data: &[u8], bosch_label: &str) -> bool {
        if data.len() != 2_097_152 {
            return false;
        }
        MAP_TEMPLATES
            .iter()
            .find(|t| t.calibrated && t.bosch_label == bosch_label)
            .map(|t| Self::new_tuned().detect_by_address(data, t).is_some())
            .unwrap_or(false)
    }

    fn calibrated_templates_by_address() -> Vec<&'static MapTemplate> {
        let mut out: Vec<&'static MapTemplate> = MAP_TEMPLATES.iter().filter(|t| t.calibrated).collect();
        out.sort_by_key(|t| t.address);
        out
    }

    fn record_len(shape: Shape, nx: usize, ny: usize) -> usize {
        match shape {
            Shape::Map => C39_MAP_HEADER_LEN + 2 * (nx + ny) + 2 * nx * ny,
            Shape::Curve => C39_CURVE_HEADER_LEN + 4 * nx,
        }
    }

    /// On-disk size of one record of this template's primary grid.
    fn block_len(template: &MapTemplate) -> usize {
        Self::record_len(template.shape, template.nx, template.ny)
    }

    /// Returns true (and records the byte range) when `map` does not overlap
    /// an already-emitted map.
    fn push_if_free(maps: &mut Vec<DetectedMap>, claimed: &mut HashSet<(u32, u32)>, map: DetectedMap) -> bool {
        let range = (map.address, map.address + map.size as u32);
        if claimed.iter().any(|&(s, e)| range.0 < e && s < range.1) {
            return false;
        }
        claimed.insert(range);
        maps.push(map);
        true
    }

    // ------------------------------ runs ------------------------------

    /// Partition the (address-sorted) templates into runs: consecutive
    /// templates of the same shape whose confirmed addresses are exactly one
    /// record apart, AND either tight (see `is_tight`) or sharing an axis
    /// key. Everything else is a run of one.
    fn sequences(templates: &[&'static MapTemplate]) -> Vec<Vec<&'static MapTemplate>> {
        let mut out: Vec<Vec<&'static MapTemplate>> = Vec::new();
        for &template in templates {
            let extends = out.last().map_or(false, |seq| {
                let prev = *seq.last().expect("non-empty sequence");
                let contiguous =
                    prev.shape == template.shape && prev.address + Self::block_len(prev) == template.address;
                contiguous
                    && (Self::block_len(prev) < 2 * C39_ADDRESS_WINDOW || Self::share_axis_key(prev, template))
            });
            if extends {
                out.last_mut().expect("non-empty").push(template);
            } else {
                out.push(vec![template]);
            }
        }
        out
    }

    /// A run whose records are shorter than two address windows: any shift
    /// of the build puts a sibling inside some member's window, so it is
    /// only ever resolved as a whole.
    fn is_tight(sequence: &[&'static MapTemplate]) -> bool {
        sequence.len() >= 2 && Self::block_len(sequence[0]) < 2 * C39_ADDRESS_WINDOW
    }

    /// Each member's position relative to the run's first member, from the
    /// confirmed addresses (grid-independent: on 13x10 driver-wish builds the
    /// records keep the 16x16 spacing).
    fn offsets(sequence: &[&'static MapTemplate]) -> Vec<usize> {
        sequence.iter().map(|t| t.address - sequence[0].address).collect()
    }

    fn share_axis_key(a: &MapTemplate, b: &MapTemplate) -> bool {
        a.axis_keys.iter().any(|ka| {
            let ka = ka.to_bytes();
            b.axis_keys.iter().any(|kb| kb.to_bytes() == ka)
        })
    }

    // ------------------------- PHASE 0: address -------------------------

    /// The exact confirmed address first, then outward by increasing
    /// distance up to `C39_ADDRESS_WINDOW`. Order matters: several template
    /// pairs here are stored back to back, so `address - 64` lies inside the
    /// previous sibling's data, and a scan that simply walked upward from
    /// `address - 64` and kept the first valid record reported a neighbour
    /// instead of the record at the address (regression test
    /// `phase0_prefers_the_block_at_the_confirmed_address`).
    fn detect_by_address(&self, data: &[u8], template: &MapTemplate) -> Option<(DetectedMap, usize)> {
        let base = template.address;
        for d in (0..=C39_ADDRESS_WINDOW).step_by(2) {
            if let Some(found) = self.try_block(data, base + d, template) {
                return Some(found);
            }
            if d > 0 {
                if let Some(off) = base.checked_sub(d) {
                    if let Some(found) = self.try_block(data, off, template) {
                        return Some(found);
                    }
                }
            }
        }
        None
    }

    /// Tight runs: the whole run's structure (header, grid, axes) must be in
    /// place at one start within the address window, searched outward; then
    /// each member is decoded at its own offset and emitted if its data
    /// passes (a heavily tuned member failing its Z range does not hide its
    /// siblings).
    fn detect_run_by_address(
        &self,
        data: &[u8],
        sequence: &[&'static MapTemplate],
        maps: &mut Vec<DetectedMap>,
        claimed: &mut HashSet<(u32, u32)>,
        found: &mut Found,
    ) {
        let first = sequence[0].address;
        let offsets = Self::offsets(sequence);
        for d in (0..=C39_ADDRESS_WINDOW).step_by(2) {
            let starts = [Some(first + d), if d > 0 { first.checked_sub(d) } else { None }];
            for start in starts.into_iter().flatten() {
                let complete = sequence
                    .iter()
                    .zip(&offsets)
                    .all(|(t, &o)| Self::structure_ok(data, start + o, t));
                if !complete {
                    continue;
                }
                for (t, &o) in sequence.iter().zip(&offsets) {
                    if let Some((map, head)) = self.try_block(data, start + o, t) {
                        if Self::push_if_free(maps, claimed, map) {
                            found.insert(t.bosch_label, head);
                        }
                    }
                }
                return;
            }
        }
    }

    // ------------------------ PHASE 1: axis keys ------------------------

    /// Every location where any of `templates`' axis keys matches byte for
    /// byte, keyed by template label, sorted. One pass over the file: the
    /// first 4 bytes are looked up first, the full key compared only on a
    /// hit. A key matching more than `C39_MAX_KEY_HITS` places is dropped (a
    /// grid shared by that much of the file is not evidence).
    fn axis_key_hits(data: &[u8], templates: &[&'static MapTemplate]) -> HashMap<&'static str, Vec<usize>> {
        struct Probe {
            label: &'static str,
            bytes: Vec<u8>,
            hits: Vec<usize>,
            saturated: bool,
        }
        let mut probes: Vec<Probe> = Vec::new();
        let mut by_prefix: HashMap<[u8; 4], Vec<usize>> = HashMap::new();
        for template in templates {
            for key in template.axis_keys {
                let bytes = key.to_bytes();
                if bytes.len() < C39_MAP_HEADER_LEN + 8 {
                    continue;
                }
                let prefix = [bytes[0], bytes[1], bytes[2], bytes[3]];
                by_prefix.entry(prefix).or_default().push(probes.len());
                probes.push(Probe { label: template.bosch_label, bytes, hits: Vec::new(), saturated: false });
            }
        }

        let mut off = 0;
        while off + 4 <= data.len() {
            let prefix = [data[off], data[off + 1], data[off + 2], data[off + 3]];
            if let Some(indices) = by_prefix.get(&prefix) {
                for &pi in indices {
                    let probe = &mut probes[pi];
                    if probe.saturated {
                        continue;
                    }
                    let end = off + probe.bytes.len();
                    if end <= data.len() && data[off..end] == probe.bytes[..] {
                        probe.hits.push(off);
                        if probe.hits.len() > C39_MAX_KEY_HITS {
                            probe.saturated = true;
                        }
                    }
                }
            }
            off += 2;
        }

        let mut out: HashMap<&'static str, Vec<usize>> = HashMap::new();
        for probe in probes {
            if probe.saturated {
                continue;
            }
            out.entry(probe.label).or_default().extend(probe.hits);
        }
        for hits in out.values_mut() {
            hits.sort_unstable();
            hits.dedup();
        }
        out
    }

    fn hits_for<'a>(hits: &'a HashMap<&'static str, Vec<usize>>, label: &str) -> &'a [usize] {
        hits.get(label).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Resolve one sequence from the axis-key hits.
    ///
    /// A run of one: the hits are tried nearest-to-confirmed-address first;
    /// the first that decodes under the template's checks AND does not
    /// overlap an already-emitted map wins; a hit beyond
    /// `C39_KEY_AMBIGUITY_WINDOW` is not tried at all.
    ///
    /// A run of N >= 2: only a COMPLETE run is trusted. A start qualifies
    /// when every member's slot (start + its confirmed offset) is either
    /// where phase 0 already found that member, or matched by that member's
    /// own axis keys -- or by the keys of a sibling it shares a grid with.
    /// The qualifying start nearest the run's confirmed start (within the
    /// ambiguity window) is used, and members not yet found are emitted at
    /// their slots. A partial run emits nothing: a member resolved on its
    /// own would just be labelled after whichever sibling sits closest.
    fn resolve_sequence(
        &self,
        data: &[u8],
        sequence: &[&'static MapTemplate],
        hits: &HashMap<&'static str, Vec<usize>>,
        maps: &mut Vec<DetectedMap>,
        claimed: &mut HashSet<(u32, u32)>,
        found: &mut Found,
    ) {
        let first = sequence[0];

        if sequence.len() == 1 {
            let mut ordered: Vec<usize> = Self::hits_for(hits, first.bosch_label).to_vec();
            ordered.sort_by_key(|&h| h.abs_diff(first.address));
            for hit in ordered {
                if hit.abs_diff(first.address) > C39_KEY_AMBIGUITY_WINDOW {
                    break;
                }
                if let Some((map, head)) = self.try_block_keyed(data, hit, first) {
                    if Self::push_if_free(maps, claimed, map) {
                        found.insert(first.bosch_label, head);
                        return;
                    }
                }
            }
            return;
        }

        let offsets = Self::offsets(sequence);
        let start = {
            let slot_ok = |k: usize, pos: usize| -> bool {
                let member = sequence[k];
                found.get(member.bosch_label) == Some(&pos)
                    || sequence.iter().any(|s| {
                        (s.bosch_label == member.bosch_label || Self::share_axis_key(s, member))
                            && Self::hits_for(hits, s.bosch_label).binary_search(&pos).is_ok()
                    })
            };
            let mut starts: Vec<usize> = Vec::new();
            for (k, t) in sequence.iter().enumerate() {
                let own = Self::hits_for(hits, t.bosch_label).iter().copied().chain(found.get(t.bosch_label).copied());
                starts.extend(own.filter_map(|p| p.checked_sub(offsets[k])));
            }
            starts.sort_unstable();
            starts.dedup();
            starts
                .into_iter()
                .filter(|&s| s.abs_diff(first.address) <= C39_KEY_AMBIGUITY_WINDOW)
                .filter(|&s| (0..sequence.len()).all(|k| slot_ok(k, s + offsets[k])))
                .min_by_key(|&s| s.abs_diff(first.address))
        };
        let Some(start) = start else {
            return;
        };
        for (k, template) in sequence.iter().enumerate() {
            if found.contains_key(template.bosch_label) {
                continue;
            }
            if let Some((map, head)) = self.try_block_keyed(data, start + offsets[k], template) {
                if Self::push_if_free(maps, claimed, map) {
                    found.insert(template.bosch_label, head);
                }
            }
        }
    }

    /// Phase-1 decode: same checks as phase 0 (including the stock/tuned Z
    /// range the detector was built with), plus the axis-key confidence
    /// bonus.
    fn try_block_keyed(&self, data: &[u8], off: usize, template: &MapTemplate) -> Option<(DetectedMap, usize)> {
        let (mut map, head) = self.try_block(data, off, template)?;
        map.confidence = (map.confidence + C39_AXIS_KEY_BONUS).min(0.99);
        Some((map, head))
    }

    // --------------------------- single values ---------------------------

    fn detect_svbl(
        &self,
        data: &[u8],
        found: &Found,
        maps: &mut Vec<DetectedMap>,
        claimed: &mut HashSet<(u32, u32)>,
    ) {
        let Some(&head) = found.get(C39_SVBL_ANCHOR) else {
            return;
        };
        let off = head + C39_SVBL_OFFSET;
        let Some(raw) = Self::read_u16_be(data, off) else {
            return;
        };
        let value = raw as f64 * C39_SVBL_FACTOR;
        let (lo, hi) = if self.tuned_mode { C39_SVBL_RANGE_TUNED } else { C39_SVBL_RANGE_STOCK };
        if value < lo || value > hi {
            return;
        }
        let mut map = DetectedMap::new(off as u32, 2, MapDimensions::OneDimensional { length: 1 }, DataType::UInt16);
        map.name = Some(C39_SVBL_NAME.to_string());
        map.category = Some(MapCategory::TurboBoostPressure.display_name().to_string());
        map.unit = Some("hPa".to_string());
        map.correction_factor = Some(C39_SVBL_FACTOR);
        map.offset = Some(0.0);
        map.confidence = 0.90;
        map.description = Some(format!(
            "EDC16C39 {} (Bosch: PCR_DesMax) - scalar 0x{:06X}, {} record + 0x{:X}",
            C39_SVBL_NAME, off, C39_SVBL_ANCHOR, C39_SVBL_OFFSET
        ));
        Self::push_if_free(maps, claimed, map);
    }

    // ------------------------ record decoding ------------------------

    /// Decode the map record at `off` without any interpretation.
    pub fn read_block(data: &[u8], off: usize, max_nx: usize, max_ny: usize) -> Option<CalBlock> {
        let nx = Self::read_u16_be(data, off)? as usize;
        let ny = Self::read_u16_be(data, off + 2)? as usize;
        if nx < 2 || ny < 2 || nx > max_nx || ny > max_ny {
            return None;
        }
        let x_off = off + C39_MAP_HEADER_LEN;
        let y_off = x_off + 2 * nx;
        let data_addr = y_off + 2 * ny;
        let x_axis = Self::read_i16_be_slice(data, x_off, nx)?;
        let y_axis = Self::read_i16_be_slice(data, y_off, ny)?;
        let z = Self::read_i16_be_slice(data, data_addr, nx * ny)?;
        Some(CalBlock { addr: off, nx, ny, x_axis, y_axis, data_addr, z })
    }

    /// Decode the curve record at `off` without any interpretation.
    pub fn read_curve(data: &[u8], off: usize, max_n: usize) -> Option<CalBlock> {
        let n = Self::read_u16_be(data, off)? as usize;
        if n < 2 || n > max_n {
            return None;
        }
        let x_off = off + C39_CURVE_HEADER_LEN;
        let data_addr = x_off + 2 * n;
        let x_axis = Self::read_i16_be_slice(data, x_off, n)?;
        let z = Self::read_i16_be_slice(data, data_addr, n)?;
        Some(CalBlock { addr: off, nx: n, ny: 1, x_axis, y_axis: Vec::new(), data_addr, z })
    }

    fn read_record(data: &[u8], off: usize, template: &MapTemplate) -> Option<CalBlock> {
        match template.shape {
            Shape::Map => Self::read_block(data, off, C39_MAX_AXIS_PTS, C39_MAX_AXIS_PTS),
            Shape::Curve => Self::read_curve(data, off, C39_MAX_AXIS_PTS),
        }
    }

    /// Header, grid and axes only -- no look at the data.
    fn structure_ok(data: &[u8], off: usize, template: &MapTemplate) -> bool {
        Self::read_record(data, off, template).map_or(false, |b| Self::structure_matches(&b, template))
    }

    fn structure_matches(block: &CalBlock, template: &MapTemplate) -> bool {
        let (x_type, y_type) = template.axes;
        match template.shape {
            Shape::Map => {
                template.accepts_grid(block.nx, block.ny)
                    && Self::axis_is_valid(&block.x_axis, x_type)
                    && Self::axis_is_valid(&block.y_axis, y_type)
            }
            Shape::Curve => block.nx == template.nx && Self::axis_is_valid(&block.x_axis, x_type),
        }
    }

    /// Returns the map and the record head it was read from.
    fn try_block(&self, data: &[u8], off: usize, template: &MapTemplate) -> Option<(DetectedMap, usize)> {
        let block = Self::read_record(data, off, template)?;
        if !Self::structure_matches(&block, template) {
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
        let (rows, per_row) = match template.shape {
            Shape::Map => (block.nx, block.ny),
            Shape::Curve => (1, block.nx),
        };
        let score = self.score(&values, rows, per_row, template)?;
        Some((Self::build_map(&block, template, score), off))
    }

    fn score(&self, values: &[f64], rows: usize, per_row: usize, template: &MapTemplate) -> Option<f64> {
        let (lo, hi) = if self.tuned_mode { template.z_range_tuned } else { template.z_range_stock };
        if values.iter().any(|v| *v < lo || *v > hi) {
            return None;
        }
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if !template.allow_flat && (max - min).abs() < f64::EPSILON {
            return None;
        }
        let monotonic = Self::row_monotonicity(values, rows, per_row);
        Some((0.60 + 0.40 * monotonic).min(0.99))
    }

    /// Share of rows (`per_row` values each) that are monotonic in either
    /// direction.
    fn row_monotonicity(values: &[f64], rows: usize, per_row: usize) -> f64 {
        if rows == 0 || per_row < 2 || values.len() < rows * per_row {
            return 0.0;
        }
        let mut good = 0usize;
        for r in 0..rows {
            let row = &values[r * per_row..(r + 1) * per_row];
            let up = row.windows(2).all(|w| w[1] >= w[0]);
            let down = row.windows(2).all(|w| w[1] <= w[0]);
            if up || down {
                good += 1;
            }
        }
        good as f64 / rows as f64
    }

    fn provenance(template: &MapTemplate) -> String {
        if template.symbol_confirmed {
            format!("Bosch: {}", template.bosch_label)
        } else {
            "no reference symbol".to_string()
        }
    }

    fn build_map(block: &CalBlock, template: &MapTemplate, score: f64) -> DetectedMap {
        let (x_type, y_type) = template.axes;
        let data_type = if template.signed { DataType::Int16 } else { DataType::UInt16 };
        let mut map = match template.shape {
            Shape::Map => DetectedMap::new(
                block.data_addr as u32,
                block.nx * block.ny * 2,
                MapDimensions::TwoDimensional { rows: block.nx, cols: block.ny },
                data_type,
            ),
            Shape::Curve => DetectedMap::new(
                block.data_addr as u32,
                block.nx * 2,
                MapDimensions::OneDimensional { length: block.nx },
                data_type,
            ),
        };
        map.name = Some(template.name.to_string());
        map.category = Some(template.category.display_name().to_string());
        map.unit = Some(template.unit.to_string());
        map.correction_factor = Some(template.z_factor);
        map.offset = Some(template.z_offset);
        map.confidence = score as f32;
        match template.shape {
            Shape::Map => {
                // Rows of the record are the first axis in memory (X, nx
                // entries), columns the second (Y, ny entries). The
                // frontend's left axis is the row axis, so it is served the
                // record's X axis, and vice versa.
                map.x_axis_address = Some((block.addr + C39_MAP_HEADER_LEN + 2 * block.nx) as u32);
                map.y_axis_address = Some((block.addr + C39_MAP_HEADER_LEN) as u32);
                map.x_axis_correction = Some(y_type.factor());
                map.y_axis_correction = Some(x_type.factor());
                map.x_axis_offset = Some(y_type.offset());
                map.y_axis_offset = Some(x_type.offset());
                map.x_label = Some(y_type.label().to_string());
                map.y_label = Some(x_type.label().to_string());
                map.y_axis_inverted = Some(false);
                map.description = Some(format!(
                    "EDC16C39 {} ({}) - block 0x{:06X}, {}x{}",
                    template.name,
                    Self::provenance(template),
                    block.addr,
                    block.nx,
                    block.ny
                ));
            }
            Shape::Curve => {
                // Same field convention as the VAG EDC16 curves in this app
                // (e.g. their "EGR hysteresis"): the single axis is served as
                // the X axis.
                map.x_axis_address = Some((block.addr + C39_CURVE_HEADER_LEN) as u32);
                map.x_axis_correction = Some(x_type.factor());
                map.x_axis_offset = Some(x_type.offset());
                map.x_label = Some(x_type.label().to_string());
                map.description = Some(format!(
                    "EDC16C39 {} ({}) - curve 0x{:06X}, {} points",
                    template.name,
                    Self::provenance(template),
                    block.addr,
                    block.nx
                ));
            }
        }
        map
    }

    fn axis_is_valid(values: &[i16], axis: AxisType) -> bool {
        if values.len() < 2 {
            return false;
        }
        let (lo, hi) = axis.raw_range();
        if values.iter().any(|v| *v < lo || *v > hi) {
            return false;
        }
        values.windows(2).all(|w| w[1] > w[0])
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
        Some(data[off..end].chunks_exact(2).map(|c| i16::from_be_bytes([c[0], c[1]])).collect())
    }
}

impl Default for EDC16C39Detector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EGR: &str = "AirCtl_rEGRBas";
    const EGR_NAME: &str = "Exhaust Gas Recirculation Duty Cycle";

    fn template(label: &str) -> &'static MapTemplate {
        MAP_TEMPLATES.iter().find(|t| t.bosch_label == label).unwrap()
    }

    fn blank() -> Vec<u8> {
        vec![0u8; 2_097_152]
    }

    /// Plant a map record at `addr`: header + axes, then `nx` rows of `z_row`
    /// (one Z value per Y breakpoint).
    fn plant(data: &mut [u8], addr: usize, nx: usize, ny: usize, x: &[i16], y: &[i16], z_row: &[i16]) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(nx as u16).to_be_bytes());
        bytes.extend_from_slice(&(ny as u16).to_be_bytes());
        for v in x.iter().chain(y.iter()) {
            bytes.extend_from_slice(&v.to_be_bytes());
        }
        for _ in 0..nx {
            for &z in z_row {
                bytes.extend_from_slice(&z.to_be_bytes());
            }
        }
        data[addr..addr + bytes.len()].copy_from_slice(&bytes);
    }

    /// Plant a curve record at `addr`.
    fn plant_curve(data: &mut [u8], addr: usize, x: &[i16], z: &[i16]) {
        assert_eq!(x.len(), z.len());
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(x.len() as u16).to_be_bytes());
        for v in x.iter().chain(z.iter()) {
            bytes.extend_from_slice(&v.to_be_bytes());
        }
        data[addr..addr + bytes.len()].copy_from_slice(&bytes);
    }

    /// The real EGR block (from the corpus) at `addr`, Z = the Y axis
    /// values repeated per row (0..100 %, in range, monotonic).
    fn plant_egr(data: &mut [u8], addr: usize) {
        let key = &template(EGR).axis_keys[0];
        plant(data, addr, key.nx, key.ny, key.x, key.y, key.y);
    }

    fn names(maps: &[DetectedMap]) -> Vec<String> {
        maps.iter().map(|m| m.name.clone().unwrap_or_default()).collect()
    }

    fn by_address(maps: &[DetectedMap]) -> Vec<(u32, String)> {
        let mut out: Vec<(u32, String)> = maps.iter().map(|m| (m.address, m.name.clone().unwrap())).collect();
        out.sort();
        out
    }

    /// Members of a run, in storage order.
    fn run(prefix: &str) -> Vec<&'static MapTemplate> {
        let mut out: Vec<&'static MapTemplate> =
            MAP_TEMPLATES.iter().filter(|t| t.bosch_label.starts_with(prefix)).collect();
        out.sort_by_key(|t| t.address);
        out
    }

    #[test]
    fn detects_the_egr_block_at_its_confirmed_address() {
        let mut data = blank();
        plant_egr(&mut data, template(EGR).address);
        let maps = EDC16C39Detector::new().detect(&data);
        assert_eq!(names(&maps), vec![EGR_NAME]);
    }

    #[test]
    fn detects_the_egr_block_by_axis_key_even_when_moved() {
        // Same block, moved 0x1000 bytes away from its usual address --
        // only the axis-key phase (not the address phase) can still find it.
        let mut data = blank();
        plant_egr(&mut data, template(EGR).address + 0x1000);
        let maps = EDC16C39Detector::new().detect(&data);
        assert_eq!(names(&maps), vec![EGR_NAME]);
    }

    #[test]
    fn a_file_of_the_wrong_size_yields_no_maps() {
        let data = vec![0u8; 1_048_576];
        assert!(EDC16C39Detector::new().detect(&data).is_empty());
    }

    #[test]
    fn random_noise_at_the_confirmed_address_is_not_reported() {
        let mut data = blank();
        let t = template(EGR);
        for (i, b) in data[t.address..t.address + 256].iter_mut().enumerate() {
            *b = ((i * 7 + 13) % 251) as u8;
        }
        let maps = EDC16C39Detector::new().detect(&data);
        assert!(maps.is_empty());
    }

    /// The case that disproved "one result per template" in an earlier
    /// revision: a structurally valid block with a grid NOT in the key list
    /// at the confirmed address (found by phase 0, which never looks at
    /// keys) plus the listed key grid 0x1000 bytes later (a phase-1 hit
    /// within the ambiguity window). Two different real tables, one name.
    #[test]
    fn a_template_is_never_reported_twice_across_phases() {
        let mut data = blank();
        let t = template(EGR);
        let key = &t.axis_keys[0];
        let mut other_x = key.x.to_vec();
        other_x[1] += 50; // a real-looking grid that is not any listed key
        plant(&mut data, t.address, key.nx, key.ny, &other_x, key.y, key.y);
        plant_egr(&mut data, t.address + 0x1000);
        let maps = EDC16C39Detector::new_tuned().detect(&data);
        assert_eq!(names(&maps), vec![EGR_NAME]);
        assert_eq!(maps[0].y_axis_address, Some((t.address + C39_MAP_HEADER_LEN) as u32));
    }

    /// Several template pairs are stored back to back, so `address - 64` is
    /// inside the previous block's data: a valid decoy there must not beat
    /// the block at the confirmed address.
    #[test]
    fn phase0_prefers_the_block_at_the_confirmed_address() {
        let mut data = blank();
        let t = template(EGR);
        plant_egr(&mut data, t.address - 64);
        plant_egr(&mut data, t.address);
        let maps = EDC16C39Detector::new().detect(&data);
        assert_eq!(names(&maps), vec![EGR_NAME]);
        assert_eq!(maps[0].y_axis_address, Some((t.address + C39_MAP_HEADER_LEN) as u32));
    }

    /// The five `InjCrv_Bas` maps share one grid and are stored in order,
    /// one block apart. On a build shifted by a whole block or more, each
    /// member's nearest hit is a sibling's block; only resolving the run as
    /// a whole keeps the labels in storage order. Both shift directions.
    #[test]
    fn a_shifted_sibling_run_keeps_its_labels_in_storage_order() {
        for shift in [-0x400i64, 0x400, -0x1000] {
            let mut data = blank();
            let members = run("InjCrv_Bas");
            assert_eq!(members.len(), 5);
            let key = &members[0].axis_keys[0];
            let stride = EDC16C39Detector::block_len(members[0]);
            let start = (members[0].address as i64 + shift) as usize;
            for k in 0..members.len() {
                let z_row: Vec<i16> = (0..key.ny as i16).map(|c| (k as i16) * 10 + c).collect();
                plant(&mut data, start + k * stride, key.nx, key.ny, key.x, key.y, &z_row);
            }
            let maps = EDC16C39Detector::new().detect(&data);
            let expected: Vec<(u32, String)> = (0..5)
                .map(|k| {
                    let block = start + k * stride;
                    (
                        (block + C39_MAP_HEADER_LEN + 2 * (key.nx + key.ny)) as u32,
                        format!("Injection Timing {}", k + 1),
                    )
                })
                .collect();
            assert_eq!(by_address(&maps), expected, "shift {:#x}", shift);
        }
    }

    /// A partial run (3 of 5 blocks) is ambiguous by construction and must
    /// not be labelled at all.
    #[test]
    fn a_partial_sibling_run_is_not_guessed() {
        let mut data = blank();
        let first = template("InjCrv_Bas1");
        let key = &first.axis_keys[0];
        let stride = EDC16C39Detector::block_len(first);
        let start = first.address + 0x400;
        for k in 0..3 {
            let z_row: Vec<i16> = (0..key.ny as i16).map(|c| (k as i16) * 10 + c).collect();
            plant(&mut data, start + k * stride, key.nx, key.ny, key.x, key.y, &z_row);
        }
        let maps = EDC16C39Detector::new().detect(&data);
        assert!(maps.is_empty(), "got {:?}", names(&maps));
    }

    /// Stock mode must be stock in BOTH phases: a block whose Z exceeds the
    /// stock range but fits the tuned one is rejected by `new()` and
    /// accepted by `new_tuned()`, at the confirmed address and when moved.
    #[test]
    fn stock_mode_is_honoured_by_the_axis_key_phase_too() {
        let t = template(EGR);
        let key = &t.axis_keys[0];
        // 200 % duty: beyond stock (130 %) and within tuned (300 %).
        let z_row: Vec<i16> = key.y.iter().map(|&v| v * 2).collect();
        for addr in [t.address, t.address + 0x1000] {
            let mut data = blank();
            plant(&mut data, addr, key.nx, key.ny, key.x, key.y, &z_row);
            assert!(EDC16C39Detector::new().detect(&data).is_empty(), "stock accepted tuned Z at {:#x}", addr);
            assert_eq!(names(&EDC16C39Detector::new_tuned().detect(&data)), vec![EGR_NAME], "at {:#x}", addr);
        }
    }

    #[test]
    fn detects_a_curve_at_its_address_and_by_axis_key_when_moved() {
        let t = template("EngPrt_trqLim");
        let key = &t.axis_keys[0];
        let z: Vec<i16> = (0..key.nx as i16).map(|i| 1000 + i * 100).collect(); // 100..340 Nm
        for addr in [t.address, t.address + 0x1000] {
            let mut data = blank();
            plant_curve(&mut data, addr, key.x, &z);
            let maps = EDC16C39Detector::new().detect(&data);
            assert_eq!(names(&maps), vec!["Torque Limiter"], "at {:#x}", addr);
            assert!(matches!(maps[0].dimensions, MapDimensions::OneDimensional { length: 25 }));
            assert_eq!(maps[0].address, (addr + 2 + 2 * key.nx) as u32);
            assert_eq!(maps[0].x_axis_address, Some((addr + 2) as u32));
        }
    }

    /// The seven gear-limiter curves are 62 bytes apart. The reference
    /// database's real +0xC0 shift puts gear 1 six bytes from gear 4's
    /// corpus address: any per-member address search would label it "4".
    #[test]
    fn a_tight_curve_run_is_never_labelled_member_by_member() {
        let gears = run("TrqMaxGear");
        assert_eq!(gears.len(), 7);
        let base = gears[0].address;
        for shift in [0i64, 0xC0, -0xC0, 0x3F8] {
            let mut data = blank();
            let start = (base as i64 + shift) as usize;
            let mut expected = Vec::new();
            for t in &gears {
                let key = &t.axis_keys[0];
                let off = start + (t.address - base);
                plant_curve(&mut data, off, key.x, &vec![2000i16; key.nx]); // flat 200 Nm, as on stock files
                expected.push(((off + 2 + 2 * key.nx) as u32, t.name.to_string()));
            }
            expected.sort();
            let maps = EDC16C39Detector::new().detect(&data);
            assert_eq!(by_address(&maps), expected, "shift {:#x}", shift);
        }
    }

    /// Hysteresis 1/3 and 2/4 share an axis. The real Grande Punto/Punto
    /// shift (+0xFC) puts curve 1 0x30 bytes from curve 3's corpus address.
    #[test]
    fn interleaved_egr_hysteresis_pairs_resolve_on_a_shifted_build() {
        let hys = run("EgrHys");
        assert_eq!(hys.len(), 4);
        let base = hys[0].address;
        for shift in [0usize, 0xFC] {
            let mut data = blank();
            let mut expected = Vec::new();
            for (k, t) in hys.iter().enumerate() {
                let key = &t.axis_keys[0];
                let off = base + shift + (t.address - base);
                let z: Vec<i16> = vec![500 * (k as i16 + 1); key.nx]; // 5..20 mg
                plant_curve(&mut data, off, key.x, &z);
                expected.push(((off + 2 + 2 * key.nx) as u32, t.name.to_string()));
            }
            expected.sort();
            let maps = EDC16C39Detector::new().detect(&data);
            assert_eq!(by_address(&maps), expected, "shift {:#x}", shift);
        }
    }

    /// Every real EGR-off tune in the corpus zeroes the hysteresis curves:
    /// that file must still report them.
    #[test]
    fn zeroed_egr_hysteresis_is_still_reported() {
        let mut data = blank();
        let hys = run("EgrHys");
        for t in &hys {
            let key = &t.axis_keys[0];
            plant_curve(&mut data, t.address, key.x, &vec![0i16; key.nx]);
        }
        let maps = EDC16C39Detector::new_tuned().detect(&data);
        assert_eq!(maps.len(), 4, "got {:?}", names(&maps));
    }

    #[test]
    fn a_flat_block_is_only_accepted_where_the_family_is_known_to_be_flat() {
        // PCR_DesMaxAP: one 4800-5600 hPa ceiling on all 18 corpus files carrying it.
        let ap = template("PCR_DesMaxAP");
        let key = &ap.axis_keys[0];
        let mut data = blank();
        plant(&mut data, ap.address, key.nx, key.ny, key.x, key.y, &vec![2500i16; key.ny]);
        assert!(names(&EDC16C39Detector::new().detect(&data)).contains(&"Boost Pressure Limiter".to_string()));
        // Rail_PointBase is never flat on a real file.
        let rail = template("Rail_PointBase");
        let key = &rail.axis_keys[0];
        let mut data = blank();
        plant(&mut data, rail.address, key.nx, key.ny, key.x, key.y, &vec![8000i16; key.ny]);
        assert!(EDC16C39Detector::new().detect(&data).is_empty());
    }

    #[test]
    fn a_grid_declared_by_the_reference_database_is_accepted_and_others_are_not() {
        let t = template("AccPed_trqENU");
        let row = |n: i16, step: i16| -> Vec<i16> { (0..n).map(|i| i * step).collect() };
        let mut data = blank();
        plant(&mut data, t.address, 13, 10, &row(13, 400), &row(10, 800), &row(10, 100));
        assert_eq!(names(&EDC16C39Detector::new().detect(&data)), vec![t.name.to_string()]);
        let mut data = blank();
        plant(&mut data, t.address, 12, 10, &row(12, 400), &row(10, 800), &row(10, 100));
        assert!(EDC16C39Detector::new().detect(&data).is_empty());
    }

    #[test]
    fn svbl_is_read_only_at_its_offset_from_a_found_boost_limiter_record() {
        let ap = template(C39_SVBL_ANCHOR);
        let key = &ap.axis_keys[0];
        let scalar_at = ap.address + C39_SVBL_OFFSET;
        let with = |block: bool, raw: u16| -> Vec<DetectedMap> {
            let mut data = blank();
            if block {
                plant(&mut data, ap.address, key.nx, key.ny, key.x, key.y, &vec![2500i16; key.ny]);
            }
            data[scalar_at..scalar_at + 2].copy_from_slice(&raw.to_be_bytes());
            EDC16C39Detector::new().detect(&data)
        };
        let maps = with(true, 2500);
        let svbl = maps.iter().find(|m| m.name.as_deref() == Some(C39_SVBL_NAME)).expect("SVBL");
        assert_eq!(svbl.address, scalar_at as u32);
        assert!(!names(&with(false, 2500)).contains(&C39_SVBL_NAME.to_string()), "no anchor record");
        assert!(!names(&with(true, 100)).contains(&C39_SVBL_NAME.to_string()), "200 hPa is not a limit");
    }

    /// Every harvested axis key is real data read from a real file, so the
    /// structural gates must accept all of them -- a key the gate rejects is
    /// dead code and silently loses that build's map.
    #[test]
    fn every_axis_key_passes_its_templates_axis_gates() {
        for t in MAP_TEMPLATES.iter().filter(|t| t.calibrated) {
            assert!(!t.axis_keys.is_empty(), "{} has no axis key", t.name);
            for (i, key) in t.axis_keys.iter().enumerate() {
                match t.shape {
                    Shape::Map => {
                        assert!(t.accepts_grid(key.nx, key.ny), "{} key {} grid {}x{}", t.bosch_label, i, key.nx, key.ny);
                        assert!(
                            EDC16C39Detector::axis_is_valid(key.y, t.axes.1),
                            "{} key {}: Y {:?} rejected by {:?}",
                            t.bosch_label,
                            i,
                            key.y,
                            t.axes.1
                        );
                    }
                    Shape::Curve => {
                        assert_eq!((key.nx, key.ny), (t.nx, 0), "{} key {} is not a curve key", t.bosch_label, i);
                        assert!(key.y.is_empty(), "{} key {} has a Y axis", t.bosch_label, i);
                    }
                }
                assert!(
                    EDC16C39Detector::axis_is_valid(key.x, t.axes.0),
                    "{} key {}: X {:?} rejected by {:?}",
                    t.bosch_label,
                    i,
                    key.x,
                    t.axes.0
                );
            }
        }
    }

    #[test]
    fn no_template_carries_a_duplicate_axis_key() {
        for t in MAP_TEMPLATES {
            let mut seen: HashSet<Vec<u8>> = HashSet::new();
            for key in t.axis_keys {
                assert!(seen.insert(key.to_bytes()), "{} has a duplicate axis key", t.bosch_label);
            }
        }
    }

    #[test]
    fn template_labels_are_unique() {
        let mut seen = HashSet::new();
        for t in MAP_TEMPLATES {
            assert!(seen.insert(t.bosch_label), "duplicate label {}", t.bosch_label);
        }
    }

    /// An unsigned template with a negative floor can never reach that
    /// floor (`v as u16` is never negative): the gate would be dead.
    #[test]
    fn unsigned_templates_have_non_negative_floors() {
        for t in MAP_TEMPLATES.iter().filter(|t| !t.signed) {
            assert!(t.z_range_stock.0 >= 0.0, "{} stock floor {}", t.bosch_label, t.z_range_stock.0);
            assert!(t.z_range_tuned.0 >= 0.0, "{} tuned floor {}", t.bosch_label, t.z_range_tuned.0);
        }
    }

    /// The identifier's structural check must be the detector's own.
    #[test]
    fn confirmed_block_probe_matches_the_detector() {
        let mut data = blank();
        assert!(!EDC16C39Detector::confirmed_block_present(&data, EGR));
        plant_egr(&mut data, template(EGR).address);
        assert!(EDC16C39Detector::confirmed_block_present(&data, EGR));
        assert!(!EDC16C39Detector::confirmed_block_present(&data, "no such family"));
    }
}
