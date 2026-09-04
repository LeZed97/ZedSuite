// Correct EDC15P detector based on actual C# implementation
// CRITICAL FIX: Uses LITTLE ENDIAN and reads axis lengths from file

use crate::models::{DetectedMap, MapDimensions, DataType};
use std::collections::{HashSet, HashMap};

/// Represents a codeblock in EDC15 files
/// EDC15P files typically have 2-3 codeblocks, each containing similar maps
#[derive(Debug, Clone)]
struct Codeblock {
    id: u32,
    start_address: u32,
    end_address: u32,
}

pub mod complete_patterns;

#[allow(unused_imports)]
use complete_patterns::EDC15PMapPattern;

/// EDC15P ECU specific detector
/// Based on Bosch EDC15P series (VAG 1.9 TDI PD engines)
/// Implements the CheckMap algorithm from VAGEDCSuite EDC15PFileParser.cs
pub struct EDC15PDetector {
    patterns: Vec<EDC15PMapPattern>,
}

impl EDC15PDetector {
    pub fn new() -> Self {
        Self {
            patterns: EDC15PMapPattern::load_patterns(),
        }
    }

    /// Detect maps using dynamic CheckMap algorithm
    /// This works with ANY EDC15P file by scanning the structure
    pub fn detect(&self, data: &[u8]) -> Vec<DetectedMap> {
        log::debug!("🔍 Starting dynamic EDC15P detection");
        
        let mut maps = Vec::new();
        let mut detected_addresses = HashSet::new();

        // PRIORITY: Detect multi-smoke limiters FIRST (before generic detection)
        // This ensures we find all 3 smoke limiters per codeblock instead of just 1
        self.find_smoke_limiter_maps(data, &mut maps, &mut detected_addresses);

        // Scan word by word (2 bytes at a time) - C# CheckMap algorithm
        // IMPORTANT: Scan more aggressively to find all maps
        let mut t = 0;
        while t < data.len().saturating_sub(100) {
            if let Some((detected_maps_vec, skip_len)) = self.check_map(data, t) {
                // Add all detected maps (can be multiple if MapSelector is detected)
                for detected_map in detected_maps_vec {
                    // Avoid duplicate detections
                    if !detected_addresses.contains(&detected_map.address) {
                        detected_addresses.insert(detected_map.address);
                        maps.push(detected_map);
                    }
                }
                
                // Skip logic from C#
                let mut skip = skip_len;
                if skip > 2 {
                    skip -= 2;
                }
                if skip % 2 > 0 {
                    skip -= 1;
                }
                if skip < 2 {
                    skip = 2;
                }
                // Be more aggressive: skip less to find overlapping maps
                // C# sometimes skips only 2 bytes to find all maps
                if skip > 20 {
                    skip = 20; // Limit skip to find more maps
                }
                t += skip;
            } else {
                t += 2;
            }
        }

        // SPECIAL: Detect SOI maps by finding 10-repeat MapSelectors
        // SOI maps have a special structure: 10 consecutive maps of 448 bytes followed by a MapSelector
        self.detect_soi_maps_by_selector(data, &mut maps, &mut detected_addresses);

        // SPECIAL: Detect Start IQ maps by axis pattern (0xEC/0xC1 with 8x9 or 9x8)
        self.detect_start_iq_maps(data, &mut maps, &mut detected_addresses);

        // SPECIAL: Detect maps by byte sequence patterns (like C# FindSVBL, FindSVRL, FindBIPline)
        self.detect_special_maps_by_sequence(data, &mut maps, &mut detected_addresses);

        // SPECIAL: VCDS diagnostic single-value maps (IQ/MAF/MAP/Torque limits & displays)
        self.detect_vcds_diag_sequences(data, &mut maps, &mut detected_addresses);
        
        maps.sort_by_key(|m| m.address);
        let filtered = self.filter_maps(maps);
        
        // Detect codeblocks and assign them to maps
        let codeblocks = self.detect_codeblocks(&filtered, data);
        let mut maps_with_codeblocks = self.assign_codeblocks_to_maps(filtered, &codeblocks);
        
        // If we have multiple codeblocks, search for similar maps in other codeblocks
        if codeblocks.len() >= 2 {
            maps_with_codeblocks = self.find_similar_maps_in_other_codeblocks(maps_with_codeblocks, &codeblocks, data);
        }
        
        // Classify maps using NameKnownMaps logic from zededc15pfile.cs
        let mut classified = self.name_known_maps(data, maps_with_codeblocks.clone());
        
        // Distinguish IQ by MAF and IQ by MAP based on X axis values
        classified = self.distinguish_iq_limiter_maps(data, classified);
        
        // Filter duplicate maps by type (one per flashbank)
        classified = self.filter_egr_maps(classified);
        
        // Filter nearby maps of same size - keep highest address (real map is after axis data)
        classified = self.filter_nearby_duplicate_maps(classified);
        
        // Filter false SOI maps - only keep numbered ones from detect_soi_maps_by_selector
        classified = self.filter_false_soi_maps(classified);
        
        // Fix Injector duration maps - renumber and swap axes
        classified = self.fix_injector_duration_maps(classified);
        
        // DISABLED: Inversed driver wish detection removed - no longer needed
        // classified = self.filter_zeroed_inversed_driver_wish(classified, data);

        // Filter invalid EGR temperature maps (missing temp axis or flat data)
        classified = self.filter_invalid_egr_temp_maps(classified, data);
        
        // Normalize diagnostics (VCDS / Measurement Blocks): force folder "VCDS diagnostic" and strip address suffix
        classified = self.normalize_diagnostics(classified);
        
        // NOTE: Hardcoded maps DISABLED for universal detection
        // All maps are now detected dynamically and filtered by filter_duplicate_maps_by_type
        // This ensures the detection works on ALL EDC15P files, not just specific ones
        /*
        let hardcoded = HardcodedEDC15PMaps::get_all_maps();
        for mut hc_map in hardcoded {
            // ... hardcoded map loading disabled ...
        }
        */
        
        // Filter: only keep maps with defined names (not "3D Map Size:" or "2D Map Size:") and exclude unneeded
        let final_maps: Vec<DetectedMap> = classified.into_iter()
            .filter(|map| {
                if let Some(ref name) = map.name {
                    // Must NOT start with generic map names (unclassified)
                    if name.starts_with("3D Map Size:") || name.starts_with("2D Map Size:") {
                        return false;
                    }
                    // Launch control : la zone 700 octets existe toujours dans
                    // le binaire, mais la carte n'est utilisable que si la
                    // solution l'a activée. On retire ici la version issue du
                    // balayage ; detect_launch_control la réinjecte seulement
                    // lorsque son axe de vitesse est réellement écrit.
                    if name.to_lowercase().contains("launch control") {
                        return false;
                    }
                    // Must NOT be in excluded categories
                    if let Some(ref subcat) = map.subcategory {
                        if subcat == "8-Toclass/Noneed" {
                            return false;
                        }
                    }
                    true
                } else {
                    false
                }
            })
            .collect();

        // Assign categories based on map names (for proper frontend folder display)
        let mut categorized_maps = self.assign_categories_by_name(final_maps);

        // Launch control : ajouté uniquement s'il est ACTIVÉ dans le fichier
        // (solution appliquée) — dossier « Launch control ».
        let mut lc_addresses: std::collections::HashSet<u32> =
            categorized_maps.iter().map(|m| m.address).collect();
        super::launch_control::detect_launch_control(data, &mut categorized_maps, &mut lc_addresses);

        // EGR temperature map : passe dédiée sur la structure des axes — le
        // balayage générique la rate quand la carte est uniforme (EGR à zéro).
        super::egr_temperature::detect_egr_temperature(data, &mut categorized_maps, &mut lc_addresses);

        log::debug!("✅ Dynamic detection: {} maps found in {} codeblocks ({} after filtering)",
                   maps_with_codeblocks.len(), codeblocks.len(), categorized_maps.len());
        categorized_maps
    }

    /// Assign categories to maps based on their names
    /// This ensures maps are displayed in the correct folders in the frontend
    fn assign_categories_by_name(&self, maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        maps.into_iter().map(|mut map| {
            if let Some(ref name) = map.name {
                let lower = name.to_lowercase();

                // BIP (injecteurs-pompe) : dossier Injection system, comme sur EDC16
                if lower.starts_with("bip") {
                    map.category = Some("Injection system".to_string());
                }
                // Start of Injection (dont « SOI selector »)
                else if lower.contains("start of injection") || lower.contains("soi") {
                    map.category = Some("Start of injection".to_string());
                }
                // Injector duration / Fuel
                else if lower.contains("injector duration") || lower.contains("selector for injector") {
                    map.category = Some("Injection system".to_string());
                }
                // Smoke limiters
                else if lower.contains("smoke") {
                    map.category = Some("Smoke limitation".to_string());
                }
                // IQ limiters
                else if lower.contains("iq by maf") || lower.contains("iq by map") || lower.contains("iq limiter") || lower.contains("iq by air intake") {
                    map.category = Some("Smoke limitation".to_string());
                }
                // Launch control : dossier dédié (la carte n'apparaît que
                // lorsque la solution l'a activée)
                else if lower.contains("launch control") {
                    map.category = Some("Launch control".to_string());
                }
                // Torque limiters
                else if lower.contains("torque limiter") {
                    map.category = Some("Engine torque limiters".to_string());
                }
                // Driver wish
                else if lower.contains("driver wish") {
                    map.category = Some("Engine fuel request".to_string());
                }
                // N75 duty cycle / Boost actuator -> Turbo Boost Pressure Control
                else if lower.contains("n75") || lower.contains("boost actuator") {
                    map.category = Some("Turbo boost pressure control".to_string());
                }
                // Boost / Turbo (other boost maps) -> Turbo boost pressure
                else if lower.contains("boost") || lower.contains("turbo") {
                    map.category = Some("Turbo boost pressure".to_string());
                }
                // EGR
                else if lower.contains("egr") {
                    map.category = Some("EGR".to_string());
                }
                // Idle RPM
                else if lower.contains("idle rpm") || lower.contains("idle speed") {
                    map.category = Some("Idle speed RPM".to_string());
                }
                // SVBL (boost limiter) -> Turbo boost pressure
                else if lower.contains("svbl") {
                    map.category = Some("Turbo boost pressure".to_string());
                }
                // SVRL (rev limiter) -> Maximum RPM limiter
                else if lower.contains("svrl") {
                    map.category = Some("Maximum RPM limiter".to_string());
                }
                // MAP/MAF switch (BEFORE VCDS and MAF checks)
                else if lower.contains("map/maf switch") {
                    map.category = Some("Smoke limitation".to_string());
                }
                // Diagnostics / VCDS / Measurement (BEFORE MAF check to catch "vcds maf display offset")
                else if lower.contains("vcds") || lower.contains("measurement") || lower.contains("diagnostic") {
                    map.category = Some("VCDS diagnostic".to_string());
                }
                // MAP linearization
                else if lower.contains("map linearization") {
                    map.category = Some("MAP sensor".to_string());
                }
                // Start IQ
                else if lower.contains("start iq") {
                    map.category = Some("Engine fuel request".to_string());
                }
                // Default: Other
                else if map.category.as_deref() == Some("Detected maps") {
                    map.category = Some("Other".to_string());
                }
            }
            map
        }).collect()
    }

    /// Check axis count and detect MapSelectors (like C# CheckAxisCount)
    /// Returns (axis_count, map_selectors)
    fn check_axis_count(&self, offset: usize, data: &[u8]) -> (usize, Vec<crate::models::MapSelectorInfo>) {
        let mut axis_count = 0;
        let mut map_selectors = Vec::new();
        let mut axis_found = true;
        let mut t = offset;
        
        // Count consecutive axes
        while axis_found && t < data.len().saturating_sub(4) {
            axis_found = false;
            let axis_id = u16::from_le_bytes([data[t], data[t + 1]]);
            
            if self.is_axis_id(axis_id) {
                let axis_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
                if axis_len > 0 && axis_len < 32 {
                    axis_count += 1;
                    axis_found = true;
                    t += 4 + (axis_len * 2);
                }
            }
        }
        
        // If 3 or more axes, search for selectors (like C# line 5939)
        // CRITICAL FIX: Changed from > 3 to >= 3 to detect SOI maps with Z-axis selector
        let mut bytes_to_search = 5120 + 16;
        if axis_count >= 3 {
            let _start_t = t;
            while bytes_to_search > 0 && t < data.len().saturating_sub(4) {
                let axis_id = u16::from_le_bytes([data[t], data[t + 1]]);
                
                if self.is_axis_id(axis_id) {
                    let axis_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
                    if axis_len <= 10 && axis_len > 0 {
                        // Check if this is a valid selector (like C# line 5950-5986)
                        let mut selector_valid = true;
                        let mut prev_selector = 0u16;
                        
                        // Read selector data (after axis data)
                        // C#: for (int i = 0; i < (axislen * 2); i += 2)
                        let selector_data_offset = t + 4 + (axis_len * 2);
                        if selector_data_offset + (axis_len * 2) <= data.len() {
                            for i in 0..axis_len {
                                let data_offset = selector_data_offset + (i * 2);
                                if data_offset + 1 >= data.len() {
                                    selector_valid = false;
                                    break;
                                }
                                
                                // C#: uint selValue = Convert.ToUInt32(allBytes[t + 4 + (axislen * 2) + i]) + Convert.ToUInt32(allBytes[t + 4 + (axislen * 2) + 1 + i]);
                                // This is NOT little endian! It's byte[0] + byte[1] (not swapped)
                                let sel_value = data[data_offset] as u16 + ((data[data_offset + 1] as u16) << 8);
                                
                                // Check validation rules (like C# line 5963-5977)
                                // C#: if (allBytes[t + 4 + (axislen * 2) + i] != 0)
                                if data[data_offset] != 0 {
                                    // C#: if(allBytes[t + 4 + (axislen * 2) + i] != 0x40) selectorValid = false;
                                    if data[data_offset] != 0x40 {
                                        selector_valid = false;
                                    }
                                    break;
                                }
                                // C#: if (allBytes[t + 4 + (axislen * 2) + 1 + i] > 9)
                                if data[data_offset + 1] > 9 {
                                    selector_valid = false;
                                    break;
                                }
                                // C#: if (prevSelector > selValue)
                                if prev_selector > sel_value {
                                    selector_valid = false;
                                    break;
                                }
                                prev_selector = sel_value;
                            }
                            
                            if selector_valid {
                                // Create MapSelector (like C# line 5991-6004)
                                let mut map_data = Vec::new();
                                let mut map_indexes = Vec::new();
                                
                                // Read MapData (axis values)
                                for i in 0..axis_len {
                                    let data_offset = t + 4 + (i * 2);
                                    if data_offset + 2 <= data.len() {
                                        let value = u16::from_le_bytes([data[data_offset], data[data_offset + 1]]);
                                        map_data.push(value);
                                    }
                                }
                                
                                // Read MapIndexes (selector values)
                                // C#: uint selValue = Convert.ToUInt32(allBytes[t + 4 + (axislen * 2) + i]) + Convert.ToUInt32(allBytes[t + 4 + (axislen * 2) + 1 + i]);
                                // This is byte[0] + byte[1] (not little endian, just addition)
                                for i in 0..axis_len {
                                    let data_offset = selector_data_offset + (i * 2);
                                    if data_offset + 1 < data.len() {
                                        let value = data[data_offset] as u16 + (data[data_offset + 1] as u16);
                                        map_indexes.push(value);
                                    }
                                }
                                
                                map_selectors.push(crate::models::MapSelectorInfo {
                                    num_repeats: axis_len,
                                    selector_address: t as u32,
                                    map_data,
                                    map_indexes,
                                });
                                
                                if map_selectors.len() > 5 {
                                    break;
                                }
                                
                                bytes_to_search = 5120 + 16; // Reset search range
                            }
                        }
                    }
                }
                t += 2;
                bytes_to_search -= 2;
            }
        }
        
        (axis_count, map_selectors)
    }

    /// CheckMap implementation following C# logic exactly
    /// Structure: [X_ID][X_len][X_data][Y_ID][Y_len][Y_data][Map_data]
    /// Returns Vec of maps (multiple maps if MapSelector is detected)
    fn check_map(&self, data: &[u8], t: usize) -> Option<(Vec<DetectedMap>, usize)> {
        if t + 100 > data.len() {
            return None;
        }
        
        // Check axis count and detect MapSelectors (like C# line 5569)
        let (axis_count, map_selectors) = self.check_axis_count(t, data);
        let _dont_gen_maps = axis_count > 3; // Like C# line 5577

        // Read X axis ID (LITTLE ENDIAN!)
        // C#: int xaxisid = (allBytes[t + 1] * 256) + allBytes[t]
        let x_axis_id = u16::from_le_bytes([data[t], data[t + 1]]);

        // Check if it's a valid axis ID
        if !self.is_axis_id(x_axis_id) {
            return None;
        }

        // Read X axis length (LITTLE ENDIAN!)
        // C#: int xaxislen = (allBytes[t + 3] * 256) + allBytes[t + 2]
        let x_axis_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;

        // Validate X axis length
        if !self.is_valid_length(x_axis_len, x_axis_id) {
            return None;
        }

        // Calculate Y axis position
        // C#: t + 4 + (xaxislen * 2)
        let y_offset = t + 4 + (x_axis_len * 2);
        
        if y_offset + 4 > data.len() {
            return None;
        }

        // Read Y axis ID (LITTLE ENDIAN!)
        let y_axis_id = u16::from_le_bytes([data[y_offset], data[y_offset + 1]]);

        // Read Y axis length (LITTLE ENDIAN!)
        let y_axis_len = u16::from_le_bytes([data[y_offset + 2], data[y_offset + 3]]) as usize;

        // Check if Y axis is valid - if not, create a 1D map (like C# line 6537-6557)
        let is_2d_map = self.is_axis_id(y_axis_id) && self.is_valid_length(y_axis_len, y_axis_id);

        // Calculate map data position and length
        let (map_offset, map_length) = if is_2d_map {
            // 2D map: map starts after both axes
            (t + 8 + (x_axis_len * 2) + (y_axis_len * 2), x_axis_len * y_axis_len * 2)
        } else {
            // 1D map: map starts after X axis only
            (t + 4 + (x_axis_len * 2), x_axis_len * 2)
        };

        if map_offset + map_length > data.len() {
            return None;
        }

        // VAGEDCSuite does NOT validate map data - it adds ALL detected maps!
        // No verify_map_data call here - just check bounds
        
        // For 1D maps, create a simple map without pattern matching
        if !is_2d_map {
            let mut map = DetectedMap::new(
                map_offset as u32,
                map_length,
                MapDimensions::TwoDimensional { rows: 1, cols: x_axis_len },
                DataType::UInt16,
            );
            map.name = Some(format!("2D Map Size: {}x1 Loc: {:06X}", x_axis_len, map_offset));
            map.confidence = 0.4;
            map.x_axis_address = Some((t + 4) as u32);
            map.y_axis_address = None;
            map.x_axis_correction = Some(1.0);
            map.correction_factor = Some(1.0);
            map.offset = Some(0.0);
            return Some((vec![map], map_offset + map_length - t));
        }

        // Try to match against known patterns first (preferred - has metadata)
        if map_length == 280 {
            log::debug!("🔍 check_map: size 280 detected! x_id=0x{:04X}, y_id=0x{:04X}, x_len={}, y_len={}",
                x_axis_id, y_axis_id, x_axis_len, y_axis_len);
        }
        if let Some(pattern) = self.find_matching_pattern(map_length, x_axis_id, y_axis_id, x_axis_len, y_axis_len) {
            if map_length == 280 {
                log::debug!("🔍 Pattern matched: '{}' for size 280", pattern.name);
            }
            let mut maps = Vec::new();
            
            // If MapSelector is detected, create multiple maps (like C# line 5763-5792)
            // IMPORTANT: Check ALL MapSelectors, not just the first one
            // CRITICAL: Only create maps where map_indexes[i] > 0 (active maps)
            if !map_selectors.is_empty() && !_dont_gen_maps {
                for ms in &map_selectors {
                    if ms.num_repeats > 0 {
                        // Calculate map size from memory between map start and selector
                        let selector_addr = ms.selector_address as usize;
                        if selector_addr > map_offset {
                            let memsize = (selector_addr - map_offset) / 2; // in words
                            let mapsize = memsize / ms.num_repeats;

                            if (x_axis_len * y_axis_len) == mapsize {
                                // Create multiple maps (like C# line 5763-5792)
                                // CRITICAL: Only create maps where map_indexes[i] > 0
                                let mut current_address = map_offset;
                                for i in 0..ms.num_repeats {
                                    // Check if this map slot is active (like C# MapIndexes[maprepeat] > 0)
                                    let is_active = if i == 0 {
                                        true // First map is always active
                                    } else {
                                        ms.map_indexes.get(i).map_or(false, |&idx| idx > 0)
                                    };

                                    if is_active {
                                        let mut map = self.create_map_from_pattern(
                                            pattern,
                                            current_address as u32,
                                            x_axis_id,
                                            y_axis_id,
                                            x_axis_len,
                                            y_axis_len,
                                            t as u32,
                                        );

                                        // For temperature-based maps, add temperature to name
                                        if ms.num_repeats > 1 && !ms.map_data.is_empty() {
                                            if let Some(&temp_raw) = ms.map_data.get(i) {
                                                // Convert raw value to Celsius: value * 0.1 - 273.1
                                                let temp_celsius = (temp_raw as f64 * 0.1) - 273.1;
                                                let temp_rounded = temp_celsius.round() as i32;
                                                // Update map name with temperature
                                                if let Some(ref name) = map.name {
                                                    let base_name = name.split(" [").next().unwrap_or(name);
                                                    map.name = Some(format!("{} {} °C", base_name, temp_rounded));
                                                }
                                            }
                                        }

                                        map.map_selector = Some(ms.clone());
                                        maps.push(map);
                                    }
                                    current_address += mapsize * 2;
                                }

                                // C#: lastFlashAddress += ms.NumRepeats * 4 + 4;
                                let last_flash_address = current_address + (ms.num_repeats * 4) + 4;

                                // Calculate total size to skip
                                let total_size = last_flash_address - t;
                                return Some((maps, total_size));
                            }
                        }
                    }
                }
            }
            
            // Single map (no MapSelector or MapSelector doesn't match)
            let mut map = self.create_map_from_pattern(
                pattern,
                map_offset as u32,
                x_axis_id,
                y_axis_id,
                x_axis_len,
                y_axis_len,
                t as u32,
            );
            
            // Attach MapSelector if found (for SOI maps, etc.)
            if !map_selectors.is_empty() {
                map.map_selector = Some(map_selectors[0].clone());
            }
            
            maps.push(map);
            
            // Calculate total size to skip
            let total_size = map_offset - t + map_length;
            return Some((maps, total_size));
        }
        
        // Check for Z axis (third axis) - like in C# CheckMap
        // C#: int zaxisid = (Convert.ToInt32(allBytes[t + 9 + (xaxislen * 2) + (yaxislen * 2)]) * 256) + Convert.ToInt32(allBytes[t + 8 + (xaxislen * 2) + (yaxislen * 2)]);
        let z_axis_offset = map_offset;
        let mut last_flash_address = map_offset;
        let mut z_axis_len = 0;
        let mut maxis_id = 0u16;
        let mut maxis_len = 0;
        let mut _maxis_address = 0;
        
        if z_axis_offset + 4 <= data.len() {
            let z_axis_id = u16::from_le_bytes([data[z_axis_offset], data[z_axis_offset + 1]]);
            if self.is_axis_id(z_axis_id) {
                z_axis_len = u16::from_le_bytes([data[z_axis_offset + 2], data[z_axis_offset + 3]]) as usize;
                if self.is_valid_length(z_axis_len, z_axis_id) {
                    // Found a Z axis! Calculate adjusted map offset (like C# line 5621-5623)
                    let len2skip = 4 + z_axis_len * 2;
                    let adjusted_len2skip = if len2skip < 16 { 16 } else { len2skip };
                    last_flash_address = map_offset + adjusted_len2skip;
                    
                    // Check for M axis (fourth axis) - like C# line 5733-5742
                    let maxis_offset = t + 13 + (x_axis_len * 2) + (y_axis_len * 2) + (z_axis_len * 2);
                    if maxis_offset + 4 <= data.len() {
                        maxis_id = u16::from_le_bytes([data[maxis_offset], data[maxis_offset + 1]]);
                        if self.is_axis_id(maxis_id) {
                            maxis_len = u16::from_le_bytes([data[maxis_offset + 2], data[maxis_offset + 3]]) as usize;
                            if self.is_valid_length(maxis_len, maxis_id) {
                                _maxis_address = t + 16 + (x_axis_len * 2) + (y_axis_len * 2);
                                last_flash_address += (maxis_len * 2) + 4;
                            }
                        }
                    }
                    
                    // Now handle MapSelectors (like C# line 5746-5794)
                    // CRITICAL: Only create maps where map_indexes[i] > 0 (active maps)
                    if !_dont_gen_maps && !map_selectors.is_empty() {
                        let mut maps = Vec::new();

                        for ms in &map_selectors {
                            // Check memory size between map start and selector (like C# line 5751-5752)
                            let selector_addr = ms.selector_address as usize;
                            if selector_addr > last_flash_address {
                                let memsize = (selector_addr - last_flash_address) / 2; // in words
                                if ms.num_repeats > 0 {
                                    let mapsize = memsize / ms.num_repeats;

                                    // Check if first axis set matches (like C# line 5756)
                                    if (x_axis_len * y_axis_len) == mapsize {
                                        // Create multiple maps (like C# line 5763-5792)
                                        let mut current_address = last_flash_address;
                                        for i in 0..ms.num_repeats {
                                            // Check if this map slot is active
                                            let is_active = if i == 0 {
                                                true
                                            } else {
                                                ms.map_indexes.get(i).map_or(false, |&idx| idx > 0)
                                            };

                                            if is_active {
                                                let mut map = self.create_generic_map(
                                                    current_address as u32,
                                                    x_axis_id,
                                                    y_axis_id,
                                                    x_axis_len,
                                                    y_axis_len,
                                                    t as u32,
                                                );

                                                // Add temperature to name if applicable
                                                if ms.num_repeats > 1 && !ms.map_data.is_empty() {
                                                    if let Some(&temp_raw) = ms.map_data.get(i) {
                                                        let temp_celsius = (temp_raw as f64 * 0.1) - 273.1;
                                                        let temp_rounded = temp_celsius.round() as i32;
                                                        if let Some(ref name) = map.name {
                                                            let base_name = name.split(" [").next().unwrap_or(name);
                                                            map.name = Some(format!("{} {} °C", base_name, temp_rounded));
                                                        }
                                                    }
                                                }

                                                map.map_selector = Some(ms.clone());
                                                maps.push(map);
                                            }
                                            current_address += mapsize * 2;
                                        }

                                        last_flash_address = current_address + (ms.num_repeats * 4) + 4;

                                        // Calculate total size to skip
                                        let total_size = last_flash_address - t;
                                        return Some((maps, total_size));
                                    }
                                    // Check if second axis set matches (like C# line 5795)
                                    else if (z_axis_len * maxis_len) == mapsize && maxis_len > 0 {
                                        // Create multiple maps with second axis set
                                        let mut current_address = last_flash_address;
                                        for i in 0..ms.num_repeats {
                                            let is_active = if i == 0 {
                                                true
                                            } else {
                                                ms.map_indexes.get(i).map_or(false, |&idx| idx > 0)
                                            };

                                            if is_active {
                                                let mut map = self.create_generic_map(
                                                    current_address as u32,
                                                    maxis_id,
                                                    z_axis_id,
                                                    maxis_len,
                                                    z_axis_len,
                                                    t as u32,
                                                );

                                                if ms.num_repeats > 1 && !ms.map_data.is_empty() {
                                                    if let Some(&temp_raw) = ms.map_data.get(i) {
                                                        let temp_celsius = (temp_raw as f64 * 0.1) - 273.1;
                                                        let temp_rounded = temp_celsius.round() as i32;
                                                        if let Some(ref name) = map.name {
                                                            let base_name = name.split(" [").next().unwrap_or(name);
                                                            map.name = Some(format!("{} {} °C", base_name, temp_rounded));
                                                        }
                                                    }
                                                }

                                                map.map_selector = Some(ms.clone());
                                                maps.push(map);
                                            }
                                            current_address += mapsize * 2;
                                        }

                                        last_flash_address = current_address + (ms.num_repeats * 4) + 4;

                                        let total_size = last_flash_address - t;
                                        return Some((maps, total_size));
                                    }
                                }
                            }
                        }
                    }
                    
                    // Single map with z-axis (no MapSelector match)
                    if last_flash_address + map_length <= data.len() {
                        let mut map = self.create_generic_map(
                            last_flash_address as u32,
                            x_axis_id,
                            y_axis_id,
                            x_axis_len,
                            y_axis_len,
                            t as u32,
                        );
                        
                        if !map_selectors.is_empty() {
                            map.map_selector = Some(map_selectors[0].clone());
                        }
                        
                        let total_size = last_flash_address - t + map_length;
                        return Some((vec![map], total_size));
                    }
                }
            }
        }
        
        // VAGEDCSuite adds ALL detected maps, even without pattern match!
        // They create a generic map with a name like "3D Map Size: ..."
        // Only check: length < 800 bytes (from AddToSymbolCollection line 6188)
        if map_length < 800 {
            let mut maps = Vec::new();
            
            // If MapSelector is detected, create multiple maps
            // IMPORTANT: Check ALL MapSelectors, not just the first one
            // CRITICAL: Only create maps where map_indexes[i] > 0 (active maps)
            if !map_selectors.is_empty() && !_dont_gen_maps {
                for ms in &map_selectors {
                    if ms.num_repeats > 0 {
                        let selector_addr = ms.selector_address as usize;
                        if selector_addr > map_offset {
                            let memsize = (selector_addr - map_offset) / 2; // in words
                            let mapsize = memsize / ms.num_repeats;

                            if (x_axis_len * y_axis_len) == mapsize {
                                // Create multiple maps
                                let mut current_address = map_offset;
                                for i in 0..ms.num_repeats {
                                    let is_active = if i == 0 {
                                        true
                                    } else {
                                        ms.map_indexes.get(i).map_or(false, |&idx| idx > 0)
                                    };

                                    if is_active {
                                        let mut map = self.create_generic_map(
                                            current_address as u32,
                                            x_axis_id,
                                            y_axis_id,
                                            x_axis_len,
                                            y_axis_len,
                                            t as u32,
                                        );

                                        if ms.num_repeats > 1 && !ms.map_data.is_empty() {
                                            if let Some(&temp_raw) = ms.map_data.get(i) {
                                                let temp_celsius = (temp_raw as f64 * 0.1) - 273.1;
                                                let temp_rounded = temp_celsius.round() as i32;
                                                if let Some(ref name) = map.name {
                                                    let base_name = name.split(" [").next().unwrap_or(name);
                                                    map.name = Some(format!("{} {} °C", base_name, temp_rounded));
                                                }
                                            }
                                        }

                                        map.map_selector = Some(ms.clone());
                                        maps.push(map);
                                    }
                                    current_address += mapsize * 2;
                                }

                                // C#: lastFlashAddress += ms.NumRepeats * 4 + 4;
                                let last_flash_address = current_address + (ms.num_repeats * 4) + 4;

                                let total_size = last_flash_address - t;
                                return Some((maps, total_size));
                            }
                        }
                    }
                }
            }
            
            // Single generic map
            let mut map = self.create_generic_map(
                map_offset as u32,
                x_axis_id,
                y_axis_id,
                x_axis_len,
                y_axis_len,
                t as u32,
            );
            
            // Attach MapSelector if found (for SOI maps, etc.)
            if !map_selectors.is_empty() {
                map.map_selector = Some(map_selectors[0].clone());
            }
            
            maps.push(map);
            
            // Calculate total size to skip
            let total_size = map_offset - t + map_length;
            return Some((maps, total_size));
        }
        
        None
    }

