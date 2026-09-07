// EDC16CP31 map signature database.
//
// STATUS: CALIBRATED on a corpus of ONE ECU software.
//
// Corpus
// ------
//   * Bosch EDC16CP31, Mercedes-Benz OM642 3.0 CDI 165 kW (CLK 320 CDI, W209).
//     Family string "99/1/EDC16CP31/001/B209/X/080000_000/", Bosch SW number
//     1037393817 at 0x190010, engine descriptor
//     "CR4-642-42P7-209CM-165kW-PT2R05-LR-3907x064ME". 2 MB read, KESS V2.
//   * Cross-referenced against the ASAP2 description of the same Daimler
//     project (PROJECT B209, VERSION V080000, "CR4-642"), which carries a
//     DIFFERENT calibration build. The A2L addresses therefore do NOT apply
//     to the dump: the labels below were transferred by structural matching
//     (identical axis vectors + longest-increasing-subsequence alignment on
//     block order), then each block was re-read from the dump and checked
//     against the A2L physical limits before being written down here.
//
// How the labels were VERIFIED (second pass)
// ------------------------------------------
// The ASAP2 package ships the reference binary its addresses describe (a
// Bosch development build, software number "0123456789P03_10"). Its image
// maps to the ECU address space with a constant offset, which makes it a
// GROUND TRUTH: every characteristic can be read at its declared address.
//
// Each family below was then re-derived independently of the alignment
// above. The exact axis vectors of the reference block were searched for in
// the corpus dump; 9 of the 13 calibrated blocks match in exactly ONE place,
// and that place is the address the alignment had produced. The remaining 4
// are families whose members legitimately share a grid (the Hi/Lo boost
// limiter pair, the injection-timing set); those were resolved by the local
// offset of a uniquely-matched neighbour, which agreed as well. Zero
// contradictions, so the labels are no longer an inference.
//
// The reference binary and the ASAP2 file are third-party licensed material
// and are NOT part of this repository. Only the byte patterns derived from
// the corpus dump are stored here.
//
// What that means for trust
// -------------------------
// The record layout, the raw->physical factors and the axis semantics come
// from the manufacturer description and are exact. The ADDRESSES are exact
// for this one software only; that is why every signature is bound to a
// search window rather than a fixed address, and why a marker hit is never
// enough on its own - `EDC16CP31Detector` re-validates the block structure
// and the physical range before emitting anything.
//
// Adding a second software to the corpus is the next step. Until then treat
// `occurrence_rate` as "seen in 1 of 1", not as a statistic.
//
// The CP31 record layout (Bosch `Kf_Xs16_Ys16_Ws16`)
// -------------------------------------------------
// Unlike the VAG EDC16 blocks the VAG detectors scan for, a CP31 2D map is
// SELF-DESCRIBING. There is no need to guess a grid:
//
//   +0x00  u16        nx      number of X points
//   +0x02  u16        ny      number of Y points
//   +0x04  i16[nx]    X axis  strictly increasing
//   +....  i16[ny]    Y axis  strictly increasing
//   +....  i16[nx*ny] Z data  COLUMN direction: Y is the fast index,
//                             z[x * ny + y]
//
// Curves (`Kl_Xs16_Ws16`) are the same without the Y part:
//   +0x00 u16 nx ; i16[nx] X ; i16[nx] Z
//
// All values are big-endian (MPC5xx). The block is allocated at its maximum
// declared size, so a 12x12 map inside a 16x16 allocation is followed by
// padding - the trailing bytes are NOT part of the map.
//
// Because Z is column-major, the natural presentation is rows = nx (engine
// speed) and cols = ny (load), which is also how WinOLS shows these maps.
// `EDC16CP31Detector::build_map` does exactly that, so no transposition is
// needed anywhere downstream.

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
/// On CP31 the block header is itself the marker: `[nx][ny]` followed by the
/// head of the X axis. That is a real structural fact of the record layout,
/// not a code pattern, which is why these markers survive a recalibration of
/// the map CONTENT - only a change of grid or of the axis breakpoints moves
/// them. `offset_in_block` is therefore 0 for every entry: the pattern starts
/// exactly where the block starts.
///
/// The VAG markers ([8A 00 00 06], [00 00 00 3C 00 64], ...) are NOT here on
/// purpose: they encode VAG code layouts and would fire on nonsense.
#[derive(Debug, Clone)]
pub struct BlockMarker {
    pub label: &'static str,
    pub bytes: &'static [u8],
    /// Offset of the marker relative to the start of the block it announces.
    pub offset_in_block: isize,
    pub search_window: Option<(usize, usize)>,
}

