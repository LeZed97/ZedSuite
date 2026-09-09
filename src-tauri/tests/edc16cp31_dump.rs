//! Fixture test for the EDC16CP31 detector, run against a REAL dump.
//!
//! Dumps and damos files are never committed (licensing, and a .bin is not
//! ours to redistribute), so this test is `#[ignore]`d by default and reads
//! the file from an environment variable:
//!
//!   ZEDSUITE_CP31_DUMP=/path/to/original.bin \
//!     cargo test --test edc16cp31_dump -- --ignored --nocapture
//!
//! The expected addresses below are the ones confirmed on the reference
//! corpus (OM642 3.0 CDI 165 kW, Bosch SW 1037393817). A different software
//! build WILL have different addresses - in that case the test is expected
//! to fail on the addresses while still finding the same families, which is
//! exactly the signal you want when adding a second file to the corpus.

use zedsuite_lib::detector::ecu::bosch::EDC16CP31Detector;

fn dump() -> Option<Vec<u8>> {
    let p = std::env::var("ZEDSUITE_CP31_DUMP").ok()?;
    std::fs::read(p).ok()
}

#[test]
#[ignore = "needs ZEDSUITE_CP31_DUMP"]
fn finds_every_calibrated_family_on_the_reference_dump() {
    let Some(data) = dump() else {
        panic!("set ZEDSUITE_CP31_DUMP to a 2 MB EDC16CP31 read");
    };
    assert_eq!(data.len(), 0x200000, "expected a 2 MB dump");

    let maps = EDC16CP31Detector::new().detect(&data);
    for m in &maps {
        println!(
            "0x{:06X} {:<34} {:?} conf {:.2}  {}",
            m.address,
            m.name.clone().unwrap_or_default(),
            m.dimensions,
            m.confidence,
            m.description.clone().unwrap_or_default()
        );
    }

    // (family, block address, data address, rows = nx, cols = ny)
    let expected: &[(&str, u32, u32, usize, usize)] = &[
        ("Driver Wish", 0x1918DE, 0x191902, 8, 8),
        ("EGR air mass target", 0x194442, 0x19447E, 12, 16),
        ("Quantity Limiter by boost pressure", 0x1A7FE8, 0x1A801C, 12, 12),
        ("Smoke Limiter by boost pressure", 0x1A8986, 0x1A89CA, 16, 16),
        ("Torque to IQ Conversion", 0x1A9514, 0x1A955C, 16, 18),
        ("Start of injection", 0x1B07EC, 0x1B0830, 16, 16),
        ("VNT Duty Cycle", 0x1DB4F0, 0x1DB524, 16, 8),
        ("Boost Limiter", 0x1E3E36, 0x1E3E72, 16, 12),
        ("Boost Target", 0x1E4BCE, 0x1E4C0A, 16, 12),
        ("Rail Pressure Target", 0x1F1BF2, 0x1F1C36, 16, 16),
        ("Rail Pressure Limiter", 0x1F2964, 0x1F2990, 8, 12),
    ];

    for (name, block, data_addr, rows, cols) in expected {
        let hit = maps
            .iter()
            .find(|m| m.name.as_deref() == Some(name) && m.address == *data_addr)
            .unwrap_or_else(|| panic!("{name} not found at 0x{data_addr:06X}"));
        assert_eq!(hit.y_axis_address, Some(block + 4), "{name}: engine-speed axis");
        assert_eq!(
            hit.x_axis_address,
            Some(block + 4 + 2 * *rows as u32),
            "{name}: load axis"
        );
        assert_eq!(hit.size, rows * cols * 2, "{name}: data size");
    }

    // Every family must be represented, and the scan must not drown the
    // editor in candidates: the reference dump yields 25 blocks, all of them
    // real members of the eleven calibrated families.
    for (name, ..) in expected {
        assert!(maps.iter().any(|m| m.name.as_deref() == Some(name)), "{name} missing");
    }
    assert!(
        maps.len() <= 40,
        "{} maps detected - the zones are too loose",
        maps.len()
    );
    // No map may straddle the end of the checksummed region.
    for m in &maps {
        assert!(m.address >= 0x190000 && m.address + m.size as u32 <= 0x1FCFFC);
    }
}
