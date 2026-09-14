// GENERATED reference data, then hand-maintained -- see signatures.rs's module doc
// for the corpus and method. Included via `include!` so the axis-key arrays don't
// have to be retyped by hand. Axis meanings and Z sign/range were corrected
// against an independent reference database and real cell values after the
// first generation; see each template's fields and mod.rs's tests.

/// Axis key for `AccPed_trqENU` (Driver Wish (Neutral)).
/// 16x16, address 0x1C1CFC, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_ACCPED_TRQENU: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 4750, 5000, 5200, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 4750, 4900, 5000, 5500, 6100, 6200],
        y: &[0, 82, 123, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 3750, 4000, 4500, 4750, 5000, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[82, 164, 573, 1229, 2376, 3195, 4014, 4833, 5571, 6308, 7045, 7782, 8192, 8233, 8274, 8356],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 850, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[0, 82, 328, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51809513.0281013675.383008.v0.bin)
    },
];

/// Axis key for `AirCtl_mDesBas` (Air Mass Setpoint).
/// 14x16, address 0x1C352C, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_AIRCTL_MDESBAS: &[AxisKey] = &[
    AxisKey {
        nx: 14,
        ny: 16,
        x: &[750, 840, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750],
        y: &[400, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500, 5000],
    },
    AxisKey {
        nx: 14,
        ny: 16,
        x: &[750, 900, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4250],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 14,
        ny: 16,
        x: &[700, 800, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500, 5000],
        // real, confirmed: CROMA (Fiat.Croma.Bosch.EDC16C39.51806274.391910.v0.bin)
    },
    AxisKey {
        nx: 14,
        ny: 16,
        x: &[650, 740, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4250],
        y: &[400, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500, 5000],
        // real, confirmed: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 14,
        ny: 16,
        x: &[700, 800, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750],
        y: &[400, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500, 5000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `AirCtl_rEGRBas` (EGR Duty Cycle Base).
/// 8x16, address 0x1C4384, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_AIRCTL_REGRBAS: &[AxisKey] = &[
    AxisKey {
        nx: 8,
        ny: 16,
        x: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500],
        y: &[0, 1229, 1638, 2048, 2458, 2867, 3277, 3686, 4096, 4915, 5325, 5734, 6144, 6963, 7782, 8192],
        // also byte-identical on: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
];

/// Axis key for `LmbdSmkHigh` (Smoke Limiter (Lambda High)).
/// 16x16, address 0x1CD0D0, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_LMBDSMKHIGH: &[AxisKey] = &[
    // NOTE: the original un-annotated key here (x starting at 200 RPM,
    // otherwise identical to the entry below) was REMOVED after the
    // expanded corpus proved it unsafe: on Croma, Bravo and Grande Punto
    // real dumps that exact grid also occurs, byte-for-byte, ~0x48C bytes
    // after this family's own confirmed address -- a real but DIFFERENT
    // table, not LmbdSmkHigh, that this key was mislabelling as a second
    // "Smoke Limiter (Lambda High)" hit. Since phase 0 (address window)
    // already finds this family's real block directly on every corpus file
    // that key was redundant for its own origin (Alfa 159/147) and actively
    // wrong elsewhere, so it is dropped rather than special-cased further.
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[600, 800, 1000, 1125, 1250, 1375, 1500, 1675, 1750, 1875, 2000, 2250, 2500, 2750, 3000, 3750],
        y: &[3000, 3500, 4000, 4500, 5000, 5600, 6200, 6600, 7000, 7400, 8000, 8400, 9000, 9550, 10000, 10500],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[600, 800, 1000, 1125, 1250, 1375, 1500, 1675, 1750, 1875, 2000, 2250, 2500, 2750, 3000, 3750],
        y: &[3000, 3500, 4000, 4500, 5000, 5600, 6200, 6600, 7000, 7400, 8000, 8400, 9000, 9500, 10000, 10500],
        // real, confirmed: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[700, 900, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4500],
        y: &[3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500, 8000, 8500, 9000, 9500, 10000, 10500],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[500, 700, 800, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4250, 4500],
        y: &[3500, 4200, 4500, 5000, 5500, 6000, 6500, 7000, 7500, 8000, 8500, 9000, 9250, 9500, 9750, 10000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51809513.0281013675.383008.v0.bin), and
        // matches the address-window (phase 0) hit directly on Croma, Bravo and Grande Punto too.
    },
];