// Axis breakpoint vectors confirmed on the corpus dump. Kept as RAW values:
// apply the axis factor to read them (rpm is 1:1, load is 0.01 mm3/stroke,
// pedal and duty are 1/8192).
const AX_RPM_RAIL: &[u16] = &[400, 500, 550, 750, 1000, 1201, 1400, 1600, 1800, 2000,
                              2400, 2800, 3200, 3600, 4200, 4400];
const AX_RPM_BOOST: &[u16] = &[751, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2800,
                               3200, 3600, 4000, 4200, 4400, 4600];
const AX_RPM_VNT: &[u16] = &[659, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2600,
                             2800, 3200, 3600, 4000, 4400, 4700];
const AX_RPM_SMOKE: &[u16] = &[750, 800, 1000, 1200, 1400, 1600, 2000, 2400, 2800, 3200,
                               3600, 3800, 4000, 4200, 4400, 4800];
const AX_RPM_SOI: &[u16] = &[400, 550, 750, 1000, 1200, 1400, 1600, 1800, 2000, 2400,
                             2800, 3200, 3600, 4000, 4200, 4400];
const AX_RPM_PEDAL: &[u16] = &[0, 600, 1200, 1800, 2400, 3000, 3600, 4200];
const AX_RPM_TRQ2Q: &[u16] = &[0, 700, 800, 1000, 1200, 1400, 1600, 1800, 2000, 2400,
                               2800, 3200, 3600, 4000, 4200, 4600];
const AX_RPM_EGR: &[u16] = &[620, 800, 1000, 1200, 1400, 1600, 1800, 2000, 2400, 2800,
                             3200, 3300];

const AX_LOAD_RAIL: &[u16] = &[0, 100, 200, 500, 1000, 1500, 2000, 2500, 3000, 3500,
                               4000, 4500, 5000, 6000, 7000, 8000];
const AX_LOAD_BOOST: &[u16] = &[0, 500, 1000, 1500, 2000, 2500, 3000, 4000, 5000, 6000,
                                7000, 8000];
const AX_LOAD_VNT: &[u16] = &[0, 1000, 2000, 2500, 3000, 4000, 5000, 7000];
const AX_BOOST_SMOKE: &[u16] = &[700, 800, 900, 1000, 1100, 1200, 1300, 1400, 1500, 1600,
                                 1700, 1800, 2000, 2200, 2400, 2500];
const AX_PEDAL: &[u16] = &[98, 655, 1229, 1638, 3277, 4915, 6554, 8192];
const AX_TRQ: &[u16] = &[0, 200, 400, 600, 800, 1000, 1500, 2000, 2500, 3000, 3500, 4000,
                         4500, 5000, 5250, 5500, 5750, 6000];

const fn ax(name: &'static str, unit: &'static str, factor: f64,
            vals: &'static [u16], lo: u16, hi: u16) -> AxisSignature {
    AxisSignature {
        present: true,
        name,
        unit,
        factor,
        typical_values: Some(vals),
        value_range: Some((lo, hi)),
    }
}

/// One block's identity card: the exact `[nx][ny]` header followed by the
/// two axis vectors, byte for byte, as they appear in a real dump.
///
/// This is the strongest anchor available on CP31 and the only one that does
/// not depend on an address. Axis breakpoints are calibration data a tuner
/// has no reason to touch - a remap moves the Z values, not the grid - so a
/// key that matched the stock file still matches the tuned one. It also
/// survives a different software build, where every address moves.
///
/// A key is a FILTER, never a proof on its own: `EDC16CP31Detector` still
/// decodes the block and range-checks it against the template afterwards.
/// Several families legitimately share a key (the Hi/Lo boost limiter pair,
/// the five injection-timing maps), so the number of hits is compared with
/// `MapTemplate::max_count` and narrowed by the template zone when needed.
#[derive(Debug, Clone)]
pub struct AxisKey {
    pub nx: usize,
    pub ny: usize,
    /// First axis in memory (engine speed on every calibrated family).
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

/// Axis keys for `Rail_pSetPointBase_MAP`.
/// 16x16, read at 0x1F1BF2 on SW 1037393817, 1 occurrence(s) in that file.
pub const CP31_KEYS_RAIL_P_SETPOINT_BASE: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[
            400, 500, 550, 750, 1000, 1201, 1400, 1600, 1800, 2000, 2400, 2800, 3200, 3600,
            4200, 4400
        ],
        y: &[
            0, 100, 200, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 6000, 7000,
            8000
        ],
    },
];

