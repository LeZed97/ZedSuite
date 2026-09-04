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
            Some(ECUType::EDC16U1) | Some(ECUType::EDC16U31) | Some(ECUType::EDC16U34)
        );

        // Use smart detector (identifies ECU type first, then uses appropriate patterns)
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
        ) || forced_is_edc16;

        Self::finalize_maps(result.maps, is_edc16)
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
    fn finalize_maps(maps: Vec<DetectedMap>, is_edc16: bool) -> Vec<DetectedMap> {
        let mut out: Vec<DetectedMap> = Vec::with_capacity(maps.len());
        for mut map in maps {
            if let Some(name) = map.name.take() {
                map.name = Some(
                    name.replace("Linearization", "Linearisation")
                        .replace("linearization", "linearisation"),
                );
            }
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
