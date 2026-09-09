use crate::models::DetectedMap;

// Detection modules
pub mod ecu_identifier;
pub mod smart_detector;
pub mod ecu;

// Public exports
pub use ecu_identifier::{ECUIdentifier, ECUType};
pub use smart_detector::SmartDetector;

/// Main map detection engine with intelligent ECU identification
/// This is the CORE ALGORITHM - now with multi-ECU support
pub struct MapDetector {
    smart_detector: SmartDetector,
}

impl MapDetector {
    pub fn new() -> Self {
        Self {
            smart_detector: SmartDetector::new(),
        }
    }

    /// Parse a caller-provided ECU type string (as stored in projects /
    /// sent by the web app) into a concrete ECUType.
    fn parse_ecu_type(s: &str) -> Option<ECUType> {
        match s.trim().to_uppercase().as_str() {
            "EDC16U31" => Some(ECUType::EDC16U31),
            "EDC16U34" => Some(ECUType::EDC16U34),
            "EDC16U1" => Some(ECUType::EDC16U1),
            "EDC16CP31" => Some(ECUType::EDC16CP31),
            "EDC15P" => Some(ECUType::EDC15P),
            "EDC15VM" => Some(ECUType::EDC15VM),
            "EDC15V" => Some(ECUType::EDC15V),
            "EDC15M" => Some(ECUType::EDC15M),
            "EDC15C" => Some(ECUType::EDC15C),
            _ => None,
        }
    }

    /// Detection method with tuned mode support
    /// tuned_mode: Enable extended value ranges for modified/tuned files
    /// ecu_type: optional explicit type from the caller — when valid it
    /// overrides the automatic identification (the auto-identifier cannot
    /// always tell U31/U34 apart on unusual firmware layouts).
    pub fn detect_maps_with_options(&self, data: &[u8], ecu_type: Option<&str>, tuned_mode: bool) -> Vec<DetectedMap> {
        log::debug!("🔍 Starting intelligent map detection on {} bytes (tuned_mode: {}, ecu_type: {:?})",
                    data.len(), tuned_mode, ecu_type);

        let forced = ecu_type.and_then(Self::parse_ecu_type);
        let forced_is_edc16 = matches!(
            forced,
            Some(ECUType::EDC16U1)
                | Some(ECUType::EDC16U31)
                | Some(ECUType::EDC16U34)
                | Some(ECUType::EDC16CP31)
        );

        // Use smart detector (identifies ECU type first, then uses appropriate patterns)
        let forced_for_family = forced.clone();
        let result = self.smart_detector.detect_maps_with_options(data, tuned_mode, forced);

        log::debug!("🎯 Smart Detection Results:");
        log::debug!("   ECU: {:?} {:?}", result.ecu_identification.manufacturer, result.ecu_identification.ecu_type);
        log::debug!("   Confidence: {:.1}%", result.ecu_identification.confidence * 100.0);
        log::debug!("   Maps found: {}", result.maps.len());
        log::debug!("   Processing time: {}ms", result.processing_time_ms);
        if tuned_mode {
            log::debug!("   Mode: TUNED (extended value ranges)");
        }

        let is_edc16 = matches!(
            result.ecu_identification.ecu_type,
            ECUType::EDC16U1
                | ECUType::EDC16U31
                | ECUType::EDC16U34
                | ECUType::EDC16U
                | ECUType::EDC16C
                | ECUType::EDC16CP
                | ECUType::EDC16CP31
        ) || forced_is_edc16;

        // Famille pour la normalisation des axes : le type forcé par l'appelant
        // prime, sinon l'identification automatique.
        let family = match forced_for_family.unwrap_or(result.ecu_identification.ecu_type.clone()) {
            ECUType::EDC15P | ECUType::EDC15V => "EDC15P",
            ECUType::EDC15VM => "EDC15VM",
            _ if is_edc16 => "EDC16",
            _ => "",
        };

        Self::finalize_maps(result.maps, is_edc16, family)
    }

