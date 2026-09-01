//! Détection ciblée de l'« EGR temperature map » (EDC15P et EDC15VM).
//!
//! Petite carte 5×6 (RPM × température) qui module l'EGR selon la température
//! moteur. Elle existe sur une partie des fichiers seulement, et le balayage
//! générique la rate quand ses données sont uniformes (ex. mise à zéro par un
//! stage) — d'où cette passe dédiée sur la structure des axes, insensible au
//! contenu de la carte (référence : EDCSuite, mappack NV2 Benicio).
//!
//! Structure à partir de l'adresse de signature (« sig ») :
//!   sig+0  : ID d'axe Y (octet fort 0xEC/0xEA/0xDA/0xC0 — famille RPM/mbar)
//!   sig+2  : nombre de valeurs Y = 5
//!   sig+4  : 5 régimes (u16 LE) croissants, premier ≤ 1500, dernier ≤ 6500
//!   sig+14 : ID d'axe X (octet fort 0xC1 — température)
//!   sig+16 : nombre de valeurs X = 6
//!   sig+18 : 6 températures (u16 LE, Kelvin ×10) croissantes, 1500..5000,
//!            écart ≥ 100 (mêmes gardes que le filtre EDC15P historique)
//!   sig+30 : données de la carte (5 × 6 × 2 = 60 octets) — contenu libre,
//!            une carte entièrement à zéro est légitime (EGR coupé)

use std::collections::HashSet;

use crate::models::{DataType, DetectedMap, MapDimensions};

const Y_COUNT: usize = 5;
const X_COUNT: usize = 6;
const Y_AXIS_OFFSET: usize = 4;
const X_AXIS_OFFSET: usize = 18;
const MAP_OFFSET: usize = 30;
const MAP_SIZE: usize = Y_COUNT * X_COUNT * 2;

fn u16_at(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

/// Vrai si la structure axes Y(5 RPM) + X(6 températures) est présente ici.
fn matches_at(data: &[u8], sig: usize) -> bool {
    if sig + MAP_OFFSET + MAP_SIZE > data.len() {
        return false;
    }

    // Axe Y : famille RPM/mbar, exactement 5 valeurs croissantes
    let y_hi = (u16_at(data, sig) >> 8) as u8;
    if !matches!(y_hi, 0xEC | 0xEA | 0xDA | 0xC0) {
        return false;
    }
    if u16_at(data, sig + 2) != Y_COUNT as u16 {
        return false;
    }
    let mut prev = 0u16;
    for i in 0..Y_COUNT {
        let v = u16_at(data, sig + Y_AXIS_OFFSET + i * 2);
        if i == 0 {
            if v > 1500 {
                return false;
            }
        } else if v <= prev {
            return false;
        }
        if v > 6500 {
            return false;
        }
        prev = v;
    }

    // Axe X : température (0xC1), exactement 6 valeurs croissantes plausibles
    let x_hi = (u16_at(data, sig + 14) >> 8) as u8;
    if x_hi != 0xC1 {
        return false;
    }
    if u16_at(data, sig + 16) != X_COUNT as u16 {
        return false;
    }
    let mut prev = 0u16;
    let mut min = u16::MAX;
    let mut max = 0u16;
    for i in 0..X_COUNT {
        let v = u16_at(data, sig + X_AXIS_OFFSET + i * 2);
        if i > 0 && v <= prev {
            return false;
        }
        prev = v;
        min = min.min(v);
        max = max.max(v);
    }
    if min < 1500 || max > 5000 || max - min < 100 {
        return false;
    }

    true
}

/// Ajoute une « EGR temperature map » pour chaque instance trouvée (une par
/// codeblock sur les multimap). Remplace une éventuelle carte du balayage
/// générique à la même adresse.
pub fn detect_egr_temperature(
    data: &[u8],
    maps: &mut Vec<DetectedMap>,
    detected_addresses: &mut HashSet<u32>,
) {
    if data.len() < MAP_OFFSET + MAP_SIZE {
        return;
    }

    for sig in 0..data.len() - (MAP_OFFSET + MAP_SIZE) {
        if !matches_at(data, sig) {
            continue;
        }

        let map_addr = (sig + MAP_OFFSET) as u32;
        maps.retain(|m| m.address != map_addr);

        let mut map = DetectedMap::new(
            map_addr,
            MAP_SIZE,
            MapDimensions::TwoDimensional { rows: Y_COUNT, cols: X_COUNT },
            DataType::UInt16,
        );
        map.name = Some("EGR temperature map".to_string());
        map.category = Some("EGR".to_string());
        map.subcategory = Some("EGR".to_string());
        map.correction_factor = Some(1.0);
        map.x_axis_address = Some((sig + X_AXIS_OFFSET) as u32);
        map.y_axis_address = Some((sig + Y_AXIS_OFFSET) as u32);
        // Température stockée en Kelvin ×10 → affichage en °C (×0.1 − 273)
        map.x_axis_correction = Some(0.1);
        map.x_axis_offset = Some(-273.0);
        map.y_axis_correction = Some(1.0);
        map.x_label = Some("Temperature (°C)".to_string());
        map.y_label = Some("Engine speed (rpm)".to_string());
        map.y_axis_inverted = Some(true);
        map.description = Some(
            "EGR compensation by temperature | X: Temperature (°C) | Y: Engine speed (rpm)"
                .to_string(),
        );
        map.confidence = 0.95;

        log::debug!(
            "🌡️ EGR temperature map : signature 0x{:X}, carte 0x{:X}",
            sig,
            map_addr
        );

        detected_addresses.insert(map_addr);
        maps.push(map);
    }
}
