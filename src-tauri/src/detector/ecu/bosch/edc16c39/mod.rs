// Bosch EDC16C39 detector (Fiat/Alfa Romeo Multijet common rail).
//
// STATUS: CALIBRATED for the Alfa 159/147-class 2MB passenger-car layout
// only. See `signatures.rs`'s module doc for the corpus and this module's
// "Known limits" section below for what is NOT yet covered.
//
// Why this detector does not look like the VAG ones
// ---------------------------------------------------
// The VAG EDC16 modules scan for a plausible [axis][axis][data] triple and
// guess the grid from a list of candidate sizes. EDC16C39 does not need
// that: every 2D map here uses a self-describing record --
//
//   +0x00  u16        nx          number of X points (engine speed)
//   +0x02  u16        ny          number of Y points (second axis)
//   +0x04  i16[nx]    X axis      strictly increasing
//   +....  i16[ny]    Y axis      strictly increasing
//   +....  i16[nx*ny] Z, column direction (Y is the fast index)
//
// so the grid is read, not guessed -- same record shape independently
// confirmed on the (unrelated OEM) EDC16CP31 family, and reused here as the
// same generic mechanism, not shared code: this module is its own
// implementation, calibrated against a completely different real corpus.
//
// A second real finding, specific to this family and DIFFERENT from CP31:
// on every one of the 3 real dumps in the corpus, a given family's block
// sits at the EXACT SAME ADDRESS, not just the same axis-key shape. CP31's
// addresses moved per software build; these do not, at least across the
// three real builds checked. `MapTemplate::address` is therefore used as a
// primary signal (a small window around it), with the axis key as a second,
// address-independent check -- see `detect_by_address` and
// `detect_by_axis_keys` below.
//
// Safety contract of this module
// -------------------------------
// `detect()` returns an empty vector for any file that is not the
// calibrated 2MB layout, and never falls back to the VAG detectors. Per
// CONTRIBUTING.md: "A false 'unsupported file' is annoying; a false 'this
// is an EDC16' corrupts someone's ECU."
//
// Known limits of a 3-file corpus
// ---------------------------------
//   * All 3 corpus files are Alfa Romeo 159/147 1.9 JTD/JTDm passenger cars.
//     Fiat Ducato (commercial van, same "EDC16C39" WinOLS label) was checked
//     directly against this layout and does NOT match at these addresses --
//     it is a different physical calibration on the same base chip and is
//     explicitly NOT covered. Fiat Bravo/Doblo/Croma/Punto and other Fiat
//     Group passenger cars sharing the Alfa platform are UNCHECKED, not
//     assumed to work.
//   * `z_range_stock`/`z_range_tuned` are derived from only 3 real files'
//     observed spans, generously widened -- not yet the "20 files, know
//     what's always there" bar CONTRIBUTING.md asks for.
//   * No completeness/invariant report yet (needs a real multi-file bench,
//     see docs/PORTING-EDC16C39.md).
//   * 1D curves (TrqMaxGear1..6+R, EngPrt_trqLim) and scalars (SpdLimMax,
//     PCR_DesMax) use a different on-disk record this module does not read
//     yet -- confirmed present (see docs/PORTING-EDC16C39.md) but not
//     implemented, matching CP31's own staged curve support.
//   * `TrqStrtBas` was seen at a stable address but with a DIFFERENT grid on
//     one of the 3 files (16x16 vs 10x16) -- deliberately left uncalibrated
//     rather than guessed at.

mod signatures;

pub use signatures::{AxisKey, AxisType, MapTemplate, MAP_TEMPLATES};

use crate::models::{DataType, DetectedMap, MapDimensions};
use std::collections::HashSet;

const C39_BLOCK_HEADER_LEN: usize = 4;
const C39_MAX_AXIS_PTS: usize = 32;
const C39_AXIS_KEY_BONUS: f32 = 0.15;
/// A key matching more places than this describes a grid shared by half the
/// file, not a family -- discarded rather than trusted.
const C39_MAX_KEY_HITS: usize = 64;
/// Half-width of the search window placed around `MapTemplate::address`.
/// Wide enough to absorb a few bytes of drift from an unrelated calibration
/// change earlier in the file, narrow enough that it cannot wander into a
/// neighbouring family's block.
const C39_ADDRESS_WINDOW: usize = 64;
/// When an axis key matches more than once, only a hit within this many
/// bytes of the confirmed address is trusted -- see `detect_by_axis_keys`.
/// Wider than `C39_ADDRESS_WINDOW` on purpose: this is the fallback for a
/// build where the block moved further than a few bytes, so it needs more
/// room, while still being far short of "anywhere in the file".
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
    /// Corpus-analysis mode: ignores address windows and axis-key hit caps
    /// to see what the templates would match anywhere in the file. Results
    /// are hypotheses to eyeball, never wired to the UI.
    exploratory: bool,
}

