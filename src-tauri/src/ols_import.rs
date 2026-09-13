//! WinOLS `.ols` project file import.
//!
//! ZedSuite's detectors work on a raw ECU dump (flash/EEPROM bytes starting at the real base
//! address). A `.ols` file is NOT that: it is a WinOLS project *container* -- a small metadata
//! header (vehicle/ECU identification, free-text notes) followed by one or more saved ROM
//! *versions* (e.g. "Original" + "Stage 1", both kept in the same file so WinOLS can diff them).
//! Handing a `.ols` file's raw bytes straight to `ECUIdentifier::identify`/`MapDetector` -- which
//! is what happened before this module existed -- reads the metadata header as if it were ROM
//! content and, on a multi-version file, would not know which of the N saved copies to use.
//!
//! This module only recognises the container and extracts one chosen version's raw bytes; it
//! changes nothing about identification or detection, which keep running exactly as before on
//! whatever bytes they are given (now the correctly-extracted ROM instead of the raw container).
//!
//! Field layout, offsets and the version-directory anchor below are container *format knowledge*
//! -- independently reimplemented by NXT Tronic against real `.ols` files from a large tuning
//! archive, cross-checked against the documentation and reference implementation in the
//! GPL-3.0-or-later "romHEX14" project (<https://github.com/ctabuyo/romHEX14-community>,
//! `src/io/ols/{OlsHeader,OlsProjectMetadata,OlsMagicScanner,OlsVersionDirectory}.cpp`). Not
//! vendored or translated line-for-line: different types, different decomposition, no shared
//! text -- the same clean-room posture as every other reimplementation of this format.

const OLS_HEADER_MAGIC: u32 = 0x0000_000B;
const OLS_HEADER_LEN: usize = 24;
const SIGNATURE: &[u8] = b"WinOLS File";

/// Generous upper bound on a single string field's declared length -- large enough for any real
/// Notes/RevisionTag field, small enough to reject a misaligned read that lands on raw ROM binary
/// data and misreads a few of its bytes as a length prefix (observed on real files: an unrelated
/// field absorbing 100+ KB of ROM content as "string" data before a cap like this existed).
const MAX_CSTRING_LEN: usize = 8192;

/// Only the first 4096 bytes of the file are a legitimate home for the version-directory anchor --
/// a spurious 4-byte match deep inside megabytes of ROM data doesn't count.
const ANCHOR_SCAN_LIMIT: usize = 4096;

/// One of several magic markers WinOLS scatters through a `.ols` file as section anchors; this is
/// the one that sits immediately after the version directory's 12-byte header.
const M1_MAGIC: u32 = 0x4200_7899;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OlsHeader {
    format_version: u32,
}

fn parse_header(data: &[u8]) -> Option<OlsHeader> {
    if data.len() < OLS_HEADER_LEN {
        return None;
    }
    let magic = u32::from_le_bytes(data[0..4].try_into().unwrap());
    if magic != OLS_HEADER_MAGIC || &data[4..15] != SIGNATURE || data[15] != 0x00 {
        return None;
    }
    let format_version = u32::from_le_bytes(data[16..20].try_into().unwrap());
    Some(OlsHeader { format_version })
}

/// Where the metadata block starts, relative to `format_version`. WinOLS 4.x-era files (format
/// version < 200) start the same reader 4 bytes earlier than the modern 24-byte header layout.
fn metadata_offset(format_version: u32) -> usize {
    if format_version >= 200 { 0x18 } else { 0x14 }
}

/// Cursor over the metadata block's length-prefixed fields (MFC `CArchive`-style: a 4-byte
/// little-endian length, then that many bytes).
struct FieldReader<'a> {
    data: &'a [u8],
    pos: usize,
    format_version: u32,
}

impl<'a> FieldReader<'a> {
    fn new(data: &'a [u8], pos: usize, format_version: u32) -> Self {
        Self { data, pos, format_version }
    }

    fn ensure(&self, n: usize) -> Option<()> {
        (self.pos.saturating_add(n) <= self.data.len()).then_some(())
    }

    /// A length-prefixed `CString` field. From format_version 439 onward the length itself can
    /// carry one of three sentinel negative codes WinOLS uses for blank states instead of writing
    /// an actual string body; below 439 a non-positive length just means "empty", and a positive
    /// length is followed by one literal NUL byte on disk.
    fn cstring(&mut self) -> Option<String> {
        self.ensure(4)?;
        let len = i32::from_le_bytes(self.data[self.pos..self.pos + 4].try_into().unwrap());
        self.pos += 4;

        if self.format_version >= 439 {
            return match len {
                -1 => Some("-".to_string()),
                -2 => Some("?".to_string()),
                -3 => Some("%".to_string()),
                n if n <= 0 => Some(String::new()),
                n => {
                    let n = n as usize;
                    if n > MAX_CSTRING_LEN {
                        return None;
                    }
                    self.ensure(n)?;
                    let body = &self.data[self.pos..self.pos + n];
                    self.pos += n;
                    Some(decode_ols_string(body))
                }
            };
        }

        if len <= 0 {
            return Some(String::new());
        }
        let n = len as usize;
        if n > MAX_CSTRING_LEN {
            return None;
        }
        self.ensure(n + 1)?;
        let body = &self.data[self.pos..self.pos + n];
        self.pos += n;
        let nul = self.data[self.pos];
        self.pos += 1;
        (nul == 0x00).then(|| decode_ols_string(body))
    }

