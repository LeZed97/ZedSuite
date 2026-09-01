use serde::{Deserialize, Serialize};

/// ECU identification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ECUIdentification {
    pub manufacturer: ECUManufacturer,
    pub ecu_type: ECUType,
    pub variant: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub part_number: Option<String>,
    pub confidence: f32,
}

/// ECU Manufacturers
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ECUManufacturer {
    Bosch,
    Siemens,
    Delphi,
    Magneti,
    Denso,
    Continental,
    Valeo,
    Hitachi,
    ACDelco,
    Unknown,
}

impl ECUManufacturer {
    /// Full brand name shown to users (the Debug name is a short identifier).
    pub fn display_name(&self) -> &'static str {
        match self {
            ECUManufacturer::Bosch => "Bosch",
            ECUManufacturer::Siemens => "Siemens",
            ECUManufacturer::Delphi => "Delphi",
            ECUManufacturer::Magneti => "Magneti Marelli",
            ECUManufacturer::Denso => "Denso",
            ECUManufacturer::Continental => "Continental",
            ECUManufacturer::Valeo => "Valeo",
            ECUManufacturer::Hitachi => "Hitachi",
            ECUManufacturer::ACDelco => "ACDelco",
            ECUManufacturer::Unknown => "Unknown",
        }
    }
}

/// ECU Types (organized by manufacturer)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ECUType {
    // Bosch EDC15 family (Diesel) - 512KB files only
    EDC15P,      // VAG (VW, Audi, Seat, Skoda) - MPC555/556
    EDC15C,      // PSA (Peugeot, Citroën)
    EDC15M,      // BMW, Rover
    EDC15V,      // Volvo
    EDC15VM,     // Various

    // Bosch EDC16 family (Diesel) - VAG specific variants
    EDC16U1,     // VAG 1.9/2.0/2.5 TDI - MPC556LF - 512KB/1MB
    EDC16U31,    // VAG 1.9/2.0/2.5 TDI PD - MPC563 - 512KB/1MB
    EDC16U34,    // VAG 1.9/2.0 TDI transversal - MPC562 - 2MB
    EDC16U,      // VAG generic (fallback)
    EDC16C,      // PSA
    EDC16CP,     // PSA newer

    // Bosch EDC17 family (Diesel)
    EDC17C,
    EDC17CP,
    EDC17U,

    // Bosch ME7 family (Gasoline)
    ME7_1,
    ME7_5,
    ME7_8,
    ME7_9,

    // Bosch MED family (Gasoline)
    MED9_1,
    MED9_5,
    MED17_1,
    MED17_5,

    // Siemens/Continental
    SID201,
    SID803,
    SID807,
    SIMOS,

    // Delphi
    DCM3_5,
    DCM3_7,

    // Denso
    SH705x,

    Unknown,
}

/// ECU Identifier - detects ECU type from binary data
pub struct ECUIdentifier;

impl ECUIdentifier {
    /// Maximum bytes to scan for identification
    const MAX_SCAN_BYTES: usize = 100_000;

    /// Exact dump sizes of the supported Bosch families:
    /// EDC15P/EDC15VM = 512KB, EDC16U1/U31 = 512KB or 1MB,
    /// EDC16U34 (and 2MB U31 hybrids) = 2MB.
    /// Anything else is rejected up front — the first guard against foreign
    /// files being misread as a supported ECU.
    const SUPPORTED_SIZES: [usize; 3] = [524_288, 1_048_576, 2_097_152];

    /// Identify ECU from binary data using multiple detection methods.
    /// STRICT by design: a file is only identified as a supported ECU when it
    /// carries positive evidence (Bosch HW prefix, family strings, structural
    /// signatures). Size alone NEVER decides — a random 2MB dump from another
    /// manufacturer must come back Unknown, not EDC16.
    pub fn identify(data: &[u8]) -> ECUIdentification {
        log::debug!("🔍 Starting ECU identification on {} bytes", data.len());

        // Quick validation: only the exact dump sizes of supported families
        if !Self::SUPPORTED_SIZES.contains(&data.len()) {
            log::warn!("⚠️  Unsupported file size ({} bytes)", data.len());
            return Self::unknown_ecu(0.10);
        }

        if Self::is_text_file(data) {
            log::warn!("⚠️  File appears to be text");
            return Self::unknown_ecu(0.05);
        }

        // Negative gate: newer Bosch families (EDC17/MED17) share the
        // 0281/1037 ASCII metadata of the EDC16 and also ship 2MB dumps.
        // Identify them explicitly so they can never pass for an EDC16.
        if let Some(id) = Self::identify_unsupported_bosch(data) {
            log::debug!("🚫 Unsupported Bosch family detected: {:?}", id.ecu_type);
            return id;
        }

        // Try detection methods in order of reliability

        // 1. Hardware prefix detection (most reliable for Bosch)
        log::debug!("Method 1: Checking hardware prefixes...");
        if let Some(id) = Self::identify_by_hardware_prefix(data) {
            log::debug!("✅ Identified by HW prefix: {:?} {:?} (confidence: {:.2}%)", 
                      id.manufacturer, id.ecu_type, id.confidence * 100.0);
            return id;
        }
        
        // 2. ASCII strings
        log::debug!("Method 2: Checking ASCII strings...");
        if let Some(id) = Self::identify_by_ascii_strings(data) {
            log::debug!("✅ Identified by ASCII: {:?} {:?} (confidence: {:.2}%)", 
                      id.manufacturer, id.ecu_type, id.confidence * 100.0);
            return id;
        }
        
        // 3. Part numbers
        log::debug!("Method 3: Checking part numbers...");
        if let Some(id) = Self::identify_by_part_number(data) {
            log::debug!("✅ Identified by part number: {:?} {:?} (confidence: {:.2}%)", 
                      id.manufacturer, id.ecu_type, id.confidence * 100.0);
            return id;
        }
        
        // 4. Binary patterns and structure
        log::debug!("Method 4: Checking binary patterns...");
        if let Some(id) = Self::identify_by_binary_patterns(data) {
            log::debug!("✅ Identified by binary patterns: {:?} {:?} (confidence: {:.2}%)", 
                      id.manufacturer, id.ecu_type, id.confidence * 100.0);
            return id;
        }
        
        // 5. File structure
        log::debug!("Method 5: Checking file structure...");
        if let Some(id) = Self::identify_by_structure(data) {
            log::debug!("✅ Identified by structure: {:?} {:?} (confidence: {:.2}%)", 
                      id.manufacturer, id.ecu_type, id.confidence * 100.0);
            return id;
        }
        
        // 6. Heuristic fallback
        log::warn!("⚠️  Using heuristics");
        Self::identify_by_heuristics(data)
    }
    
    /// Negative gate: identify newer Bosch families that are NOT supported
    /// but share the EDC16's ASCII metadata (0281 HW numbers, 1037 SW
    /// numbers, VAG part numbers) and dump sizes. Without this check a 2MB
    /// EDC17 dump would sail through the EDC16 heuristics.
    /// The family strings ("EDC17C46", "MED17.5", ...) are present in every
    /// firmware of these families and never appear in EDC15/EDC16 dumps.
    fn identify_unsupported_bosch(data: &[u8]) -> Option<ECUIdentification> {
        let families: [(&[u8], ECUType, &str); 2] = [
            (b"EDC17", ECUType::EDC17C, "EDC17 (not supported)"),
            (b"MED17", ECUType::MED17_1, "MED17 (not supported)"),
        ];
        for (needle, ecu_type, variant) in families {
            if Self::contains_sequence(data, needle) {
                return Some(ECUIdentification {
                    manufacturer: ECUManufacturer::Bosch,
                    ecu_type,
                    variant: Some(variant.to_string()),
                    software_version: None,
                    hardware_version: None,
                    part_number: None,
                    confidence: 0.85,
                });
            }
        }
        None
    }

    /// Check if file is text
    fn is_text_file(data: &[u8]) -> bool {
        let sample_size = std::cmp::min(1000, data.len());
        let sample = &data[0..sample_size];
        let printable_count = sample.iter()
            .filter(|&&b| (b >= 0x20 && b <= 0x7E) || b == 0x0A || b == 0x0D)
            .count();
        (printable_count as f32 / sample_size as f32) > 0.80
    }
    