    /// Special detection for SOI maps based on temperature axis (ID 0xC5, len=10)
    /// SOI maps have a unique structure: 10 consecutive maps of 448 bytes each,
    /// preceded by shared axes and followed by a temperature axis
    fn detect_soi_maps_by_selector(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        const SOI_MAP_SIZE: usize = 448; // 14x16 * 2 bytes
        const SOI_NUM_MAPS: usize = 10;
        
        // Search for temperature axis with ID 0xC5 and len=10
        // This axis contains temperatures in Kelvin * 10 (e.g., 2480 = -25°C)
        for t in (0..data.len().saturating_sub(50)).step_by(2) {
            let axis_id = u16::from_le_bytes([data[t], data[t + 1]]);
            let axis_id_high = (axis_id >> 8) as u8;
            
            // Check if this is a temperature axis (ID 0xC5)
            if axis_id_high == 0xC5 {
                if t + 3 >= data.len() { continue; }
                let axis_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
                
                // Check for exactly 10 elements (for SOI temperature selector)
                if axis_len == 10 {
                    // Read the temperature values
                    let mut temps_raw: Vec<u16> = Vec::new();
                    let mut valid_temps = true;
                    
                    for i in 0..axis_len {
                        let data_offset = t + 4 + (i * 2);
                        if data_offset + 1 >= data.len() {
                            valid_temps = false;
                            break;
                        }
                        let val = u16::from_le_bytes([data[data_offset], data[data_offset + 1]]);
                        // Temperature values should be in range 2400-3800 (Kelvin * 10)
                        // -25°C = 248.15K * 10 = 2481.5
                        // 110°C = 383.15K * 10 = 3831.5
                        if val < 2200 || val > 4000 {
                            valid_temps = false;
                            break;
                        }
                        temps_raw.push(val);
                    }
                    
                    if !valid_temps || temps_raw.len() != 10 { continue; }
                    
                    // Convert to Celsius: temp_c = (raw / 10) - 273.15
                    let temps_celsius: Vec<i32> = temps_raw.iter()
                        .map(|&v| ((v as f64 / 10.0) - 273.15).round() as i32)
                        .collect();
                    
                    // Verify temperatures are increasing (roughly)
                    let mut is_increasing = true;
                    for i in 1..temps_celsius.len() {
                        if temps_celsius[i] < temps_celsius[i-1] - 5 { // Allow small variations
                            is_increasing = false;
                                break;
                            }
                    }
                    
                    if !is_increasing { continue; }
                    
                    // Validation: Check temperature range is automotive (-40 to +110°C)
                    // All temps must be in this range for SOI maps
                    let all_in_range = temps_celsius.iter().all(|&temp| temp >= -40 && temp <= 110);
                    if !all_in_range {
                        log::debug!("⏭️ Skipping temperature axis at 0x{:X} - temps out of automotive range: {:?}", t, temps_celsius);
                        continue;
                    }
                    
                    // Check spread: first temp should be cold (< 10°C), last should be HOT (>= 85°C)
                    // Real SOI maps have last temp = 90°C, false positives have 80°C
                    let first_temp = temps_celsius[0];
                    let last_temp = temps_celsius[temps_celsius.len() - 1];
                    if first_temp > 10 || last_temp < 85 {
                        log::debug!("⏭️ Skipping temperature axis at 0x{:X} - range not typical for SOI (last temp {} < 85): {:?}", t, last_temp, temps_celsius);
                        continue;
                    }
                    
                    // Found a valid temperature axis!
                    // The 10 SOI maps are BEFORE this axis
                        let total_map_data = SOI_MAP_SIZE * SOI_NUM_MAPS;
                        if t < total_map_data { continue; }
                        
                        let first_map_addr = t - total_map_data;
                        
                    log::debug!("🔥 Found SOI temperature axis at 0x{:X}, temps: {:?}", t, temps_celsius);
                    log::debug!("   SOI maps start at 0x{:X}", first_map_addr);
                    
                    // Find shared axes for SOI maps
                    // Axes are located BEFORE the maps
                    // Y axis (RPM): ID 0xF9, len=16
                    // X axis (IQ): ID 0xEB, len=14
                    let mut shared_x_axis_addr: Option<u32> = None;
                    let mut shared_y_axis_addr: Option<u32> = None;
                    let mut shared_x_axis_id: Option<u16> = None;
                    let mut shared_y_axis_id: Option<u16> = None;
                    
                    // Search for Y axis (RPM) - ID 0xF9, length 16
                    let search_start = if first_map_addr > 3000 { first_map_addr - 3000 } else { 0 };
                    for offset in (search_start..first_map_addr).step_by(2) {
                        if offset + 36 >= data.len() { continue; } // 4 bytes header + 32 bytes (16 values)
                        
                        let id = u16::from_le_bytes([data[offset], data[offset + 1]]);
                        let id_high = (id >> 8) as u8;
                        let len = u16::from_le_bytes([data[offset + 2], data[offset + 3]]) as usize;
                        
                        if id_high == 0xF9 && len == 16 {
                            // Verify values look like RPM (100-5000 range)
                            let first_val = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
                            let last_val = u16::from_le_bytes([data[offset + 4 + 30], data[offset + 4 + 31]]);
                            if first_val >= 100 && first_val <= 1000 && last_val >= 3000 && last_val <= 6000 {
                                shared_y_axis_addr = Some((offset + 4) as u32);
                                shared_y_axis_id = Some(id);
                                log::debug!("  ✅ Found Y axis (RPM) at 0x{:X} (data at 0x{:X}), ID=0x{:04X}", offset, offset + 4, id);
                                break;
                            }
                        }
                    }
                    
                    // Search for X axis (IQ) - ID 0xEB, length 14
                    for offset in (search_start..first_map_addr).step_by(2) {
                        if offset + 32 >= data.len() { continue; } // 4 bytes header + 28 bytes (14 values)
                        
                        let id = u16::from_le_bytes([data[offset], data[offset + 1]]);
                        let id_high = (id >> 8) as u8;
                        let len = u16::from_le_bytes([data[offset + 2], data[offset + 3]]) as usize;
                        
                        if id_high == 0xEB && len == 14 {
                            // Verify values look like IQ (0-5500 raw range)
                            let first_val = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
                            if first_val == 0 {
                                shared_x_axis_addr = Some((offset + 4) as u32);
                                shared_x_axis_id = Some(id);
                                log::debug!("  ✅ Found X axis (IQ) at 0x{:X} (data at 0x{:X}), ID=0x{:04X}", offset, offset + 4, id);
                                break;
                            }
                        }
                    }
                    
                    // « SOI selector » : le sélecteur de température lui-même
                    // (1×10, K×10 → °C), listé par WinOLS/EDCSuite et demandé
                    // par les utilisateurs. Adresse = les valeurs de l'axe C5.
                    {
                        let sel_addr = (t + 4) as u32;
                        maps.retain(|m| m.address != sel_addr);
                        detected_addresses.remove(&sel_addr);
                        let mut sel = DetectedMap::new(
                            sel_addr,
                            SOI_NUM_MAPS * 2,
                            MapDimensions::TwoDimensional { rows: 1, cols: SOI_NUM_MAPS },
                            DataType::UInt16,
                        );
                        sel.name = Some("SOI selector".to_string());
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
                        detected_addresses.insert(sel_addr);
                        maps.push(sel);
                    }

                    // Create 10 SOI maps with their temperatures
                        for i in 0..SOI_NUM_MAPS {
                            let map_addr = (first_map_addr + (i * SOI_MAP_SIZE)) as u32;
                            
                        // Remove any existing map at this address - temperature-based detection takes priority
                        if detected_addresses.contains(&map_addr) {
                            maps.retain(|m| m.address != map_addr);
                            log::debug!("  ⚠️ Replacing existing SOI map at 0x{:X} with temperature-based version", map_addr);
                        }
                            
                            let mut map = DetectedMap::new(
                                map_addr,
                                SOI_MAP_SIZE,
                                MapDimensions::TwoDimensional { rows: 16, cols: 14 },
                                DataType::UInt16,
                            );
                            
                        // Use actual temperature from the selector
                        let temp = temps_celsius[i];
                        map.name = Some(format!("Start of injection (SOI) {}°C", temp));
                            map.category = Some("Detected maps".to_string());
                            map.subcategory = Some("1-Fuel".to_string());
                        map.description = Some(format!(
                            "SOI | X: IQ (mg/st) | Y: Engine speed (rpm) | Axis IDs: X=0x{:04X} Y=0x{:04X}",
                            shared_x_axis_id.unwrap_or(0),
                            shared_y_axis_id.unwrap_or(0)
                        ));
                            map.correction_factor = Some(-0.023437);
                            map.offset = Some(78.0);
                            map.x_axis_correction = Some(0.01);
                        map.y_axis_correction = Some(1.0);
                            map.confidence = 0.95;
                            
                            // Assign shared axis addresses (all SOI maps in same codeblock share these axes)
                            map.x_axis_address = shared_x_axis_addr;
                            map.y_axis_address = shared_y_axis_addr;
                            
                            // Create MapSelector info
                            map.map_selector = Some(crate::models::MapSelectorInfo {
                                num_repeats: SOI_NUM_MAPS,
                                selector_address: t as u32,
                            map_data: temps_raw.iter().map(|&v| v as u16).collect(),
                                map_indexes: (0..10).collect(),
                            });
                        
                        log::debug!("  📍 Created SOI map {} at 0x{:X} (temp: {}°C)", i + 1, map_addr, temp);
                            
                            detected_addresses.insert(map_addr);
                            maps.push(map);
                    }
                }
            }
        }
    }

    /// Detect Start IQ maps by searching for their characteristic axis pattern
    /// Start IQ maps have: Y axis (0xEC, RPM, 8 values) followed by X axis (0xC1, Temp, 9 values)
    /// Map size: 8 * 9 * 2 = 144 bytes
    fn detect_start_iq_maps(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        // Search for pattern: [0xEC** axis ID][08 00][8 values][0xC1** axis ID][09 00][9 values][map data]

        let mut t = 0x20000; // Start at typical data section

        while t < data.len().saturating_sub(200) {
            // Check for Y axis first (RPM): high byte 0xEC
            let id1 = u16::from_le_bytes([data[t], data[t + 1]]);
            let id1_high = (id1 >> 8) as u8;

            if id1_high == 0xEC {
                let len1 = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;

                // Check if this is RPM axis with 8 values
                if len1 == 8 {
                    // Calculate position of second axis
                    let axis2_offset = t + 4 + (len1 * 2);

                    if axis2_offset + 4 < data.len() {
                        let id2 = u16::from_le_bytes([data[axis2_offset], data[axis2_offset + 1]]);
                        let id2_high = (id2 >> 8) as u8;
                        let len2 = u16::from_le_bytes([data[axis2_offset + 2], data[axis2_offset + 3]]) as usize;

                        // Check if second axis is Temperature (0xC1) with 9 values
                        if id2_high == 0xC1 && len2 == 9 {
                            let map_offset = axis2_offset + 4 + (len2 * 2);
                            let map_size = len1 * len2 * 2; // 8 * 9 * 2 = 144 bytes
                            let already_detected = detected_addresses.contains(&(map_offset as u32));

                            if map_offset + map_size <= data.len() && !already_detected {
                                // Verify map data looks valid (not all zeros or all FFs)
                                let map_data = &data[map_offset..map_offset + map_size];
                                let non_zero = map_data.iter().filter(|&&b| b != 0x00 && b != 0xFF).count();

                                if non_zero > map_size / 4 {
                                    log::debug!("🎯 Found Start IQ map at 0x{:X} (Y axis at 0x{:X}, X axis at 0x{:X})",
                                        map_offset, t, axis2_offset);

                                    let start_iq = DetectedMap {
                                        id: format!("siq_{:06X}", map_offset),
                                        address: map_offset as u32,
                                        size: map_size,
                                        // Display as 9 cols (Temp) x 8 rows (RPM)
                                        dimensions: MapDimensions::TwoDimensional { rows: 8, cols: 9 },
                                        data_type: DataType::UInt16,
                                        name: Some("Start IQ".to_string()),
                                        category: Some("Engine fuel request".to_string()),
                                        subcategory: Some("4-Misc".to_string()),
                                        description: Some(format!(
                                            "Start IQ (mg/st) | X: Coolant temp (°C) | Y: Engine speed (rpm)"
                                        )),
                                        unit: Some("mg/st".to_string()),
                                        confidence: 0.9,
                                        // Swap addresses: X should be Temp, Y should be RPM
                                        x_axis_address: Some(axis2_offset as u32), // Temp axis
                                        y_axis_address: Some(t as u32),            // RPM axis
                                        correction_factor: Some(0.01),
                                        offset: Some(0.0),
                                        x_axis_correction: Some(0.1),    // Temp: factor 0.1
                                        x_axis_offset: Some(-273.1),      // Temp: offset -273.1 (Kelvin to Celsius)
                                        y_axis_correction: Some(1.0),    // RPM: factor 1.0
                                        y_axis_offset: Some(0.0),
                                        x_label: Some("degC".to_string()),
                                        y_label: Some("rpm".to_string()),
                                        y_axis_inverted: Some(true),
                                        is_little_endian: Some(true),
                                        codeblock_id: None,
                                        codeblock_start_address: None,
                                        codeblock_end_address: None,
                                        map_selector: None,
                                    };

                                    detected_addresses.insert(map_offset as u32);
                                    maps.push(start_iq);
                                }
                            }
                        }
                    }
                }
            }

            t += 2;
        }
    }

    /// Detect special maps using byte sequence patterns (like C# FindSVBL, FindSVRL, FindBIPline, etc.)
    /// These are single-value or special structure maps that can't be detected by the standard check_map algorithm
    fn detect_special_maps_by_sequence(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        // SVRL (Single Value Rev Limiter) - sequence: 00 0E xx xx 32 00 70 17
        self.find_svrl_sequence(data, maps, detected_addresses);
        
        // SVBL (Single Value Boost Limiter) - similar detection
        self.find_svbl_sequence(data, maps, detected_addresses);
        
        // Left Foot Brake Switch - sequence: 27 00 00 64 00 01
        self.find_left_foot_brake_switch(data, maps, detected_addresses);
        
        // MAP/MAF Switch - sequence: 41 02 xx xx 00 01 01 00
        self.find_map_maf_switch(data, maps, detected_addresses);
        
        // BIP temperature correction - sequence: 0A 00 4D 09 E3 09 47 0A
        self.find_bip_temp_correction(data, maps, detected_addresses);
        
        // Selector for injector duration - 1x6 map with specific pattern
        self.find_selector_for_injector_duration(data, maps, detected_addresses);
        
        // Boost target map detection - look for specific patterns
        self.find_boost_target_maps(data, maps, detected_addresses);
        
        // Boost correction by temperature - both axes have ID 0xDA
        self.find_boost_correction_by_temp(data, maps, detected_addresses);

        // IQ by air intake temp - 3x3 map with 0xC1 (temp) + 0xEC (rpm) axes
        self.find_iq_by_air_intake_temp(data, maps, detected_addresses);
        
        // NOTE: find_iq_limiter_maps DISABLED - uses hardcoded addresses that don't work for all files
        // IQ by MAF and MAP are now detected dynamically via patterns and classify_maps_by_data
        // self.find_iq_limiter_maps(data, maps, detected_addresses);
        
        // Driver wish maps - 256 bytes (8x16) with EC/C0 axis IDs
        self.find_driver_wish_maps(data, maps, detected_addresses);
        
        // Idle RPM detection - typically 1x2 or 1x1 maps
        self.find_idle_rpm_maps(data, maps, detected_addresses);

        // NOTE: Smoke limiter detection moved to beginning of detect() function
        // to ensure multi-smoke limiters are found before generic detection
    }

    /// Find SVRL (Single Value Rev Limiter) by byte sequence
    /// Pattern: SVRL is always AFTER the sequence 0727 0E00
    /// Example: 0727 0E00 1432 -> SVRL = 1432 (5170 RPM)
    /// Example: 0727 0E00 11A8 -> SVRL = 11A8 (4520 RPM)
    /// In little endian bytes: [27 07 00 0E] then [SVRL 2 bytes]
    fn find_svrl_sequence(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        // Pattern: 0727 0E00 in little endian = [27 07 00 0E]
        // SVRL is the 2 bytes immediately AFTER this pattern
        let marker_pattern: [u8; 4] = [0x27, 0x07, 0x00, 0x0E];
        
        let mut offset = 0;
        while offset < data.len().saturating_sub(6) {
            // Search for the marker pattern
            if let Some(found_offset) = self.find_exact_sequence(data, offset, &marker_pattern) {
                // SVRL is 4 bytes after the start of the marker (right after the pattern)
                let svrl_address = (found_offset + 4) as u32;
                
                if (svrl_address as usize) + 2 <= data.len() && !detected_addresses.contains(&svrl_address) {
                    let value = u16::from_le_bytes([data[svrl_address as usize], data[svrl_address as usize + 1]]);
                    
                    // Validate: SVRL values are typically 4000-7000 RPM
                    if value >= 3000 && value <= 8000 {
                        let mut map = DetectedMap::new(
                            svrl_address,
                            2,
                            MapDimensions::TwoDimensional { rows: 1, cols: 1 },
                            DataType::UInt16,
                        );
                        
                        map.name = Some("SVRL - RPM Limiter".to_string());
                        map.subcategory = Some("Maximum RPM limiter".to_string());
                        map.category = Some("Maximum RPM limiter".to_string());
                        map.description = Some(format!("Maximum engine RPM limit: {} rpm", value));
                        map.unit = Some("rpm".to_string());
                        map.confidence = 0.98;
                        map.correction_factor = Some(1.0);
                        
                        log::debug!("🎯 Found SVRL at 0x{:X} = {} rpm (marker at 0x{:X})", svrl_address, value, found_offset);
                        
                        detected_addresses.insert(svrl_address);
                        maps.push(map);
                    }
                }
                
                offset = found_offset + 1;
            } else {
                break;
            }
        }
    }

    /// Find SVBL (Single Value Boost Limiter) by byte sequence
    /// Pattern: SVBL is BEFORE the sequence 7ADF 0028
    /// Case 1: [SVBL] [DF 7A 28 00] - SVBL at offset -2 (e.g. v17g4.bin)
    /// Case 2: [SVBL] [00 C3] [DF 7A 28 00] - SVBL at offset -4 (e.g. v38test.Bin)
    fn find_svbl_sequence(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        // Pattern: 7ADF 0028 in little endian = [DF 7A 28 00]
        let marker_pattern: [u8; 4] = [0xDF, 0x7A, 0x28, 0x00];
        
        let mut offset = 4; // Start at 4 to allow reading 4 bytes before
        while offset < data.len().saturating_sub(4) {
            // Search for the marker pattern
            if let Some(found_offset) = self.find_exact_sequence(data, offset, &marker_pattern) {
                // Check if there's C300 (00 C3) before the marker
                // If [00 C3] is at offset -2, then SVBL is at offset -4
                // Otherwise SVBL is at offset -2
                let svbl_offset = if found_offset >= 4 
                    && data[found_offset - 2] == 0x00 
                    && data[found_offset - 1] == 0xC3 {
                    // Pattern: [SVBL] [00 C3] [DF 7A 28 00]
                    found_offset - 4
                } else if found_offset >= 2 {
                    // Pattern: [SVBL] [DF 7A 28 00]
                    found_offset - 2
                } else {
                    offset = found_offset + 1;
                    continue;
                };
                
                let svbl_address = svbl_offset as u32;
                
                if !detected_addresses.contains(&svbl_address) {
                    let value = u16::from_le_bytes([data[svbl_address as usize], data[svbl_address as usize + 1]]);
                    
                    // Validate: SVBL values are typically 0 (disabled) or 1000-7000 mbar
                    if value == 0 || (value >= 1000 && value <= 7000) {
                        let mut map = DetectedMap::new(
                            svbl_address,
                            2,
                            MapDimensions::TwoDimensional { rows: 1, cols: 1 },
                            DataType::UInt16,
                        );
                        
                        let status = if value == 0 { " (disabled)" } else { "" };
                        map.name = Some("SVBL (Single value boost limiter)".to_string());
                        map.subcategory = Some("Turbo boost pressure".to_string());
                        map.category = Some("Turbo boost pressure".to_string());
                        map.description = Some(format!("Maximum boost pressure limit: {} mbar{}", value, status));
                        map.unit = Some("mbar".to_string());
                        map.confidence = 0.98;
                        map.correction_factor = Some(1.0);
                        
                        log::debug!("🎯 Found SVBL at 0x{:X} = {} mbar{} (marker at 0x{:X})", svbl_address, value, status, found_offset);
                        
                        detected_addresses.insert(svbl_address);
                        maps.push(map);
                    }
                }
                
                offset = found_offset + 1;
            } else {
                break;
            }
        }
    }
    
