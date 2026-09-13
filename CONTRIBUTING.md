# Contributing to ZedSuite

Thanks for your interest. Contributions are welcome, and **support for new ECU families** is the one I want the most. This document explains how the detection engine is organized, what it takes to add a family, and the bar a detector has to meet before it ships in the app. Please read the bar before you start: it is what decides whether a contribution can be integrated, and knowing it up front saves both of us a lot of time.

A word on how this works. I maintain ZedSuite on my free time and I will not build new families myself (the README says why). I am happy to review and integrate a pull request that follows the standards below, and to answer questions along the way. What I cannot do is finish a contribution: if the bench, the invariants or the axes are not done, the work is not half done, it is not done, because a detector that is wrong on some files misleads the people who trust the map list. A clean contribution that covers less is always preferred to a wide one that is not verified.

## What the app promises

ZedSuite is meant to stay simple: you open a dump, you get a short list of maps with a clear name, the right axes and the right units, and you can trust it. The whole value of the tool is in that trust.

A false positive, or a real map with a wrong axis or a wrong factor, does the exact opposite of what the app promises. The user edits a "boost map" that is actually a diagnostic table, or reads mbar where the axis is mg, flashes the file, and the app is the one that lied. One experience like that and the tool gets the reputation of being unreliable, which is worse than not supporting the family at all. So the rule is: **fewer maps that are right rather than more maps that are almost right**. Every family in the app is held to that, and the detectors are the part of the project that gets the most rework: after months on the detection algorithms, hundreds of files on the bench and seven releases, small bugs still turn up here and there. A contribution that does not reach the bar is not rejected out of strictness, it just cannot be exposed to users yet.

## Repository layout

```
src/                     Next.js frontend (TypeScript/React)
  lib/map-cell-layout.ts row/column layout and axis labels shared by the map viewer and the mappack export
  lib/mappack-export.ts  WinOLS mappack export (reads the same detection fields as the viewer)
src-tauri/src/           Rust desktop shell + detection engine
  commands.rs            IPC commands (identify_ecu, detect_maps, list_ecus, save_binary_file),
                         DETECTOR_VERSION, completeness reports (build_expected_report_*)
  models.rs              DetectedMap and shared types
  detector/
    ecu_identifier.rs    identifies WHICH ECU a binary is (strict, evidence-based)
    smart_detector.rs    routes a binary to the right family detector
    ecu/
      bosch/             one folder per family: edc15p/, edc15vm/, edc16u1/, edc16u31/, edc16u34/
src-tauri/examples/      bench tools: dump_maps, expected_report, identify_dir
src-tauri/ecus.json      the families declared to the frontend (ECU list page, status, sizes)
```

## How detection works

1. The user drops a binary. The frontend calls the `identify_ecu` command.
2. `ecu_identifier.rs` inspects the bytes and returns a manufacturer + `ECUType` + confidence. **Identification must be evidence-based**: hardware number strings, family markers, structural signatures at known offsets. File size alone is never enough. The app refuses files it cannot positively identify, which is what protects users from opening (and corrupting) a dump from an unsupported ECU.
3. On project creation the frontend calls `detect_maps`. `smart_detector.rs` routes to the family detector, which returns a list of `DetectedMap` (address, dimensions, axes, correction factors, category, description). `commands.rs` then builds the completeness report shown in the editor from a list of maps the family always carries.
4. `DETECTOR_VERSION` in `commands.rs` is bumped whenever the output of a detector changes, so that existing projects re-detect at the next opening.

## How the existing detectors were built

Every family in the app went through the same loop, and a new family is expected to go through it too.

1. **Locate the maps by hand** on real dumps with the references available: WinOLS packs, damos/A2L when there is one, the map lists of the reference tools.
2. **Extract the structural signatures** that identify each family: dimension headers, axis identifiers and axis layouts, selector blocks, spacing between maps. A map is found by its structure, never by a fixed address. Axis keys copied from one file only match that software.
3. **Run the detector on a bench of real files, stock AND tuned**, and compare its output file by file with the references until they match. Tuned files matter: the detector must behave on maps that have been edited, cleared or moved by a tune, not only on factory calibrations.
4. **Chase the false positives**, category by category, until the output is quiet.
5. **Write the invariants** of the family into the completeness report: what this ECU always carries, from knowledge of the ECU, not from statistics on the corpus.

A detector is not done when it finds the maps on the file it was written on. It is done when it behaves on files it has never seen.