    /// Libellés, facteurs et unités d'axes : corrections centralisées, par
    /// famille et par nom de map, de ce que les détecteurs émettent de faux
    /// ou d'incomplet (audit du 08/09/2026 sur un fichier par famille). Ne
    /// touche ni aux adresses ni aux dimensions : seulement ce qui est
    /// affiché. Les libellés suivent la forme « Grandeur (unité) », l'unité
    /// entre parenthèses étant celle que l'afficheur reprend dans le coin.
    fn normalise_axis_metadata(map: &mut DetectedMap, family: &str) {
        let name = map.name.clone().unwrap_or_default();
        let lower = name.to_ascii_lowercase();
        let x_label = map.x_label.clone().unwrap_or_default();
        let y_label = map.y_label.clone().unwrap_or_default();
        let is_edc16 = family == "EDC16";

        // MAP linearisation (EDC15P / EDC15VM) : deux points « pression à
        // une tension capteur ». L'axe est brut en pas de convertisseur
        // 10 bits (82 = 0,40 V, 989 = 4,83 V), affiché en millivolts.
        if lower == "map linearisation" {
            map.x_label = Some("Sensor voltage (mV)".to_string());
            map.x_axis_correction = Some(5000.0 / 1024.0);
            map.x_axis_offset = Some(0.0);
            if map.unit.as_deref().unwrap_or("").is_empty() {
                map.unit = Some("mbar".to_string());
            }
            map.description = Some(
                "Boost pressure read at two sensor voltages | X: Sensor voltage (mV)".to_string(),
            );
        }

        // Débit d'air en abscisse des limiteurs de fumée : « mg/st » seul ne
        // dit pas ce que c'est.
        let airflow_x = lower.starts_with("smoke limiter")
            || lower.starts_with("iq by maf limiter");
        if airflow_x && (x_label == "mg/st" || x_label == "mg/stroke") {
            map.x_label = Some(if is_edc16 { "Airflow (mg/stroke)" } else { "Airflow (mg/st)" }.to_string());
        }

        match family {
            "EDC15VM" => {
                // Correction de suralimentation par température : l'axe X est
                // la température d'admission (bruts 2531..3431, dixièmes de
                // kelvin) et l'axe Y la pression demandée (500..2500 mbar) ;
                // l'annotation par identifiant les affichait en mg/st ×0.01.
                if lower == "boost correction by temperature" {
                    map.x_label = Some("IAT (°C)".to_string());
                    map.x_axis_correction = Some(0.1);
                    map.x_axis_offset = Some(-273.1);
                    map.y_label = Some("Requested boost (mbar)".to_string());
                    map.y_axis_correction = Some(1.0);
                    map.y_axis_offset = Some(0.0);
                    map.unit = Some("mbar".to_string());
                    map.description = Some(
                        "Boost correction (mbar) | X: IAT (°C) | Y: Requested boost (mbar)".to_string(),
                    );
                }
                // N75 : sur certains logiciels (012GN) l'axe X sortait sans
                // libellé et brut (0..4500) ; c'est la quantité injectée ×0.01
                // comme sur le P et sur le 012M.
                if lower == "n75 duty cycle" {
                    if x_label.is_empty() || map.x_axis_correction.unwrap_or(1.0) == 1.0 {
                        map.x_label = Some("IQ (mg/st)".to_string());
                        map.x_axis_correction = Some(0.01);
                        map.x_axis_offset = Some(0.0);
                    } else if x_label == "mg/st" {
                        map.x_label = Some("IQ (mg/st)".to_string());
                    }
                    if y_label.is_empty() || y_label == "rpm" {
                        map.y_label = Some("Engine speed (rpm)".to_string());
                    }
                    if map.unit.as_deref().unwrap_or("").is_empty() {
                        map.unit = Some("%".to_string());
                    }
                }
                // Unités manquantes, alignées sur l'EDC15P
                if map.unit.as_deref().unwrap_or("").is_empty()
                    && (lower == "driver wish"
                        || lower == "start iq"
                        || lower.starts_with("iq by map limiter")
                        || lower.starts_with("iq by maf limiter")
                        || lower == "torque limiter")
                {
                    map.unit = Some("mg/st".to_string());
                }
            }
            "EDC15P" => {
                // Courbe 1×16 sur l'axe régime, sans libellé
                if lower.starts_with("boost actuator upper limit curve") && x_label.is_empty() {
                    map.x_label = Some("Engine speed (rpm)".to_string());
                }
            }
            "EDC16" => {
                // Limiteur de couple principal (20×3 / 22×4) : aucun libellé,
                // X = régime, Y = pression atmosphérique
                if lower == "torque limiter" && x_label.is_empty() && y_label.is_empty() {
                    map.x_label = Some("Engine speed (rpm)".to_string());
                    map.y_label = Some("Atm pressure (mbar)".to_string());
                }
                // SOI limiter : l'axe X est la température d'eau (bruts
                // 2531..3521 en dixièmes de kelvin), pas des degrés vilebrequin
                if lower == "soi limiter" {
                    map.x_label = Some("Water temp (°C)".to_string());
                }
                // N75 : quantité injectée en abscisse, même forme que sur EDC15
                if lower.starts_with("n75 duty cycle") && x_label == "mg/stroke" {
                    map.x_label = Some("IQ (mg/stroke)".to_string());
                }
            }
            _ => {}
        }
    }