impl EDC16C39Detector {
    pub fn new() -> Self {
        Self { tuned_mode: false, exploratory: false }
    }

    pub fn new_tuned() -> Self {
        Self { tuned_mode: true, exploratory: false }
    }

    pub fn exploratory() -> Self {
        Self { tuned_mode: true, exploratory: true }
    }

    pub fn is_calibrated() -> bool {
        MAP_TEMPLATES.iter().any(|t| t.calibrated)
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
        if !self.exploratory && !Self::is_calibrated() {
            return Vec::new();
        }

        let mut maps: Vec<DetectedMap> = Vec::new();
        let mut claimed: HashSet<(u32, u32)> = HashSet::new();

        // PHASE 0: confirmed address, +/- a small window. The primary
        // signal for this family -- see this module's header.
        for map in self.detect_by_address(data) {
            self.push_if_free(&mut maps, &mut claimed, map);
        }

        // PHASE 1: axis-key walk, address-independent. Catches a build
        // where the address moved (not observed yet, but the corpus is
        // only 3 files -- see "Known limits").
        for map in self.detect_by_axis_keys(data) {
            self.push_if_free(&mut maps, &mut claimed, map);
        }

        maps.sort_by_key(|m| m.address);
        maps
    }

    fn push_if_free(&self, maps: &mut Vec<DetectedMap>, claimed: &mut HashSet<(u32, u32)>, map: DetectedMap) {
        let range = (map.address, map.address + map.size as u32);
        if claimed.iter().any(|&(s, e)| range.0 < e && s < range.1) {
            return;
        }
        claimed.insert(range);
        maps.push(map);
    }

    // ------------------------- PHASE 0: address -------------------------

    fn detect_by_address(&self, data: &[u8]) -> Vec<DetectedMap> {
        let mut out = Vec::new();
        for template in MAP_TEMPLATES {
            if !self.exploratory && !template.calibrated {
                continue;
            }
            let lo = template.address.saturating_sub(C39_ADDRESS_WINDOW);
            let hi = (template.address + C39_ADDRESS_WINDOW).min(data.len());
            let mut off = lo;
            let mut found = false;
            while off < hi && !found {
                if let Some(map) = self.try_block(data, off, template) {
                    out.push(map);
                    found = true;
                }
                off += 2;
            }
        }
        out
    }

    // ------------------------ PHASE 1: axis keys ------------------------

    /// A `MapTemplate` carries the exact `[nx][ny] + X + Y` bytes of the one
    /// block it describes, read from 3 real dumps. Searching for those bytes
    /// identifies the family without knowing where it lives -- but several
    /// of this family's breakpoint grids turn out to be shared by more than
    /// one real map (a common RPM/quantity axis reused across otherwise
    /// unrelated tables, discovered empirically against real files: see
    /// signatures.rs's module doc), so a raw hit list is NOT enough evidence
    /// on its own here, unlike a rarer grid would be. Kept conservative:
    /// when a key matches more than once, only hits close to the
    /// independently-confirmed `template.address` are trusted; if NONE are
    /// close and there is more than one hit, the whole key is treated as
    /// ambiguous and skipped rather than guessed at.
    fn detect_by_axis_keys(&self, data: &[u8]) -> Vec<DetectedMap> {
        let mut out = Vec::new();
        for template in MAP_TEMPLATES {
            if !self.exploratory && !template.calibrated {
                continue;
            }
            for key in template.axis_keys {
                let bytes = key.to_bytes();
                if bytes.len() < C39_BLOCK_HEADER_LEN + 8 {
                    continue;
                }
                let mut hits: Vec<usize> = Vec::new();
                let mut off = 0;
                while off + bytes.len() <= data.len() {
                    if data[off..off + bytes.len()] == bytes[..] {
                        hits.push(off);
                        if hits.len() > C39_MAX_KEY_HITS {
                            break;
                        }
                    }
                    off += 2;
                }
                if hits.is_empty() || hits.len() > C39_MAX_KEY_HITS {
                    continue;
                }
                if hits.len() > 1 {
                    let near_confirmed: Vec<usize> = hits
                        .iter()
                        .copied()
                        .filter(|&h| h.abs_diff(template.address) <= C39_KEY_AMBIGUITY_WINDOW)
                        .collect();
                    if near_confirmed.is_empty() {
                        // Ambiguous: this grid is shared by several tables and none
                        // of the hits is where we independently know this family
                        // lives. Skip rather than guess -- see this fn's doc.
                        continue;
                    }
                    hits = near_confirmed;
                }
                for hit in hits {
                    if let Some(mut map) = self.try_block_ranged(data, hit, template, true) {
                        map.confidence = (map.confidence + C39_AXIS_KEY_BONUS).min(0.99);
                        out.push(map);
                    }
                }
            }
        }
        out
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
        self.try_block_ranged(data, off, template, self.tuned_mode)
    }