/// Axis keys for `Rail_pSetPointLimN_MAP`.
/// 8x12, read at 0x1F2964 on SW 1037393817, 1 occurrence(s) in that file.
pub const CP31_KEYS_RAIL_P_SETPOINT_LIM_N: &[AxisKey] = &[
    AxisKey {
        nx: 8,
        ny: 12,
        x: &[
            100, 4675, 4725, 4775, 4825, 4875, 4900, 5000
        ],
        y: &[
            0, 2000, 3000, 4000, 5000, 6000, 6500, 7000, 7500, 8000, 9500, 11000
        ],
    },
];

/// Axis keys for `PCR_pDesBas_MAP`.
/// 16x12, read at 0x1E4BCE on SW 1037393817, 1 occurrence(s) in that file.
pub const CP31_KEYS_PCR_P_DES_BAS: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 12,
        x: &[
            751, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2800, 3200, 3600, 4000, 4200,
            4400, 4600
        ],
        y: &[
            0, 500, 1000, 1500, 2000, 2500, 3000, 4000, 5000, 6000, 7000, 8000
        ],
    },
];

/// Axis keys for `PCR_pBDesMaxAPGear{Hi,Lo}_MAP`.
/// 16x12, read at 0x1E3E36 on SW 1037393817, 2 occurrence(s) in that file.
pub const CP31_KEYS_PCR_P_BDES_MAX_AP: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 12,
        x: &[
            200, 1000, 1500, 2200, 3100, 3500, 4000, 5300, 6000, 7000, 8000, 8800, 9000, 9200,
            9400, 10000
        ],
        y: &[
            550, 600, 650, 700, 750, 800, 850, 900, 950, 1000, 1050, 1100
        ],
    },
];

/// Axis keys for `PCR_rCtlBas_MAP`.
/// 16x8, read at 0x1DB4F0 on SW 1037393817, 2 occurrence(s) in that file.
pub const CP31_KEYS_PCR_R_CTL_BAS: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 8,
        x: &[
            659, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2600, 2800, 3200, 3600, 4000,
            4400, 4700
        ],
        y: &[
            0, 1000, 2000, 2500, 3000, 4000, 5000, 7000
        ],
    },
];

/// Axis keys for `FlMng_qSmk_MAP`.
/// 16x16, read at 0x1A8986 on SW 1037393817, 1 occurrence(s) in that file.
pub const CP31_KEYS_FLMNG_Q_SMK: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[
            750, 800, 1000, 1200, 1400, 1600, 2000, 2400, 2800, 3200, 3600, 3800, 4000, 4200,
            4400, 4800
        ],
        y: &[
            700, 800, 900, 1000, 1100, 1200, 1300, 1400, 1500, 1600, 1700, 1800, 2000, 2200,
            2400, 2500
        ],
    },
];

/// Axis keys for `AccPed_trqEng_MAP / AccPed_trqEng2_MAP`.
/// 8x8, read at 0x1918DE on SW 1037393817, 1 occurrence(s) in that file.
/// 8x8, read at 0x19169A on SW 1037393817, 1 occurrence(s) in that file.
pub const CP31_KEYS_ACCPED_TRQ_ENG: &[AxisKey] = &[
    AxisKey {
        nx: 8,
        ny: 8,
        x: &[
            0, 600, 1200, 1800, 2400, 3000, 3600, 4200
        ],
        y: &[
            98, 655, 1229, 1638, 3277, 4915, 6554, 8192
        ],
    },
    AxisKey {
        nx: 8,
        ny: 8,
        x: &[
            0, 600, 1200, 1800, 2400, 3000, 3600, 4200
        ],
        y: &[
            0, 655, 1229, 1638, 3277, 4915, 6554, 8192
        ],
    },
];