    /// Reads and discards one `CString` field, for a field this module doesn't surface.
    fn skip_string(&mut self) -> Option<()> {
        self.cstring().map(|_| ())
    }

    /// Reads and discards one 8-byte field (`LastWriteTime`), for symmetry with `skip_string`.
    fn skip_u64(&mut self) -> Option<()> {
        self.ensure(8)?;
        self.pos += 8;
        Some(())
    }
}

/// WinOLS writes plain ASCII almost always, occasionally UTF-8, and -- on older files, for
/// accented characters -- Windows-1252. Legible-enough decoding: ASCII fast path, then UTF-8,
/// then a lossy byte-as-codepoint fallback.
fn decode_ols_string(raw: &[u8]) -> String {
    if raw.iter().all(|&b| b < 0x80) {
        return String::from_utf8_lossy(raw).into_owned();
    }
    if let Ok(s) = std::str::from_utf8(raw) {
        return s.to_string();
    }
    raw.iter().map(|&b| b as char).collect()
}

/// The handful of metadata fields worth showing in the version picker. Every project field before
/// `RevisionTag` in the on-disk order (see `parse_light_metadata`) has a fixed position, so this
/// only walks as far as needed and stops -- no need to parse the later, version-gated fields this
/// module doesn't use.
#[derive(Debug, Clone, Default)]
pub struct OlsMetadata {
    pub make: String,
    pub model: String,
    pub manufacturer: String,
    pub ecu_name: String,
    pub hw_number: String,
    pub sw_number: String,
    pub revision_tag: String,
}