    /// Method 1: Identify by hardware prefix (Bosch uses 0261/0265 for petrol, 0281 for diesel)
    fn identify_by_hardware_prefix(data: &[u8]) -> Option<ECUIdentification> {
        let scan_limit = std::cmp::min(Self::MAX_SCAN_BYTES, data.len());
        let file_size = data.len();

        // Search for Bosch hardware prefixes
        // Diesel: 0281 (hex: 30 32 38 31)
        // Petrol: 0261, 0265 (hex: 30 32 36 31/35)

        // Look for "0281" pattern (Bosch diesel)
        if Self::contains_hex_pattern(&data[0..scan_limit], &[0x30, 0x32, 0x38, 0x31]) {
            log::debug!("Found Bosch diesel HW prefix: 0281");

            // Extract HW number to determine ECU variant
            let hw_number = Self::extract_bosch_hw_number_full(data);
            let hw_prefix = hw_number.as_ref().map(|h| &h[..7]).unwrap_or("");

            // CRITICAL: File size determines ECU family
            // EDC15P: ONLY 512KB (524288 bytes)
            // EDC16U1/U31: 512KB or 1MB (524288 or 1048576 bytes)
            // EDC16U34: 2MB (2097152 bytes)

            if file_size == 524288 {
                // 512KB - Could be EDC15P, EDC15VM, or EDC16U1/U31
                // Check for EDC15 specific patterns first. The V4.1 codeblock
                // signature is present in every EDC15P/VM dump and is what
                // separates a real EDC15 from a foreign 512KB Bosch file.
                if Self::has_edc15_characteristics(data) && Self::has_v41_signature(data) {
                    let sw_sg_result = Self::extract_vag_sw_number(data);
                    let software_version = match sw_sg_result {
                        Some((sw, Some(sg))) => Some(format!("{} {}", sw, sg)),
                        Some((sw, None)) => Some(sw),
                        None => None,
                    };

                    // Distinguish EDC15P vs EDC15VM
                    // EDC15VM codeblocks contain V4.1 signature: 0x67 FF FF FF FF FF FF 56 34 2E 31
                    let is_edc15vm = Self::has_edc15vm_signature(data);
                    let ecu_type = if is_edc15vm { ECUType::EDC15VM } else { ECUType::EDC15P };
                    let variant_str = if is_edc15vm { "VAG TDI VM" } else { "VAG TDI" };

                    log::debug!("512KB EDC15 file identified as {:?}", ecu_type);

                    return Some(ECUIdentification {
                        manufacturer: ECUManufacturer::Bosch,
                        ecu_type,
                        variant: Some(variant_str.to_string()),
                        software_version,
                        hardware_version: hw_number,
                        part_number: Self::extract_edc16_vag_part(data).or_else(|| Self::extract_vag_part_number(data)),
                        confidence: 0.85,
                    });
                }

                // Check for EDC16 patterns
                if Self::has_edc16_characteristics(data) {
                    let (ecu_type, variant) = Self::detect_edc16_variant(data, hw_prefix);
                    let sw_number = Self::extract_edc16_sw_number(data);

                    return Some(ECUIdentification {
                        manufacturer: ECUManufacturer::Bosch,
                        ecu_type,
                        variant: Some(variant),
                        software_version: sw_number,
                        hardware_version: hw_number,
                        part_number: Self::extract_edc16_vag_part(data).or_else(|| Self::extract_vag_part_number(data)),
                        confidence: 0.80,
                    });
                }
            } else if file_size == 1048576 {
                // 1MB — EDC15VM+ d'abord : les A6 2.5 V6 TDI (VP44) sont des
                // EDC15VM en flash 1 Mo, truffés de signatures V4.1
                // (0x4001, 0x14001… 0xF0001) qu'aucun EDC16 ne porte.
                // Sans ce test ils partaient en EDC16U1 (Stage X les
                // identifie bien EDC15VM).
                if Self::has_v41_signature(data) {
                    log::debug!("1MB file with V4.1 signatures → EDC15VM (VM+)");
                    return Some(ECUIdentification {
                        manufacturer: ECUManufacturer::Bosch,
                        ecu_type: ECUType::EDC15VM,
                        variant: Some("VAG TDI VP44 (1MB)".to_string()),
                        software_version: None,
                        hardware_version: hw_number,
                        part_number: Self::extract_vag_part_number(data),
                        confidence: 0.85,
                    });
                }

                // 1MB - EDC16U1 or EDC16U31 (NOT EDC15P!)
                log::debug!("1MB file detected - checking for EDC16");

                if Self::has_edc16_characteristics(data) {
                    let (ecu_type, variant) = Self::detect_edc16_variant(data, hw_prefix);
                    // La TAILLE est décisive : U31 (MPC563) et U34 (MPC562)
                    // sont des flash 2 Mo — un fichier 1 Mo est un U1, même
                    // quand son numéro Bosch porte un préfixe 0281012x/13x
                    // (vérifié au banc : Altea 016HE, Caddy 016GP…). Sans ça,
                    // ces fichiers partaient sur le détecteur U31 dont la
                    // géographie mémoire est calée 2 Mo.
                    let ecu_type = match ecu_type {
                        ECUType::EDC16U31 | ECUType::EDC16U34 => {
                            log::debug!(
                                "1MB file identified as {:?} by Bosch prefix — overriding to EDC16U1 (size rule)",
                                ecu_type
                            );
                            ECUType::EDC16U1
                        }
                        other => other,
                    };
                    let sw_number = Self::extract_edc16_sw_number(data);

                    return Some(ECUIdentification {
                        manufacturer: ECUManufacturer::Bosch,
                        ecu_type,
                        variant: Some(variant),
                        software_version: sw_number,
                        hardware_version: hw_number,
                        part_number: Self::extract_edc16_vag_part(data).or_else(|| Self::extract_vag_part_number(data)),
                        confidence: 0.85,
                    });
                }

                // Fallback to EDC16U1 for 1MB files
                return Some(ECUIdentification {
                    manufacturer: ECUManufacturer::Bosch,
                    ecu_type: ECUType::EDC16U1,
                    variant: Some("VAG TDI".to_string()),
                    software_version: Self::extract_edc16_sw_number(data),
                    hardware_version: hw_number,
                    part_number: Self::extract_edc16_vag_part(data).or_else(|| Self::extract_vag_part_number(data)),
                    confidence: 0.70,
                });
            } else if file_size == 2097152 {
                // 2MB - EDC16U34 or EDC16U31, but ONLY with structural
                // evidence. A 2MB file with a stray "0281" string and no
                // EDC16 layout falls through to the generic (blocked) result.
                if let Some((ecu_type, variant)) = Self::identify_2mb_edc16(data) {
                    log::debug!("2MB file detected - {:?}", ecu_type);

                    let sw_number = Self::extract_edc16_sw_number(data);

                    return Some(ECUIdentification {
                        manufacturer: ECUManufacturer::Bosch,
                        ecu_type,
                        variant: Some(variant),
                        software_version: sw_number,
                        hardware_version: hw_number,
                        part_number: Self::extract_edc16_vag_part(data).or_else(|| Self::extract_vag_part_number(data)),
                        confidence: 0.90,
                    });
                }
            }

            // Generic Bosch diesel
            return Some(ECUIdentification {
                manufacturer: ECUManufacturer::Bosch,
                ecu_type: ECUType::Unknown,
                variant: Some("Diesel ECU".to_string()),
                software_version: None,
                hardware_version: hw_number,
                part_number: Self::extract_edc16_vag_part(data),
                confidence: 0.60,
            });
        }

        // Look for "0261" or "0265" pattern (Bosch petrol)
        if Self::contains_hex_pattern(&data[0..scan_limit], &[0x30, 0x32, 0x36, 0x31]) ||
           Self::contains_hex_pattern(&data[0..scan_limit], &[0x30, 0x32, 0x36, 0x35]) {
            log::debug!("Found Bosch petrol HW prefix: 0261/0265");

            return Some(ECUIdentification {
                manufacturer: ECUManufacturer::Bosch,
                ecu_type: ECUType::Unknown,
                variant: Some("Petrol ECU".to_string()),
                software_version: None,
                hardware_version: Some("0261/0265xxx".to_string()),
                part_number: Self::extract_edc16_vag_part(data),
                confidence: 0.75,
            });
        }

        None
    }

