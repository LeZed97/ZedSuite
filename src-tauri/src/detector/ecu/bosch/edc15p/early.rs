//! Classement par forme et valeurs d'axes pour les générations précoces
//! d'EDC15P (1999-2002 : 038906019A, 019AJ…), et hystérésis EGR (toutes
//! générations).
//!
//! Les premiers logiciels PD n'utilisent pas les identifiants d'axes des
//! fichiers suivants (régime `EA38`/`E082` au lieu de `EC2E`, IQ `E8xx`/`DExx`
//! au lieu de `C0xx`, températures `C13x`/`C14x`…) : le classifieur par
//! identifiants ne nomme presque rien. Les formes et les valeurs des axes,
//! elles, sont stables d'une génération à l'autre : un axe régime se
//! reconnaît à ses valeurs, une température à sa bande Kelvin ×10, une
//! pédale à sa pleine échelle 10000. On identifie donc l'identifiant régime
//! du fichier (le plus fréquent), puis chaque map générique par (taille,
//! longueurs d'axes, nature des axes, plage des données).
//!
//! Les conventions de sortie (quel axe fichier devient X, dimensions) copient
//! celles du classifieur standard pour chaque nom, vérifiées sur 038906019GD :
//! pour presque toutes les maps 2D, X = second axe du fichier, Y = premier
//! (régime), lignes = longueur du premier axe ; les durées d'injection
//! gardent X = premier axe (IQ).

use super::layout::Edc15pLayout;
use crate::models::{DataType, DetectedMap, MapDimensions};
use std::collections::HashMap;

/// Axe lu dans le fichier : identifiant complet et valeurs brutes.
struct Axis {
    id: u16,
    values: Vec<u16>,
}

impl Axis {
    fn len(&self) -> usize {
        self.values.len()
    }
    fn first(&self) -> u16 {
        *self.values.first().unwrap_or(&0)
    }
    fn last(&self) -> u16 {
        *self.values.last().unwrap_or(&0)
    }
    fn increasing(&self) -> bool {
        self.values.windows(2).all(|w| w[0] <= w[1]) && self.last() > self.first()
    }
    /// Kelvin ×10 : -53 °C .. +157 °C
    fn is_temperature(&self) -> bool {
        self.len() >= 3 && self.values.iter().all(|&v| (2200..=4300).contains(&v))
    }
    /// Position pédale en centièmes de % (pleine échelle 10000)
    fn is_pedal(&self) -> bool {
        self.increasing() && self.first() <= 400 && (6000..=10000).contains(&self.last())
    }
    /// Débit d'air en dixièmes de mg/coup (300..1000 mg/coup)
    fn is_airflow(&self) -> bool {
        self.increasing() && self.first() >= 1500 && self.last() >= 5000
    }
    /// Pression en mbar (ambiante ou suralimentation)
    fn is_pressure(&self) -> bool {
        self.increasing() && self.first() >= 400 && self.last() <= 3500 && !self.is_temperature()
    }
    /// Quantité injectée en centièmes de mg/coup
    fn is_iq(&self) -> bool {
        self.increasing() && self.first() <= 300 && (1500..=6500).contains(&self.last())
    }
    /// Idem, départ tolérant (les corrections SOI partent à 5 mg)
    fn is_iq_loose(&self) -> bool {
        self.increasing() && self.first() <= 1000 && (1500..=6500).contains(&self.last())
    }
}

/// Lit un axe dont les DONNÉES commencent à `data_addr` (en-tête
/// `[id u16][len u16]` juste avant).
fn read_axis_at(data: &[u8], data_addr: u32) -> Option<Axis> {
    let a = data_addr as usize;
    if a < 4 || a + 2 > data.len() {
        return None;
    }
    let id = u16::from_le_bytes([data[a - 4], data[a - 3]]);
    let len = u16::from_le_bytes([data[a - 2], data[a - 1]]) as usize;
    if len == 0 || len > 32 || a + len * 2 > data.len() {
        return None;
    }
    let values = (0..len)
        .map(|i| u16::from_le_bytes([data[a + 2 * i], data[a + 2 * i + 1]]))
        .collect();
    Some(Axis { id, values })
}

