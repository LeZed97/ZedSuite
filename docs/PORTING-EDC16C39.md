# Porting notes: Bosch EDC16C39 (Fiat / Alfa Romeo Multijet)

Status: **calibrated for the Alfa 159/147-class 2MB passenger-car layout only**,
opened as a draft PR per CONTRIBUTING.md while the corpus grows. See
`src-tauri/src/detector/ecu/bosch/edc16c39/signatures.rs`'s module doc for the
per-family reference table and `mod.rs`'s doc for the detection mechanism.

## Corpus

3 real dumps, pulled from a large third-party tuning archive (not committed,
per CONTRIBUTING.md's ground rules):

| Vehicle | Engine | Bosch SW | Real, independent build? |
|---|---|---|---|
| Alfa Romeo 159 | 1.9 JTDm | (reference build) | yes |
| Alfa Romeo 159 | 1.9 JTDm | different real-world flash | yes |
| Alfa Romeo 147 | 1.9 JTD | different real-world flash | yes |

Plus 2 files used only to confirm what is **NOT** covered:
- Fiat Ducato (commercial van): same WinOLS "EDC16C39" label, genuinely
  different calibration layout (confirmed: none of the 21 calibrated
  addresses decode to anything on this file). Explicitly out of scope.
- Fiat Bravo: a 256KB dump under the same label — either a different flash
  chip generation or an EEPROM-only read; also out of scope, not investigated
  further this pass.

## Method

1. **A real, independent, third-party ECU reference database** (not this
   project's own guesswork) gave a starting list of ~30 named Bosch internal
   symbols for one specific chip build, with declared address, dimensions,
   scale factor and physical unit each — the equivalent role an A2L/damos
   plays for the CP31 port, cross-checked against real files rather than
   trusted blindly.
2. Every one of those ~30 addresses was read directly out of the 3 real
   corpus dumps above. 22 came back with **byte-identical address, block
   dimensions AND breakpoint values across all three** independent files —
   accepted as calibrated. The rest (1D curves, scalars, and one family whose
   grid genuinely differed between two of the three files) were left out
   rather than guessed at — see "Known limits" below.
3. Confirmed the on-disk record is the same self-describing
   `[nx][ny][X][Y][Z]` layout already known from the (unrelated OEM)
   EDC16CP31 family — read directly from real bytes at the reference
   address, not assumed by analogy.
4. A real, load-bearing correctness bug was caught by testing against more
   than one file rather than trusting the first: several families' physical
   range needed a `signed`/`unsigned` distinction (`InjCrv_Bas*`,
   `AccPed_trq*`, `TrqEngDriveAway` are legitimately negative — timing
   retard, engine-braking torque) and the generic axis-range gate borrowed
   from CP31 (tuned to a Mercedes corpus) rejected several real EDC16C39
   axes outright until widened to this platform's own observed values.

## What this PR adds

- `src-tauri/src/detector/ecu/bosch/edc16c39/{mod.rs,signatures.rs}`: 21
  calibrated 2D map families.
- Identifier gate in `ecu_identifier.rs`: the literal `"EDC16C39"` family
  string (found in every one of the 5 real files checked, including the two
  out-of-scope ones — a real, findable marker, unlike the HW-prefix
  heuristic the VAG variants rely on, which this chip's *prefix* shares with
  several unrelated OEMs), raised to high confidence only once the confirmed
  EGR block also decodes structurally.
- `src-tauri/examples/edc16c39_real_check.rs`: bench tool, same convention as
  `dump_maps`.

## Bench result (3 corpus files, `edc16c39_real_check`)

| File | Maps found |
|---|---|
| Alfa 159 (reference build) | 23 |
| Alfa 159 (different build) | 22 |
| Alfa 147 | 22 |
| Fiat Ducato (out of scope) | **0** — confirms the safety contract holds |

## Known limits (read before trusting this beyond the 3-file corpus)

- **All 3 corpus files are the same vehicle class** (Alfa 159/147 1.9
  JTD/JTDm). CONTRIBUTING.md's bar is 50 files across software versions and
  engines; this is a starting point, not a finished detector. Fiat Group
  passenger cars sharing the same platform (Bravo, Doblo, Croma, Grande
  Punto, Giulietta, Mito, Brera, Spider — all real models seen carrying this
  same "EDC16C39" WinOLS label in the archive this corpus was drawn from) are
  UNCHECKED, not assumed to work.
- **No completeness/invariant report yet** — needs the "20 files, know what's
  always there" pass CONTRIBUTING.md describes, not statistics from 3 files.
- **`PCR_pBDesMaxAP` (Boost Pressure Limiter) is address-confirmed but not yet
  detected**: its Z data reads as a flat constant on all 3 corpus files, and
  the generic "a calibration map is never flat" rejection (needed everywhere
  else, and inherited from the CP31 design) throws it out. Possibly a
  genuinely fixed ceiling on this engine, possibly a fourth file would show
  real variation — not enough evidence yet to special-case the check safely.
- **`AirCtl_rEGRBas` (EGR) and `LmbdSmkHigh`/`LmbdSmkLow` (smoke limiter) each
  report twice**, at two real, back-to-back, structurally-identical
  addresses, consistently across all 3 files. Not yet root-caused: possibly
  two genuine real tables (e.g. a second calibration variant), possibly one
  is something else that happens to share the exact grid. Both instances
  decode to physically plausible values either way, so this is a duplicate
  count, not a wrong-value risk — but it means the completeness report can't
  be written honestly yet (see previous point).
- **1D curves and scalars are not read at all** (`TrqMaxGear1..6+R`,
  `EngPrt_trqLim`, `SpdLimMax`, `PCR_pDesMax`) — confirmed present at fixed
  real addresses via the same reference database, structurally different
  on-disk record this pass did not implement a reader for. Matches CP31's
  own staged curve support (2/47 shipped initially).
- **`TrqStrtBas`** was seen at a stable address but with a genuinely
  different grid on one of the 3 files (16x16 vs 10x16) — left out rather
  than guessed at.
- **No mappack export, no "solution"** — matches CONTRIBUTING.md's rule and
  the same caution CP31 took with a one-software corpus.

## What would get this out of draft

Per CONTRIBUTING.md's own bar: a corpus in the range of 50 real files across
software versions and the Fiat Group models/engines that carry this chip,
stock and tuned; a bench table like the one above run against that full
corpus; real invariants ("this ECU always carries N of X") written from that
corpus, not copied from the 3-file spot-check above; and the two open
duplicate-address/flat-map questions resolved with real evidence rather than
left as documented gaps.