    /// Structural discrimination of 2MB EDC16 files (EDC16U34 vs EDC16U31).
    /// The Bosch HW-number prefixes are NOT reliable (real U34 files carry
    /// 0281012xxx numbers), so the distinction is structural:
    ///  - an explicit "EDC16U31" ASCII string in the calibration, or
    ///  - the U31 shared-axes Driver Wish selector signature
    ///    [27 10 00 00 8A 00 00 06] (pedal-axis end + selector marker),
    ///    which never appears on U34 layouts (verified on the 33-file corpus), or
    ///  - the DTC control-table signature [81 D4 x3 17 48] in the calibration
    ///    area: every U34 firmware keeps it in 0x1CD18E-0x1CF2A4 (< 0x1D0000),
    ///    while the U31 hybrid layouts (03G906016JA/KN, Crafter) place it at
    ///    0x1D04C8+.
    /// Returns None when NO structural signature matches: a 2MB file without
    /// any EDC16 layout evidence must not be labeled EDC16 (it could be a dump
    /// from a completely different 2MB ECU).
    fn identify_2mb_edc16(data: &[u8]) -> Option<(ECUType, String)> {
        if Self::contains_sequence(data, b"EDC16U31")
            || Self::contains_sequence(data, &[0x27, 0x10, 0x00, 0x00, 0x8A, 0x00, 0x00, 0x06])
        {
            return Some((ECUType::EDC16U31, "VAG TDI (MPC563)".to_string()));
        }

        // SW véhicule utilitaire (Transporter/Touareg R5, 1.9 PD commercial)
        // dans la zone HW VAG des fichiers 2 Mo : ces familles n'existent
        // qu'en U31 côté 2 Mo (T5 Stage 2 : 070997016M à 0x1C0CBE, alors que
        // sa signature DTC tombe à 0x1CFAEA < 0x1D0000 et le classait U34).
        if data.len() >= 0x1C1000 {
            let zone = &data[0x1C0800..0x1C1000];
            for sw in [&b"070906"[..], b"070997", b"038906", b"038997"] {
                if zone.windows(sw.len()).any(|w| w == sw) {
                    return Some((ECUType::EDC16U31, "VAG TDI (MPC563)".to_string()));
                }
            }
        }

        // Position-based check on the DTC control-table signature,
        // restricted to the calibration area to skip stray code matches
        // (e.g. VWEos2l has a spurious copy at 0x18BE0E).
        let sig34: [u8; 8] = [0x81, 0xD4, 0x81, 0xD4, 0x81, 0xD4, 0x17, 0x48];
        let end = data.len().min(0x1D8000);
        for i in 0x1C0000..end.saturating_sub(sig34.len()) {
            if data[i..i + 8] == sig34 {
                return if i >= 0x1D0000 {
                    Some((ECUType::EDC16U31, "VAG TDI (MPC563)".to_string()))
                } else {
                    Some((ECUType::EDC16U34, "VAG TDI".to_string()))
                };
            }
        }

        None
    }

    /// Detect EDC16 variant based on HW prefix and file patterns
    fn detect_edc16_variant(data: &[u8], hw_prefix: &str) -> (ECUType, String) {
        // 2MB files: the Bosch HW-number prefix mapping below is NOT reliable
        // (real U34 files carry 0281012xxx numbers) — always use the
        // structural discrimination instead, whatever path called us.
        // No structural match on a 2MB file => Unknown (never EDC16 by size).
        if data.len() == 2097152 {
            return Self::identify_2mb_edc16(data)
                .unwrap_or((ECUType::Unknown, "2MB layout not recognized".to_string()));
        }

        // Based on HW number prefixes from Grok info:
        // EDC16U1: 0281010xxx, 0281011xxx
        // EDC16U31: 0281012xxx, 0281014xxx
        // EDC16U34: 02810121xx, 0281013xxx

        // Check for MPC processor signatures
        let has_mpc556 = Self::contains_sequence(data, b"MPC556") ||
                         Self::contains_sequence(data, b"mpc556");
        let has_mpc562 = Self::contains_sequence(data, b"MPC562") ||
                         Self::contains_sequence(data, b"mpc562");
        let has_mpc563 = Self::contains_sequence(data, b"MPC563") ||
                         Self::contains_sequence(data, b"mpc563");

        if has_mpc556 || hw_prefix.starts_with("0281010") || hw_prefix.starts_with("0281011") {
            return (ECUType::EDC16U1, "VAG TDI (MPC556LF)".to_string());
        }

        if has_mpc563 || hw_prefix.starts_with("0281012") && !hw_prefix.starts_with("02810121") ||
           hw_prefix.starts_with("0281014") {
            return (ECUType::EDC16U31, "VAG TDI (MPC563)".to_string());
        }

        if has_mpc562 || hw_prefix.starts_with("02810121") || hw_prefix.starts_with("0281013") {
            return (ECUType::EDC16U34, "VAG TDI (MPC562)".to_string());
        }

        // Default to EDC16U1 for unknown variants
        (ECUType::EDC16U1, "VAG TDI".to_string())
    }

    /// Check for EDC16 characteristics
    fn has_edc16_characteristics(data: &[u8]) -> bool {
        if data.len() < 10000 {
            return false;
        }

        // EDC16 uses different patterns than EDC15
        // Look for EDC16 specific strings
        if Self::contains_sequence(data, b"EDC16") ||
           Self::contains_sequence(data, b"EDC 16") {
            return true;
        }

        // Check for MPC5xx processor signatures (EDC16 uses MPC556/562/563)
        if Self::contains_sequence(data, b"MPC5") {
            return true;
        }

        // Check for 1037 software version pattern (EDC16 SW starts with 1037).
        // 2MB dumps keep their metadata in the calibration half (SW number at
        // 0x180010), so scan that window too — some dumps carry NO other
        // ASCII evidence (no 0281/EDC16/03G906 strings).
        let mut windows: Vec<(usize, usize)> = vec![(0, std::cmp::min(100_000, data.len()))];
        if data.len() == 2_097_152 {
            windows.push((0x180000, 0x180000 + 100_000));
        }
        for (start, end) in windows {
            let end = std::cmp::min(end, data.len());
            for i in start..end.saturating_sub(10) {
                if data[i..].starts_with(b"1037") {
                    // Verify next 6 chars are digits
                    let mut valid = true;
                    for j in 4..10 {
                        if i + j < data.len() && !data[i + j].is_ascii_digit() {
                            valid = false;
                            break;
                        }
                    }
                    if valid {
                        return true;
                    }
                }
            }
        }

        // Check for 03G906 part number pattern (EDC16 VAG)
        if Self::contains_sequence(data, b"03G906") {
            return true;
        }

        false
    }

    /// Extract EDC16 software version
    /// Uses specific offsets based on file size:
    /// - 1MB files: SW at offset 0x10
    /// - 2MB files: SW at offset 0x180010 (data starts at 0x180000)
    /// Falls back to scanning if specific offsets don't work
    fn extract_edc16_sw_number(data: &[u8]) -> Option<String> {
        let file_size = data.len();

        // EDC16 specific offsets for SW number (Bosch format 1037XXXXXX)
        let sw_offsets: Vec<usize> = if file_size == 2097152 {
            // 2MB EDC16U34: data in second half, SW at 0x180010
            vec![0x180010, 0x10]
        } else if file_size == 1048576 {
            // 1MB EDC16U1/U31: SW at 0x10
            vec![0x10]
        } else {
            vec![0x10]
        };

        // Try specific offsets first
        for &offset in &sw_offsets {
            if let Some(sw) = Self::extract_sw_at_offset(data, offset) {
                log::debug!("Found EDC16 SW number at specific offset 0x{:X}: {}", offset, sw);
                return Some(sw);
            }
        }

        // Fallback: scan for 1037XXXXXX pattern
        // For 2MB files, start scanning from 0x180000
        let scan_start = if file_size == 2097152 { 0x180000 } else { 0 };
        let scan_limit = std::cmp::min(scan_start + 500000, data.len());

        for i in scan_start..scan_limit.saturating_sub(10) {
            if data[i..].starts_with(b"1037") {
                let mut sw_number = String::from("1037");
                let mut valid = true;

                for j in 4..10 {
                    if i + j < data.len() && data[i + j].is_ascii_digit() {
                        sw_number.push(data[i + j] as char);
                    } else {
                        valid = false;
                        break;
                    }
                }

                if valid && sw_number.len() == 10 {
                    log::debug!("Found EDC16 Bosch SW number: {} at offset 0x{:X}", sw_number, i);
                    return Some(sw_number);
                }
            }
        }

        None
    }

