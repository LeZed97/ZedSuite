# Porting notes: Bosch EDC16C39 (Fiat / Alfa Romeo Multijet)

Status: **calibrated for the 2MB passenger-car layout shared by Alfa
159/147/GT/Brera and Fiat Bravo/Croma/Doblo/Grande Punto/Punto**, confirmed
NOT to cover Fiat Ducato (commercial van, same chip, different layout);
opened as a draft PR per CONTRIBUTING.md while the corpus grows past 42
real dumps toward the 50+ bar. See
`src-tauri/src/detector/ecu/bosch/edc16c39/signatures.rs`'s module doc for the
per-family reference table and `mod.rs`'s doc for the detection mechanism.

## Corpus

Grew from an initial 3-file spot-check to **54 real dumps**, pulled from a
large third-party tuning archive (not committed, per CONTRIBUTING.md's
ground rules) plus a dedicated seedbox search for more Fiat/Alfa material:

- **42 are the calibrated 2MB layout** and were run through
  `edc16c39_real_check`, spanning: Alfa Romeo 147 (3, incl. a Ducati Corse
  170hp build), 159 (3), GT (1), Brera (2); Fiat Bravo (3), Croma (3), Doblo
  (2), Grande Punto (6), Punto (2), Ducato (10, all real, all **0 maps** —
  see below); plus 7 further real Alfa/Fiat 0281xxxxxx builds not tied to a
  named model in the archive.
- **12 are 256KB partial extracts** (Alfa Brera/Giulietta, Fiat
  Bravo/Croma/Doblo) under the same "EDC16C39" WinOLS label — too small to
  be the full flash, not run through the detector, not assumed to work.

Fiat Ducato (commercial van) is the one model checked in volume (10 real,
independent files, several software revisions) and confirmed **consistently
0 maps** on every one — genuinely a different calibration layout on the same
base chip, not a detector gap. Every other model above has at least one real
file detecting maps; a few individual builds (see "Known limits") detect 0
or very few, which on inspection reflects that specific software revision's
memory layout rather than a missed family.

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
5. As the corpus grew past the original 3 files, two further real
   mislabeling bugs surfaced and were fixed the same way — by testing
   against the new files, not by guessing:
   - **Cross-family axis-key ambiguity**: several map families
     (`InjCrv_Bas1`..`Bas5` in particular) share a byte-identical breakpoint
     grid, so searching for one family's key also matched its siblings' real
     addresses. Fixed by keeping only the single hit nearest that
     template's own confirmed address, and by treating an axis key as
     ambiguous (skipped, not guessed) if even its nearest hit is far outside
     a bounded window.
   - **Same-template multi-key duplication**: several templates accumulated
     more than one historical axis-key variant (one per real software build
     the corpus happened to add). On specific real Croma/Bravo/Grande Punto
     and Alfa dumps, TWO of a single template's own variants each matched a
     *different* real address in the same file — one the family's real
     slot, the other a different, unrelated real table that happened to
     share the same grid — and both got reported under the same name.
     Root cause: each key's hits were resolved (and pushed) independently,
     so two keys on the same template could each contribute their own
     address. Fixed by pooling every key's candidate hits per template
     first, then keeping only the single closest-to-address candidate
     across all of them combined.
6. A full code review of the draft (max effort, every finding verified with
   a temporary test or a real dump) then falsified three claims the
   previous items had made, and found the real cause of the Grande Punto
   "plateau" — fixed, with each one turned into a regression test:
   - "one result per template" was NOT structural: phase 0 (address) and
     phase 1 (axis key) could still each emit the same family at two
     addresses. Now a template found by either phase is never emitted
     again (`a_template_is_never_reported_twice_across_phases`).
   - Sibling families that share a grid AND are stored back to back
     (`InjCrv_Bas1..5`, `LmbdSmkHigh/Low`, `AccPed_trqEngA/B`) were still
     resolved member by member, so on a real Alfa 159 2.4 JTD build shifted
     by +0x400 curves 2 and 3 were reported as "4" and "5". Phase 1 now
     resolves such a run as a whole — only a complete run, in storage order
     (`a_shifted_sibling_run_keeps_its_labels_in_storage_order`, both shift
     directions; a partial run is not guessed at).
   - The axis-range gates described as "observed min/max" rejected four of
     the harvested keys themselves (Brera/Doblo grids), and `Trq2qBas` was
     read unsigned although 4 real files carry cells above 0x7FFF that only
     make sense signed. Those two together were the whole "15-16 of 21"
     Grande Punto/Punto plateau — every one of those 8 files now reaches
     18-20. Every key must now pass its template's gates
     (`every_axis_key_passes_its_templates_axis_gates`).
   - Eight families had a wrong physical meaning on their second axis
     (labelled "rpm" or "mm3" where the reference database and the
     breakpoint values say mg/Hub, Nm, degC, hPa, or bar x mm3 for
     `InjVCD_tiET`); the address window scanned upward from `address - 64`
     and kept the first valid block, so a back-to-back neighbour could win
     over the block at the address; phase 1 ignored stock/tuned mode.
   - Outside the detector: the app's name-based EGR layout heuristic
     (VAG 13x16, transposed) garbled this family's genuinely 8x16 EGR block
     in the viewer, the editor write-back and the export — the map is now
     named without the token that triggers it; the VAG EDC16 checksum
     corrector and the VAG completeness rules were both reached through
     `includes("EDC16")` and are now restricted to U1/U31/U34; the bundled
     `mappack_export_enabled: false` flag was not consumed anywhere and is
     now a real client-side gate.

## What this PR adds

- `src-tauri/src/detector/ecu/bosch/edc16c39/{mod.rs,signatures.rs}`: 21
  calibrated 2D map families (20 fire on the best real files; the 21st,
  `PCR_DesMaxAP`, is a flat constant on every corpus file and is rejected by
  the generic never-flat gate — see Known limits).
