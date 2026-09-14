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
    AxisKey {
        nx: 13,
        ny: 10,
        x: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2500, 3000, 3500, 4000, 5000, 5300],
        y: &[0, 82, 287, 492, 1311, 2212, 3277, 5243, 6554, 8192],
        // real, confirmed: Fiat.Croma.Bosch.EDC16C39.51806274.391910.v0.bin
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
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[600, 800, 1000, 1125, 1250, 1375, 1500, 1675, 1750, 1875, 2000, 2250, 2500, 2750, 3000, 3750],
        y: &[3000, 3500, 4000, 4500, 5000, 5600, 6200, 6600, 7000, 7400, 7750, 8400, 9000, 9550, 10000, 10500],
        // real, confirmed: Alfa147_Ducati_170hp_0281015228_395299.bin, read at the slot the reference
        // database declares for this exact build (0x1CD438 High, 0x1CD67C Low)
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
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[600, 800, 1000, 1125, 1250, 1375, 1500, 1675, 1750, 1875, 2000, 2250, 2500, 2750, 3000, 3750],
        y: &[3000, 3500, 4000, 4500, 5000, 5600, 6200, 6600, 7000, 7400, 7750, 8400, 9000, 9550, 10000, 10500],
        // real, confirmed: Alfa147_Ducati_170hp_0281015228_395299.bin, read at the slot the reference
        // database declares for this exact build (0x1CD438 High, 0x1CD67C Low)
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
    AxisKey {
        nx: 13,
        ny: 10,
        x: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2500, 3000, 3500, 4000, 5000, 5300],
        y: &[0, 82, 287, 492, 1311, 2212, 3277, 5243, 6554, 8192],
        // real, confirmed: Fiat.Croma.Bosch.EDC16C39.51806274.391910.v0.bin
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
    AxisKey {
        nx: 13,
        ny: 10,
        x: &[500, 750, 1000, 1250, 1500, 1750, 2000, 2500, 3000, 3500, 4000, 5000, 5300],
        y: &[0, 82, 287, 492, 1311, 2212, 3277, 5243, 6554, 8192],
        // real, confirmed: Fiat.Croma.Bosch.EDC16C39.51806274.391910.v0.bin
    },
];

/// Map families confirmed on the Alfa 159/147-class EDC16C39 layout (2MB dump).
///
/// address/nx/ny/axis-key confirmed byte-identical across 3 real dumps (see this file's
/// module doc). z_range_stock/tuned are the observed span of REAL Z data across those same
/// 3 files (correctly signed/unsigned per the ECM-derived reference's declared physical
/// range), widened -- not yet the "20 files" bar CONTRIBUTING.md ultimately asks for.
/// Axis keys for `EngPrt_trqLim` (Torque Limiter). 25-point curve, corpus address 0x1CBCF6;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_ENGPRT_TRQLIM: &[AxisKey] = &[
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[0, 400, 410, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4750, 5000, 5250, 5500, 5750, 6000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+6 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[0, 400, 410, 990, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4750, 5000, 5250, 5500, 5750, 6000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.386726.v0.bin (+7 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[0, 400, 410, 990, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4750, 4900, 5000, 5500, 5750, 6000],
        y: &[],
        // real, confirmed: Alfa.Bosch.EDC16C39.0281014459.394001.v0.bin (+2 more file(s) with this exact grid)
    },
];

/// Axis keys for `TrqMaxGear1` (Gearbox Torque Limiter 1). 15-point curve, corpus address 0x1CE5D6;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_TRQMAXGEAR1: &[AxisKey] = &[
    AxisKey {
        nx: 15,
        ny: 0,
        x: &[0, 400, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500, 5000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+16 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 15,
        ny: 0,
        x: &[400, 410, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500, 5000],
        y: &[],
        // real, confirmed: Fiat.Doblo.Bosch.EDC16C39.51798817.384357.v0.bin
    },
];

/// Axis keys for `TrqMaxGear2` (Gearbox Torque Limiter 2). 15-point curve, corpus address 0x1CE614;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_TRQMAXGEAR2: &[AxisKey] = &[
    AxisKey {
        nx: 15,
        ny: 0,
        x: &[0, 400, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3500, 4000, 4500, 5000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+17 more file(s) with this exact grid)
    },
];

