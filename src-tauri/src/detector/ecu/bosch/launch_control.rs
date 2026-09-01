//! Détection du Launch Control (EDC15P et EDC15VM).
//!
//! La cartographie de launch control (25×14, 700 octets) est présente dans le
//! binaire d'origine mais NEUTRALISÉE : son axe Y porte un descripteur
//! corrompu. La solution « Launch Control » de ZedSuite réécrit cet axe avec
//! des paliers de vitesse véhicule (0, 20, 40 … 260 km/h), ce qui active la
//! carte.
//!
//! Structure à partir de l'adresse de signature (« sig ») :
//!   sig+0  : 2 octets quelconques (joker dans la signature d'origine)
//!   sig+2  : en-tête d'axe Y (0x00 0x0E = 14 valeurs)
//!   sig+4  : 14 valeurs de vitesse (u16 LE)
//!   sig+36 : valeurs d'axe X (régime), décalées de 2 octets pour aligner les
//!            étiquettes sur les données — même convention que la version web
//!   sig+86 : données de la carte (25 × 14 × 2 = 700 octets)
//!
//! Désactivé, l'axe Y vaut `02 00 80 00 00 0A` … ; activé, il vaut
//! `00 0E 00 00 14 00 28 00 …`. On ne détecte donc la carte QUE lorsqu'elle
//! est réellement active — sinon elle n'a pas lieu d'être proposée à l'édition.

use std::collections::HashSet;

use crate::models::{DataType, DetectedMap, MapDimensions};

/// Paliers de vitesse écrits par la solution (km/h).
const SPEED_STEPS: [u16; 14] = [0, 20, 40, 60, 80, 100, 120, 140, 160, 180, 200, 220, 240, 260];

/// Décalages de la structure, relatifs à l'adresse de signature.
const Y_AXIS_OFFSET: usize = 4;
const X_AXIS_OFFSET: usize = 36;
const MAP_OFFSET: usize = 86;
const MAP_SIZE: usize = 700;

/// Vrai si l'axe Y de launch control est présent et activé à cette adresse.
///
/// Les deux octets qui précèdent l'en-tête sont des jokers (ils diffèrent d'un
/// fichier à l'autre : `FF FF` sur certains, `80 C2` sur d'autres) — seules
/// comptent l'en-tête `00 0E` et les 14 valeurs de vitesse, une suite assez
/// spécifique pour ne pas produire de faux positifs.
fn is_active_at(data: &[u8], sig: usize) -> bool {
    if sig + MAP_OFFSET + MAP_SIZE > data.len() {
        return false;
    }
    // Deux encodages d'en-tête rencontrés dans les fichiers réels :
    //   `00 0E` — écrit par la solution ZedSuite (convention de la web app) ;
    //   `0E 00` — little-endian canonique, fichiers livrés avec le launch
    //             control déjà actif (ex. multimap Benicio 038906019).
    // Les 14 paliers exacts 0→260 km/h vérifiés ensuite écartent tout faux
    // positif quel que soit l'en-tête accepté.
    let hdr = (data[sig + 2], data[sig + 3]);
    if hdr != (0x00, 0x0E) && hdr != (0x0E, 0x00) {
        return false;
    }
    SPEED_STEPS.iter().enumerate().all(|(i, &expected)| {
        let off = sig + Y_AXIS_OFFSET + i * 2;
        u16::from_le_bytes([data[off], data[off + 1]]) == expected
    })
}

/// Ajoute la carte « Launch control map » pour chaque instance ACTIVÉE.
///
/// Appelée par les détecteurs EDC15P et EDC15VM après le nommage : la carte
/// n'apparaît dans l'arbre que si l'utilisateur a appliqué la solution.
pub fn detect_launch_control(
    data: &[u8],
    maps: &mut Vec<DetectedMap>,
    detected_addresses: &mut HashSet<u32>,
) {
    if data.len() < MAP_OFFSET + MAP_SIZE {
        return;
    }

    for sig in 0..data.len().saturating_sub(MAP_OFFSET + MAP_SIZE) {
        if !is_active_at(data, sig) {
            continue;
        }

        let map_addr = (sig + MAP_OFFSET) as u32;

        // Le balayage générique peut avoir trouvé la même zone sous un autre
        // nom : on la remplace par la version complète et correctement décrite.
        maps.retain(|m| m.address != map_addr);

        let mut map = DetectedMap::new(
            map_addr,
            MAP_SIZE,
            MapDimensions::TwoDimensional { rows: 14, cols: 25 },
            DataType::UInt16,
        );
        map.name = Some("Launch control map".to_string());
        map.category = Some("Launch control".to_string());
        map.subcategory = Some("Launch control".to_string());
        map.correction_factor = Some(0.01);
        map.unit = Some("mg/st".to_string());
        map.x_axis_address = Some((sig + X_AXIS_OFFSET) as u32);
        map.y_axis_address = Some((sig + Y_AXIS_OFFSET) as u32);
        map.x_axis_correction = Some(1.0);
        map.y_axis_correction = Some(0.15625);
        map.x_label = Some("Engine speed (rpm)".to_string());
        map.y_label = Some("Vehicle speed (km/h)".to_string());
        map.y_axis_inverted = Some(true);
        map.description =
            Some("IQ limit | X: Engine speed (rpm) | Y: Vehicle speed (km/h)".to_string());
        map.confidence = 0.98;

        log::debug!(
            "🚀 Launch control ACTIF : signature 0x{:X}, carte 0x{:X}",
            sig,
            map_addr
        );

        detected_addresses.insert(map_addr);
        maps.push(map);
    }
}