/// Axis key for `LmbdSmkLow` (Smoke Limiter (Lambda Low)).
/// 16x16, address 0x1CD314, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_LMBDSMKLOW: &[AxisKey] = &[
    // NOTE: see the matching comment on C39_KEYS_LMBDSMKHIGH -- the
    // original un-annotated key (x starting at 200 RPM) was removed for the
    // same reason: it is a real but different table's grid, not this one's.
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[600, 800, 1000, 1125, 1250, 1375, 1500, 1675, 1750, 1875, 2000, 2250, 2500, 2750, 3000, 3750],
        y: &[3000, 3500, 4000, 4500, 5000, 5600, 6200, 6600, 7000, 7400, 8000, 8400, 9000, 9550, 10000, 10500],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[600, 800, 1000, 1125, 1250, 1375, 1500, 1675, 1750, 1875, 2000, 2250, 2500, 2750, 3000, 3750],
        y: &[3000, 3500, 4000, 4500, 5000, 5600, 6200, 6600, 7000, 7400, 8000, 8400, 9000, 9500, 10000, 10500],
        // real, confirmed: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[700, 900, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4500],
        y: &[3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500, 8000, 8500, 9000, 9500, 10000, 10500],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[500, 700, 800, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4250, 4500],
        y: &[3500, 4200, 4500, 5000, 5500, 6000, 6500, 7000, 7500, 8000, 8500, 9000, 9250, 9500, 9750, 10000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51809513.0281013675.383008.v0.bin)
    },
];

/// Axis key for `Trq2qBas` (Torque to Quantity Conversion).
/// 16x16, address 0x1CDBB2, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_TRQ2QBAS: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500, 5000],
        y: &[0, 50, 250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2500, 3000, 3500, 4000, 4500, 5000],
        // also byte-identical on: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500, 5000],
        y: &[0, 50, 250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `InjVCD_tiET` (Injector Valve Command Duration).
/// 20x20, address 0x1DC08A, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_INJVCD_TIET: &[AxisKey] = &[
    AxisKey {
        nx: 20,
        ny: 20,
        x: &[500, 1200, 1210, 2000, 2500, 3000, 4000, 6000, 8000, 10000, 12000, 13000, 14000, 15000, 16000, 18000, 18010, 18020, 18030, 18040],
        y: &[0, 20, 100, 150, 200, 250, 300, 350, 400, 500, 600, 700, 800, 1000, 1500, 2000, 3000, 4000, 6000, 8000],
        // also byte-identical on: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 20,
        ny: 20,
        x: &[500, 1200, 1210, 2000, 2500, 3000, 4000, 6000, 8000, 10000, 12000, 13000, 14000, 15000, 16000, 18000, 18010, 18020, 18030, 18040],
        y: &[0, 20, 100, 150, 200, 250, 300, 350, 400, 500, 600, 700, 800, 1000, 1500, 2000, 3000, 4000, 7000, 10000],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 20,
        ny: 20,
        x: &[500, 600, 1000, 2000, 2500, 3000, 4000, 6000, 8000, 10000, 12000, 13000, 13500, 14000, 14500, 15000, 16000, 17000, 18000, 20000],
        y: &[0, 20, 100, 150, 200, 250, 300, 350, 400, 500, 600, 700, 800, 1000, 1500, 2000, 3000, 4000, 5000, 7000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `PCR_CtlBas` (Turbo Duty Cycle Base).
/// 16x16, address 0x1DF5FE, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_PCR_CTLBAS: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 5000],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 4000, 5000, 6000, 6500, 7000],
        // also byte-identical on: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 5000],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 4000, 5000, 6000, 6500, 7000, 7500],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 5000],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 5000],
        y: &[400, 600, 1200, 1600, 2000, 2400, 2800, 3200, 3600, 4000, 4400, 4800, 5200, 5600, 6000, 6400],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51809513.0281013675.383008.v0.bin)
    },
];