    /// Post-traitement commun à toutes les familles, appliqué à la sortie de
    /// chaque détecteur (choix d'affichage) :
    /// - orthographe « linearisation » dans tous les noms de maps (jamais
    ///   « linearization ») ;
    /// - maps masquées sur tous les calculateurs : « Inverse driver wish »
    ///   et « MAF linearisation » ;
    /// - EDC16 : seules les durations 00-05 et le Duration Selector sont
    ///   affichés — « Duration 06+ », « Duration (Dynamic) » et « Duration
    ///   min. injection break after main injection » sont masquées (comme
    ///   sur EDC15, où seules les 6 durations existent) ;
    /// - (les trois maps BIP EDC16 sont affichées depuis la 1.1.3, dossier
    ///   Injection system — demande des utilisateurs) ;
    /// - EDC16 : dossier « Fuel Correction » reversé dans « Injection
    ///   system », dossiers « Airflow » et « DPF » reversés dans « Other ».
    fn finalize_maps(maps: Vec<DetectedMap>, is_edc16: bool, family: &str) -> Vec<DetectedMap> {
        let mut out: Vec<DetectedMap> = Vec::with_capacity(maps.len());
        for mut map in maps {
            if let Some(name) = map.name.take() {
                map.name = Some(
                    name.replace("Linearization", "Linearisation")
                        .replace("linearization", "linearisation"),
                );
            }
            // Après l'orthographe : la normalisation des axes travaille sur le
            // nom définitif
            Self::normalise_axis_metadata(&mut map, family);
            let lower = map.name.as_deref().unwrap_or("").to_ascii_lowercase();

            // Masquées partout
            if (lower.contains("inverse") && lower.contains("driver"))
                || lower.contains("maf linearisation")
            {
                continue;
            }

            // EDC16 : ne garder que Duration 00-05 + Duration Selector
            if is_edc16 && lower.starts_with("duration") {
                let keep = lower.starts_with("duration selector")
                    || (lower.starts_with("duration 0")
                        && lower
                            .as_bytes()
                            .get(10)
                            .map_or(false, |c| (b'0'..=b'5').contains(c)));
                if !keep {
                    continue;
                }
            }

            // EDC16 : reclassement des dossiers
            if is_edc16 {
                if let Some(cat) = map.category.as_deref() {
                    if cat.eq_ignore_ascii_case("fuel correction") {
                        map.category = Some("Injection system".to_string());
                    } else if cat.eq_ignore_ascii_case("airflow") || cat.eq_ignore_ascii_case("dpf")
                    {
                        map.category = Some("Other".to_string());
                    }
                }
            }

            out.push(map);
        }
        // EDC16 : les SOI dynamiques sortent des trois détecteurs sous
        // plusieurs noms (« Start of injection Dynamic », « (Dynamic) »,
        // « (dynamic) 01 » sur l'U1…). Un seul nom pour tous, numéroté par
        // adresse croissante : « Start of injection (Dynamic) 01 », « 02 »…
        if is_edc16 {
            let is_dynamic_soi = |name: &str| {
                let l = name.to_ascii_lowercase();
                l.starts_with("start of injection") && l.contains("dynamic")
            };
            let mut idx: Vec<usize> = out
                .iter()
                .enumerate()
                .filter(|(_, m)| m.name.as_deref().map_or(false, is_dynamic_soi))
                .map(|(i, _)| i)
                .collect();
            idx.sort_by_key(|&i| out[i].address);
            for (n, &i) in idx.iter().enumerate() {
                out[i].name = Some(format!("Start of injection (Dynamic) {:02}", n + 1));
            }
        }
        out
    }

}

impl Default for MapDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_detector_creation() {
        let detector = MapDetector::new();
        // Should not panic
    }

}
