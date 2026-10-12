ZedSuite GN / GQ / GK / FD - developer dump and consolidated update
Prepared: 2026-10-12; final source includes explicit source-bank and saved/working choices
Scope: map detection fixes, ECU Compatibility reference, and experimental Multi-Map Beta. No other ECU conversion.
Base: LeZed97/ZedSuite master 77d75fb9e28df04379fd6d1912197947f1cce0c0, 1.2.6.
Baseline verification: all four existing modified files have identical Git blob
hashes in the supplied source ZIP and that upstream commit. Four added files; eight source files total.
Detector cache revision: 50 -> 52 through the existing version-refresh mechanism.

CHANGES
1. GN4308 SOI limiter: retain known 12x5 F9/C5 tables at 0x5AFFC and
   0x7AFFC when a modified monotonic temperature axis exceeds stock ranges.
   Preserve Kelvin*10 decoding; add a warning to suspicious calibration.
   Do not relabel the values using a guessed alternate temperature encoding.
2. IQ by MAP: recognize extended DA pressure axes through 4000 raw/mbar
   for the scoped structure, using monotonicity/start/range checks to avoid
   misclassifying N75 maps with a different axis range.
3. MAP linearization: admit DC9C upper-pressure endpoints 3501..5000 with
   existing ADC/low-pressure checks. Preserve factor 1.0. This does not
   validate any installed sensor's electrical or ECU operating range.
4. Three adjacent 64 KiB calibration banks: for the exact 512 KiB signature
   layout 0x50000/0x60000/0x70000, shift grouping boundaries by -0x4000.
   CB1 [4C000,5C000), CB2 [5C000,6C000), CB3 [6C000,7C000).
   This restores per-bank MAF-temperature detection and driver-wish grouping.
   Existing separated two-bank grouping is preserved.
5. Inverse driver wish: preserve EDC15VM results through app finalization;
   retain the prior filtering for other ECU families and MAF linearization.
6. Start IQ: recognize both 9x9 and GQ 9x8 structures with scoped axis families.
   A stock 9x8 table is displayed as 9x8; detection does not resize ECU data.

Detector-only submission. Experimental conversion and UI are in separate review branches.

Fresh validation: complete Rust library test suite 64 PASS; two local fixture tests PASS.
Seven ECU-specific detector dumps are supplied in Map_Dump.json (before the app finalizer).

| Fixture | ECU-specific map count | Review notes |
|---|---:|---|
| GN4308 OEM | 80 | Stock limiter and both Start IQ layouts |
| GN4308 modified SOI | 82 | Monotonic modified temperature axis; Kelvin scale retained and warning emitted |
| GN supplied multimap | 123 | Three bank grouping and extended IQ/MAP axes |
| FD4102 supplied | 120 | Three occupied original banks |
| GQ4307 stock | 80 | Manual 9x9 and automatic 9x8 Start IQ |
| GQ supplied multimap | 120 | Start IQ recognized in final 9x8 bank |
| GK KD34 stock | 80 | Original two occupied banks |

Counts are observations, not a full reference-mappack completeness claim.
Selected map dimensions, axis fields and fixture hashes are in Map_Dump.json.
The complete maintainer stock/tuned bench was not available; wider-family byte-identical
regression comparison and independent axis/reference review remain outstanding.
No ECU dumps, mappacks or private file paths are committed. Product version unchanged.