/// Axis key for `PCR_DesBas` (Boost Pressure Target).
/// 16x16, address 0x1E132C, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_PCR_DESBAS: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4750, 5000],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 4000, 5000, 6000, 6500, 7000],
        // also byte-identical on: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4750, 4900],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 4000, 5000, 6000, 6500, 7000, 7500],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4750, 5000],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4750, 5000],
        y: &[400, 800, 1200, 1600, 2000, 2400, 2800, 3200, 3600, 4000, 4400, 4800, 5200, 5600, 6000, 6400],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51809513.0281013675.383008.v0.bin)
    },
];

/// Axis key for `PCR_DesMaxAP` (Boost Pressure Limiter).
/// 16x16, address 0x1E15B2, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_PCR_DESMAXAP: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500],
        y: &[630, 680, 730, 780, 830, 880, 930, 980, 1030, 1080, 1081, 1082, 1083, 1084, 1085, 1086],
        // also byte-identical on: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
];

/// Axis key for `Rail_PointBase` (Rail Pressure Target).
/// 16x16, address 0x1E67E8, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_RAIL_POINTBASE: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 700, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500, 5000, 6000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `Rail_PointLimTem` (Rail Pressure Limiter (Thermal)).
/// 8x8, address 0x1E6BA2, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_RAIL_POINTLIMTEM: &[AxisKey] = &[
    AxisKey {
        nx: 8,
        ny: 8,
        x: &[1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500],
        y: &[3031, 3131, 3231, 3331, 3431, 3531, 3631, 3731],
        // also byte-identical on: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 8,
        ny: 8,
        x: &[1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000],
        y: &[3031, 3131, 3231, 3331, 3431, 3531, 3631, 3731],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
    AxisKey {
        nx: 8,
        ny: 8,
        x: &[1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500],
        y: &[3131, 3231, 3331, 3431, 3531, 3631, 3731, 3831],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51809513.0281013675.383008.v0.bin)
    },
];

/// Axis key for `Rail_PointMax` (Rail Pressure Maximum).
/// 12x8, address 0x1E6C48, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_RAIL_POINTMAX: &[AxisKey] = &[
    AxisKey {
        nx: 12,
        ny: 8,
        x: &[250, 750, 1000, 1250, 1500, 2000, 2500, 2750, 3000, 3250, 3500, 5000],
        y: &[500, 1000, 2000, 3000, 4000, 5000, 6000, 7000],
        // also byte-identical on: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 12,
        ny: 8,
        x: &[250, 750, 1000, 1250, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5500],
        y: &[500, 1000, 2000, 3000, 4000, 5000, 6000, 7000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `InjCrv_Bas1` (Injection Timing 1).
/// 16x16, address 0x1D05DC, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_INJCRV_BAS1: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 700, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `InjCrv_Bas2` (Injection Timing 2).
/// 16x16, address 0x1D0820, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_INJCRV_BAS2: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 700, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `InjCrv_Bas3` (Injection Timing 3).
/// 16x16, address 0x1D0A64, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_INJCRV_BAS3: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 700, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `InjCrv_Bas4` (Injection Timing 4).
/// 16x16, address 0x1D0CA8, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_INJCRV_BAS4: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 700, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `InjCrv_Bas5` (Injection Timing 5).
/// 16x16, address 0x1D0EEC, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_INJCRV_BAS5: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 700, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 3000, 3500, 3750, 4000, 4250, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[250, 500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500],
        y: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 5000, 6000, 7000],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
];

/// Axis key for `TrqEngDriveAway` (Drive-Away Torque).
/// 16x16, address 0x1C1AB8, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_TRQENGDRIVEAWAY: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 4750, 5000, 5200, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8233, 8274, 8356],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 4750, 5000, 5200, 6000, 6100, 6200],
        y: &[0, 82, 123, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8233, 8274, 8356],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: CROMA (Fiat.Croma.Bosch.EDC16C39.51806274.391910.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8233, 8274, 8356],
        // real, confirmed: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 500, 750, 1000, 1400, 1800, 2200, 2600, 3000, 3400, 3800, 4200, 4600, 5000, 5500, 5700],
        y: &[0, 82, 246, 819, 1065, 1229, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8274],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 850, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[82, 164, 573, 1229, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8233, 8274, 8356],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51809513.0281013675.383008.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 850, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[0, 41, 328, 737, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8233, 8274, 8356],
        // real, confirmed: GRANDE.PUNTO (Fiat.Grande.Punto.Bosch.EDC16C39.51795844.0281012899.378691.v0.bin)
    },
];