/// Axis keys for `TrqMaxGear3` (Gearbox Torque Limiter 3). 15-point curve, corpus address 0x1CE652;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_TRQMAXGEAR3: &[AxisKey] = &[
    AxisKey {
        nx: 15,
        ny: 0,
        x: &[0, 400, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500, 5000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+17 more file(s) with this exact grid)
    },
];

/// Axis keys for `TrqMaxGear4` (Gearbox Torque Limiter 4). 15-point curve, corpus address 0x1CE690;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_TRQMAXGEAR4: &[AxisKey] = &[
    AxisKey {
        nx: 15,
        ny: 0,
        x: &[0, 400, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500, 5000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+17 more file(s) with this exact grid)
    },
];

/// Axis keys for `TrqMaxGear5` (Gearbox Torque Limiter 5). 15-point curve, corpus address 0x1CE6CE;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_TRQMAXGEAR5: &[AxisKey] = &[
    AxisKey {
        nx: 15,
        ny: 0,
        x: &[0, 400, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500, 5000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+17 more file(s) with this exact grid)
    },
];

/// Axis keys for `TrqMaxGear6` (Gearbox Torque Limiter 6). 15-point curve, corpus address 0x1CE70C;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_TRQMAXGEAR6: &[AxisKey] = &[
    AxisKey {
        nx: 15,
        ny: 0,
        x: &[0, 400, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500, 5000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+17 more file(s) with this exact grid)
    },
];

/// Axis keys for `TrqMaxGearR` (Gearbox Torque Limiter R). 15-point curve, corpus address 0x1CE74A;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_TRQMAXGEARR: &[AxisKey] = &[
    AxisKey {
        nx: 15,
        ny: 0,
        x: &[0, 400, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 4000, 4500, 5000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+17 more file(s) with this exact grid)
    },
];

/// Axis keys for `EgrHys1` (EGR hysteresis 1). 25-point curve, corpus address 0x1C4AD4;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_EGRHYS1: &[AxisKey] = &[
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[500, 1000, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4750, 5000, 5250, 5500, 5750, 6000, 6250, 6500, 6750, 7000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+25 more file(s) with this exact grid)
    },
];

/// Axis keys for `EgrHys2` (EGR hysteresis 2). 25-point curve, corpus address 0x1C4B3A;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_EGRHYS2: &[AxisKey] = &[
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[750, 800, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4501, 4502, 4503, 4504, 4505, 4506, 4507, 4508],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+18 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[0, 1000, 1250, 1500, 2000, 3000, 3100, 3400, 4000, 4500, 4510, 4520, 4530, 4540, 4550, 4560, 4570, 4580, 4590, 4600, 4610, 4620, 4630, 4640, 4650],
        y: &[],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281012145.377554.v0.bin (+3 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[700, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4501, 4502, 4503, 4504, 4505, 4506, 4507, 4508],
        y: &[],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281013138.380494.v0.bin (+1 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[650, 700, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4501, 4502, 4503, 4504, 4505, 4506, 4507, 4508],
        y: &[],
        // real, confirmed: Alfa.Bosch.EDC16C39.0281012146.389875.v0.bin
    },
];

/// Axis keys for `EgrHys3` (EGR hysteresis 3). 25-point curve, corpus address 0x1C4BA0;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_EGRHYS3: &[AxisKey] = &[
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[500, 1000, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4750, 5000, 5250, 5500, 5750, 6000, 6250, 6500, 6750, 7000],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+25 more file(s) with this exact grid)
    },
];