    /// Extract SW number at a specific offset
    fn extract_sw_at_offset(data: &[u8], offset: usize) -> Option<String> {
        if offset + 10 > data.len() {
            return None;
        }

        // Check for 1037 prefix
        if !data[offset..].starts_with(b"1037") {
            return None;
        }

        let mut sw_number = String::from("1037");

        for j in 4..10 {
            if data[offset + j].is_ascii_digit() {
                sw_number.push(data[offset + j] as char);
            } else {
                return None;
            }
        }

        if sw_number.len() == 10 {
            Some(sw_number)
        } else {
            None
        }
    }

    /// Extract full Bosch HW number (scans entire file)
    /// For EDC16 files (1MB/2MB), uses specific offsets based on file size:
    /// - 1MB files: HW at offset ~0xD0258 (Bosch format 0281XXXXXX)
    /// - 2MB files: HW at offset ~0x1C0CD2 (VAG format 03G906XXXXX)
    fn extract_bosch_hw_number_full(data: &[u8]) -> Option<String> {
        let file_size = data.len();

        // Try EDC16 specific offsets first based on file size
        if file_size == 2097152 {
            // 2MB EDC16U34/U31: HW in VAG format in the 0x1C0Axx-0x1C0Exx
            // string area (U31 Superb has its part number at 0x1C0B18)
            if let Some(hw) = Self::extract_edc16_hw_at_zone(data, 0x1C0A00, 0x1C0F00) {
                return Some(hw);
            }
        } else if file_size == 1048576 {
            // 1MB EDC16U1/U31: HW at ~0xD0200 area (fallback 0xD0000+ where
            // some firmwares put a first copy of the part number, and
            // 0xFCxxx for variants that relocate the metadata block there)
            for &(zs, ze) in &[(0xD0200usize, 0xD0400usize), (0xD0000, 0xD0200), (0xFC000, 0xFD000)] {
                if let Some(hw) = Self::extract_edc16_hw_at_zone(data, zs, ze) {
                    return Some(hw);
                }
            }
        }

        // Fallback: try the EDC15P metadata area (0x53000-0x54000)
        if let Some(hw) = Self::extract_bosch_hw_number(data) {
            return Some(hw);
        }

        // Last resort: scan for 0281 pattern
        // For 2MB files, scan second half
        let scan_start = if file_size == 2097152 { 0x180000 } else { 0 };
        let scan_limit = std::cmp::min(scan_start + 500000, data.len());

        for i in scan_start..scan_limit.saturating_sub(10) {
            if data[i..].starts_with(b"0281") {
                let mut hw_number = String::from("0281");
                let mut valid = true;

                for j in 4..10 {
                    if i + j < data.len() && data[i + j].is_ascii_digit() {
                        hw_number.push(data[i + j] as char);
                    } else {
                        valid = false;
                        break;
                    }
                }

                if valid && hw_number.len() == 10 {
                    log::debug!("Found Bosch HW number: {} at offset 0x{:X}", hw_number, i);
                    return Some(hw_number);
                }
            }
        }

        None
    }

    /// Extract EDC16 HW number from a specific zone
    /// Supports both Bosch format (0281XXXXXX) and VAG format (03G906XXXXX)
    /// For VAG format, there are often TWO numbers:
    /// - First one is the SW part number (e.g., 03G906021AB)
    /// - Second one is the HW part number (e.g., 03G906021HP) - this is the one we want
    fn extract_edc16_hw_at_zone(data: &[u8], start: usize, end: usize) -> Option<String> {
        if end > data.len() {
            return None;
        }

        let zone = &data[start..end];

        // First try Bosch format: 0281XXXXXX (10 digits)
        for i in 0..zone.len().saturating_sub(10) {
            if zone[i..].starts_with(b"0281") {
                let mut hw_number = String::from("0281");
                let mut valid = true;

                for j in 4..10 {
                    if zone[i + j].is_ascii_digit() {
                        hw_number.push(zone[i + j] as char);
                    } else {
                        valid = false;
                        break;
                    }
                }

                if valid && hw_number.len() == 10 {
                    log::debug!("Found Bosch HW number in zone: {} at offset 0x{:X}", hw_number, start + i);
                    return Some(hw_number);
                }
            }
        }

        // Then try VAG format via the generic scanner.
        // IMPORTANT: We collect ALL matches and return the LAST one (which is typically the HW number)
        let mut found_numbers: Vec<(String, usize)> = Vec::new();

        {
            for (hw_number, i) in Self::find_vag_numbers_in_zone(zone) {
                {
                    log::debug!("Found VAG number in zone: {} at offset 0x{:X}", hw_number, start + i);
                    found_numbers.push((hw_number, i));
                }
            }
        }

        // Return the LAST found number (which is typically the HW part number)
        // The first one is usually the SW part number
        if let Some((hw_number, offset)) = found_numbers.last() {
            log::debug!("Using VAG HW number (last of {}): {} at offset 0x{:X}",
                       found_numbers.len(), hw_number, start + offset);
            return Some(hw_number.clone());
        }

        None
    }

    /// Scan a zone for VAG part numbers using the generic pattern:
    /// '0' + digit + alphanumeric + "906" + 3 digits + 0-2 uppercase letters.
    /// Covers 03G906016JA, 038906016K (single-letter suffix), 070906016BA,
    /// 076906016xx, 04L906xxx, 045906019xx, ...
    /// Returns all matches as (number, offset-in-zone), in file order.
    fn find_vag_numbers_in_zone(zone: &[u8]) -> Vec<(String, usize)> {
        let mut found: Vec<(String, usize)> = Vec::new();
        for i in 0..zone.len().saturating_sub(11) {
            if zone[i] == b'0'
                && zone[i + 1].is_ascii_digit()
                && (zone[i + 2].is_ascii_digit() || zone[i + 2].is_ascii_uppercase())
                && zone[i + 3..i + 6] == *b"906"
                && zone[i + 6].is_ascii_digit()
                && zone[i + 7].is_ascii_digit()
                && zone[i + 8].is_ascii_digit()
            {
                let mut number = String::new();
                for k in 0..9 {
                    number.push(zone[i + k] as char);
                }
                // Optional 1-2 uppercase letter suffix (JA, K, BA, ...)
                if i + 9 < zone.len() && zone[i + 9].is_ascii_uppercase() {
                    number.push(zone[i + 9] as char);
                    if i + 10 < zone.len() && zone[i + 10].is_ascii_uppercase() {
                        number.push(zone[i + 10] as char);
                    }
                }
                found.push((number, i));
            }
        }
        found
    }

    /// Extract the VAG part number of an EDC16 file (1MB/2MB) from its
    /// metadata string area. Returns the LAST match (HW part number).
    fn extract_edc16_vag_part(data: &[u8]) -> Option<String> {
        let zones: &[(usize, usize)] = match data.len() {
            2097152 => &[(0x1C0A00, 0x1C0F00)],
            // Some 1MB firmwares relocate the metadata block to 0xFCxxx
            1048576 => &[(0xD0000, 0xD0400), (0xFC000, 0xFD000)],
            _ => return None,
        };
        for &(start, end) in zones {
            if end > data.len() {
                continue;
            }
            if let Some((n, _)) = Self::find_vag_numbers_in_zone(&data[start..end]).last() {
                return Some(n.clone());
            }
        }
        None
    }