/// Axis key for `AccPed_trqEngA` (Driver Wish A).
/// 16x16, address 0x1C1630, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_ACCPED_TRQENGA: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 500, 1000, 1500, 2000, 2500, 3500, 4000, 4500, 4750, 5000, 5200, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 4750, 4900, 5000, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 3750, 4000, 4500, 4750, 5000, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[82, 164, 573, 1229, 2376, 3195, 4014, 4833, 5571, 6308, 7045, 7782, 8192, 8233, 8274, 8356],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 850, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[0, 82, 328, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51809513.0281013675.383008.v0.bin)
    },
];

/// Axis key for `AccPed_trqEngB` (Driver Wish B).
/// 16x16, address 0x1C1874, confirmed byte-identical on 3 independent real dumps
/// (Alfa 159 x2 builds, Alfa 147).
pub const C39_KEYS_ACCPED_TRQENGB: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 4750, 5000, 5200, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 4750, 4900, 5000, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: BRERA (Alfa.Brera.Bosch.EDC16C39.51820069.0281014459.390927.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 3750, 4000, 4500, 4750, 5000, 6000, 6100, 6200],
        y: &[0, 82, 410, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: CROMA (Fiat.Croma.Bosch.EDC16C39.51820797.394821.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[82, 164, 573, 1229, 2376, 3195, 4014, 4833, 5571, 6308, 7045, 7782, 8192, 8233, 8274, 8356],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 400, 850, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5200, 5900, 6000, 6100, 6200],
        y: &[0, 82, 328, 819, 1638, 2458, 3277, 4096, 4915, 5734, 6554, 7373, 8192, 8198, 8199, 8200],
        // real, confirmed: DOBLO (Fiat.Doblo.Bosch.EDC16C39.51809513.0281013675.383008.v0.bin)
    },
];