    /// Find exact byte sequence in data (no mask)
    fn find_exact_sequence(&self, data: &[u8], start_offset: usize, pattern: &[u8]) -> Option<usize> {
        if start_offset + pattern.len() > data.len() {
            return None;
        }
        
        for i in start_offset..=(data.len() - pattern.len()) {
            let mut matches = true;
            for (j, &p) in pattern.iter().enumerate() {
                if data[i + j] != p {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Some(i);
            }
        }
        None
    }
    
    /// Find Left Foot Brake Switch by byte sequence
    /// C# pattern: { 0x27, 0x00, 0x00, 0x64, 0x00, 0x01 } with mask { 1, 1, 1, 1, 1, 0 }
    /// Limit to one per codeblock to avoid duplicates
    fn find_left_foot_brake_switch(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        let pattern: [u8; 6] = [0x27, 0x00, 0x00, 0x64, 0x00, 0x01];
        let mask: [u8; 6] = [1, 1, 1, 1, 1, 0];
        
        // Codeblock boundaries (typical offsets)
        let _codeblock_ranges: [(u32, u32); 3] = [
            (0x4C000, 0x5C000),  // Codeblock 1
            (0x5C000, 0x6C000),  // Codeblock 2
            (0x6C000, 0x7C000),  // Codeblock 3
        ];
        
        let mut found_per_codeblock: [bool; 3] = [false; 3];
        
        let mut offset = 0;
        while offset < data.len().saturating_sub(6) {
            if let Some(found_offset) = self.find_sequence_with_mask(data, offset, &pattern, &mask) {
                let lfb_address = (found_offset + 5) as u32;
                
                // Determine which codeblock this belongs to
                // Use typical codeblock offset of 0x10000
                let codeblock_idx = if lfb_address >= 0x4C000 && lfb_address < 0x5C000 {
                    Some(0)  // Codeblock 1
                } else if lfb_address >= 0x5C000 && lfb_address < 0x6C000 {
                    Some(1)  // Codeblock 2
                } else if lfb_address >= 0x6C000 && lfb_address < 0x7C000 {
                    Some(2)  // Codeblock 3
                } else {
                    None
                };
                
                if let Some(cb_idx) = codeblock_idx {
                    // Only add if we haven't found one in this codeblock yet
                    if !found_per_codeblock[cb_idx] && !detected_addresses.contains(&lfb_address) {
                        log::debug!("🎯 Found Left foot brake switch at 0x{:X} in codeblock {}", lfb_address, cb_idx + 1);
                        // Read the actual value
                        let value = if (lfb_address as usize) + 2 <= data.len() {
                            u16::from_le_bytes([data[lfb_address as usize], data[lfb_address as usize + 1]])
                        } else { 0 };
                        let state = if value == 1 { "ON" } else { "OFF" };
                        
                        let mut map = DetectedMap::new(
                            lfb_address,
                            2,
                            MapDimensions::TwoDimensional { rows: 1, cols: 1 },
                            DataType::UInt16,
                        );
                        
                        map.name = Some("Left foot brake switch".to_string());
                        map.category = Some("Other".to_string());
                        map.subcategory = Some("Switches".to_string());
                        map.description = Some(format!("Left foot brake: {} (1=ON, 0=OFF)", state));
                        map.confidence = 0.95;
                        map.codeblock_id = Some(cb_idx as u32 + 1);
                        
                        detected_addresses.insert(lfb_address);
                        found_per_codeblock[cb_idx] = true;
                        maps.push(map);
                    }
                }
                
                offset = found_offset + 1;
            } else {
                break;
            }
        }
    }

    /// Find MAP/MAF Switch by byte sequence
    /// C# pattern: { 0x41, 0x02, 0xFF, 0xFF, 0x00, 0x01, 0x01, 0x00 } with mask { 1, 1, 0, 0, 1, 1, 1, 1 }
    /// Limit to one per codeblock to avoid duplicates
    fn find_map_maf_switch(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        let pattern: [u8; 8] = [0x41, 0x02, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00];
        let mask: [u8; 8] = [1, 1, 0, 0, 1, 1, 1, 1];
        
        // Codeblock boundaries (typical offsets)
        let _codeblock_ranges: [(u32, u32); 3] = [
            (0x4C000, 0x5C000),  // Codeblock 1
            (0x5C000, 0x6C000),  // Codeblock 2
            (0x6C000, 0x7C000),  // Codeblock 3
        ];
        
        let mut found_per_codeblock: [bool; 3] = [false; 3];
        
        let mut offset = 0;
        while offset < data.len().saturating_sub(8) {
            if let Some(found_offset) = self.find_sequence_with_mask(data, offset, &pattern, &mask) {
                let switch_address = (found_offset + 2) as u32;
                
                // Determine which codeblock this belongs to
                // Use typical codeblock offset of 0x10000
                let codeblock_idx = if switch_address >= 0x4C000 && switch_address < 0x5C000 {
                    Some(0)  // Codeblock 1
                } else if switch_address >= 0x5C000 && switch_address < 0x6C000 {
                    Some(1)  // Codeblock 2
                } else if switch_address >= 0x6C000 && switch_address < 0x7C000 {
                    Some(2)  // Codeblock 3
                } else {
                    None
                };
                
                if let Some(cb_idx) = codeblock_idx {
                    // Only add if we haven't found one in this codeblock yet
                    if !found_per_codeblock[cb_idx] && !detected_addresses.contains(&switch_address) {
                        log::debug!("🎯 Found MAP/MAF switch at 0x{:X} in codeblock {}", switch_address, cb_idx + 1);
                        // Read the actual value
                        let value = if (switch_address as usize) + 2 <= data.len() {
                            u16::from_le_bytes([data[switch_address as usize], data[switch_address as usize + 1]])
                        } else { 0 };
                        let mode = if value == 0 { "MAF" } else if value == 257 { "MAP" } else { "Unknown" };
                        
                        let mut map = DetectedMap::new(
                            switch_address,
                            2,
                            MapDimensions::TwoDimensional { rows: 1, cols: 1 },
                            DataType::UInt16,
                        );
                        
                        map.name = Some("MAP/MAF switch".to_string());
                        map.subcategory = Some("Smoke limitation".to_string());
                        map.category = Some("Smoke limitation".to_string());
                        map.description = Some(format!("Sensor mode: {} (0=MAF, 257=MAP)", mode));
                        map.confidence = 0.95;
                        map.codeblock_id = Some(cb_idx as u32 + 1);
                        
                        detected_addresses.insert(switch_address);
                        found_per_codeblock[cb_idx] = true;
                        maps.push(map);
                    }
                }
                
                offset = found_offset + 1;
            } else {
                break;
            }
        }
    }


    /// Find BIP temperature correction by byte sequence
    /// C# pattern: { 0x0A, 0x00, 0x4D, 0x09, 0xE3, 0x09, 0x47, 0x0A }
    fn find_bip_temp_correction(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        let pattern: [u8; 8] = [0x0A, 0x00, 0x4D, 0x09, 0xE3, 0x09, 0x47, 0x0A];
        
        let mut offset = 0;
        while offset < data.len().saturating_sub(42) {
            if let Some(found_offset) = self.find_sequence_exact(data, offset, &pattern) {
                let bip_temp_address = (found_offset + 22) as u32;
                let x_axis_address = (found_offset + 2) as u32;

                // Le balayage générique réserve souvent cette adresse avec une
                // carte sans nom (jetée ensuite) : la carte BIP nommée prend
                // le dessus — avant, elle était simplement ignorée et aucune
                // BIP n'apparaissait sur EDC15P.
                {
                    maps.retain(|m| m.address != bip_temp_address);
                    detected_addresses.remove(&bip_temp_address);
                    let mut map = DetectedMap::new(
                        bip_temp_address,
                        20,
                        MapDimensions::TwoDimensional { rows: 1, cols: 10 },
                        DataType::UInt16,
                    );

                    map.name = Some("BIP temperature correction".to_string());
                    map.description = Some("BIP correction vs coolant temperature | X: Temperature (°C)".to_string());
                    // Degrés vilebrequin : brut 273..256 × 0.023437 = 6.4..6.0 °
                    // (l'ancien 0.000244 donnait 0.07, sans sens physique)
                    map.unit = Some("°".to_string());
                    map.x_axis_address = Some(x_axis_address);
                    map.x_label = Some("°C".to_string());
                    map.x_axis_correction = Some(0.1);
                    map.x_axis_offset = Some(-273.1);
                    map.correction_factor = Some(0.023437);
                    map.confidence = 0.95;

                    detected_addresses.insert(bip_temp_address);
                    maps.push(map);
                }

                // Juste après : « BIP SOI correction » = axe C5 de 7 deltas
                // SOI signés (±384 brut = ±9 °) + 7 valeurs (polo 0x54680,
                // golf4 ARL 0x544F8 : même structure sur tous les fichiers P)
                let soi_axis = found_offset + 22 + 20;
                if soi_axis + 4 + 28 <= data.len()
                    && data[soi_axis + 1] == 0xC5
                    && u16::from_le_bytes([data[soi_axis + 2], data[soi_axis + 3]]) == 7
                {
                    let axis_data = (soi_axis + 4) as u32;
                    let map_addr = (soi_axis + 4 + 14) as u32;
                    maps.retain(|m| m.address != map_addr);
                    detected_addresses.remove(&map_addr);
                    let mut map = DetectedMap::new(
                        map_addr,
                        14,
                        MapDimensions::TwoDimensional { rows: 1, cols: 7 },
                        DataType::Int16,
                    );
                    map.name = Some("BIP SOI correction".to_string());
                    map.description = Some("BIP correction vs SOI delta | X: SOI delta (°)".to_string());
                    map.unit = Some("°".to_string());
                    map.x_axis_address = Some(axis_data);
                    map.x_label = Some("°".to_string());
                    map.x_axis_correction = Some(0.023437);
                    map.x_axis_offset = Some(0.0);
                    map.correction_factor = Some(0.023437);
                    map.confidence = 0.9;
                    detected_addresses.insert(map_addr);
                    maps.push(map);
                }
                
                offset = found_offset + 1;
            } else {
                break;
            }
        }
    }

    /// Find Selector for injector duration
    /// Structure: [Y_ID=EC][Y_len=6][Y_data=12bytes][Map_data=12bytes]
    /// Map values are: 0, 256, 512, 768, 1024, 1280 (index * 256)
    fn find_selector_for_injector_duration(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        log::debug!("🔍 Searching for Selector for injector duration by pattern...");
        
        // Pattern: 6 consecutive values with increment of 256
        // Original: [0, 256, 512, 768, 1024, 1280]
        // Tuned files may have first value modified: [256, 256, 512, 768, 1024, 1280]
        // We check that values 2-6 follow the expected pattern: [_, 256, 512, 768, 1024, 1280]
        
        // Search through the file for this pattern
        // These maps are typically in flashbanks (0x40000-0x7FFFF)
        let mut found_count = 0;
        let mut candidates: Vec<(u32, bool)> = Vec::new(); // (address, has_valid_axis)
        
        for offset in (0x40000..data.len().saturating_sub(24)).step_by(2) {
            // Check if values 2-6 match expected pattern (index 1-5)
            // First value (index 0) can be 0 or modified by tuning
            let mut is_match = true;
            
            // Values at indices 1-5 must be: 256, 512, 768, 1024, 1280
            let expected_vals: [u16; 5] = [256, 512, 768, 1024, 1280];
            
            for (i, &expected) in expected_vals.iter().enumerate() {
                let idx = offset + (i + 1) * 2; // Start from index 1
                if idx + 1 >= data.len() {
                    is_match = false;
                    break;
                }
                let val = u16::from_le_bytes([data[idx], data[idx + 1]]);
                if val != expected {
                    is_match = false;
                    break;
                }
            }
            
            // Also check that first value is reasonable (0-512 range)
            if is_match {
                let first_val = u16::from_le_bytes([data[offset], data[offset + 1]]);
                if first_val > 512 {
                    is_match = false;
                }
            }
            
            if !is_match { continue; }
            
            // STRICT validation: Y axis MUST have ID 0xEC with length 6
            // Axis header is at offset -16 from map (4 bytes header + 12 bytes data before map)
            let axis_offset = offset.saturating_sub(16);
            if axis_offset + 4 > data.len() { continue; }
            
            let axis_id = u16::from_le_bytes([data[axis_offset], data[axis_offset + 1]]);
            let axis_len = u16::from_le_bytes([data[axis_offset + 2], data[axis_offset + 3]]);
            let axis_id_high = (axis_id >> 8) as u8;
            
            // MUST have EC axis ID with length 6
            if axis_id_high != 0xEC || axis_len != 6 {
                log::debug!("⏭️ Skipping pattern at 0x{:X} - invalid axis (ID=0x{:04X}, len={})", offset, axis_id, axis_len);
                continue;
            }
            
            candidates.push((offset as u32, true));
        }
        
        // Only keep ONE selector per codeblock (the one closest to known relative offset)
        // Typical offset within codeblock: 0x545C (relative to codeblock start)
        // Codeblock starts: 0x4C000 (CB2), 0x5C000 (CB3), 0x6C000 (CB5)
        let mut cb_best: [Option<u32>; 3] = [None; 3]; // One for each codeblock (2, 3, 5 -> indices 0, 1, 2)
        
        for (addr, _) in &candidates {
            let flashbank = self.get_flashbank_from_address(*addr);
            // Calculate relative offset within codeblock
            let codeblock_start = match flashbank {
                Some(1) => 0x4C000u32,
                Some(2) => 0x5C000u32,
                Some(3) => 0x6C000u32,
                _ => continue,
            };
            let relative = (*addr).saturating_sub(codeblock_start) as i32;
            let target_relative = 0x945Ci32 - 0x4C000i32; // Known offset: 0x5545C - 0x4C000 = 0x845C
            let distance = (relative - target_relative).abs();
            
            if let Some(fb) = flashbank {
                let cb_idx = (fb - 1) as usize;
                if cb_idx < 3 {
                    if cb_best[cb_idx].is_none() {
                        cb_best[cb_idx] = Some(*addr);
                    } else {
                        let existing_addr = cb_best[cb_idx].unwrap();
                        let existing_relative = existing_addr.saturating_sub(codeblock_start) as i32;
                        if distance < (existing_relative - target_relative).abs() {
                            cb_best[cb_idx] = Some(*addr);
                        }
                    }
                }
            }
        }
        
        let final_addrs: Vec<u32> = cb_best.into_iter().flatten().collect();
        
        for map_addr in final_addrs {
            if detected_addresses.contains(&map_addr) { 
                maps.retain(|m| m.address != map_addr);
            }
            
            log::debug!("🎯 Found Selector for injector duration at 0x{:X}", map_addr);
            
            let mut map = DetectedMap::new(
                map_addr,
                12, // 6 values * 2 bytes
                MapDimensions::TwoDimensional { rows: 6, cols: 1 },
                DataType::UInt16,
            );
            
            map.name = Some("Selector for injector duration".to_string());
            map.category = Some("Detected maps".to_string());
            map.subcategory = Some("1-Fuel".to_string());
            
            // Y axis is at offset -12 from map (right after axis header)
            let axis_offset = (map_addr as usize).saturating_sub(16);
            map.y_axis_address = Some((axis_offset + 4) as u32);
            map.y_axis_correction = Some(-0.023437); // Negative factor
            map.y_axis_offset = Some(78.0); // Offset to get SOI values (0, 4, 9, 15, 21, 27)
            map.x_label = Some("".to_string()); // No X label for 1xN map
            map.y_label = Some("SOI".to_string()); // Start of Injection angle - just "SOI" not "SOIdegC"
            map.y_axis_inverted = Some(true); // Display with small values at top (like WinOLS)
            map.correction_factor = Some(0.003906); // 1/256 for map values
            map.confidence = 0.95;
            
            detected_addresses.insert(map_addr);
            maps.push(map);
            found_count += 1;
        }
        
        log::debug!("✅ Found {} Selector for injector duration maps", found_count);
    }


    /// Find Boost target maps - typically 320 bytes (16x10) with specific axis IDs
    fn find_boost_target_maps(&self, data: &[u8], maps: &mut Vec<DetectedMap>, _detected_addresses: &mut HashSet<u32>) {
        // Look for 320-byte maps with EC/EA axis IDs that haven't been classified
        // These are often Boost target maps
        for map in maps.iter_mut() {
            if map.size == 320 && map.subcategory.is_none() {
                // Check if this could be a Boost target map based on data values
                // Boost values are typically 1000-3000 mbar
                let map_start = map.address as usize;
                if map_start + 320 <= data.len() {
                    let mut valid_boost_values = 0;
                    for i in (0..320).step_by(2) {
                        if map_start + i + 2 <= data.len() {
                            let value = u16::from_le_bytes([data[map_start + i], data[map_start + i + 1]]);
                            if value >= 800 && value <= 3500 {
                                valid_boost_values += 1;
                            }
                        }
                    }
                    // If most values are in boost range, classify as Boost target
                    if valid_boost_values > 100 {
                        map.name = Some("Boost target map".to_string());
                        map.subcategory = Some("3-Turbo".to_string());
                        // Même vérité fichier que le classifieur 320 : X affiché
                        // = IQ (axe court, ×0.01), Y = RPM (axe long, ×1.0). Le
                        // scanner générique assigne X au premier axe du fichier
                        // (souvent le RPM) → on lit la longueur réelle de chaque
                        // axe ([ID u16][len u16 LE][valeurs], len à adresse-2)
                        // et on échange les adresses si X pointe l'axe long.
                        let axis_len_at = |addr: Option<u32>| -> Option<usize> {
                            let a = addr? as usize;
                            if a >= 2 && a <= data.len() {
                                Some(u16::from_le_bytes([data[a - 2], data[a - 1]]) as usize)
                            } else {
                                None
                            }
                        };
                        if let (Some(xl), Some(yl)) = (
                            axis_len_at(map.x_axis_address),
                            axis_len_at(map.y_axis_address),
                        ) {
                            if xl > yl && xl * yl * 2 == map.size {
                                std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                                map.dimensions = MapDimensions::TwoDimensional { rows: xl, cols: yl };
                                log::debug!("🔄 Boost target 320 (reclass): swapped axis addresses (X was RPM axis)");
                            }
                        }
                        // Build description with axis info if available
                        let x_axis_addr = map.x_axis_address.unwrap_or(0);
                        let y_axis_addr = map.y_axis_address.unwrap_or(0);
                        map.description = Some(format!(
                            "Target boost pressure (mbar) | X: IQ (mg/st) | Y: Engine speed (rpm) | Axis IDs: X=0x{:04X} Y=0x{:04X}",
                            x_axis_addr, y_axis_addr
                        ));
                        map.unit = Some("mbar".to_string());
                        map.correction_factor = Some(1.0);
                        map.x_axis_correction = Some(0.01); // IQ needs *0.01
                        map.y_axis_correction = Some(1.0);  // RPM as-is
                    }
                }
            }
        }
    }

    /// Find Boost correction by temperature maps - 320 bytes (10x16) with both axes having ID 0xDA
    /// Structure: [Y_ID=DA][Y_len=16][Y_data(32)][X_ID=DA][X_len=10][X_data(20)][Map(320)]
    fn find_boost_correction_by_temp(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        log::debug!("🔍 Searching for Boost correction by temperature maps...");
        // Boost correction by temperature characteristics:
        // - Size: 320 bytes (10 cols x 16 rows x 2 bytes)
        // - X axis: IAT (°C) with factor 0.1, offset -273, 10 values, ID high byte DA
        // - Y axis: Requested boost (mbar) with factor 1.0, 16 values, ID high byte DA
        // - Both axes have the same ID high byte (0xDA)
        
        for t in (0..data.len().saturating_sub(400)).step_by(2) {
            if t + 4 > data.len() { continue; }
            
            // Look for first axis header (DA**) with length 16 (Y axis = boost)
            let potential_y_id = u16::from_le_bytes([data[t], data[t + 1]]);
            let y_id_high = (potential_y_id >> 8) as u8;
            
            if y_id_high != 0xDA { continue; }

            // 16 lignes de boost en standard, mais la variante 6L AXR n'en a
            // que 6 ([DA6C][6] à 0x52030) — accepter 4-16
            let y_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
            if !(4..=16).contains(&y_len) { continue; }

            // Y axis data follows: y_len values * 2 bytes
            let x_axis_start = t + 4 + (y_len * 2);
            if x_axis_start + 4 > data.len() { continue; }

            // Look for second axis header (DA**) — 10 températures en
            // standard, 9 sur l'AXR ([DA6A][9])
            let potential_x_id = u16::from_le_bytes([data[x_axis_start], data[x_axis_start + 1]]);
            let x_id_high = (potential_x_id >> 8) as u8;

            if x_id_high != 0xDA { continue; }

            let x_len = u16::from_le_bytes([data[x_axis_start + 2], data[x_axis_start + 3]]) as usize;
            if !(8..=12).contains(&x_len) { continue; }

            // X axis data follows: x_len values * 2 bytes
            let map_start = x_axis_start + 4 + (x_len * 2);
            let map_size = y_len * x_len * 2;
            
            log::debug!("🔍 Found potential Boost correction by temp structure at t=0x{:X}, map_start=0x{:X}", t, map_start);
            
            if map_start + map_size > data.len() { 
                log::debug!("   ❌ Skipping: map_start + map_size > data.len()");
                continue; 
            }
            
            // Check if this address is already detected - if so, UPDATE the existing map
            let already_exists = detected_addresses.contains(&(map_start as u32));
            if already_exists {
                log::debug!("   ℹ️ Address already detected, will update existing map");
            }
            
            // Validate: X axis values should be temperature (2000-4500 raw = -70°C to +180°C)
            let mut valid_temp = 0;
            for i in 0..x_len {
                let offset = x_axis_start + 4 + (i * 2);
                if offset + 1 < data.len() {
                    let value = u16::from_le_bytes([data[offset], data[offset + 1]]);
                    // Temperature range: 2000 (-70°C) to 4500 (+180°C)
                    if value >= 2000 && value <= 4500 {
                        valid_temp += 1;
                    }
                }
            }
            
            // Validate: Y axis values should be boost (500-3000 mbar)
            let mut valid_boost = 0;
            for i in 0..y_len {
                let offset = t + 4 + (i * 2);
                if offset + 1 < data.len() {
                    let value = u16::from_le_bytes([data[offset], data[offset + 1]]);
                    // Boost range: 500-3000 mbar
                    if value >= 400 && value <= 3500 {
                        valid_boost += 1;
                    }
                }
            }
            
            // Debug: log validation results
            log::debug!("   Validation: valid_temp={}/{}, valid_boost={}/{}", valid_temp, x_len, valid_boost, y_len);
            
            // If most values are valid, this is likely Boost correction by
            // temperature (seuils proportionnels aux dims variables)
            if valid_temp >= x_len * 7 / 10 && valid_boost >= y_len * 3 / 4 {
                log::debug!("🎯 Found Boost correction by temperature at 0x{:X} (Y axis at 0x{:X}, X axis at 0x{:X})", 
                    map_start, t, x_axis_start);
                
                if already_exists {
                    // Update existing map
                    for existing_map in maps.iter_mut() {
                        if existing_map.address == map_start as u32 {
                            log::debug!("   ✅ Updating existing map from '{}' to 'Boost correction by temperature'", 
                                existing_map.name.as_deref().unwrap_or("unknown"));
                            existing_map.name = Some("Boost correction by temperature".to_string());
                            existing_map.category = Some("Detected maps".to_string());
                            existing_map.subcategory = Some("2-Limiters".to_string());
                            existing_map.description = Some(format!(
                                "Boost correction (mbar) | X: IAT (°C) | Y: Requested boost (mbar)"
                            ));
                            existing_map.unit = Some("mbar".to_string());
                            existing_map.dimensions =
                                MapDimensions::TwoDimensional { rows: y_len, cols: x_len };
                            existing_map.x_axis_address = Some((x_axis_start + 4) as u32);
                            existing_map.y_axis_address = Some((t + 4) as u32);
                            existing_map.correction_factor = Some(1.0);
                            existing_map.x_axis_correction = Some(0.1);
                            existing_map.y_axis_correction = Some(1.0);
                            existing_map.x_axis_offset = Some(-273.0);
                            existing_map.y_axis_offset = Some(0.0);
                            break;
                        }
                    }
                } else {
                    // Create new map
                    let boost_corr = DetectedMap {
                        id: format!("bcbt_{:06X}", map_start),
                        address: map_start as u32,
                        size: map_size,
                        dimensions: MapDimensions::TwoDimensional { rows: y_len, cols: x_len },
                        data_type: DataType::UInt16,
                        name: Some("Boost correction by temperature".to_string()),
                        category: Some("Detected maps".to_string()),
                        subcategory: Some("2-Limiters".to_string()),
                        description: Some(format!(
                            "Boost correction (mbar) | X: IAT (°C) | Y: Requested boost (mbar) | Axis IDs: X=0x{:04X} Y=0x{:04X}",
                            x_axis_start + 4, t + 4
                        )),
                        unit: Some("mbar".to_string()),
                        confidence: 0.95,
                        x_axis_address: Some((x_axis_start + 4) as u32),
                        y_axis_address: Some((t + 4) as u32),
                        correction_factor: Some(1.0),
                        x_axis_correction: Some(0.1),
                        y_axis_correction: Some(1.0),
                        offset: Some(0.0),
                        x_axis_offset: Some(-273.0),
                        y_axis_offset: Some(0.0),
                        x_label: None,
                        y_label: None,
                        y_axis_inverted: None,
                        is_little_endian: None,
                        codeblock_id: None,
                        codeblock_start_address: None,
                        codeblock_end_address: None,
                        map_selector: None,
                    };

                    detected_addresses.insert(map_start as u32);
                    maps.push(boost_corr);
                }
            }
        }
    }

    /// Find "IQ by air intake temp" maps - 3x3 IQ limiter based on intake air temperature
    /// Structure: [0xC1** hdr][03 00][3 temp values][0xEC** hdr][03 00][3 rpm values][3x3 map data]
    /// Verified on Leon ARL 150 EDO: Y axis 0x4F300 (60-70°C), X axis 0x4F30A (3500-4250 rpm),
    /// data 0x4F310 (100.00 mg/stroke) — same structure in codeblock 2 at 0x6F300/0x6F30A/0x6F310
    fn find_iq_by_air_intake_temp(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        log::debug!("🔍 Searching for IQ by air intake temp maps...");

        // Full structure size from Y header to end of map data:
        // 4 (Y hdr) + 6 (Y values) + 4 (X hdr) + 6 (X values) + 18 (3x3 map) = 38 bytes
        for t in (0..data.len().saturating_sub(40)).step_by(2) {
            // Y axis header: ID high byte 0xC1 (temperature), length 3
            if data[t + 1] != 0xC1 { continue; }
            let y_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
            if y_len != 3 { continue; }

            // Y values: 3 strictly ascending IAT raw values
            // (factor 0.1, offset -273 => raw 2731-4731 = 0 to 200°C)
            let y0 = u16::from_le_bytes([data[t + 4], data[t + 5]]);
            let y1 = u16::from_le_bytes([data[t + 6], data[t + 7]]);
            let y2 = u16::from_le_bytes([data[t + 8], data[t + 9]]);
            if !(2731..=4731).contains(&y0) || !(2731..=4731).contains(&y1) || !(2731..=4731).contains(&y2) { continue; }
            if !(y0 < y1 && y1 < y2) { continue; }

            // X axis header immediately after: ID high byte 0xEC (engine speed), length 3
            let x_hdr = t + 10;
            if data[x_hdr + 1] != 0xEC { continue; }
            let x_len = u16::from_le_bytes([data[x_hdr + 2], data[x_hdr + 3]]) as usize;
            if x_len != 3 { continue; }

            // X values: 3 strictly ascending RPM values (500-6500)
            let x0 = u16::from_le_bytes([data[x_hdr + 4], data[x_hdr + 5]]);
            let x1 = u16::from_le_bytes([data[x_hdr + 6], data[x_hdr + 7]]);
            let x2 = u16::from_le_bytes([data[x_hdr + 8], data[x_hdr + 9]]);
            if !(500..=6500).contains(&x0) || !(500..=6500).contains(&x1) || !(500..=6500).contains(&x2) { continue; }
            if !(x0 < x1 && x1 < x2) { continue; }

            // Map data: 3x3 u16 values, plausible IQ (factor 0.01 => raw 0-12000 = 0-120 mg/stroke)
            let map_start = x_hdr + 10;
            let map_size = 18; // 3 x 3 x 2 bytes
            if map_start + map_size > data.len() { continue; }

            let mut max_val: u16 = 0;
            let mut all_plausible = true;
            for i in 0..9 {
                let v = u16::from_le_bytes([data[map_start + i * 2], data[map_start + i * 2 + 1]]);
                if v > 12000 { all_plausible = false; break; }
                if v > max_val { max_val = v; }
            }
            if !all_plausible || max_val < 500 { continue; }

            let already_exists = detected_addresses.contains(&(map_start as u32));

            log::debug!("🎯 Found IQ by air intake temp at 0x{:X} (Y axis at 0x{:X}, X axis at 0x{:X}, already_exists={})",
                map_start, t + 4, x_hdr + 4, already_exists);

            let mut updated = false;
            if already_exists {
                // Update the existing (generic) map instead of duplicating
                for existing_map in maps.iter_mut() {
                    if existing_map.address == map_start as u32 {
                        existing_map.name = Some("IQ by air intake temp".to_string());
                        existing_map.category = Some("Detected maps".to_string());
                        existing_map.subcategory = Some("2-Limiters".to_string());
                        existing_map.description = Some(
                            "IQ limit (mg/stroke) | X: Engine speed (rpm) | Y: IAT (°C)".to_string()
                        );
                        existing_map.unit = Some("mg/stroke".to_string());
                        existing_map.dimensions = MapDimensions::TwoDimensional { rows: 3, cols: 3 };
                        existing_map.x_axis_address = Some((x_hdr + 4) as u32);
                        existing_map.y_axis_address = Some((t + 4) as u32);
                        existing_map.correction_factor = Some(0.01);
                        existing_map.x_axis_correction = Some(1.0);
                        existing_map.y_axis_correction = Some(0.1);
                        existing_map.x_axis_offset = Some(0.0);
                        existing_map.y_axis_offset = Some(-273.0);
                        existing_map.x_label = Some("rpm".to_string());
                        existing_map.y_label = Some("degC".to_string());
                        updated = true;
                        break;
                    }
                }
                if !updated {
                    log::debug!("   ℹ️ Address 0x{:X} was in detected_addresses but no map matches — creating new map", map_start);
                }
            }
            if !updated {
                let iq_iat = DetectedMap {
                    id: format!("iqiat_{:06X}", map_start),
                    address: map_start as u32,
                    size: map_size,
                    dimensions: MapDimensions::TwoDimensional { rows: 3, cols: 3 },
                    data_type: DataType::UInt16,
                    name: Some("IQ by air intake temp".to_string()),
                    category: Some("Detected maps".to_string()),
                    subcategory: Some("2-Limiters".to_string()),
                    description: Some(
                        "IQ limit (mg/stroke) | X: Engine speed (rpm) | Y: IAT (°C)".to_string()
                    ),
                    unit: Some("mg/stroke".to_string()),
                    confidence: 0.95,
                    x_axis_address: Some((x_hdr + 4) as u32),
                    y_axis_address: Some((t + 4) as u32),
                    correction_factor: Some(0.01),
                    x_axis_correction: Some(1.0),
                    y_axis_correction: Some(0.1),
                    offset: Some(0.0),
                    x_axis_offset: Some(0.0),
                    y_axis_offset: Some(-273.0),
                    x_label: Some("rpm".to_string()),
                    y_label: Some("degC".to_string()),
                    y_axis_inverted: None,
                    is_little_endian: None,
                    codeblock_id: None,
                    codeblock_start_address: None,
                    codeblock_end_address: None,
                    map_selector: None,
                };

                detected_addresses.insert(map_start as u32);
                maps.push(iq_iat);
            }
        }
    }

    /// Find Driver wish maps - 256 bytes with EC/C0 axis IDs
    /// These maps determine requested injection quantity based on throttle position
    /// Searches for EC(TPS)+C0(RPM) pattern only (the generic check_map handles C0+EC order)
    fn find_driver_wish_maps(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        log::debug!("🔍 [DRIVER WISH] Starting search in {} byte file", data.len());
        let mut found_count = 0;

        // Known Driver wish dimension pairs: (TPS_len, RPM_len) -> map_size
        let valid_pairs: &[(usize, usize)] = &[(16, 8), (8, 16), (10, 12), (12, 10), (9, 12), (12, 9), (8, 12), (12, 8)];

        for t in (0..data.len().saturating_sub(300)).step_by(2) {
            if t + 4 > data.len() { continue; }

            let first_id = u16::from_le_bytes([data[t], data[t + 1]]);
            let first_id_high = (first_id >> 8) as u8;

            // First axis must be EC (TPS)
            if first_id_high != 0xEC { continue; }

            let first_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
            if first_len == 0 || first_len > 20 { continue; }

            // Second axis
            let second_start = t + 4 + (first_len * 2);
            if second_start + 4 > data.len() { continue; }

            let second_id = u16::from_le_bytes([data[second_start], data[second_start + 1]]);
            let second_id_high = (second_id >> 8) as u8;

            // Second axis must be C0 (RPM)
            if second_id_high != 0xC0 { continue; }

            let second_len = u16::from_le_bytes([data[second_start + 2], data[second_start + 3]]) as usize;
            if second_len == 0 || second_len > 20 { continue; }

            // Check if this is a known Driver wish dimension pair
            if !valid_pairs.contains(&(first_len, second_len)) { continue; }

            let map_start = second_start + 4 + (second_len * 2);
            let map_size = first_len * second_len * 2;
            if map_start + map_size > data.len() { continue; }
            if detected_addresses.contains(&(map_start as u32)) { continue; }

            // Validate map data: values should be in Driver wish range
            let mut valid_values = 0;
            let mut total_values = 0;
            for i in (0..map_size).step_by(2) {
                let value = u16::from_le_bytes([data[map_start + i], data[map_start + i + 1]]);
                total_values += 1;
                if value <= 10000 { valid_values += 1; }
            }

            if valid_values <= total_values * 3 / 4 { continue; }

            // Driver wish: always display with more values as columns, fewer as rows
            let larger_len = first_len.max(second_len);
            let smaller_len = first_len.min(second_len);

            log::debug!("🎯 Found Driver wish map at 0x{:X} ({}x{} display)",
                map_start, larger_len, smaller_len);

            let driver_wish = DetectedMap {
                id: format!("dw_{:06X}", map_start),
                address: map_start as u32,
                size: map_size,
                dimensions: MapDimensions::TwoDimensional { rows: larger_len, cols: smaller_len },
                // WinOLS: bSigned=1 — IQ négatifs (frein moteur) en 0xFFxx
                data_type: DataType::Int16,
                name: Some("Driver wish".to_string()),
                category: Some("Engine fuel request".to_string()),
                subcategory: None,
                description: Some(format!(
                    "Requested IQ (mg/st) | X: Throttle position (%) | Y: Engine speed (rpm)"
                )),
                unit: Some("mg/st".to_string()),
                confidence: 0.9,
                x_axis_address: Some((second_start + 4) as u32),
                y_axis_address: Some((t + 4) as u32),
                correction_factor: Some(0.01),
                x_axis_correction: Some(0.01),
                y_axis_correction: Some(1.0),
                offset: Some(0.0),
                x_axis_offset: Some(0.0),
                y_axis_offset: Some(0.0),
                x_label: None,
                y_label: None,
                y_axis_inverted: None,
                is_little_endian: None,
                codeblock_id: None,
                codeblock_start_address: None,
                codeblock_end_address: None,
                map_selector: None,
            };

            detected_addresses.insert(map_start as u32);
            maps.push(driver_wish);
            found_count += 1;
        }

        log::debug!("🔍 [DRIVER WISH] Search complete: {} Driver Wish maps detected", found_count);
    }

    /// Find Idle RPM maps - typically small 1D maps
    fn find_idle_rpm_maps(&self, data: &[u8], maps: &mut Vec<DetectedMap>, _detected_addresses: &mut HashSet<u32>) {
        // Idle RPM values are typically around 800-1200 RPM
        // Look for small maps with these values
        for map in maps.iter_mut() {
            if map.size == 4 && map.subcategory.is_none() {
                let map_start = map.address as usize;
                if map_start + 4 <= data.len() {
                    let value1 = u16::from_le_bytes([data[map_start], data[map_start + 1]]);
                    let value2 = u16::from_le_bytes([data[map_start + 2], data[map_start + 3]]);
                    
                    // Check if values look like idle RPM (700-1400 RPM range)
                    if value1 >= 700 && value1 <= 1400 && value2 >= 700 && value2 <= 1400 {
                        map.name = Some("Idle RPM".to_string());
                        map.subcategory = Some("4-Misc".to_string());
                        map.description = Some("Target idle engine speed".to_string());
                        map.unit = Some("rpm".to_string());
                    }
                }
            }
        }
    }

    /// Find Smoke Limiter maps - can have multiple maps with temperature selector
    /// Structure: [Y_axis F9 16vals][X_axis DA 13vals][Z_axis temp selector][Maps 416 bytes each]
    /// EDCSuite: if MapSelector.MapIndexes[i] > 0, the map at that index is active
    fn find_smoke_limiter_maps(&self, data: &[u8], maps: &mut Vec<DetectedMap>, detected_addresses: &mut HashSet<u32>) {
        use std::io::Write;
        let mut log_file = std::fs::OpenOptions::new()
            .create(true).append(true)
            .open(r"C:\temp\smoke_detection.log").ok();

        if let Some(ref mut f) = log_file {
            let _ = writeln!(f, "\n=== find_smoke_limiter_maps called ===");
            let _ = writeln!(f, "data len={}", data.len());
        }

        // Early exit if file too small
        if data.len() < 0x4C000 + 500 {
            if let Some(ref mut f) = log_file {
                let _ = writeln!(f, "File too small, skipping");
            }
            return;
        }

        // Look for smoke limiter structure: Y axis 0xF9, X axis 0xDA, optional Z axis (temperature selector)
        let mut t = 0x4C000; // Start from typical codeblock area
        let mut found_count = 0;
        let mut y_f9_count = 0;
        let end_pos = data.len().saturating_sub(500);

        if let Some(ref mut f) = log_file {
            let _ = writeln!(f, "Scanning from 0x{:X} to 0x{:X}", t, end_pos);
        }

        while t < end_pos {
            // Look for Y axis: ID high byte 0xF9, length 16
            let y_id = u16::from_le_bytes([data[t], data[t + 1]]);
            let y_id_high = (y_id >> 8) as u8;

            if y_id_high == 0xF9 {
                y_f9_count += 1;
                let y_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;

                if y_len == 16 {
                    // Calculate X axis position
                    let x_offset = t + 4 + (y_len * 2);

                    if x_offset + 4 < data.len() {
                        let x_id = u16::from_le_bytes([data[x_offset], data[x_offset + 1]]);
                        let x_id_high = (x_id >> 8) as u8;
                        let x_len = u16::from_le_bytes([data[x_offset + 2], data[x_offset + 3]]) as usize;

                        // Check for X axis: ID high byte 0xDA, length 13
                        if x_id_high == 0xDA && x_len == 13 {
                            // Found smoke limiter axes! Now check for Z axis (temperature selector)
                            let z_offset = x_offset + 4 + (x_len * 2);

                            found_count += 1;
                            if let Some(ref mut f) = log_file {
                                let _ = writeln!(f, "Found Y+X #{} at 0x{:X}: Y_ID=0x{:04X}({}), X_ID=0x{:04X}({}), Z at 0x{:X}",
                                    found_count, t, y_id, y_len, x_id, x_len, z_offset);
                            }

                            let map_size = 416; // 13 * 16 * 2 = 416 bytes

                            if z_offset + 4 < data.len() {
                                let z_id = u16::from_le_bytes([data[z_offset], data[z_offset + 1]]);
                                let z_id_high = (z_id >> 8) as u8;

                                log::debug!("🔍 [SMOKE]    Z axis check: ID=0x{:04X}, high=0x{:02X}, need 0xC1", z_id, z_id_high);

                                // Check if there's a Z axis with temperature selector (ID 0xC1)
                                // This is the key to multi-smoke limiter detection
                                if z_id_high == 0xC1 {
                                    let z_len = u16::from_le_bytes([data[z_offset + 2], data[z_offset + 3]]) as usize;

                                    log::debug!("🔍 [SMOKE] ✅ Z axis FOUND at 0x{:X}: ID=0x{:04X}, len={}", z_offset, z_id, z_len);

                                    if z_len > 0 && z_len <= 10 {
                                        // Found temperature selector!
                                        // Read temperature values and map indexes
                                        let temp_data_offset = z_offset + 4;
                                        let selector_offset = temp_data_offset + (z_len * 2);
                                        let first_map_offset = selector_offset + (z_len * 2);

                                        if first_map_offset + (map_size * z_len) <= data.len() {
                                            // Read temperature values
                                            let mut temperatures: Vec<f64> = Vec::new();
                                            for i in 0..z_len {
                                                let temp_raw = u16::from_le_bytes([
                                                    data[temp_data_offset + i * 2],
                                                    data[temp_data_offset + i * 2 + 1]
                                                ]);
                                                let temp_celsius = (temp_raw as f64 * 0.1) - 273.1;
                                                temperatures.push(temp_celsius.round());
                                            }

                                            // Read selector indexes
                                            let mut indexes: Vec<u16> = Vec::new();
                                            for i in 0..z_len {
                                                let idx = data[selector_offset + i * 2] as u16
                                                    + ((data[selector_offset + i * 2 + 1] as u16) << 8);
                                                indexes.push(idx);
                                            }

                                                        if let Some(ref mut f) = log_file {
                                                let _ = writeln!(f, "Multi-smoke at 0x{:X}: {} maps, temps={:?}, indexes={:?}, first_map=0x{:X}",
                                                    t, z_len, temperatures, indexes, first_map_offset);
                                            }

                                            // Keep all non-empty maps (even if they have identical data)
                                            // Each map corresponds to a temperature threshold (-20°C, 40°C, 80°C etc.)
                                            // and a tuner may modify them independently
                                            let mut active_indices: Vec<usize> = Vec::new();

                                            for i in 0..z_len {
                                                let map_addr = first_map_offset + (i * map_size);
                                                if map_addr + map_size <= data.len() {
                                                    let mut non_zero_count = 0;
                                                    for j in 0..map_size {
                                                        if data[map_addr + j] != 0 {
                                                            non_zero_count += 1;
                                                        }
                                                    }
                                                    // Skip only truly empty maps (< 5% non-zero bytes)
                                                    if non_zero_count >= map_size / 20 {
                                                        active_indices.push(i);
                                                    }
                                                }
                                            }

                                            // Always keep at least the first map (there's always at least 1 active smoke limiter per codeblock)
                                            if active_indices.is_empty() && z_len > 0 {
                                                active_indices.push(0);
                                            }

                                            let active_count = active_indices.len();

                                            if let Some(ref mut f) = log_file {
                                                let _ = writeln!(f, "Active non-empty maps: {} out of {} (active: {:?})",
                                                    active_count, z_len, active_indices);
                                            }

                                            // Create a map for each UNIQUE temperature slot only
                                            for &i in &active_indices {
                                                    let map_addr = first_map_offset + (i * map_size);

                                                    if !detected_addresses.contains(&(map_addr as u32)) {
                                                        let temp = temperatures[i] as i32;
                                                        // Only show temperature in name if multiple UNIQUE maps exist
                                                        let name = if active_count > 1 {
                                                            format!("Smoke limiter {} °C", temp)
                                                        } else {
                                                            "Smoke limiter".to_string()
                                                        };

                                                        let smoke_map = DetectedMap {
                                                            id: format!("smoke_{:06X}", map_addr),
                                                            address: map_addr as u32,
                                                            size: map_size,
                                                            name: Some(name),
                                                            category: Some("Detected maps".to_string()),
                                                            subcategory: Some("2-Limiters".to_string()),
                                                            description: Some("Maximum IQ (mg) | X: Airflow (mg/st) | Y: Engine speed (rpm)".to_string()),
                                                            unit: Some("mg".to_string()),
                                                            dimensions: MapDimensions::TwoDimensional { rows: 16, cols: 13 },
                                                            data_type: DataType::UInt16,
                                                            x_axis_address: Some((x_offset + 4) as u32),
                                                            y_axis_address: Some((t + 4) as u32),
                                                            x_axis_correction: Some(0.1),
                                                            y_axis_correction: Some(1.0),
                                                            x_axis_offset: None,
                                                            y_axis_offset: None,
                                                            correction_factor: Some(0.01),
                                                            offset: None,
                                                            x_label: Some("mg/st".to_string()),
                                                            y_label: Some("rpm".to_string()),
                                                            y_axis_inverted: None,
                                                            is_little_endian: None,
                                                            confidence: 0.95,
                                                            codeblock_id: if map_addr >= 0x4C000 && map_addr < 0x5C000 { Some(2) } else if map_addr >= 0x5C000 && map_addr < 0x6C000 { Some(3) } else if map_addr >= 0x6C000 && map_addr < 0x7C000 { Some(5) } else { None },
                                                            codeblock_start_address: None,
                                                            codeblock_end_address: None,
                                                            map_selector: None,
                                                        };

                                                        maps.push(smoke_map);
                                                        detected_addresses.insert(map_addr as u32);
                                                        if let Some(ref mut f) = log_file {
                                                            let _ = writeln!(f, "✅ Added smoke limiter {} at 0x{:X} (temp={}°C)", i + 1, map_addr, temp);
                                                        }
                                                    }
                                            }

                                            // Skip past this entire structure
                                            t = first_map_offset + (map_size * z_len);
                                            continue;
                                        }
                                    }
                                }

                                // No Z axis - single smoke limiter
                                let map_addr = z_offset;
                                if !detected_addresses.contains(&(map_addr as u32)) && map_addr + map_size <= data.len() {
                                    let smoke_map = DetectedMap {
                                        id: format!("smoke_{:06X}", map_addr),
                                        address: map_addr as u32,
                                        size: map_size,
                                        name: Some("Smoke limiter".to_string()),
                                        category: Some("Detected maps".to_string()),
                                        subcategory: Some("2-Limiters".to_string()),
                                        description: Some("Maximum IQ (mg) | X: Airflow (mg/st) | Y: Engine speed (rpm)".to_string()),
                                        unit: Some("mg".to_string()),
                                        dimensions: MapDimensions::TwoDimensional { rows: 16, cols: 13 },
                                        data_type: DataType::UInt16,
                                        x_axis_address: Some((x_offset + 4) as u32),
                                        y_axis_address: Some((t + 4) as u32),
                                        x_axis_correction: Some(0.1),
                                        y_axis_correction: Some(1.0),
                                        x_axis_offset: None,
                                        y_axis_offset: None,
                                        correction_factor: Some(0.01),
                                        offset: None,
                                        x_label: Some("mg/st".to_string()),
                                        y_label: Some("rpm".to_string()),
                                        y_axis_inverted: None,
                                        is_little_endian: None,
                                        confidence: 0.95,
                                        codeblock_id: if map_addr >= 0x4C000 && map_addr < 0x5C000 { Some(2) } else if map_addr >= 0x5C000 && map_addr < 0x6C000 { Some(3) } else if map_addr >= 0x6C000 && map_addr < 0x7C000 { Some(5) } else { None },
                                        codeblock_start_address: None,
                                        codeblock_end_address: None,
                                        map_selector: None,
                                    };

                                    maps.push(smoke_map);
                                    detected_addresses.insert(map_addr as u32);
                                    log::debug!("✅ Added single smoke limiter at 0x{:X}", map_addr);
                                }
                            }
                        }
                    }
                }
            }
            t += 2;
        }
        if let Some(ref mut f) = log_file {
            let _ = writeln!(f, "=== Scan complete ===");
            let _ = writeln!(f, "Y_F9 occurrences: {}", y_f9_count);
            let _ = writeln!(f, "Smoke structures: {}", found_count);
            let _ = writeln!(f, "Maps in vector: {}", maps.len());
        }
    }

    /// Helper function to find a byte sequence with mask
    fn find_sequence_with_mask(&self, data: &[u8], offset: usize, pattern: &[u8], mask: &[u8]) -> Option<usize> {
        if pattern.len() != mask.len() {
            return None;
        }
        
        for i in offset..data.len().saturating_sub(pattern.len()) {
            let mut matches = true;
            for (j, (&p, &m)) in pattern.iter().zip(mask.iter()).enumerate() {
                if m == 1 && data[i + j] != p {
                    matches = false;
                    break;
                }
            }
            if matches {
                return Some(i);
            }
        }
        None
    }

    /// Helper function to find an exact byte sequence
    fn find_sequence_exact(&self, data: &[u8], offset: usize, pattern: &[u8]) -> Option<usize> {
        for i in offset..data.len().saturating_sub(pattern.len()) {
            if data[i..i + pattern.len()] == pattern[..] {
                return Some(i);
            }
        }
        None
    }

    /// Check if a value is a valid axis ID (C#: isAxisID())
    /// EXACTLY like VAGEDCSuite - no extended ranges
    /// Also accepts 0x0000 for empty axes (used in 1x1, 1x2, etc. maps)
    fn is_axis_id(&self, axis_id: u16) -> bool {
        // Accept 0x0000 for empty axes (used in single-value maps and linearization maps)
        if axis_id == 0x0000 {
            return true;
        }
        
        let idstrip = (axis_id / 256) as u8;
        // EXACTLY like VAGEDCSuite EDC15PFileParser.cs line 6226-6237
        if idstrip == 0xDB { return true; }
        if matches!(idstrip, 0xC0 | 0xC1 | 0xC2 | 0xC4 | 0xC5) { return true; }
        if matches!(idstrip, 0xE0 | 0xE4 | 0xE5 | 0xE9 | 0xEA | 0xEB | 0xEC) { return true; }
        if matches!(idstrip, 0xDA | 0xDD | 0xDE) { return true; }
        if matches!(idstrip, 0xF9 | 0xFE) { return true; }
        if idstrip == 0xE8 { return true; }
        false
    }

    /// Check if axis length is valid (C#: isValidLength())
    /// EXACTLY like VAGEDCSuite EDC15PFileParser.cs line 6209-6223
    fn is_valid_length(&self, length: usize, axis_id: u16) -> bool {
        let idstrip = (axis_id / 256) as u8;
        // EXACTLY like VAGEDCSuite: if ((idstrip & 0xF0) == 0xE0) then length > 0 && length <= 32
        // else length > 0 && length < 32
        if (idstrip & 0xF0) == 0xE0 {
            length > 0 && length <= 32
        } else {
            length > 0 && length < 32
        }
    }

    /// Find a pattern that matches the detected map
    /// Very lenient matching - tries multiple strategies to find a match
    fn find_matching_pattern(
        &self,
        map_length: usize,
        x_axis_id: u16,
        y_axis_id: u16,
        x_axis_len: usize,
        y_axis_len: usize,
    ) -> Option<&EDC15PMapPattern> {
        let x_id_high = (x_axis_id >> 8) as u8;
        let y_id_high = (y_axis_id >> 8) as u8;

        // Debug: log search parameters for size 280
        if map_length == 280 {
            log::debug!("🔎 find_matching_pattern: len={}, x_id_high=0x{:02X}, y_id_high=0x{:02X}, x_len={}, y_len={}",
                map_length, x_id_high, y_id_high, x_axis_len, y_axis_len);
            // List all patterns with length 280
            for p in &self.patterns {
                if p.length == 280 {
                    log::debug!("   Pattern '{}': x_len={}, y_len={}, x_id=0x{:02X}, y_id=0x{:02X}",
                        p.name, p.x_axis_length, p.y_axis_length, p.x_axis_id_high, p.y_axis_id_high);
                }
            }
        }

        // Strategy 1: Exact match with SPECIFIC axis IDs (no wildcards)
        // Only match patterns that have non-zero axis IDs that match exactly
        for pattern in &self.patterns {
            if pattern.length == map_length &&
               pattern.x_axis_length == x_axis_len &&
               pattern.y_axis_length == y_axis_len &&
               pattern.x_axis_id_high != 0x00 &&
               pattern.y_axis_id_high != 0x00 &&
               pattern.x_axis_id_high == x_id_high &&
               pattern.y_axis_id_high == y_id_high {
                return Some(pattern);
            }
        }
        
        // Strategy 2: Exact match with swapped axes and SPECIFIC axis IDs
        for pattern in &self.patterns {
            if pattern.length == map_length &&
               pattern.x_axis_length == y_axis_len && 
               pattern.y_axis_length == x_axis_len &&
               pattern.x_axis_id_high != 0x00 && 
               pattern.y_axis_id_high != 0x00 &&
               pattern.x_axis_id_high == y_id_high &&
               pattern.y_axis_id_high == x_id_high {
                return Some(pattern);
            }
        }
        
        // Strategy 3: Match with wildcard axis IDs (pattern has 0x00)
        for pattern in &self.patterns {
            if pattern.length == map_length &&
               pattern.x_axis_length == x_axis_len && 
               pattern.y_axis_length == y_axis_len {
                if (pattern.x_axis_id_high == 0x00 || pattern.x_axis_id_high == x_id_high) &&
                   (pattern.y_axis_id_high == 0x00 || pattern.y_axis_id_high == y_id_high) {
                    return Some(pattern);
                }
            }
        }
        
        // Strategy 4: Match with swapped axes and wildcard axis IDs
        for pattern in &self.patterns {
            if pattern.length == map_length &&
               pattern.x_axis_length == y_axis_len && 
               pattern.y_axis_length == x_axis_len {
                if (pattern.x_axis_id_high == 0x00 || pattern.x_axis_id_high == y_id_high) &&
                   (pattern.y_axis_id_high == 0x00 || pattern.y_axis_id_high == x_id_high) {
                    return Some(pattern);
                }
            }
        }
        
        None
    }
    
    /// Create a generic map when no pattern matches but structure is valid
    fn create_generic_map(
        &self,
        map_address: u32,
        x_axis_id: u16,
        y_axis_id: u16,
        x_axis_len: usize,
        y_axis_len: usize,
        structure_start: u32,
    ) -> DetectedMap {
        let dimensions = MapDimensions::TwoDimensional {
            rows: y_axis_len,
            cols: x_axis_len,
        };

        let mut map = DetectedMap::new(
            map_address,
            x_axis_len * y_axis_len * 2,
            dimensions,
            DataType::UInt16,
        );

        // EXACTLY like VAGEDCSuite: X_axis_address = t + 4, Y_axis_address = t + 8 + (xaxislen * 2)
        // Structure: [X_ID(2)][X_len(2)][X_data(x_axis_len*2)][Y_ID(2)][Y_len(2)][Y_data(y_axis_len*2)][Map_data]
        // X_data starts at: t + 4
        // Y_data starts at: t + 8 + (x_axis_len * 2)
        let x_axis_address = structure_start + 4;
        let y_axis_address = structure_start + 8 + (x_axis_len as u32 * 2);
        
        // EXACTLY like VAGEDCSuite: "3D Map Size: ... Loc: ... IDs: ..."
        map.name = Some(format!(
            "3D Map Size: {}x{} Loc: {:06X} IDs: X {:04X} Y {:04X} Xr {:02X} Yr {:02X} Len: {}",
            x_axis_len, y_axis_len, map_address, x_axis_id, y_axis_id,
            (x_axis_id / 256) as u8, (y_axis_id / 256) as u8, map.size
        ));
        map.description = Some(format!(
            "Detected map | X: 0x{:04X} (len={}) | Y: 0x{:04X} (len={})",
            x_axis_id, x_axis_len, y_axis_id, y_axis_len
        ));
        map.confidence = 0.5; // Lower confidence for generic maps
        map.x_axis_address = Some(x_axis_address);
        map.y_axis_address = Some(y_axis_address);
        map.x_axis_correction = Some(1.0);
        map.y_axis_correction = Some(1.0);
        map.correction_factor = Some(1.0);
        map.offset = Some(0.0);

        map
    }

    /// Create a DetectedMap with all metadata including axis addresses
    fn create_map_from_pattern(
        &self,
        pattern: &EDC15PMapPattern,
        map_address: u32,
        x_axis_id: u16,
        y_axis_id: u16,
        x_axis_len: usize,
        y_axis_len: usize,
        structure_start: u32,
    ) -> DetectedMap {
        // Calculate axis addresses based on structure
        // CRITICAL: In EDC15P, the detection algorithm finds the FIRST axis ID it encounters
        // The structure is ALWAYS: [First_ID][First_len][First_data][Second_ID][Second_len][Second_data][Map_data]
        // But which axis comes first depends on the file!
        //
        // structure_start = t (where we found the first axis ID)
        // map_address = t + 8 + (x_axis_len * 2) + (y_axis_len * 2) (calculated in check_map)
        //
        // We need to determine which axis was detected first by comparing IDs with the pattern
        let x_axis_id_high = (x_axis_id >> 8) as u8;
        let y_axis_id_high = (y_axis_id >> 8) as u8;
        let pattern_x_id_high = pattern.x_axis_id_high;
        let pattern_y_id_high = pattern.y_axis_id_high;
        
        // CRITICAL FIX: Determine which axis is which by comparing IDs with pattern
        // In check_map, we read the FIRST axis ID as "x_axis_id" and SECOND as "y_axis_id"
        // But these are actually "first_axis_id" and "second_axis_id" - we need to match them to pattern's X and Y
        // 
        // Pattern defines:
        // - pattern.x_axis_id_high: high byte of X axis ID (e.g., 0xEC for temperature)
        // - pattern.y_axis_id_high: high byte of Y axis ID (e.g., 0xC1 for RPM)
        //
        // We need to check if the first detected axis (x_axis_id) matches pattern's X or Y
        let first_axis_matches_x = pattern_x_id_high != 0x00 && x_axis_id_high == pattern_x_id_high;
        let first_axis_matches_y = pattern_y_id_high != 0x00 && x_axis_id_high == pattern_y_id_high;
        let second_axis_matches_x = pattern_x_id_high != 0x00 && y_axis_id_high == pattern_x_id_high;
        let second_axis_matches_y = pattern_y_id_high != 0x00 && y_axis_id_high == pattern_y_id_high;
        
        // Determine which axis is X and which is Y based on pattern matching
        let (_actual_x_axis_id, _actual_y_axis_id, actual_x_axis_len, actual_y_axis_len, axes_were_swapped) = if first_axis_matches_x && second_axis_matches_y && !first_axis_matches_y && !second_axis_matches_x {
            // First axis is X, second is Y (unambiguous)
            (x_axis_id, y_axis_id, x_axis_len, y_axis_len, false)
        } else if first_axis_matches_y && second_axis_matches_x && !first_axis_matches_x && !second_axis_matches_y {
            // First axis is Y, second is X - SWAP THEM! (unambiguous)
            (y_axis_id, x_axis_id, y_axis_len, x_axis_len, true)
        } else if first_axis_matches_x && first_axis_matches_y && second_axis_matches_x && second_axis_matches_y {
            // AMBIGUOUS: Both axes have same ID (e.g., both 0xDA for Boost correction by temperature)
            // Use dimensions to determine swap
            let dimensions_match_direct = pattern.x_axis_length == x_axis_len && pattern.y_axis_length == y_axis_len;
            let dimensions_match_swapped = pattern.x_axis_length == y_axis_len && pattern.y_axis_length == x_axis_len;
            
            if dimensions_match_direct {
                log::debug!("🔄 Ambiguous IDs for '{}', dimensions match direct order", pattern.name);
                (x_axis_id, y_axis_id, x_axis_len, y_axis_len, false)
            } else if dimensions_match_swapped {
                log::debug!("🔄 Ambiguous IDs for '{}', dimensions match SWAPPED order", pattern.name);
            (y_axis_id, x_axis_id, y_axis_len, x_axis_len, true)
            } else {
                log::debug!("⚠️  Ambiguous IDs for '{}' and dimensions don't match, using default", pattern.name);
                (x_axis_id, y_axis_id, x_axis_len, y_axis_len, false)
            }
        } else {
            // Fallback: assume first is X, second is Y (original behavior)
            log::debug!("⚠️  Could not match axis IDs to pattern for '{}', using default order", pattern.name);
            (x_axis_id, y_axis_id, x_axis_len, y_axis_len, false)
        };
        
        // Determine final display dimensions using the semantic axis lengths
        // actual_x_axis_len / actual_y_axis_len already account for swaps detected above
        let mut final_rows = actual_y_axis_len;
        let mut final_cols = actual_x_axis_len;
        
        // Boost target maps: Display as 14 rows (RPM) x 10 cols (IQ) like Symbol
        log::debug!("🔍 create_map_from_pattern: pattern='{}', length={}, actual_x={}, actual_y={}, final_rows={}, final_cols={}",
            pattern.name, pattern.length, actual_x_axis_len, actual_y_axis_len, final_rows, final_cols);
        if pattern.name.contains("Boost target") {
            let expected_rows = if pattern.length == 280 { 14 } else { 16 }; // RPM rows
            let expected_cols = 10; // IQ cols
            log::debug!("🎯 Boost target detected! expected_rows={}, expected_cols={}", expected_rows, expected_cols);

            if final_rows != expected_rows || final_cols != expected_cols {
                log::debug!(
                    "⚠️  Adjusting Boost target dimensions from {}x{} to {}x{} (map=0x{:X})",
                    final_rows, final_cols, expected_rows, expected_cols, map_address
                );
                final_rows = expected_rows;
                final_cols = expected_cols;
                // NOTE: Don't swap actual_x_axis_len/actual_y_axis_len - they represent file structure
                // The axis addresses are based on file structure, not display dimensions
            }
            log::debug!("🎯 After adjustment: final_rows={}, final_cols={}, actual_x={}, actual_y={}",
                final_rows, final_cols, actual_x_axis_len, actual_y_axis_len);
        }
        
        let dimensions = MapDimensions::TwoDimensional {
            rows: final_rows,    // Y axis = rows = vertical
            cols: final_cols,    // X axis = columns = horizontal
        };

        // CRITICAL FIX: Smoke limiter has 16-byte gap before map data
        // Detection calculates 0x4DCBE but real address is 0x4DCCE (+16 bytes)
        let corrected_map_address = if pattern.name == "Smoke limiter" {
            log::debug!("🔧 Smoke limiter address correction: 0x{:X} -> 0x{:X}", 
                map_address, map_address + 16);
            map_address + 16
        } else {
            map_address
        };

        let mut map = DetectedMap::new(
            corrected_map_address,
            x_axis_len * y_axis_len * 2,
            dimensions,
            DataType::UInt16,
        );

        // Now calculate addresses based on which axis comes first in the file structure
        // CRITICAL: first_axis_is_x refers to the ORIGINAL order in the file (before swap)
        // If axes_were_swapped, then first_axis_is_x = false (because first was Y)
        // If axes_were_NOT_swapped, then first_axis_is_x = true (because first was X)
        let first_axis_is_x = !axes_were_swapped;
        
        let (x_axis_address, y_axis_address) = if first_axis_is_x {
            // X detected first: [X_ID][X_len][X_data][Y_ID][Y_len][Y_data][Map_data]
            (structure_start + 4, structure_start + 4 + (actual_x_axis_len as u32 * 2) + 4)
        } else {
            // Y detected first: [Y_ID][Y_len][Y_data][X_ID][X_len][X_data][Map_data]
            (structure_start + 4 + (actual_y_axis_len as u32 * 2) + 4, structure_start + 4)
        };
        
        // CRITICAL DEBUG: Verify calculated addresses match expected from JSON
        // For "Start IQ (1)": map=0x4D480, X_axis=0x4D46E, Y_axis=0x4D458
        // Expected offsets from map_address:
        // - X_axis offset: 0x4D480 - 0x4D46E = 0x12 (18 bytes before map)
        // - Y_axis offset: 0x4D480 - 0x4D458 = 0x28 (40 bytes before map)
        // 
        // Verification: structure_start should be: map_address - 8 - (x_axis_len * 2) - (y_axis_len * 2)
        // For Start IQ (1): 9x9 map, so: 0x4D480 - 8 - 18 - 18 = 0x4D480 - 44 = 0x4D454
        // Then X_axis = 0x4D454 + 4 = 0x4D458 (if Y detected first) or 0x4D454 + 4 = 0x4D458 (if X detected first)
        // But expected X_axis = 0x4D46E, which is 0x4D458 + 0x16 = 0x4D46E
        // So there's a 22-byte offset we're missing!
        
        // CRITICAL FIX: Calculate axis addresses from map_address
        // Structure: [First_ID][First_len][First_data][Second_ID][Second_len][Second_data][Map_data]
        // map_address = structure_start + 8 + (x_axis_len * 2) + (y_axis_len * 2)
        // So: structure_start = map_address - 8 - (x_axis_len * 2) - (y_axis_len * 2)
        //
        // For "Start IQ (1)" (9x9): map=0x4D480, X_axis=0x4D46E, Y_axis=0x4D458
        // - X_axis offset from map: 0x4D480 - 0x4D46E = 0x12 (18 bytes)
        // - Y_axis offset from map: 0x4D480 - 0x4D458 = 0x28 (40 bytes)
        //
        // If Y detected first: [Y_ID][Y_len][Y_data][X_ID][X_len][X_data][Map_data]
        // - Y_data at: structure_start + 4 = map_address - 8 - (x_axis_len * 2) - (y_axis_len * 2) + 4
        // - Y_data at: map_address - 4 - (x_axis_len * 2) - (y_axis_len * 2)
        // - For 9x9: Y_data = map_address - 4 - 18 - 18 = map_address - 40 = 0x4D480 - 40 = 0x4D458 ✓
        // - X_data at: structure_start + 4 + (y_axis_len * 2) + 4 = map_address - 8 - (x_axis_len * 2) - (y_axis_len * 2) + 4 + (y_axis_len * 2) + 4
        // - X_data at: map_address - (x_axis_len * 2) = map_address - 18 = 0x4D480 - 18 = 0x4D46E ✓
        //
        // If X detected first: [X_ID][X_len][X_data][Y_ID][Y_len][Y_data][Map_data]
        // - X_data at: structure_start + 4 = map_address - 8 - (x_axis_len * 2) - (y_axis_len * 2) + 4
        // - X_data at: map_address - 4 - (x_axis_len * 2) - (y_axis_len * 2)
        // - For 9x9: X_data = map_address - 4 - 18 - 18 = map_address - 40 = 0x4D480 - 40 = 0x4D458 (WRONG! Should be 0x4D46E)
        // - Y_data at: structure_start + 4 + (x_axis_len * 2) + 4 = map_address - 8 - (x_axis_len * 2) - (y_axis_len * 2) + 4 + (x_axis_len * 2) + 4
        // - Y_data at: map_address - (y_axis_len * 2) = map_address - 18 = 0x4D480 - 18 = 0x4D46E (WRONG! Should be 0x4D458)
        //
        // So the pattern must be: Y detected first (Y axis ID comes before X axis ID in the file)
        // This matches the JSON where Y_axis (RPM) is at 0x4D458 and X_axis (temp) is at 0x4D46E
        
        let (expected_x_offset_from_map, expected_y_offset_from_map) = if first_axis_is_x {
            // X detected first: [X_ID][X_len][X_data][Y_ID][Y_len][Y_data][Map_data]
            // X_data at: map_address - 4 - (actual_x_axis_len * 2) - (actual_y_axis_len * 2)
            // Y_data at: map_address - (actual_y_axis_len * 2)
            (map_address - 4 - (actual_x_axis_len as u32 * 2) - (actual_y_axis_len as u32 * 2), map_address - (actual_y_axis_len as u32 * 2))
        } else {
            // Y detected first: [Y_ID][Y_len][Y_data][X_ID][X_len][X_data][Map_data]
            // Y_data at: map_address - 4 - (actual_x_axis_len * 2) - (actual_y_axis_len * 2)
            // X_data at: map_address - (actual_x_axis_len * 2)
            (map_address - (actual_x_axis_len as u32 * 2), map_address - 4 - (actual_x_axis_len as u32 * 2) - (actual_y_axis_len as u32 * 2))
        };
        
        log::debug!(
            "🔍 Axis Address Calculation Debug for '{}':",
            pattern.name
        );
        log::debug!(
            "   structure_start=0x{:X}, map_address=0x{:X}, first_axis_is_x={}, actual_x_len={}, actual_y_len={}",
            structure_start, map_address, first_axis_is_x, actual_x_axis_len, actual_y_axis_len
        );
        log::debug!(
            "   Axis ID matching: first_axis=0x{:04X} (matches X={}, Y={}), second_axis=0x{:04X} (matches X={}, Y={})",
            x_axis_id, first_axis_matches_x, first_axis_matches_y, y_axis_id, second_axis_matches_x, second_axis_matches_y
        );
        log::debug!(
            "   Calculated from structure_start: X_axis=0x{:X}, Y_axis=0x{:X}",
            x_axis_address, y_axis_address
        );
        log::debug!(
            "   Calculated from map_address: X_axis=0x{:X}, Y_axis=0x{:X}",
            expected_x_offset_from_map, expected_y_offset_from_map
        );
        
        // CRITICAL: Use addresses calculated from map_address (more reliable)
        // IMPORTANT: map_address was calculated using ORIGINAL x_axis_len and y_axis_len (before swap)
        // So we must use ORIGINAL lengths for address calculation, but determine which is which using actual_x/y
        // 
        // map_address = structure_start + 8 + (x_axis_len * 2) + (y_axis_len * 2)
        // where x_axis_len and y_axis_len are the ORIGINAL lengths as read from file
        //
        // If first_axis_is_x (original order): first_axis_len = x_axis_len, second_axis_len = y_axis_len
        // If !first_axis_is_x (Y first): first_axis_len = y_axis_len, second_axis_len = x_axis_len
        //
        // For address calculation from map_address:
        // - If first_axis_is_x: [X_ID][X_len][X_data][Y_ID][Y_len][Y_data][Map_data]
        //   - X_data at: map_address - 4 - (x_axis_len * 2) - (y_axis_len * 2)
        //   - Y_data at: map_address - (y_axis_len * 2)
        // - If !first_axis_is_x: [Y_ID][Y_len][Y_data][X_ID][X_len][X_data][Map_data]
        //   - Y_data at: map_address - 4 - (y_axis_len * 2) - (x_axis_len * 2)
        //   - X_data at: map_address - (x_axis_len * 2)
        //
        // But we need to map these to actual X and Y based on which axis is which semantically
        // actual_x_axis_len and actual_y_axis_len are the CORRECT semantic lengths (after swap if needed)
        // But for address calculation, we need the ORIGINAL file order lengths
        
        // CRITICAL: Calculate addresses from map_address
        // map_address = structure_start + 8 + (first_axis_len * 2) + (second_axis_len * 2)
        // where first_axis_len and second_axis_len are the ORIGINAL lengths as read from file
        //
        // Structure: [First_ID][First_len][First_data][Second_ID][Second_len][Second_data][Map_data]
        //
        // IMPORTANT: After swap detection, we know which axis is X and which is Y semantically,
        // but for address calculation, we need to know which axis comes FIRST in the file structure.
        //
        // The key insight: map_address was calculated using ORIGINAL x_axis_len and y_axis_len (before swap),
        // so we must use ORIGINAL lengths for address calculation. But we need to map the calculated addresses
        // to semantic X and Y based on which axis comes first in the file.
        //
        // If first_axis_is_x (X detected first):
        //   Structure: [X_ID][X_len][X_data][Y_ID][Y_len][Y_data][Map_data]
        //   first_axis_len = x_axis_len (original), second_axis_len = y_axis_len (original)
        //   X_data at: map_address - 4 - (x_axis_len * 2) - (y_axis_len * 2)
        //   Y_data at: map_address - (y_axis_len * 2)
        //
        // If !first_axis_is_x (Y detected first):
        //   Structure: [Y_ID][Y_len][Y_data][X_ID][X_len][X_data][Map_data]
        //   first_axis_len = y_axis_len (original), second_axis_len = x_axis_len (original)
        //   Y_data at: map_address - 4 - (y_axis_len * 2) - (x_axis_len * 2)
        //   X_data at: map_address - (x_axis_len * 2)
        
        // CRITICAL: Calculate addresses from map_address
        // map_address was calculated using ORIGINAL x_axis_len and y_axis_len (before swap)
        // So we must use ORIGINAL lengths for offset calculation
        //
        // But we need to map the calculated addresses to semantic X and Y based on:
        // 1. Which axis comes first in the file (first_axis_is_x)
        // 2. Which axis is which semantically (actual_x_axis_len, actual_y_axis_len)
        //
        // If first_axis_is_x (X detected first):
        //   Structure: [X_ID][X_len][X_data][Y_ID][Y_len][Y_data][Map_data]
        //   first_axis_data at: map_address - 4 - (x_axis_len * 2) - (y_axis_len * 2)
        //   second_axis_data at: map_address - (y_axis_len * 2)
        //   Since first_axis_is_x, first_axis = X, second_axis = Y
        //   So: X_data = first_axis_data, Y_data = second_axis_data
        //
        // If !first_axis_is_x (Y detected first):
        //   Structure: [Y_ID][Y_len][Y_data][X_ID][X_len][X_data][Map_data]
        //   first_axis_data at: map_address - 4 - (y_axis_len * 2) - (x_axis_len * 2)
        //   second_axis_data at: map_address - (x_axis_len * 2)
        //   Since !first_axis_is_x, first_axis = Y, second_axis = X
        //   So: Y_data = first_axis_data, X_data = second_axis_data
        
        // CRITICAL: Calculate addresses from map_address
        // map_address was calculated using ORIGINAL x_axis_len and y_axis_len (before swap)
        // So we must use ORIGINAL lengths for offset calculation
        //
        // But we need to map the calculated addresses to semantic X and Y based on:
        // 1. Which axis comes first in the file (first_axis_is_x)
        // 2. Which axis is which semantically (actual_x_axis_len, actual_y_axis_len)
        //
        // IMPORTANT: After swap detection:
        // - If axes_were_swapped: first axis in file = Y (semantic), second = X (semantic)
        // - If axes_were_NOT_swapped: first axis in file = X (semantic), second = Y (semantic)
        //
        // For address calculation from map_address:
        // - If first_axis_is_x (X detected first): [X_ID][X_len][X_data][Y_ID][Y_len][Y_data][Map_data]
        //   - first_axis_data (X) at: map_address - 4 - (x_axis_len * 2) - (y_axis_len * 2)
        //   - second_axis_data (Y) at: map_address - (y_axis_len * 2)
        // - If !first_axis_is_x (Y detected first): [Y_ID][Y_len][Y_data][X_ID][X_len][X_data][Map_data]
        //   - first_axis_data (Y) at: map_address - 4 - (y_axis_len * 2) - (x_axis_len * 2)
        //   - second_axis_data (X) at: map_address - (x_axis_len * 2)
        
        // CRITICAL: Calculate final addresses using the ORIGINAL file order lengths
        // map_address was calculated using: structure_start + 8 + (x_axis_len * 2) + (y_axis_len * 2)
        // where x_axis_len and y_axis_len are the ORIGINAL lengths as read from file
        //
        // We need to map these addresses to semantic X and Y based on which axis comes first:
        let actual_x_bytes = (actual_x_axis_len as u32).saturating_mul(2);
        let actual_y_bytes = (actual_y_axis_len as u32).saturating_mul(2);
        let (first_axis_bytes, second_axis_bytes) = if first_axis_is_x {
            (actual_x_bytes, actual_y_bytes)
        } else {
            (actual_y_bytes, actual_x_bytes)
        };

        let (final_x_axis_address, final_y_axis_address) = if first_axis_is_x {
            // First axis corresponds to semantic X, second axis to semantic Y
            (
                map_address - 4 - first_axis_bytes - second_axis_bytes,
                map_address - second_axis_bytes,
            )
        } else {
            // First axis corresponds to semantic Y, second axis to semantic X
            (
                map_address - second_axis_bytes,
                map_address - 4 - first_axis_bytes - second_axis_bytes,
            )
        };
        
        log::debug!(
            "✅ FINAL addresses for '{}': X_axis=0x{:X}, Y_axis=0x{:X} (using actual_x_len={}, actual_y_len={}, first_axis_is_x={})",
            pattern.name, final_x_axis_address, final_y_axis_address, actual_x_axis_len, actual_y_axis_len, first_axis_is_x
        );
        
        // Log for debugging
        log::debug!(
            "📍 Map '{}' - structure_start=0x{:X}, X_data=0x{:X} (len={}), Y_data=0x{:X} (len={}), map_addr=0x{:X}",
            pattern.name, structure_start, x_axis_address, x_axis_len, y_axis_address, y_axis_len, map_address
        );
        log::debug!(
            "   Corrections: X_corr={}, X_offset={}, Y_corr={}, Y_offset={}",
            pattern.x_axis_correction, pattern.x_axis_offset, pattern.y_axis_correction, pattern.y_axis_offset
        );

        map.name = Some(format!("{}", pattern.name));
        // Driver wish : valeurs signées (WinOLS bSigned=1) — les cellules frein
        // moteur portent des IQ négatifs (raw 0xFFxx → -0.9, pas 654.5)
        if pattern.name.contains("Driver wish") {
            map.data_type = DataType::Int16;
        }
        map.unit = Some(pattern.z_axis_descr.clone());
        map.description = Some(format!(
            "{} | X: {} ({}) | Y: {} ({}) | Axis IDs: X=0x{:04X} Y=0x{:04X}",
            pattern.z_axis_descr,
            pattern.x_axis_descr,
            pattern.x_axis_units,
            pattern.y_axis_descr,
            pattern.y_axis_units,
            x_axis_id,
            y_axis_id
        ));
        map.confidence = 0.95;
        
        // Set axis addresses and correction factors
        // CRITICAL: Use addresses calculated from map_address (more reliable)
        // The calculation logic should work correctly for all maps
        // NOTE: NO swap needed for IQ by MAF/MAP - the axis calculation already handles the correct order
        
        // CRITICAL: For Boost target 280-byte maps, SWAP X and Y addresses
        // Symbol expects: X=0x5690E (IQ), Y=0x568EE (RPM)
        // Current calculation gives: final_x=0x568EE (RPM), final_y=0x5690E (IQ)
        if pattern.name.contains("Boost target") && map.size == 280 {
            // SWAP: final_y becomes X (IQ), final_x becomes Y (RPM)
            map.x_axis_address = Some(final_y_axis_address);
            map.y_axis_address = Some(final_x_axis_address);
            // Update description with SWAPPED addresses
            map.description = Some(format!(
                "Target boost pressure (mbar) | X: IQ (mg/st) | Y: Engine speed (rpm) | Axis: X=0x{:04X}(IQ) Y=0x{:04X}(RPM)",
                final_y_axis_address, final_x_axis_address
            ));
            log::debug!("BOOST TARGET 280 SWAP: X=0x{:X}(IQ) Y=0x{:X}(RPM)", final_y_axis_address, final_x_axis_address);
        } else {
            map.x_axis_address = Some(final_x_axis_address);
            map.y_axis_address = Some(final_y_axis_address);
        }

        log::debug!(
            "🎯 Calculated addresses for '{}' (map=0x{:X}): X_axis=0x{:X}, Y_axis=0x{:X} (first_axis_is_x={}, actual_x_len={}, actual_y_len={})",
            pattern.name, map_address, final_x_axis_address, final_y_axis_address, first_axis_is_x, actual_x_axis_len, actual_y_axis_len
        );
        
        // Debug: Log expected addresses for Start IQ maps
        if pattern.name.contains("Start IQ") {
            log::debug!(
                "Start IQ Debug - map=0x{:X}, X=0x{:X}, Y=0x{:X}, swapped={}",
                map_address, final_x_axis_address, final_y_axis_address, axes_were_swapped
            );
        }
        map.correction_factor = Some(pattern.correction);
        map.offset = Some(pattern.offset);
        map.x_axis_correction = Some(pattern.x_axis_correction);
        map.y_axis_correction = Some(pattern.y_axis_correction);
        // CRITICAL FIX: Set axis offsets from pattern (for temperature conversions like Kelvin → Celsius)
        map.x_axis_offset = Some(pattern.x_axis_offset);
        map.y_axis_offset = Some(pattern.y_axis_offset);

        map
    }

    /// Filter out false positives
    /// EXACTLY like VAGEDCSuite AddToSymbolCollection - only check for duplicates by address
    /// BUT: prefer maps with specific names (like "°C") over generic ones ("3D Map Size:")
    fn filter_maps(&self, maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        if maps.is_empty() {
            return Vec::new();
        }
        
        let mut best_by_address: HashMap<u32, DetectedMap> = HashMap::new();

        for map in maps {
            let addr = map.address;
            let map_name = map.name.clone().unwrap_or_default();
            let is_specific = map_name.contains("°C") || // SOI with temperature
                (!map_name.starts_with("3D Map Size:") && !map_name.contains("[0x")); // Named map without address suffix
            
            // Log SOI maps for debugging
            if map_name.contains("injection") || map_name.contains("SOI") {
                log::debug!("🔍 filter_maps: SOI map at 0x{:X}: '{}' (specific={})", addr, map_name, is_specific);
            }
            
            if let Some(existing) = best_by_address.get(&addr) {
                let existing_name = existing.name.clone().unwrap_or_default();
                let existing_is_specific = existing_name.contains("°C") || 
                    (!existing_name.starts_with("3D Map Size:") && !existing_name.contains("[0x"));
                
                // Replace if new map is more specific
                if is_specific && !existing_is_specific {
                    log::debug!("🔄 Replacing generic map at 0x{:X} '{}' with specific: '{}'", addr, existing_name, map_name);
                    best_by_address.insert(addr, map);
                } else if map_name.contains("°C") || existing_name.contains("°C") {
                    log::debug!("⚠️ Duplicate at 0x{:X}: keeping '{}', discarding '{}' (new_specific={}, existing_specific={})", 
                        addr, existing_name, map_name, is_specific, existing_is_specific);
                }
                // Keep existing if it's specific or if new is not more specific
            } else {
                best_by_address.insert(addr, map);
            }
        }

        let mut filtered: Vec<DetectedMap> = best_by_address.into_values().collect();
        filtered.sort_by_key(|m| m.address);
        filtered
    }
    
    /// Determine flashbank from address (EDCSuite style)
    /// - Flashbank 1 (codeblock 5): 0x40000-0x5FFFF
    /// - Flashbank 2 (codeblock 2 manual): 0x60000-0x7FFFF
    fn get_flashbank_from_address(&self, address: u32) -> Option<u32> {
        // EDC15P codeblocks are 0x10000 (64KB) blocks starting at 0x4C000
        // Based on EDCSuite:
        // - Codeblock 2: 0x4C000 - 0x5BFFF
        // - Codeblock 3: 0x5C000 - 0x6BFFF
        // - Codeblock 5: 0x6C000 - 0x7BFFF
        // We use internal IDs 1, 2, 3 for deduplication (one map per codeblock)
        if address >= 0x4C000 && address < 0x5C000 {
            Some(1) // Codeblock 2 (EDCSuite)
        } else if address >= 0x5C000 && address < 0x6C000 {
            Some(2) // Codeblock 3 (EDCSuite)
        } else if address >= 0x6C000 && address < 0x7C000 {
            Some(3) // Codeblock 5 (EDCSuite)
        } else if address >= 0x40000 && address < 0x4C000 {
            Some(1) // Early addresses in first codeblock
        } else if address >= 0x7C000 && address < 0x80000 {
            Some(3) // Late addresses in last codeblock
        } else {
            None
        }
    }
    
    /// Distinguish IQ by MAF and IQ by MAP based on X axis values
    /// Both are detected with the same pattern "IQ by MAF limiter", this function renames to "IQ by MAP limiter" when appropriate
    fn distinguish_iq_limiter_maps(&self, data: &[u8], maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        maps.into_iter().map(|mut map| {
            // Check if this is an IQ by MAF limiter that might actually be IQ by MAP
            if let Some(ref name) = map.name {
                if name.contains("IQ by MAF limiter") {
                    // X axis contains the distinguishing data:
                    // - IQ by MAF: X axis = Airflow (values >= 2500 raw, e.g., 3000 = 300 mg/st)
                    // - IQ by MAP: X axis = Boost pressure (values < 2500, e.g., 800-2200 mbar)
                    if let Some(x_addr) = map.x_axis_address {
                        if (x_addr as usize) + 2 < data.len() {
                            let first_x_val = u16::from_le_bytes([
                                data[x_addr as usize], 
                                data[x_addr as usize + 1]
                            ]);
                            
                            // IQ by MAF: X axis >= 2500 (Airflow 3000-10500 raw = 300-1050 mg/st * 0.1)
                            // IQ by MAP: X axis < 2500 (Boost pressure 800-2200 mbar)
                            if first_x_val < 2500 {
                                // This is actually IQ by MAP limiter (Boost pressure)
                                let new_name = name.replace("IQ by MAF limiter", "IQ by MAP limiter");
                                log::debug!("🔄 Renamed '{}' to '{}' (first X val={} < 2500 = Boost pressure)", 
                                    name, new_name, first_x_val);
                                map.name = Some(new_name);
                                map.description = Some("Max IQ by boost pressure | X: Boost (mbar) | Y: Engine speed (rpm)".to_string());
                                // For IQ by MAP, X axis is Boost (no factor needed, already in mbar)
                                map.x_axis_correction = Some(1.0);
                } else {
                                log::debug!("✅ Confirmed IQ by MAF limiter at 0x{:X} (first X val={} >= 2500 = Airflow)", 
                                    map.address, first_x_val);
                            }
                        }
                    }
                }
            }
            map
        }).collect()
    }
    
    /// Filter duplicate maps by type - keeps only ONE map per type per flashbank
    /// This is a universal solution for maps that should only appear once per flashbank
    fn filter_duplicate_maps_by_type(&self, maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        // List of map type prefixes that should be deduplicated
        // IQ by MAF and IQ by MAP are BOTH filtered (one of each per codeblock)
        let unique_per_flashbank = vec![
            // "Smoke limiter" moved to unique_by_full_name - we want 3 per codeblock (one per temp)
            "IQ by MAF limiter",  // One per codeblock
            "IQ by MAP limiter",  // One per codeblock (different from MAF)
            "Torque limiter",
            "Boost target map",
            "N75 duty cycle",
            "Driver wish",
            // "Inversed driver wish" - REMOVED: no longer detected
            "Boost limit map",
            "Boost correction by temperature",
            "Limit of overboost protection",
            "SOI limiter",
            "MAF correction by temperature",  // One per codeblock
            "EGR temperature map",  // One per codeblock
            "EGR",  // One per codeblock (keep lowest address)
        ];

        // Maps that use their FULL name for deduplication (one per name per flashbank)
        // SOI maps are handled separately by filter_false_soi_maps - we want 10 per codeblock
        // Smoke limiters: we want 3 per codeblock (one per temperature: -20°C, 40°C, 80°C)
        let unique_by_full_name: Vec<&str> = vec![
            "Smoke limiter",  // Full name includes temperature, e.g., "Smoke limiter -20 °C"
        ];
        
        // First pass: collect all maps by type and flashbank, keeping the HIGHEST address
        // Key: (map_type_prefix, flashbank) -> (highest_address, map_index)
        let mut best_per_flashbank: std::collections::HashMap<(String, u32), (u32, usize)> = std::collections::HashMap::new();
        
        for (idx, map) in maps.iter().enumerate() {
            let map_name = map.name.as_ref().map(|s| s.as_str()).unwrap_or("");
            
            // Check if this map type should be deduplicated by prefix
            let mut matched_type: Option<String> = None;
            for type_prefix in &unique_per_flashbank {
                if map_name.starts_with(type_prefix) {
                    matched_type = Some(type_prefix.to_string());
                    break;
                }
            }
            // Special handling for EGR (416 bytes): deduplicate separately from "EGR temperature map"
            if matched_type.is_none() && (map_name == "EGR" || map_name.starts_with("EGR 0")) {
                matched_type = Some("EGR".to_string());
            }
            
            // Check if this map should be deduplicated by FULL name (for SOI maps with temps)
            if matched_type.is_none() {
                for type_prefix in &unique_by_full_name {
                    if map_name.starts_with(type_prefix) {
                        // Use the full name as the key (e.g., "Start of injection (SOI) 90°C")
                        matched_type = Some(map_name.to_string());
                        break;
                    }
                }
            }
            
            if let Some(map_type) = matched_type {
                if let Some(flashbank) = self.get_flashbank_from_address(map.address) {
                    let key = (map_type.clone(), flashbank);
                    
                    if let Some((existing_addr, _)) = best_per_flashbank.get(&key) {
                        // For EGR, keep the one with LOWEST address (real EGR is at beginning of codeblock)
                        // For other maps, keep the one with HIGHEST address (real map is after axis data)
                        let should_replace = if map_type == "EGR" || map_type.starts_with("EGR ") {
                            map.address < *existing_addr
                        } else {
                            map.address > *existing_addr
                        };
                        
                        if should_replace {
                            log::debug!("🔄 {} at 0x{:X} replacing 0x{:X} (flashbank {})", 
                                map_type, map.address, existing_addr, flashbank);
                            best_per_flashbank.insert(key, (map.address, idx));
                        }
                    } else {
                        best_per_flashbank.insert(key, (map.address, idx));
                    }
                }
            }
        }
        
        // Second pass: filter maps, keeping only the best one per type per flashbank
        let mut filtered = Vec::new();
        for (idx, map) in maps.into_iter().enumerate() {
            let map_name = map.name.as_ref().map(|s| s.as_str()).unwrap_or("");
            
            // Check if this map type should be deduplicated by prefix
            let mut matched_key: Option<String> = None;
            for type_prefix in &unique_per_flashbank {
                if map_name.starts_with(type_prefix) {
                    matched_key = Some(type_prefix.to_string());
                    break;
                }
            }
            
            // Check if this map should be deduplicated by FULL name
            if matched_key.is_none() {
                for type_prefix in &unique_by_full_name {
                    if map_name.starts_with(type_prefix) {
                        matched_key = Some(map_name.to_string());
                        break;
                    }
                }
            }
            
            if let Some(map_type) = matched_key {
                if let Some(flashbank) = self.get_flashbank_from_address(map.address) {
                    let key = (map_type.clone(), flashbank);
                    
                    if let Some((_, best_idx)) = best_per_flashbank.get(&key) {
                        if idx == *best_idx {
                            // This is the best one, keep it
                            log::debug!("✅ Keeping {} at 0x{:X} (flashbank {}) - highest address", 
                                map_type, map.address, flashbank);
                        filtered.push(map);
                    } else {
                            // Not the best one, skip it
                            log::debug!("⏭️ Skipping {} at 0x{:X} (flashbank {}) - not highest address", 
                                map_type, map.address, flashbank);
                        }
                                } else {
                            filtered.push(map);
                        }
                } else {
                    filtered.push(map);
                    }
                } else {
                // Not a deduplicated type, keep it
                    filtered.push(map);
                }
        }
        
        filtered
    }
    
    /// Filter EGR maps to keep only one per codeblock (legacy - now uses filter_duplicate_maps_by_type)
    fn filter_egr_maps(&self, maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        // Now handled by filter_duplicate_maps_by_type
        self.filter_duplicate_maps_by_type(maps)
    }
    
    /// Filter nearby maps with same name prefix - false positives are often detected near real maps
    /// If two maps with similar names are within 50 bytes, keep only the highest address
    /// (real map data comes after axis data, so it has higher address)
    fn filter_nearby_duplicate_maps(&self, maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        const PROXIMITY_THRESHOLD: u32 = 50; // bytes - 4DCCE - 4DCBE = 16 bytes
        
        let mut filtered: Vec<DetectedMap> = Vec::new();
        
        for map in maps {
            // Ne pas filtrer les petites maps diagnostics 1x1 (taille 2) : elles sont nombreuses et proches
            let is_small_diag = (map.subcategory.as_deref() == Some("VCDS diagnostic") || map.category.as_deref() == Some("VCDS diagnostic")) && map.size == 2;
            if is_small_diag {
                filtered.push(map);
                continue;
            }

            let map_name = map.name.as_ref().map(|s| s.as_str()).unwrap_or("");
            // Extract base name without address (e.g., "Smoke limiter" from "Smoke limiter [0x04DCBE]")
            let map_base_name: String = map_name.split('[').next().unwrap_or("").trim().to_string();
            
            // Check if there's already a similar map nearby
            let mut dominated = false;
            let mut dominates_idx: Option<usize> = None;
            
            for (idx, existing) in filtered.iter().enumerate() {
                let existing_name = existing.name.as_ref().map(|s| s.as_str()).unwrap_or("");
                let existing_base_name: String = existing_name.split('[').next().unwrap_or("").trim().to_string();
                
                // Same base name and same flashbank?
                if !map_base_name.is_empty() && map_base_name == existing_base_name {
                    let same_flashbank = self.get_flashbank_from_address(existing.address) == 
                                        self.get_flashbank_from_address(map.address);
                    if same_flashbank {
                        let distance = if map.address > existing.address {
                            map.address - existing.address
            } else {
                            existing.address - map.address
                        };
                        
                        if distance < PROXIMITY_THRESHOLD {
                            // Maps are close - keep the one with higher address
                            if map.address > existing.address {
                                // New map has higher address, it dominates the existing one
                                dominates_idx = Some(idx);
                                log::debug!("🔄 {} at 0x{:X} dominates nearby map at 0x{:X} - keeping higher", 
                                    map_base_name, map.address, existing.address);
                            } else {
                                // Existing map has higher address, new map is dominated
                                dominated = true;
                                log::debug!("⏭️ Skipping {} at 0x{:X} - dominated by nearby map at 0x{:X}", 
                                    map_base_name, map.address, existing.address);
                            }
                            break;
                        }
                    }
                }
            }
            
            if let Some(idx) = dominates_idx {
                // Remove the dominated map and add the new one
                filtered.remove(idx);
                filtered.push(map);
            } else if !dominated {
                filtered.push(map);
            }
        }
        
        filtered
    }

    /// VCDS diagnostic sequences (IQ/MAP/Torque limits & display offsets) comme EDCSuite (FindVCDSIQDiag1)
    /// On scanne une séquence bien connue puis on crée une grappe de maps 1x1 avec corrections dédiées
    /// Note: The pattern varies between ECU files, so we use a shorter, more universal pattern
    fn detect_vcds_diag_sequences(&self, data: &[u8], maps: &mut Vec<DetectedMap>, seen: &mut HashSet<u32>) {
        log::debug!("🔎 detect_vcds_diag_sequences CALLED, data len = {}", data.len());
        // Shorter universal pattern that works across different EDC15P files
        // The key sequence is "01 69 10 0F 05 09" which appears in all files
        const PATTERN1: [u8; 6] = [0x01, 0x69, 0x10, 0x0F, 0x05, 0x09];
        const PATTERN2: [u8; 10] = [0xD4, 0xFE, 0x91, 0x00, 0xB6, 0xFE, 0x8C, 0x05, 0xB6, 0xFE];

        // Helpers
        let find_all = |pat: &[u8]| -> Vec<usize> {
            let mut res = Vec::new();
            let mut i = 0;
            while i + pat.len() <= data.len() {
                if &data[i..i + pat.len()] == pat {
                    res.push(i);
                }
                i += 1;
            }
            res
        };

        let mut add_map = |addr: u32, name: &str, corr: f64, off: f64, description: &str, unit: &str, signed: bool| {
            if seen.contains(&addr) {
                log::debug!("⏭️ VCDS map {} at 0x{:X} already seen, skipping", name, addr);
                return;
            }
            log::debug!("➕ Adding VCDS map: {} at 0x{:X} (signed: {})", name, addr, signed);
            let dimensions = MapDimensions::TwoDimensional { rows: 1, cols: 1 };
            let data_type = if signed { DataType::Int16 } else { DataType::UInt16 };
            let mut m = DetectedMap::new(addr, 2, dimensions, data_type);
            m.name = Some(name.to_string()); // pas d'adresse dans le nom
            m.category = Some("VCDS diagnostic".to_string());
            m.subcategory = Some("VCDS diagnostic".to_string());
            m.correction_factor = Some(corr);
            m.offset = Some(off);
            m.confidence = 0.6;
            // Description détaillée avec format X: ... Y: ... Z: ...
            if !description.is_empty() {
                m.description = Some(format!("X: {} Y: Z:", description));
                m.x_label = Some(description.to_string());
            }
            if !unit.is_empty() {
                m.unit = Some(unit.to_string());
            }
            seen.insert(addr);
            maps.push(m);
        };

        // PATTERN1 group (IQ/MAP/Torque)
        // Note: Offsets adjusted for shorter pattern (pattern now starts 4 bytes later)
        // Old offsets were relative to 0xFF,0xFF,0xFF,0xFF prefix, new offsets are relative to 0x01,0x69...
        let bases = find_all(&PATTERN1);
        log::debug!("🔎 VCDS PATTERN1 found {} times: {:?}", bases.len(), bases.iter().map(|b| format!("0x{:X}", b)).collect::<Vec<_>>());
        for base in bases {
            let base_u32 = base as u32;
            log::debug!("🔎 Processing PATTERN1 base at 0x{:X}", base_u32);
            // IQ limits 1..10 (offsets adjusted: old - 4)
            let iq_offsets = [8_u32, 52, 60, 104, 116, 120, 140, 192, 216, 224];
            for (idx, off) in iq_offsets.iter().enumerate() {
                let addr = base_u32 + off;
                log::debug!("  Adding IQ Limit {} at 0x{:X}", idx + 1, addr);
                add_map(
                    addr,
                    &format!("VCDS Diagnostic IQ Limit {}", idx + 1),
                    0.00390625,
                    -0.15234375,
                    "IQ Limit for VCDS group display in mg/stroke. Raw value×0.0039-0.15=mg shown. Common: 25600→100mg, 17920→70mg. Whole numbers only!",
                    "mg",
                    true // signed
                );
            }
            // MAP limits 1..3 (offsets adjusted: old - 4)
            let map_offsets = [24_u32, 28, 68];
            for (idx, off) in map_offsets.iter().enumerate() {
                let addr = base_u32 + off;
                add_map(
                    addr,
                    &format!("VCDS Diagnostic MAP Limit {}", idx + 1),
                    1.0,
                    0.0,
                    "MAP sensor limit for VCDS display in mbar. Values: -238(stock 2.5bar), 29960(3bar sensor), 40200(4bar sensor). Negative=relative to atmospheric",
                    "mbar",
                    true // signed - important for negative values like -238
                );
            }
            // Torque limit (offset adjusted: 188 - 4 = 184)
            let tor_addr = base_u32 + 184;
            add_map(
                tor_addr,
                "VCDS Diagnostic Torque Limit",
                0.00390625,
                -0.203125,
                "Torque limit for VCDS display. Real torque=displayed×4.12. Ex: 100→412Nm, 146→601Nm, 195→803Nm. Raw×0.0039-0.2=displayed value",
                "NM",
                true // signed
            );
        }

        // PATTERN2 group (display offsets)
        let bases2 = find_all(&PATTERN2);
        log::debug!("🔎 VCDS PATTERN2 found {} times: {:?}", bases2.len(), bases2.iter().map(|b| format!("0x{:X}", b)).collect::<Vec<_>>());
        for base in bases2 {
            let base_u32 = base as u32;
            // MAF Display offset removed per user request
            // Ces valeurs sont des facteurs de mise à l'échelle pour VCDS
            // Pour calculer la limite max affichable: Limite_Max = 255 / Valeur × Multiplicateur
            // Exemple: IQ=364 → 255/364×100 = 70mg max | IQ=255 → 255/255×100 = 100mg max
            let entries: [(u32, &str, &str); 3] = [
                (42_u32, "VCDS Diagnostic Torque Display offset", "Scaling factor for VCDS torque display. Calculate max: 255/VALUE×1000=Nm. Ex: 620→411Nm, 425→600Nm. To change: new_value=255/desired_Nm×1000"),
                (66_u32, "VCDS Diagnostic MAP Display offset", "Scaling factor for VCDS MAP display. Calculate max: 255/VALUE×10000=mbar. Ex: 638→4000mbar(4bar), 833→3061mbar(3bar). To change: new_value=255/desired_mbar×10000"),
                (138_u32, "VCDS Diagnostic IQ Display offset", "Scaling factor for VCDS IQ display. Calculate max: 255/VALUE×100=mg. Ex: 255→100mg, 364→70mg. To change: new_value=255/desired_mg×100"),
            ];
            for (off, label, desc) in entries {
                let addr = base_u32 + off;
                add_map(addr, label, 1.0, 0.0, desc, "", true); // signed
            }
        }
    }

    /// Forcer le dossier Diagnostics et nettoyer les noms (pas d'adresse dans le nom)
    fn normalize_diagnostics(&self, maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        maps
            .into_iter()
            .map(|mut m| {
                let n = m.name.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
                // Exclude specific maps that should NOT be in VCDS Diagnostic
                let is_excluded = n.contains("left foot brake")
                    || n.contains("svbl")
                    || n.contains("svrl")
                    || n.contains("map/maf switch");
                let is_diag = !is_excluded && (
                    m.subcategory.as_deref() == Some("VCDS diagnostic")
                    || m.category.as_deref() == Some("VCDS diagnostic")
                    || n.contains("vcds")  // All VCDS maps are diagnostics
                    || m.size == 2
                );
                if is_diag {
                    m.category = Some("VCDS diagnostic".to_string());
                    m.subcategory = Some("VCDS diagnostic".to_string());
                    if let Some(name) = &m.name {
                        if let Some(pos) = name.find('[') {
                            m.name = Some(name[..pos].trim().to_string());
                        } else if let Some(pos) = name.find('(') { // au cas où le suffixe est entre ()
                            m.name = Some(name[..pos].trim().to_string());
                        }
                    }
                }
                m
            })
            .collect()
    }
    
    // REMOVED: filter_zeroed_inversed_driver_wish - Inversed driver wish detection disabled

    /// Filter invalid EGR temperature maps: must have a temperature axis (0xC1**) and the other axis in mbar family (0xEC/0xEA/0xDA/0xC0),
    /// temp axis must be in plausible range/spread, and map data must not be flat.
    fn filter_invalid_egr_temp_maps(&self, maps: Vec<DetectedMap>, data: &[u8]) -> Vec<DetectedMap> {
        maps.into_iter()
            .filter(|map| {
                let name = map.name.as_deref().unwrap_or("").to_lowercase();
                if !name.contains("egr temperature map") {
                    return true;
                }
                // Basic dimension check already done during classification (5x6 or 6x5)
                // Axis validation based on stored axis IDs
                let x_id = map.x_axis_address.and_then(|addr| {
                    let off = addr.saturating_sub(4) as usize;
                    if off + 2 <= data.len() { Some(u16::from_le_bytes([data[off], data[off + 1]])) } else { None }
                });
                let y_id = map.y_axis_address.and_then(|addr| {
                    let off = addr.saturating_sub(4) as usize;
                    if off + 2 <= data.len() { Some(u16::from_le_bytes([data[off], data[off + 1]])) } else { None }
                });
                let x_hi = x_id.map(|v| (v >> 8) as u8);
                let y_hi = y_id.map(|v| (v >> 8) as u8);
                let is_temp_x = x_hi == Some(0xC1);
                let _is_temp_y = y_hi == Some(0xC1);
                let _is_mbar_x = matches!(x_hi, Some(0xEC | 0xEA | 0xDA | 0xC0));
                let is_mbar_y = matches!(y_hi, Some(0xEC | 0xEA | 0xDA | 0xC0));

                // Dimensions (semantic): cols = X, rows = Y
                let (x_len, y_len) = match map.dimensions {
                    MapDimensions::OneDimensional { length } => (length, 1),
                    MapDimensions::TwoDimensional { cols, rows } => (cols, rows),
                    MapDimensions::ThreeDimensional { x, y, .. } => (x, y),
                };

                // STRONG filter: temp must be on X, mbar on Y, with expected lengths 6x5
                let valid_axes = is_temp_x && is_mbar_y && x_len == 6 && y_len == 5;
                if !valid_axes {
                    log::debug!(
                        "⏭️ Skipping EGR temp at 0x{:X}: invalid axes/order/len (x_hi={:?}, y_hi={:?}, x_len={}, y_len={})",
                        map.address, x_hi, y_hi, x_len, y_len
                    );
                    return false;
                }
                // Check temperature axis plausibility: raw temps should be ~2000-4500 (≈ -70°C à 180°C) and have some spread
                let (temp_addr, temp_len) = if is_temp_x {
                    (
                        map.x_axis_address,
                        match map.dimensions {
                            MapDimensions::OneDimensional { length } => length,
                            MapDimensions::TwoDimensional { cols, .. } => cols,
                            MapDimensions::ThreeDimensional { x, .. } => x,
                        },
                    )
                } else {
                    (
                        map.y_axis_address,
                        match map.dimensions {
                            MapDimensions::OneDimensional { .. } => 1,
                            MapDimensions::TwoDimensional { rows, .. } => rows,
                            MapDimensions::ThreeDimensional { y, .. } => y,
                        },
                    )
                };
                if let Some(addr) = temp_addr {
                    let mut vals = Vec::new();
                    for i in 0..temp_len {
                        let off = addr as usize + i * 2;
                        if off + 2 <= data.len() {
                            vals.push(u16::from_le_bytes([data[off], data[off + 1]]));
                        }
                    }
                    if !vals.is_empty() {
                        let min = *vals.iter().min().unwrap_or(&0);
                        let max = *vals.iter().max().unwrap_or(&0);
                        let spread = max.saturating_sub(min);
                        if min < 1500 || max > 5000 || spread < 100 {
                            log::debug!(
                                "⏭️ Skipping EGR temp at 0x{:X}: temp axis out of range or flat (min={}, max={}, spread={})",
                                map.address, min, max, spread
                            );
                            return false;
                        }
                    }
                }
                // PAS de filtre « données plates » ici : une EGR temperature
                // map entièrement à zéro est LÉGITIME (EGR coupé par un stage,
                // ex. multimap Benicio) et EDCSuite l'affiche aussi. Les gardes
                // ci-dessus (IDs d'axes 0xC1/0xEC, 6x5 exact, plage et écart de
                // l'axe température) suffisent à écarter les faux positifs.
                true
            })
            .collect()
    }

    /// Filter and rename SOI maps with correct temperatures
    /// Filter false SOI maps and keep only valid ones:
    /// - Maps already with temperature (from detect_soi_maps_by_selector) are kept as-is
    /// - Maps with "(12)" suffix are filtered IF maps with temps already exist (to avoid duplicates)
    /// - Other SOI maps are filtered as false positives
    fn filter_false_soi_maps(&self, maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        // First, check if we have SOI maps with temperatures already (from detect_soi_maps_by_selector)
        // These maps have map_selector set AND their name contains "°C"
        // Support 3 codeblocks (CB2, CB3, CB5 = internal IDs 1, 2, 3)
        let mut has_temp_soi = [false, false, false]; // For codeblocks 2, 3, 5
        
        for map in &maps {
            // Check if this is a temperature-based SOI map (has map_selector from detect_soi_maps_by_selector)
            let is_temp_soi = map.map_selector.is_some() && 
                map.name.as_ref().map_or(false, |n| n.contains("Start of injection") && n.contains("°C"));
            
            if is_temp_soi {
                let flashbank = self.get_flashbank_from_address(map.address);
                if let Some(fb) = flashbank {
                    if fb >= 1 && fb <= 3 {
                        has_temp_soi[(fb - 1) as usize] = true;
                    }
                }
                log::debug!("✅ Found temperature SOI map: {:?} at 0x{:X} (FB{:?})", map.name, map.address, flashbank);
            }
        }
        
        log::debug!("🌡️ SOI maps with temperatures: CB2={}, CB3={}, CB5={}", has_temp_soi[0], has_temp_soi[1], has_temp_soi[2]);
        
        // Filter maps
        let mut result: Vec<DetectedMap> = Vec::new();
        
        for map in maps.into_iter() {
            if let Some(ref name) = map.name {
                if name.contains("Start of injection") && !name.contains("SOI limiter") {
                    // Case 1: Has temperature in name (from detect_soi_maps_by_selector) - KEEP
                    if name.contains("°C") {
                        log::debug!("✅ Keeping SOI with temperature: {} at 0x{:X}", name, map.address);
                        result.push(map);
                        continue;
                    }
                    
                    // Case 2: No temperature in name - FILTER if we have temperature-based SOI maps for this codeblock
                    let flashbank = self.get_flashbank_from_address(map.address);
                    let has_temps = flashbank.map_or(false, |fb| {
                        fb >= 1 && fb <= 3 && has_temp_soi[(fb - 1) as usize]
                    });
                    
                    if has_temps {
                        // Already have temperature-named maps for this codeblock, filter this one
                        log::debug!("🗑️ Filtering SOI without temperature: {} at 0x{:X}", name, map.address);
                        continue;
                    }
                    
                    // No temps for this codeblock - this shouldn't happen normally, but keep the map
                    log::debug!("⚠️ Keeping SOI (no temp maps in codeblock): {} at 0x{:X}", name, map.address);
                    result.push(map);
                    continue;
                }
            }
            // Keep all non-SOI maps
            result.push(map);
        }
        
        let final_soi_count = result.iter()
            .filter(|m| m.name.as_ref().map_or(false, |n| n.contains("Start of injection") && !n.contains("SOI limiter")))
            .count();
        
        log::debug!("🔥 SOI filter: {} SOI maps remaining", final_soi_count);
        
        result
    }

    /// Fix Injector duration maps - renumber by size and address, swap axes, filter invalid
    /// - 200 bytes (10x10) = Injector duration 00 or 05
    /// - 480 bytes (16x15) = Injector duration 01-04 (by address order)
    /// - 570 bytes (19x15) = Injector duration 01-04 (by address order) - alternate size
    /// - 198 bytes (11x9) = Injector duration 05
    fn fix_injector_duration_maps(&self, maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        let valid_sizes = [200, 480, 570, 198, 220];
        let mut result: Vec<DetectedMap> = Vec::new();
        
        // Collect ALL Injector duration maps by codeblock with addresses sorted
        // Support 3 codeblocks (internal IDs 1, 2, 3 = EDCSuite CB2, CB3, CB5)
        let mut cb_addrs: [Vec<(u32, usize)>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        
        // First pass: collect all injector duration map addresses
        for map in &maps {
            if let Some(ref name) = map.name {
                if name.contains("Injector duration") && !name.contains("Selector") {
                    if !valid_sizes.contains(&map.size) { continue; }
                    
                    let flashbank = self.get_flashbank_from_address(map.address);
                    if let Some(fb) = flashbank {
                        if fb >= 1 && fb <= 3 {
                            cb_addrs[(fb - 1) as usize].push((map.address, map.size));
                        }
                    }
                }
            }
        }
        
        // Sort by address within each codeblock
        for addrs in &mut cb_addrs {
            addrs.sort_by_key(|&(addr, _)| addr);
        }
        
        // Create address->number mapping
        // Pattern: 00 (200B first), 01-04 (480B), 05 (200B/198B last)
        let create_numbering = |addrs: &[(u32, usize)]| -> std::collections::HashMap<u32, String> {
            let mut mapping = std::collections::HashMap::new();
            if addrs.is_empty() { return mapping; }
            
            let mut dur_mid_count = 0; // For 480 or 570 byte maps (duration 01-04)
            let first_200_assigned = std::cell::Cell::new(false);

            for (addr, size) in addrs {
                let num = match *size {
                    200 => {
                        if !first_200_assigned.get() {
                            first_200_assigned.set(true);
                            "00".to_string()
                        } else {
                            "05".to_string() // Second 200B map is Duration 05
                        }
                    },
                    480 | 570 => {
                        dur_mid_count += 1;
                        format!("{:02}", dur_mid_count) // 01, 02, 03, 04
                    },
                    198 | 220 => "05".to_string(),
                    _ => continue,
                };
                mapping.insert(*addr, num);
            }
            mapping
        };
        
        let cb_numbering: Vec<_> = cb_addrs.iter().map(|a| create_numbering(a)).collect();
        
        log::debug!("🔧 Injector durations: CB2={}, CB3={}, CB5={}", 
            cb_addrs[0].len(), cb_addrs[1].len(), cb_addrs[2].len());
        
        // Second pass: process all maps
        for mut map in maps {
            if let Some(ref name) = map.name {
                if name.contains("Injector duration") && !name.contains("Selector") {
                    // Skip invalid sizes
                    if !valid_sizes.contains(&map.size) {
                        log::debug!("🗑️ Skipping invalid Injector duration at 0x{:X} (size {})", map.address, map.size);
                        continue;
                    }
                    
                    let flashbank = self.get_flashbank_from_address(map.address);
                    let numbering = flashbank.and_then(|fb| {
                        if fb >= 1 && fb <= 3 { Some(&cb_numbering[(fb - 1) as usize]) } else { None }
                    });
                    
                    if let Some(numbering) = numbering {
                        if let Some(num) = numbering.get(&map.address) {
                            log::debug!("  🔧 0x{:X} ({}B) -> Injector duration {}", map.address, map.size, num);
                            map.name = Some(format!("Injector duration {}", num));
                            
                            // Injector duration axes based on EDCSuite:
                            // X axis = IQ (mg/stroke) - raw values ×100, factor 0.01
                            // Y axis = RPM - raw values are direct RPM, factor 1.0
                            map.x_axis_correction = Some(0.01); // IQ factor
                            map.y_axis_correction = Some(1.0); // RPM factor

                            // Duration 01-05 have signed values (negative timing advance)
                            if num != "00" {
                                map.data_type = DataType::Int16;
                            }
                            
                            log::debug!("    ✅ Axes: X(IQ)=0x{:X}, Y(RPM)=0x{:X}", 
                                map.x_axis_address.unwrap_or(0), map.y_axis_address.unwrap_or(0));
                        }
                    }
                }
            }
            result.push(map);
        }
        
        result
    }

    /// Classify maps using NameKnownMaps logic from zededc15pfile.cs
    /// This function assigns names, categories, and subcategories to detected maps
    /// IMPORTANT: This function should classify ALL maps, including generic ones (with "3D Map Size:")
    fn name_known_maps(&self, data: &[u8], maps: Vec<DetectedMap>) -> Vec<DetectedMap> {
        let mut classified = Vec::new();
        // Clone maps for counting (we need to iterate over maps and also count from all maps)
        let all_maps = maps.clone();
        // Track codeblocks where EGR has already been classified
        let mut egr_codeblocks = HashSet::new();
        
        for mut map in maps {
            // Si déjà classé en Diagnostics (VCDS), ne pas écraser la catégorie/sous-catégorie
            let name_lower = map.name.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
            let is_diag = map.subcategory.as_deref() == Some("VCDS diagnostic")
                || name_lower.contains("vcds");  // All VCDS maps are diagnostics
            if is_diag {
                classified.push(map);
                continue;
            }

            // Get map dimensions
            let (x_len, y_len) = match map.dimensions {
                MapDimensions::TwoDimensional { rows, cols } => (cols, rows),
                _ => {
                    // Keep non-2D maps as-is
                    classified.push(map);
                    continue;
                }
            };
            
            // Get axis IDs from addresses if available
            // The ID is 4 bytes BEFORE the data address (structure: [ID 2bytes][Len 2bytes][Data])
            // For 1D maps, y_axis_address may be None
            let x_axis_id = if let Some(x_addr) = map.x_axis_address {
                let id_addr = x_addr.saturating_sub(4) as usize;
                if id_addr + 2 <= data.len() {
                    u16::from_le_bytes([data[id_addr], data[id_addr + 1]])
                } else {
                    classified.push(map);
                    continue;
                }
                } else {
                    classified.push(map);
                    continue;
                };
            
            let y_axis_id = if let Some(y_addr) = map.y_axis_address {
                let id_addr = y_addr.saturating_sub(4) as usize;
                if id_addr + 2 <= data.len() {
                    u16::from_le_bytes([data[id_addr], data[id_addr + 1]])
            } else {
                    0u16
                }
            } else {
                0u16 // 1D map - no Y axis
            };
            
            let x_axis_id_high = (x_axis_id >> 8) as u8;
            let y_axis_id_high = (y_axis_id >> 8) as u8;
            
            // Classify based on length, dimensions, and axis IDs (from zededc15pfile.cs)
            // IMPORTANT: Try to classify ALL maps, including generic ones
            let mut classified_this = false;
            
            // First, try to match against patterns for maps that weren't matched during detection
            // This helps classify generic maps that were created without pattern matching
            if map.name.as_ref().map_or(false, |n| n.starts_with("3D Map Size:")) {
                if let Some(pattern) = self.find_matching_pattern(map.size, x_axis_id, y_axis_id, x_len, y_len) {
                    // Found a pattern match! Use it to classify this generic map
                    // But we need to handle multiple maps with same pattern (like SOI maps with different temperatures)
                    if pattern.name.contains("Start of injection (SOI)") && map.size == 448 && x_len == 14 && y_len == 16 {
                        // SOI maps - just number them
                        let soi_count = all_maps.iter()
                            .filter(|m| m.codeblock_id == map.codeblock_id)
                            .filter(|m| m.name.as_ref().map_or(false, |n| n.contains("Start of injection (SOI)")))
                            .count() + 1;
                        map.category = Some("Detected maps".to_string());
                        map.subcategory = Some("1-Fuel".to_string());
                        map.name = Some(format!("Start of injection (SOI) ({})", soi_count));
                        map.correction_factor = Some(-0.023437);
                        map.offset = Some(78.0);
                        map.x_axis_correction = Some(0.01);
                        classified_this = true;
                    } else if pattern.name.contains("Injector duration") {
                        // Injector duration - count them
                        let inj_dur_count = all_maps.iter()
                            .filter(|m| m.codeblock_id == map.codeblock_id)
                            .filter(|m| m.name.as_ref().map_or(false, |n| n.starts_with("Injector duration")))
                            .count() + 1;
                        map.category = Some("Detected maps".to_string());
                        map.subcategory = Some("1-Fuel".to_string());
                        map.name = Some(format!("Injector duration {:02} [0x{:06X}]", inj_dur_count, map.address));
                        map.correction_factor = Some(pattern.correction);
                        map.offset = Some(pattern.offset);
                        map.x_axis_correction = Some(pattern.x_axis_correction);
                        map.y_axis_correction = Some(pattern.y_axis_correction);
                        map.x_axis_offset = Some(pattern.x_axis_offset);
                        map.y_axis_offset = Some(pattern.y_axis_offset);
                        classified_this = true;
                    } else if pattern.name.to_lowercase().contains("launch control") {
                        // Launch control maps - name without address (will be fully classified later)
                        map.name = Some("Launch control map".to_string());
                        map.category = Some("Detected maps".to_string());
                        map.subcategory = Some("2-Limiters".to_string());
                        map.correction_factor = Some(pattern.correction);
                        map.offset = Some(pattern.offset);
                        map.x_axis_correction = Some(pattern.x_axis_correction);
                        map.y_axis_correction = Some(pattern.y_axis_correction);
                        map.x_axis_offset = Some(pattern.x_axis_offset);
                        map.y_axis_offset = Some(pattern.y_axis_offset);
                        classified_this = true;
                    } else {
                        // Generic pattern match - use pattern metadata
                        map.name = Some(format!("{} [0x{:06X}]", pattern.name, map.address));
                        map.category = Some("Detected maps".to_string());
                        map.subcategory = Some(pattern.subcategory.clone());
                        map.correction_factor = Some(pattern.correction);
                        map.offset = Some(pattern.offset);
                        map.x_axis_correction = Some(pattern.x_axis_correction);
                        map.y_axis_correction = Some(pattern.y_axis_correction);
                        map.x_axis_offset = Some(pattern.x_axis_offset);
                        map.y_axis_offset = Some(pattern.y_axis_offset);
                        classified_this = true;
                    }
                }
            }
            
            // Length 700 (25x14) - Launch control map
            // Detect by size only - 700 bytes is unique to Launch control maps
            // Binary structure: [1st_axis_header 4b][1st_axis_data 28b][2nd_axis_header 4b][2nd_axis_data 50b][Map_data 700b]
            // Detection finds: 1st axis = 14 values (vehicle speed), 2nd axis = 25 values (RPM)
            // But WinOLS/EDCSuite labels: X = RPM (25 cols), Y = Vehicle speed (14 rows)
            // So we need to SWAP the axis addresses when we swap the dimensions!
            if map.size == 700 {
                log::debug!("🚀 Launch Control map BEFORE classification: address=0x{:06X}, x_axis=0x{:?}, y_axis=0x{:?}, dims={:?}",
                    map.address, map.x_axis_address, map.y_axis_address, map.dimensions);
                map.category = Some("Detected maps".to_string());
                map.subcategory = Some("2-Limiters".to_string());
                map.name = Some("Launch control map".to_string());

                // Force correct dimensions: 25 columns (RPM) x 14 rows (vehicle speed)
                map.dimensions = MapDimensions::TwoDimensional { rows: 14, cols: 25 };

                // CRITICAL: Swap axis addresses to match the new dimension order!
                // Originally: x_axis_address = 1st axis (14-value vehicle speed data)
                //             y_axis_address = 2nd axis (25-value RPM data)
                // After swap: x_axis_address = 2nd axis (25-value RPM data) -> X = RPM
                //             y_axis_address = 1st axis (14-value vehicle speed data) -> Y = vehicle speed
                let old_x_addr = map.x_axis_address;
                let old_y_addr = map.y_axis_address;
                map.x_axis_address = old_y_addr; // X now points to RPM data (25 values)
                map.y_axis_address = old_x_addr; // Y now points to vehicle speed data (14 values)

                // Correction factors from EDCSuite:
                // X-axis (RPM): factor 1.0, no offset
                // Y-axis (vehicle speed km/h): factor 0.156250, bBackwards=1 (inverted!)
                // Z-axis (IQ limit): factor 0.01
                map.x_axis_correction = Some(1.0);
                map.y_axis_correction = Some(0.156250);
                map.correction_factor = Some(0.01);
                map.y_axis_inverted = Some(true); // bBackwards=1 - Y axis should be displayed inverted

                // Add axis labels and description
                map.x_label = Some("Engine speed (rpm)".to_string());
                map.y_label = Some("Vehicle speed (km/h)".to_string());
                map.unit = Some("mg/st".to_string());
                map.description = Some("IQ limit | X: Engine speed (rpm) | Y: Vehicle speed (km/h)".to_string());

                classified_this = true;
            }
            // Length 570 (19x15) - Injector duration 01-04
            else if map.size == 570 {
                if (x_axis_id_high == 0xEC && y_axis_id_high == 0xC5) ||
                   (x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC) ||
                   (x_axis_id_high == 0xC4 && y_axis_id_high == 0xEA) ||
                   (x_axis_id_high == 0xC4 && y_axis_id_high == 0xEC) {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    // Count 570-byte maps with lower address in this codeblock
                    let maps_before = all_maps.iter()
                        .filter(|m| m.codeblock_id == map.codeblock_id)
                        .filter(|m| m.size == 570)
                        .filter(|m| m.address < map.address)
                        .count();
                    // Numbers 01-04 for 570-byte maps (based on address order)
                    let num = maps_before + 1;
                    map.name = Some(format!("Injector duration {:02}", num));
                    // Swap axes: file has Y first (IQ), then X (RPM), but we want X=RPM, Y=IQ
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    map.x_axis_correction = Some(1.0); // RPM factor
                    map.y_axis_correction = Some(0.01); // IQ factor
                    map.correction_factor = Some(0.023437);
                    classified_this = true;
                }
            }
            // Length 480 (16x15) - Injector duration 01-04
            // File structure: [Y_ID=C5][Y_len=15][Y_data=IQ][X_ID=EC][X_len=16][X_data=RPM][Map]
            // Semantic: X=RPM, Y=IQ - need to swap axes
            else if map.size == 480 {
                if (x_axis_id_high == 0xEC && y_axis_id_high == 0xC5) ||
                   (x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC) {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    // Count 480-byte maps with lower address in this codeblock
                    let maps_before = all_maps.iter()
                        .filter(|m| m.codeblock_id == map.codeblock_id)
                        .filter(|m| m.size == 480)
                        .filter(|m| m.address < map.address)
                        .count();
                    // Numbers 01-04 for 480-byte maps (based on address order)
                    let num = maps_before + 1;
                    map.name = Some(format!("Injector duration {:02}", num));
                    // Swap axes: file has Y first (IQ), then X (RPM), but we want X=RPM, Y=IQ
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    map.x_axis_correction = Some(1.0); // RPM factor
                    map.y_axis_correction = Some(0.01); // IQ factor
                    map.correction_factor = Some(0.023437);
                    classified_this = true;
                }
            }
            // Length 416 (13x16) - Smoke limiter, IQ by MAF limiter 1, N75, EGR
            // NOTE: 416 bytes with 13x16 dimensions and X=Airflow is "IQ by MAF limiter 1"
            else if map.size == 416 {
                // DISABLED: Smoke limiter detection now handled by find_smoke_limiter_maps
                // which properly identifies all 3 temperature variants (-20°C, 40°C, 80°C)
                // Smoke limiter: X=Airflow (ID 0xDA), Y=RPM (ID 0xF9)
                // In file: first axis is Y (0xF9), second is X (0xDA)
                // So detected x_axis_id_high=0xF9 (actually Y), y_axis_id_high=0xDA (actually X)
                // if x_axis_id_high == 0xF9 && y_axis_id_high == 0xDA {
                //     // This is a Smoke limiter detected without pattern match
                //     // The pattern should handle this, but if not, classify here
                //     map.category = Some("Detected maps".to_string());
                //     map.subcategory = Some("2-Limiters".to_string());
                //     map.name = Some(format!("Smoke limiter [0x{:06X}]", map.address));
                //     map.correction_factor = Some(0.01);
                //     map.x_axis_correction = Some(0.1);
                //     classified_this = true;
                // } else
                if x_axis_id_high == 0xEC && y_axis_id_high == 0xDA {
                    // IQ by MAF or IQ by MAP (13x16) - 416 bytes
                    // Both have same axis IDs: Y=RPM (EC), X=Airflow/Boost (DA)
                    // In file: first axis is Y (EC), second is X (DA)
                    // So code's x_axis_id=EC (RPM), y_axis_id=DA (Airflow/Boost)
                    // Distinguish by X axis values (stored in y_axis_address in code):
                    // - IQ by MAF: X axis >= 2500 (Airflow 3000-10500 * 0.1 = 300-1050 mg/st)
                    // - IQ by MAP: X axis < 2500 (Boost pressure 800-2200 mbar)
                    if let Some(y_addr) = map.y_axis_address {
                        if (y_addr as usize) + 2 < data.len() {
                            let first_x_val = u16::from_le_bytes([data[y_addr as usize], data[y_addr as usize + 1]]);
                            
                            if first_x_val >= 2500 {
                                // IQ by MAF limiter - Airflow values (3000+ raw = 300+ mg/st)
                                map.category = Some("Detected maps".to_string());
                                map.subcategory = Some("2-Limiters".to_string());
                                map.name = Some("IQ by MAF limiter".to_string());
                                map.description = Some("Max IQ by airflow | X: Airflow (mg/st) | Y: Engine speed (rpm)".to_string());
                                map.correction_factor = Some(0.01);
                                // Swap axes - code has them inverted
                                let temp_x = map.x_axis_address;
                                map.x_axis_address = map.y_axis_address;
                                map.y_axis_address = temp_x;
                                map.x_axis_correction = Some(0.1);  // Airflow factor
                                map.y_axis_correction = Some(1.0);  // RPM factor
                                log::debug!("✅ IQ by MAF limiter at 0x{:X} (first X val={})", map.address, first_x_val);
                                classified_this = true;
                            } else {
                                // IQ by MAP limiter - Boost pressure values (800-2200 mbar)
                                map.category = Some("Detected maps".to_string());
                                map.subcategory = Some("2-Limiters".to_string());
                                map.name = Some("IQ by MAP limiter".to_string());
                                map.description = Some("Max IQ by boost pressure | X: Boost (mbar) | Y: Engine speed (rpm)".to_string());
                                map.correction_factor = Some(0.01);
                                // Swap axes - code has them inverted
                                let temp_x = map.x_axis_address;
                                map.x_axis_address = map.y_axis_address;
                                map.y_axis_address = temp_x;
                                map.x_axis_correction = Some(1.0);  // Boost factor (already mbar)
                                map.y_axis_correction = Some(1.0);  // RPM factor
                                log::debug!("✅ IQ by MAP limiter at 0x{:X} (first X val={})", map.address, first_x_val);
                                classified_this = true;
                            }
                        }
                    }
                } else if x_axis_id_high == 0xEC && y_axis_id_high == 0xEA {
                    // N75 duty cycle - X axis is IQ (mg/stroke), Y axis is RPM
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("3-Turbo".to_string());
                    map.name = Some(format!("N75 duty cycle [0x{:06X}]", map.address));
                    map.description = Some("Duty cycle % | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                    map.unit = Some("%".to_string());
                    map.correction_factor = Some(-0.01); // Z = value * -0.01 + 100
                    map.offset = Some(100.0);
                    map.x_axis_correction = Some(0.01); // X axis: IQ needs *0.01 to get mg/stroke
                    map.y_axis_correction = Some(1.0);  // Y axis: RPM as-is
                    classified_this = true;
                } else if (x_axis_id_high == 0xEC && (y_axis_id_high == 0xC0 || y_axis_id_high == 0xE9)) ||
                           ((y_axis_id_high == 0xEC) && (x_axis_id_high == 0xC0 || x_axis_id_high == 0xE9)) {
                    // EGR setpoint (13x16) - allow swapped axis order and RPM ID variant 0xE9
                    let has_expected_dims = (x_len == 13 && y_len == 16) || (x_len == 16 && y_len == 13);
                    if has_expected_dims {
                        if let Some(map_codeblock) = if map.address >= 0x4C000 && map.address < 0x5C000 {
                            Some(1)
                        } else if map.address >= 0x5C000 && map.address < 0x6C000 {
                            Some(2)
                        } else if map.address >= 0x6C000 && map.address < 0x7C000 {
                            Some(3)
                        } else {
                            None
                        } {
                            if !egr_codeblocks.contains(&map_codeblock) {
                                // Swap axes so that X = IQ (mg/st), Y = RPM
                                std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);

                                map.category = Some("Detected maps".to_string());
                                map.subcategory = Some("4-Misc".to_string());
                                map.name = Some("EGR".to_string());
                                map.unit = Some("mg/st".to_string());

                                map.correction_factor = Some(0.1);  // EGR values need 0.1 factor
                                map.offset = Some(0.0);
                                map.x_axis_correction = Some(0.01); // IQ
                                map.x_axis_offset = Some(0.0);
                                map.y_axis_correction = Some(1.0);  // RPM
                                map.y_axis_offset = Some(0.0);
                                
                                let x_addr = map.x_axis_address.unwrap_or(0);
                                let y_addr = map.y_axis_address.unwrap_or(0);
                                map.description = Some(format!(
                                    "EGR setpoint (mg/st) | X: IQ (mg/st) | Y: Engine speed (rpm) | Axis IDs: X=0x{:04X} Y=0x{:04X}",
                                    x_addr, y_addr
                                ));
                                egr_codeblocks.insert(map_codeblock);
                                log::debug!("🎯 Classified EGR at 0x{:X} in codeblock {}", map.address, map_codeblock);
                                classified_this = true;
                            } else {
                                log::debug!("⏭️  Skipping EGR at 0x{:X} - already have EGR in codeblock {}", map.address, map_codeblock);
                            }
                        }
                    }
                }
            }
            // Length 192 - Driver wish: EC + C0 axes in either order
            // 192 bytes = 96 values
            else if map.size == 192 {
                let is_driver_wish = (x_axis_id_high == 0xEC && y_axis_id_high == 0xC0) ||
                                     (x_axis_id_high == 0xC0 && y_axis_id_high == 0xEC);
                if is_driver_wish {
                    let (larger_len, smaller_len) = if x_len >= y_len {
                        (x_len, y_len)
                    } else {
                        (y_len, x_len)
                    };
                    let orig_x = map.x_axis_address;
                    let orig_y = map.y_axis_address;
                    map.x_axis_address = orig_y;
                    map.y_axis_address = orig_x;

                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("4-Misc".to_string());
                    map.name = Some("Driver wish".to_string());
                    // WinOLS: bSigned=1 — les cellules frein moteur portent des
                    // IQ négatifs (raw 0xFFxx), sinon affichés ~654.5 au lieu de -0.9
                    map.data_type = DataType::Int16;
                    map.description = Some(format!(
                        "Requested IQ (mg/st) | X: Throttle position (%) | Y: Engine speed (rpm)"
                    ));
                    map.unit = Some("mg/st".to_string());
                    map.correction_factor = Some(0.01);
                    map.x_axis_correction = Some(0.01);
                    map.y_axis_correction = Some(1.0);
                    map.dimensions = MapDimensions::TwoDimensional { rows: larger_len, cols: smaller_len };
                    classified_this = true;
                }
            }
            // Length 180 (10x9) - Start IQ, Boost limit, Injector duration
            else if map.size == 180 {
                // Start IQ: 10 cols (Temp) x 9 rows (RPM)
                // WinOLS display: X = Coolant temp (°C), Y = Engine speed (RPM)
                // Axis IDs: X=0xC1 (Temp), Y=0xEC (RPM)
                let is_start_iq_normal = x_len == 10 && y_len == 9 && x_axis_id_high == 0xC1 && y_axis_id_high == 0xEC;
                let is_start_iq_swapped = x_len == 9 && y_len == 10 && x_axis_id_high == 0xEC && y_axis_id_high == 0xC1;

                if is_start_iq_normal || is_start_iq_swapped {
                    // Start IQ - force correct dimensions for display
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("4-Misc".to_string());
                    map.name = Some("Start IQ".to_string());
                    map.correction_factor = Some(0.01);
                    map.unit = Some("mg/st".to_string());
                    map.description = Some("Start IQ (mg/st) | X: Coolant temp (°C) | Y: Engine speed (rpm)".to_string());
                    map.x_label = Some("degC".to_string());
                    map.y_label = Some("rpm".to_string());

                    // If axes are swapped in file, swap addresses to correct order
                    if is_start_iq_swapped {
                        std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                        log::debug!("🔄 Start IQ: Swapped axis addresses to correct order (X=Temp, Y=RPM)");
                    }

                    // X = Temp (correction 0.1, offset -273.1), Y = RPM (correction 1.0)
                    map.x_axis_correction = Some(0.1);
                    map.x_axis_offset = Some(-273.1);
                    map.y_axis_correction = Some(1.0);
                    map.y_axis_offset = Some(0.0);
                    // Force dimensions: 10 cols (Temp) x 9 rows (RPM)
                    map.dimensions = MapDimensions::TwoDimensional { rows: 9, cols: 10 };
                    classified_this = true;
                } else if x_len == 9 && y_len == 10 {
                    if x_axis_id_high == 0xEC && y_axis_id_high == 0xC0 {
                        // Boost limit map - X=RPM (0xEC), Y=Atm pressure (0xC0)
                        map.category = Some("Detected maps".to_string());
                        map.subcategory = Some("2-Limiters".to_string());
                        map.name = Some("Boost limit map".to_string());
                        classified_this = true;
                    } else if (x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC) ||
                              (x_axis_id_high == 0xC4 && y_axis_id_high == 0xEA) {
                        // Injector duration
                        map.category = Some("Detected maps".to_string());
                        map.subcategory = Some("1-Fuel".to_string());
                        let inj_dur_count = all_maps.iter()
                            .filter(|m| m.codeblock_id == map.codeblock_id)
                            .filter(|m| m.name.as_ref().map_or(false, |n| n.starts_with("Injector duration")))
                            .count() + 1;
                        map.name = Some(format!("Injector duration {:02} [0x{:06X}]", inj_dur_count, map.address));
                        map.y_axis_correction = Some(0.01);
                        map.correction_factor = Some(0.023437);
                        classified_this = true;
                    }
                }
            }
            // Length 150 (3x25) - Torque limiter
            else if map.size == 150 && x_len == 3 && y_len == 25 {
                map.category = Some("Detected maps".to_string());
                map.subcategory = Some("2-Limiters".to_string());
                map.name = Some("Torque limiter".to_string());
                map.correction_factor = Some(0.01);
                classified_this = true;
            }
            // Length 126 (3x21) - Torque limiter
            else if map.size == 126 && x_len == 3 && y_len == 21 {
                map.category = Some("Detected maps".to_string());
                map.subcategory = Some("2-Limiters".to_string());
                map.name = Some("Torque limiter".to_string());
                map.correction_factor = Some(0.01);
                classified_this = true;
            }
            // Length 120 (3x20) - Torque limiter
            else if map.size == 120 && x_len == 3 && y_len == 20 {
                map.category = Some("Detected maps".to_string());
                map.subcategory = Some("2-Limiters".to_string());
                map.name = Some("Torque limiter".to_string());
                map.correction_factor = Some(0.01);
                classified_this = true;
            }
            // Length 132 (3x22) - Torque limiter
            else if map.size == 132 && x_len == 3 && y_len == 22 {
                map.category = Some("Detected maps".to_string());
                map.subcategory = Some("2-Limiters".to_string());
                map.name = Some("Torque limiter".to_string());
                map.correction_factor = Some(0.01);
                classified_this = true;
            }
            // Length 138 (3x23) - Torque limiter
            else if map.size == 138 && x_len == 3 && y_len == 23 {
                map.category = Some("Detected maps".to_string());
                map.subcategory = Some("2-Limiters".to_string());
                map.name = Some("Torque limiter".to_string());
                map.correction_factor = Some(0.01);
                classified_this = true;
            }
            // Length 200 (10x10) - Injector duration 00, Boost limit, Limit of overboost protection
            else if map.size == 200 && x_len == 10 && y_len == 10 {
                // Injector duration 00: File has [Y=IQ(C5)][X=RPM(EC)], semantic X=RPM, Y=IQ
                if (x_axis_id_high == 0xEC && y_axis_id_high == 0xC5) ||
                   (x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC) ||
                   (x_axis_id_high == 0xC4 && y_axis_id_high == 0xEA) ||
                   (x_axis_id_high == 0xC4 && y_axis_id_high == 0xEC) {
                    // Injector duration 00 - always "00" for this size
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    map.name = Some("Injector duration 00".to_string());
                    // Swap axes: file has Y first (IQ), then X (RPM)
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    map.x_axis_correction = Some(1.0); // RPM factor
                    map.y_axis_correction = Some(0.01); // IQ factor
                    map.correction_factor = Some(0.023437);
                    classified_this = true;
                } else if (x_axis_id_high == 0xEC || x_axis_id_high == 0xEA) && y_axis_id_high == 0xC0 {
                    // Boost limit map - X=RPM (0xEC/0xEA), Y=Atm pressure (0xC0)
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("2-Limiters".to_string());
                    map.name = Some("Boost limit map".to_string());
                    map.correction_factor = Some(1.0);
                    classified_this = true;
                } else if x_axis_id_high == 0xEC && y_axis_id_high == 0xC0 {
                    // Limit of overboost protection
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("2-Limiters".to_string());
                    map.name = Some(format!("Limit of overboost protection [0x{:06X}]", map.address));
                    map.correction_factor = Some(1.0);
                    map.x_axis_correction = Some(0.01);
                    classified_this = true;
                }
            }
            // Length 256 - Driver wish: EC + C0 axes in either order
            // 256 bytes = 128 values = X_len * Y_len * 2
            // Display convention: larger axis as columns, smaller axis as rows
            else if map.size == 256 {
                let is_driver_wish = (x_axis_id_high == 0xEC && y_axis_id_high == 0xC0) ||
                                     (x_axis_id_high == 0xC0 && y_axis_id_high == 0xEC);
                if is_driver_wish {
                    let (larger_len, smaller_len) = if x_len >= y_len {
                        (x_len, y_len)
                    } else {
                        (y_len, x_len)
                    };
                    // Swap axis addresses: put RPM on X (cols/top) and TPS% on Y (rows/left)
                    let orig_x = map.x_axis_address;
                    let orig_y = map.y_axis_address;
                    map.x_axis_address = orig_y;
                    map.y_axis_address = orig_x;

                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("4-Misc".to_string());
                    map.name = Some("Driver wish".to_string());
                    // WinOLS: bSigned=1 — IQ négatifs (frein moteur) en 0xFFxx
                    map.data_type = DataType::Int16;
                    map.description = Some(format!(
                        "Requested IQ (mg/st) | X: Throttle position (%) | Y: Engine speed (rpm)"
                    ));
                    map.unit = Some("mg/st".to_string());
                    map.correction_factor = Some(0.01);
                    map.x_axis_correction = Some(0.01);
                    map.y_axis_correction = Some(1.0);
                    map.dimensions = MapDimensions::TwoDimensional { rows: larger_len, cols: smaller_len };
                    classified_this = true;
                }
            }
            // Length 320 - Boost target map, Boost correction by temperature (C# line 1634)
            // Vérité fichier (validée octets + WinOLS 19ORIpolo) : l'axe court
            // (10 valeurs, ID 0xC0) = IQ ×0.01, l'axe long (16 valeurs, ID
            // 0xEC/0xEA) = RPM ×1.0 ; data = 16 lignes (RPM) × 10 colonnes (IQ).
            // L'affichage attend X=IQ (colonnes) / Y=RPM (lignes) : on échange
            // les adresses si X pointe l'axe RPM. Décision par LONGUEURS (les
            // IDs varient EC/EA selon les variantes et l'ordre fichier change).
            else if map.size == 320 {
                let is_boost_ids =
                    ((x_axis_id_high == 0xEC || x_axis_id_high == 0xEA) && y_axis_id_high == 0xC0)
                    || (x_axis_id_high == 0xC0 && (y_axis_id_high == 0xEC || y_axis_id_high == 0xEA));
                if is_boost_ids {
                    // ATTENTION : x_len/y_len viennent des dims (cols/rows), pas
                    // des adresses. La longueur RÉELLE de l'axe pointé par chaque
                    // adresse se lit dans le fichier : [ID u16][len u16 LE][valeurs]
                    // → len à adresse-2 (même convention que les IDs à adresse-4).
                    let axis_len_at = |addr: Option<u32>| -> Option<usize> {
                        let a = addr? as usize;
                        if a >= 2 && a <= data.len() {
                            Some(u16::from_le_bytes([data[a - 2], data[a - 1]]) as usize)
                        } else {
                            None
                        }
                    };
                    let x_file_len = axis_len_at(map.x_axis_address);
                    let y_file_len = axis_len_at(map.y_axis_address);
                    let x_is_rpm = match (x_file_len, y_file_len) {
                        (Some(xl), Some(yl)) if xl != yl && xl * yl * 2 == map.size => xl > yl,
                        _ => x_axis_id_high == 0xEC || x_axis_id_high == 0xEA,
                    };
                    if x_is_rpm {
                        std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                        log::debug!("🔄 Boost target 320: swapped axis addresses (X was RPM axis)");
                    }
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("3-Turbo".to_string());
                    map.name = Some("Boost target map".to_string());
                    map.description = Some(format!(
                        "Target boost pressure (mbar) | X: IQ (mg/st) | Y: Engine speed (rpm) | Axis IDs: X=0x{:04X} Y=0x{:04X}",
                        map.x_axis_address.unwrap_or(0), map.y_axis_address.unwrap_or(0)
                    ));
                    map.unit = Some("mbar".to_string());
                    map.correction_factor = Some(1.0);
                    map.x_axis_correction = Some(0.01); // X is IQ: *0.01
                    map.y_axis_correction = Some(1.0);  // Y is RPM
                    map.x_label = Some("mg/st".to_string());
                    map.y_label = Some("rpm".to_string());
                    map.y_axis_inverted = Some(true);
                    // Dimensions en orientation fichier : lignes = RPM, colonnes = IQ
                    if let (Some(xl), Some(yl)) = (x_file_len, y_file_len) {
                        let (iq_len, rpm_len) = if x_is_rpm { (yl, xl) } else { (xl, yl) };
                        if iq_len != rpm_len && iq_len * rpm_len * 2 == map.size {
                            map.dimensions = MapDimensions::TwoDimensional { rows: rpm_len, cols: iq_len };
                        }
                    }
                    classified_this = true;
                } else if x_axis_id_high == 0xEC && y_axis_id_high == 0xEA {
                    // Boost correction by temperature
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("2-Limiters".to_string());
                    map.name = Some(format!("Boost correction by temperature [0x{:06X}]", map.address));
                    map.correction_factor = Some(1.0);
                    map.x_axis_correction = Some(0.1);
                    map.x_axis_offset = Some(-273.1);
                    classified_this = true;
                } else if x_axis_id_high == 0xEC && y_axis_id_high == 0xDA {
                    // IQ by MAP limiter (C# line 1661)
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("2-Limiters".to_string());
                    map.name = Some(format!("IQ by MAP limiter [0x{:06X}]", map.address));
                    map.description = Some("Maximum IQ (mg) | X: Boost pressure (mbar) | Y: Engine speed (rpm)".to_string());
                    map.correction_factor = Some(0.01);
                    classified_this = true;
                }
            }
            // Length 360 (15x12) - Injector duration
            else if map.size == 360 && x_len == 15 && y_len == 12 {
                if (x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC) ||
                   (x_axis_id_high == 0xC4 && y_axis_id_high == 0xEA) {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    let inj_dur_count = all_maps.iter()
                        .filter(|m| m.codeblock_id == map.codeblock_id)
                        .filter(|m| m.name.as_ref().map_or(false, |n| n.starts_with("Injector duration")))
                        .count() + 1;
                    map.name = Some(format!("Injector duration {:02} [0x{:06X}]", inj_dur_count, map.address));
                    map.y_axis_correction = Some(0.01);
                    map.correction_factor = Some(0.023437);
                    classified_this = true;
                }
            }
            // Length 390 (15x13) - Injector duration
            else if map.size == 390 && x_len == 15 && y_len == 13 {
                if x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    let inj_dur_count = all_maps.iter()
                        .filter(|m| m.codeblock_id == map.codeblock_id)
                        .filter(|m| m.name.as_ref().map_or(false, |n| n.starts_with("Injector duration")))
                        .count() + 1;
                    map.name = Some(format!("Injector duration {:02} [0x{:06X}]", inj_dur_count, map.address));
                    map.y_axis_correction = Some(0.01);
                    map.correction_factor = Some(0.023437);
                    classified_this = true;
                }
            }
            // Length 570 (15x19 or 19x15) - Injector duration 01-04 (C# line 1047)
            // Note: Primary classification is in earlier if-else branch
            else if map.size == 570 {
                if (x_axis_id_high == 0xEC && y_axis_id_high == 0xC5) ||
                   (x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC) ||
                   (x_axis_id_high == 0xC4 && y_axis_id_high == 0xEA) ||
                   (x_axis_id_high == 0xC4 && y_axis_id_high == 0xEC) {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    // Count 570-byte maps with lower address in this codeblock
                    let maps_before = all_maps.iter()
                        .filter(|m| m.codeblock_id == map.codeblock_id)
                        .filter(|m| m.size == 570)
                        .filter(|m| m.address < map.address)
                        .count();
                    let num = maps_before + 1;
                    map.name = Some(format!("Injector duration {:02}", num));
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    map.x_axis_correction = Some(1.0);
                    map.y_axis_correction = Some(0.01);
                    map.correction_factor = Some(0.023437);
                    classified_this = true;
                }
            }
            // Length 480 (15x16 or 16x15) - Injector duration 01-04 (C# line 1109)
            // File structure: [Y_ID=C5][Y_len=15][Y_data=IQ][X_ID=EC][X_len=16][X_data=RPM][Map]
            else if map.size == 480 {
                if (x_axis_id_high == 0xEC && y_axis_id_high == 0xC5) ||
                   (x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC) ||
                   (x_axis_id_high == 0xC4 && y_axis_id_high == 0xEA) {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    // Count 480-byte maps with lower address in this codeblock
                    let maps_before = all_maps.iter()
                        .filter(|m| m.codeblock_id == map.codeblock_id)
                        .filter(|m| m.size == 480)
                        .filter(|m| m.address < map.address)
                        .count();
                    // Numbers 01-04 for 480-byte maps
                    let num = maps_before + 1;
                    map.name = Some(format!("Injector duration {:02}", num));
                    // Swap axes: file has Y first (IQ), then X (RPM)
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    map.x_axis_correction = Some(1.0); // RPM factor
                    map.y_axis_correction = Some(0.01); // IQ factor
                    map.correction_factor = Some(0.023437);
                    classified_this = true;
                }
            }
            // Length 280 (10x14 or 14x10) - Boost target map (C# line 1723)
            // Symbol display: 14 rows (RPM) x 10 cols (IQ)
            // File structure: [EC axis, 14 RPM values][C0 axis, 10 IQ values][280 bytes map]
            // EC (0xEC/0xEA) = RPM axis (14 values: 1000-4750)
            // C0 (0xC0) = IQ axis (10 values: 0-45 mg/st)
            // We want: X=IQ (cols), Y=RPM (rows)
            else if map.size == 280 {
                log::debug!("Processing size 280 map at 0x{:06X}: name={:?}, x_id_high=0x{:02X}, y_id_high=0x{:02X}, dims={:?}",
                    map.address, map.name, x_axis_id_high, y_axis_id_high, map.dimensions);
                let x_addr = map.x_axis_address.unwrap_or(0);
                let y_addr = map.y_axis_address.unwrap_or(0);

                // Check if this is a Boost target map by name OR by typical axis IDs
                let is_boost_target_by_name = map.name.as_ref().map_or(false, |n| n.contains("Boost target"));
                let is_boost_target_by_axis = ((x_axis_id_high == 0xEC || x_axis_id_high == 0xEA) && y_axis_id_high == 0xC0) ||
                                               (x_axis_id_high == 0xC0 && (y_axis_id_high == 0xEC || y_axis_id_high == 0xEA));

                // For Boost target maps: ALWAYS ensure X=IQ(0x5690E), Y=RPM(0x568EE)
                // Symbol expects these exact addresses
                if is_boost_target_by_name || is_boost_target_by_axis {
                    // Check current addresses - if X points to RPM data (lower address), swap
                    // RPM axis is at 0x568EE, IQ axis is at 0x5690E
                    // If x_addr < y_addr, then X is pointing to RPM (wrong), need swap
                    if x_addr < y_addr {
                        log::debug!("Boost target 280: SWAPPING X=0x{:X}<->Y=0x{:X} (X was RPM, should be IQ)", x_addr, y_addr);
                        std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    }
                    let final_x = map.x_axis_address.unwrap_or(0);
                    let final_y = map.y_axis_address.unwrap_or(0);
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("3-Turbo".to_string());
                    map.name = Some("Boost target map".to_string());
                    map.description = Some(format!(
                        "Target boost pressure (mbar) | X: IQ (mg/st) | Y: Engine speed (rpm) | Axis: X=0x{:04X}(IQ) Y=0x{:04X}(RPM)",
                        final_x, final_y
                    ));
                    map.unit = Some("mbar".to_string());
                    map.correction_factor = Some(1.0);
                    map.x_axis_correction = Some(0.01);
                    map.y_axis_correction = Some(1.0);
                    map.x_label = Some("mg/st".to_string());
                    map.y_label = Some("rpm".to_string());
                    map.y_axis_inverted = Some(true);
                    map.dimensions = MapDimensions::TwoDimensional { rows: 14, cols: 10 };
                    classified_this = true;
                }
            }
            // Length 448 (14x16 or 16x14) - SOI maps with different temperatures
            // CRITICAL: SOI maps are detected by MapSelector.NumRepeats == 10 in C#
            // Also accept maps with compatible axis IDs even without MapSelector
            else if map.size == 448 && ((x_len == 14 && y_len == 16) || (x_len == 16 && y_len == 14)) {
                // SKIP if map already has temperature in name (from detect_soi_maps_by_selector)
                let already_has_temp = map.name.as_ref().map_or(false, |n| n.contains("°C"));
                if already_has_temp {
                    // Keep the temperature-based name, just ensure metadata is set
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    map.correction_factor = Some(-0.023437);
                    map.offset = Some(78.0);
                    map.x_axis_correction = Some(0.01);
                    classified_this = true;
                } else {
                // Check if this map has a MapSelector with 10 repeats (like in C# line 1155)
                let is_soi_by_selector = map.map_selector.as_ref().map_or(false, |ms| ms.num_repeats == 10);
                
                // Check axis IDs for SOI pattern
                let is_soi_by_axis_ids = 
                    (x_axis_id_high == 0xEA && (y_axis_id_high == 0xC0 || y_axis_id_high == 0xE9)) ||
                    (y_axis_id_high == 0xEA && (x_axis_id_high == 0xC0 || x_axis_id_high == 0xE9));
                
                if is_soi_by_selector || is_soi_by_axis_ids {
                        // SOI maps detected - just number them
                    let soi_count = all_maps.iter()
                        .filter(|m| m.codeblock_id == map.codeblock_id)
                        .filter(|m| m.name.as_ref().map_or(false, |n| n.contains("Start of injection (SOI)")))
                            .count() + 1;
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                        map.name = Some(format!("Start of injection (SOI) ({})", soi_count));
                    map.correction_factor = Some(-0.023437);
                    map.offset = Some(78.0);
                    map.x_axis_correction = Some(0.01);
                    classified_this = true;
                    }
                }
            }
            // Length 162 (9x9) - Start IQ (1) and (2)
            // WinOLS display: X = Coolant temp (°C), Y = Engine speed (RPM)
            // Axis IDs: X=0xC1 (Temp), Y=0xEC (RPM) or swapped
            else if map.size == 162 && x_len == 9 && y_len == 9 {
                let is_start_iq_normal = x_axis_id_high == 0xC1 && y_axis_id_high == 0xEC;
                let is_start_iq_swapped = x_axis_id_high == 0xEC && y_axis_id_high == 0xC1;

                if is_start_iq_normal || is_start_iq_swapped {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("4-Misc".to_string());
                    map.name = Some("Start IQ".to_string());
                    map.correction_factor = Some(0.01);
                    map.unit = Some("mg/st".to_string());
                    map.description = Some("Start IQ (mg/st) | X: Coolant temp (°C) | Y: Engine speed (rpm)".to_string());
                    map.x_label = Some("degC".to_string());
                    map.y_label = Some("rpm".to_string());

                    // If axes are swapped in file, swap addresses to correct order
                    if is_start_iq_swapped {
                        std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                        log::debug!("🔄 Start IQ (162): Swapped axis addresses to correct order (X=Temp, Y=RPM)");
                    }

                    // X = Temp (correction 0.1, offset -273.1), Y = RPM (correction 1.0)
                    map.x_axis_correction = Some(0.1);
                    map.x_axis_offset = Some(-273.1);
                    map.y_axis_correction = Some(1.0);
                    map.y_axis_offset = Some(0.0);
                    // Force dimensions: 9 cols (Temp) x 9 rows (RPM)
                    map.dimensions = MapDimensions::TwoDimensional { rows: 9, cols: 9 };
                    classified_this = true;
                }
            }
            // Length 198 (11x9) - Injector duration 05 (always 05 for this size)
            // File structure: [Y_ID=C5][Y_len=9][Y_data=IQ][X_ID=EC][X_len=11][X_data=RPM][Map]
            else if map.size == 198 && x_len == 11 && y_len == 9 {
                if (x_axis_id_high == 0xEC && y_axis_id_high == 0xC5) ||
                   (x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC) {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    map.name = Some("Injector duration 05".to_string());
                    // Swap axes: file has Y first (IQ), then X (RPM)
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    map.x_axis_correction = Some(1.0); // RPM factor
                    map.y_axis_correction = Some(0.01); // IQ factor
                    map.correction_factor = Some(0.023437);
                    classified_this = true;
                }
            }
            // Length 220 - DISABLED: Measurement Blocks detection removed as per requirements
            // else if map.size == 220 && ((x_len == 11 && y_len == 10) || (x_len == 10 && y_len == 11)) { ... }
            // Length 220 (10x11) - Injector duration 05, variante 6L AXR
            // ([C5C0][10][IQ 0..3000][EC38][11][RPM 0..2500] à 0x551AC) —
            // le gate sur les IDs C5/EC évite les Measurement Blocks.
            else if map.size == 220
                && ((x_len == 11 && y_len == 10) || (x_len == 10 && y_len == 11))
                && ((x_axis_id_high == 0xEC && y_axis_id_high == 0xC5)
                    || (x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC))
            {
                map.category = Some("Detected maps".to_string());
                map.subcategory = Some("1-Fuel".to_string());
                map.name = Some("Injector duration 05".to_string());
                // Fichier : premier axe = IQ (C5), second = RPM (EC) → swap
                // pour présenter le RPM en X (même convention que le 198)
                if y_axis_id_high == 0xC5 {
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                }
                map.x_axis_correction = Some(1.0); // RPM factor
                map.y_axis_correction = Some(0.01); // IQ factor
                map.correction_factor = Some(0.023437);
                map.data_type = DataType::Int16;
                classified_this = true;
            }
            // Length 32 (1x16 or 16x1) - PID maps, Boost actuator upper limit curve
            // 2D maps: x_len=1, y_len=16
            // 1D maps: x_len=16, y_len=1
            else if map.size == 32 && ((x_len == 1 && y_len == 16) || (x_len == 16 && y_len == 1)) {
                // For 1D maps (16x1), check x_axis_id; for 2D maps (1x16), check y_axis_id
                let check_id_high = if x_len == 16 { x_axis_id_high } else { y_axis_id_high };
                
                if check_id_high == 0xEC {
                    // Boost actuator upper limit curve (N75) - axis is RPM (0xEC)
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("3-Turbo".to_string());
                    map.name = Some("Boost actuator upper limit curve (N75)".to_string());
                    map.description = Some("Actuator limit (%) | Axis: Engine speed (rpm)".to_string());
                    map.correction_factor = Some(0.01);
                    if x_len == 16 {
                        map.x_axis_correction = Some(1.0);
                    } else {
                        map.y_axis_correction = Some(1.0);
                    }
                    classified_this = true;
                }
            }
            // Length 4 (2x1) - MAP linearization, Idle RPM (1D maps)
            // For 1D maps: dimensions are TwoDimensional { rows: 1, cols: axis_len }
            // So x_len = cols = 2, y_len = rows = 1
            else if map.size == 4 && x_len == 2 && y_len == 1 {
                // Idle RPM: X axis ID high byte is 0xC1 (Temperature)
                if x_axis_id_high == 0xC1 {
                    let idle_count = all_maps.iter()
                        .filter(|m| m.codeblock_id == map.codeblock_id)
                        .filter(|m| m.name.as_ref().map_or(false, |n| n.starts_with("Idle RPM")))
                        .count() + 1;
                    // Limit to 2 per codeblock
                    if idle_count <= 2 {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("4-Misc".to_string());
                        map.name = Some(format!("Idle RPM ({})", idle_count));
                        map.description = Some("Target engine speed | X: Coolant temperature (°C)".to_string());
                    map.correction_factor = Some(1.0);
                        map.x_axis_correction = Some(0.1);
                        map.x_axis_offset = Some(-273.1);
                    classified_this = true;
                    }
                }
                // MAP linearization: X axis ID in [0xEBA2, 0xEBA4, 0xE9BC]
                else if x_axis_id == 0xEBA2 || x_axis_id == 0xEBA4 || x_axis_id == 0xE9BC {
                    map.category = Some("MAP sensor".to_string());
                    map.subcategory = Some("MAP sensor".to_string());
                    map.name = Some("MAP linearization".to_string());
                    map.correction_factor = Some(1.0);
                    map.x_label = Some("".to_string());
                    map.y_label = Some("".to_string());
                    classified_this = true;
                }
            }
            // Length 12 (1x6) - Selector for injector duration
            // Values are indices: 0, 256, 512, 768, 1024, 1280 (index * 256)
            else if map.size == 12 && x_len == 1 && y_len == 6 {
                if y_axis_id_high == 0xEC || y_axis_id_high == 0xEA {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    map.name = Some("Selector for injector duration".to_string());
                    map.correction_factor = Some(0.003906);
                    map.y_axis_correction = Some(-0.023437);
                    map.y_axis_offset = Some(78.0);
                    classified_this = true;
                }
            }
            // Length 2 (1x1) - Single value maps
            // SVBL and SVRL are detected separately with specific addresses (find_svbl_sequence, find_svrl_sequence)
            // Don't classify generic 1x1 maps here to avoid false positives
            else if map.size == 2 && x_len == 1 && y_len == 1 {
                // Only classify if it's at a known switch address pattern
                // Otherwise leave unclassified (will be filtered out)
                // SVBL/SVRL have their own dedicated detection functions
            }
            // Length 60 (6x5 or 5x6) - EGR temperature map (tighter validation)
            else if map.size == 60 {
                log::debug!("🔍 Size 60 map at 0x{:X}: x_len={}, y_len={}", map.address, x_len, y_len);
                if (x_len == 6 && y_len == 5) || (x_len == 5 && y_len == 6) {
                    // Require: one axis temperature (0xC1**) AND the other in typical mbar family (0xEC/0xEA/0xDA/0xC0)
                    let is_temp_x = x_axis_id_high == 0xC1;
                    let is_temp_y = y_axis_id_high == 0xC1;
                    let is_mbar_x = matches!(x_axis_id_high, 0xEC | 0xEA | 0xDA | 0xC0);
                    let is_mbar_y = matches!(y_axis_id_high, 0xEC | 0xEA | 0xDA | 0xC0);
                    let valid_axes = (is_temp_x && is_mbar_y) || (is_temp_y && is_mbar_x);

                    if valid_axes {
                        log::debug!("✅ Classified EGR temperature map at 0x{:X}", map.address);
                        map.category = Some("Detected maps".to_string());
                        map.subcategory = Some("4-Misc".to_string());
                        map.name = Some("EGR temperature map".to_string());
                        map.x_axis_correction = Some(0.1);
                        map.x_axis_offset = Some(-273.1);
                        classified_this = true;
                    } else {
                        log::debug!(
                            "⏭️ Skipping EGR temperature map at 0x{:X}: axes not temp+mbar (x_high=0x{:02X}, y_high=0x{:02X})",
                            map.address, x_axis_id_high, y_axis_id_high
                        );
                    }
                }
            }
            // Length 128 (8x8) - MAF correction by temp, Expected fuel temperature
            else if map.size == 128 && x_len == 8 && y_len == 8 {
                // EDCSuite logic: X_axis_ID/256 == 0xEC && Y_axis_ID/256 == 0xC1
                // In file: X=EC(RPM), Y=C1(IAT) but display as X=IAT, Y=RPM (swapped)
                let is_maf_corr = x_axis_id_high == 0xEC && y_axis_id_high == 0xC1;

                // Validate map data is not all zeros (some files have different structure)
                let map_start = map.address as usize;
                let map_end = map_start + map.size;
                let has_valid_data = if map_end <= data.len() {
                    data[map_start..map_end].iter().any(|&b| b != 0)
                } else {
                    false
                };

                // Validate temperature axis (Y axis = C1 = IAT in file structure)
                // Temperature values should be in Kelvin*10 range (about 1930 to 4730 for -80°C to 200°C)
                // Raw values below 1000 are likely RPM values, not temperatures
                let has_valid_temp_axis = if let Some(y_addr) = map.y_axis_address {
                    let y_start = y_addr as usize;
                    if y_start + y_len * 2 <= data.len() {
                        // Check if temperature values are reasonable:
                        // 1. Raw values should be >= 1930 (corresponds to -80°C: (1930*0.1)-273.1=-80.1)
                        // 2. Raw values should be <= 4730 (corresponds to 200°C: (4730*0.1)-273.1=199.9)
                        // 3. Values should not start at 0 (that would be -273°C, impossible)
                        let first_val = u16::from_le_bytes([data[y_start], data[y_start + 1]]);
                        let is_not_rpm_axis = first_val >= 1500; // Temp raw values start around 1930+

                        is_not_rpm_axis && (0..y_len).all(|i| {
                            let val = u16::from_le_bytes([data[y_start + i*2], data[y_start + i*2 + 1]]);
                            let temp = (val as f64 * 0.1) - 273.1;
                            temp >= -80.0 && temp <= 200.0
                        })
                    } else {
                        false
                    }
                } else {
                    false
                };

                if is_maf_corr && has_valid_data && has_valid_temp_axis {
                    // MAF correction by temperature - Limit value
                    // EDCSuite: X_axis_descr = "Intake air temperature", Y_axis_descr = "Engine speed (rpm)"
                    // So we swap: file X(EC/RPM)->display Y, file Y(C1/IAT)->display X
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("2-Limiters".to_string());
                    map.name = Some("MAF correction by temperature".to_string());
                    map.data_type = DataType::Int16; // Signed values
                    map.correction_factor = Some(0.01);
                    map.description = Some("Limit | X: IAT (°C) | Y: Engine speed (rpm)".to_string());

                    // Swap axes for correct display (file X->display Y, file Y->display X)
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    // After swap: X=IAT(C1), Y=RPM(EC)
                    map.x_axis_correction = Some(0.1);
                    map.x_axis_offset = Some(-273.1);
                    map.y_axis_correction = Some(1.0);
                    map.x_label = Some("°C".to_string());
                    map.y_label = Some("rpm".to_string());
                    classified_this = true;
                } else if x_axis_id_high == 0xEC && y_axis_id_high == 0xC0 {
                    // Expected fuel temperature
                    // File structure: X=EC(RPM,8), Y=C0(IQ,8)
                    // Display: X=IQ (mg/st), Y=RPM
                    // Map values: raw * 0.1 - 273 = °C (Int16 signed)
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("4-Misc".to_string());
                    map.name = Some("Expected fuel temperature".to_string());
                    map.data_type = DataType::Int16; // Signed values
                    map.correction_factor = Some(0.1);
                    map.offset = Some(-273.0); // Temperature offset for map data
                    map.unit = Some("°C".to_string());

                    // Swap axes: file X(EC/RPM)->display Y, file Y(C0/IQ)->display X
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    // After swap: X=IQ(C0), Y=RPM(EC)
                    map.x_axis_correction = Some(0.01); // IQ factor
                    map.y_axis_correction = Some(1.0);  // RPM factor
                    map.description = Some("Fuel temp | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                    map.x_label = Some("mg/st".to_string());
                    map.y_label = Some("rpm".to_string());
                    classified_this = true;
                }
            }
            // Length 144 (8x9 or 9x8) - Start IQ, Fuel volume correction
            else if map.size == 144 {
                // Start IQ: 8 cols (Temp) x 9 rows (RPM) or various swapped configurations
                // WinOLS display: X = Coolant temp (°C), Y = Engine speed (RPM)
                // Case 1: X=Temp(C1) 8 vals, Y=RPM(EC) 9 vals
                let is_start_iq_normal = x_len == 8 && y_len == 9 && x_axis_id_high == 0xC1 && y_axis_id_high == 0xEC;
                // Case 2: X=RPM(EC) 9 vals, Y=Temp(C1) 8 vals (axes swapped)
                let is_start_iq_swapped = x_len == 9 && y_len == 8 && x_axis_id_high == 0xEC && y_axis_id_high == 0xC1;
                // Case 3: X=RPM(EC) 8 vals, Y=Temp(C1) 9 vals (IDs swapped but not lengths)
                let is_start_iq_ids_swapped = x_len == 8 && y_len == 9 && x_axis_id_high == 0xEC && y_axis_id_high == 0xC1;
                // Case 4: X=Temp(C1) 9 vals, Y=RPM(EC) 8 vals (lengths swapped but not IDs)
                let is_start_iq_lens_swapped = x_len == 9 && y_len == 8 && x_axis_id_high == 0xC1 && y_axis_id_high == 0xEC;

                if is_start_iq_normal || is_start_iq_swapped || is_start_iq_ids_swapped || is_start_iq_lens_swapped {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("4-Misc".to_string());
                    map.name = Some("Start IQ".to_string());
                    map.correction_factor = Some(0.01);
                    map.unit = Some("mg/st".to_string());
                    map.description = Some("Start IQ (mg/st) | X: Coolant temp (°C) | Y: Engine speed (rpm)".to_string());
                    map.x_label = Some("degC".to_string());
                    map.y_label = Some("rpm".to_string());

                    // If axes IDs are swapped (X=RPM, Y=Temp), swap addresses to correct order
                    // Cases where X has RPM (0xEC) need address swap so X becomes Temp
                    if is_start_iq_swapped || is_start_iq_ids_swapped {
                        std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                        log::debug!("🔄 Start IQ (144): Swapped axis addresses to correct order (X=Temp, Y=RPM)");
                    }

                    // X = Temp (correction 0.1, offset -273.1), Y = RPM (correction 1.0)
                    map.x_axis_correction = Some(0.1);
                    map.x_axis_offset = Some(-273.1);
                    map.y_axis_correction = Some(1.0);
                    map.y_axis_offset = Some(0.0);
                    // Force dimensions: 9 cols (Temp) x 8 rows (RPM) - based on JSON definition
                    map.dimensions = MapDimensions::TwoDimensional { rows: 8, cols: 9 };
                    classified_this = true;
                // Fuel volume correction - EDCSuite logic:
                // X_axis_length == 9 && Y_axis_length == 8 && X_axis_ID/256 == 0xEC && Y_axis_ID/256 == 0xC0
                // In file: X=EC(RPM,9), Y=C0(IQ,8) but display as X=IQ, Y=RPM (swapped)
                } else if x_len == 9 && y_len == 8 && x_axis_id_high == 0xEC && y_axis_id_high == 0xC0 {
                    // Fuel volume correction - IQ correction per 100K
                    // File structure: X=RPM(9 vals), Y=IQ(8 vals)
                    // Display (like EDCSuite): X=IQ(8 cols), Y=RPM(9 rows)
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    map.name = Some("Fuel volume correction".to_string());
                    map.data_type = DataType::Int16; // Signed values
                    map.correction_factor = Some(0.002441);

                    // EDCSuite swaps axes: file X(EC/RPM)->display Y, file Y(C0/IQ)->display X
                    std::mem::swap(&mut map.x_axis_address, &mut map.y_axis_address);
                    // After swap: X=IQ(C0,8 vals), Y=RPM(EC,9 vals)
                    // Also swap dimensions: display as 8 cols (IQ) x 9 rows (RPM)
                    map.dimensions = MapDimensions::TwoDimensional { rows: 9, cols: 8 };
                    map.x_axis_correction = Some(0.01); // IQ factor
                    map.y_axis_correction = Some(1.0);  // RPM factor
                    map.description = Some("IQ correction per 100K | X: IQ (mg/st) | Y: Engine speed (rpm)".to_string());
                    map.x_label = Some("mg/st".to_string());
                    map.y_label = Some("rpm".to_string());
                    classified_this = true;
                }
            }
            // Length 160 (8x10 or 10x8) - Injector duration, BIP SOI Correction
            else if map.size == 160 {
                if x_len == 8 && y_len == 10 && x_axis_id_high == 0xC5 && y_axis_id_high == 0xEC {
                    // Injector duration
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    let inj_dur_count = all_maps.iter()
                        .filter(|m| m.codeblock_id == map.codeblock_id)
                        .filter(|m| m.name.as_ref().map_or(false, |n| n.starts_with("Injector duration")))
                        .count() + 1;
                    map.name = Some(format!("Injector duration {:02} [0x{:06X}]", inj_dur_count, map.address));
                    map.y_axis_correction = Some(0.01);
                    map.correction_factor = Some(0.023437);
                    classified_this = true;
                } else if x_len == 10 && y_len == 8 {
                    // BIP SOI Correction
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("1-Fuel".to_string());
                    map.name = Some(format!("BIP SOI Correction [0x{:06X}]", map.address));
                    map.correction_factor = Some(0.00390625);
                    map.x_axis_correction = Some(0.023437);
                    map.x_axis_offset = Some(-78.0);
                    classified_this = true;
                }
            }
            // Length 308 (11x14) - SOI limiter (temperature)
            // Semantic: X=Temperature (11 vals), Y=RPM (14 vals)
            else if map.size == 308 && x_len == 11 && y_len == 14 {
                map.category = Some("Detected maps".to_string());
                map.subcategory = Some("2-Limiters".to_string());
                map.name = Some("SOI limiter".to_string());
                map.correction_factor = Some(-0.023437);
                map.offset = Some(78.0);
                map.x_axis_correction = Some(0.1);
                map.x_axis_offset = Some(-273.1);
                classified_this = true;
            }
            // Length 384 (12x16 or 16x12) - DISABLED: Inversed driver wish detection removed as per requirements
            // else if map.size == 384 { ... }
            // Length 320 (16x10) - MAF airmass correction by temp - DISABLED per user request
            // This map detection is not needed and was producing incorrect results
            // else if map.size == 320 && x_len == 16 && y_len == 10 && x_axis_id_high != 0xDA { ... }
            // Length 320 (10x16 swapped to 16x10) - Boost correction by temperature
            // Both axes have ID 0xDA
            else if map.size == 320 && x_len == 16 && y_len == 10 && x_axis_id_high == 0xDA && y_axis_id_high == 0xDA {
                map.category = Some("Detected maps".to_string());
                map.subcategory = Some("2-Limiters".to_string());
                map.name = Some("Boost correction by temperature".to_string());
                map.correction_factor = Some(1.0);
                map.x_axis_correction = Some(0.1);
                map.x_axis_offset = Some(-273.1);
                // Note: axes are swapped in file - X_file=Y_semantic (Boost), Y_file=X_semantic (IAT)
                classified_this = true;
            }
            // Length 448 (16x14) - Injection correction (Height)
            else if map.size == 448 && x_len == 16 && y_len == 14 {
                if x_axis_id_high == 0xF9 && y_axis_id_high == 0xEB {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("Other".to_string());
                    map.name = Some(format!("Injection correction (Height) [0x{:06X}]", map.address));
                    map.correction_factor = Some(0.023437);
                    map.x_axis_correction = Some(0.01);
                    classified_this = true;
                }
            }
            // Length 200 (10x10) - Fuel temp overheat protection
            else if map.size == 200 && x_len == 10 && y_len == 10 {
                if x_axis_id_high == 0xEC {
                    map.category = Some("Detected maps".to_string());
                    map.subcategory = Some("Other".to_string());
                    map.name = Some(format!("Fuel temp overheat protection [0x{:06X}]", map.address));
                    map.correction_factor = Some(0.0001);
                    map.y_axis_correction = Some(0.1);
                    map.y_axis_offset = Some(-273.1);
                    classified_this = true;
                }
            }
            
            // If map was classified, add it; otherwise keep original name (might be from pattern matching)
            if classified_this || map.name.as_ref().map_or(false, |n| !n.starts_with("3D Map Size:")) {
                classified.push(map);
            }
        }
        
        classified
    }
    
    /// Lit les métadonnées d'un codeblock (ID et adresse de table) comme dans CheckCodeBlock de l'impl C#
    fn read_codeblock_metadata(&self, data: &[u8], meta_offset: usize) -> Option<(u32, u32)> {
        // Besoin d'au moins 0x1004 octets après l'offset pour lire la table
        let end_of_table_offset = meta_offset.checked_add(0x1001)?;
        let codeblock_addr_offset = meta_offset.checked_add(0x1003)?;
        if end_of_table_offset >= data.len() || codeblock_addr_offset >= data.len() {
            return None;
        }

        let end_of_table = meta_offset
            + u16::from_le_bytes([data[meta_offset + 0x1000], data[meta_offset + 0x1001]]) as usize;

        // Même garde-fou que l'impl originale : si la table vaut 0xC3C3, on ignore
        if end_of_table == meta_offset + 0xC3C3 {
            return None;
        }

        let codeblock_address = meta_offset
            + u16::from_le_bytes([data[meta_offset + 0x1002], data[meta_offset + 0x1003]]) as usize;

        if codeblock_address + 1 >= data.len() {
            return None;
        }

        let codeblock_id =
            u16::from_le_bytes([data[codeblock_address], data[codeblock_address + 1]]) as u32;

        Some((codeblock_id, codeblock_address as u32))
    }

    /// Extrait les IDs de codeblock à partir des offsets standards (0x50000, 0x60000, 0x70000)
    /// Cela permet d'éviter le mapping fixe 4C000->2, 6C000->5 sur les fichiers où les IDs sont inversés.
    fn extract_codeblock_ids_from_metadata(&self, data: &[u8]) -> HashMap<u32, (u32, u32)> {
        let mut ids = HashMap::new();
        // Clef = adresse de début attendue, valeur = (code_id, address_id dans le fichier)
        let candidates = [
            (0x4C000u32, 0x50000usize),
            (0x5C000u32, 0x60000usize),
            (0x6C000u32, 0x70000usize),
        ];

        for (start_addr, meta_offset) in candidates {
            // Si le fichier est plus petit que l'offset, on passe
            if meta_offset + 0x1004 > data.len() {
                continue;
            }

            if let Some((code_id, address_id)) = self.read_codeblock_metadata(data, meta_offset) {
                if code_id != 0 {
                    ids.insert(start_addr, (code_id, address_id));
                }
            }
        }

        ids
    }

    /// Detect codeblocks by analyzing map address ranges
    /// EDC15P files have 2-3 codeblocks, each containing similar maps
    /// 
    /// EDCSuite codeblock structure (from real files):
    /// - Codeblock 2: 0x4C000 - 0x5BFFF (Address ID: 0x51CE6)
    /// - Codeblock 3: 0x5C000 - 0x6BFFF (Address ID: 0x61CE6)
    /// - Codeblock 5: 0x6C000 - 0x7BFFF (Address ID: 0x71CE6)
    fn detect_codeblocks(&self, maps: &[DetectedMap], data: &[u8]) -> Vec<Codeblock> {
        if maps.is_empty() {
            return Vec::new();
        }

        // Tente de lire les IDs réels dans le binaire (comme VerifyCodeBlocks/CheckCodeBlock)
        let metadata_ids = self.extract_codeblock_ids_from_metadata(data);

        // EDC15P codeblocks are 0x10000 (64KB) blocks starting at 0x4C000
        // Check which codeblocks have maps
        let mut has_cb2 = false; // 0x4C000 - 0x5BFFF
        let mut has_cb3 = false; // 0x5C000 - 0x6BFFF
        let mut has_cb5 = false; // 0x6C000 - 0x7BFFF
        
        let mut cb2_start = 0x5C000u32;
        let mut cb2_end = 0x4C000u32;
        let mut cb3_start = 0x6C000u32;
        let mut cb3_end = 0x5C000u32;
        let mut cb5_start = 0x7C000u32;
        let mut cb5_end = 0x6C000u32;

        for map in maps {
            let addr = map.address;
            let end = addr + map.size as u32;
            
            // Determine which codeblock based on EDCSuite address ranges
            if addr >= 0x4C000 && addr < 0x5C000 {
                has_cb2 = true;
                cb2_start = cb2_start.min(addr);
                cb2_end = cb2_end.max(end);
            } else if addr >= 0x5C000 && addr < 0x6C000 {
                has_cb3 = true;
                cb3_start = cb3_start.min(addr);
                cb3_end = cb3_end.max(end);
            } else if addr >= 0x6C000 && addr < 0x7C000 {
                has_cb5 = true;
                cb5_start = cb5_start.min(addr);
                cb5_end = cb5_end.max(end);
            } else if addr >= 0x40000 && addr < 0x4C000 {
                // Early addresses go to codeblock 2
                has_cb2 = true;
                cb2_start = cb2_start.min(addr);
                cb2_end = cb2_end.max(end);
            } else if addr >= 0x7C000 && addr < 0x80000 {
                // Late addresses go to codeblock 5
                has_cb5 = true;
                cb5_start = cb5_start.min(addr);
                cb5_end = cb5_end.max(end);
            }
        }

        let mut codeblocks = Vec::new();

        // Create codeblocks with EDCSuite-compatible IDs
        if has_cb2 {
            let id = metadata_ids.get(&0x4C000).map(|(id, _)| *id).unwrap_or(2);
                codeblocks.push(Codeblock {
                id, // Peut être 2 (par défaut) ou la valeur lue dans le fichier
                start_address: cb2_start,
                end_address: cb2_end,
            });
        }
        
        if has_cb3 {
            let id = metadata_ids.get(&0x5C000).map(|(id, _)| *id).unwrap_or(3);
            codeblocks.push(Codeblock {
                id, // Peut être 3 (par défaut) ou la valeur lue dans le fichier
                start_address: cb3_start,
                end_address: cb3_end,
            });
        }
        
        if has_cb5 {
            let id = metadata_ids.get(&0x6C000).map(|(id, _)| *id).unwrap_or(5);
        codeblocks.push(Codeblock {
                id, // Peut être 5 (par défaut) ou la valeur lue dans le fichier
                start_address: cb5_start,
                end_address: cb5_end,
            });
        }

        log::debug!("📦 Detected {} codeblocks from {} maps", codeblocks.len(), maps.len());
        for cb in &codeblocks {
            // Pour le log, on essaie de récupérer l'adresse de la table (address_id) si elle existe
            let base_key = if cb.start_address < 0x5C000 {
                0x4C000
            } else if cb.start_address < 0x6C000 {
                0x5C000
            } else {
                0x6C000
            };

            if let Some((meta_id, addr_id)) = metadata_ids.get(&base_key) {
                log::debug!(
                    "  Codeblock {}: 0x{:X} - 0x{:X} (size: {} bytes, id lue={}, address_id=0x{:X})",
                    cb.id,
                    cb.start_address,
                    cb.end_address,
                    cb.end_address - cb.start_address,
                    meta_id,
                    addr_id
                );
            } else {
                log::debug!(
                    "  Codeblock {}: 0x{:X} - 0x{:X} (size: {} bytes, id par défaut)",
                    cb.id,
                    cb.start_address,
                    cb.end_address,
                    cb.end_address - cb.start_address
                );
            }
        }

        codeblocks
    }

    /// Assigne un codeblock aux maps à partir des blocs détectés (IDs issus du binaire si dispo)
    /// Fallback : plages EDCSuite classiques si aucun bloc détecté n'englobe l'adresse
    fn assign_codeblocks_to_maps(&self, mut maps: Vec<DetectedMap>, codeblocks: &[Codeblock]) -> Vec<DetectedMap> {
        let mut assigned_count = 0;
        for map in &mut maps {
            // Priorité : utiliser les codeblocks détectés (avec ID réel si présent dans le binaire)
            if let Some(cb) = codeblocks
                .iter()
                .find(|cb| map.address >= cb.start_address && map.address < cb.end_address)
            {
                map.codeblock_id = Some(cb.id);
                map.codeblock_start_address = Some(cb.start_address);
                map.codeblock_end_address = Some(cb.end_address);
                assigned_count += 1;
                continue;
            }

            // Fallback: mapping statique par plage si on n'a pas trouvé de codeblock détecté
            let (cb_id, cb_start, cb_end) = self.get_codeblock_info_from_address(map.address);
            
            if let Some(id) = cb_id {
                map.codeblock_id = Some(id);
                map.codeblock_start_address = cb_start;
                map.codeblock_end_address = cb_end;
                    assigned_count += 1;
            } else {
                log::debug!(
                    "⚠️  Map at 0x{:X} ({}) not assigned to any codeblock",
                    map.address,
                    map.name.as_ref().unwrap_or(&"Unknown".to_string())
                );
            }
        }
        log::debug!("✅ Assigned {} maps to codeblocks", assigned_count);
        maps
    }
    
    /// Get codeblock info (id, start, end) from address using fixed EDCSuite ranges
    fn get_codeblock_info_from_address(&self, address: u32) -> (Option<u32>, Option<u32>, Option<u32>) {
        // EDCSuite codeblock ranges:
        // - Codeblock 2: 0x4C000 - 0x5BFFF (first flashbank)
        // - Codeblock 3: 0x5C000 - 0x6BFFF (second flashbank)
        // - Codeblock 5: 0x6C000 - 0x7BFFF (third flashbank)
        if address >= 0x4C000 && address < 0x5C000 {
            (Some(2), Some(0x4C000), Some(0x5C000))
        } else if address >= 0x5C000 && address < 0x6C000 {
            (Some(3), Some(0x5C000), Some(0x6C000))
        } else if address >= 0x6C000 && address < 0x7C000 {
            (Some(5), Some(0x6C000), Some(0x7C000))
        } else if address >= 0x40000 && address < 0x4C000 {
            // Early addresses go to codeblock 2
            (Some(2), Some(0x4C000), Some(0x5C000))
        } else if address >= 0x7C000 && address < 0x80000 {
            // Late addresses go to codeblock 5
            (Some(5), Some(0x6C000), Some(0x7C000))
        } else {
            (None, None, None)
        }
    }

    /// Find similar maps in other codeblocks
    /// If a map is found in one codeblock, search for similar maps (same pattern) in other codeblocks
    fn find_similar_maps_in_other_codeblocks(
        &self,
        maps: Vec<DetectedMap>,
        codeblocks: &[Codeblock],
        data: &[u8],
    ) -> Vec<DetectedMap> {
        // Group maps by pattern (name and dimensions)
        let mut maps_by_pattern: std::collections::HashMap<String, Vec<&DetectedMap>> = std::collections::HashMap::new();
        for map in &maps {
            if let Some(name) = &map.name {
                let key = format!("{}|{:?}", name, map.dimensions);
                maps_by_pattern.entry(key).or_insert_with(Vec::new).push(map);
            }
        }

        let mut additional_maps = Vec::new();

        // For each map pattern found in a codeblock, search in other codeblocks
        for (_pattern_key, pattern_maps) in &maps_by_pattern {
            // Get the codeblocks where this pattern was found
            let found_codeblocks: HashSet<u32> = pattern_maps
                .iter()
                .filter_map(|m| m.codeblock_id)
                .collect();

            // Search in codeblocks where this pattern wasn't found yet
            for codeblock in codeblocks {
                if !found_codeblocks.contains(&codeblock.id) {
                    // Search for similar map in this codeblock
                    if let Some(reference_map) = pattern_maps.first() {
                        if let Some(similar_map) = self.search_similar_map_in_codeblock(
                            reference_map,
                            codeblock,
                            data,
                        ) {
                            additional_maps.push(similar_map);
                        }
                    }
                }
            }
        }

        // Combine original maps with additional maps found in other codeblocks
        let mut all_maps = maps;
        all_maps.extend(additional_maps);
        all_maps.sort_by_key(|m| m.address);
        all_maps
    }

    /// Search for a similar map in a specific codeblock
    /// Similar means same pattern (name, dimensions, axis IDs)
    fn search_similar_map_in_codeblock(
        &self,
        reference_map: &DetectedMap,
        codeblock: &Codeblock,
        data: &[u8],
    ) -> Option<DetectedMap> {
        // Search within the codeblock address range
        let search_start = codeblock.start_address as usize;
        let search_end = codeblock.end_address.min(data.len() as u32) as usize;

        // Get reference pattern info
        let _reference_name = reference_map.name.as_ref()?;
        let reference_dims = match &reference_map.dimensions {
            MapDimensions::TwoDimensional { rows, cols } => (*rows, *cols),
            _ => return None,
        };

        // Search for maps with same dimensions in this codeblock
        let mut t = search_start;
        while t < search_end.saturating_sub(100) {
            if let Some((detected_maps_vec, skip_len)) = self.check_map(data, t) {
                // Process all maps returned (can be multiple if MapSelector is detected)
                for detected_map in detected_maps_vec {
                    // Check if dimensions match
                    let detected_dims = match &detected_map.dimensions {
                        MapDimensions::TwoDimensional { rows, cols } => (*rows, *cols),
                        _ => {
                            continue;
                        }
                    };

                    // CRITICAL FIX: Check both normal and swapped dimensions
                    // Reference map has semantic order (after swap in create_map_from_pattern)
                    // Detected map has file order (before any swap)
                    // They should match if either:
                    // 1. detected_dims == reference_dims (no swap needed)
                    // 2. detected_dims == (reference_dims.1, reference_dims.0) (swap needed)
                    let swapped_reference_dims = (reference_dims.1, reference_dims.0);
                    let dims_match = detected_dims == reference_dims || detected_dims == swapped_reference_dims;
                    
                    if dims_match {
                        // Check if pattern matches (same name)
                        // We need to get axis IDs from the detected map structure
                        // For now, let's check if the pattern name matches by trying to match the pattern
                        // We'll use a simpler approach: check if dimensions and size match
                        let map_size = detected_map.size;
                        if map_size == reference_map.size {
                            // Found similar map in this codeblock - create a new map with codeblock info
                            let mut similar_map = detected_map;
                        similar_map.codeblock_id = Some(codeblock.id);
                        similar_map.codeblock_start_address = Some(codeblock.start_address);
                        similar_map.codeblock_end_address = Some(codeblock.end_address);
                        // CRITICAL: Copy ALL metadata from reference, including axis addresses and corrections
                        similar_map.name = reference_map.name.clone();
                        similar_map.unit = reference_map.unit.clone();
                        similar_map.description = reference_map.description.clone();
                        // Copy corrections from reference map
                        similar_map.x_axis_correction = reference_map.x_axis_correction;
                        similar_map.x_axis_offset = reference_map.x_axis_offset;
                        similar_map.y_axis_correction = reference_map.y_axis_correction;
                        similar_map.y_axis_offset = reference_map.y_axis_offset;
                        similar_map.correction_factor = reference_map.correction_factor;
                        similar_map.offset = reference_map.offset;
                        
                        // CRITICAL: Recalculate axis addresses based on the actual structure_start of this map
                        // The structure_start is where the axis IDs are located (t in check_map)
                        let structure_start = t as u32;
                        // We need to get the axis IDs and lengths from the detected structure
                        // But we can't easily get them from detected_map, so we'll use the reference's pattern
                        // Actually, we should recalculate based on the actual structure at t
                        // For now, let's try to match the pattern and recalculate properly
                            // CRITICAL FIX: First read axis IDs, then find matching pattern
                            // The pattern must match BOTH dimensions AND axis IDs
                            if t + 8 < data.len() {
                                let first_axis_id = u16::from_le_bytes([data[t], data[t + 1]]);
                                let first_axis_len = u16::from_le_bytes([data[t + 2], data[t + 3]]) as usize;
                                let second_offset = t + 4 + (first_axis_len * 2);
                                if second_offset + 4 <= data.len() {
                                    let second_axis_id = u16::from_le_bytes([data[second_offset], data[second_offset + 1]]);
                                    let second_axis_len = u16::from_le_bytes([data[second_offset + 2], data[second_offset + 3]]) as usize;
                                    
                                    let first_id_high = (first_axis_id >> 8) as u8;
                                    let second_id_high = (second_axis_id >> 8) as u8;
                                    
                                    // Find pattern that matches size, dimensions AND axis IDs
                                    // CRITICAL: Search in two passes to prioritize specific IDs over wildcards
                                    // Pass 1: Look for patterns with SPECIFIC (non-zero) axis IDs
                                    // Pass 2: Fall back to wildcard patterns (0x00) only if no specific match
                                    
                                    let pattern = self.patterns.iter().find(|p| {
                                        if p.length != map_size {
                                            return false;
                                        }
                                        // Only match patterns with SPECIFIC IDs in this pass
                                        if p.x_axis_id_high == 0x00 || p.y_axis_id_high == 0x00 {
                                            return false;
                                        }
                                        // Check normal order: first_axis=X, second_axis=Y
                                        let normal_match = 
                                            p.x_axis_length == first_axis_len &&
                                            p.y_axis_length == second_axis_len &&
                                            p.x_axis_id_high == first_id_high &&
                                            p.y_axis_id_high == second_id_high;
                                        // Check swapped order: first_axis=Y, second_axis=X
                                        let swapped_match = 
                                            p.x_axis_length == second_axis_len &&
                                            p.y_axis_length == first_axis_len &&
                                            p.x_axis_id_high == second_id_high &&
                                            p.y_axis_id_high == first_id_high;
                                        normal_match || swapped_match
                                    }).or_else(|| {
                                        // Pass 2: Fall back to wildcard patterns
                                        self.patterns.iter().find(|p| {
                                            if p.length != map_size {
                                                return false;
                                            }
                                            let normal_match = 
                                                p.x_axis_length == first_axis_len &&
                                                p.y_axis_length == second_axis_len &&
                                                (p.x_axis_id_high == 0x00 || p.x_axis_id_high == first_id_high) &&
                                                (p.y_axis_id_high == 0x00 || p.y_axis_id_high == second_id_high);
                                            let swapped_match = 
                                                p.x_axis_length == second_axis_len &&
                                                p.y_axis_length == first_axis_len &&
                                                (p.x_axis_id_high == 0x00 || p.x_axis_id_high == second_id_high) &&
                                                (p.y_axis_id_high == 0x00 || p.y_axis_id_high == first_id_high);
                                            normal_match || swapped_match
                                        })
                                    });
                                    
                                    if let Some(pattern) = pattern {
                                    let x_axis_id = first_axis_id;
                                    let x_axis_len = first_axis_len;
                                    let _y_offset = second_offset;
                                    let y_axis_id = second_axis_id;
                                    let y_axis_len = second_axis_len;
                                    
                                        let map_address = structure_start + 8
                                            + (x_axis_len as u32 * 2)
                                            + (y_axis_len as u32 * 2);
                                        
                                        // Recalculate axis addresses using same logic as create_map_from_pattern
                                        let x_axis_id_high = (x_axis_id >> 8) as u8;
                                        let y_axis_id_high = (y_axis_id >> 8) as u8;
                                        let pattern_x_id_high = pattern.x_axis_id_high;
                                        let pattern_y_id_high = pattern.y_axis_id_high;
                                        
                                        let (_actual_x_axis_id, _actual_y_axis_id, actual_x_axis_len, actual_y_axis_len, axes_were_swapped) =
                                            if pattern_x_id_high != 0x00 && x_axis_id_high == pattern_x_id_high &&
                                               pattern_y_id_high != 0x00 && y_axis_id_high == pattern_y_id_high {
                                                (x_axis_id, y_axis_id, x_axis_len, y_axis_len, false)
                                            } else if pattern_y_id_high != 0x00 && x_axis_id_high == pattern_y_id_high &&
                                                      pattern_x_id_high != 0x00 && y_axis_id_high == pattern_x_id_high {
                                                (y_axis_id, x_axis_id, y_axis_len, x_axis_len, true)
                                            } else {
                                                (x_axis_id, y_axis_id, x_axis_len, y_axis_len, false)
                                            };
                                        
                                        let first_axis_is_x = !axes_were_swapped;
                                        let actual_x_bytes = (actual_x_axis_len as u32) * 2;
                                        let actual_y_bytes = (actual_y_axis_len as u32) * 2;
                                        let (first_axis_bytes, second_axis_bytes) = if first_axis_is_x {
                                            (actual_x_bytes, actual_y_bytes)
                                        } else {
                                            (actual_y_bytes, actual_x_bytes)
                                        };
                                        
                                        let (x_axis_address, y_axis_address) = if first_axis_is_x {
                                            (
                                                map_address - 4 - first_axis_bytes - second_axis_bytes,
                                                map_address - second_axis_bytes,
                                            )
                                        } else {
                                            (
                                                map_address - second_axis_bytes,
                                                map_address - 4 - first_axis_bytes - second_axis_bytes,
                                            )
                                        };
                                        
                                        // Update axis addresses and corrections from pattern
                                        similar_map.x_axis_address = Some(x_axis_address);
                                        similar_map.y_axis_address = Some(y_axis_address);
                                        similar_map.x_axis_correction = Some(pattern.x_axis_correction);
                                        similar_map.x_axis_offset = Some(pattern.x_axis_offset);
                                        similar_map.y_axis_correction = Some(pattern.y_axis_correction);
                                        similar_map.y_axis_offset = Some(pattern.y_axis_offset);
                                        similar_map.correction_factor = Some(pattern.correction);
                                        similar_map.offset = Some(pattern.offset);
                                    
                                    log::debug!(
                                        "🔄 Found similar map '{}' in codeblock {} - recalculated axis addresses: X=0x{:X}, Y=0x{:X}",
                                        reference_map.name.as_ref().unwrap_or(&"Unknown".to_string()),
                                        codeblock.id,
                                        x_axis_address,
                                        y_axis_address
                                    );
                                    
                                    return Some(similar_map);
                                    }
                                }
                            }
                        }
                    }
                }

                // Skip logic
                let mut skip = skip_len;
                if skip > 2 {
                    skip -= 2;
                }
                if skip % 2 > 0 {
                    skip -= 1;
                }
                if skip < 2 {
                    skip = 2;
                }
                t += skip;
            } else {
                t += 2;
            }
        }

        None
    }
}

impl Default for EDC15PDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edc15_detector_creation() {
        let detector = EDC15PDetector::new();
        assert!(detector.patterns.len() > 0);
    }

}

