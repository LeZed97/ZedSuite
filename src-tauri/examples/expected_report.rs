//! Rapport de complétude d'un binaire, tel que l'éditeur l'affiche
//! (familles attendues vs trouvées, via `commands::detect_maps`).
//!
//! Usage : cargo run --release --example expected_report -- <fichier.bin> [ecu_type]
use std::env;
use std::fs;

use base64::Engine;
use zedsuite_lib::commands::{detect_maps, DetectMapsArgs};

fn main() {
    let _ = env_logger::try_init();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: expected_report <fichier.bin> [ecu_type]");
        std::process::exit(1);
    }
    let path = &args[1];
    let data = fs::read(path).expect("lecture du fichier");
    let ident = zedsuite_lib::detector::ECUIdentifier::identify(&data);
    let ecu_type = Some(
        args.get(2)
            .cloned()
            .unwrap_or_else(|| format!("{:?}", ident.ecu_type)),
    );
    let request = DetectMapsArgs {
        file_data_base64: base64::engine::general_purpose::STANDARD.encode(&data),
        file_name: path.clone(),
        ecu_type: ecu_type.clone(),
        tuned_mode: false,
    };
    let response = detect_maps(request).expect("detection");
    println!(
        "fichier: {} | ecu: {} | maps: {}",
        path,
        ecu_type.unwrap_or_else(|| "?".to_string()),
        response.total_maps
    );
    let Some(report) = response.expected_maps else {
        println!("(pas de rapport de complétude pour ce type d'ECU)");
        return;
    };
    let mut expected_total = 0usize;
    let mut found_total = 0usize;
    for status in &report {
        expected_total += status.expected;
        found_total += status.found.min(status.expected);
        let flag = if status.found >= status.expected { "  " } else { "!!" };
        println!(
            "{} {:60} {:>2} / {:<2}",
            flag, status.label, status.found, status.expected
        );
    }
    let pct = if expected_total > 0 {
        100.0 * found_total as f64 / expected_total as f64
    } else {
        0.0
    };
    println!("TOTAL {} / {} ({:.0} %)", found_total, expected_total, pct);
}