    /// Method 2: ASCII strings
    fn identify_by_ascii_strings(data: &[u8]) -> Option<ECUIdentification> {
        let scan_limit = std::cmp::min(Self::MAX_SCAN_BYTES, data.len());
        let search_data = &data[0..scan_limit];
        let file_size = data.len();

        // Check for EDC16 strings first (before EDC15)
        if Self::contains_sequence(search_data, b"EDC16") ||
           Self::contains_sequence(search_data, b"EDC 16") ||
           Self::contains_sequence(search_data, b"EDC_16") {

            let hw_number = Self::extract_bosch_hw_number_full(data);
            let hw_prefix = hw_number.as_ref().map(|h| &h[..7]).unwrap_or("");
            let (ecu_type, variant) = Self::detect_edc16_variant(data, hw_prefix);

            return Some(ECUIdentification {
                manufacturer: ECUManufacturer::Bosch,
                ecu_type,
                variant: Some(variant),
                software_version: Self::extract_edc16_sw_number(data),
                hardware_version: hw_number,
                part_number: Self::extract_edc16_vag_part(data).or_else(|| Self::extract_vag_part_number(data)),
                confidence: 0.95,
            });
        }

        // Bosch EDC15P / EDC15VM - ONLY for 512KB files
        if (Self::contains_sequence(search_data, b"EDC 15P") ||
            Self::contains_sequence(search_data, b"EDC15P") ||
            Self::contains_sequence(search_data, b"EDC_15P")) && file_size == 524288 {
            let is_edc15vm = Self::has_edc15vm_signature(data);
            let ecu_type = if is_edc15vm { ECUType::EDC15VM } else { ECUType::EDC15P };
            return Some(ECUIdentification {
                manufacturer: ECUManufacturer::Bosch,
                ecu_type,
                variant: Some("VAG".to_string()),
                software_version: Self::extract_software_version(search_data),
                hardware_version: None,
                part_number: Self::extract_vag_part_number(search_data),
                confidence: 0.95,
            });
        }

        // Bosch EDC15C
        if Self::contains_sequence(search_data, b"EDC 15C") ||
           Self::contains_sequence(search_data, b"EDC15C") {
            return Some(ECUIdentification {
                manufacturer: ECUManufacturer::Bosch,
                ecu_type: ECUType::EDC15C,
                variant: Some("PSA".to_string()),
                software_version: Self::extract_software_version(search_data),
                hardware_version: None,
                part_number: Self::extract_edc16_vag_part(data),
                confidence: 0.95,
            });
        }

        // Generic Bosch
        if Self::contains_sequence(search_data, b"BOSCH") ||
           Self::contains_sequence(search_data, b"Bosch") {
            return Some(ECUIdentification {
                manufacturer: ECUManufacturer::Bosch,
                ecu_type: ECUType::Unknown,
                variant: None,
                software_version: None,
                hardware_version: None,
                part_number: Self::extract_edc16_vag_part(data),
                confidence: 0.60,
            });
        }

        None
    }
    
    /// Method 3: Part numbers
    fn identify_by_part_number(data: &[u8]) -> Option<ECUIdentification> {
        let scan_limit = std::cmp::min(Self::MAX_SCAN_BYTES, data.len());
        let search_data = &data[0..scan_limit];
        let file_size = data.len();

        if let Some(part_num) = Self::extract_vag_part_number(search_data) {
            // 03G906xxx = EDC16 VAG (2.0 TDI PD, etc.)
            if part_num.starts_with("03G906") || part_num.starts_with("03G 906") {
                let hw_number = Self::extract_bosch_hw_number_full(data);
                let hw_prefix = hw_number.as_ref().map(|h| &h[..7]).unwrap_or("");
                let (ecu_type, variant) = Self::detect_edc16_variant(data, hw_prefix);

                return Some(ECUIdentification {
                    manufacturer: ECUManufacturer::Bosch,
                    ecu_type,
                    variant: Some(variant),
                    software_version: Self::extract_edc16_sw_number(data),
                    hardware_version: hw_number,
                    part_number: Some(part_num),
                    confidence: 0.90,
                });
            }

            // 070906xxx = EDC16 VAG (2.5 TDI, Transporter T5)
            if part_num.starts_with("070906") || part_num.starts_with("070 906") {
                let hw_number = Self::extract_bosch_hw_number_full(data);

                return Some(ECUIdentification {
                    manufacturer: ECUManufacturer::Bosch,
                    ecu_type: ECUType::EDC16U1,
                    variant: Some("VAG 2.5 TDI".to_string()),
                    software_version: Self::extract_edc16_sw_number(data),
                    hardware_version: hw_number,
                    part_number: Some(part_num),
                    confidence: 0.90,
                });
            }

            // 038906xxx = EDC15P VAG (1.9 TDI PD) - ONLY for 512KB files
            if (part_num.starts_with("038906") || part_num.starts_with("038 906")) && file_size == 524288 {
                return Some(ECUIdentification {
                    manufacturer: ECUManufacturer::Bosch,
                    ecu_type: ECUType::EDC15P,
                    variant: Some("VAG".to_string()),
                    software_version: None,
                    hardware_version: None,
                    part_number: Some(part_num),
                    confidence: 0.90,
                });
            }
        }

        None
    }
    
    /// Method 4: Binary patterns (checksums, magic bytes, processor signatures)
    /// STRICT: every branch requires positive family evidence — file size on
    /// its own never yields an ECU type here.
    fn identify_by_binary_patterns(data: &[u8]) -> Option<ECUIdentification> {
        let file_size = data.len();

        // 1MB = EDC16U1 or EDC16U31, only with EDC16 evidence
        if file_size == 1048576 {
            // EDC15VM+ 1 Mo d'abord (A6 2.5 V6 TDI) : signatures V4.1
            // présentes → jamais un EDC16 (même garde que la branche
            // principale).
            if Self::has_v41_signature(data) {
                log::debug!("1MB file with V4.1 signatures (binary patterns) → EDC15VM");
                return Some(ECUIdentification {
                    manufacturer: ECUManufacturer::Bosch,
                    ecu_type: ECUType::EDC15VM,
                    variant: Some("VAG TDI VP44 (1MB)".to_string()),
                    software_version: None,
                    hardware_version: Self::extract_bosch_hw_number_full(data),
                    part_number: Self::extract_vag_part_number(data),
                    confidence: 0.80,
                });
            }

            log::debug!("1MB file in binary patterns - checking for EDC16");

            if Self::has_edc16_characteristics(data) {
                let hw_number = Self::extract_bosch_hw_number_full(data);
                let hw_prefix = hw_number.as_ref().map(|h| &h[..7]).unwrap_or("");
                let (ecu_type, variant) = Self::detect_edc16_variant(data, hw_prefix);
                // Même règle de TAILLE que la branche principale : un 1 Mo
                // est un U1 même si son numéro Bosch porte un préfixe U31/U34
                // (Altea 016HE, Caddy 016GP…) — U31/U34 = flash 2 Mo.
                let ecu_type = match ecu_type {
                    ECUType::EDC16U31 | ECUType::EDC16U34 => ECUType::EDC16U1,
                    other => other,
                };

                return Some(ECUIdentification {
                    manufacturer: ECUManufacturer::Bosch,
                    ecu_type,
                    variant: Some(variant),
                    software_version: Self::extract_edc16_sw_number(data),
                    hardware_version: hw_number,
                    part_number: Self::extract_edc16_vag_part(data),
                    confidence: 0.75,
                });
            }

            return None;
        }

        // 2MB = EDC16U34 or EDC16U31, only with EDC16 evidence AND a
        // recognized structural layout
        if file_size == 2097152 {
            if Self::has_edc16_characteristics(data) {
                if let Some((ecu_type, variant)) = Self::identify_2mb_edc16(data) {
                    log::debug!("2MB file in binary patterns - {:?}", ecu_type);

                    return Some(ECUIdentification {
                        manufacturer: ECUManufacturer::Bosch,
                        ecu_type,
                        variant: Some(variant),
                        software_version: Self::extract_edc16_sw_number(data),
                        hardware_version: Self::extract_bosch_hw_number_full(data),
                        part_number: Self::extract_edc16_vag_part(data),
                        confidence: 0.80,
                    });
                }
            }

            return None;
        }

        // EDC15P / EDC15VM: ONLY 512KB files, and only when the V4.1
        // codeblock signature confirms a real EDC15 dump
        if file_size == 524288 {
            if !Self::has_v41_signature(data) {
                return None;
            }
            let is_edc15vm = Self::has_edc15vm_signature(data);
            let ecu_type = if is_edc15vm { ECUType::EDC15VM } else { ECUType::EDC15P };

            // Check for TriCore processor signature (Infineon TC1766/TC1796)
            if Self::has_tricore_signature(data) {
                log::debug!("Detected TriCore processor signature");

                // Check for EDC15 specific patterns
                if Self::has_edc15_characteristics(data) {
                    let hw_number = Self::extract_bosch_hw_number(data);
                    let sw_sg_result = Self::extract_vag_sw_number(data);
                    let software_version = match sw_sg_result {
                        Some((sw, Some(sg))) => Some(format!("{} {}", sw, sg)),
                        Some((sw, None)) => Some(sw),
                        None => None,
                    };

                    return Some(ECUIdentification {
                        manufacturer: ECUManufacturer::Bosch,
                        ecu_type: ecu_type.clone(),
                        variant: Some("TriCore TC1766/TC1796".to_string()),
                        software_version,
                        hardware_version: hw_number,
                        part_number: Self::extract_edc16_vag_part(data),
                        confidence: 0.80,
                    });
                }
            }

            // Check for checksum at end (EDC15 pattern)
            if data.len() >= 4 {
                let last_4 = &data[data.len()-4..];
                if !(last_4[0] == 0xFF && last_4[1] == 0xFF && last_4[2] == 0xFF && last_4[3] == 0xFF) {
                    log::debug!("Detected non-FF checksum at end (EDC15 pattern)");

                    let hw_number = Self::extract_bosch_hw_number(data);
                    let sw_sg_result = Self::extract_vag_sw_number(data);
                    let software_version = match sw_sg_result {
                        Some((sw, Some(sg))) => Some(format!("{} {}", sw, sg)),
                        Some((sw, None)) => Some(sw),
                        None => None,
                    };

                    return Some(ECUIdentification {
                        manufacturer: ECUManufacturer::Bosch,
                        ecu_type,
                        variant: None,
                        software_version,
                        hardware_version: hw_number,
                        part_number: Self::extract_edc16_vag_part(data),
                        confidence: 0.70,
                    });
                }
            }
        }

        None
    }

