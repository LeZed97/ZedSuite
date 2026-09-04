// EDC15VM detector - Bosch EDC15 family (VM variant)
// 512KB files, LITTLE-ENDIAN, codeblock-based detection

use crate::models::{DetectedMap, MapDimensions, DataType};
use super::edc15p::complete_patterns::EDC15PMapPattern;
use std::collections::HashSet;

mod patterns;

/// V4.1 codeblock signature
const CODEBLOCK_SIGNATURE: [u8; 11] = [
    0x67, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x56, 0x34, 0x2E, 0x31,
];

/// Codeblock size in bytes
const CODEBLOCK_SIZE: usize = 0x10000;

#[derive(Debug, Clone)]
struct Codeblock {
    id: u32,
    start_address: u32,
    end_address: u32,
}

pub struct EDC15VMDetector;

impl EDC15VMDetector {
    pub fn new() -> Self {
        Self
    }

    pub fn detect(&self, data: &[u8]) -> Vec<DetectedMap> {
        let codeblocks = self.find_codeblocks(data);
        log::debug!("EDC15VM: found {} codeblocks", codeblocks.len());

        let mut maps = Vec::new();
        let mut detected_addresses: HashSet<u32> = HashSet::new();

        // Chaque codeblock est scanné sur SA région. Les données de
        // calibration précèdent le bloc de code signé, d'où la marge de
        // 0x20000 en amont ; elle est bornée par la fin du codeblock
        // précédent pour que deux blocs ne se disputent pas la même zone.
        // Sans ces bornes, le premier codeblock balayait tout le fichier et
        // s'attribuait la totalité des cartes — les fichiers multimap se
        // retrouvaient avec un seul jeu de cartographies.
        let mut regions: Vec<(u32, usize, usize)> = Vec::new();
        let mut previous_end = 0usize;
        for cb in &codeblocks {
            let region_start = (cb.start_address.saturating_sub(0x20000) as usize).max(previous_end);
            let region_end = (cb.end_address as usize).min(data.len());
            if region_start < region_end {
                regions.push((cb.id, region_start, region_end));
                self.scan_region(data, region_start, region_end, cb, &mut maps, &mut detected_addresses);
            }
            previous_end = region_end;
        }

        maps.sort_by_key(|m| m.address);
        let classified = self.name_known_maps(data, maps);
        let mut result = self.assign_categories(classified);

        // Remet les cartes du balayage générique dans le sens du fichier.
        // Applique après le nommage — qui raisonne sur la convention du
        // balayage — et avant les détecteurs spécialisés, dont les cartes
        // sont déjà orientées correctement.
        Self::fix_scan_orientation(&mut result);

        // Rebuild detected_addresses from classified maps only
        // (name_known_maps drops unclassified maps, freeing their addresses for special detectors)
        detected_addresses.clear();
        for m in &result {
            detected_addresses.insert(m.address);
        }

        // Special detections
        self.detect_soi_maps(&codeblocks, data, &mut result, &mut detected_addresses);
        // Fallback mono-SOI (fichiers sans sélecteur C4/C5) — après le
        // nommage : s'ancre sur les « SOI limiter (temp) » déjà classés
        self.detect_single_soi_maps(data, &mut result, &mut detected_addresses);
        self.find_svrl_vm(data, &mut result, &mut detected_addresses);
        self.find_left_foot_brake_vm(data, &mut result, &mut detected_addresses);
        self.detect_smoke_limiters(data, &mut result, &mut detected_addresses);
        self.find_svbl(data, &mut result, &mut detected_addresses);
        self.find_switch_activation_smoke(data, &mut result, &mut detected_addresses);
        self.find_maf_linearization(&codeblocks, data, &mut result, &mut detected_addresses);
        self.find_idle_rpm_maps(data, &mut result);

        // Les EDC15P et EDC15VM partagent l'essentiel de leurs cartes (seule
        // l'injection diffère : pompe distributrice contre injecteurs-pompe).
        // On réutilise donc la base de patterns EDC15P, qui porte déjà noms,
        // descriptions, unités et facteurs des deux axes.
        self.apply_edc15p_patterns(data, &mut result);

        // Complète ce que les patterns n'ont pas couvert. Sans cette passe
        // l'interface retombe sur ses valeurs par défaut ("degC" en X, "rpm"
        // en Y) et affiche donc des axes faux.
        self.annotate_axes(data, &mut result);

        // Les détecteurs spécialisés (SOI, limiteurs de fumée, SVBL…) créent
        // leurs cartes hors du balayage par codeblock et ne renseignent donc
        // pas d'appartenance. On la déduit de l'adresse, faute de quoi elles
        // se retrouveraient toutes dans un même bloc fictif — ce qui fausse
        // aussi bien l'affichage que la déduplication ci-dessous.
        // Attribution du codeblock par PROXIMITÉ au bloc de code, et non par
        // région : les régions se partagent le fichier à une frontière nette,
        // alors que les données de calibration d'un bloc débordent un peu
        // au-delà de lui (vérifié sur g4 110 ALH : la carte 0x60BAC appartient
        // au bloc 0x50000-0x60000 mais tombait dans la région du bloc suivant,
        // d'où deux « MAP linearization » attribuées au même codeblock).
        for map in result.iter_mut() {
            let addr = map.address as usize;
            let nearest = codeblocks.iter().min_by_key(|cb| {
                let start = cb.start_address as usize;
                let end = cb.end_address as usize;
                if addr >= start && addr < end {
                    0
                } else if addr < start {
                    start - addr
                } else {
                    addr - end
                }
            });
            if let Some(cb) = nearest {
                map.codeblock_id = Some(cb.id);
            }
        }

        Self::dedupe_unique_per_codeblock(data, &mut result);

        // Launch control : la zone existe toujours dans le binaire mais la
        // carte n'est ajoutée que si la solution l'a activée (axe de vitesse
        // véhicule réellement écrit) — dossier « Launch control ».
        result.retain(|m| {
            !m.name
                .as_deref()
                .map(|n| n.to_lowercase().contains("launch control"))
                .unwrap_or(false)
        });
        super::launch_control::detect_launch_control(data, &mut result, &mut detected_addresses);

        // EGR temperature map : passe dédiée sur la structure des axes — le
        // balayage générique la rate quand la carte est uniforme (EGR à zéro).
        super::egr_temperature::detect_egr_temperature(data, &mut result, &mut detected_addresses);

        result.sort_by_key(|m| m.address);
        result
    }

    /// Rétablit l'orientation des cartes issues du balayage générique.
    ///
    /// Une structure du fichier s'enchaîne ainsi : `[axe1][axe2][données]`, et
    /// les valeurs sont rangées par ligne le long de l'axe1 — autrement dit
    /// axe1 porte les LIGNES et axe2 les COLONNES. Le balayage nommait le
    /// premier axe « X » et déclarait `rows = len(axe2), cols = len(axe1)`,
    /// soit exactement l'inverse : les cartes étaient lues transposées, ce qui
    /// entrelaçait les valeurs (une « Driver wish » repartait à zéro au milieu
    /// de chaque ligne au lieu de croître avec le régime).
    ///
    /// Vérifié en mesurant la régularité des variations le long des axes sur
    /// le dump de référence : 0.96 contre 0.83 pour « Driver wish », 1.00
    /// contre 0.88 pour la pompe N146, 0.86 contre 0.70 pour la consigne de
    /// suralimentation.
    fn fix_scan_orientation(maps: &mut Vec<DetectedMap>) {
        for map in maps.iter_mut() {
            let (rows, cols) = match map.dimensions {
                MapDimensions::TwoDimensional { rows, cols } => (rows, cols),
                _ => continue,
            };
            // Les cartes 1D n'ont qu'un axe : rien à réorienter.
            if rows <= 1 || cols <= 1 {
                continue;
            }
            let (x, y) = match (map.x_axis_address, map.y_axis_address) {
                (Some(x), Some(y)) => (x, y),
                _ => continue,
            };
            map.dimensions = MapDimensions::TwoDimensional { rows: cols, cols: rows };
            // X désigne désormais l'axe des colonnes (le second du fichier) et
            // Y celui des lignes (le premier).
            map.x_axis_address = Some(x.max(y));
            map.y_axis_address = Some(x.min(y));
        }
    }