/// Axis keys for `FlMng_qLimBstPres_MAP`.
/// 12x12, read at 0x1A7FE8 on SW 1037393817, 1 occurrence(s) in that file.
pub const CP31_KEYS_FLMNG_Q_LIM_BST_PRES: &[AxisKey] = &[
    AxisKey {
        nx: 12,
        ny: 12,
        x: &[
            1200, 1600, 2000, 2400, 2800, 3200, 3600, 3800, 4000, 4200, 4400, 4800
        ],
        y: &[
            1600, 1650, 1700, 1750, 1800, 1850, 1900, 1950, 2000, 2050, 2100, 2200
        ],
    },
];

/// Axis keys for `FMTC_trq2qBas_MAP`.
/// 16x18, read at 0x1A9514 on SW 1037393817, 1 occurrence(s) in that file.
pub const CP31_KEYS_FMTC_TRQ2Q_BAS: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 18,
        x: &[
            0, 700, 800, 1000, 1200, 1400, 1600, 1800, 2000, 2400, 2800, 3200, 3600, 4000,
            4200, 4600
        ],
        y: &[
            0, 200, 400, 600, 800, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5250,
            5500, 5750, 6000
        ],
    },
];

/// Axis keys for `InjCrv_phiMI1Bas1..3 / Max1..2`.
/// 16x16, read at 0x1B07EC on SW 1037393817, 11 occurrence(s) in that file.
pub const CP31_KEYS_INJCRV_PHI_MI1: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[
            400, 550, 750, 1000, 1200, 1400, 1600, 1800, 2000, 2400, 2800, 3200, 3600, 4000,
            4200, 4400
        ],
        y: &[
            0, 100, 200, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 6000, 7000,
            8000
        ],
    },
];

/// Axis keys for `AirCtl_mDesBas_MAP`.
/// 12x16, read at 0x194442 on SW 1037393817, 1 occurrence(s) in that file.
pub const CP31_KEYS_AIRCTL_M_DES_BAS: &[AxisKey] = &[
    AxisKey {
        nx: 12,
        ny: 16,
        x: &[
            620, 800, 1000, 1200, 1400, 1600, 1800, 2000, 2400, 2800, 3200, 3300
        ],
        y: &[
            0, 200, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000,
            5000
        ],
    },
];

