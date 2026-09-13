//! Manual real-file check for the EDC16C39 detector, same convention as
//! `dump_maps`. Not part of the app.
//! Usage: cargo run --release --example edc16c39_real_check -- <file1.bin> [file2.bin ...]

use zedsuite_lib::detector::ecu::bosch::EDC16C39Detector;

fn main() {
    let detector = EDC16C39Detector::new_tuned();
    for path in std::env::args().skip(1) {
        let data = std::fs::read(&path).expect("read input file");
        let maps = detector.detect(&data);
        println!("=== {path} ({} bytes) -> {} map(s) ===", data.len(), maps.len());
        for m in &maps {
            println!(
                "  0x{:06X} {:>16} conf={:.2} {}",
                m.address,
                m.name.as_deref().unwrap_or("?"),
                m.confidence,
                m.description.as_deref().unwrap_or("")
            );
        }
    }
}