    /// Cartes qui n'existent qu'en un seul exemplaire par codeblock.
    ///
    /// La liste est volontairement restreinte : d'autres cartes existent
    /// légitimement en plusieurs exemplaires dans un même bloc (amorçage,
    /// correction de ralenti, les dix cartes d'avance à l'injection…) et les
    /// dédupliquer en supprimerait de vraies.
    const UNIQUE_PER_CODEBLOCK: &'static [&'static str] = &["EGR hysteresis"];

    /// Cartes qui n'existent qu'en UN exemplaire par codeblock.
    const SINGLE_PER_CODEBLOCK: &'static [&'static str] =
        &["MAF correction by temperature", "Boost target map"];

    /// Nombre de cellules RENSEIGNÉES d'une carte : ni nulles, ni saturées.
    ///
    /// Sert à départager deux candidates de même nom. Une zone laissée non
    /// calibrée est remplie de 0x0000 ou de 0xFFxx, alors qu'une vraie
    /// cartographie porte des valeurs dans une plage physique — c'est donc
    /// celle qui en compte le PLUS qu'il faut conserver.
    fn meaningful_cells(data: &[u8], map: &DetectedMap) -> usize {
        let start = map.address as usize;
        let end = (start + map.size).min(data.len());
        let mut count = 0;
        let mut i = start;
        while i + 1 < end {
            let v = u16::from_le_bytes([data[i], data[i + 1]]);
            if v != 0 && v < 0xFF00 {
                count += 1;
            }
            i += 2;
        }
        count
    }

    /// Ne conserve que les meilleurs exemplaires par codeblock des cartes
    /// listées, en gardant celles qui portent le plus de valeurs réelles.
    ///
    /// « EGR hysteresis » va PAR DEUX dans le fichier (seuils d'activation et
    /// de coupure, deux courbes 20×1 consécutives — vérifié sur SG3350 :
    /// données à 0x75248 et 0x7529C, chacune précédée de son axe régime
    /// 0xDD de 20 valeurs, conformément à la référence Stage X). L'ancienne
    /// déduplication à un seul exemplaire supprimait la seconde vraie carte.
    fn dedupe_unique_per_codeblock(data: &[u8], maps: &mut Vec<DetectedMap>) {
        // Cartes en un seul exemplaire par codeblock (ex. MAF correction by
        // temperature : le balayage en trouvait deux dans le même bloc).
        Self::keep_n_per_codeblock(data, maps, Self::SINGLE_PER_CODEBLOCK, 1);
        // Cartes en paire (hystérésis EGR : seuils d'activation/coupure).
        Self::keep_n_per_codeblock(data, maps, Self::UNIQUE_PER_CODEBLOCK, 2);
    }

    /// Ne garde que `keep` exemplaires par codeblock des cartes nommées,
    /// en privilégiant celles qui portent le plus de valeurs réelles.
    fn keep_n_per_codeblock(
        data: &[u8],
        maps: &mut Vec<DetectedMap>,
        names: &[&str],
        keep: usize,
    ) {
        let keep_per_codeblock = keep;
        for unique_name in names {
            // Regroupe les candidates par codeblock avec leur score.
            let mut by_cb: std::collections::HashMap<u32, Vec<(u32, usize)>> =
                std::collections::HashMap::new();
            for map in maps.iter() {
                if map.name.as_deref() != Some(*unique_name) {
                    continue;
                }
                let cb = map.codeblock_id.unwrap_or(0);
                let score = Self::meaningful_cells(data, map);
                by_cb.entry(cb).or_default().push((map.address, score));
            }
            if by_cb.is_empty() {
                continue;
            }
            // Par codeblock : les N candidates les moins saturées survivent.
            let mut kept: std::collections::HashMap<u32, Vec<u32>> =
                std::collections::HashMap::new();
            for (cb, mut cands) in by_cb {
                cands.sort_by_key(|&(addr, score)| (std::cmp::Reverse(score), addr));
                kept.insert(
                    cb,
                    cands.iter().take(keep_per_codeblock).map(|&(a, _)| a).collect(),
                );
            }
            maps.retain(|map| {
                if map.name.as_deref() != Some(*unique_name) {
                    return true;
                }
                let cb = map.codeblock_id.unwrap_or(0);
                match kept.get(&cb) {
                    Some(addrs) => addrs.contains(&map.address),
                    None => true,
                }
            });
        }
    }

    /// Nom lisible de la grandeur portée par une unité, pour composer des
    /// descriptions du même format que celles de l'EDC15P.
    fn quantity_name(unit: &str) -> &'static str {
        match unit {
            "rpm" => "Engine speed",
            "%" => "Pedal position",
            "degC" => "Temperature",
            "mbar" => "Pressure",
            "mg/st" => "Injection quantity",
            "mg/Hub" => "Air mass",
            "kg/h" => "Air flow",
            "mV" | "V" => "Voltage",
            _ => "Value",
        }
    }

    /// Identifiant de famille et longueur d'un axe, lus dans son en-tête.
    fn axis_header(data: &[u8], data_addr: u32) -> Option<(u8, usize)> {
        let hdr = (data_addr as usize).checked_sub(4)?;
        if hdr + 4 > data.len() {
            return None;
        }
        let len = u16::from_le_bytes([data[hdr + 2], data[hdr + 3]]) as usize;
        if len == 0 || len > 64 {
            return None;
        }
        Some((data[hdr + 1], len))
    }

    /// Cherche le pattern EDC15P décrivant une carte.
    ///
    /// Reprend les quatre stratégies du détecteur EDC15P : correspondance
    /// exacte sur les identifiants d'axes, puis axes intervertis, puis les
    /// deux mêmes passes en tolérant les identifiants génériques (0x00).
    /// Le booléen retourné indique que le pattern décrit les axes dans
    /// l'ordre inverse du nôtre — ses métadonnées X s'appliquent alors à
    /// notre axe Y, et réciproquement.
    fn match_edc15p_pattern<'a>(
        patterns: &'a [EDC15PMapPattern],
        len: usize,
        x_id: u8,
        y_id: u8,
        x_len: usize,
        y_len: usize,
    ) -> Option<(&'a EDC15PMapPattern, bool)> {
        for p in patterns {
            if p.length == len
                && p.x_axis_length == x_len
                && p.y_axis_length == y_len
                && p.x_axis_id_high != 0x00
                && p.y_axis_id_high != 0x00
                && p.x_axis_id_high == x_id
                && p.y_axis_id_high == y_id
            {
                return Some((p, false));
            }
        }
        for p in patterns {
            if p.length == len
                && p.x_axis_length == y_len
                && p.y_axis_length == x_len
                && p.x_axis_id_high != 0x00
                && p.y_axis_id_high != 0x00
                && p.x_axis_id_high == y_id
                && p.y_axis_id_high == x_id
            {
                return Some((p, true));
            }
        }
        for p in patterns {
            if p.length == len && p.x_axis_length == x_len && p.y_axis_length == y_len {
                if (p.x_axis_id_high == 0x00 || p.x_axis_id_high == x_id)
                    && (p.y_axis_id_high == 0x00 || p.y_axis_id_high == y_id)
                {
                    return Some((p, false));
                }
            }
        }
        for p in patterns {
            if p.length == len && p.x_axis_length == y_len && p.y_axis_length == x_len {
                if (p.x_axis_id_high == 0x00 || p.x_axis_id_high == y_id)
                    && (p.y_axis_id_high == 0x00 || p.y_axis_id_high == x_id)
                {
                    return Some((p, true));
                }
            }
        }
        None
    }

    /// Applique la base de patterns EDC15P : nom, catégorie, unité de la
    /// grandeur mesurée, facteurs des deux axes et description « unité |
    /// X: … | Y: … », dans l'ordre d'axes réellement retenu pour la carte.
    fn apply_edc15p_patterns(&self, data: &[u8], maps: &mut Vec<DetectedMap>) {
        let patterns = EDC15PMapPattern::load_patterns();

        for map in maps.iter_mut() {
            let (x_addr, y_addr) = match (map.x_axis_address, map.y_axis_address) {
                (Some(x), Some(y)) => (x, y),
                _ => continue,
            };
            let (x_id, x_len) = match Self::axis_header(data, x_addr) {
                Some(v) => v,
                None => continue,
            };
            let (y_id, y_len) = match Self::axis_header(data, y_addr) {
                Some(v) => v,
                None => continue,
            };

            let (pattern, swapped) = match Self::match_edc15p_pattern(
                &patterns, map.size, x_id, y_id, x_len, y_len,
            ) {
                Some(v) => v,
                None => continue,
            };

            // Métadonnées d'axes ramenées dans notre ordre.
            let (xd, xu, xc, xo, yd, yu, yc, yo) = if swapped {
                (
                    &pattern.y_axis_descr, &pattern.y_axis_units,
                    pattern.y_axis_correction, pattern.y_axis_offset,
                    &pattern.x_axis_descr, &pattern.x_axis_units,
                    pattern.x_axis_correction, pattern.x_axis_offset,
                )
            } else {
                (
                    &pattern.x_axis_descr, &pattern.x_axis_units,
                    pattern.x_axis_correction, pattern.x_axis_offset,
                    &pattern.y_axis_descr, &pattern.y_axis_units,
                    pattern.y_axis_correction, pattern.y_axis_offset,
                )
            };

            // Le détecteur VM enrichit certains noms d'une température
            // (« … 86°C ») : cette précision est plus utile que le nom
            // générique du pattern, on la conserve.
            let keep_name = map
                .name
                .as_deref()
                .map(|n| n.contains("°C"))
                .unwrap_or(false);
            if !keep_name {
                map.name = Some(pattern.name.clone());
                map.category = Some(pattern.category.clone());
            }

            if !pattern.z_axis_descr.is_empty() {
                map.unit = Some(pattern.z_axis_descr.clone());
            }
            map.correction_factor = Some(pattern.correction);
            map.offset = Some(pattern.offset);
            if !xu.is_empty() {
                map.x_label = Some(xu.clone());
            }
            if !yu.is_empty() {
                map.y_label = Some(yu.clone());
            }
            map.x_axis_correction = Some(xc);
            map.x_axis_offset = Some(xo);
            map.y_axis_correction = Some(yc);
            map.y_axis_offset = Some(yo);
            map.description = Some(format!(
                "{} | X: {} ({}) | Y: {} ({})",
                pattern.z_axis_descr, xd, xu, yd, yu
            ));
            map.confidence = map.confidence.max(0.95);
        }
    }

    /// Grandeur physique portée par un axe, déduite de son identifiant de
    /// famille et de l'amplitude de ses valeurs.
    ///
    /// L'identifiant seul ne suffit pas : `0xC0` sert aussi bien à la position
    /// pédale (0..10000 = 0..100 %) qu'à une pression ambiante (500..1000
    /// mbar), et `0xDB` à une pression de suralimentation comme à un débit
    /// d'air. L'amplitude des valeurs lue dans le fichier lève l'ambiguïté.
    ///
    /// Retourne (libellé, facteur, offset).
    fn axis_meta(id_high: u8, min_raw: u16, max_raw: u16) -> (&'static str, f64, f64) {
        // Une température est codée en Kelvin ×10 : elle occupe donc une bande
        // étroite et haute (≈ -50 °C à 115 °C), ce qui la distingue nettement
        // d'une pression ou d'un débit.
        let looks_temperature = min_raw >= 2200 && max_raw <= 3900;

        match id_high {
            // Régime moteur — brut en tr/min
            0xDD | 0xF9 | 0xEB => ("rpm", 1.0, 0.0),
            // Température — Kelvin ×10
            0xC1 | 0xC3 | 0xC4 | 0xC5 => ("degC", 0.1, -273.1),
            // Famille partagée, toujours au centième : pression ambiante
            // (bande étroite sous ~1000), pédale (pleine échelle 10000 = 100 %)
            // ou quantité injectée (butée bien plus basse, ~4500 = 45 mg/coup).
            0xC0 | 0xC2 => {
                if max_raw <= 1200 {
                    ("mbar", 1.0, 0.0)
                } else if max_raw >= 9000 && max_raw <= 12000 {
                    ("%", 0.01, 0.0)
                } else if max_raw < 9000 {
                    ("mg/st", 0.01, 0.0)
                } else {
                    // Au-delà de la pleine échelle pédale : grandeur non
                    // identifiée, on laisse le détecteur décider.
                    ("", 1.0, 0.0)
                }
            }
            // Famille partagée : température d'admission, pression de
            // suralimentation ou débit d'air selon l'amplitude relevée.
            0xDB => {
                if looks_temperature {
                    ("degC", 0.1, -273.1)
                } else if max_raw <= 3000 {
                    ("mbar", 1.0, 0.0)
                } else {
                    ("mg/Hub", 0.1, 0.0)
                }
            }
            // Quantité injectée, ou tension mesurée pour la linéarisation du
            // débitmètre (pleine échelle ≈ 5 V sur 10 bits → pas 0.488759 mV).
            0xD9 => {
                if max_raw > 5000 {
                    ("mV", 0.488759, 0.0)
                } else {
                    ("mg/st", 0.01, 0.0)
                }
            }
            // Quantité injectée — centièmes de mg/coup
            0xDA | 0xDC | 0xDE | 0xE0 => ("mg/st", 0.01, 0.0),
            _ => ("", 1.0, 0.0),
        }
    }

    /// Lit l'en-tête d'un axe (identifiant + longueur placés juste avant les
    /// données) et renvoie sa description.
    fn read_axis(data: &[u8], data_addr: u32) -> Option<(&'static str, f64, f64)> {
        let hdr = (data_addr as usize).checked_sub(4)?;
        if hdr + 4 > data.len() {
            return None;
        }
        let id_high = data[hdr + 1];
        let len = u16::from_le_bytes([data[hdr + 2], data[hdr + 3]]) as usize;
        if len == 0 || len > 64 || data_addr as usize + len * 2 > data.len() {
            return None;
        }
        let mut min_raw = u16::MAX;
        let mut max_raw = 0u16;
        for i in 0..len {
            let o = data_addr as usize + i * 2;
            let v = u16::from_le_bytes([data[o], data[o + 1]]);
            min_raw = min_raw.min(v);
            max_raw = max_raw.max(v);
        }
        Some(Self::axis_meta(id_high, min_raw, max_raw))
    }

    /// Complète libellés et facteurs d'axes, sans écraser ce qu'un détecteur
    /// spécialisé a déjà fixé volontairement.
    fn annotate_axes(&self, data: &[u8], maps: &mut Vec<DetectedMap>) {
        for map in maps.iter_mut() {
            // Le facteur est déduit de l'axe réellement pointé : il fait donc
            // autorité sur celui qu'un détecteur avait posé en supposant une
            // orientation X/Y qui n'est pas toujours celle des adresses
            // finalement retenues (d'où des régimes divisés par cent, etc.).
            if let Some(addr) = map.x_axis_address {
                if let Some((label, factor, offset)) = Self::read_axis(data, addr) {
                    if !label.is_empty() {
                        map.x_label = Some(label.to_string());
                        map.x_axis_correction = Some(factor);
                        map.x_axis_offset = Some(offset);
                    }
                }
            }
            if let Some(addr) = map.y_axis_address {
                if let Some((label, factor, offset)) = Self::read_axis(data, addr) {
                    if !label.is_empty() {
                        map.y_label = Some(label.to_string());
                        map.y_axis_correction = Some(factor);
                        map.y_axis_offset = Some(offset);
                    }
                }
            }
            // « Boost limit map » : les DEUX axes sont bruts (tr/min et
            // mbar). L'annotation par ID voyait l'axe C0 (habituellement une
            // quantité injectée) et le déguisait en « mg/st ×0.01 » — l'axe
            // pression s'affichait 10.50 au lieu de 1050 mbar.
            if map.name.as_deref() == Some("Boost limit map") {
                if map.x_label.as_deref().map_or(false, |l| l.contains("mg")) {
                    map.x_label = Some("mbar".to_string());
                }
                if map.y_label.as_deref().map_or(false, |l| l.contains("mg")) {
                    map.y_label = Some("mbar".to_string());
                }
                map.x_axis_correction = Some(1.0);
                map.y_axis_correction = Some(1.0);
                map.x_axis_offset = Some(0.0);
                map.y_axis_offset = Some(0.0);
            }
            // Description détaillée « grandeur | X: nom (unité) | Y: … », dans
            // le même format que l'EDC15P.
            if map.description.is_none() {
                let x = map.x_label.clone().unwrap_or_default();
                let y = map.y_label.clone().unwrap_or_default();
                if !x.is_empty() || !y.is_empty() {
                    let axis = |unit: &str| -> String {
                        if unit.is_empty() {
                            String::new()
                        } else {
                            format!("{} ({})", Self::quantity_name(unit), unit)
                        }
                    };
                    let head = match map.unit.as_deref() {
                        Some(u) if !u.is_empty() => format!("{} | ", u),
                        _ => String::new(),
                    };
                    map.description = Some(if y.is_empty() {
                        format!("{}X: {}", head, axis(&x))
                    } else {
                        format!("{}X: {} | Y: {}", head, axis(&x), axis(&y))
                    });
                }
            }
        }
    }

    /// Find codeblocks by searching for V4.1 signature
    fn find_codeblocks(&self, data: &[u8]) -> Vec<Codeblock> {
        let mut codeblocks = Vec::new();
        let sig_len = CODEBLOCK_SIGNATURE.len();

        let mut i = 0;
        while i + sig_len <= data.len() {
            if data[i..i + sig_len] == CODEBLOCK_SIGNATURE {
                let cb_start = (i / CODEBLOCK_SIZE) * CODEBLOCK_SIZE;
                let cb_end = cb_start + CODEBLOCK_SIZE;
                // Numérotation à partir de 1, comme sur EDC15P : l'interface
                // affiche « Codeblock 1 » même lorsqu'il n'y en a qu'un
                // (un id 0 était interprété comme « pas de codeblock »).
                let id = codeblocks.len() as u32 + 1;

                if !codeblocks.iter().any(|c: &Codeblock| c.start_address == cb_start as u32) {
                    codeblocks.push(Codeblock {
                        id,
                        start_address: cb_start as u32,
                        end_address: cb_end.min(data.len()) as u32,
                    });
                }
                i = cb_end;
                continue;
            }
            i += 1;
        }
        codeblocks
    }

    /// Scan a region word-by-word for axis structures
    fn scan_region(
        &self,
        data: &[u8],
        start: usize,
        end: usize,
        cb: &Codeblock,
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        let mut t = start;
        while t + 8 < end && t + 8 < data.len() {
            if let Some(map) = self.try_detect_map(data, t, cb) {
                if !detected_addresses.contains(&map.address) {
                    detected_addresses.insert(map.address);
                    maps.push(map);
                }
            }
            t += 2;
        }
    }

    /// Try to detect a map at offset t: [X_ID][X_len][X_data][Y_ID][Y_len][Y_data][Map_data]
    fn try_detect_map(&self, data: &[u8], t: usize, cb: &Codeblock) -> Option<DetectedMap> {
        if t + 4 > data.len() {
            return None;
        }

        let x_axis_id = u16::from_le_bytes([data[t], data[t + 1]]);
        let x_id_high = (x_axis_id >> 8) as u8;

        if x_axis_id == 0 || !patterns::is_valid_axis_id(x_id_high) {
            return None;
        }

        let x_axis_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
        if !patterns::is_valid_length(x_id_high, x_axis_len) {
            return None;
        }

        let y_off = t + 4 + (x_axis_len * 2);
        if y_off + 4 > data.len() {
            return None;
        }

        let y_axis_id = u16::from_le_bytes([data[y_off], data[y_off + 1]]);
        let y_id_high = (y_axis_id >> 8) as u8;

        let is_2d = y_axis_id != 0
            && patterns::is_valid_axis_id(y_id_high)
            && {
                let y_axis_len = u16::from_le_bytes([data[y_off + 2], data[y_off + 3]]) as usize;
                patterns::is_valid_length(y_id_high, y_axis_len)
            };

        if !is_2d {
            // 1D map
            let map_offset = t + 4 + (x_axis_len * 2);
            let map_length = x_axis_len * 2;
            if map_offset + map_length > data.len() {
                return None;
            }
            let mut map = DetectedMap::new(
                map_offset as u32,
                map_length,
                MapDimensions::TwoDimensional { rows: 1, cols: x_axis_len },
                DataType::UInt16,
            );
            map.name = Some(format!("1D Map {}x1 at {:06X}", x_axis_len, map_offset));
            map.confidence = 0.4;
            map.is_little_endian = Some(true);
            map.x_axis_address = Some((t + 4) as u32);
            map.codeblock_id = Some(cb.id);
            map.codeblock_start_address = Some(cb.start_address);
            map.codeblock_end_address = Some(cb.end_address);
            return Some(map);
        }

        // 2D map
        let y_axis_len = u16::from_le_bytes([data[y_off + 2], data[y_off + 3]]) as usize;
        let map_offset = y_off + 4 + (y_axis_len * 2);
        let map_length = x_axis_len * y_axis_len * 2;

        if map_length == 0 || map_offset + map_length > data.len() {
            return None;
        }

        let mut map = DetectedMap::new(
            map_offset as u32,
            map_length,
            MapDimensions::TwoDimensional { rows: y_axis_len, cols: x_axis_len },
            DataType::UInt16,
        );
        map.name = Some(format!(
            "2D Map {}x{} at {:06X}",
            x_axis_len, y_axis_len, map_offset
        ));
        map.confidence = 0.5;
        map.is_little_endian = Some(true);
        map.x_axis_address = Some((t + 4) as u32);
        map.y_axis_address = Some((y_off + 4) as u32);
        map.x_axis_correction = Some(1.0);
        map.y_axis_correction = Some(1.0);
        map.correction_factor = Some(1.0);
        map.offset = Some(0.0);
        map.codeblock_id = Some(cb.id);
        map.codeblock_start_address = Some(cb.start_address);
        map.codeblock_end_address = Some(cb.end_address);
        Some(map)
    }

    /// Classify detected maps by size and axis IDs
    /// Reference axis pairs from 4MKGOLF.bin analysis:
    ///   Driver wish 256B: DD/C0 (16x8)
    ///   Cranking fuel 162B: DD/C1 (9x9)
    ///   Torque limiter fuel 114B: C0/DD (3x19)
    ///   Smoke limit MAP 416B: DD/DB (16x13)
    ///   Turbo boost corr 320B: DB/DB (16x10)
    ///   N146 480B: DD/C0 (16x15)
    ///   Desired air EGR 416B: DD/D9 (16x13)
    ///   Variable geom 416B: DD/DA (16x13)
    ///   Boost request 320B: DD/C0 (16x10)
    ///   Boost limiter 200B: C0/DD (10x10)
    ///   Inverse driver wish 384B: DD/C0 (12x16)
    ///   EGR hysteresis 40B: DD (20x1)
    ///   Torque limiter limp 12B: DD (6x1)
    ///   Boost sensor linear 4B: DD (2x1)
    ///   Idle RPM 4B: C1 (2x1)
    fn name_known_maps(&self, data: &[u8], maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        let mut classified = Vec::new();

        for mut map in maps {
            let (x_len, y_len) = match map.dimensions {
                MapDimensions::TwoDimensional { rows, cols } => (cols, rows),
                _ => {
                    classified.push(map);
                    continue;
                }
            };

            // Read axis ID high bytes
            let x_id_high = map.x_axis_address.and_then(|addr| {
                let id_addr = addr.saturating_sub(4) as usize;
                if id_addr + 2 <= data.len() {
                    Some((u16::from_le_bytes([data[id_addr], data[id_addr + 1]]) >> 8) as u8)
                } else {
                    None
                }
            });
            let y_id_high = map.y_axis_address.and_then(|addr| {
                let id_addr = addr.saturating_sub(4) as usize;
                if id_addr + 2 <= data.len() {
                    Some((u16::from_le_bytes([data[id_addr], data[id_addr + 1]]) >> 8) as u8)
                } else {
                    None
                }
            });

            let xh = x_id_high.unwrap_or(0);
            let yh = y_id_high.unwrap_or(0);

            let mut named = false;

            // ── Size 480/512/544 (16x15-17) - N146 Pump voltage ──
            // 4MKGOLF: DD(16)/C0(15) = 480B at 0x740E4 ; le golf zlatarica
            // a un axe IQ à 17 points ([C064][0x11] à 0x600F6) = 544B
            if !named && matches!(map.size, 480 | 512 | 544) {
                if matches!((xh, yh),
                    (0xDD, 0xC0) | (0xC0, 0xDD) | (0xDC, 0xC0) | (0xC0, 0xDC) |
                    (0xE0, 0xC2) | (0xC2, 0xE0) | (0xEB, 0xC0) | (0xC0, 0xEB) |
                    (0xDD, 0xC2) | (0xC2, 0xDD)
                ) {
                    map.name = Some("N146 Pump voltage map".to_string());
                    // 1.221 mV/bit exprimé en volts : brut 1219..1952 →
                    // 1.49..2.38 V. L'ancien 1.221 affichait des milliers
                    // (millivolts) sous un libellé de tension.
                    map.correction_factor = Some(0.001221);
                    map.unit = Some("V".to_string());
                    map.x_axis_correction = Some(0.01);
                    map.category = Some("Fuel".to_string());
                    named = true;
                }
            }

            // ── Size 448 (14x16 or 16x14) - SOI (standalone, not from selector) ──
            // DISABLED: SOI maps are detected by detect_soi_maps() using the C4 temperature selector.
            // The axis scan often finds false positives for 448-byte maps that are actually SOI data
            // without the proper temperature context. Only detect_soi_maps should create SOI entries.
            // if !named && map.size == 448 { ... }

            // ── Size 416 (16x13 or 13x16) - Multiple map types ──
            // Smoke MAP: DD/DB (16x13) at 0x6D270
            // Desired air EGR: DD/D9 (16x13) at 0x74A3A
            // Variable geometry: DD/DA (16x13) at 0x763E0
            // Also: N75, EGR from EDC15P patterns
            if !named && map.size == 416 {
                let axes = (xh, yh);

                // Smoke limitation by MAP: DD/DB
                if matches!(axes, (0xDD, 0xDB) | (0xDB, 0xDD) | (0xDC, 0xDB) | (0xDB, 0xDC)) {
                    map.name = Some("IQ by MAP limiter".to_string());
                    map.correction_factor = Some(0.01);
                    map.x_axis_correction = Some(0.1);
                    map.category = Some("Smoke limitation".to_string());
                    named = true;
                }
                // DD/D9 : DEUX familles partagent ces IDs en 416 o (réf
                // EDCSuite golf zlatarica) — EGR 01 (rpm 0..5355) à 0x59820
                // et N75 duty cycle (rpm 760..4242) à 0x5B26C. Le départ de
                // l'axe RPM (id DD) tranche : ≥400 = N75.
                else if matches!(axes, (0xDD, 0xD9) | (0xD9, 0xDD) | (0xDC, 0xD9) | (0xD9, 0xDC)) {
                    let rpm_addr = if xh == 0xDD || xh == 0xDC {
                        map.x_axis_address
                    } else {
                        map.y_axis_address
                    };
                    let rpm0 = rpm_addr
                        .and_then(|a| {
                            let a = a as usize;
                            if a + 2 <= data.len() {
                                Some(u16::from_le_bytes([data[a], data[a + 1]]))
                            } else {
                                None
                            }
                        })
                        .unwrap_or(0);
                    if rpm0 >= 400 {
                        map.name = Some("N75 duty cycle".to_string());
                        map.correction_factor = Some(-0.01);
                        map.offset = Some(100.0);
                        map.category = Some("Turbo boost pressure control".to_string());
                    } else {
                        map.name = Some("EGR".to_string());
                        map.correction_factor = Some(0.1);
                        map.x_axis_correction = Some(0.01);
                        map.category = Some("EGR".to_string());
                    }
                    named = true;
                }
                // DD/DA 416 : la PLAGE de l'axe DA tranche (réf Stage X
                // golf : « Smoke limitation by MAP » = x 800..2000 mbar à
                // 0x5CCEE, data plate 5000) — ≤2600 = pression → IQ by MAP ;
                // sinon variable geometry / N75 (4MKGOLF 0x763E0).
                else if matches!(axes,
                    (0xDD, 0xDA) | (0xDA, 0xDD) | (0xDC, 0xDA) | (0xDA, 0xDC)
                ) {
                    let (da_addr, da_len) = if xh == 0xDA {
                        (map.x_axis_address, x_len)
                    } else {
                        (map.y_axis_address, y_len)
                    };
                    let da_max = da_addr
                        .map(|addr| {
                            (0..da_len)
                                .map(|i| {
                                    let off = addr as usize + i * 2;
                                    if off + 1 < data.len() {
                                        u16::from_le_bytes([data[off], data[off + 1]])
                                    } else {
                                        0
                                    }
                                })
                                .max()
                                .unwrap_or(0)
                        })
                        .unwrap_or(0);
                    if da_max <= 2600 && da_max >= 1200 {
                        map.name = Some("IQ by MAP limiter".to_string());
                        map.correction_factor = Some(0.01);
                        map.category = Some("Smoke limitation".to_string());
                    } else {
                        map.name = Some("N75 duty cycle".to_string());
                        map.correction_factor = Some(-0.01);
                        map.offset = Some(100.0);
                        map.category = Some("Turbo boost pressure control".to_string());
                    }
                    named = true;
                }
                // Structure DD/C0 416B : sur 4MKGOLF (0x74E82) ce n'est PAS
                // l'EGR (Stage X) — mais le stock_012GN n'a AUCUN DD/D9 et
                // son EGR vit ici (0x54E0C : rpm 0..5355, IQ 0..5100, data
                // 2300..8500). Gates de profil stricts pour ne prendre que
                // le cas EGR : rpm part de 0, axe X part de ~0 et finit
                // 4500-6000, données en plage air (≤12000).
                else if matches!(axes, (0xDD, 0xC0) | (0xC0, 0xDD) | (0xDC, 0xC0) | (0xC0, 0xDC)) {
                    let read_axis = |addr: Option<u32>, len: usize| -> Vec<u16> {
                        addr.map(|a| {
                            (0..len)
                                .map(|i| {
                                    let off = a as usize + i * 2;
                                    if off + 1 < data.len() {
                                        u16::from_le_bytes([data[off], data[off + 1]])
                                    } else {
                                        0
                                    }
                                })
                                .collect()
                        })
                        .unwrap_or_default()
                    };
                    let (rpm, iq) = if xh == 0xDD || xh == 0xDC {
                        (read_axis(map.x_axis_address, x_len), read_axis(map.y_axis_address, y_len))
                    } else {
                        (read_axis(map.y_axis_address, y_len), read_axis(map.x_axis_address, x_len))
                    };
                    let dmax = (0..(map.size / 2).min(220))
                        .map(|i| {
                            let off = map.address as usize + i * 2;
                            if off + 1 < data.len() {
                                u16::from_le_bytes([data[off], data[off + 1]])
                            } else {
                                0
                            }
                        })
                        .max()
                        .unwrap_or(0);
                    if !rpm.is_empty()
                        && !iq.is_empty()
                        && rpm[0] <= 100
                        && iq[0] <= 100
                        && (4500..=6000).contains(iq.last().unwrap_or(&0))
                        && dmax <= 12000
                    {
                        map.name = Some("EGR".to_string());
                        map.correction_factor = Some(0.1);
                        map.x_axis_correction = Some(0.01);
                        map.category = Some("EGR".to_string());
                        named = true;
                    }
                }
                // N75 duty cycle: DD/EA, EB/EA, EC/DA etc.
                else if matches!(axes,
                    (0xDD, 0xEA) | (0xEA, 0xDD) | (0xEB, 0xEA) | (0xEA, 0xEB) |
                    (0xEC, 0xDA) | (0xDA, 0xEC)
                ) {
                    map.name = Some("N75 duty cycle".to_string());
                    map.correction_factor = Some(-0.01);
                    map.offset = Some(100.0);
                    map.category = Some("Turbo boost pressure control".to_string());
                    named = true;
                }
            }

            // Taille 384 : « Inverse driver wish » n'est pas reconnue par le
            // détecteur EDC15P ; volontairement laissée non classée pour que
            // les deux familles exposent la même liste de cartes.

            // ── Size 320 (16x10 or 10x16) ──
            // Boost request: DD/C0 (16x10) at 0x760D4
            // Turbo boost pressure correction: DB/DB (16x10) at 0x71F50
            if !named && map.size == 320 {
                let axes = (xh, yh);

                // Turbo boost pressure correction: DB/DB
                // DA/DA = même structure que l'EDC15P (boost 800..2500 ×
                // temps 2531..3431, golf ALH 0x5CED0) — ajoutée au banc VM
                if matches!(axes, (0xDB, 0xDB) | (0xDA, 0xDA)) {
                    map.name = Some("Boost correction by temperature".to_string());
                    map.correction_factor = Some(1.0);
                    map.category = Some("Turbo boost pressure".to_string());
                    named = true;
                }
                // Boost target / request: DD/C0, DC/C0, EB/C0, DD/DC
                else if matches!(axes,
                    (0xDD, 0xC0) | (0xC0, 0xDD) | (0xDC, 0xC0) | (0xC0, 0xDC) |
                    (0xE0, 0xC3) | (0xC3, 0xE0) | (0xEB, 0xC0) | (0xC0, 0xEB) |
                    (0xDD, 0xDC) | (0xDC, 0xDD)
                ) {
                    map.name = Some("Boost target map".to_string());
                    map.correction_factor = Some(1.0);
                    map.category = Some("Turbo boost pressure".to_string());
                    named = true;
                }
                // Limitation de débit : l'EDC15VM n'a pas de « Smoke
                // limiter » — seulement une limitation par masse d'air (MAF)
                // et une par pression (MAP). La famille se lit sur la PLAGE
                // de l'axe X : brut > 2600 = masse d'air (300..850 mg/st avec
                // un facteur 0.1), sinon pression en mbar.
                else if matches!(axes,
                    (0xDD, 0xDA) | (0xDA, 0xDD) | (0xE0, 0xDC) | (0xDC, 0xE0) |
                    (0xEB, 0xDA) | (0xDA, 0xEB)
                ) {
                    let x_axis_max = map
                        .x_axis_address
                        .map(|addr| {
                            (0..x_len)
                                .map(|i| {
                                    let off = addr as usize + i * 2;
                                    if off + 1 < data.len() {
                                        u16::from_le_bytes([data[off], data[off + 1]])
                                    } else {
                                        0
                                    }
                                })
                                .max()
                                .unwrap_or(0)
                        })
                        .unwrap_or(0);
                    if x_axis_max > 2600 {
                        map.name = Some("IQ by MAF limiter".to_string());
                        map.x_axis_correction = Some(0.1);
                    } else {
                        map.name = Some("IQ by MAP limiter".to_string());
                        map.x_axis_correction = Some(1.0);
                    }
                    map.correction_factor = Some(0.01);
                    map.category = Some("Smoke limitation".to_string());
                    named = true;
                }
            }

            // ── Size 384 (16x12) DD/DA - IQ by MAF/MAP (variante golf
            // zlatarica : [DD(16)] 861..5355 + [DA(12)] 3000..9500 à
            // 0x5C990) — même règle de plage d'axe X que le 320.
            // DD/C0 384 (12x16) = « Inverse driver wish » : rétablie
            // (réfs EDCSuite golf 0x5BE82 / Seat leon 0x56BEA).
            if !named && map.size == 384 {
                if matches!((xh, yh), (0xDD, 0xC0) | (0xC0, 0xDD) | (0xDC, 0xC0) | (0xC0, 0xDC)) {
                    map.name = Some("Inverse driver wish".to_string());
                    map.correction_factor = Some(1.0);
                    map.category = Some("Engine fuel request".to_string());
                    named = true;
                } else if matches!((xh, yh),
                    (0xDD, 0xDA) | (0xDA, 0xDD) | (0xDC, 0xDA) | (0xDA, 0xDC)
                ) {
                    let x_axis_max = map
                        .x_axis_address
                        .map(|addr| {
                            (0..x_len)
                                .map(|i| {
                                    let off = addr as usize + i * 2;
                                    if off + 1 < data.len() {
                                        u16::from_le_bytes([data[off], data[off + 1]])
                                    } else {
                                        0
                                    }
                                })
                                .max()
                                .unwrap_or(0)
                        })
                        .unwrap_or(0);
                    if x_axis_max > 2600 {
                        map.name = Some("IQ by MAF limiter".to_string());
                        map.x_axis_correction = Some(0.1);
                    } else {
                        map.name = Some("IQ by MAP limiter".to_string());
                        map.x_axis_correction = Some(1.0);
                    }
                    map.correction_factor = Some(0.01);
                    map.category = Some("Smoke limitation".to_string());
                    named = true;
                }
            }

            // ── Size 256 (16x8 or 8x16) - Driver wish ──
            // 4MKGOLF: DD(16)/C0(8) = 256B at 0x6C40E
            if !named && map.size == 256 {
                if matches!((xh, yh),
                    (0xDD, 0xC0) | (0xC0, 0xDD) | (0xEB, 0xC0) | (0xC0, 0xEB) |
                    (0xDC, 0xC0) | (0xC0, 0xDC)
                ) {
                    map.name = Some("Driver wish".to_string());
                    map.correction_factor = Some(0.01);
                    map.category = Some("Engine fuel request".to_string());
                    named = true;
                }
            }

            // ── Size 200 (10x10) - Boost limit ──
            // 4MKGOLF: C0(10)/DD(10) = 200B at 0x766CA
            if !named && map.size == 200 && x_len == 10 && y_len == 10 {
                // (0xDD, 0xF9) portait « Inverse driver wish » — absente du
                // détecteur EDC15P, donc laissée non classée.
                if matches!((xh, yh),
                    (0xC0, 0xDD) | (0xDD, 0xC0) | (0xC0, 0xDC) | (0xDC, 0xC0) |
                    (0xC2, 0xE0) | (0xE0, 0xC2) | (0xC0, 0xEB) | (0xEB, 0xC0) |
                    (0xC1, 0xF9) | (0xF9, 0xC1)
                ) {
                    map.name = Some("Boost limit map".to_string());
                    map.correction_factor = Some(1.0);
                    // Les deux axes sont bruts : régime en tr/min et pression
                    // atmosphérique en mbar. Sans le 1.0 explicite, l'axe
                    // pression héritait d'un 0.01 et s'affichait « 15.00 »
                    // en mg/st au lieu de 1500 mbar.
                    map.x_axis_correction = Some(1.0);
                    map.y_axis_correction = Some(1.0);
                    map.unit = Some("mbar".to_string());
                    map.description = Some(
                        "Boost limit | X: Engine speed (rpm) | Y: Atmospheric pressure (mbar)".to_string(),
                    );
                    map.category = Some("Turbo boost pressure".to_string());
                    named = true;
                }
            }

            // ── Size 192 (8x12 or 12x8) - Driver wish ──
            // Les combinaisons (0xC2, 0xC0) et (0xC2, 0xDD) portaient
            // « Inverse driver wish » — absente du détecteur EDC15P, donc
            // laissée non classée.
            if !named && map.size == 192 {
                if matches!((xh, yh),
                    (0xE0, 0xC1) | (0xC1, 0xE0) | (0xE0, 0xC2) | (0xC2, 0xE0) |
                    (0xDD, 0xC0) | (0xC0, 0xDD) | (0xEB, 0xC0) | (0xC0, 0xEB)
                ) {
                    map.name = Some("Driver wish".to_string());
                    map.correction_factor = Some(0.01);
                    map.category = Some("Engine fuel request".to_string());
                    named = true;
                }
            }

            // ── Size 182 (7x13 or 13x7) ──
            if !named && map.size == 182 {
                if matches!((xh, yh), (0xDC, 0xC0) | (0xC0, 0xDC)) {
                    map.name = Some("Driver wish".to_string());
                    map.correction_factor = Some(0.01);
                    map.category = Some("Engine fuel request".to_string());
                    named = true;
                } else if matches!((xh, yh),
                    (0xDD, 0xC3) | (0xC3, 0xDD) | (0xDC, 0xC5) | (0xC5, 0xDC)
                ) {
                    map.name = Some("SOI limiter (temp)".to_string());
                    map.correction_factor = Some(0.01);
                    map.category = Some("Start of injection".to_string());
                    named = true;
                }
            }

            // ── Size 162 (9x9) - Cranking fuel map (Start IQ) ──
            // 4MKGOLF: DD(9)/C1(9) at 0x6C64A and 0x6C72C
            if !named && map.size == 162 && x_len == 9 && y_len == 9 {
                if matches!((xh, yh),
                    (0xDD, 0xC1) | (0xC1, 0xDD) | (0xEB, 0xC1) | (0xC1, 0xEB) |
                    (0xDC, 0xC1) | (0xC1, 0xDC)
                ) {
                    map.name = Some("Start IQ".to_string());
                    map.correction_factor = Some(0.01);
                    map.category = Some("Engine fuel request".to_string());
                    named = true;
                }
            }

            // ── Size 128 (8x8) - MAF correction by temp ──
            if !named && map.size == 128 && x_len == 8 && y_len == 8 {
                if matches!((xh, yh),
                    (0xDD, 0xC1) | (0xC1, 0xDD) | (0xDC, 0xC1) | (0xC1, 0xDC) |
                    (0xE0, 0xC0) | (0xC0, 0xE0) | (0xEB, 0xC1) | (0xC1, 0xEB)
                ) {
                    // Même nom et même classement que sur EDC15P, où ces
                    // cartes sont rangées dans « Other ».
                    map.name = Some("MAF correction by temperature".to_string());
                    map.correction_factor = Some(0.01);
                    map.category = Some("Other".to_string());
                    named = true;
                }
            }

            // ── Torque limiter: 114 (3x19 or 19x3), 120 (3x20), 138 (3x23), 150 (3x25) ──
            // 4MKGOLF: C0(3)/DD(19) = 114B at 0x6CA8E
            if !named && matches!(map.size, 114 | 120 | 138 | 150)
                && (x_len == 3 || y_len == 3)
            {
                if matches!((xh, yh),
                    (0xC0, 0xDD) | (0xDD, 0xC0) | (0xC0, 0xDC) | (0xDC, 0xC0) |
                    (0xC0, 0xEB) | (0xEB, 0xC0)
                ) || y_len == 0 {
                    map.name = Some("Torque limiter".to_string());
                    map.correction_factor = Some(0.01);
                    map.category = Some("Engine torque limiters".to_string());
                    named = true;
                }
            }

            // ── 1D maps ──

            // EGR hysteresis: size 40 (20x1), axis DD
            // 4MKGOLF: DD(20) at 0x75248, 0x7529C
            if !named && map.size == 40 && x_len == 20 && y_len == 1 {
                if xh == 0xDD || xh == 0xDC || xh == 0xEB {
                    map.name = Some("EGR hysteresis".to_string());
                    // Facteur validé sur dump réel (g4 110 ALH) : brut 0..3060
                    // → 0..30.6 mg/Hub. Avec 0.1 on obtenait 306 mg/Hub, soit
                    // presque la consigne d'air complète — impossible pour une
                    // bande d'hystérésis.
                    map.correction_factor = Some(0.01);
                    map.unit = Some("mg/Hub".to_string());
                    map.category = Some("EGR".to_string());
                    named = true;
                }
            }

            // Torque limiter limp mode: size 12 (6x1), axis DD
            // 4MKGOLF: DD(6) at 0x6D708 → values: 0, 2500, 2500, 2500, 1400, 0
            // Validate: peak value in IQ range (500-5000), and not all identical
            if !named && map.size == 12 && x_len == 6 && y_len == 1 {
                if xh == 0xDD || xh == 0xDC || xh == 0xEB {
                    let off = map.address as usize;
                    if off + 12 <= data.len() {
                        let mut vals = [0u16; 6];
                        let mut max_v = 0u16;
                        let mut all_same = true;
                        for i in 0..6 {
                            vals[i] = u16::from_le_bytes([data[off + i * 2], data[off + i * 2 + 1]]);
                            if vals[i] > max_v { max_v = vals[i]; }
                            if i > 0 && vals[i] != vals[0] { all_same = false; }
                        }
                        // Cartes limp mode : retirées de la liste (demande
                        // utilisateur, comme sur les autres calculateurs).
                        // On saute la carte entièrement — sans quoi elle
                        // resterait visible sous son nom générique de scan
                        // (« 1D Map 6x1 at … »).
                        if !all_same && max_v >= 500 && max_v <= 5000 {
                            continue;
                        }
                    }
                }
            }

            // Size 4 (2x1) - Boost sensor linearization, MAP linearization, or Idle RPM
            if !named && map.size == 4 && x_len == 2 && y_len == 1 {
                let x_axis_full_id = map.x_axis_address.and_then(|addr| {
                    let id_addr = addr.saturating_sub(4) as usize;
                    if id_addr + 2 <= data.len() {
                        Some(u16::from_le_bytes([data[id_addr], data[id_addr + 1]]))
                    } else {
                        None
                    }
                });
                let xfull = x_axis_full_id.unwrap_or(0);

                // MAP linearization : l'ID d'axe VARIE par SW (DCD4 sur
                // golf/jetta, DD28 sur Seat leon, DC9C sur stock_012GN —
                // Stage X la trouve, EDCSuite non) ; c'est le CONTENU qui
                // est invariant : axe ADC [~82, ~989] mV → pression
                // [~200, ~2600] mbar.
                let content_ok = {
                    let axis_ok = map
                        .x_axis_address
                        .map(|a| {
                            let a = a as usize;
                            if a + 4 <= data.len() {
                                let a1 = u16::from_le_bytes([data[a], data[a + 1]]);
                                let a2 = u16::from_le_bytes([data[a + 2], data[a + 3]]);
                                // a1 ≥ 20 : écarte la courbe « boost sensor »
                                // ([0,1023] → [0,1267]) qui partagerait des
                                // bornes trop lâches
                                (20..500).contains(&a1) && a2 > 500 && a2 < 1200
                            } else {
                                false
                            }
                        })
                        .unwrap_or(false);
                    let d = map.address as usize;
                    let data_ok = if d + 4 <= data.len() {
                        let d1 = u16::from_le_bytes([data[d], data[d + 1]]);
                        let d2 = u16::from_le_bytes([data[d + 2], data[d + 3]]);
                        d1 < 1000 && (2000..=3500).contains(&d2)
                    } else {
                        false
                    };
                    axis_ok && data_ok
                };
                if xfull == 0xDCD4 || xfull == 0xDD28 || content_ok {
                    // Même nom et même dossier que sur EDC15P
                    map.name = Some("MAP linearization".to_string());
                    map.correction_factor = Some(1.0);
                    map.category = Some("MAP sensor".to_string());
                    named = true;
                }
                // Idle RPM: C1 axis (coolant temperature)
                else if xh == 0xC1 {
                    map.name = Some("Idle RPM".to_string());
                    map.correction_factor = Some(1.0);
                    map.category = Some("Idle speed RPM".to_string());
                    named = true;
                }
            }

            if named {
                map.confidence = 0.85;
            }

            // Only keep classified maps
            if named {
                classified.push(map);
            }
        }

        classified
    }

    // ═══════════════════════════════════════════════════════════════════
    //  Special detection functions
    // ═══════════════════════════════════════════════════════════════════

    /// Left foot brake switch — MÊME séquence que l'EDC15P
    /// ([27 00 00 64 00 xx], valeur à +5) : présente 1×/codeblock sur tous
    /// les VM du banc, elle n'était simplement pas cherchée. Le codeblock
    /// est attribué ensuite par proximité comme pour les autres passes.
    fn find_left_foot_brake_vm(
        &self,
        data: &[u8],
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        let pattern: [u8; 5] = [0x27, 0x00, 0x00, 0x64, 0x00];
        let mut pos = 0;
        while pos + 7 < data.len() {
            let found = match data[pos..].windows(5).position(|w| w == pattern) {
                Some(rel) => pos + rel,
                None => break,
            };
            pos = found + 1;
            let addr = (found + 5) as u32;
            let value = u16::from_le_bytes([data[addr as usize], data[addr as usize + 1]]);
            if value > 1 || detected_addresses.contains(&addr) {
                continue;
            }
            let state = if value == 1 { "ON" } else { "OFF" };
            let mut map = DetectedMap::new(
                addr,
                2,
                MapDimensions::TwoDimensional { rows: 1, cols: 1 },
                DataType::UInt16,
            );
            map.name = Some("Left foot brake switch".to_string());
            // Dossier « Other », comme sur EDC15P
            map.category = Some("Other".to_string());
            // Comme la SVBL : pas de valeur dans la description, la valeur
            // est déjà la cellule de la carte.
            map.description = Some("Left foot brake behaviour (0=OFF, 1=ON)".to_string());
            map.confidence = 0.92;
            map.correction_factor = Some(1.0);
            map.is_little_endian = Some(true);
            log::debug!("EDC15VM: Left foot brake switch at 0x{:X} = {}", addr, value);
            detected_addresses.insert(addr);
            maps.push(map);
        }
    }

    /// SVRL (limiteur de régime, valeur seule). Même marqueur que l'EDC15P
    /// ([27 07 00 0E]) mais layout différent : sur VM la valeur est à
    /// marqueur−8, précédée des constantes [0x6000][0x2000] (vérifié sur
    /// golf/Tamas/g4 110/a6 : 6000 tr/min à chaque fois, 1 par codeblock).
    fn find_svrl_vm(
        &self,
        data: &[u8],
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        let marker: [u8; 4] = [0x27, 0x07, 0x00, 0x0E];
        let mut pos = 12;
        while pos + 4 < data.len() {
            let found = match data[pos..].windows(4).position(|w| w == marker) {
                Some(rel) => pos + rel,
                None => break,
            };
            pos = found + 1;
            if found < 12 {
                continue;
            }
            let le16 = |o: usize| u16::from_le_bytes([data[o], data[o + 1]]);
            // Contexte : [0x6000][0x2000] à −12, valeur à −8
            if le16(found - 12) != 0x6000 || le16(found - 10) != 0x2000 {
                continue;
            }
            let value = le16(found - 8);
            if !(3000..=8000).contains(&value) {
                continue;
            }
            let addr = (found - 8) as u32;
            if detected_addresses.contains(&addr) {
                continue;
            }
            let mut map = DetectedMap::new(
                addr,
                2,
                MapDimensions::TwoDimensional { rows: 1, cols: 1 },
                DataType::UInt16,
            );
            map.name = Some("SVRL - RPM Limiter".to_string());
            map.category = Some("Maximum RPM limiter".to_string());
            // Comme la SVBL : pas de valeur dans la description
            map.description = Some("Maximum engine RPM limit:".to_string());
            map.unit = Some("rpm".to_string());
            map.confidence = 0.95;
            map.correction_factor = Some(1.0);
            map.is_little_endian = Some(true);
            log::debug!("EDC15VM: SVRL at 0x{:X} = {} rpm", addr, value);
            detected_addresses.insert(addr);
            maps.push(map);
        }
    }

    /// Fallback « une seule map SOI » : certains EDC15VM (golf zlatarica,
    /// 038906012L, jetta…) n'ont PAS de sélecteur de température C4/C5 —
    /// leur(s) map(s) SOI ([DDxx][12-16 RPM] + [DCxx][6-10 IQ], données
    /// signées faibles) sont juste AVANT le « SOI limiter (temp) » de
    /// chaque codeblock (golf : 2×14x7 à 0x60EFC/0x61010, limiteur à
    /// 0x611C6). Confirme l'intuition de ZedPerf (« certains ont juste une
    /// seule map SOI »).
    fn detect_single_soi_maps(
        &self,
        data: &[u8],
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        let has_soi = maps.iter().any(|m| {
            m.name
                .as_deref()
                .map_or(false, |n| n.starts_with("Start of injection"))
        });
        if has_soi {
            return;
        }
        let lim_addrs: Vec<usize> = maps
            .iter()
            .filter(|m| m.name.as_deref() == Some("SOI limiter (temp)"))
            .map(|m| m.address as usize)
            .collect();
        let le16 = |o: usize| u16::from_le_bytes([data[o], data[o + 1]]);
        for la in lim_addrs {
            let lo = la.saturating_sub(0x400);
            let mut found: Vec<(usize, usize, usize, usize, usize)> = Vec::new();
            let mut i = lo;
            while i + 60 < la {
                let l1 = le16(i + 2) as usize;
                if (le16(i) >> 8) as u8 != 0xDD || !(12..=16).contains(&l1) {
                    i += 2;
                    continue;
                }
                let rpm: Vec<u16> = (0..l1).map(|k| le16(i + 4 + k * 2)).collect();
                if rpm.windows(2).any(|w| w[0] > w[1])
                    || !(3000..=6000).contains(&rpm[l1 - 1])
                {
                    i += 2;
                    continue;
                }
                let j = i + 4 + l1 * 2;
                let l2 = le16(j + 2) as usize;
                if (le16(j) >> 8) as u8 != 0xDC || !(6..=10).contains(&l2) {
                    i += 2;
                    continue;
                }
                let iq: Vec<u16> = (0..l2).map(|k| le16(j + 4 + k * 2)).collect();
                if iq.windows(2).any(|w| w[0] > w[1]) || !(2000..=6000).contains(&iq[l2 - 1]) {
                    i += 2;
                    continue;
                }
                let d0 = j + 4 + l2 * 2;
                let dsz = l1 * l2 * 2;
                if d0 + dsz > la {
                    i += 2;
                    continue;
                }
                // Données SOI : degrés signés faibles (±3000 = ±70° max)
                let vals_ok = (0..l1 * l2).all(|k| {
                    let v = le16(d0 + k * 2) as i16;
                    (-3000..=3000).contains(&v)
                });
                if !vals_ok || detected_addresses.contains(&(d0 as u32)) {
                    i += 2;
                    continue;
                }
                found.push((d0, i + 4, j + 4, l1, l2));
                i = d0 + dsz;
            }
            // La map SOI PRINCIPALE (« Start of injection (N108 SOI) » chez
            // EDCSuite, 16×13 à 0x612D0 sur le golf) est APRÈS le limiteur.
            {
                let hi = (la + 0x400).min(data.len().saturating_sub(500));
                let mut i = la;
                while i + 60 < hi {
                    let l1 = le16(i + 2) as usize;
                    if (le16(i) >> 8) as u8 != 0xDD || !(12..=16).contains(&l1) {
                        i += 2;
                        continue;
                    }
                    let rpm: Vec<u16> = (0..l1).map(|k| le16(i + 4 + k * 2)).collect();
                    if rpm.windows(2).any(|w| w[0] > w[1])
                        || !(3000..=6000).contains(&rpm[l1 - 1])
                    {
                        i += 2;
                        continue;
                    }
                    let j = i + 4 + l1 * 2;
                    let l2 = le16(j + 2) as usize;
                    if (le16(j) >> 8) as u8 != 0xDC || !(6..=14).contains(&l2) {
                        i += 2;
                        continue;
                    }
                    let iq: Vec<u16> = (0..l2).map(|k| le16(j + 4 + k * 2)).collect();
                    if iq.windows(2).any(|w| w[0] > w[1]) || !(2000..=6000).contains(&iq[l2 - 1])
                    {
                        i += 2;
                        continue;
                    }
                    let d0 = j + 4 + l2 * 2;
                    let dsz = l1 * l2 * 2;
                    if d0 + dsz > data.len() || detected_addresses.contains(&(d0 as u32)) {
                        i += 2;
                        continue;
                    }
                    let vals_ok = (0..l1 * l2).all(|k| {
                        let v = le16(d0 + k * 2) as i16;
                        (-3000..=3000).contains(&v)
                    });
                    if !vals_ok {
                        i += 2;
                        continue;
                    }
                    let mut map = DetectedMap::new(
                        d0 as u32,
                        dsz,
                        MapDimensions::TwoDimensional { rows: l1, cols: l2 },
                        DataType::Int16,
                    );
                    map.name = Some("Start of injection (N108 SOI)".to_string());
                    map.category = Some("Start of injection".to_string());
                    map.unit = Some("deg CrS".to_string());
                    map.correction_factor = Some(0.023437);
                    map.is_little_endian = Some(true);
                    map.y_axis_address = Some((i + 4) as u32);
                    map.y_axis_correction = Some(1.0);
                    map.x_axis_address = Some((j + 4) as u32);
                    map.x_axis_correction = Some(0.01);
                    map.confidence = 0.9;
                    log::debug!(
                        "EDC15VM: SOI principale (N108) {}x{} at 0x{:X}",
                        l1,
                        l2,
                        d0
                    );
                    detected_addresses.insert(d0 as u32);
                    maps.push(map);
                    break;
                }
            }

            for (idx, &(d0, y0, x0, l1, l2)) in found.iter().enumerate() {
                let mut map = DetectedMap::new(
                    d0 as u32,
                    l1 * l2 * 2,
                    MapDimensions::TwoDimensional { rows: l1, cols: l2 },
                    DataType::Int16,
                );
                map.name = Some(if idx == 0 {
                    "Start of injection (SOI)".to_string()
                } else {
                    format!("Start of injection (SOI) {}", idx + 1)
                });
                map.category = Some("Start of injection".to_string());
                map.unit = Some("deg CrS".to_string());
                map.correction_factor = Some(0.023437);
                map.is_little_endian = Some(true);
                map.y_axis_address = Some(y0 as u32);
                map.y_axis_correction = Some(1.0);
                map.x_axis_address = Some(x0 as u32);
                map.x_axis_correction = Some(0.01);
                map.confidence = 0.88;
                log::debug!(
                    "EDC15VM: single-SOI {}x{} at 0x{:X} (limiter 0x{:X})",
                    l1,
                    l2,
                    d0,
                    la
                );
                detected_addresses.insert(d0 as u32);
                maps.push(map);
            }
        }
    }

    /// Detect smoke limiter maps with temperature selector
    /// Structure: [Y_axis F9 len=16][X_axis DA/DB len=13][Z_axis C1 len=N][N×temps][N×indexes][N×416byte maps]
    /// 4MKGOLF: F9(16) at 0x6CB80, DB(13) at 0x6CBA4, C1(3) at 0x6CBC2
    ///   temps: -40°C, 0°C, 140°C → 3 maps at 0x6CBD2, 0x6CD72, 0x6CF12
    fn detect_smoke_limiters(
        &self,
        data: &[u8],
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        const SMOKE_MAP_SIZE: usize = 416; // 13*16*2

        let mut t = 0;
        while t + 100 < data.len() {
            // Look for Y axis: ID high=0xF9, len=16
            let y_id = u16::from_le_bytes([data[t], data[t + 1]]);
            if (y_id >> 8) as u8 != 0xF9 {
                t += 2;
                continue;
            }
            let y_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
            if y_len != 16 {
                t += 2;
                continue;
            }

            // X axis: ID high=0xDA or 0xDB, len=13
            let x_off = t + 4 + y_len * 2;
            if x_off + 4 > data.len() {
                t += 2;
                continue;
            }
            let x_id = u16::from_le_bytes([data[x_off], data[x_off + 1]]);
            let x_hi = (x_id >> 8) as u8;
            if x_hi != 0xDA && x_hi != 0xDB {
                t += 2;
                continue;
            }
            let x_len = u16::from_le_bytes([data[x_off + 2], data[x_off + 3]]) as usize;
            if x_len != 13 {
                t += 2;
                continue;
            }

            // Z axis (temperature selector): ID high=0xC1, len=1..10
            let z_off = x_off + 4 + x_len * 2;
            if z_off + 4 > data.len() {
                t += 2;
                continue;
            }
            let z_id = u16::from_le_bytes([data[z_off], data[z_off + 1]]);
            if (z_id >> 8) as u8 != 0xC1 {
                t += 2;
                continue;
            }
            let z_len = u16::from_le_bytes([data[z_off + 2], data[z_off + 3]]) as usize;
            if z_len == 0 || z_len > 10 {
                t += 2;
                continue;
            }

            // Read temperatures (Kelvin×10)
            let temps_off = z_off + 4;
            let indexes_off = temps_off + z_len * 2;
            let first_map_off = indexes_off + z_len * 2;

            if first_map_off + SMOKE_MAP_SIZE * z_len > data.len() {
                t += 2;
                continue;
            }

            let mut temps_c: Vec<i32> = Vec::new();
            let mut valid = true;
            for i in 0..z_len {
                let raw = u16::from_le_bytes([data[temps_off + i * 2], data[temps_off + i * 2 + 1]]);
                let tc = ((raw as f64 * 0.1) - 273.15).round() as i32;
                if tc < -100 || tc > 200 {
                    valid = false;
                    break;
                }
                temps_c.push(tc);
            }
            if !valid {
                t += 2;
                continue;
            }

            log::debug!("EDC15VM: Smoke limiter selector at 0x{:X}, {} temps: {:?}", z_off, z_len, temps_c);

            // Remove any previously-detected maps at these addresses (from axis scan)
            for i in 0..z_len {
                let map_addr = (first_map_off + i * SMOKE_MAP_SIZE) as u32;
                maps.retain(|m| m.address != map_addr);
                detected_addresses.remove(&map_addr);
            }

            // Famille selon la PLAGE de l'axe X (l'ID ne suffit pas : les
            // deux familles utilisent 0xDB sur g4 110 ALH). Masse d'air =
            // brut 3000..8770 (×0.1 → 300..877 mg/st) ; pression = brut
            // 800..2000 mbar. Seuil à 2600 entre les deux plages.
            let x_axis_max = (0..x_len)
                .map(|i| u16::from_le_bytes([data[x_off + 4 + i * 2], data[x_off + 4 + i * 2 + 1]]))
                .max()
                .unwrap_or(0);
            // Noms identiques à l'EDC15P. L'EDC15VM n'a pas de « Smoke
            // limiter » : seulement une limitation par masse d'air (MAF) et
            // une par pression (MAP).
            let family = if x_axis_max > 2600 { "IQ by MAF limiter" } else { "IQ by MAP limiter" };

            // TOUS les slots du sélecteur sont émis : la référence EDCSuite
            // du Seat leon 012FN liste les 3 « Smoke limiter » du trio
            // (0x4CBD2/0x4CD72/0x4CF32), slots vides compris — l'ancien
            // filtre « cartes renseignées uniquement » n'en gardait qu'une.
            let keep: Vec<usize> = (0..z_len).collect();

            let mut rank = 0usize;
            for &i in &keep {
                let map_addr = (first_map_off + i * SMOKE_MAP_SIZE) as u32;
                if detected_addresses.contains(&map_addr) {
                    continue;
                }
                rank += 1;
                let mut map = DetectedMap::new(
                    map_addr,
                    SMOKE_MAP_SIZE,
                    MapDimensions::TwoDimensional { rows: 16, cols: 13 },
                    DataType::UInt16,
                );
                map.name = Some(if keep.len() > 1 && rank > 1 {
                    format!("{} {}", family, rank)
                } else {
                    family.to_string()
                });
                map.category = Some("Smoke limitation".to_string());
                map.correction_factor = Some(0.01);
                map.x_axis_correction = Some(0.1);
                map.confidence = 0.92;
                map.x_axis_address = Some((x_off + 4) as u32);
                map.y_axis_address = Some((t + 4) as u32);
                map.map_selector = Some(crate::models::MapSelectorInfo {
                    num_repeats: z_len,
                    selector_address: z_off as u32,
                    map_data: Vec::new(),
                    map_indexes: (0..z_len as u16).collect(),
                });
                detected_addresses.insert(map_addr);
                maps.push(map);
            }

            t += 2;
        }
    }

    /// Find exact byte sequence in data
    fn find_exact_sequence(&self, data: &[u8], start_offset: usize, pattern: &[u8]) -> Option<usize> {
        if start_offset + pattern.len() > data.len() {
            return None;
        }
        for i in start_offset..=(data.len() - pattern.len()) {
            if data[i..i + pattern.len()] == *pattern {
                return Some(i);
            }
        }
        None
    }

    /// Find SVBL (Single Value Boost Limiter) using the same 3 sequences as EDC15V C# code
    /// 4MKGOLF: SVBL at 0x71B3C = 2750 mbar
    fn find_svbl(
        &self,
        data: &[u8],
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        // Sequence 1: [DF 7A 28 00 00 00 00 00 DF 7A] → SVBL at -2 (or -4 if [00 C3] precedes)
        let seq1: [u8; 10] = [0xDF, 0x7A, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00, 0xDF, 0x7A];
        let mut offset = 4;
        loop {
            match self.find_exact_sequence(data, offset, &seq1) {
                None => break,
                Some(pos) => {
                    let svbl_off = if pos >= 4
                        && data[pos - 2] == 0x00
                        && data[pos - 1] == 0xC3
                    {
                        pos - 4
                    } else if pos >= 2 {
                        pos - 2
                    } else {
                        offset = pos + 1;
                        continue;
                    };
                    self.add_svbl_map(data, svbl_off as u32, maps, detected_addresses);
                    offset = pos + 1;
                }
            }
        }

        // Sequence 2: [28 04 28 04 28 04 28 04 28 04 FF 03] → SVBL at +42
        let seq2: [u8; 12] = [0x28, 0x04, 0x28, 0x04, 0x28, 0x04, 0x28, 0x04, 0x28, 0x04, 0xFF, 0x03];
        offset = 0;
        loop {
            match self.find_exact_sequence(data, offset, &seq2) {
                None => break,
                Some(pos) => {
                    if pos + 42 + 2 <= data.len() {
                        let mut svbl_off = pos + 42;
                        if svbl_off + 1 < data.len()
                            && data[svbl_off] == 0x00
                            && data[svbl_off + 1] == 0xC3
                        {
                            svbl_off -= 2;
                        }
                        self.add_svbl_map(data, svbl_off as u32, maps, detected_addresses);
                    }
                    offset = pos + 1;
                }
            }
        }

        // Sequence 3: [7F C3 10 27 10 27] → SVBL at +8
        let seq3: [u8; 6] = [0x7F, 0xC3, 0x10, 0x27, 0x10, 0x27];
        offset = 0;
        loop {
            match self.find_exact_sequence(data, offset, &seq3) {
                None => break,
                Some(pos) => {
                    if pos + 8 + 2 <= data.len() {
                        let mut svbl_off = pos + 8;
                        if svbl_off + 1 < data.len()
                            && data[svbl_off] == 0x00
                            && data[svbl_off + 1] == 0xC3
                        {
                            svbl_off -= 2;
                        }
                        self.add_svbl_map(data, svbl_off as u32, maps, detected_addresses);
                    }
                    offset = pos + 1;
                }
            }
        }
    }

    fn add_svbl_map(
        &self,
        data: &[u8],
        addr: u32,
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        if detected_addresses.contains(&addr) {
            return;
        }
        let off = addr as usize;
        if off + 2 > data.len() {
            return;
        }
        let value = u16::from_le_bytes([data[off], data[off + 1]]);
        if value == 0 || (value >= 1000 && value <= 7000) {
            let mut map = DetectedMap::new(
                addr,
                2,
                MapDimensions::TwoDimensional { rows: 1, cols: 1 },
                DataType::UInt16,
            );
            map.name = Some("SVBL (Single value boost limiter)".to_string());
            map.category = Some("Turbo boost pressure".to_string());
            // Description volontairement réduite au libellé : la valeur est
            // déjà la cellule de la carte (demande utilisateur).
            map.description = Some("Maximum boost pressure limit:".to_string());
            map.unit = Some("mbar".to_string());
            map.confidence = 0.98;
            map.correction_factor = Some(1.0);
            log::debug!("EDC15VM: SVBL at 0x{:X} = {} mbar", addr, value);
            detected_addresses.insert(addr);
            maps.push(map);
        }
    }

    /// Find "Switch activation for smoke limiter" - a single 1x1 value
    /// Located near the SVBL marker area. Value is typically 0 or 1.
    /// 4MKGOLF: at 0x71AF0, value=0
    fn find_switch_activation_smoke(
        &self,
        data: &[u8],
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        // The switch activation sits right before SVBL in the binary.
        // Search for the pattern: [01 01] [01 00] [01 41] around smoke/SVBL area
        // More reliable: search for the exact SVBL marker and go backwards
        // In 4MKGOLF: 0x71AF0 is 76 bytes before SVBL at 0x71B3C

        // Strategy: look for SVBL maps we already found, then search backwards.
        // UN SWITCH PAR CODEBLOCK : itérer sur TOUS les SVBL (l'ancien
        // `break` après le premier laissait les codeblocks 2/3 sans switch).
        let svbl_addrs: Vec<usize> = maps
            .iter()
            .filter(|m| m.name.as_deref() == Some("SVBL (Single value boost limiter)"))
            .map(|m| m.address as usize)
            .collect();
        for svbl_addr in svbl_addrs {
            {
                // Switch activation is typically 0x4C (76) bytes before SVBL
                // Search in the range [svbl - 0x100, svbl)
                if svbl_addr < 0x100 {
                    continue;
                }
                let search_start = svbl_addr - 0x100;
                // Look for a 1x1 value (0 or 1) that makes sense as a switch
                // The C# code identifies this by specific structure, we use proximity to SVBL
                let candidate = svbl_addr - 0x4C; // 0x71B3C - 0x4C = 0x71AF0
                if candidate + 2 <= data.len() {
                    let val = u16::from_le_bytes([data[candidate], data[candidate + 1]]);
                    if val <= 1 {
                        let addr = candidate as u32;
                        if !detected_addresses.contains(&addr) {
                            let mut map = DetectedMap::new(
                                addr,
                                2,
                                MapDimensions::TwoDimensional { rows: 1, cols: 1 },
                                DataType::UInt16,
                            );
                            // Même présentation que le switch MAP/MAF de
                            // l'EDC15P : le mode courant + la valeur à écrire
                            // pour basculer, directement dans la description.
                            let mode = if val == 0 { "MAF" } else { "MAP" };
                            map.name = Some("MAP/MAF switch".to_string());
                            map.category = Some("Smoke limitation".to_string());
                            // Comme la SVBL : pas de valeur dans la description
                            map.description = Some("Sensor mode (0=MAF, 1=MAP)".to_string());
                            map.confidence = 0.90;
                            map.correction_factor = Some(1.0);
                            log::debug!("EDC15VM: Switch activation smoke at 0x{:X} = {}", addr, val);
                            detected_addresses.insert(addr);
                            maps.push(map);
                        }
                    }
                }
            }
        }
    }

    /// Detect SOI maps by temperature axis selector
    /// EDC15VM uses axis ID high byte 0xC4 (not 0xC5 like EDC15P)
    /// Structure: [N×448byte maps][C4_selector_axis with N temps][N indexes]
    /// 4MKGOLF: C4 selector at 0x79B58 len=10, maps at 0x789D8-0x79998
    ///   Also C4 at 0x789C0 len=5 (different selector within same region)
    fn detect_soi_maps(
        &self,
        _codeblocks: &[Codeblock],
        data: &[u8],
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        const SOI_MAP_SIZE: usize = 448;

        for t in (0..data.len().saturating_sub(50)).step_by(2) {
            let axis_id = u16::from_le_bytes([data[t], data[t + 1]]);
            // EDC15VM SOI selector uses 0xC4 or 0xC5 axis ID high byte
            let hi = (axis_id >> 8) as u8;
            if hi != 0xC4 && hi != 0xC5 {
                continue;
            }
            if t + 3 >= data.len() {
                continue;
            }
            let axis_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
            // SOI selector: typically exactly 10 temperature entries (or 1 for single SOI)
            // Reject 2-9 to avoid false positives from other C4/C5 selectors
            if axis_len == 0 || (axis_len >= 2 && axis_len <= 9) || axis_len > 10 {
                continue;
            }

            // Read temperature values (Kelvin × 10)
            let mut temps_raw: Vec<u16> = Vec::new();
            let mut valid = true;
            for i in 0..axis_len {
                let off = t + 4 + i * 2;
                if off + 1 >= data.len() {
                    valid = false;
                    break;
                }
                let val = u16::from_le_bytes([data[off], data[off + 1]]);
                if val < 2000 || val > 4500 {
                    valid = false;
                    break;
                }
                temps_raw.push(val);
            }
            if !valid || temps_raw.is_empty() {
                continue;
            }

            let temps_c: Vec<i32> = temps_raw
                .iter()
                .map(|&v| ((v as f64 / 10.0) - 273.15).round() as i32)
                .collect();

            // Validate temperature spread
            let first_t = temps_c[0];
            let last_t = temps_c[temps_c.len() - 1];
            if first_t > 50 || last_t < 30 {
                continue;
            }
            // Temperatures must be increasing
            let mut increasing = true;
            for i in 1..temps_c.len() {
                if temps_c[i] < temps_c[i - 1] {
                    increasing = false;
                    break;
                }
            }
            if !increasing {
                continue;
            }

            // After temperature values: map indexes (axis_len × u16)
            let indexes_off = t + 4 + axis_len * 2;
            if indexes_off + axis_len * 2 > data.len() {
                continue;
            }
            let mut valid_indexes = true;
            for i in 0..axis_len {
                let idx = u16::from_le_bytes([data[indexes_off + i * 2], data[indexes_off + i * 2 + 1]]);
                if idx != (i as u16 * 256) {
                    valid_indexes = false;
                    break;
                }
            }
            if !valid_indexes {
                continue;
            }

            // SOI maps are BEFORE the selector axis
            let total_map_data = SOI_MAP_SIZE * axis_len;
            if t < total_map_data {
                continue;
            }
            let first_map_addr = t - total_map_data;

            log::debug!("EDC15VM: SOI temp axis (0x{:02X}) at 0x{:X}, {} temps: {:?}, maps start 0x{:X}",
                hi, t, axis_len, temps_c, first_map_addr);

            // Search shared axes before maps (within 3000 bytes)
            let search_start = if first_map_addr > 3000 { first_map_addr - 3000 } else { 0 };
            let mut shared_x_addr: Option<u32> = None;
            let mut shared_y_addr: Option<u32> = None;
            let mut shared_x_id: Option<u16> = None;
            let mut shared_y_id: Option<u16> = None;

            // Y axis RPM: 0xF9 len=16
            for off in (search_start..first_map_addr).step_by(2) {
                if off + 36 >= data.len() {
                    continue;
                }
                let id = u16::from_le_bytes([data[off], data[off + 1]]);
                let len = u16::from_le_bytes([data[off + 2], data[off + 3]]) as usize;
                if (id >> 8) as u8 == 0xF9 && len == 16 {
                    let first_val = u16::from_le_bytes([data[off + 4], data[off + 5]]);
                    let last_val = u16::from_le_bytes([data[off + 4 + 30], data[off + 4 + 31]]);
                    // La borne basse doit inclure 0 : beaucoup d'axes de régime
                    // démarrent à 0 tr/min (moteur arrêté). L'ancien seuil à 100
                    // rejetait ces axes et les cartes SOI se retrouvaient sans
                    // axe Y (constaté sur g4 110 ALH : axe 0xF948 à 0x78094,
                    // valeurs 0, 819, 987 … 5355).
                    if first_val <= 1500 && last_val >= 2000 && last_val <= 8000 {
                        shared_y_addr = Some((off + 4) as u32);
                        shared_y_id = Some(id);
                        break;
                    }
                }
            }

            // X axis IQ: 0xDC or 0xEB len=14
            for off in (search_start..first_map_addr).step_by(2) {
                if off + 32 >= data.len() {
                    continue;
                }
                let id = u16::from_le_bytes([data[off], data[off + 1]]);
                let id_hi = (id >> 8) as u8;
                let len = u16::from_le_bytes([data[off + 2], data[off + 3]]) as usize;
                if (id_hi == 0xEB || id_hi == 0xDC) && len == 14 {
                    let first_val = u16::from_le_bytes([data[off + 4], data[off + 5]]);
                    if first_val == 0 {
                        shared_x_addr = Some((off + 4) as u32);
                        shared_x_id = Some(id);
                        break;
                    }
                }
            }

            // Remove any previously-detected maps at SOI addresses
            for i in 0..axis_len {
                let map_addr = (first_map_addr + i * SOI_MAP_SIZE) as u32;
                maps.retain(|m| m.address != map_addr);
                detected_addresses.remove(&map_addr);
            }

            // « SOI selector » : le sélecteur de température lui-même (1×N,
            // K×10 → °C), demandé par les utilisateurs (WinOLS/EDCSuite le
            // listent). Adresse = les valeurs de l'axe C4/C5.
            {
                let sel_addr = (t + 4) as u32;
                maps.retain(|m| m.address != sel_addr);
                detected_addresses.remove(&sel_addr);
                let mut sel = DetectedMap::new(
                    sel_addr,
                    axis_len * 2,
                    MapDimensions::TwoDimensional { rows: 1, cols: axis_len },
                    DataType::UInt16,
                );
                sel.name = Some("SOI selector".to_string());
                sel.category = Some("Start of injection".to_string());
                sel.description = Some("SOI map selector: coolant temperature thresholds (°C)".to_string());
                sel.unit = Some("°C".to_string());
                sel.correction_factor = Some(0.1);
                sel.offset = Some(-273.1);
                sel.confidence = 0.95;
                // Pas d'axes dans le fichier : X = numéro de la map SOI (1..10),
                // Y vide — sans ça l'interface affichait « degC / rpm » et un faux
                // axe 0, 5, 10… qui rendait le sélecteur illisible.
                sel.x_label = Some("SOI map #".to_string());
                sel.y_label = Some(String::new());
                sel.is_little_endian = Some(true);
                detected_addresses.insert(sel_addr);
                maps.push(sel);
            }

            for i in 0..axis_len {
                let map_addr = (first_map_addr + i * SOI_MAP_SIZE) as u32;
                // Int16 : l'angle SOI passe négatif après le PMH — en non
                // signé, un -0.5° s'affichait 654.9 (0xFFxx × 0.01).
                let mut map = DetectedMap::new(
                    map_addr,
                    SOI_MAP_SIZE,
                    MapDimensions::TwoDimensional { rows: 16, cols: 14 },
                    DataType::Int16,
                );
                let temp = temps_c[i];
                map.name = Some(format!("Start of injection (SOI) {}°C", temp));
                map.category = Some("Start of injection".to_string());
                map.description = Some(format!(
                    "SOI | X: IQ (mg/st) | Y: Engine speed (rpm) | Axis IDs: X=0x{:04X} Y=0x{:04X}",
                    shared_x_id.unwrap_or(0),
                    shared_y_id.unwrap_or(0)
                ));
                // Sur EDC15VM l'angle est codé au centième de degré : brut
                // 570..1350 → 5.7..13.5 °CrS, cohérent avec l'avance d'un TDI.
                // Le 0.023437 des patterns EDC15P donnait 13..32 °, hors plage.
                map.correction_factor = Some(0.01);
                map.unit = Some("deg CrS".to_string());
                map.x_axis_correction = Some(0.01);
                map.y_axis_correction = Some(1.0);
                map.confidence = 0.95;
                map.x_axis_address = shared_x_addr;
                map.y_axis_address = shared_y_addr;
                map.map_selector = Some(crate::models::MapSelectorInfo {
                    num_repeats: axis_len,
                    selector_address: t as u32,
                    map_data: temps_raw.clone(),
                    map_indexes: (0..axis_len as u16).collect(),
                });
                detected_addresses.insert(map_addr);
                maps.push(map);
            }
        }
    }

    /// Find MAF linearization: axis ID 0xD904 len=13, size=26 (from EDC15V C# code)
    /// 4MKGOLF: D9(13) at 0x75466
    fn find_maf_linearization(
        &self,
        _codeblocks: &[Codeblock],
        data: &[u8],
        maps: &mut Vec<DetectedMap>,
        detected_addresses: &mut HashSet<u32>,
    ) {
        // Deux variantes : [D904][13] (golf/Seat leon, courbe 13 pts) et
        // [DCF2][32] (jetta 0x60C10 : courbe 32 pts SIGNÉE partant en
        // négatif, même forme que la MAF linearization EDC16)
        let mut t = 0;
        while t + 32 < data.len() {
            let axis_id = u16::from_le_bytes([data[t], data[t + 1]]);
            if axis_id == 0xD904 || axis_id == 0xDCF2 {
                let axis_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
                let expected_len = if axis_id == 0xD904 { 13 } else { 32 };
                if axis_len == expected_len {
                    let map_offset = t + 4 + axis_len * 2;
                    let map_length = axis_len * 2;
                    // Garde anti-FP pour la variante 32 pts : axe croissant
                    // finissant haut (masse d'air jusqu'à ~10000)
                    if axis_id == 0xDCF2 {
                        let ax: Vec<u16> = (0..axis_len)
                            .map(|k| u16::from_le_bytes([data[t + 4 + k * 2], data[t + 5 + k * 2]]))
                            .collect();
                        if ax.windows(2).any(|w| w[0] >= w[1])
                            || !(6000..=20000).contains(&ax[axis_len - 1])
                        {
                            t += 2;
                            continue;
                        }
                    }
                    if map_offset + map_length <= data.len() {
                        let map_addr = map_offset as u32;
                        if !detected_addresses.contains(&map_addr) {
                            // RÉTABLIE (29/08) : les références EDCSuite
                            // fournies par ZedPerf (golf zlatarica 0x60ADE,
                            // Seat leon 0x55466) listent « MAF linearization »
                            // sur les DEUX générations de VM — l'ancien
                            // masquage est levé.
                            maps.retain(|m| m.address != map_addr);
                            let mut map = DetectedMap::new(
                                map_addr,
                                map_length,
                                MapDimensions::TwoDimensional { rows: axis_len, cols: 1 },
                                // Variante 32 pts : données signées (départ
                                // négatif, jetta 0xFFD1…)
                                if axis_id == 0xDCF2 { DataType::Int16 } else { DataType::UInt16 },
                            );
                            map.name = Some("MAF linearization".to_string());
                            map.category = Some("Airflow".to_string());
                            map.unit = Some("kg/h".to_string());
                            map.correction_factor = Some(0.1);
                            map.is_little_endian = Some(true);
                            map.x_axis_address = Some((t + 4) as u32);
                            map.x_axis_correction = Some(1.0);
                            map.confidence = 0.9;
                            log::debug!("EDC15VM: MAF linearization at 0x{:X}", map_addr);
                            detected_addresses.insert(map_addr);
                            maps.push(map);
                        }
                    }
                }
            }
            t += 2;
        }
    }

    /// Post-process: validate Idle RPM maps (values must be 700-1400 RPM)
    fn find_idle_rpm_maps(&self, data: &[u8], maps: &mut Vec<DetectedMap>) {
        maps.retain_mut(|map| {
            if map.name.as_deref() == Some("Idle RPM") {
                let off = map.address as usize;
                if off + 4 <= data.len() {
                    let v1 = u16::from_le_bytes([data[off], data[off + 1]]);
                    let v2 = u16::from_le_bytes([data[off + 2], data[off + 3]]);
                    if v1 >= 700 && v1 <= 1400 && v2 >= 700 && v2 <= 1400 {
                        map.description = Some("Target idle engine speed".to_string());
                        map.unit = Some("rpm".to_string());
                        return true;
                    }
                    return false;
                }
            }
            true
        });
    }

    /// Assign frontend categories based on map names
    fn assign_categories(&self, maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        maps.into_iter().map(|mut map| {
            if map.category.is_none() {
                if let Some(ref name) = map.name {
                    let lower = name.to_lowercase();
                    if lower.contains("n146") || lower.contains("pump voltage") {
                        map.category = Some("Fuel".to_string());
                    } else if lower.contains("soi") || lower.contains("start of injection") {
                        map.category = Some("Start of injection".to_string());
                    } else if lower.contains("smoke") {
                        map.category = Some("Smoke limitation".to_string());
                    } else if lower.contains("driver wish") || lower.contains("cranking") {
                        map.category = Some("Engine fuel request".to_string());
                    } else if lower.contains("n75") || lower.contains("wastegate") || lower.contains("variable geometry") {
                        map.category = Some("Turbo boost pressure control".to_string());
                    } else if lower.contains("boost") || lower.contains("svbl") {
                        map.category = Some("Turbo boost pressure".to_string());
                    } else if lower.contains("egr") || lower.contains("desired air") {
                        map.category = Some("EGR".to_string());
                    } else if lower.contains("torque") {
                        map.category = Some("Engine torque limiters".to_string());
                    } else if lower.contains("idle") {
                        map.category = Some("Idle speed RPM".to_string());
                    } else if lower.contains("maf") {
                        map.category = Some("Injection system".to_string());
                    } else if lower.contains("map linearization") || lower.contains("boost pressure sensor") {
                        map.category = Some("Turbo boost pressure".to_string());
                    } else {
                        map.category = Some("Other".to_string());
                    }
                }
            }
            map
        }).collect()
    }
}