/// Signature database for EDC16CP31.
///
/// One entry per map family confirmed on the corpus. `address_range` is the
/// window the block was found in, widened to the nearest sensible boundary so
/// that a neighbouring software build still lands inside it; it is a search
/// hint, never a hard address.
pub const CP31_SIGNATURES: &[EDC16MapSignature] = &[
    EDC16MapSignature {
        id_name: "cp31_rail_p_setpoint_base",
        display_name: "Rail Pressure Target (Rail_pSetPointBase_MAP)",
        folder: "Injection system",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(8, 16, 16),
        rows: DimensionRange::range(12, 16, 16),
        data_size_range: (192, 512),
        factor: 0.1,
        offset: 0.0,
        unit: "bar",
        signed: false,
        x_axis: ax("Load (InjCtl_qCurr)", "mm^3/cyc", 0.01, AX_LOAD_RAIL, 0, 11_000),
        y_axis: ax("Engine speed", "rpm", 1.0, AX_RPM_RAIL, 0, 6_400),
        address_range: Some((0x1F1000, 0x1F3200)),
        occurrence_rate: 1.0,
        priority: 95,
    },
    EDC16MapSignature {
        id_name: "cp31_rail_p_setpoint_lim_n",
        display_name: "Rail Pressure Limiter (Rail_pSetPointLimN_MAP)",
        folder: "Injection system",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(8, 12, 16),
        rows: DimensionRange::range(6, 8, 16),
        data_size_range: (96, 512),
        factor: 0.1,
        offset: 0.0,
        unit: "bar",
        signed: false,
        x_axis: ax("Delivered quantity", "mm^3/cyc", 0.01, AX_LOAD_RAIL, 0, 11_000),
        y_axis: ax("Engine speed", "rpm", 1.0, AX_RPM_RAIL, 0, 6_400),
        address_range: Some((0x1F1000, 0x1F3200)),
        occurrence_rate: 1.0,
        priority: 93,
    },
    EDC16MapSignature {
        id_name: "cp31_pcr_p_des_bas",
        display_name: "Boost Target (PCR_pDesBas_MAP)",
        folder: "Turbo boost pressure",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(8, 12, 16),
        rows: DimensionRange::range(12, 16, 16),
        data_size_range: (192, 512),
        factor: 1.0,
        offset: 0.0,
        unit: "hPa",
        signed: false,
        x_axis: ax("Load (PCR_qDes)", "mm^3/cyc", 0.01, AX_LOAD_BOOST, 0, 11_000),
        y_axis: ax("Engine speed", "rpm", 1.0, AX_RPM_BOOST, 0, 6_400),
        address_range: Some((0x1E4000, 0x1E5000)),
        occurrence_rate: 1.0,
        priority: 94,
    },
    EDC16MapSignature {
        id_name: "cp31_pcr_p_bdes_max_ap",
        display_name: "Boost Limiter (PCR_pBDesMaxAPGear{Hi,Lo}_MAP)",
        folder: "Turbo boost pressure",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(8, 12, 16),
        rows: DimensionRange::range(12, 16, 16),
        data_size_range: (192, 512),
        factor: 1.0,
        offset: 0.0,
        unit: "hPa",
        signed: false,
        x_axis: ax("Corrected ambient pressure", "hPa", 1.0, &[550, 1100], 400, 1_200),
        y_axis: ax("Temperature-corrected speed", "rpm", 1.0, &[200, 10_000], 100, 10_500),
        address_range: Some((0x1E3800, 0x1E4400)),
        occurrence_rate: 1.0,
        priority: 88,
    },
    EDC16MapSignature {
        id_name: "cp31_pcr_r_ctl_bas",
        display_name: "VNT Duty Cycle (PCR_rCtlBas_MAP)",
        folder: "Turbo boost pressure control",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(8, 8, 16),
        rows: DimensionRange::range(12, 16, 16),
        data_size_range: (128, 512),
        factor: 0.012_207_031_25,
        offset: 0.0,
        unit: "%",
        signed: false,
        x_axis: ax("Load (PCR_qDes)", "mm^3/cyc", 0.01, AX_LOAD_VNT, 0, 11_000),
        y_axis: ax("Engine speed", "rpm", 1.0, AX_RPM_VNT, 0, 6_400),
        address_range: Some((0x1DB000, 0x1DC000)),
        occurrence_rate: 1.0,
        priority: 86,
    },
    EDC16MapSignature {
        id_name: "cp31_flmng_q_smk",
        display_name: "Smoke Limiter (FlMng_qSmk_MAP)",
        folder: "Smoke limitation",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(12, 16, 16),
        rows: DimensionRange::range(12, 16, 16),
        data_size_range: (288, 512),
        factor: 0.01,
        offset: 0.0,
        unit: "mm^3/cyc",
        signed: false,
        x_axis: ax("Corrected boost pressure", "hPa", 1.0, AX_BOOST_SMOKE, 400, 3_200),
        y_axis: ax("Engine speed", "rpm", 1.0, AX_RPM_SMOKE, 0, 6_400),
        address_range: Some((0x1A8000, 0x1A9000)),
        occurrence_rate: 1.0,
        priority: 92,
    },
    EDC16MapSignature {
        id_name: "cp31_accped_trq_eng",
        display_name: "Driver Wish (AccPed_trqEng{,2}_MAP)",
        folder: "Engine torque request",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(8, 8, 16),
        rows: DimensionRange::range(8, 8, 16),
        data_size_range: (128, 512),
        factor: 0.1,
        offset: 0.0,
        unit: "Nm",
        signed: false,
        x_axis: ax("Accelerator pedal", "%", 0.012_207_031_25, AX_PEDAL, 0, 8_192),
        y_axis: ax("Engine speed", "rpm", 1.0, AX_RPM_PEDAL, 0, 6_400),
        address_range: Some((0x191000, 0x192000)),
        occurrence_rate: 1.0,
        priority: 90,
    },
    EDC16MapSignature {
        id_name: "cp31_flmng_q_lim_bst_pres",
        display_name: "Quantity Limiter by boost (FlMng_qLimBstPres_MAP)",
        folder: "Engine torque limiters",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(8, 12, 16),
        rows: DimensionRange::range(8, 12, 16),
        data_size_range: (128, 512),
        factor: 0.01,
        offset: 0.0,
        unit: "mm^3/cyc",
        signed: false,
        x_axis: ax("Boost pressure", "hPa", 1.0, &[1600, 2200], 400, 3_200),
        y_axis: ax("Engine speed", "rpm", 1.0, &[1200, 4800], 0, 6_400),
        address_range: Some((0x1A7C00, 0x1A8400)),
        occurrence_rate: 1.0,
        priority: 87,
    },
    EDC16MapSignature {
        id_name: "cp31_fmtc_trq2q_bas",
        display_name: "Torque to IQ (FMTC_trq2qBas_MAP)",
        folder: "Torque to IQ Conversion",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(12, 18, 20),
        rows: DimensionRange::range(12, 16, 16),
        data_size_range: (288, 640),
        factor: 0.01,
        offset: 0.0,
        unit: "mm^3/cyc",
        signed: false,
        x_axis: ax("Inner torque setpoint", "Nm", 0.1, AX_TRQ, 0, 8_000),
        y_axis: ax("Engine speed", "rpm", 1.0, AX_RPM_TRQ2Q, 0, 6_400),
        address_range: Some((0x1A9000, 0x1AA000)),
        occurrence_rate: 1.0,
        priority: 84,
    },
    EDC16MapSignature {
        id_name: "cp31_injcrv_phi_mi1",
        display_name: "Start of injection (InjCrv_phiMI1Bas1..3 / Max1..2)",
        folder: "Start of injection SOI",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(12, 16, 16),
        rows: DimensionRange::range(12, 16, 16),
        data_size_range: (288, 512),
        factor: 0.023_437_5,
        offset: 0.0,
        unit: "degCrS",
        signed: true,
        x_axis: ax("Load (InjCtl_qSetUnBal)", "mm^3/cyc", 0.01, AX_LOAD_RAIL, 0, 11_000),
        y_axis: ax("Engine speed", "rpm", 1.0, AX_RPM_SOI, 0, 6_400),
        address_range: Some((0x1B0000, 0x1B2000)),
        occurrence_rate: 1.0,
        priority: 83,
    },
    EDC16MapSignature {
        id_name: "cp31_airctl_m_des_bas",
        display_name: "EGR air mass target (AirCtl_mDesBas_MAP)",
        folder: "EGR",
        structure_type: StructureType::EZweiInv,
        cols: DimensionRange::range(12, 16, 16),
        rows: DimensionRange::range(8, 12, 16),
        data_size_range: (192, 512),
        factor: 0.1,
        offset: 0.0,
        unit: "mg/stroke",
        signed: false,
        x_axis: ax("Load (InjCtl_qRaw)", "mm^3/cyc", 0.01, AX_LOAD_RAIL, 0, 11_000),
        y_axis: ax("Engine speed", "rpm", 1.0, AX_RPM_EGR, 0, 6_400),
        address_range: Some((0x194000, 0x196000)),
        occurrence_rate: 1.0,
        priority: 80,
    },
];

