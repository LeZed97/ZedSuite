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
// that: every 2D map here uses a self-describing record --
//
//   +0x00  u16        nx          number of X points
//   +0x02  u16        ny          number of Y points
//   +0x04  i16[nx]    X axis      strictly increasing
//   +....  i16[ny]    Y axis      strictly increasing
//   +....  i16[nx*ny] Z, column direction (Y is the fast index)
//
// so the grid is read, not guessed -- same record shape independently
// confirmed on the (unrelated OEM) EDC16CP31 family, and reused here as the
// same generic mechanism, not shared code: this module is its own
// implementation, calibrated against a completely different real corpus.
//
// Two real layout facts this detector leans on (both measured on the
// corpus, both re-checked by tests below):
//   1. On most real builds a given family's block sits at, or within a few
//      bytes of, the same address (`MapTemplate::address`). That is the
//      primary signal (phase 0). A minority of real builds (a Fiat Bravo
//      1.6 MultiJet 120cv, an Alfa 0281018045 build) carry several families
//      tens of KB away; those are found by the axis-key phase when they
//      stay within `C39_KEY_AMBIGUITY_WINDOW`, and are deliberately left
//      UNDETECTED beyond it rather than guessed at.
//   2. Some families are stored as a RUN of back-to-back blocks that share
//      the same breakpoint grid and differ only in their Z data: the five
//      `InjCrv_Bas1..5` injection-timing curves (stride 0x244, in order),
//      `LmbdSmkHigh`/`LmbdSmkLow`, `AccPed_trqEngA`/`EngB`. Searching for
//      one member's grid therefore legitimately matches at every member's
//      real address. Phase 1 resolves such a run as a whole (see
//      `resolve_sequence`), never member by member -- resolving each member
//      to "its nearest hit" mislabels the run as soon as the build is
//      shifted (verified on a real Alfa 159 2.4 JTD dump shifted +0x400:
//      per-member resolution reported curves 2 and 3 as "4" and "5").
//
// Safety contract of this module
// -------------------------------
// `detect()` returns an empty vector for any file that is not the
// calibrated 2MB layout, and never falls back to the VAG detectors. Per
// CONTRIBUTING.md: "A false 'unsupported file' is annoying; a false 'this
// is an EDC16' corrupts someone's ECU." The same contract extends to
// labels: a template is emitted AT MOST ONCE per file, across both phases
// (`found` set in `detect`), because two of this family's real tables can
// share a byte-identical axis grid closely enough that trusting every
// match reported the same map name twice at two different real addresses
// (caught on real Croma/Bravo/Grande Punto/Alfa dumps; regression test
// `a_template_is_never_reported_twice_across_phases`).
//
// Known limits (corpus: 42 real 2MB dumps + 12 partial 256KB extracts)
// -----------------------------------------------------------------------
//   * Fiat Ducato (commercial van, same "EDC16C39" WinOLS label) does not
//     match this layout at all on every real Ducato file checked (10) --
//     confirmed a different physical calibration on the same base chip,
//     explicitly NOT covered.
//   * Coverage: Alfa 159/147/GT/Brera and Fiat Croma/Doblo/Grande
//     Punto/Punto detect 16-20 of the 21 templates on every real file
//     (526 maps over the 42-file corpus); a minority of real Bravo and
//     unnamed Alfa/Fiat builds detect 0-7 (that specific software revision's
//     layout, see above).
//   * `PCR_DesMaxAP` (Boost Pressure Limiter) is calibrated but its Z data
//     is a flat constant on every corpus file, so the generic "a calibration
//     map is never flat" gate rejects it -- it never fires today.
//   * `z_range_stock`/`z_range_tuned` are derived from this corpus's observed
//     spans, generously widened -- still not the "20 files, know what's
//     always there" bar CONTRIBUTING.md asks for on every family.
//   * No completeness/invariant report yet (needs a real multi-file bench,
//     see docs/PORTING-EDC16C39.md).
//   * 1D curves (TrqMaxGear1..6+R, EngPrt_trqLim) and scalars (SpdLimMax,
//     PCR_DesMax) use a different on-disk record this module does not read
//     yet -- confirmed present (see docs/PORTING-EDC16C39.md) but not
//     implemented, matching CP31's own staged curve support.
//   * `TrqStrtBas` was seen at a stable address but with a DIFFERENT grid on
//     one real file (16x16 vs 10x16) -- deliberately left uncalibrated
//     rather than guessed at.

mod signatures;

