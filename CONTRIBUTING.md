# Contributing to ZedSuite

Thanks for your interest! The most valuable contribution is **support for new ECU families**. This document walks through how the detection engine is organized and what it takes to add one.

## Repository layout

```
src/                     Next.js frontend (TypeScript/React)
src-tauri/src/           Rust desktop shell + detection engine
  commands.rs            IPC commands (identify_ecu, detect_maps, list_ecus, save_binary_file)
  models.rs              DetectedMap and shared types
  detector/
    ecu_identifier.rs    identifies WHICH ECU a binary is (strict, evidence-based)
    smart_detector.rs    routes a binary to the right family detector
    ecu/
      bosch/             one folder per family: edc15p/, edc15vm/, edc16u1/, edc16u31/, edc16u34/
```

## How detection works

1. The user drops a binary. The frontend calls the `identify_ecu` command.
2. `ecu_identifier.rs` inspects the bytes and returns a manufacturer + `ECUType` + confidence. **Identification must be evidence-based**: hardware number strings, family markers, structural signatures at known offsets. File size alone is never enough — the app refuses files it cannot positively identify, which is what protects users from opening (and corrupting) a dump from an unsupported ECU.
3. On project creation the frontend calls `detect_maps`. `smart_detector.rs` routes to the family detector, which returns a list of `DetectedMap` (address, dimensions, axes, correction factors, category…).

## Adding a new ECU family

Say you want to add Siemens PPD1.x:

1. **Create the detector module**
   - `src-tauri/src/detector/ecu/siemens/mod.rs` (`pub mod ppd1;`)
   - `src-tauri/src/detector/ecu/siemens/ppd1/mod.rs` with a `PPD1Detector` exposing `pub fn detect(&self, data: &[u8]) -> Vec<DetectedMap>`
   - Declare the manufacturer in `src-tauri/src/detector/ecu/mod.rs` (`pub mod siemens;`)
   - Look at `bosch/edc16u34/` for a complete example of signature-based detection, and `bosch/edc15p/` for pattern/codeblock-based detection.

2. **Teach the identifier**
   - Add the `ECUType` variant(s) in `ecu_identifier.rs`
   - Add an identification method with **positive evidence only** (family strings, part-number formats, structural markers at fixed offsets). Add the family's dump size(s) to `SUPPORTED_SIZES`.
   - Add unit tests, including a negative test proving that a random file of the same size is NOT identified as your ECU (see `test_foreign_2mb_file_is_not_edc16`).

3. **Route it**
   - `smart_detector.rs`: add a match arm routing your `ECUType` to your detector.
   - `detector/mod.rs`: add the string form to `parse_ecu_type` so stored projects re-detect correctly.

4. **Declare it to the frontend**
   - `src-tauri/ecus.json`: add an entry (id, name, file sizes, vehicles…)
   - `src/lib/local/detector.ts`: add the type to `SUPPORTED_ECUS`
   - `src/lib/ecu-database.ts`: add the entries shown on the ECU list page
   - If the family is big-endian, check `src/lib/ecu-endianness.ts`

5. **Validate**
   - `cd src-tauri && cargo test`
   - Run the app (`npm run app:dev`) and import real dumps of your family, plus dumps of OTHER ECUs with the same file size to confirm they are rejected.

## Ground rules

- `cargo test` must pass; add tests for anything you fix or add.
- Never commit ECU dumps — `.bin` files are gitignored on purpose.
- Frontend code is TypeScript, comments in the Rust engine are English.
- Keep identification strict. A false "unsupported file" is annoying; a false "this is an EDC16" corrupts someone's ECU. When in doubt, return `Unknown`.

## Local data

Projects live in `%APPDATA%/com.zedsuite.app/projects/<id>/`:
- `project.json` — metadata + detection results
- `original.bin` — the imported binary, untouched
- `versions.json`, `edits-<versionId>.json` — versioning data

Deleting a project folder removes it from the app.
