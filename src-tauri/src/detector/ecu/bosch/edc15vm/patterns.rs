/// Valid axis ID check for EDC15VM ECUs
/// Based on the Bosch EDC15VM axis ID table
pub fn is_valid_axis_id(id_high: u8) -> bool {
    matches!(id_high,
        0xDB |
        0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC4 | 0xC5 |
        0xD0 | 0xD5 | 0xD7 | 0xD9 | 0xDA | 0xDC | 0xDD | 0xDE |
        0xE0 | 0xE4 | 0xE5 | 0xE6 | 0xE7 | 0xE8 | 0xE9 | 0xEA | 0xEB | 0xEC |
        0xEF | 0xF9 | 0xFE
    )
}

/// Valid axis length check for EDC15VM ECUs
/// Most axes allow 1-31, but 0xEB and 0xDC allow up to 32
pub fn is_valid_length(id_high: u8, length: usize) -> bool {
    if length == 0 {
        return false;
    }
    match id_high {
        0xEB | 0xDC => length <= 32,
        _ => length <= 31,
    }
}