/// Axis keys for `EgrHys4` (EGR hysteresis 4). 25-point curve, corpus address 0x1C4C06;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_EGRHYS4: &[AxisKey] = &[
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[750, 800, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4501, 4502, 4503, 4504, 4505, 4506, 4507, 4508],
        y: &[],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+18 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[0, 1000, 1250, 1500, 2000, 3000, 3100, 3400, 4000, 4500, 4510, 4520, 4530, 4540, 4550, 4560, 4570, 4580, 4590, 4600, 4610, 4620, 4630, 4640, 4650],
        y: &[],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281012145.377554.v0.bin (+3 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[700, 750, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4501, 4502, 4503, 4504, 4505, 4506, 4507, 4508],
        y: &[],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281013138.380494.v0.bin (+1 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 25,
        ny: 0,
        x: &[650, 700, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250, 4500, 4501, 4502, 4503, 4504, 4505, 4506, 4507, 4508],
        y: &[],
        // real, confirmed: Alfa.Bosch.EDC16C39.0281012146.389875.v0.bin
    },
];

/// Axis keys for `TrqStrtBas` (Cranking Torque Map). 16x16 (+ declared alternate grids), corpus address 0x1E7374;
/// each read at that family's slot on a real corpus dump.
pub const C39_KEYS_TRQSTRTBAS: &[AxisKey] = &[
    AxisKey {
        nx: 10,
        ny: 16,
        x: &[0, 50, 400, 500, 600, 700, 800, 900, 1000, 1100],
        y: &[2431, 2481, 2531, 2631, 2731, 2831, 2931, 3131, 3331, 3631, 3741, 3751, 3761, 3771, 3781, 3791],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+5 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 100, 400, 500, 600, 650, 700, 750, 800, 850, 900, 946, 1000, 1050, 1150, 1250],
        y: &[2431, 2531, 2581, 2631, 2731, 2831, 2931, 3131, 3331, 3731, 3741, 3751, 3761, 3771, 3781, 3791],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.386726.v0.bin (+2 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 100, 400, 500, 600, 650, 700, 750, 800, 850, 900, 950, 1000, 1050, 1150, 1250],
        y: &[2431, 2531, 2631, 2731, 2831, 2931, 3131, 3331, 3531, 3731, 3741, 3751, 3761, 3771, 3781, 3791],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281012145.377554.v0.bin (+5 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 100, 400, 500, 603, 650, 700, 750, 800, 850, 900, 946, 1000, 1050, 1150, 1250],
        y: &[2431, 2531, 2631, 2731, 2831, 2931, 3131, 3331, 3531, 3731, 3741, 3751, 3761, 3771, 3781, 3791],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281013138.380494.v0.bin (+1 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 10,
        ny: 16,
        x: &[0, 200, 400, 500, 600, 650, 700, 800, 1000, 1250],
        y: &[2431, 2481, 2531, 2631, 2731, 2831, 2931, 3131, 3331, 3631, 3741, 3751, 3761, 3771, 3781, 3791],
        // real, confirmed: FiatCroma_1.9mjtd_0281012147_A.bin
    },
];

/// Axis key for IA_FuelAccel1 (Fuel During Acceleration 1). 16x16, address 0x1B19A0.
pub const C39_KEYS_IA_ACCEL_1: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 500, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250],
        y: &[0, 410, 819, 1229, 1638, 2048, 2867, 3277, 3686, 4096, 4506, 5325, 5734, 6554, 7373, 8192],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];

/// Axis key for IA_FuelAccel2 (Fuel During Acceleration 2). 16x16, address 0x1B1BE4.
pub const C39_KEYS_IA_ACCEL_2: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 500, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250],
        y: &[0, 410, 819, 1229, 1638, 2048, 2867, 3277, 3686, 4096, 4506, 5325, 5734, 6554, 7373, 8192],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];

/// Axis key for IA_FuelAccel3 (Fuel During Acceleration 3). 16x16, address 0x1B1E28.
pub const C39_KEYS_IA_ACCEL_3: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 500, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250],
        y: &[0, 410, 819, 1229, 1638, 2048, 2867, 3277, 3686, 4096, 4506, 5325, 5734, 6554, 7373, 8192],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];

/// Axis key for IA_FuelAccel4 (Fuel During Acceleration 4). 16x16, address 0x1B206C.
pub const C39_KEYS_IA_ACCEL_4: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 500, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250],
        y: &[0, 410, 819, 1229, 1638, 2048, 2867, 3277, 3686, 4096, 4506, 5325, 5734, 6554, 7373, 8192],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];