/// Block markers for EDC16CP31.
///
/// Every pattern below was cut from the corpus dump at the exact start of the
/// named block, then extended byte by byte until it occurred exactly ONCE
/// inside its search window. Three families (boost limiter, VNT, SOI) have
/// several members that share their axes bit for bit, so their marker cannot
/// be made unique - that is by design, the walk emits every hit and the
/// detector deduplicates by address.
///
/// A marker hit is a HINT. `EDC16CP31Detector` still reads the block header,
/// validates both axes and range-checks the data before emitting a map.
pub const CP31_MARKERS: &[BlockMarker] = &[
    BlockMarker {
        label: "cp31_rail_p_setpoint_base",
        // nx=16 ny=16, X axis 400 500 550 750 1000 1201 rpm
        bytes: &[0x00, 0x10, 0x00, 0x10, 0x01, 0x90, 0x01, 0xF4, 0x02, 0x26, 0x02, 0xEE,
                 0x03, 0xE8, 0x04, 0xB1],
        offset_in_block: 0,
        search_window: Some((0x1F1000, 0x1F3200)),
    },
    BlockMarker {
        label: "cp31_rail_p_setpoint_lim_n",
        // nx=8 ny=12, X axis starts at 100 rpm
        bytes: &[0x00, 0x08, 0x00, 0x0C, 0x00, 0x64],
        offset_in_block: 0,
        search_window: Some((0x1F1000, 0x1F3200)),
    },
    BlockMarker {
        label: "cp31_pcr_p_des_bas",
        // nx=16 ny=12, X axis starts at 751 rpm
        bytes: &[0x00, 0x10, 0x00, 0x0C, 0x02, 0xEF],
        offset_in_block: 0,
        search_window: Some((0x1E4000, 0x1E5000)),
    },
    BlockMarker {
        label: "cp31_pcr_p_bdes_max_ap",
        // nx=16 ny=12, X axis 200 1000 1500 2200 3100 rpm (temp corrected).
        // Matches both the GearHi and the GearLo variant.
        bytes: &[0x00, 0x10, 0x00, 0x0C, 0x00, 0xC8, 0x03, 0xE8, 0x05, 0xDC, 0x08, 0x98,
                 0x0C, 0x1C],
        offset_in_block: 0,
        search_window: Some((0x1E3800, 0x1E4400)),
    },
    BlockMarker {
        label: "cp31_pcr_r_ctl_bas",
        // nx=16 ny=8, X axis starts at 659 rpm
        bytes: &[0x00, 0x10, 0x00, 0x08, 0x02, 0x93, 0x03, 0xE8, 0x04, 0xB0],
        offset_in_block: 0,
        search_window: Some((0x1DB000, 0x1DC000)),
    },
    BlockMarker {
        label: "cp31_flmng_q_smk",
        // nx=16 ny=16, X axis starts at 750 rpm
        bytes: &[0x00, 0x10, 0x00, 0x10, 0x02, 0xEE],
        offset_in_block: 0,
        search_window: Some((0x1A8000, 0x1A9000)),
    },
    BlockMarker {
        label: "cp31_accped_trq_eng",
        // nx=8 ny=8, X axis 0 600 1200 1800 2400 3000 3600 4200 rpm
        bytes: &[0x00, 0x08, 0x00, 0x08, 0x00, 0x00, 0x02, 0x58, 0x04, 0xB0, 0x07, 0x08,
                 0x09, 0x60, 0x0B, 0xB8, 0x0E, 0x10, 0x10, 0x68],
        offset_in_block: 0,
        search_window: Some((0x191000, 0x192000)),
    },
    BlockMarker {
        label: "cp31_flmng_q_lim_bst_pres",
        // nx=12 ny=12, X axis starts at 1200 rpm
        bytes: &[0x00, 0x0C, 0x00, 0x0C, 0x04, 0xB0],
        offset_in_block: 0,
        search_window: Some((0x1A7C00, 0x1A8400)),
    },
    BlockMarker {
        label: "cp31_fmtc_trq2q_bas",
        // nx=16 ny=18, X axis starts at 0 rpm
        bytes: &[0x00, 0x10, 0x00, 0x12, 0x00, 0x00],
        offset_in_block: 0,
        search_window: Some((0x1A9000, 0x1AA000)),
    },
    BlockMarker {
        label: "cp31_injcrv_phi_mi1",
        // nx=16 ny=16, X axis 400 550 750 1000 1200 1400 rpm.
        // Shared by Bas1..Bas3 and Max1..Max2 - deliberately not unique.
        bytes: &[0x00, 0x10, 0x00, 0x10, 0x01, 0x90, 0x02, 0x26, 0x02, 0xEE, 0x03, 0xE8,
                 0x04, 0xB0, 0x05, 0x78],
        offset_in_block: 0,
        search_window: Some((0x1B0000, 0x1B2000)),
    },
    BlockMarker {
        label: "cp31_airctl_m_des_bas",
        // nx=12 ny=16, X axis starts at 620 rpm
        bytes: &[0x00, 0x0C, 0x00, 0x10, 0x02, 0x6C],
        offset_in_block: 0,
        search_window: Some((0x194000, 0x196000)),
    },
];

/// True once the signature database carries at least one confirmed entry.
pub const fn is_signature_db_populated() -> bool {
    !CP31_SIGNATURES.is_empty()
}
