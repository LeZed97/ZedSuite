//! Dump des maps détectées pour un binaire, en JSON — sert à comparer la
//! détection avec une référence externe (WinOLS / Stage X).
//!
//! Usage : cargo run --example dump_maps -- <fichier.bin> [ecu_type]

use std::env;
use std::fs;

fn main() {
    let _ = env_logger::try_init();
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: dump_maps <fichier.bin> [ecu_type]");
        std::process::exit(1);
    }
    let path = &args[1];
    let forced = args.get(2).map(|s| s.as_str());

    let data = fs::read(path).expect("lecture du fichier");
    eprintln!("fichier: {} ({} octets)", path, data.len());

    let ident = zedsuite_lib::detector::ECUIdentifier::identify(&data);
    eprintln!(
        "identification: {:?} {:?} (confiance {:.0}%)",
        ident.manufacturer,
        ident.ecu_type,
        ident.confidence * 100.0
    );

    let detector = zedsuite_lib::detector::MapDetector::new();
    let maps = detector.detect_maps_with_options(&data, forced, false);
    eprintln!("maps détectées: {}", maps.len());

    let out: Vec<_> = maps
        .iter()
        .map(|m| {
            let (cols, rows) = match &m.dimensions {
                zedsuite_lib::models::MapDimensions::OneDimensional { length } => (*length, 1usize),
                zedsuite_lib::models::MapDimensions::TwoDimensional { rows, cols } => (*cols, *rows),
                zedsuite_lib::models::MapDimensions::ThreeDimensional { x, y, .. } => (*x, *y),
            };
            serde_json::json!({
                "name": m.name,
                "address": format!("{:X}", m.address),
                "address_dec": m.address,
                "cols": cols,
                "rows": rows,
                "size": m.size,
                "x_axis": m.x_axis_address.map(|a| format!("{:X}", a)),
                "y_axis": m.y_axis_address.map(|a| format!("{:X}", a)),
                "category": m.category,
                "codeblock": m.codeblock_id,
                "factor": m.correction_factor,
                "offset": m.offset,
                "unit": m.unit,
                "x_label": m.x_label,
                "y_label": m.y_label,
                "x_corr": m.x_axis_correction,
                "y_corr": m.y_axis_correction,
                "x_off": m.x_axis_offset,
                "y_off": m.y_axis_offset,
                "description": m.description,
            })
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}