/// Axis key for IA_FuelAccel5 (Fuel During Acceleration 5). 16x16, address 0x1B22B0.
pub const C39_KEYS_IA_ACCEL_5: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 300, 500, 800, 1000, 1500, 2000, 2500, 2700, 3000, 3300, 3700, 4000, 4500, 5000, 6000],
        y: &[0, 410, 819, 1229, 1638, 2048, 2867, 3277, 3686, 4096, 4506, 5325, 5734, 6554, 7373, 8192],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];

/// Axis key for IA_FuelAccel6 (Fuel During Acceleration 6). 16x16, address 0x1B24F4.
pub const C39_KEYS_IA_ACCEL_6: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 300, 500, 800, 1000, 1500, 2000, 2500, 2700, 3000, 3300, 3700, 4000, 4500, 5000, 6000],
        y: &[0, 410, 819, 1229, 1638, 2048, 2867, 3277, 3686, 4096, 4506, 5325, 5734, 6554, 7373, 8192],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];

/// Axis key for IA_FuelAccel7 (Fuel During Acceleration 7). 16x16, address 0x1B2738.
pub const C39_KEYS_IA_ACCEL_7: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 300, 500, 800, 1000, 1500, 2000, 2500, 2700, 3000, 3300, 3700, 4000, 4500, 5000, 6000],
        y: &[0, 410, 819, 1229, 1638, 2048, 2867, 3277, 3686, 4096, 4506, 5325, 5734, 6554, 7373, 8192],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];

/// Axis key for IA_FuelAccel8 (Fuel During Acceleration 8). 16x16, address 0x1B297C.
pub const C39_KEYS_IA_ACCEL_8: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 500, 1000, 1250, 1500, 1750, 2000, 2250, 2500, 2750, 3000, 3250, 3500, 3750, 4000, 4250],
        y: &[0, 410, 819, 1229, 1638, 2048, 2458, 2867, 3277, 3686, 4096, 4915, 5734, 6554, 7373, 8192],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];





/// Axis key for I3_InjCrvCorr1 (Injection Timing Correction 1). 16x16, address 0x1D1F98.
pub const C39_KEYS_I3_INJCRV_CORR1: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 830, 870, 880, 920, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+7 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 850, 900, 950, 980, 1050, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281013138.380494.v0.bin
    },
];

/// Axis key for I3_InjCrvCorr2 (Injection Timing Correction 2). 16x16, address 0x1D21DC.
pub const C39_KEYS_I3_INJCRV_CORR2: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 830, 870, 880, 920, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+7 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 850, 900, 950, 980, 1050, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281013138.380494.v0.bin
    },
];

/// Axis key for I3_InjCrvCorr3 (Injection Timing Correction 3). 16x16, address 0x1D2420.
pub const C39_KEYS_I3_INJCRV_CORR3: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 830, 870, 880, 920, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+7 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 850, 900, 950, 980, 1050, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281013138.380494.v0.bin
    },
];

/// Axis key for I3_InjCrvCorr4 (Injection Timing Correction 4). 16x16, address 0x1D2664.
pub const C39_KEYS_I3_INJCRV_CORR4: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 830, 870, 880, 920, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+7 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 850, 900, 950, 980, 1050, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281013138.380494.v0.bin
    },
];

/// Axis key for I3_InjCrvCorr5 (Injection Timing Correction 5). 16x16, address 0x1D28A8.
pub const C39_KEYS_I3_INJCRV_CORR5: &[AxisKey] = &[
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 830, 870, 880, 920, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.147.Bosch.EDC16C39.0281012882.386722.v0.bin (+7 more file(s) with this exact grid)
    },
    AxisKey {
        nx: 16,
        ny: 16,
        x: &[0, 850, 900, 950, 980, 1050, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000],
        y: &[0, 500, 1000, 1500, 2000, 2500, 3000, 3500, 4000, 4500, 5000, 5500, 6000, 6500, 7000, 7500],
        // real, confirmed: Alfa.159.Bosch.EDC16C39.0281013138.380494.v0.bin
    },
];