    fn try_block_ranged(
        &self,
        data: &[u8],
        off: usize,
        template: &MapTemplate,
        wide_range: bool,
    ) -> Option<DetectedMap> {
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

        let score = self.score(&values, block.nx, block.ny, template, wide_range)?;
        Some(self.build_map(&block, template, score))
    }

    fn score(&self, values: &[f64], nx: usize, ny: usize, template: &MapTemplate, wide_range: bool) -> Option<f64> {
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

    fn build_map(&self, block: &CalBlock, template: &MapTemplate, score: f64) -> DetectedMap {
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
        map.x_axis_address = Some((block.addr + C39_BLOCK_HEADER_LEN + 2 * block.nx) as u32);
        map.y_axis_address = Some((block.addr + C39_BLOCK_HEADER_LEN) as u32);
        map.x_axis_correction = Some(y_type.factor());
        map.y_axis_correction = Some(x_type.factor());
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

    /// Builds a minimal 2MB buffer with exactly one real calibrated block
    /// (EGR duty cycle, from the real corpus) planted at its confirmed
    /// address, everything else zeroed.
    fn fixture_with_egr_block() -> Vec<u8> {
        let mut data = vec![0u8; 2_097_152];
        let template = MAP_TEMPLATES.iter().find(|t| t.bosch_label == "AirCtl_rEGRBas").unwrap();
        let key = &template.axis_keys[0];
        let bytes = key.to_bytes();
        // Z: reuse the Y axis values as the data rows (matches the real
        // corpus files, which all happen to carry this exact pattern on at
        // least one saved version).
        let mut z = Vec::new();
        for _ in 0..key.nx {
            for &y in key.y {
                z.extend_from_slice(&y.to_be_bytes());
            }
        }
        let addr = template.address;
        data[addr..addr + bytes.len()].copy_from_slice(&bytes);
        data[addr + bytes.len()..addr + bytes.len() + z.len()].copy_from_slice(&z);
        data
    }

    #[test]
    fn detects_the_egr_block_at_its_confirmed_address() {
        let data = fixture_with_egr_block();
        let maps = EDC16C39Detector::new().detect(&data);
        assert!(maps.iter().any(|m| m.name.as_deref() == Some("EGR Duty Cycle Base")));
    }

    #[test]
    fn detects_the_egr_block_by_axis_key_even_when_moved() {
        // Same block, moved 0x1000 bytes away from its usual address --
        // only the axis-key phase (not the address phase) can still find it.
        let mut data = vec![0u8; 2_097_152];
        let template = MAP_TEMPLATES.iter().find(|t| t.bosch_label == "AirCtl_rEGRBas").unwrap();
        let key = &template.axis_keys[0];
        let bytes = key.to_bytes();
        let mut z = Vec::new();
        for _ in 0..key.nx {
            for &y in key.y {
                z.extend_from_slice(&y.to_be_bytes());
            }
        }
        let addr = template.address + 0x1000;
        data[addr..addr + bytes.len()].copy_from_slice(&bytes);
        data[addr + bytes.len()..addr + bytes.len() + z.len()].copy_from_slice(&z);

        let maps = EDC16C39Detector::new().detect(&data);
        assert!(maps.iter().any(|m| m.name.as_deref() == Some("EGR Duty Cycle Base")));
    }

    #[test]
    fn a_file_of_the_wrong_size_yields_no_maps() {
        let data = vec![0u8; 1_048_576];
        assert!(EDC16C39Detector::new().detect(&data).is_empty());
    }

    #[test]
    fn random_noise_at_the_confirmed_address_is_not_reported() {
        let mut data = vec![0u8; 2_097_152];
        let template = MAP_TEMPLATES.iter().find(|t| t.bosch_label == "AirCtl_rEGRBas").unwrap();
        for (i, b) in data[template.address..template.address + 256].iter_mut().enumerate() {
            *b = ((i * 7 + 13) % 251) as u8;
        }
        let maps = EDC16C39Detector::new().detect(&data);
        assert!(!maps.iter().any(|m| m.name.as_deref() == Some("EGR Duty Cycle Base")));
    }

    #[test]
    fn every_calibrated_template_has_a_non_empty_axis_key() {
        for t in MAP_TEMPLATES {
            if t.calibrated {
                assert!(!t.axis_keys.is_empty(), "{} has no axis key", t.name);
            }
        }
    }
}
