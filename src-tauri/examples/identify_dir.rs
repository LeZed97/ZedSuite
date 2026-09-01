// Dev tool: run the ECU identifier on every binary in a directory (recursive).
//   cargo run --example identify_dir -- <dir>
// Used to validate identification against a corpus of real dumps, including
// dumps of UNSUPPORTED ECUs (which must come back Unknown / not EDC1x).

use std::path::Path;

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

fn main() {
    let dir = std::env::args().nth(1).expect("usage: identify_dir <dir>");
    let mut files = Vec::new();
    walk(Path::new(&dir), &mut files);
    files.sort();

    for path in files {
        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let id = zedsuite_lib::detector::ECUIdentifier::identify(&data);
        println!(
            "{:60} {:8} KB  -> {:?} {:?} ({:.0}%)  hw={:?} sw={:?}",
            path.file_name().unwrap_or_default().to_string_lossy(),
            data.len() / 1024,
            id.manufacturer,
            id.ecu_type,
            id.confidence * 100.0,
            id.hardware_version.as_deref().unwrap_or("-"),
            id.software_version.as_deref().unwrap_or("-"),
        );
    }
}
