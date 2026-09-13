//! Manual validation/bench tool for `.ols` import, same convention as `dump_maps`/`expected_report`.
//! Inspects a `.ols` file and extracts every listed version to `<file>.vN.bin` next to it, so the
//! output can be byte-compared against a known-good extraction from another tool.
//!
//! Usage: cargo run --release --example ols_import_check -- <file.ols>

use zedsuite_lib::ols_import;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: ols_import_check <file.ols>");
        std::process::exit(2);
    });
    let data = std::fs::read(&path).expect("read input file");

    let Some(info) = ols_import::inspect(&data) else {
        println!("{path}: not recognised as a .ols container");
        return;
    };
    println!(
        "{path}: make={:?} model={:?} manufacturer={:?} ecu_name={:?} hw={:?} sw={:?}",
        info.make, info.model, info.manufacturer, info.ecu_name, info.hw_number, info.sw_number
    );
    for v in &info.versions {
        println!("  version {} ({} bytes) label={:?}", v.index, v.size, v.label);
        let bytes = ols_import::extract_version(&data, v.index).expect("extract");
        let out = format!("{path}.v{}.bin", v.index);
        std::fs::write(&out, &bytes).expect("write output");
        println!("    -> {out}");
    }
}