    /// Method 5: File structure
    /// STRICT: like method 4, size alone never yields an ECU type.
    fn identify_by_structure(data: &[u8]) -> Option<ECUIdentification> {
        let file_size = data.len();

        log::debug!("Analyzing file structure for size: {} bytes", file_size);

        // EDC15P / EDC15VM: ONLY 512KB (524288 bytes) with the V4.1 signature
        if file_size == 524288 {
            if Self::has_edc15_characteristics(data) && Self::has_v41_signature(data) {
                log::debug!("512KB file with EDC15 characteristics");

                let is_edc15vm = Self::has_edc15vm_signature(data);
                let ecu_type = if is_edc15vm { ECUType::EDC15VM } else { ECUType::EDC15P };

                let hw_number = Self::extract_bosch_hw_number(data);
                let sw_sg_result = Self::extract_vag_sw_number(data);
                let software_version = match sw_sg_result {
                    Some((sw, Some(sg))) => Some(format!("{} {}", sw, sg)),
                    Some((sw, None)) => Some(sw),
                    None => None,
                };

                return Some(ECUIdentification {
                    manufacturer: ECUManufacturer::Bosch,
                    ecu_type,
                    variant: None,
                    software_version,
                    hardware_version: hw_number,
                    part_number: Self::extract_edc16_vag_part(data),
                    confidence: 0.65,
                });
            }
        }

        // EDC16U1/U31: 1MB (1048576 bytes), only with EDC16 evidence
        if file_size == 1048576 {
            if Self::has_edc16_characteristics(data) {
                log::debug!("1MB file structure - EDC16U1/U31");

                let hw_number = Self::extract_bosch_hw_number_full(data);
                let hw_prefix = hw_number.as_ref().map(|h| &h[..7]).unwrap_or("");
                let (ecu_type, variant) = Self::detect_edc16_variant(data, hw_prefix);

                return Some(ECUIdentification {
                    manufacturer: ECUManufacturer::Bosch,
                    ecu_type,
                    variant: Some(variant),
                    software_version: Self::extract_edc16_sw_number(data),
                    hardware_version: hw_number,
                    part_number: Self::extract_edc16_vag_part(data),
                    confidence: 0.70,
                });
            }
            return None;
        }

        // EDC16U34/U31: 2MB (2097152 bytes) — needs EDC16 evidence AND a
        // recognized structural layout
        if file_size == 2097152 {
            if Self::has_edc16_characteristics(data) {
                if let Some((ecu_type, variant)) = Self::identify_2mb_edc16(data) {
                    log::debug!("2MB file structure - {:?}", ecu_type);

                    return Some(ECUIdentification {
                        manufacturer: ECUManufacturer::Bosch,
                        ecu_type,
                        variant: Some(variant),
                        software_version: Self::extract_edc16_sw_number(data),
                        hardware_version: Self::extract_bosch_hw_number_full(data),
                        part_number: Self::extract_edc16_vag_part(data),
                        confidence: 0.75,
                    });
                }
            }
            return None;
        }

        None
    }

    /// Method 6: Final fallback.
    /// Deliberately does NOT guess an ECU type from the file size: if none of
    /// the evidence-based methods matched, the file is reported Unknown and
    /// the frontend refuses the import. This is what guarantees that a 2MB
    /// dump from an unsupported ECU is never opened as an EDC16.
    fn identify_by_heuristics(_data: &[u8]) -> ECUIdentification {
        Self::unknown_ecu(0.20)
    }
    
    // Helper methods
    
    fn unknown_ecu(confidence: f32) -> ECUIdentification {
        ECUIdentification {
            manufacturer: ECUManufacturer::Unknown,
            ecu_type: ECUType::Unknown,
            variant: None,
            software_version: None,
            hardware_version: None,
            part_number: None,
            confidence,
        }
    }
    
    fn contains_sequence(data: &[u8], pattern: &[u8]) -> bool {
        data.windows(pattern.len()).any(|window| window == pattern)
    }
    
    fn contains_hex_pattern(data: &[u8], pattern: &[u8]) -> bool {
        Self::contains_sequence(data, pattern)
    }
    
    /// Check for TriCore processor signature
    fn has_tricore_signature(data: &[u8]) -> bool {
        // TriCore processors have specific instruction patterns
        // Look for common TriCore opcodes in first 10KB
        let scan_limit = std::cmp::min(10000, data.len());
        
        // TriCore uses 16-bit and 32-bit instructions
        // Common patterns: MOVH (0xB9), MTCR (0xCD), etc.
        let mut tricore_pattern_count = 0;
        
        for i in 0..scan_limit.saturating_sub(2) {
            // Look for TriCore instruction patterns
            if data[i] == 0xB9 || data[i] == 0xCD || data[i] == 0x8D {
                tricore_pattern_count += 1;
            }
        }
        
        tricore_pattern_count > 50
    }
    
    /// True when the file contains at least one V4.1 codeblock signature
    /// (0x67 FF FF FF FF FF FF 56 34 2E 31). Present in every EDC15P and
    /// EDC15VM dump — its absence rules out the EDC15 family, which keeps
    /// foreign 512KB files (e.g. Bosch ME7 petrol) from being opened as EDC15.
    fn has_v41_signature(data: &[u8]) -> bool {
        let pattern: [u8; 11] = [0x67, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x56, 0x34, 0x2E, 0x31];
        Self::contains_sequence(data, &pattern)
    }

