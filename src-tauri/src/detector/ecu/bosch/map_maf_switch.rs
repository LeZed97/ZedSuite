//! Switch MAP/MAF des EDC16 (U1, U31, U34).
//!
//! L'EDC15 porte un switch qui dit quel capteur alimente la limitation de
//! fumée ; l'EDC16 a le sien, documenté dans les définitions WinOLS de la
//! famille sous le nom « MAF/MAP switch (0 MAF, 1 MAP) ». C'est un OCTET
//! non signé, facteur 1, entouré de deux switchs voisins :
//!
//! ```text
//!   01 00 02 00 01 00 [switch] 0a dd 00
//!   ^^                ^^^^^^^^
//!   |                 le switch MAP/MAF
//!   le switch « pression de suralimentation (0) / pression atmo (1) »
//!   pour la limitation de fumée, six octets avant ; le switch
//!   « lambda (0) / cartes de fumée (1) » est deux octets avant.
//! ```
//!
//! Relevé sur les 317 fichiers du banc EDC16 : le motif est présent sur les
//! 184 U34 (dont 22 avec un exemplaire par bloc de calibration), sur 43 des
//! 116 U1 et sur 2 des 17 U31 — les autres logiciels U1/U31 rangent ces
//! octets autrement, le switch lambda pouvant même être ailleurs dans le
//! fichier. On ne détecte donc QUE cette disposition, quitte à ne rien
//! trouver sur les variantes, plutôt que de nommer un octet au hasard.
//! Demandé par georgiminchev04 sur un 03G906021KH (issue #24).

use std::collections::HashSet;

use crate::models::{DataType, DetectedMap, MapDimensions};

/// Longueur du motif, switch compris.
const PATTERN_LEN: usize = 10;
/// Position du switch dans le motif.
const SWITCH_OFFSET: usize = 6;

/// Ajoute le switch MAP/MAF pour chaque exemplaire trouvé (un par bloc de
/// calibration sur les fichiers qui en ont deux).
pub fn detect_map_maf_switch(
    data: &[u8],
    maps: &mut Vec<DetectedMap>,
    detected_addresses: &mut HashSet<u32>,
) {
    if data.len() < PATTERN_LEN {
        return;
    }

    for i in 0..=data.len() - PATTERN_LEN {
        // Les six octets d'en-tête, puis l'octet du switch (0 ou 1), puis la
        // queue 0A DD 00 : dix octets dont un seul varie.
        if data[i] != 0x01
            || data[i + 1] != 0x00
            || data[i + 2] != 0x02
            || data[i + 3] != 0x00
            || data[i + 4] != 0x01
            || data[i + 5] != 0x00
        {
            continue;
        }
        let value = data[i + SWITCH_OFFSET];
        if value > 1 {
            continue;
        }
        if data[i + 7] != 0x0A || data[i + 8] != 0xDD || data[i + 9] != 0x00 {
            continue;
        }

        let addr = (i + SWITCH_OFFSET) as u32;
        if detected_addresses.contains(&addr) || maps.iter().any(|m| m.address == addr) {
            continue;
        }

        let mut map = DetectedMap::new(
            addr,
            1,
            MapDimensions::TwoDimensional { rows: 1, cols: 1 },
            DataType::UInt8,
        );
        map.name = Some("MAP/MAF switch".to_string());
        map.category = Some("Smoke limitation".to_string());
        map.description =
            Some("Sensor used by the smoke limitation: 0 = MAF, 1 = MAP".to_string());
        map.unit = Some("-".to_string());
        map.correction_factor = Some(1.0);
        map.offset = Some(0.0);
        map.confidence = 0.95;

        log::debug!(
            "🎯 [EDC16] MAP/MAF switch at 0x{:X} = {} ({})",
            addr,
            value,
            if value == 0 { "MAF" } else { "MAP" }
        );

        detected_addresses.insert(addr);
        maps.push(map);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_with(pattern: &[u8], at: usize) -> Vec<u8> {
        let mut data = vec![0x55u8; at + pattern.len() + 64];
        data[at..at + pattern.len()].copy_from_slice(pattern);
        data
    }

    #[test]
    fn finds_the_switch_set_to_map() {
        let data = file_with(&[0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x01, 0x0A, 0xDD, 0x00], 0x400);
        let mut maps = Vec::new();
        let mut seen = HashSet::new();
        detect_map_maf_switch(&data, &mut maps, &mut seen);
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].address, 0x406);
        assert_eq!(maps[0].size, 1);
    }

    #[test]
    fn finds_the_switch_set_to_maf() {
        let data = file_with(&[0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x00, 0x0A, 0xDD, 0x00], 0x100);
        let mut maps = Vec::new();
        let mut seen = HashSet::new();
        detect_map_maf_switch(&data, &mut maps, &mut seen);
        assert_eq!(maps.len(), 1);
        assert_eq!(maps[0].address, 0x106);
    }

    #[test]
    fn ignores_a_broken_tail() {
        let data = file_with(&[0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x01, 0x0A, 0xDE, 0x00], 0x200);
        let mut maps = Vec::new();
        let mut seen = HashSet::new();
        detect_map_maf_switch(&data, &mut maps, &mut seen);
        assert!(maps.is_empty());
    }

    #[test]
    fn ignores_a_value_that_is_not_a_switch() {
        let data = file_with(&[0x01, 0x00, 0x02, 0x00, 0x01, 0x00, 0x07, 0x0A, 0xDD, 0x00], 0x200);
        let mut maps = Vec::new();
        let mut seen = HashSet::new();
        detect_map_maf_switch(&data, &mut maps, &mut seen);
        assert!(maps.is_empty());
    }
}