/// Reads only the fields `OlsMetadata` exposes, in their real on-disk order, stopping (leaving
/// the rest at their default) at the first field it can't read rather than guessing.
fn parse_light_metadata(data: &[u8], offset: usize, format_version: u32) -> OlsMetadata {
    let mut m = OlsMetadata::default();
    let mut r = FieldReader::new(data, offset, format_version);

    macro_rules! field {
        ($dst:expr) => {
            match r.cstring() {
                Some(v) => $dst = v,
                None => return m,
            }
        };
    }
    macro_rules! skip {
        () => {
            if r.skip_string().is_none() {
                return m;
            }
        };
    }

    field!(m.make);
    field!(m.model);
    skip!(); // VehicleType
    skip!(); // Year
    skip!(); // FuelType
    skip!(); // CapacityRaw
    skip!(); // Output_kW_PS
    skip!(); // Drivetrain
    skip!(); // Memory
    field!(m.manufacturer);
    field!(m.ecu_name);
    field!(m.hw_number);
    field!(m.sw_number);
    skip!(); // ProductionNo
    skip!(); // EngineCode
    skip!(); // Transmission
    if r.skip_u64().is_none() {
        return m; // LastWriteTime
    }
    skip!(); // OriginalFileName
    skip!(); // OLSVersionString
    skip!(); // Reserved2
    field!(m.revision_tag);

    m
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionDirectory {
    pub num_versions: u32,
    pub version_data_start: u32,
    pub version_record_size: u32,
}

/// Looks for the M1 anchor within the first 4096 bytes of `prefix` and, if found, reads the
/// 12-byte version-directory header sitting immediately before it: `[numVersions-1: u32]
/// [versionDataStart: u32] [versionRecordSize: u32]`, immediately followed by the anchor itself.
fn find_version_directory(prefix: &[u8], total_file_size: u64) -> Option<VersionDirectory> {
    let scan_limit = prefix.len().min(ANCHOR_SCAN_LIMIT);
    let needle = M1_MAGIC.to_le_bytes();
    let anchor_pos = prefix[..scan_limit].windows(4).position(|w| w == needle)?;
    if anchor_pos < 12 {
        return None;
    }
    let hdr_ofs = anchor_pos - 12;
    if hdr_ofs + 16 > prefix.len() {
        return None;
    }

    let n_minus1 = u32::from_le_bytes(prefix[hdr_ofs..hdr_ofs + 4].try_into().unwrap());
    let v_start = u32::from_le_bytes(prefix[hdr_ofs + 4..hdr_ofs + 8].try_into().unwrap());
    let v_rec_sz = u32::from_le_bytes(prefix[hdr_ofs + 8..hdr_ofs + 12].try_into().unwrap());
    if v_rec_sz == 0 || u64::from(v_start) >= total_file_size {
        return None;
    }

    let num_versions = n_minus1.wrapping_add(1);
    let expected_end = u64::from(v_start) + 4 + u64::from(v_rec_sz) * u64::from(num_versions);
    if expected_end != total_file_size {
        // Layout doesn't fit as cleanly as usual -- treat as "not a version directory" rather
        // than risk slicing an extraction at the wrong offsets.
        return None;
    }

    Some(VersionDirectory { num_versions, version_data_start: v_start, version_record_size: v_rec_sz })
}

/// Splits a `RevisionTag` string into one label per saved version, or `None` if it doesn't match
/// either known convention (or genuinely doesn't apply -- e.g. an unfamiliar WinOLS revision).
/// Two label conventions seen on real files: entries separated by the literal 2-byte sequence
/// `,\t` (comma immediately followed by a tab), or -- a different tuning shop's convention --
/// plain `,` with no tab. Both are only trusted when they split into EXACTLY `expected_versions`
/// entries (from the version directory itself, never guessed from this string alone); an
/// unfamiliar shape returns `None` rather than mislabeling a version.
fn parse_revision_labels(raw: &str, expected_versions: u32) -> Option<Vec<String>> {
    if expected_versions == 0 {
        return None;
    }
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    if close <= open {
        return None;
    }
    let inner = &raw[open + 1..close];

    let clean = |s: &str| -> String {
        // "Name:\tValue" -> "Name (Value)"; a bare word is kept as-is.
        match s.find(":\t") {
            Some(idx) => {
                let name = s[..idx].trim();
                let value = s[idx + 2..].trim();
                if value.is_empty() { name.to_string() } else { format!("{name} ({value})") }
            }
            None => s.trim().to_string(),
        }
    };

    let tab_entries: Vec<&str> = inner.split(",\t").collect();
    if tab_entries.len() as u32 == expected_versions {
        return Some(tab_entries.into_iter().map(clean).collect());
    }
    let comma_entries: Vec<&str> = inner.split(',').collect();
    if comma_entries.len() as u32 == expected_versions {
        return Some(comma_entries.into_iter().map(clean).collect());
    }
    None
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OlsVersionInfo {
    pub index: u32,
    pub size: u64,
    /// Human-readable label for this saved version ("Original", "Stage 2 (DPFoff EGRoff)", ...)
    /// when `RevisionTag` could be confidently split; `None` otherwise (still a perfectly usable
    /// version, just without a per-step description).
    pub label: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OlsInspection {
    pub make: String,
    pub model: String,
    pub manufacturer: String,
    pub ecu_name: String,
    pub hw_number: String,
    pub sw_number: String,
    pub versions: Vec<OlsVersionInfo>,
}

/// Recognises a `.ols` WinOLS project container and lists its saved versions, or `None` when
/// `data` isn't one (wrong magic/signature, or no version directory found) -- callers should fall
/// back to treating the file as a raw dump exactly as before this module existed.
pub fn inspect(data: &[u8]) -> Option<OlsInspection> {
    let header = parse_header(data)?;
    let meta = parse_light_metadata(data, metadata_offset(header.format_version), header.format_version);
    let dir = find_version_directory(data, data.len() as u64)?;

    let labels = parse_revision_labels(&meta.revision_tag, dir.num_versions);
    let versions = (0..dir.num_versions)
        .map(|i| OlsVersionInfo {
            index: i,
            size: u64::from(dir.version_record_size),
            label: labels.as_ref().and_then(|l| l.get(i as usize).cloned()),
        })
        .collect();

    Some(OlsInspection {
        make: meta.make,
        model: meta.model,
        manufacturer: meta.manufacturer,
        ecu_name: meta.ecu_name,
        hw_number: meta.hw_number,
        sw_number: meta.sw_number,
        versions,
    })
}

/// Extracts one saved version's raw ROM bytes (by index, as listed by `inspect`) out of a `.ols`
/// container. `data` must be the FULL file (not just the prefix `inspect` needs) since a version's
/// bytes can sit anywhere up to the end of the file.
pub fn extract_version(data: &[u8], version_index: u32) -> Result<Vec<u8>, String> {
    let header = parse_header(data).ok_or("not a .ols file (bad header/signature)")?;
    let _ = header; // format_version isn't needed again here, only for the metadata block above.
    let dir = find_version_directory(data, data.len() as u64).ok_or("no version directory found")?;
    if version_index >= dir.num_versions {
        return Err(format!(
            "version_index {version_index} out of range (file has {} version(s))",
            dir.num_versions
        ));
    }

    let rec_sz = dir.version_record_size as usize;
    let start = dir.version_data_start as usize + 4 + rec_sz * version_index as usize;
    let end = start + rec_sz;
    if end > data.len() {
        return Err("version data extends past end of file".to_string());
    }
    Ok(data[start..end].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encodes a `CString` field the way format_version < 439 files do: 4-byte LE length, the
    /// bytes, and (only when non-empty) one trailing NUL.
    fn enc_str(s: &str) -> Vec<u8> {
        let mut out = (s.len() as i32).to_le_bytes().to_vec();
        if !s.is_empty() {
            out.extend_from_slice(s.as_bytes());
            out.push(0);
        }
        out
    }

    /// A minimal single-version `.ols`-shaped buffer: real header + just enough metadata fields
    /// to reach RevisionTag, followed by a version directory (1 version) and that version's raw
    /// "ROM" bytes.
    fn single_version_file(ecu_name: &str, rom: &[u8]) -> Vec<u8> {
        let format_version: u32 = 288;
        let mut buf = Vec::new();
        buf.extend_from_slice(&OLS_HEADER_MAGIC.to_le_bytes());
        buf.extend_from_slice(SIGNATURE);
        buf.push(0);
        buf.extend_from_slice(&format_version.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // declared_size, unused

        // make, model, type, year, fuel, capacity, output, drivetrain, memory, manufacturer,
        // ecu_name, hw, sw, production_no, engine_code, transmission
        for s in ["Seat", "Leon", "-", "-", "-", "-", "-", "-", "Eprom", "Bosch", ecu_name, "hw1", "sw1", "-", "-", "-"] {
            buf.extend_from_slice(&enc_str(s));
        }
        buf.extend_from_slice(&0u64.to_le_bytes()); // last_write_time
        for s in ["orig.ols", "OLS 5.0", ""] {
            buf.extend_from_slice(&enc_str(s));
        }
        buf.extend_from_slice(&enc_str("0 (Original)")); // revision_tag

        // Version directory header + M1 anchor: real files carry this near the front of the
        // file (well within the first 4096 bytes `find_version_directory` scans), separate from
        // where the version data itself lives -- which sits at `version_data_start` and must be
        // the LAST thing in the file (that's what the size-consistency check verifies).
        let version_data_start = (buf.len() + 16) as u32; // right after the 16-byte dir header + M1 anchor
        buf.extend_from_slice(&0u32.to_le_bytes()); // num_versions - 1
        buf.extend_from_slice(&version_data_start.to_le_bytes());
        buf.extend_from_slice(&(rom.len() as u32).to_le_bytes());
        buf.extend_from_slice(&M1_MAGIC.to_le_bytes());

        buf.extend_from_slice(&0u32.to_le_bytes()); // the +4 field immediately before version 0's bytes
        buf.extend_from_slice(rom);

        buf
    }

    #[test]
    fn inspects_a_single_version_file() {
        let rom = vec![0xAB; 256];
        let file = single_version_file("MED17.5.20", &rom);
        let info = inspect(&file).expect("should recognise the container");
        assert_eq!(info.ecu_name, "MED17.5.20");
        assert_eq!(info.manufacturer, "Bosch");
        assert_eq!(info.versions.len(), 1);
        assert_eq!(info.versions[0].size, 256);
    }

    #[test]
    fn extracts_the_single_version_bytes_exactly() {
        let rom = (0u8..=255).collect::<Vec<u8>>();
        let file = single_version_file("MED17.5.20", &rom);
        let extracted = extract_version(&file, 0).expect("should extract");
        assert_eq!(extracted, rom);
    }

    #[test]
    fn returns_none_for_a_non_ols_file() {
        let raw_bin = vec![0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE];
        assert!(inspect(&raw_bin).is_none());
    }

    #[test]
    fn out_of_range_version_index_is_an_error() {
        let rom = vec![0x11; 64];
        let file = single_version_file("EDC16U31", &rom);
        assert!(extract_version(&file, 1).is_err());
    }

    #[test]
    fn parses_revision_labels_with_the_tab_convention() {
        let raw = "1 (original,\t Stage1:\t NOCS)";
        let labels = parse_revision_labels(raw, 2).expect("should parse");
        assert_eq!(labels, vec!["original".to_string(), "Stage1 (NOCS)".to_string()]);
    }

    #[test]
    fn parses_revision_labels_with_the_plain_comma_fallback() {
        let raw = "2 (Original, viezu fuelsave vblue 00937.ols, Maxchip pure savings)";
        let labels = parse_revision_labels(raw, 3).expect("should parse via comma fallback");
        assert_eq!(labels[1], "viezu fuelsave vblue 00937.ols");
    }

    #[test]
    fn refuses_to_guess_when_entry_count_does_not_match() {
        assert!(parse_revision_labels("1 (original,\t Stage1:\t NOCS)", 5).is_none());
    }
}