For scale: the VAG EDC15 and EDC16 families were built on more than 350 dumps, mostly stock, with a share of tuned files, some modified on purpose and some at random. On EDC15P the completeness check is 35 rules per codeblock written from tuner knowledge: one EGR map, one driver wish plus two start-quantity maps, one torque limiter, the six injector durations 00 to 05 always with their selector, ten SOI maps, and so on. On EDC16 the invariants come from experience with these engines: every VAG EDC16 has exactly six main durations with their selector, the gearbox torque limiter slot, smoke limiters by MAF, MAP and lambda, a driver wish of 12 to 16 rpm points by 8 or 9 pedal points with the ninth being kickdown. Those rules are what let the app say "this file has been modified" instead of counting whatever it found. A Marelli MJD6 detector exists that matches its reference lists on every file I have, and it is still not in the app: it has not been run on enough stock dumps yet, and being at 100 % on the reference was not enough to expose it to users.

## The bar for a new family

This is what gets a new detector merged. It is the same for everyone, including me.

1. **A corpus.** In the range of 50 files minimum, stock and tuned, across the software versions and the engines the ECU is fitted to, power levels and years. The app works **per ECU type, not per engine**: a VAG EDC16 serves a 1.9, a 2.0 and a 2.5 TDI, and a detector has to behave on every engine its ECU was fitted to. If dumps of the family are rare and you get to 30 real files with the different engines represented, open the discussion anyway.
2. **A bench script.** Run the detector on every file, dump the map list per file, and diff it against a reference (WinOLS pack, A2L, damos, or your own verified annotations). The result I need to see is a table: N files, N at the expected map list, and the differences explained one by one. `cargo run --release --example dump_maps -- <file>` and `cargo run --release --example expected_report -- <file>` are the tools the existing families are benched with; reuse them.
3. **Invariants from the ECU, not from the corpus.** Once you have seen twenty files of the family you know what is always there: how many rail pressure maps, how many boost limiters, whether the smoke limiter always comes with its thermal twin, what grid the driver wish always has. Write those into the completeness report; that is what turns the badge into something users can trust. "At least one of each" is not an invariant.
4. **Zero regression on the existing bench.** The output of every existing family must be byte-identical before and after your change (`dump_maps` on the bench, diffed). Your identification gate must also reject foreign files of the same size (other manufacturers' 512 KB, 1 MB and 2 MB dumps), especially if it runs before the existing logic.
5. **Right axes and units on every map**, verified against the file, not only against the reference names. See the next section.
6. **No "solutions"** (DPF off, EGR off, one-click tunes). Those are a decision on my side and are not integrated for now, whatever the quality of the code. Keep them in your fork.

Until the bar is met, the family stays out of the released app and the pull request stays open as a draft. Integration is then done on my side: master keeps one commit per release, so there is no direct merge button here. Your commits are credited as co-authors of the release commit, and your name goes in CONTRIBUTORS.md. Your name goes in the release notes and the README, and I will ask you to stay the reference person for reports on that family if I cannot test it myself.

## Axes, units and factors

Every `DetectedMap` field is displayed to the user and exported as-is to WinOLS, so a wrong field is a wrong map. The conventions:

- `address`, `dimensions` (`rows` × `cols`), `data_type`: the data as stored in the file, row-major, `cols` values per row. If the family stores some maps transposed, say so in `src/lib/map-cell-layout.ts` (`shouldSwapAxes`), never by shuffling the dimensions.
- `x_axis_address` / `y_axis_address`: where the axis values are, `cols` values for X and `rows` values for Y. Leave them empty when the axis has no values in the file (selectors, curves indexed 1..N): the app then shows "." instead of inventing an axis.
- `x_axis_correction` / `x_axis_offset` (same for Y): display value = raw × correction + offset. **The corrections belong to the address they sit next to.** Do not emit an rpm axis with the factor of an IQ axis and rely on the viewer to swap them.
- `correction_factor` / `offset` and `unit`: same rule for the cell values.
- `x_label` / `y_label`: the axis label as the user should read it, in the form `Quantity (unit)`, for example `Engine speed (rpm)` or `IQ (mg/st)`. Without them the app and the export fall back to the description, which must then contain `X: Quantity (unit) | Y: Quantity (unit)`. No label at all is acceptable; a default label ("Load", "mbar") that does not come from the file is not.
- `description`: what the map does, plus the axis part above. Never embed the current value of the map in it (it is stale as soon as the user edits).
- `name`: stable, unique per codeblock, without addresses. Suffixes like `[0x...]` are only for maps the detector could not tell apart, and they are a sign that the rule is not finished.

Check the axes on the bench, not by reading the code: read the axis values at the exported addresses with the exported factors and make sure they are monotonic and plausible for the unit (rpm in 0..8000, IQ in 0..200 mg/st, temperatures in -60..1000 °C, pressures in 0..4500 mbar). A map that displays right in the app but exports wrong in the mappack, or the reverse, is a bug.

## Adding a new ECU family

Say you want to add Siemens PPD1.x:

1. **Create the detector module**
   - `src-tauri/src/detector/ecu/siemens/mod.rs` (`pub mod ppd1;`)
   - `src-tauri/src/detector/ecu/siemens/ppd1/mod.rs` with a `PPD1Detector` exposing `pub fn detect(&self, data: &[u8]) -> Vec<DetectedMap>`
   - Declare the manufacturer in `src-tauri/src/detector/ecu/mod.rs` (`pub mod siemens;`)
   - Look at `bosch/edc16u34/` for a complete example of signature-based detection, and `bosch/edc15p/` for pattern/codeblock-based detection. Keep the module self-contained: no change to the existing families' behaviour.

2. **Teach the identifier**
   - Add the `ECUType` variant(s) in `ecu_identifier.rs`
   - Add an identification method with **positive evidence only** (family strings, part-number formats, structural markers at fixed offsets). Add the family's dump size(s) to `SUPPORTED_SIZES`.
   - Add unit tests, including a negative test proving that a random file of the same size is NOT identified as your ECU (see `test_foreign_2mb_file_is_not_edc16`), and one proving the existing families are still identified as before.

3. **Route it**
   - `smart_detector.rs`: add a match arm routing your `ECUType` to your detector.
   - `detector/mod.rs`: add the string form to `parse_ecu_type` so stored projects re-detect correctly.
   - `commands.rs`: add a `build_expected_report_<family>` with the invariants of the family, and bump `DETECTOR_VERSION`.

4. **Declare it to the frontend**
   - `src-tauri/ecus.json`: add an entry (id, name, family, file sizes, vehicles, map categories). Set `"status": "beta"` until the bar is met.
   - `src/lib/local/detector.ts`: add the type to `SUPPORTED_ECUS`
   - If the family is big-endian, check `src/lib/ecu-endianness.ts`
   - The mappack export and the power estimate work from the `DetectedMap` fields; there is nothing to add there if the fields are right.

5. **Validate**
   - `cd src-tauri && cargo test`, then `npx tsc --noEmit` if you touched the frontend.
   - Run the bench script of point 2 of the bar on your corpus, and `dump_maps` on the existing bench before and after your change.
   - Run the app (`npm run app:dev`), import real dumps of your family, plus dumps of OTHER ECUs with the same file size to confirm they are rejected, and open every map once: axes, units, values.

6. **Open the pull request** with: the bench table (files, expected list, differences explained), the corpus description (software versions, engines, stock/tuned split), the invariants you wrote and where they come from, and a note on what the detector does NOT cover yet. A pull request without a bench table cannot be evaluated.

## Ground rules

- `cargo test` must pass; add tests for anything you fix or add.
- Never commit ECU dumps, mappacks or reference files: `.bin` files are gitignored on purpose, and the references you used are yours.
- Frontend code is TypeScript; comments in the Rust engine are English.
- Keep identification strict. A false "unsupported file" is annoying; a false "this is an EDC16" corrupts someone's ECU. When in doubt, return `Unknown`.
- Do not bump the version numbers (`package.json`, `Cargo.toml`, `tauri.conf.json`): the version lives in the release.
- Keep the scope of a pull request to one family or one fix. A detector, a solution and a UI change in the same PR cannot be evaluated separately.

## Local data

Projects live in `%APPDATA%/com.zedsuite.app/projects/<id>/` on Windows, `~/Library/Application Support/com.zedperf.zedsuite/projects/<id>/` on macOS and `~/.local/share/com.zedsuite.app/projects/<id>/` on Linux:
- `project.json` — metadata + detection results
- `original.bin` — the imported binary, untouched
- `versions.json`, `edits-<versionId>.json` — versioning data

Deleting a project folder removes it from the app.

## Talking about it

If you are considering a family, open an issue or a draft pull request early with what you have (corpus size, references, what the ECU is fitted to). It is much easier to agree on the invariants and the bench before the code exists than after. Links to reach me are in the README.