/// Axis key for BS_TurboPressureCorr1 (Turbo Pressure Correction 1). 20x16, address 0x1E367A.
pub const C39_KEYS_BS_TURBO2_1: &[AxisKey] = &[
    AxisKey {
        nx: 20,
        ny: 16,
        x: &[400, 600, 720, 850, 1000, 1200, 1350, 1500, 1800, 2000, 2250, 2500, 2750, 3000, 3300, 3600, 3900, 4200, 4600, 5000],
        y: &[0, 200, 400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2600, 2900, 3200],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];

/// Axis key for BS_TurboPressureCorr2 (Turbo Pressure Correction 2). 20x16, address 0x1E3946.
pub const C39_KEYS_BS_TURBO2_2: &[AxisKey] = &[
    AxisKey {
        nx: 20,
        ny: 16,
        x: &[400, 600, 720, 850, 1000, 1200, 1350, 1500, 1800, 2000, 2250, 2500, 2750, 3000, 3300, 3600, 3900, 4200, 4600, 5000],
        y: &[0, 200, 400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000, 2200, 2400, 2600, 2900, 3200],
        // real, confirmed: Fiat.DUCATO.Bosch.EDC16C39.0281016051.398951.v0.bin (+5 more file(s) with this exact grid)
    },
];

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
        symbol_confirmed: true,
        shape: Shape::Map,
        // Grids the reference database declares for this family on other real builds.
        alt_grids: &[(13, 10), (13, 9), (16, 11), (16, 12)],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        // Grids the reference database declares for this family on other real builds (16x8 on 55 of 222).
        alt_grids: &[(16, 8)],
        // A single ceiling on every one of the 18 corpus files that carry it (4800-5600 hPa),
        // raised by real tunes: flat is the family, not an erased zone.
        allow_flat: true,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        // Grids the reference database declares for this family on other real builds.
        alt_grids: &[(13, 14)],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        // Grids the reference database declares for this family on other real builds.
        alt_grids: &[(13, 10)],
        allow_flat: false,
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
        symbol_confirmed: true,
        shape: Shape::Map,
        // Grids the reference database declares for this family on other real builds.
        alt_grids: &[(13, 10)],
        allow_flat: false,
        calibrated: true,
    },
    // Engine-protection torque ceiling over engine speed. Reference database: 25-point curve on
    // 220 of 222 builds; decodes at its declared address on the shared corpus build.
    MapTemplate {
        name: "Torque Limiter",
        bosch_label: "EngPrt_trqLim",
        category: MapCategory::EngineTorqueLimiters,
        nx: 25,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 600.00),
        z_range_tuned: (0.00, 3276.70),
        unit: "Nm",
        address: 0x1CBCF6,
        signed: false,
        axis_keys: C39_KEYS_ENGPRT_TRQLIM,
        symbol_confirmed: true,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    // Seven back-to-back 15-point curves (gears 1..6, R), a constant 0x3E apart on 221 of the
    // reference database's 222 builds. Mostly flat on stock files (a single ceiling, 1000 Nm
    // = not limiting), hence allow_flat; resolved only as a complete run (see mod.rs).
    MapTemplate {
        name: "Gearbox Torque Limiter 1",
        bosch_label: "TrqMaxGear1",
        category: MapCategory::GearboxTorqueLimiter,
        nx: 15,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (100.00, 1100.00),
        z_range_tuned: (0.00, 3276.70),
        unit: "Nm",
        address: 0x1CE5D6,
        signed: false,
        axis_keys: C39_KEYS_TRQMAXGEAR1,
        symbol_confirmed: true,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    MapTemplate {
        name: "Gearbox Torque Limiter 2",
        bosch_label: "TrqMaxGear2",
        category: MapCategory::GearboxTorqueLimiter,
        nx: 15,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (100.00, 1100.00),
        z_range_tuned: (0.00, 3276.70),
        unit: "Nm",
        address: 0x1CE614,
        signed: false,
        axis_keys: C39_KEYS_TRQMAXGEAR2,
        symbol_confirmed: true,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    MapTemplate {
        name: "Gearbox Torque Limiter 3",
        bosch_label: "TrqMaxGear3",
        category: MapCategory::GearboxTorqueLimiter,
        nx: 15,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (100.00, 1100.00),
        z_range_tuned: (0.00, 3276.70),
        unit: "Nm",
        address: 0x1CE652,
        signed: false,
        axis_keys: C39_KEYS_TRQMAXGEAR3,
        symbol_confirmed: true,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    MapTemplate {
        name: "Gearbox Torque Limiter 4",
        bosch_label: "TrqMaxGear4",
        category: MapCategory::GearboxTorqueLimiter,
        nx: 15,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (100.00, 1100.00),
        z_range_tuned: (0.00, 3276.70),
        unit: "Nm",
        address: 0x1CE690,
        signed: false,
        axis_keys: C39_KEYS_TRQMAXGEAR4,
        symbol_confirmed: true,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    MapTemplate {
        name: "Gearbox Torque Limiter 5",
        bosch_label: "TrqMaxGear5",
        category: MapCategory::GearboxTorqueLimiter,
        nx: 15,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (100.00, 1100.00),
        z_range_tuned: (0.00, 3276.70),
        unit: "Nm",
        address: 0x1CE6CE,
        signed: false,
        axis_keys: C39_KEYS_TRQMAXGEAR5,
        symbol_confirmed: true,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    MapTemplate {
        name: "Gearbox Torque Limiter 6",
        bosch_label: "TrqMaxGear6",
        category: MapCategory::GearboxTorqueLimiter,
        nx: 15,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (100.00, 1100.00),
        z_range_tuned: (0.00, 3276.70),
        unit: "Nm",
        address: 0x1CE70C,
        signed: false,
        axis_keys: C39_KEYS_TRQMAXGEAR6,
        symbol_confirmed: true,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    MapTemplate {
        name: "Gearbox Torque Limiter R",
        bosch_label: "TrqMaxGearR",
        category: MapCategory::GearboxTorqueLimiter,
        nx: 15,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (100.00, 1100.00),
        z_range_tuned: (0.00, 3276.70),
        unit: "Nm",
        address: 0x1CE74A,
        signed: false,
        axis_keys: C39_KEYS_TRQMAXGEARR,
        symbol_confirmed: true,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    // EGR hysteresis 1..4: four back-to-back 25-point curves over engine speed in the
    // air-control zone. The reference database does not name them, so symbol_confirmed is
    // false and these ids are ours. Identified by: (a) structure -- two high/low pairs, 1/3
    // and 2/4 each sharing an axis, the high curve a constant 2-5 mg above the low one on
    // every corpus file, the shape the VAG EDC16 families here report as "EGR hysteresis";
    // (b) real edits -- every EGR-off tune in the corpus sets them to 0, and a Stage 1 Croma
    // tune scales them by the same 1.25 as its injected quantity, hence mg/stroke. Found on
    // 26 of 42 corpus files (0x1C4AD4, or +0xFC on Grande Punto/Punto); resolved only as a
    // complete run (see mod.rs).
    MapTemplate {
        name: "EGR hysteresis 1",
        bosch_label: "EgrHys1",
        category: MapCategory::Egr,
        nx: 25,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 80.00),
        z_range_tuned: (0.00, 327.67),
        unit: "mg/stroke",
        address: 0x1C4AD4,
        signed: false,
        axis_keys: C39_KEYS_EGRHYS1,
        symbol_confirmed: false,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    MapTemplate {
        name: "EGR hysteresis 2",
        bosch_label: "EgrHys2",
        category: MapCategory::Egr,
        nx: 25,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 80.00),
        z_range_tuned: (0.00, 327.67),
        unit: "mg/stroke",
        address: 0x1C4B3A,
        signed: false,
        axis_keys: C39_KEYS_EGRHYS2,
        symbol_confirmed: false,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    MapTemplate {
        name: "EGR hysteresis 3",
        bosch_label: "EgrHys3",
        category: MapCategory::Egr,
        nx: 25,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 80.00),
        z_range_tuned: (0.00, 327.67),
        unit: "mg/stroke",
        address: 0x1C4BA0,
        signed: false,
        axis_keys: C39_KEYS_EGRHYS3,
        symbol_confirmed: false,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    MapTemplate {
        name: "EGR hysteresis 4",
        bosch_label: "EgrHys4",
        category: MapCategory::Egr,
        nx: 25,
        ny: 1,
        axes: (AxisType::Rpm, AxisType::Rpm),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 80.00),
        z_range_tuned: (0.00, 327.67),
        unit: "mg/stroke",
        address: 0x1C4C06,
        signed: false,
        axis_keys: C39_KEYS_EGRHYS4,
        symbol_confirmed: false,
        shape: Shape::Curve,
        alt_grids: &[],
        allow_flat: true,
        calibrated: true,
    },
    // Starting torque over engine speed x coolant temperature. The reference database declares
    // it in nine grids across its 222 builds (16x16 on 97, 10x16 on 65, ...); the corpus carries
    // 16x16 and 10x16. Left out of earlier revisions for exactly that grid spread.
    MapTemplate {
        name: "Cranking Torque Map",
        bosch_label: "TrqStrtBas",
        category: MapCategory::EngineTorqueRequest,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Temperature),
        z_factor: 0.1000000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 600.00),
        z_range_tuned: (0.00, 3276.70),
        unit: "Nm",
        address: 0x1E7374,
        signed: false,
        axis_keys: C39_KEYS_TRQSTRTBAS,
        symbol_confirmed: true,
        shape: Shape::Map,
        alt_grids: &[(10, 16), (9, 10), (12, 10), (8, 10), (14, 16), (11, 10), (12, 14), (16, 8)],
        allow_flat: false,
        calibrated: true,
    },
    // Real, but not Bosch-named: ECM Titanium's "axis_table" DRT variant (a categorical English
    // description per driver-vehicle entry, not the per-chip Bosch symbol the named_maps variant
    // carries) lists 8 back-to-back 16x16 maps here as "Fuel during acceleration", one stride apart,
    // each independently confirmed on 6 real corpus files. accepts_grid/axis checks are unaffected by
    // the missing Bosch name; symbol_confirmed is false so build_map's description says so honestly.
    MapTemplate {
        name: "Fuel During Acceleration 1",
        bosch_label: "IA_FuelAccel1",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 60.00),
        z_range_tuned: (0.00, 120.00),
        unit: "mm3/cyc",
        address: 0x1B19A0,
        signed: false,
        axis_keys: C39_KEYS_IA_ACCEL_1,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Fuel During Acceleration 2",
        bosch_label: "IA_FuelAccel2",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 60.00),
        z_range_tuned: (0.00, 120.00),
        unit: "mm3/cyc",
        address: 0x1B1BE4,
        signed: false,
        axis_keys: C39_KEYS_IA_ACCEL_2,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Fuel During Acceleration 3",
        bosch_label: "IA_FuelAccel3",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 60.00),
        z_range_tuned: (0.00, 120.00),
        unit: "mm3/cyc",
        address: 0x1B1E28,
        signed: false,
        axis_keys: C39_KEYS_IA_ACCEL_3,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Fuel During Acceleration 4",
        bosch_label: "IA_FuelAccel4",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 60.00),
        z_range_tuned: (0.00, 120.00),
        unit: "mm3/cyc",
        address: 0x1B206C,
        signed: false,
        axis_keys: C39_KEYS_IA_ACCEL_4,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Fuel During Acceleration 5",
        bosch_label: "IA_FuelAccel5",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 60.00),
        z_range_tuned: (0.00, 120.00),
        unit: "mm3/cyc",
        address: 0x1B22B0,
        signed: false,
        axis_keys: C39_KEYS_IA_ACCEL_5,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Fuel During Acceleration 6",
        bosch_label: "IA_FuelAccel6",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 60.00),
        z_range_tuned: (0.00, 120.00),
        unit: "mm3/cyc",
        address: 0x1B24F4,
        signed: false,
        axis_keys: C39_KEYS_IA_ACCEL_6,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Fuel During Acceleration 7",
        bosch_label: "IA_FuelAccel7",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 60.00),
        z_range_tuned: (0.00, 120.00),
        unit: "mm3/cyc",
        address: 0x1B2738,
        signed: false,
        axis_keys: C39_KEYS_IA_ACCEL_7,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Fuel During Acceleration 8",
        bosch_label: "IA_FuelAccel8",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::Percent),
        z_factor: 0.0100000000,
        z_offset: 0.0,
        z_range_stock: (0.00, 60.00),
        z_range_tuned: (0.00, 120.00),
        unit: "mm3/cyc",
        address: 0x1B297C,
        signed: false,
        axis_keys: C39_KEYS_IA_ACCEL_8,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    // Real, but not Bosch-named (see IA_FuelAccel1's note). ECM Titanium's axis_table variant names
    // this 5-member 16x16 run "Injection at part throttle (Map 2)" -- distinct from InjCrv_Bas1..5
    // (different address region, same RPM x quantity axis convention), confirmed on 9 real files.
    // Z factor and physical range reuse InjCrv_Bas's (same axis convention, same real signed span).
    MapTemplate {
        name: "Injection Timing Correction 1",
        bosch_label: "I3_InjCrvCorr1",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-30.00, 25.00),
        z_range_tuned: (-90.00, 75.00),
        unit: "deg",
        address: 0x1D1F98,
        signed: true,
        axis_keys: C39_KEYS_I3_INJCRV_CORR1,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Injection Timing Correction 2",
        bosch_label: "I3_InjCrvCorr2",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-30.00, 25.00),
        z_range_tuned: (-90.00, 75.00),
        unit: "deg",
        address: 0x1D21DC,
        signed: true,
        axis_keys: C39_KEYS_I3_INJCRV_CORR2,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Injection Timing Correction 3",
        bosch_label: "I3_InjCrvCorr3",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-30.00, 25.00),
        z_range_tuned: (-90.00, 75.00),
        unit: "deg",
        address: 0x1D2420,
        signed: true,
        axis_keys: C39_KEYS_I3_INJCRV_CORR3,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Injection Timing Correction 4",
        bosch_label: "I3_InjCrvCorr4",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-30.00, 25.00),
        z_range_tuned: (-90.00, 75.00),
        unit: "deg",
        address: 0x1D2664,
        signed: true,
        axis_keys: C39_KEYS_I3_INJCRV_CORR4,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Injection Timing Correction 5",
        bosch_label: "I3_InjCrvCorr5",
        category: MapCategory::InjectionSystem,
        nx: 16,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 0.0234356691,
        z_offset: 0.0,
        z_range_stock: (-30.00, 25.00),
        z_range_tuned: (-90.00, 75.00),
        unit: "deg",
        address: 0x1D28A8,
        signed: true,
        axis_keys: C39_KEYS_I3_INJCRV_CORR5,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    // Real, but not Bosch-named (see IA_FuelAccel1's note). ECM Titanium's axis_table variant names
    // this 2-member 20x16 run "Turbo pressure" -- distinct from PCR_CtlBas/PCR_DesBas (different
    // address region, own real RPM x quantity axis), confirmed on 6 real files. Z factor reuses the
    // boost family's 2.0 hPa convention (PCR_DesBas/PCR_DesMaxAP).
    MapTemplate {
        name: "Turbo Pressure Correction 1",
        bosch_label: "BS_TurboPressureCorr1",
        category: MapCategory::TurboBoostPressure,
        nx: 20,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 2.0000000000,
        z_offset: 0.0,
        z_range_stock: (500.00, 5000.00),
        z_range_tuned: (0.00, 10000.00),
        unit: "hPa",
        address: 0x1E367A,
        signed: false,
        axis_keys: C39_KEYS_BS_TURBO2_1,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
    MapTemplate {
        name: "Turbo Pressure Correction 2",
        bosch_label: "BS_TurboPressureCorr2",
        category: MapCategory::TurboBoostPressure,
        nx: 20,
        ny: 16,
        axes: (AxisType::Rpm, AxisType::InjectionQty),
        z_factor: 2.0000000000,
        z_offset: 0.0,
        z_range_stock: (500.00, 5000.00),
        z_range_tuned: (0.00, 10000.00),
        unit: "hPa",
        address: 0x1E3946,
        signed: false,
        axis_keys: C39_KEYS_BS_TURBO2_2,
        symbol_confirmed: false,
        shape: Shape::Map,
        alt_grids: &[],
        allow_flat: false,
        calibrated: true,
    },
];