pub use signatures::{AxisKey, AxisType, MapTemplate, MAP_TEMPLATES};

use crate::models::{DataType, DetectedMap, MapDimensions};
use std::collections::{HashMap, HashSet};

const C39_BLOCK_HEADER_LEN: usize = 4;
const C39_MAX_AXIS_PTS: usize = 32;
const C39_AXIS_KEY_BONUS: f32 = 0.15;
/// A key matching more places than this describes a grid shared by half the
/// file, not a family -- discarded rather than trusted.
const C39_MAX_KEY_HITS: usize = 64;
/// Half-width of the search window placed around `MapTemplate::address`.
/// Searched outward from the exact address (see `detect_by_address`), so a
/// structurally valid neighbour inside the window can never win over the
/// block at the confirmed address itself.
const C39_ADDRESS_WINDOW: usize = 64;
/// An axis-key hit is trusted only within this many bytes of the confirmed
/// address -- see `resolve_sequence`. Wider than `C39_ADDRESS_WINDOW` on
/// purpose: this is the fallback for a build where the block moved further
/// than a few bytes, so it needs more room, while still being far short of
/// "anywhere in the file".
const C39_KEY_AMBIGUITY_WINDOW: usize = 0x4000;

/// Decoded self-describing block, before any template is applied.
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
        let mut maps: Vec<DetectedMap> = Vec::new();
        let mut claimed: HashSet<(u32, u32)> = HashSet::new();
        // One result per template across BOTH phases -- see the module doc's
        // safety contract.
        let mut found: HashSet<&'static str> = HashSet::new();

        // PHASE 0: confirmed address, searched outward. The primary signal
        // for this family -- see this module's header.
        for template in &templates {
            if let Some(map) = self.detect_by_address(data, template) {
                if Self::push_if_free(&mut maps, &mut claimed, map) {
                    found.insert(template.bosch_label);
                }
            }
        }

        // PHASE 1: axis-key walk, address-independent, for whatever phase 0
        // did not find. Catches a build where the block moved (real: a Fiat
        // Grande Punto build carries `LmbdSmkHigh` 0x580 bytes from the
        // corpus address).
        let pending: Vec<&'static MapTemplate> =
            templates.iter().copied().filter(|t| !found.contains(t.bosch_label)).collect();
        if !pending.is_empty() {
            let hits = Self::axis_key_hits(data, &pending);
            for sequence in Self::sequences(&templates) {
                if sequence.iter().all(|t| found.contains(t.bosch_label)) {
                    continue;
                }
                self.resolve_sequence(data, &sequence, &hits, &mut maps, &mut claimed, &mut found);
            }
        }

        maps.sort_by_key(|m| m.address);
        maps
    }

    /// For the identifier: does `bosch_label`'s confirmed block decode at
    /// its real address on this file, under the same checks the detector
    /// applies (grid, axis ranges, Z range, not flat)? Uses the tuned Z
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

    /// On-disk size of one block of this template: header + both axes + Z.
    fn block_len(template: &MapTemplate) -> usize {
        C39_BLOCK_HEADER_LEN + 2 * (template.nx + template.ny) + 2 * template.nx * template.ny
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

    // ------------------------- PHASE 0: address -------------------------

    /// The exact confirmed address first, then outward by increasing
    /// distance up to `C39_ADDRESS_WINDOW`. Order matters: ten template
    /// pairs here are stored back to back, so `address - 64` lies inside
    /// the previous sibling's Z data, and a scan that simply walked upward
    /// from `address - 64` and kept the first structurally valid block
    /// reported a neighbour instead of the block at the address (regression
    /// test `phase0_prefers_the_block_at_the_confirmed_address`).
    fn detect_by_address(&self, data: &[u8], template: &MapTemplate) -> Option<DetectedMap> {
        let base = template.address;
        for d in (0..=C39_ADDRESS_WINDOW).step_by(2) {
            if let Some(map) = self.try_block(data, base + d, template) {
                return Some(map);
            }
            if d > 0 {
                if let Some(off) = base.checked_sub(d) {
                    if let Some(map) = self.try_block(data, off, template) {
                        return Some(map);
                    }
                }
            }
        }
        None
    }

    // ------------------------ PHASE 1: axis keys ------------------------

    /// Every location where any of `templates`' axis keys matches byte for
    /// byte, keyed by template. One pass over the file: the 4-byte
    /// `[nx][ny]` header is looked up first, the full key compared only on
    /// a header hit. A key matching more than `C39_MAX_KEY_HITS` places is
    /// dropped (a grid shared by that much of the file is not evidence).
    fn axis_key_hits(data: &[u8], templates: &[&'static MapTemplate]) -> HashMap<&'static str, Vec<usize>> {
        struct Probe {
            label: &'static str,
            bytes: Vec<u8>,
            hits: Vec<usize>,
            saturated: bool,
        }
        let mut probes: Vec<Probe> = Vec::new();
        let mut by_header: HashMap<[u8; 4], Vec<usize>> = HashMap::new();
        for template in templates {
            for key in template.axis_keys {
                let bytes = key.to_bytes();
                if bytes.len() < C39_BLOCK_HEADER_LEN + 8 {
                    continue;
                }
                let header = [bytes[0], bytes[1], bytes[2], bytes[3]];
                by_header.entry(header).or_default().push(probes.len());
                probes.push(Probe { label: template.bosch_label, bytes, hits: Vec::new(), saturated: false });
            }
        }

        let mut off = 0;
        while off + C39_BLOCK_HEADER_LEN <= data.len() {
            let header = [data[off], data[off + 1], data[off + 2], data[off + 3]];
            if let Some(indices) = by_header.get(&header) {
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

    /// Partition the (address-sorted) templates into runs of back-to-back
    /// blocks that share a grid: consecutive templates whose confirmed
    /// addresses are exactly one block apart, with the same grid shape and
    /// at least one identical axis key. Everything else is a run of one.
    fn sequences(templates: &[&'static MapTemplate]) -> Vec<Vec<&'static MapTemplate>> {
        let mut out: Vec<Vec<&'static MapTemplate>> = Vec::new();
        for &template in templates {
            let extends = out.last().map_or(false, |seq| {
                let prev = *seq.last().expect("non-empty sequence");
                prev.address + Self::block_len(prev) == template.address
                    && prev.nx == template.nx
                    && prev.ny == template.ny
                    && Self::share_axis_key(prev, template)
            });
            if extends {
                out.last_mut().expect("non-empty").push(template);
            } else {
                out.push(vec![template]);
            }
        }
        out
    }

    fn share_axis_key(a: &MapTemplate, b: &MapTemplate) -> bool {
        a.axis_keys.iter().any(|ka| {
            let ka = ka.to_bytes();
            b.axis_keys.iter().any(|kb| kb.to_bytes() == ka)
        })
    }

    /// Resolve one sequence from the axis-key hits.
    ///
    /// A run of one: the hits are tried nearest-to-confirmed-address first;
    /// the first that decodes under the template's checks AND does not
    /// overlap an already-emitted map wins; any hit beyond
    /// `C39_KEY_AMBIGUITY_WINDOW` is not tried at all (ambiguous, skipped).
    ///
    /// A run of N >= 2 members sharing a grid: their hits are pooled and
    /// only a COMPLETE run -- N hits exactly one block apart -- is trusted,
    /// the one starting nearest the sequence's confirmed start (again within
    /// the ambiguity window). Members are then assigned in storage order,
    /// which is how the run is laid out on every real file measured. A
    /// partial run, or none, means nothing is emitted for the members phase
    /// 0 did not find: a member resolved on its own would just be labelled
    /// after whichever sibling happens to sit closest to its old address.
    fn resolve_sequence(
        &self,
        data: &[u8],
        sequence: &[&'static MapTemplate],
        hits: &HashMap<&'static str, Vec<usize>>,
        maps: &mut Vec<DetectedMap>,
        claimed: &mut HashSet<(u32, u32)>,
        found: &mut HashSet<&'static str>,
    ) {
        let first = sequence[0];

        if sequence.len() == 1 {
            let Some(candidates) = hits.get(first.bosch_label) else {
                return;
            };
            let mut ordered: Vec<usize> = candidates.clone();
            ordered.sort_by_key(|&h| h.abs_diff(first.address));
            for hit in ordered {
                if hit.abs_diff(first.address) > C39_KEY_AMBIGUITY_WINDOW {
                    break;
                }
                if let Some(map) = self.try_block_keyed(data, hit, first) {
                    if Self::push_if_free(maps, claimed, map) {
                        found.insert(first.bosch_label);
                        return;
                    }
                }
            }
            return;
        }

        let stride = Self::block_len(first);
        let mut pool: Vec<usize> =
            sequence.iter().filter_map(|t| hits.get(t.bosch_label)).flatten().copied().collect();
        pool.sort_unstable();
        pool.dedup();
        let pool_set: HashSet<usize> = pool.iter().copied().collect();

        let complete_runs = pool
            .iter()
            .copied()
            .filter(|&start| (1..sequence.len()).all(|k| pool_set.contains(&(start + k * stride))));
        let Some(start) = complete_runs.min_by_key(|&s| s.abs_diff(first.address)) else {
            return;
        };
        if start.abs_diff(first.address) > C39_KEY_AMBIGUITY_WINDOW {
            return;
        }
        for (k, template) in sequence.iter().enumerate() {
            if found.contains(template.bosch_label) {
                continue;
            }
            if let Some(map) = self.try_block_keyed(data, start + k * stride, template) {
                if Self::push_if_free(maps, claimed, map) {
                    found.insert(template.bosch_label);
                }
            }
        }
    }

    /// Phase-1 decode: same checks as phase 0 (including the stock/tuned Z
    /// range the detector was built with), plus the axis-key confidence
    /// bonus.
    fn try_block_keyed(&self, data: &[u8], off: usize, template: &MapTemplate) -> Option<DetectedMap> {
        let mut map = self.try_block(data, off, template)?;
        map.confidence = (map.confidence + C39_AXIS_KEY_BONUS).min(0.99);
        Some(map)
    }

    // ------------------------ block decoding ------------------------

    /// Decode the block at `off` without any interpretation.
    pub fn read_block(data: &[u8], off: usize, max_nx: usize, max_ny: usize) -> Option<CalBlock> {
        let nx = Self::read_u16_be(data, off)? as usize;
        let ny = Self::read_u16_be(data, off + 2)? as usize;
        if nx < 2 || ny < 2 || nx > max_nx || ny > max_ny {
            return None;
        }
        let x_off = off + C39_BLOCK_HEADER_LEN;
        let y_off = x_off + 2 * nx;
        let data_addr = y_off + 2 * ny;
        let x_axis = Self::read_i16_be_slice(data, x_off, nx)?;
        let y_axis = Self::read_i16_be_slice(data, y_off, ny)?;
        let z = Self::read_i16_be_slice(data, data_addr, nx * ny)?;
        Some(CalBlock { addr: off, nx, ny, x_axis, y_axis, data_addr, z })
    }

    fn try_block(&self, data: &[u8], off: usize, template: &MapTemplate) -> Option<DetectedMap> {
        let (x_type, y_type) = template.axes;
        let block = Self::read_block(data, off, C39_MAX_AXIS_PTS, C39_MAX_AXIS_PTS)?;

        if block.nx != template.nx || block.ny != template.ny {
            return None;
        }
        if !Self::axis_is_valid(&block.x_axis, x_type) || !Self::axis_is_valid(&block.y_axis, y_type) {
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

        let score = Self::score(&values, block.nx, block.ny, template, self.tuned_mode)?;
        Some(Self::build_map(&block, template, score))
    }

    fn score(values: &[f64], nx: usize, ny: usize, template: &MapTemplate, wide_range: bool) -> Option<f64> {
        let (lo, hi) = if wide_range { template.z_range_tuned } else { template.z_range_stock };
        if values.iter().any(|v| *v < lo || *v > hi) {
            return None;
        }
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        if (max - min).abs() < f64::EPSILON {
            return None;
        }
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

    fn build_map(block: &CalBlock, template: &MapTemplate, score: f64) -> DetectedMap {
        let (x_type, y_type) = template.axes;
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
        // Rows of the record are the first axis in memory (X, nx entries),
        // columns the second (Y, ny entries). The frontend's left axis is
        // the row axis, so it is served the record's X axis, and vice versa.
        map.x_axis_address = Some((block.addr + C39_BLOCK_HEADER_LEN + 2 * block.nx) as u32);
        map.y_axis_address = Some((block.addr + C39_BLOCK_HEADER_LEN) as u32);
        map.x_axis_correction = Some(y_type.factor());
        map.y_axis_correction = Some(x_type.factor());
        map.x_axis_offset = Some(y_type.offset());
        map.y_axis_offset = Some(x_type.offset());
        map.x_label = Some(y_type.label().to_string());
        map.y_label = Some(x_type.label().to_string());
        map.y_axis_inverted = Some(false);
        map.description = Some(format!(
            "EDC16C39 {} (Bosch: {}) - block 0x{:06X}, {}x{}",
            template.name, template.bosch_label, block.addr, block.nx, block.ny
        ));
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

    /// Plant a block at `addr`: the given header+axes bytes, then `nx` rows
    /// of `z_row` (one Z value per Y breakpoint).
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

    /// The real EGR block (from the corpus) at `addr`, Z = the Y axis
    /// values repeated per row (0..100 %, in range, monotonic).
    fn plant_egr(data: &mut [u8], addr: usize) {
        let key = &template(EGR).axis_keys[0];
        plant(data, addr, key.nx, key.ny, key.x, key.y, key.y);
    }

    fn names(maps: &[DetectedMap]) -> Vec<String> {
        maps.iter().map(|m| m.name.clone().unwrap_or_default()).collect()
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
        assert_eq!(maps[0].y_axis_address, Some((t.address + C39_BLOCK_HEADER_LEN) as u32));
    }

    /// Ten template pairs are stored back to back, so `address - 64` is
    /// inside the previous block's Z data: a valid decoy there must not beat
    /// the block at the confirmed address.
    #[test]
    fn phase0_prefers_the_block_at_the_confirmed_address() {
        let mut data = blank();
        let t = template(EGR);
        plant_egr(&mut data, t.address - 64);
        plant_egr(&mut data, t.address);
        let maps = EDC16C39Detector::new().detect(&data);
        assert_eq!(names(&maps), vec![EGR_NAME]);
        assert_eq!(maps[0].y_axis_address, Some((t.address + C39_BLOCK_HEADER_LEN) as u32));
    }

    /// The five `InjCrv_Bas` curves share one grid and are stored in order,
    /// one block apart. On a build shifted by a whole block or more, each
    /// member's nearest hit is a sibling's block; only resolving the run as
    /// a whole keeps the labels in storage order. Both shift directions.
    #[test]
    fn a_shifted_sibling_run_keeps_its_labels_in_storage_order() {
        for shift in [-0x400i64, 0x400, -0x1000] {
            let mut data = blank();
            let members: Vec<&MapTemplate> =
                ["InjCrv_Bas1", "InjCrv_Bas2", "InjCrv_Bas3", "InjCrv_Bas4", "InjCrv_Bas5"]
                    .iter()
                    .map(|l| template(l))
                    .collect();
            let key = &members[0].axis_keys[0];
            let stride = EDC16C39Detector::block_len(members[0]);
            let start = (members[0].address as i64 + shift) as usize;
            for k in 0..members.len() {
                // Distinct, in-range Z per curve so each block is
                // identifiable: raw k*10 .. (deg = raw * 0.0234).
                let z_row: Vec<i16> = (0..key.ny as i16).map(|c| (k as i16) * 10 + c).collect();
                plant(&mut data, start + k * stride, key.nx, key.ny, key.x, key.y, &z_row);
            }
            let maps = EDC16C39Detector::new().detect(&data);
            let mut got: Vec<(u32, String)> = maps.iter().map(|m| (m.address, m.name.clone().unwrap())).collect();
            got.sort();
            let expected: Vec<(u32, String)> = (0..5)
                .map(|k| {
                    let block = start + k * stride;
                    (
                        (block + C39_BLOCK_HEADER_LEN + 2 * (key.nx + key.ny)) as u32,
                        format!("Injection Timing {}", k + 1),
                    )
                })
                .collect();
            assert_eq!(got, expected, "shift {:#x}", shift);
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

    /// Every harvested axis key is real data read from a real file, so the
    /// structural axis gates must accept all of them -- a key the gate
    /// rejects is dead code and silently loses that build's map.
    #[test]
    fn every_axis_key_passes_its_templates_axis_gates() {
        for t in MAP_TEMPLATES.iter().filter(|t| t.calibrated) {
            assert!(!t.axis_keys.is_empty(), "{} has no axis key", t.name);
            for (i, key) in t.axis_keys.iter().enumerate() {
                assert_eq!((key.nx, key.ny), (t.nx, t.ny), "{} key {} grid", t.bosch_label, i);
                assert!(
                    EDC16C39Detector::axis_is_valid(key.x, t.axes.0),
                    "{} key {}: X {:?} rejected by {:?}",
                    t.bosch_label,
                    i,
                    key.x,
                    t.axes.0
                );
                assert!(
                    EDC16C39Detector::axis_is_valid(key.y, t.axes.1),
                    "{} key {}: Y {:?} rejected by {:?}",
                    t.bosch_label,
                    i,
                    key.y,
                    t.axes.1
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

    /// An unsigned template with a negative floor can never reach that
    /// floor (`v as u16` is never negative): the gate would be dead. Every
    /// unsigned family's floors must be >= 0.
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