/// Map families confirmed on the Alfa 159/147-class EDC16C39 layout (2MB dump).
///
/// address/nx/ny/axis-key confirmed byte-identical across 3 real dumps (see this file's
/// module doc). z_range_stock/tuned are the observed span of REAL Z data across those same
/// 3 files (correctly signed/unsigned per the ECM-derived reference's declared physical
/// range), widened -- not yet the "20 files" bar CONTRIBUTING.md ultimately asks for.
pub const MAP_TEMPLATES: &[MapTemplate] = &[
    MapTemplate {
        name: "Driver Wish (Neutral)",
        bosch_label: "AccPed_trqENU",
        category: MapCategory::EngineTorqueRequest,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (-139.20, 603.20),
        z_range_tuned: (-464.00, 1392.00),
        unit: "Nm",
        address: 0x1C1CFC,
        signed: true,
        axis_keys: C39_KEYS_ACCPED_TRQENU,
        calibrated: true,
    },
    MapTemplate {
        name: "Air Mass Setpoint",
        bosch_label: "AirCtl_mDesBas",
        category: MapCategory::AirControl,
        nx: 14,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (-48.00, 1488.00),
        z_range_tuned: (-720.00, 3120.00),
        unit: "mg/Hub",
        address: 0x1C352C,
        signed: true,
        axis_keys: C39_KEYS_AIRCTL_MDESBAS,
        calibrated: true,
    },
    // Named without the token "EGR" on purpose: the frontend treats any map
    // whose name contains "egr" as the VAG 13x16 EGR layout and re-transposes
    // it in five places (map-cell-layout.ts, map-viewer.tsx, editor/page.tsx),
    // which for this genuinely 8-row x 16-column block put every cell under
    // the wrong rpm/% pair and wrote edits to a different operating point
    // than the one shown. The Bosch label stays in the description.
    MapTemplate {
        name: "Exhaust Gas Recirculation Duty Cycle",
        bosch_label: "AirCtl_rEGRBas",
        category: MapCategory::Egr,
        nx: 8,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.0122070312,
        z_offset: 0.0,
        z_range_stock: (-30.00, 130.00),
        z_range_tuned: (-100.00, 300.00),
        unit: "%",
        address: 0x1C4384,
        signed: true,
        axis_keys: C39_KEYS_AIRCTL_REGRBAS,
        calibrated: true,
    },
    MapTemplate {
        name: "Smoke Limiter (Lambda High)",
        bosch_label: "LmbdSmkHigh",
        category: MapCategory::SmokeLimitation,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::AirMass),
        z_factor: 0.0010000000,
        z_offset: 0.0,
        z_range_stock: (0.60, 1.68),
        z_range_tuned: (0.12, 2.84),
        unit: "lambda",
        address: 0x1CD0D0,
        signed: false,
        axis_keys: C39_KEYS_LMBDSMKHIGH,
        calibrated: true,
    },
    MapTemplate {
        name: "Smoke Limiter (Lambda Low)",
        bosch_label: "LmbdSmkLow",
        category: MapCategory::SmokeLimitation,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::AirMass),
        z_factor: 0.0010000000,
        z_offset: 0.0,
        z_range_stock: (0.60, 1.68),
        z_range_tuned: (0.12, 2.84),
        unit: "lambda",
        address: 0x1CD314,
        signed: false,
        axis_keys: C39_KEYS_LMBDSMKLOW,
        calibrated: true,
    },
    MapTemplate {
        name: "Torque to Quantity Conversion",
        bosch_label: "Trq2qBas",
        category: MapCategory::TorqueToIqConversion,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Torque),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (-30.15, 130.65),
        z_range_tuned: (-100.50, 301.50),
        unit: "mm3/cyc",
        address: 0x1CDBB2,
        signed: true,
        axis_keys: C39_KEYS_TRQ2QBAS,
        calibrated: true,
    },
    MapTemplate {
        name: "Injector Valve Command Duration",
        bosch_label: "InjVCD_tiET",
        category: MapCategory::InjectionSystem,
        nx: 20,
        ny: 20,
        axes: (AxisType::RailPressure, AxisType::InjectionQty),
        z_factor: 2.0000000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 7800.00),
        z_range_tuned: (0.00, 18000.00),
        unit: "us",
        address: 0x1DC08A,
        signed: false,
        axis_keys: C39_KEYS_INJVCD_TIET,
        calibrated: true,
    },
    MapTemplate {
        name: "Turbo Duty Cycle Base",
        bosch_label: "PCR_CtlBas",
        category: MapCategory::TurboBoostPressureControl,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0122070312,
        z_offset: 0.0,
        z_range_stock: (23.00, 87.00),
        z_range_tuned: (-5.00, 155.00),
        unit: "%",
        address: 0x1DF5FE,
        signed: true,
        axis_keys: C39_KEYS_PCR_CTLBAS,
        calibrated: true,
    },
    MapTemplate {
        name: "Boost Pressure Target",
        bosch_label: "PCR_DesBas",
        category: MapCategory::TurboBoostPressure,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 2.0000000000,
        z_offset: 0.0,
        z_range_stock: (1096.00, 5704.00),
        z_range_tuned: (0.00, 10600.00),
        unit: "hPa",
        address: 0x1E132C,
        signed: false,
        axis_keys: C39_KEYS_PCR_DESBAS,
        calibrated: true,
    },
    MapTemplate {
        name: "Boost Pressure Limiter",
        bosch_label: "PCR_DesMaxAP",
        category: MapCategory::TurboBoostPressure,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::BoostPressure),
        z_factor: 2.0000000000,
        z_offset: 0.0,
        z_range_stock: (3500.00, 6500.00),
        z_range_tuned: (0.00, 15000.00),
        unit: "hPa",
        address: 0x1E15B2,
        signed: false,
        axis_keys: C39_KEYS_PCR_DESMAXAP,
        calibrated: true,
    },
    MapTemplate {
        name: "Rail Pressure Target",
        bosch_label: "Rail_PointBase",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 1999.00),
        z_range_tuned: (0.00, 4260.00),
        unit: "bar",
        address: 0x1E67E8,
        signed: false,
        axis_keys: C39_KEYS_RAIL_POINTBASE,
        calibrated: true,
    },
    MapTemplate {
        name: "Rail Pressure Limiter (Thermal)",
        bosch_label: "Rail_PointLimTem",
        category: MapCategory::InjectionSystem,
        nx: 8,
        ny: 8,
        axes: (AxisType::Rpm, AxisType::Temperature),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (690.00, 1810.00),
        z_range_tuned: (200.00, 3000.00),
        unit: "bar",
        address: 0x1E6BA2,
        signed: false,
        axis_keys: C39_KEYS_RAIL_POINTLIMTEM,
        calibrated: true,
    },
    MapTemplate {
        name: "Rail Pressure Maximum",
        bosch_label: "Rail_PointMax",
        category: MapCategory::InjectionSystem,
        nx: 12,
        ny: 8,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (445.60, 1866.40),
        z_range_tuned: (0.00, 3376.00),
        unit: "bar",
        address: 0x1E6C48,
        signed: false,
        axis_keys: C39_KEYS_RAIL_POINTMAX,
        calibrated: true,
    },
    MapTemplate {
        name: "Injection Timing 1",
        bosch_label: "InjCrv_Bas1",
        category: MapCategory::StartOfInjection,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-10.50, 37.50),
        z_range_tuned: (-31.50, 88.49),
        unit: "deg",
        address: 0x1D05DC,
        signed: true,
        axis_keys: C39_KEYS_INJCRV_BAS1,
        calibrated: true,
    },
    MapTemplate {
        name: "Injection Timing 2",
        bosch_label: "InjCrv_Bas2",
        category: MapCategory::StartOfInjection,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-10.50, 37.50),
        z_range_tuned: (-31.50, 88.49),
        unit: "deg",
        address: 0x1D0820,
        signed: true,
        axis_keys: C39_KEYS_INJCRV_BAS2,
        calibrated: true,
    },
    MapTemplate {
        name: "Injection Timing 3",
        bosch_label: "InjCrv_Bas3",
        category: MapCategory::StartOfInjection,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-10.50, 37.50),
        z_range_tuned: (-31.50, 88.49),
        unit: "deg",
        address: 0x1D0A64,
        signed: true,
        axis_keys: C39_KEYS_INJCRV_BAS3,
        calibrated: true,
    },
    MapTemplate {
        name: "Injection Timing 4",
        bosch_label: "InjCrv_Bas4",
        category: MapCategory::StartOfInjection,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-10.50, 37.50),
        z_range_tuned: (-31.50, 88.49),
        unit: "deg",
        address: 0x1D0CA8,
        signed: true,
        axis_keys: C39_KEYS_INJCRV_BAS4,
        calibrated: true,
    },
    MapTemplate {
        name: "Injection Timing 5",
        bosch_label: "InjCrv_Bas5",
        category: MapCategory::StartOfInjection,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-10.50, 37.50),
        z_range_tuned: (-31.50, 88.49),
        unit: "deg",
        address: 0x1D0EEC,
        signed: true,
        axis_keys: C39_KEYS_INJCRV_BAS5,
        calibrated: true,
    },
    MapTemplate {
        name: "Drive-Away Torque",
        bosch_label: "TrqEngDriveAway",
        category: MapCategory::EngineTorqueRequest,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (-139.20, 603.20),
        z_range_tuned: (-464.00, 1392.00),
        unit: "Nm",
        address: 0x1C1AB8,
        signed: true,
        axis_keys: C39_KEYS_TRQENGDRIVEAWAY,
        calibrated: true,
    },
    MapTemplate {
        name: "Driver Wish A",
        bosch_label: "AccPed_trqEngA",
        category: MapCategory::EngineTorqueRequest,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (-139.20, 603.20),
        z_range_tuned: (-464.00, 1392.00),
        unit: "Nm",
        address: 0x1C1630,
        signed: true,
        axis_keys: C39_KEYS_ACCPED_TRQENGA,
        calibrated: true,
    },
    MapTemplate {
        name: "Driver Wish B",
        bosch_label: "AccPed_trqEngB",
        category: MapCategory::EngineTorqueRequest,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (-139.20, 603.20),
        z_range_tuned: (-464.00, 1392.00),
        unit: "Nm",
        address: 0x1C1874,
        signed: true,
        axis_keys: C39_KEYS_ACCPED_TRQENGB,
        calibrated: true,
    },
];
