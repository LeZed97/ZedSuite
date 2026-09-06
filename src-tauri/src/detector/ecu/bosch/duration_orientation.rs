//! Orientation des lignes du bloc des durées d'injection EDC16.
//!
//! Sur certains logiciels (U34 1037376704, ZZZZ ori/modtest du banc) les
//! données du bloc Duration sont rangées du régime le PLUS HAUT au plus bas,
//! alors que l'axe régime qui les précède est croissant comme partout
//! ailleurs — et que les autres maps du fichier (Driver Wish, SOI…) gardent
//! l'ordre habituel. Structurellement rien ne distingue ce bloc : mêmes
//! en-têtes, mêmes axes. Seules les valeurs le trahissent : à IQ élevé une
//! durée en degrés vilebrequin croît toujours avec le régime (même temps
//! d'injection, plus de degrés), donc la dernière ligne (régime max) doit
//! porter les durées les plus longues.
//!
//! La décision se prend sur Duration 00 et 01 (les maps principales, à
//! forte pente) et s'applique à tout le bloc : le calculateur range toutes
//! ses durées de la même façon. Banc : 329 fichiers U1/U31/U34, seul le
//! logiciel ci-dessus est marqué.

use crate::models::{DetectedMap, MapDimensions};

/// Marque `rows_reversed` sur toutes les maps du bloc quand les données de
/// Duration 00/01 décroissent avec l'index de ligne malgré un axe croissant.
pub fn mark_duration_block_orientation(data: &[u8], maps: &mut [DetectedMap]) {
    let mut votes_reversed = 0usize;
    let mut votes_normal = 0usize;
    for map in maps.iter() {
        let is_main = matches!(map.name.as_deref(), Some("Duration 00") | Some("Duration 01"));
        if !is_main {
            continue;
        }
        let MapDimensions::TwoDimensional { rows, cols } = map.dimensions else { continue };
        if rows < 4 || cols < 4 {
            continue;
        }
        let Some(y_addr) = map.y_axis_address else { continue };
        let y = y_addr as usize;
        let base = map.address as usize;
        if y + rows * 2 > data.len() || base + rows * cols * 2 > data.len() {
            continue;
        }
        let rd_u = |o: usize| u16::from_be_bytes([data[o], data[o + 1]]);
        let rd_i = |o: usize| i16::from_be_bytes([data[o], data[o + 1]]) as i64;
        // L'axe régime doit être croissant (toujours vrai sur le banc) ; on
        // ne raisonne pas sur un axe déjà décroissant.
        if rd_u(y) >= rd_u(y + (rows - 1) * 2) {
            continue;
        }
        // Colonnes d'IQ élevé : moitié haute de l'axe IQ
        let mut score: i64 = 0;
        for c in cols / 2..cols {
            let first = rd_i(base + c * 2);
            let last = rd_i(base + ((rows - 1) * cols + c) * 2);
            score += last - first;
        }
        let threshold = 40 * (cols - cols / 2) as i64; // ~1 deg CrS par colonne
        if score < -threshold {
            votes_reversed += 1;
        } else if score > threshold {
            votes_normal += 1;
        }
    }
    if votes_reversed > 0 && votes_normal == 0 {
        log::debug!(
            "🔁 [EDC16] Duration block stored with descending RPM rows ({} maps marked rows_reversed)",
            maps.len()
        );
        for map in maps.iter_mut() {
            map.rows_reversed = Some(true);
        }
    }
}
