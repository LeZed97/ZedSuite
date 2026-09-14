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
independent files, several software revisions): the 34 passenger-tune
families (torque/injection curves specific to the 1.9/2.4 JTD calibration)
report **consistently 0** on all 10 — genuinely a different calibration
layout, not a detector gap. Two of the newer, platform-wide families (see
method item 8) are a real exception: they decode at 0.99 confidence on the
6 Ducato files with a newer HW number (0281015xxx-0281016xxx), and are
absent on the 4 older ones (0281012xxx-0281014xxx) — a real split, not
noise, and the safety contract still holds (a match reported is a match
present, never assumed). Every other model above has at least one real file
detecting maps; a few individual builds (see "Known limits") detect 0 or
very few, which on inspection reflects that specific software revision's
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
8. A second, independent verification pass ("check once more that every
   real ECM Titanium DRT and damos source is actually integrated") found
   this port had only ever used HALF of ECM Titanium's own DRT data: the
   `named_maps` variant (Bosch-symbol-labelled, 222 EDC16C39 builds). Its
   OTHER variant, `axis_table` (a categorical English name per driver
   entry rather than a Bosch symbol, 361 EDC16C39 builds), was mined the
   same way and cross-checked against every family already covered.
   9 candidate labels stood out; on inspection most of their volume
   turned out to already be covered families surfacing under one of ECM's
   several generic category names for the same real slot (informative,
   not new). A rigorous run-based filter — keep only a stride-consistent,
   independently multi-file-supported run, exactly the same bar
   `InjCrv_Bas1..5` had to clear — separated real new families from
   addresses that only matched via this platform's very common shared
   16x16 RPM/generic breakpoint grid (the exact "shared by half the
   file" risk this module's own key-matching code already warns about).
   One candidate initially accepted this way (a 4-member "secondary"
   driver-wish run) turned out, on cross-check against the reference
   database's OWN declared address for that exact build, to be the
   EXISTING `AccPed_trq*` family at its real Fiat Grande Punto/Punto
   address (+0xFC from the Alfa address) — not a new family at all; the
   candidate was withdrawn rather than shipped as a mislabel, and this is
   exactly the kind of self-correction the review pass above exists to
   catch. **15 real new families survived**: `IA_FuelAccel1..8` (an
   8-member run) and `BS_TurboPressureCorr1..2` (2-member) also decode on
   6 of the 10 real Ducato files (the newer HW generation) — a real,
   platform-wide overlap, not a detector fault — and `I3_InjCrvCorr1..5`
   (5-member), a second real injection-timing-correction run distinct
   from `InjCrv_Bas1..5`. None of the three groups has a confirmed Bosch
   symbol (the axis_table variant does not carry one), so they are marked
   `symbol_confirmed: false` and the app shows "no reference symbol"
   instead of a fabricated Bosch label.
9. A last-mile check for a real damos (WinOLS) file for this family, or
   any local/FTP one covering it, found none: every damos file reachable
   (local ECM Titanium install, FTP archives) either targets a different
   chip (EDC15C, confirmed by reading its own `{EDC15C}` header) or could
   not be listed without downloading a multi-gigabyte archive whose own
   directory contents gave no EDC16C39-looking file names. **Every one of
   this family's 49 templates traces back to a real ECM Titanium DRT
   record** — 30 to the Bosch-symbol `named_maps` variant, 19 to the
   categorical `axis_table` variant — none to a damos.
10. Two external oracles then measured what was still missing, instead of
   assuming the 21 families were "the" set:
   - **What real tuners edit.** Every one of the 47 corpus WinOLS projects
     saves 2+ ROM versions (58 original -> tuned steps: Stage 1/2, EGR/DPF
     off, vmax...). Each modified byte range was located inside a detected
     map or, failing that, inside the self-describing record that really
     encloses it. Before this pass only 38% of the tuner-modified
     calibration bytes fell inside a detected map; the uncovered records
     were real 1D curves (engine and per-gear torque limiters, EGR
     hysteresis), a start-torque map, the flat boost limiter, and driver
     wish maps on a 13x10 build.
   - **The reference database on the exact same builds.** Its build code is
     the last 3 digits of the Bosch hardware number and the last 4 of the
     software number; 12 corpus files have their exact build in it (32
     named entries each), which turns it into per-file ground truth.
   Added from that evidence: a curve record reader (`[n][X n][Z n]`, which
   all of the reference database's 1D entries decode as on those builds);
   `EngPrt_trqLim`, `TrqMaxGear1..6/R`, `TrqStrtBas` (nine declared grids),
   `PCR_DesMax` (a headerless scalar, read only at +0x244 from the
   `PCR_DesMaxAP` record, the offset on 220 of 222 reference builds);
   declared alternate grids for the driver wish family and `PCR_DesMaxAP`;
   flat data accepted only where real files show the family flat; and four
   EGR hysteresis curves that no reference names (identified by their
   high/low pair structure and by real edits: every EGR-off tune in the
   corpus sets them to 0, a Stage 1 tune scales them with injected
   quantity). Runs whose records are shorter than two address windows
   (gears 0x3E, hysteresis 0x66) are only ever resolved as a whole: for
   such a stride any build shift puts a sibling inside a member's window,
   and two real shifts do exactly that (+0xC0 gear runs in the reference
   database, +0xFC hysteresis on Grande Punto/Punto).

## What this PR adds

- `src-tauri/src/detector/ecu/bosch/edc16c39/{mod.rs,signatures.rs}`: 35
  calibrated families -- 22 maps (driver wish x4, air mass, EGR duty,
  smoke limiter x2, torque-to-quantity, injection timing x5, injector
  duration, turbo duty, boost target, boost limiter, rail pressure x3,
  cranking torque), 12 curves (engine torque limiter, gearbox torque
  limiter x7, EGR hysteresis x4) and the single-value boost limiter.
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

## Bench result (42 real 2MB files, `edc16c39_real_check`, of 49 families)

| Model | Files | Maps found |
|---|---|---|
| Alfa 159 | 3 | 35-40 |
| Alfa 147 (incl. a Ducati Corse 170hp build) | 3 | 40 on all 3 |
| Alfa GT | 1 | 40 |
| Alfa Brera | 2 | 35 on both |
| Fiat Croma | 3 | 35-40 |
| Fiat Doblo | 2 | 40 on both |
| Fiat Grande Punto | 6 | 38-40 |
| Fiat Punto | 2 | 40 on both |
| Fiat Bravo | 3 | 0, 5, 40 -- see "Known limits" |
| Other unnamed real Alfa/Fiat builds | 7 | 0, 0, 1, 7, 35, 35, 40 |
| Fiat Ducato | 10 | **0 of the 34 passenger families on all 10**; 10 of the 15 platform-wide families on 6 of 10 (see Corpus) |

1080 maps over the 42 files (920 before this pass; 526 before the review
fixes). No file reports the same map name twice.

- **Reference ground truth (12 files whose exact build the reference
  database lists):** 347 of its named entries detected at exactly the
  declared address, 0 missed. On one further file (Alfa 159 2.4, SW
  377554) the reference's own entries for that build code do not describe
  the file -- only 6 of its 30 structural entries even decode there, all 6
  detected at exactly that address -- while all 34 of this detector's
  own-named records there decode, at the addresses the reference gives
  for the sibling 159/Brera 2.4 build. The one reference entry never
  reported is `SpdLimMax` (see Known limits).
- **Tuned files:** the 43 tuned ROM versions saved in the corpus projects
  report 1137 maps; no family detected on an original is lost on any of
  its tuned versions (including EGR-off steps that zero the hysteresis
  curves).

## Known limits (read before trusting this beyond this corpus)

- **19 of the 49 families have no confirmed Bosch symbol** (`EgrHys1..4`,
  `IA_FuelAccel1..8`, `I3_InjCrvCorr1..5`, `BS_TurboPressureCorr1..2`):
  ECM Titanium's `axis_table` DRT variant names them with a categorical
  English description, not the per-chip Bosch internal symbol its
  `named_maps` variant carries for the other 30. `symbol_confirmed: false`
  on these, and the app shows "no reference symbol" in their description
  rather than inventing one.
- **Real tunes still edit calibrations this detector does not report,
  because no reference names them** (edited on N corpus files): an 8x8
  engine speed x temperature map (16), an 8x8 engine speed x rail pressure
  map (13), a 10x10 temperature x quantity map (9), further isolated maps
  scattered through the boost/rail/injection regions on a widely-shared
  RPM breakpoint grid this pass could not safely attribute to one real
  table (see method item 8), and on Grande Punto/Punto a fixed-capacity
  pedal table around 0x1C2040 (13) that uses neither record layout.
  Naming them would be guessing; they are listed here instead.
- **`SpdLimMax` (vehicle speed limit) is not read.** It is a headerless
  scalar, and unlike `PCR_DesMax` no family sits at a stable offset from it
  (best candidate: 68% of reference builds), so reading it would mean
  trusting a raw address.
- **Fiat Bravo is the weakest-understood model**: 3 real 2MB files, 0/5/40
  maps. The 0-map file (1.6 MultiJet 120cv) carries several families tens of
  KB away from every other build's address, outside the window this
  detector trusts, so it reports nothing rather than a guess. Three of the
  seven unnamed Alfa/Fiat builds (0, 1 and 7 maps) look like the same
  situation.
- **CONTRIBUTING.md's bar is 50 real files across software versions and
  engines**; this corpus (42 usable 2MB dumps) is close on raw count but
  still concentrated on 1.9/2.4 JTD/JTDm engines. Giulietta and Mito only
  exist here as 256KB partial extracts -- still UNCHECKED.
- **No completeness/invariant report yet** -- needs the "always N of X" pass
  CONTRIBUTING.md describes, written from real per-model statistics.
- **EGR hysteresis display**: the four curves use the same name and field
  convention as the VAG EDC16 "EGR hysteresis" curves, so the app shows them
  exactly like those (the name-based EGR layout draws a 25x1 column); the
  cell index used for reading and writing is correct in both orientations.
- **No mappack export, no checksum correction, no "solution"** -- matches
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
templates; a named reference (damos/A2L) for the tuner-edited families
still unreported; a bench table like the one above run against that full corpus;
and real invariants ("this ECU always carries N of X") written from that
corpus, not asserted from a spot-check.