- Checksum and completeness routing in `src/lib/ecu/bosch/checksums/index.ts`
  and `src-tauri/src/commands.rs` restricted to the VAG EDC16 variants they
  were validated on, so this family gets no unvalidated "checksum OK" and no
  VAG completeness report; `mappack_export_enabled: false` from `ecus.json`
  enforced client-side in the editor.
- Identifier gate in `ecu_identifier.rs`: the literal `"EDC16C39"` family
  string (found in all 54 real files checked — the 42 usable dumps AND the
  12 partial extracts AND all 8 out-of-scope Ducato files — a real,
  consistently findable marker, unlike the HW-prefix heuristic the VAG
  variants rely on, which this chip's *prefix* shares with several unrelated
  OEMs), raised to high confidence only once the confirmed
  EGR block also decodes under the detector's own checks
  (`EDC16C39Detector::confirmed_block_present` — one implementation, so the
  identifier and the detector cannot disagree). SW number via the existing
  EDC16 `1037xxxxxx` scan, which covers where every real dump carries it.
- `src-tauri/examples/edc16c39_real_check.rs`: bench tool, same convention as
  `dump_maps`.

## Bench result (42 real 2MB files, `edc16c39_real_check`, of 21 templates)

| Model | Files | Maps found (min-max) |
|---|---|---|
| Alfa 159 | 3 | 20 on all 3 |
| Alfa 147 (incl. a Ducati Corse 170hp build) | 3 | 19-20 |
| Alfa GT | 1 | 20 |
| Alfa Brera | 2 | 20 on both |
| Fiat Croma | 3 | 16-20 |
| Fiat Doblo | 2 | 20 on both |
| Fiat Grande Punto | 6 | 18-20 |
| Fiat Punto | 2 | 20 on both |
| Fiat Bravo | 3 | 0, 5, 20 -- see "Known limits", this is the weakest-understood model |
| Other real Alfa/Fiat 0281xxxxxx builds (unnamed in the archive) | 7 | 0, 0, 1, 7, 20, 20, 20 |
| Fiat Ducato | 10 | **0 on all 10** -- confirms the safety contract holds |

526 maps over the 42 files (464 before the review fixes; every changed file
went up, none went down). No file reports the same map name twice (verified
by script across all 42, and now enforced by construction in `detect()`).

## Known limits (read before trusting this beyond this corpus)

- **Fiat Bravo is the weakest-understood model**: 3 real 2MB files range
  from 0 to 20 maps found, wider than any other model. The 20-map file
  (1.9 JTDM) behaves like Alfa/Croma; the 0-map file (1.6 MultiJet 120cv)
  genuinely carries several of this corpus's templates tens of KB away from
  every other build's address (confirmed real, via the family's own
  internal relative spacing) but far outside the window this detector
  trusts, so it correctly reports nothing rather than a guess. More real
  Bravo files, especially other 1.6 MultiJet builds, would confirm whether
  this is one outlier or a second, uncalibrated address family worth its
  own template set. Three of the seven unnamed Alfa/Fiat builds (0, 1 and 7
  maps) look like the same situation.
- **CONTRIBUTING.md's bar is 50 real files across software versions and
  engines**; this corpus (42 usable 2MB dumps) is close on raw count but
  still concentrated on 1.9 JTD/JTDm-class engines. Giulietta and Mito were
  seen carrying the same "EDC16C39" WinOLS label in the source archive but
  only as 256KB partial extracts, too small to use — still UNCHECKED.
- **No completeness/invariant report yet** — needs the "always N of X" pass
  CONTRIBUTING.md describes, written from real per-model statistics, not
  asserted from a handful of files.
- **`PCR_DesMaxAP` (Boost Pressure Limiter) is calibrated but never
  fires**: its Z data reads as a flat constant (4800-5000 hPa, per file) on
  every one of the 9 real files where the block is present, and the generic
  "a calibration map is never flat" rejection (needed everywhere else,
  inherited from the CP31 design) throws it out. Possibly a genuinely fixed
  ceiling on this engine family — not enough evidence yet to special-case
  the check safely. This is why the best files report 20, not 21.
- **1D curves and scalars are not read at all** (`TrqMaxGear1..6+R`,
  `EngPrt_trqLim`, `SpdLimMax`, `PCR_DesMax`) — confirmed present at fixed
  real addresses via the same reference database, structurally different
  on-disk record this pass did not implement a reader for. Matches CP31's
  own staged curve support (2/47 shipped initially).
- **`TrqStrtBas`** was seen at a stable address but with a genuinely
  different grid on one real file (16x16 vs 10x16) — left out rather than
  guessed at.
- **No mappack export, no checksum correction, no "solution"** — matches
  CONTRIBUTING.md's rule and the same caution CP31 took with a one-software
  corpus. Export is gated off in the editor for this family; the VAG EDC16
  additive checksum corrector is not offered for it (a real EDC16C39 dump
  carries the same region descriptors, so it WOULD engage and claim
  "checksum OK" without ever having been flash-validated on this family --
  needs a flash-validated stock/modified pair before it can be enabled).

## What would get this out of draft

Per CONTRIBUTING.md's own bar: a corpus solidly past 50 real files that also
spreads across more engines (not just 1.9 JTD/JTDm), stock and tuned; real
Giulietta/Mito 2MB dumps to replace the two 256KB partial extracts currently
unusable; more Fiat Bravo 1.6 MultiJet files to resolve whether that build's
large address drift is a one-off or a second address family worth its own
templates; a bench table like the one above run against that full corpus;
and real invariants ("this ECU always carries N of X") written from that
corpus, not asserted from a spot-check.