fn read_values_i16(data: &[u8], addr: u32, count: usize) -> Vec<i16> {
    let a = addr as usize;
    (0..count)
        .filter_map(|i| {
            let o = a + 2 * i;
            if o + 2 <= data.len() {
                Some(i16::from_le_bytes([data[o], data[o + 1]]))
            } else {
                None
            }
        })
        .collect()
}

fn read_values_u16(data: &[u8], addr: u32, count: usize) -> Vec<u16> {
    let a = addr as usize;
    (0..count)
        .filter_map(|i| {
            let o = a + 2 * i;
            if o + 2 <= data.len() {
                Some(u16::from_le_bytes([data[o], data[o + 1]]))
            } else {
                None
            }
        })
        .collect()
}

fn is_generic(map: &DetectedMap) -> bool {
    map.name
        .as_deref()
        .map_or(false, |n| n.starts_with("3D Map Size:") || n.starts_with("2D Map Size:"))
}

/// Identifiant de l'axe régime du fichier : le plus fréquent parmi les axes
/// croissants d'au moins 8 points qui finissent entre 2500 et 6500 tr/min.
/// (EC2E sur 019GD, EA38 sur 019AJ, E082 sur 019A.)
pub fn rpm_axis_id(data: &[u8], maps: &[DetectedMap]) -> Option<u16> {
    let mut counts: HashMap<u16, usize> = HashMap::new();
    for map in maps {
        for addr in [map.x_axis_address, map.y_axis_address].into_iter().flatten() {
            if let Some(axis) = read_axis_at(data, addr) {
                if axis.len() >= 8 && axis.increasing() && (2500..=6500).contains(&axis.last()) {
                    *counts.entry(axis.id).or_insert(0) += 1;
                }
            }
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(id, n)| (n, id))
        .filter(|&(_, n)| n >= 3)
        .map(|(id, _)| id)
}

/// Convention de sortie « standard » du classifieur : X = second axe du
/// fichier, Y = premier axe, lignes = longueur du premier axe.
fn orient_standard(map: &mut DetectedMap, first_len: usize, second_len: usize) {
    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
    map.dimensions = MapDimensions::TwoDimensional {
        rows: first_len,
        cols: second_len,
    };
}

/// Convention des durées d'injection 01-05 : X = premier axe (IQ), Y = second
/// (régime), lignes = longueur du premier axe.
fn orient_duration(map: &mut DetectedMap, first_len: usize, second_len: usize) {
    map.dimensions = MapDimensions::TwoDimensional {
        rows: first_len,
        cols: second_len,
    };
}

fn set_common(map: &mut DetectedMap, name: &str, subcategory: &str, confidence: f32) {
    map.name = Some(name.to_string());
    map.category = Some("Detected maps".to_string());
    map.subcategory = Some(subcategory.to_string());
    map.confidence = confidence;
}

/// Classe les maps restées génériques d'un fichier de génération précoce.
pub fn classify_early_generation(data: &[u8], layout: &Edc15pLayout, maps: &mut Vec<DetectedMap>) {
    let Some(rpm_id) = rpm_axis_id(data, maps) else {
        log::debug!("EDC15P early: no RPM axis id found, skipping early classification");
        return;
    };
    log::debug!(
        "EDC15P early: generation {:?}, RPM axis id 0x{:04X}",
        layout.generation,
        rpm_id
    );

    // Identifiant de l'axe IQ des durées d'injection, par bloc : premier axe
    // des maps de 570 octets dont le second axe est le régime.
    let mut duration_id: HashMap<usize, u16> = HashMap::new();
    // Adresse du SOI limiter par bloc (14 régimes × 11 températures) : la map
    // SOI principale le suit, les corrections SOI le précèdent.
    let mut soi_limiter: HashMap<usize, u32> = HashMap::new();
    for map in maps.iter() {
        let Some(block) = layout.block_index(map.address) else { continue };
        let (Some(xa), Some(ya)) = (map.x_axis_address, map.y_axis_address) else { continue };
        let (Some(first), Some(second)) = (read_axis_at(data, xa), read_axis_at(data, ya)) else { continue };
        if map.size == 570 && second.id == rpm_id {
            duration_id.entry(block).or_insert(first.id);
        }
        if map.size == 308
            && first.id == rpm_id
            && first.len() == 14
            && second.len() == 11
            && second.is_temperature()
        {
            soi_limiter.entry(block).or_insert(map.address);
        }
    }

    // « MAP linearisation » fantaisiste : sur le 019A la table de patterns
    // prenait un [02 00][02 00] du bloc pour une linéarisation (deux valeurs
    // 2, 2). Une vraie linéarisation porte [200, 3000] (mbar aux deux points
    // de l'axe [82, 951], 019GD 0x6F47C ; 019AJ 0x7850E : [200, 2600]).
    maps.retain(|m| {
        let is_lin = m
            .name
            .as_deref()
            .map_or(false, |n| n.to_lowercase().starts_with("map lineari"));
        if !is_lin || m.size != 4 {
            return true;
        }
        let v = read_values_u16(data, m.address, 2);
        let plausible = v.len() == 2 && (50..=600).contains(&v[0]) && (1000..=5000).contains(&v[1]) && v[1] > v[0];
        if !plausible {
            log::debug!("EDC15P early: dropping implausible MAP linearisation at 0x{:X} {:?}", m.address, v);
        }
        plausible
    });

    let mut soi_correction_count: HashMap<usize, usize> = HashMap::new();
    let mut egr_seen: HashMap<usize, bool> = HashMap::new();

    for map in maps.iter_mut() {
        if !is_generic(map) {
            continue;
        }
        let Some(block) = layout.block_index(map.address) else { continue };
        let Some(xa) = map.x_axis_address else { continue };
        let Some(first) = read_axis_at(data, xa) else { continue };
        let second = map.y_axis_address.and_then(|ya| read_axis_at(data, ya));

        // ── Courbes 1D ──
        let Some(second) = second else {
            if map.size == 32 && first.len() == 16 && first.id == rpm_id {
                let vals = read_values_u16(data, map.address, 16);
                if vals.iter().all(|&v| (3000..=10000).contains(&v)) {
                    set_common(map, "Boost actuator upper limit curve (N75)", "3-Turbo", 0.85);
                    map.description = Some("Actuator limit (%) | Axis: Engine speed (rpm)".to_string());
                    map.correction_factor = Some(0.01);
                    map.x_axis_correction = Some(1.0);
                }
            }
            continue;
        };

        let nf = first.len();
        let ns = second.len();
        if nf * ns * 2 != map.size {
            continue;
        }
        let first_is_rpm = first.id == rpm_id;
        log::debug!(
            "EDC15P early: generic 0x{:X} size {} axes {}x{} ids {:04X}/{:04X} rpm={} temp={} iq={} press={}",
            map.address, map.size, nf, ns, first.id, second.id, first_is_rpm,
            second.is_temperature(), second.is_iq(), second.is_pressure()
        );
        let dur_id = duration_id.get(&block).copied();
        let limiter = soi_limiter.get(&block).copied();

        match map.size {
            // 13x16 : EGR / N75 / SOI principale (formes identiques)
            416 if nf == 16 && ns == 13 && first_is_rpm && second.is_iq() => {
                let vals = read_values_u16(data, map.address, 208);
                let after_limiter = limiter.map_or(false, |l| map.address > l && map.address < l + 0x400);
                if after_limiter && vals.iter().all(|&v| (1500..=3800).contains(&v)) {
                    orient_standard(map, nf, ns);
                    set_common(map, "Start of injection (SOI)", "1-Fuel", 0.9);
                    map.description = Some("SOI | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                    map.unit = Some("deg".to_string());
                    map.correction_factor = Some(-0.023437);
                    map.offset = Some(78.0);
                    map.x_axis_correction = Some(0.01);
                    map.y_axis_correction = Some(1.0);
                } else if first.first() >= 400 {
                    orient_standard(map, nf, ns);
                    set_common(map, "N75 duty cycle", "3-Turbo", 0.9);
                    map.description = Some("Duty cycle % | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                    map.unit = Some("%".to_string());
                    map.correction_factor = Some(-0.01);
                    map.offset = Some(100.0);
                    map.x_axis_correction = Some(0.01);
                    map.y_axis_correction = Some(1.0);
                } else if vals.iter().all(|&v| v <= 12000) && !egr_seen.get(&block).copied().unwrap_or(false) {
                    orient_standard(map, nf, ns);
                    set_common(map, "EGR", "4-Misc", 0.9);
                    map.description = Some("EGR setpoint (mg/st) | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                    map.unit = Some("mg/st".to_string());
                    map.correction_factor = Some(0.1);
                    map.offset = Some(0.0);
                    map.x_axis_correction = Some(0.01);
                    map.y_axis_correction = Some(1.0);
                    egr_seen.insert(block, true);
                }
            }
            // 12/13 débits d'air × 16 régimes
            384 | 416 if nf == 16 && first_is_rpm && second.is_airflow() => {
                orient_standard(map, nf, ns);
                set_common(map, "IQ by MAF limiter", "2-Limiters", 0.9);
                map.description = Some("Max IQ by airflow | X: Airflow (mg/st) | Y: Engine speed (rpm)".to_string());
                map.unit = Some("Max IQ".to_string());
                map.correction_factor = Some(0.01);
                map.x_axis_correction = Some(0.1);
                map.y_axis_correction = Some(1.0);
            }
            // 10 pressions × 16 régimes
            320 if nf == 16 && ns == 10 && first_is_rpm && second.is_pressure() && second.first() >= 700 => {
                orient_standard(map, nf, ns);
                set_common(map, "IQ by MAP limiter", "2-Limiters", 0.9);
                map.description = Some("Max IQ by boost pressure | X: Boost (mbar) | Y: Engine speed (rpm)".to_string());
                map.unit = Some("Max IQ".to_string());
                map.correction_factor = Some(0.01);
                map.x_axis_correction = Some(1.0);
                map.y_axis_correction = Some(1.0);
            }
            // 16 pressions × 10 températures
            320 if nf == 16 && ns == 10 && first.is_pressure() && second.is_temperature() => {
                orient_standard(map, nf, ns);
                set_common(map, "Boost correction by temperature", "2-Limiters", 0.9);
                map.description = Some("Boost correction (mbar) | X: IAT (°C) | Y: Requested boost (mbar)".to_string());
                map.unit = Some("mbar".to_string());
                map.correction_factor = Some(1.0);
                map.x_axis_correction = Some(0.1);
                map.x_axis_offset = Some(-273.0);
                map.y_axis_correction = Some(1.0);
            }
            // 16 régimes × 10 IQ, données en mbar
            320 if nf == 16 && ns == 10 && first_is_rpm && second.is_iq() => {
                let vals = read_values_u16(data, map.address, 160);
                if vals.iter().filter(|&&v| (800..=3500).contains(&v)).count() > 100 {
                    orient_standard(map, nf, ns);
                    set_common(map, "Boost target map", "3-Turbo", 0.9);
                    map.description = Some("Target boost pressure (mbar) | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                    map.unit = Some("mbar".to_string());
                    map.correction_factor = Some(1.0);
                    map.x_axis_correction = Some(0.01);
                    map.y_axis_correction = Some(1.0);
                    map.x_label = Some("mg/st".to_string());
                    map.y_label = Some("rpm".to_string());
                    map.y_axis_inverted = Some(true);
                }
            }
            // 16 régimes × 8 positions pédale
            256 if nf == 16 && ns == 8 && first_is_rpm && second.is_pedal() => {
                orient_standard(map, nf, ns);
                set_common(map, "Driver wish", "4-Misc", 0.9);
                map.data_type = DataType::Int16;
                map.description = Some("Requested IQ (mg/st) | X: Throttle position (%) | Y: Engine speed (rpm)".to_string());
                map.unit = Some("mg/st".to_string());
                map.correction_factor = Some(0.01);
                map.x_axis_correction = Some(0.01);
                map.y_axis_correction = Some(1.0);
            }
            // 8-9 régimes × 8-10 températures (019GD : 9×10, 019AJ/019A : 8×9)
            144 | 160 | 180 if (nf == 8 || nf == 9) && (8..=10).contains(&ns) && first_is_rpm && second.is_temperature() => {
                orient_standard(map, nf, ns);
                set_common(map, "Start IQ", "4-Misc", 0.9);
                map.description = Some("Start IQ (mg/st) | X: Coolant temp (°C) | Y: Engine speed (rpm)".to_string());
                map.unit = Some("mg/st".to_string());
                map.correction_factor = Some(0.01);
                map.x_axis_correction = Some(0.1);
                map.x_axis_offset = Some(-273.1);
                map.y_axis_correction = Some(1.0);
                map.y_axis_offset = Some(0.0);
                map.x_label = Some("degC".to_string());
                map.y_label = Some("rpm".to_string());
            }
            // 8 régimes × 8 températures
            128 if nf == 8 && ns == 8 && first_is_rpm && second.is_temperature() => {
                orient_standard(map, nf, ns);
                set_common(map, "MAF correction by temperature", "4-Misc", 0.9);
                map.description = Some("MAF correction | X: IAT (°C) | Y: Engine speed (rpm)".to_string());
                map.correction_factor = Some(0.01);
                map.x_axis_correction = Some(0.1);
                map.x_axis_offset = Some(-273.1);
                map.y_axis_correction = Some(1.0);
                map.x_label = Some("°C".to_string());
                map.y_label = Some("rpm".to_string());
            }
            // 8 régimes × 8 IQ, données en Kelvin ×10
            128 if nf == 8 && ns == 8 && first_is_rpm && second.is_iq() => {
                let vals = read_values_u16(data, map.address, 64);
                if vals.iter().all(|&v| (2600..=4200).contains(&v)) {
                    orient_standard(map, nf, ns);
                    set_common(map, "Expected fuel temperature", "4-Misc", 0.85);
                    map.description = Some("Expected fuel temperature (°C) | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                    map.unit = Some("°C".to_string());
                    map.correction_factor = Some(0.1);
                    map.offset = Some(-273.0);
                    map.x_axis_correction = Some(0.01);
                    map.y_axis_correction = Some(1.0);
                    map.x_label = Some("mg/st".to_string());
                    map.y_label = Some("rpm".to_string());
                }
            }
            // 9 régimes × 8 IQ
            144 if nf == 9 && ns == 8 && first_is_rpm && second.is_iq() => {
                orient_standard(map, nf, ns);
                set_common(map, "Fuel volume correction", "1-Fuel", 0.85);
                map.data_type = DataType::Int16;
                map.description = Some("IQ correction per 100K | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                map.unit = Some("IQ correction per 100K".to_string());
                map.correction_factor = Some(0.002441);
                map.x_axis_correction = Some(0.01);
                map.y_axis_correction = Some(1.0);
                map.x_label = Some("mg/st".to_string());
                map.y_label = Some("rpm".to_string());
            }
            // Durées d'injection : IQ (identifiant des 570) × régime
            570 if Some(first.id) == dur_id && second.id == rpm_id => {
                orient_duration(map, nf, ns);
                set_common(map, &format!("Injector duration [0x{:06X}]", map.address), "1-Fuel", 0.9);
                map.unit = Some("Duration".to_string());
                map.correction_factor = Some(0.023437);
                map.x_axis_correction = Some(0.01);
                map.y_axis_correction = Some(1.0);
            }
            180 if nf == 9 && ns == 10 && Some(first.id) == dur_id && second.id == rpm_id => {
                orient_duration(map, nf, ns);
                set_common(map, &format!("Injector duration [0x{:06X}]", map.address), "1-Fuel", 0.9);
                map.unit = Some("Duration".to_string());
                map.correction_factor = Some(0.023437);
                map.x_axis_correction = Some(0.01);
                map.y_axis_correction = Some(1.0);
            }
            200 if nf == 10 && ns == 10 && Some(first.id) == dur_id && second.id == rpm_id => {
                orient_standard(map, nf, ns);
                set_common(map, "Injector duration 00", "1-Fuel", 0.9);
                map.unit = Some("Duration".to_string());
                map.correction_factor = Some(0.023437);
                map.x_axis_correction = Some(0.01);
                map.y_axis_correction = Some(1.0);
            }
            // 10 pressions ambiantes × 10 régimes, données en mbar
            200 if nf == 10 && ns == 10 && first.is_pressure() && second.id == rpm_id => {
                let vals = read_values_u16(data, map.address, 100);
                if vals.iter().all(|&v| (800..=3500).contains(&v)) {
                    orient_standard(map, nf, ns);
                    set_common(map, "Boost limit map", "2-Limiters", 0.85);
                    map.description = Some("Max boost (mbar) | X: Engine speed (rpm) | Y: Atm. pressure (mbar)".to_string());
                    map.unit = Some("Max boost".to_string());
                    map.correction_factor = Some(1.0);
                    map.x_axis_correction = Some(1.0);
                    map.y_axis_correction = Some(1.0);
                }
            }
            // 10 régimes × 10 IQ (jusqu'à 6000+), données en mbar
            200 if nf == 10 && ns == 10 && first_is_rpm && second.increasing() && second.first() <= 600 && second.last() >= 4000 => {
                let vals = read_values_u16(data, map.address, 100);
                if vals.iter().all(|&v| (1500..=4500).contains(&v)) {
                    orient_standard(map, nf, ns);
                    set_common(map, "Limit of overboost protection", "2-Limiters", 0.85);
                    map.description = Some("Boost limit (mbar) | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                    map.unit = Some("Boost limit".to_string());
                    map.correction_factor = Some(1.0);
                    map.x_axis_correction = Some(0.01);
                    map.y_axis_correction = Some(1.0);
                } else if limiter.map_or(false, |l| map.address + 0x400 > l && map.address < l) && second.is_iq_loose() {
                    soi_correction(map, nf, ns, block, &mut soi_correction_count, data);
                }
            }
            // 14 régimes × 11 températures
            308 if nf == 14 && ns == 11 && first_is_rpm && second.is_temperature() => {
                orient_standard(map, nf, ns);
                set_common(map, "SOI limiter", "2-Limiters", 0.9);
                map.description = Some("SOI limit (deg) | X: Coolant temp (°C) | Y: Engine speed (rpm)".to_string());
                map.unit = Some("SOI limit".to_string());
                map.correction_factor = Some(-0.023437);
                map.offset = Some(78.0);
                map.x_axis_correction = Some(0.1);
                map.x_axis_offset = Some(-273.1);
                map.y_axis_correction = Some(1.0);
            }
            // Corrections SOI (régime × IQ, degrés signés) juste avant le limiteur
            196 | 80 if first_is_rpm && second.is_iq_loose()
                && limiter.map_or(false, |l| map.address + 0x400 > l && map.address < l) => {
                soi_correction(map, nf, ns, block, &mut soi_correction_count, data);
            }
            _ => {}
        }
    }
}

fn soi_correction(
    map: &mut DetectedMap,
    nf: usize,
    ns: usize,
    block: usize,
    counter: &mut HashMap<usize, usize>,
    data: &[u8],
) {
    let vals = read_values_i16(data, map.address, nf * ns);
    if !vals.iter().all(|&v| (-3000..=3000).contains(&v)) {
        return;
    }
    let n = counter.entry(block).or_insert(0);
    *n += 1;
    orient_standard(map, nf, ns);
    set_common(map, &format!("Start of injection correction {}", n), "1-Fuel", 0.8);
    map.data_type = DataType::Int16;
    map.description = Some("SOI correction (deg) | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
    map.unit = Some("deg".to_string());
    map.correction_factor = Some(0.023437);
    map.offset = Some(0.0);
    map.x_axis_correction = Some(0.01);
    map.y_axis_correction = Some(1.0);
}

/// Hystérésis EGR : deux courbes de 20 points sur l'axe régime, contiguës
/// dans le fichier (seuils d'IQ d'activation et de coupure de l'EGR,
/// centièmes de mg/coup, ex. 27.8 / 28.0 mg). Vérifié sur 019GD (0x75ABA /
/// 0x75B0E), 019AJ (0x7208C / 0x720E0) et 019A (0x7AD70 / 0x7ADC4) : la
/// seconde courbe suit la première de 84 octets (40 de données + en-tête +
/// axe de 20 valeurs). Mêmes courbes que sur l'EDC15VM (axe DD).
pub fn detect_egr_hysteresis(data: &[u8], layout: &Edc15pLayout, maps: &mut Vec<DetectedMap>) {
    let Some(rpm_id) = rpm_axis_id(data, maps) else { return };
    if maps.iter().any(|m| m.name.as_deref().map_or(false, |n| n.starts_with("EGR hysteresis"))) {
        return;
    }

    // Candidats : courbes génériques 1×20 sur l'axe régime, données signées plausibles
    let mut candidates: Vec<(usize, u32, usize)> = Vec::new(); // (bloc, adresse, index)
    for (idx, map) in maps.iter().enumerate() {
        if !is_generic(map) || map.size != 40 {
            continue;
        }
        let dims_ok = matches!(
            map.dimensions,
            MapDimensions::TwoDimensional { rows: 1, cols: 20 } | MapDimensions::TwoDimensional { rows: 20, cols: 1 }
        );
        if !dims_ok {
            continue;
        }
        let Some(block) = layout.block_index(map.address) else { continue };
        let Some(xa) = map.x_axis_address else { continue };
        let Some(axis) = read_axis_at(data, xa) else { continue };
        if axis.id != rpm_id || axis.len() != 20 || !axis.increasing() || !(2500..=6500).contains(&axis.last()) {
            continue;
        }
        let vals = read_values_i16(data, map.address, 20);
        if !vals.iter().all(|&v| (-100..=12000).contains(&v)) {
            continue;
        }
        candidates.push((block, map.address, idx));
    }
    candidates.sort_by_key(|c| (c.0, c.1));

    let mut done_blocks = std::collections::HashSet::new();
    let mut renames: Vec<(usize, String)> = Vec::new();
    for w in candidates.windows(2) {
        let (b1, a1, i1) = w[0];
        let (b2, a2, i2) = w[1];
        if b1 != b2 || done_blocks.contains(&b1) || a2 != a1 + 84 {
            continue;
        }
        renames.push((i1, "EGR hysteresis 1".to_string()));
        renames.push((i2, "EGR hysteresis 2".to_string()));
        done_blocks.insert(b1);
    }
    for (idx, name) in renames {
        let map = &mut maps[idx];
        log::debug!("EDC15P: {} at 0x{:X}", name, map.address);
        set_common(map, &name, "4-Misc", 0.85);
        map.data_type = DataType::Int16;
        map.description = Some("EGR IQ threshold (mg/st) | Axis: Engine speed (rpm)".to_string());
        map.unit = Some("mg/st".to_string());
        map.correction_factor = Some(0.01);
        map.offset = Some(0.0);
        map.x_axis_correction = Some(1.0);
        map.x_axis_offset = Some(0.0);
        map.x_label = Some("rpm".to_string());
        map.dimensions = MapDimensions::TwoDimensional { rows: 1, cols: 20 };
    }
}
