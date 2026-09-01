use crate::models::DetectedMap;
use crate::detector::ecu_identifier::{ECUIdentifier, ECUIdentification, ECUType};
use crate::detector::ecu::bosch::{EDC15PDetector, EDC15VMDetector, EDC16U1Detector, EDC16U31Detector, EDC16U34Detector};
use crate::detector::ecu::bosch::edc16u34::EDC16Variant;
use crate::detector::ecu::bosch::edc16u31::EDC16Variant as EDC16U31Variant;

/// Smart detector that adapts to different ECU types
/// Routes to specialized detectors based on ECU identification.
/// To add support for a new ECU family: implement a detector under
/// detector/ecu/<manufacturer>/, identify it in ecu_identifier.rs, then
/// add a routing arm in detect_maps_with_options below.
pub struct SmartDetector {
    edc15p_detector: EDC15PDetector,
    edc15vm_detector: EDC15VMDetector,
}

impl SmartDetector {
    pub fn new() -> Self {
        Self {
            edc15p_detector: EDC15PDetector::new(),
            edc15vm_detector: EDC15VMDetector::new(),
        }
    }

    /// Detection method with tuned mode support
    /// tuned_mode: Enable extended value ranges for modified/tuned files
    /// forced_type: When the caller already knows the ECU type (e.g. the
    /// project stores it), it overrides the automatic identification.
    pub fn detect_maps_with_options(&self, data: &[u8], tuned_mode: bool, forced_type: Option<ECUType>) -> DetectionResult {
        let start_time = std::time::Instant::now();

        // Step 1: Identify ECU type
        let mut ecu_id = ECUIdentifier::identify(data);
        if let Some(forced) = forced_type {
            if ecu_id.ecu_type != forced {
                log::info!("🔧 ECU type forced to {:?} by caller (auto-identified as {:?})",
                           forced, ecu_id.ecu_type);
                ecu_id.ecu_type = forced;
            }
        }

        log::debug!("🔍 ECU Identified: {:?} {:?} (confidence: {:.2}%)",
                 ecu_id.manufacturer, ecu_id.ecu_type, ecu_id.confidence * 100.0);
        if tuned_mode {
            log::debug!("📊 Tuned mode ENABLED - using extended value ranges");
        }

        // Step 2: Use ECU-specific detector based on identified type
        let detected_maps = match ecu_id.ecu_type {
            // EDC15VM - route to dedicated EDC15VM detector
            ECUType::EDC15VM => {
                log::debug!("📋 Using EDC15VM specialized detector");
                self.edc15vm_detector.detect(data)
            }
            // EDC15P family - route to EDC15P detector
            ECUType::EDC15P | ECUType::EDC15M | ECUType::EDC15V | ECUType::EDC15C => {
                log::debug!("📋 Using EDC15P specialized detector");
                self.edc15p_detector.detect(data)
            }
            // EDC16U1 - route to dedicated EDC16U1 detector
            ECUType::EDC16U1 => {
                log::debug!("📋 Using EDC16U1 specialized detector");
                self.detect_edc16u1(data, tuned_mode)
            }
            // EDC16U31 - route to dedicated EDC16U31 detector
            ECUType::EDC16U31 => {
                log::debug!("📋 Using EDC16U31 specialized detector");
                self.detect_edc16u31(data, tuned_mode)
            }
            // EDC16U34 - route to dedicated EDC16U34 detector
            ECUType::EDC16U34 => {
                log::debug!("📋 Using EDC16U34 specialized detector");
                self.detect_edc16u34(data, EDC16Variant::EDC16U34, tuned_mode)
            }
            // Other EDC16 variants - no detection for now (return empty)
            ECUType::EDC16U | ECUType::EDC16C | ECUType::EDC16CP => {
                log::debug!("⚠️ No specialized detector for {:?} yet, returning empty", ecu_id.ecu_type);
                Vec::new()
            }
            _ => {
                // No specialized detector available
                log::debug!("⚠️ No specialized detector for {:?}, returning empty", ecu_id.ecu_type);
                Vec::new()
            }
        };

        let elapsed = start_time.elapsed();

        log::debug!("✅ Detection complete: {} maps found in {:?}",
                 detected_maps.len(), elapsed);

        DetectionResult {
            ecu_identification: ecu_id,
            maps: detected_maps,
            processing_time_ms: elapsed.as_millis(),
        }
    }

    /// EDC16U31 detection using the specialized detector
    fn detect_edc16u31(&self, data: &[u8], tuned_mode: bool) -> Vec<DetectedMap> {
        let detector = if tuned_mode {
            EDC16U31Detector::new_tuned(EDC16U31Variant::EDC16U31)
        } else {
            EDC16U31Detector::new(EDC16U31Variant::EDC16U31)
        };
        let maps = detector.detect(data);
        log::debug!("   EDC16U31 detector found {} maps (tuned_mode: {})", maps.len(), tuned_mode);

        maps
    }

    /// EDC16U1 detection using the specialized detector
    fn detect_edc16u1(&self, data: &[u8], tuned_mode: bool) -> Vec<DetectedMap> {
        let detector = if tuned_mode {
            EDC16U1Detector::new_tuned()
        } else {
            EDC16U1Detector::new()
        };
        let maps = detector.detect(data);
        log::debug!("   EDC16U1 detector found {} maps (tuned_mode: {})", maps.len(), tuned_mode);

        maps
    }

    /// EDC16U34 detection using the specialized detector
    fn detect_edc16u34(&self, data: &[u8], variant: EDC16Variant, tuned_mode: bool) -> Vec<DetectedMap> {
        let detector = if tuned_mode {
            EDC16U34Detector::new_tuned(variant)
        } else {
            EDC16U34Detector::new(variant)
        };
        let maps = detector.detect(data);
        log::debug!("   EDC16U34 detector found {} maps (tuned_mode: {})", maps.len(), tuned_mode);

        maps
    }
}

impl Default for SmartDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Detection result with ECU identification
#[derive(Debug)]
pub struct DetectionResult {
    pub ecu_identification: ECUIdentification,
    pub maps: Vec<DetectedMap>,
    pub processing_time_ms: u128,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_detector_creation() {
        let detector = SmartDetector::new();
        assert!(true);
    }

}