    /// Check for EDC15VM vs EDC15P using V4.1 codeblock signature positions.
    /// Both EDC15VM and EDC15P have V4.1 signatures (0x67 FF FF FF FF FF FF 56 34 2E 31),
    /// but they differ in codeblock count and position:
    ///   - EDC15VM: exactly 1 signature at 0x70001 (single codeblock at 0x70000)
    ///   - EDC15P:  2-3 signatures, always including one at 0x50001 (codeblocks at 0x50000+)
    /// Rule: if V4.1 exists at 0x50001 → EDC15P; if only at 0x70001 → EDC15VM.
    fn has_edc15vm_signature(data: &[u8]) -> bool {
        let pattern: [u8; 11] = [0x67, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x56, 0x34, 0x2E, 0x31];

        // Discriminant le plus fiable : la référence VAG en ASCII.
        // 038906012x = EDC15VM (pompe VP44), 038906019x = EDC15P
        // (injecteurs-pompe). Certains VM (Seat leon 012FN, Jetta 012GN)
        // ont AUSSI une signature V4.1 à 0x50001 et partaient en P avec la
        // seule règle positionnelle.
        if Self::contains_sequence(data, b"038906012") {
            log::debug!("VAG part 038906012x → EDC15VM");
            return true;
        }
        if Self::contains_sequence(data, b"038906019") {
            log::debug!("VAG part 038906019x → EDC15P");
            return false;
        }

        // Check for V4.1 signature at 0x50001 (EDC15P codeblock 1)
        if data.len() > 0x50001 + pattern.len() {
            if data[0x50001..0x50001 + pattern.len()] == pattern {
                // Signature at 0x50001 → this is EDC15P, not EDC15VM
                log::debug!("V4.1 signature at 0x50001 → EDC15P (not EDC15VM)");
                return false;
            }
        }

        // Check for V4.1 signature at 0x70001 (EDC15VM single codeblock)
        if data.len() > 0x70001 + pattern.len() {
            if data[0x70001..0x70001 + pattern.len()] == pattern {
                log::debug!("V4.1 signature at 0x70001 only → EDC15VM");
                return true;
            }
        }

        // Fallback: scan entire file (rare cases with non-standard positions)
        let mut sig_count = 0u32;
        let mut first_sig_pos = 0usize;
        for i in 0..data.len().saturating_sub(pattern.len()) {
            if data[i..i + pattern.len()] == pattern {
                sig_count += 1;
                if sig_count == 1 {
                    first_sig_pos = i;
                }
            }
        }
        // EDC15VM: exactly 1 codeblock
        if sig_count == 1 {
            log::debug!("V4.1: single signature at 0x{:X} → EDC15VM", first_sig_pos);
            return true;
        }
        log::debug!("V4.1: {} signatures found → EDC15P", sig_count);
        false
    }

    fn has_edc15_characteristics(data: &[u8]) -> bool {
        if data.len() < 10000 {
            return false;
        }
        
        // Increase scan limit to 50KB for better detection
        let scan_limit = std::cmp::min(50000, data.len());
        
        // Count potential axis ID bytes (EDC15 uses specific byte values)
        // These can appear in BOTH high and low bytes of u16 values
        let mut axis_id_count = 0;
        
        // Scan ALL bytes, not just high bytes
        for i in 0..scan_limit {
            let byte = data[i];
            
            // EDC15P typical axis ID byte values - 0x0E and 0x0F are VERY common!
            // Also check traditional ranges
            if matches!(byte, 0x0E | 0x0F | 0xC0..=0xC5 | 0xE8..=0xEC | 0xF9) {
                axis_id_count += 1;
            }
        }
        
        log::debug!("EDC15 characteristics check: found {} potential axis ID bytes in {} bytes scanned", axis_id_count, scan_limit);
        
        // With 0x0E and 0x0F patterns, we should find many occurrences
        // Real EDC15P files have hundreds of these patterns
        axis_id_count > 100
    }
    
    fn extract_software_version(data: &[u8]) -> Option<String> {
        let scan_limit = std::cmp::min(10000, data.len());
        
        for i in 0..scan_limit.saturating_sub(20) {
            if i + 3 <= data.len() && &data[i..i+3] == b"EDC" {
                let search_area = &data[i..std::cmp::min(i + 50, data.len())];
                if let Some(version) = Self::extract_version_string(search_area) {
                    return Some(version);
                }
            }
        }
        None
    }
    
    fn extract_version_string(data: &[u8]) -> Option<String> {
        for i in 0..data.len().saturating_sub(5) {
            if data[i].is_ascii_digit() && data[i+1] == b'.' && data[i+2].is_ascii_digit() {
                let mut version = String::new();
                for j in i..std::cmp::min(i + 10, data.len()) {
                    if data[j].is_ascii_digit() || data[j] == b'.' {
                        version.push(data[j] as char);
                    } else {
                        break;
                    }
                }
                if version.len() >= 3 {
                    return Some(version);
                }
            }
        }
        None
    }
    
    fn extract_vag_part_number(data: &[u8]) -> Option<String> {
        let scan_limit = std::cmp::min(50000, data.len());

        for i in 0..scan_limit.saturating_sub(15) {
            if data[i].is_ascii_digit() &&
               data[i+1].is_ascii_digit() &&
               data[i+2].is_ascii_digit() {

                let mut part_num = String::new();
                let mut j = i;
                let mut digit_groups = 0;

                while j < std::cmp::min(i + 20, data.len()) && digit_groups < 3 {
                    if data[j].is_ascii_digit() {
                        part_num.push(data[j] as char);
                        if part_num.len() % 3 == 0 {
                            digit_groups += 1;
                            if digit_groups < 3 {
                                part_num.push(' ');
                            }
                        }
                    } else if data[j] == b' ' {
                        // Skip
                    } else {
                        break;
                    }
                    j += 1;
                }

                if digit_groups == 3 && part_num.len() >= 11 {
                    if j + 2 <= data.len() &&
                       data[j].is_ascii_uppercase() &&
                       data[j+1].is_ascii_uppercase() {
                        part_num.push(' ');
                        part_num.push(data[j] as char);
                        part_num.push(data[j+1] as char);
                    }
                    return Some(part_num);
                }
            }
        }
        None
    }

    /// Extract Bosch hardware number (format: 0281xxxxxx - 10 digits)
    /// Searches in metadata area typically located around 0x53000-0x54000
    fn extract_bosch_hw_number(data: &[u8]) -> Option<String> {
        // Search multiple ranges: EDC15P at 0x53000, EDC15VM may differ
        let search_ranges: &[(usize, usize)] = &[
            (0x53000, 0x55000),
            (0x70000, 0x80000),
            (0x40000, 0x53000),
        ];

        for &(range_start, range_end) in search_ranges {
            let end = std::cmp::min(range_end, data.len());
            if range_start >= end {
                continue;
            }
            let search_area = &data[range_start..end];

            for i in 0..search_area.len().saturating_sub(10) {
                if search_area[i..].starts_with(b"0281") {
                    let mut hw_number = String::from("0281");
                    let mut valid = true;

                    for j in 4..10 {
                        if i + j < search_area.len() && search_area[i + j].is_ascii_digit() {
                            hw_number.push(search_area[i + j] as char);
                        } else {
                            valid = false;
                            break;
                        }
                    }

                    if valid && hw_number.len() == 10 {
                        log::debug!("Found Bosch HW number: {} at offset 0x{:X}", hw_number, range_start + i);
                        return Some(hw_number);
                    }
                }
            }
        }

        None
    }

    /// Extract VAG software number (format: 038906019XX - VAG part number + suffix)
    /// Also extracts SG number if present (format: SGXXXX or just 4 digits)
    fn extract_vag_sw_number(data: &[u8]) -> Option<(String, Option<String>)> {
        // Known VAG ECU part number prefixes
        // EDC15P: 038906, 045906, 028906
        // EDC15VM: 028906, 036906, 03C906, 06A906, 1J0906, etc.
        let vag_prefixes: &[&[u8]] = &[
            b"038906", b"045906", b"028906", b"036906",
            b"03C906", b"06A906", b"1J0906", b"074906",
            b"03G906", b"04L906", b"03L906", b"070906",
        ];

        // Search ranges: EDC15P/VM metadata typically in 0x53000-0x54000,
        // but also scan the last 64KB for EDC15VM variants
        let search_ranges: &[(usize, usize)] = &[
            (0x53000, 0x55000),
            // Génération VM à suffixe mono-lettre (038906012L…) : la chaîne
            // qui porte le « SG nnnn » est vers 0x5EBxx — la copie de
            // 0x76Bxx dit « AG » et ferait perdre le numéro SG.
            (0x55000, 0x70000),
            (0x70000, 0x80000),
            (0x40000, 0x53000),
        ];

        for &(range_start, range_end) in search_ranges {
            let end = std::cmp::min(range_end, data.len());
            if range_start >= end {
                continue;
            }
            let search_area = &data[range_start..end];

            for prefix in vag_prefixes {
                for i in 0..search_area.len().saturating_sub(13) {
                    if search_area[i..].starts_with(prefix) {
                        let prefix_str = match std::str::from_utf8(prefix) {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let mut sw_number = String::from(prefix_str);
                        let mut valid = true;

                        // Next 3 digits
                        for j in prefix.len()..prefix.len() + 3 {
                            if i + j < search_area.len() && search_area[i + j].is_ascii_digit() {
                                sw_number.push(search_area[i + j] as char);
                            } else {
                                valid = false;
                                break;
                            }
                        }

                        // Next 2 letters (suffix like NJ, HH, KJ, MS)
                        if valid {
                            let c1_idx = i + prefix.len() + 3;
                            let c2_idx = c1_idx + 1;
                            if c2_idx < search_area.len() {
                                let c1 = search_area[c1_idx];
                                let c2 = search_area[c2_idx];

                                // Suffixe à DEUX lettres (038906019 NJ…) ou à
                                // UNE lettre (générations VM « 038906012L  … ») :
                                // la lettre unique est suivie d'un non-alphanumérique.
                                if c1.is_ascii_uppercase()
                                    && (c2.is_ascii_uppercase() || !c2.is_ascii_alphanumeric())
                                {
                                    sw_number.push(' ');
                                    sw_number.push(c1 as char);
                                    if c2.is_ascii_uppercase() {
                                        sw_number.push(c2 as char);
                                    }

                                    log::debug!("Found VAG SW number: {} at offset 0x{:X}", sw_number, range_start + i);

                                    let sg_number = Self::extract_sg_number(&search_area[i..std::cmp::min(i + 100, search_area.len())]);

                                    return Some((sw_number, sg_number));
                                }
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Extract SG number (Software Group - format: SGXXXX or just 4 digits)
    fn extract_sg_number(data: &[u8]) -> Option<String> {
        // Look for "SG" followed by digits or just 4 consecutive digits
        for i in 0..data.len().saturating_sub(6) {
            // Pattern 1: "SG" + 4 digits — avec espaces tolérés entre les
            // deux (« SG  2527 » sur les VM à suffixe mono-lettre)
            if data[i..].starts_with(b"SG") {
                let mut k = i + 2;
                while k < data.len() && data[k] == b' ' && k - i <= 4 {
                    k += 1;
                }
                let mut sg_num = String::from("SG");
                let mut valid = k + 4 <= data.len();

                for j in 0..4 {
                    if valid && data[k + j].is_ascii_digit() {
                        sg_num.push(data[k + j] as char);
                    } else {
                        valid = false;
                        break;
                    }
                }

                if valid {
                    log::debug!("Found SG number (with prefix): {}", sg_num);
                    return Some(sg_num);
                }
            }

            // Pattern 2: Just 4 digits after space (like " 7331" or " 5366")
            if i > 0 && data[i - 1] == b' ' && i + 4 < data.len() {
                let mut all_digits = true;
                let mut sg_num = String::new();

                for j in 0..4 {
                    if data[i + j].is_ascii_digit() {
                        sg_num.push(data[i + j] as char);
                    } else {
                        all_digits = false;
                        break;
                    }
                }

                if all_digits && sg_num.len() == 4 {
                    // Verify it looks like a valid SG number (not just random digits)
                    if let Ok(num) = sg_num.parse::<u32>() {
                        if num >= 1000 && num <= 9999 {
                            log::debug!("Found SG number (without prefix): {}", sg_num);
                            return Some(format!("SG{}", sg_num));
                        }
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EDC15P V4.1 codeblock signature, placed where real dumps carry it
    fn put_v41_signature(data: &mut [u8], offset: usize) {
        let pattern: [u8; 11] = [0x67, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x56, 0x34, 0x2E, 0x31];
        data[offset..offset + pattern.len()].copy_from_slice(&pattern);
    }

    #[test]
    fn test_edc15p_by_size_and_patterns() {
        let mut data = vec![0u8; 524288]; // 512KB

        // Add some axis ID patterns
        for i in (0..1000).step_by(2) {
            data[i] = 0xC5;
            data[i+1] = 0x12;
        }
        // V4.1 signature at the EDC15P codeblock position (required evidence)
        put_v41_signature(&mut data, 0x50001);

        let id = ECUIdentifier::identify(&data);
        assert_eq!(id.manufacturer, ECUManufacturer::Bosch);
        assert!(id.confidence > 0.6);
    }

    /// A 2MB file full of random-looking bytes with no Bosch evidence must
    /// come back Unknown — never EDC16 (the old size-based fallback bug).
    #[test]
    fn test_foreign_2mb_file_is_not_edc16() {
        let mut data = vec![0u8; 2097152];
        // Pseudo-random filler (deterministic LCG), no ASCII metadata
        let mut x: u32 = 0x12345678;
        for b in data.iter_mut() {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            *b = (x >> 24) as u8;
        }
        let id = ECUIdentifier::identify(&data);
        assert_eq!(id.ecu_type, ECUType::Unknown, "foreign 2MB file must be Unknown, got {:?}", id.ecu_type);
    }

    /// A 512KB file without the V4.1 codeblock signature must not be EDC15
    #[test]
    fn test_foreign_512kb_file_is_not_edc15() {
        let mut data = vec![0u8; 524288];
        let mut x: u32 = 0xCAFEBABE;
        for b in data.iter_mut() {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            *b = (x >> 24) as u8;
        }
        let id = ECUIdentifier::identify(&data);
        assert_eq!(id.ecu_type, ECUType::Unknown, "foreign 512KB file must be Unknown, got {:?}", id.ecu_type);
    }

    /// An EDC17 dump (2MB with EDC17 strings + 0281 metadata) must be
    /// identified as EDC17, never as EDC16U34
    #[test]
    fn test_edc17_2mb_is_not_edc16() {
        let mut data = vec![0xFFu8; 2097152];
        data[0x1000..0x1008].copy_from_slice(b"EDC17C46");
        data[0x2000..0x200A].copy_from_slice(b"0281016789");
        let id = ECUIdentifier::identify(&data);
        assert_ne!(id.ecu_type, ECUType::EDC16U34);
        assert_ne!(id.ecu_type, ECUType::EDC16U31);
        assert_eq!(id.ecu_type, ECUType::EDC17C);
    }

    #[test]
    fn test_hw_sw_extraction() {
        let mut data = vec![0xFF; 524288]; // 512KB filled with 0xFF

        // Add EDC15 characteristics
        for i in (0..1000).step_by(2) {
            data[i] = 0xC5;
            data[i+1] = 0x12;
        }
        // V4.1 signature at the EDC15P codeblock position (required evidence)
        put_v41_signature(&mut data, 0x50001);

        // Add HW number at 0x535C0 (like 19ORIpolo.bin)
        let hw_offset = 0x535C0 + 7; // "S10368 0281011823"
        let hw = b"0281011823";
        data[hw_offset..hw_offset+10].copy_from_slice(hw);

        // Add SW number at 0x535E0
        let sw_offset = 0x535E0;
        let sw = b"038906019NJ";
        data[sw_offset..sw_offset+11].copy_from_slice(sw);

        let id = ECUIdentifier::identify(&data);

        println!("Test result: {:?}", id);
        println!("HW: {:?}", id.hardware_version);
        println!("SW: {:?}", id.software_version);
        assert_eq!(id.manufacturer, ECUManufacturer::Bosch);
        assert_eq!(id.ecu_type, ECUType::EDC15P);

        // Check HW
        assert!(id.hardware_version.is_some(), "HW version should be extracted");
        assert_eq!(id.hardware_version, Some("0281011823".to_string()));

        // Check SW
        assert!(id.software_version.is_some(), "SW version should be extracted");
        let sw = id.software_version.as_ref().unwrap();
        assert!(sw.contains("038906019"), "SW should contain 038906019, got: {}", sw);
        assert!(sw.contains("NJ"), "SW should contain NJ, got: {}", sw);
    }
}
