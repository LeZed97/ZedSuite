// This file will be used to replace edc15_patterns.rs with a complete set of patterns
// Based on the C# EDC15PFileParser.cs analysis


#[derive(Clone)]
pub struct EDC15PMapPattern {
    pub name: String,
    pub length: usize,
    pub x_axis_length: usize,
    pub y_axis_length: usize,
    pub x_axis_id_high: u8,
    pub y_axis_id_high: u8,
    pub x_axis_id_low: u8,
    pub y_axis_id_low: u8,
    pub category: String,
    pub subcategory: String,
    pub correction: f64,
    pub offset: f64,
    pub x_axis_correction: f64,
    pub x_axis_offset: f64,
    pub y_axis_correction: f64,
    pub y_axis_offset: f64,
    pub x_axis_descr: String,
    pub y_axis_descr: String,
    pub z_axis_descr: String,
    pub x_axis_units: String,
    pub y_axis_units: String,
    pub allow_multiple: bool,
}

impl EDC15PMapPattern {
    pub fn load_patterns() -> Vec<EDC15PMapPattern> {
        let mut patterns = Vec::new();
        
        // Helper macro to reduce boilerplate
        macro_rules! add_pattern {
            ($name:expr, $len:expr, $x_len:expr, $y_len:expr, $x_id_h:expr, $y_id_h:expr,
             $cat:expr, $subcat:expr, $corr:expr, $off:expr, 
             $x_corr:expr, $x_off:expr, $y_corr:expr, $y_off:expr,
             $x_desc:expr, $y_desc:expr, $z_desc:expr, $x_unit:expr, $y_unit:expr) => {
                patterns.push(EDC15PMapPattern {
                    name: $name.to_string(),
                    length: $len,
                    x_axis_length: $x_len,
                    y_axis_length: $y_len,
                    x_axis_id_high: $x_id_h,
                    y_axis_id_high: $y_id_h,
                    x_axis_id_low: 0x00,
                    y_axis_id_low: 0x00,
                    category: $cat.to_string(),
                    subcategory: $subcat.to_string(),
                    correction: $corr,
                    offset: $off,
                    x_axis_correction: $x_corr,
                    x_axis_offset: $x_off,
                    y_axis_correction: $y_corr,
                    y_axis_offset: $y_off,
                    x_axis_descr: $x_desc.to_string(),
                    y_axis_descr: $y_desc.to_string(),
                    z_axis_descr: $z_desc.to_string(),
                    x_axis_units: $x_unit.to_string(),
                    y_axis_units: $y_unit.to_string(),
                    allow_multiple: false,
                });
            };
        }
        
        // 700 bytes - Launch control
        add_pattern!("Launch control", 700, 25, 14, 0x00, 0x00,
            "Limiters", "Launch Control", 0.01, 0.0, 1.0, 0.0, 0.15625, 0.0,
            "Engine speed", "Vehicle speed", "IQ limit", "rpm", "km/h");
        
        // 570 bytes - Injector duration 01-04 (19x15)
        add_pattern!("Injector duration", 570, 19, 15, 0xEC, 0xC5,
            "Fuel", "1-Fuel", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");

        add_pattern!("Injector duration", 570, 19, 15, 0xC5, 0xEC,
            "Fuel", "1-Fuel", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");

        add_pattern!("Injector duration", 570, 19, 15, 0xC4, 0xEA,
            "Fuel", "1-Fuel", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");

        add_pattern!("Injector duration", 570, 19, 15, 0xC4, 0xEC,
            "Fuel", "1-Fuel", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
        
        // 480 bytes - Injector duration (16x15) - Injector duration 01-04
        // File structure: [Y_ID=C5][Y_len=15][Y_data=IQ][X_ID=EC][X_len=16][X_data=RPM][Map]
        add_pattern!("Injector duration", 480, 16, 15, 0xEC, 0xC5,
            "Fuel", "1-Fuel", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
            
        add_pattern!("Injector duration", 480, 16, 15, 0xC4, 0xEA,
            "Fuel", "1-Fuel", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
        
        // 200 bytes - Injector duration 00 (10x10)
        // File structure: [Y_ID=C5][Y_len=10][Y_data=IQ][X_ID=EC][X_len=10][X_data=RPM][Map]
        add_pattern!("Injector duration", 200, 10, 10, 0xEC, 0xC5,
            "Fuel", "1-Fuel", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
        
        // 198 bytes - Injector duration 05 (11x9)
        // File structure: [Y_ID=C5][Y_len=9][Y_data=IQ][X_ID=EC][X_len=11][X_data=RPM][Map]
        add_pattern!("Injector duration", 198, 11, 9, 0xEC, 0xC5,
            "Fuel", "1-Fuel", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
        
        // 12 bytes - Selector for injector duration (1x6)
        // This is a 1D map with 6 values (indices: 0, 256, 512, 768, 1024, 1280)
        add_pattern!("Selector for injector duration", 12, 1, 6, 0x00, 0xEC,
            "Fuel", "1-Fuel", 1.0, 0.0, 1.0, 0.0, 1.0, 0.0,
            "", "Index", "Value", "", "");
        
        // 448 bytes - SOI maps (14x16)
        // In file: [Y_ID=F9][Y_len=16][Y_data=RPM][X_ID=EB][X_len=14][X_data=IQ][Map]
        // Semantic: X=IQ (14 vals, ID 0xEB), Y=RPM (16 vals, ID 0xF9)
        add_pattern!("Start of injection (SOI)", 448, 14, 16, 0xEB, 0xF9,
            "Fuel", "Injection Timing", -0.023437, 78.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "SOI degrees BTDC", "mg/st", "rpm");
        
        // Alternative with 0xEA instead of 0xEB for X axis
        add_pattern!("Start of injection (SOI)", 448, 14, 16, 0xEA, 0xF9,
            "Fuel", "Injection Timing", -0.023437, 78.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "SOI degrees BTDC", "mg/st", "rpm");
        
        // Alternative with 0xE9 instead of 0xF9 for Y axis  
        add_pattern!("Start of injection (SOI)", 448, 14, 16, 0xEB, 0xE9,
            "Fuel", "Injection Timing", -0.023437, 78.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "SOI degrees BTDC", "mg/st", "rpm");
        
        add_pattern!("Start of injection (SOI)", 448, 14, 16, 0xEA, 0xE9,
            "Fuel", "Injection Timing", -0.023437, 78.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "SOI degrees BTDC", "mg/st", "rpm");
        
        // REMOVED: Injection correction maps (448 bytes, 16x14) 
        // They conflicted with SOI maps which have same size and similar axis IDs
        
        // 416 bytes - Multiple map types (13x16)
        // Smoke limiter: X=Airflow (13 values, ID 0xDA), Y=RPM (16 values, ID 0xF9)
        // File structure: [Y_ID=F9][Y_len=16][Y_data][X_ID=DA][X_len=13][X_data][Map_data]
        // DISABLED - Multi-smoke detection in mod.rs handles this with temperature selector
        // add_pattern!("Smoke limiter", 416, 13, 16, 0xDA, 0xF9,
        //     "Limiters", "Smoke", 0.01, 0.0, 0.1, 0.0, 1.0, 0.0,
        //     "Airflow", "Engine speed", "Max IQ", "mg/st", "rpm");
            
        // REMOVED: IQ by MAP limiter 416 bytes - this was incorrect
        // 416 bytes with EC/DA axis IDs is "IQ by MAF limiter 1", not "IQ by MAP limiter"
        // The correct pattern is defined later with 13x16 dimensions
            
        // N75 416-byte pattern moved to line ~184 with correct axis order
            
        // REMOVED: SOI 416 bytes (16x13) - incorrect dimensions, only 14x16 is valid
            
        add_pattern!("EGR", 416, 16, 13, 0xEC, 0xC0,
            "Misc", "EGR", 0.1, 0.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "MAF", "mg/st", "rpm");
        
        // 390 bytes - Injector duration limiter (15x13)
        add_pattern!("Injector duration", 390, 15, 13, 0xC5, 0xEC,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");

        // NOTE: Inversed driver wish pattern REMOVED - not needed
        // NOTE: Smoke limiter 384 bytes pattern REMOVED - incorrect size
        // Real Smoke limiters are 416 bytes (13x16), defined as hardcoded maps
            
        add_pattern!("EGR", 384, 16, 12, 0xEC, 0xC0,
            "Misc", "EGR", 0.1, 0.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "MAF", "mg/st", "rpm");
        
        // 360 bytes - Injector duration (15x12)
        add_pattern!("Injector duration", 360, 15, 12, 0xC5, 0xEC,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
        
        // 352 bytes - Multiple types (16x11)
        add_pattern!("EGR", 352, 16, 11, 0xEC, 0xC0,
            "Misc", "EGR", 0.1, 0.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "MAF", "mg/st", "rpm");
            
        // N75 duty cycle - multiple variants with different sizes
        // 416 bytes (13x16) - Verified in 1.9 TDI files
        // AxisX = IQ (13 values, ID 0xEA), AxisY = RPM (16 values, ID 0xEC)
        add_pattern!("N75 duty cycle", 416, 13, 16, 0xEA, 0xEC,
            "Turbo boost pressure control", "N75", -0.01, 100.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "Duty cycle %", "mg/st", "rpm");
        // 352 bytes (16x11) - Other variants
        add_pattern!("N75 duty cycle", 352, 16, 11, 0xEC, 0xEA,
            "Turbo boost pressure control", "N75", -0.01, 100.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "Duty cycle %", "mg/st", "rpm");
        add_pattern!("N75 duty cycle", 352, 11, 16, 0xEA, 0xEC,
            "Turbo boost pressure control", "N75", -0.01, 100.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "Duty cycle %", "mg/st", "rpm");
        
        // 320 bytes - BOOST TARGET MAP (10x16) - IMPORTANT!
        // Columns=10 (X axis = IQ mg/stroke), Rows=16 (Y axis = Engine speed rpm)
        // NOTE: Different ECU variants use different axis IDs for the same data!
        // Variant 1: IQ has ID 0xC0, RPM has ID 0xEC (verified in 1.9 TDI files)
        add_pattern!("Boost target map", 320, 10, 16, 0xC0, 0xEC,
            "Turbo", "Boost Target", 1.0, 0.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "Boost target", "mg/st", "rpm");
        // Variant 2: IQ has ID 0xEC, RPM has ID 0xC0    
        add_pattern!("Boost target map", 320, 10, 16, 0xEC, 0xC0,
            "Turbo", "Boost Target", 1.0, 0.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "Boost target", "mg/st", "rpm");
        // Variant 3: IQ has ID 0xEA, RPM has ID 0xC0            
        add_pattern!("Boost target map", 320, 10, 16, 0xEA, 0xC0,
            "Turbo", "Boost Target", 1.0, 0.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "Boost target", "mg/st", "rpm");
            
        // NOTE: IQ by MAP limiter pattern REMOVED - same axis IDs as IQ by MAF (DA, EC)
        // Both maps are detected as "IQ by MAF/MAP limiter" and distinguished in classify_maps_by_data
        // based on X axis values:
        // - IQ by MAP: X axis < 2500 (Boost pressure 800-2200 mbar)
        // - IQ by MAF: X axis >= 2500 (Airflow 3000-10500 * 0.1)
            
        // DISABLED per user request - MAF airmass correction by temp not needed
        // add_pattern!("MAF airmass correction by temp", 320, 16, 10, 0x00, 0x00,
        //     "Misc", "MAF", 0.1, 0.0, 0.1, -273.1, 0.1, 0.0,
        //     "Temperature", "Airflow", "Corrected airflow", "°C", "mg/st");
        
        // 308 bytes - SOI limiter by temperature (11x14)
        // Structure in file: [Y_ID=F9][Y_len=14][Y_data][X_ID=C5][X_len=11][X_data][Map]
        // Semantic: X=Temperature (11 vals, ID 0xC5), Y=RPM (14 vals, ID 0xF9)
        add_pattern!("SOI limiter", 308, 11, 14, 0xC5, 0xF9,
            "Limiters", "SOI", -0.023437, 78.0, 0.1, -273.1, 1.0, 0.0,
            "Temperature", "Engine speed", "SOI limit", "°C", "rpm");
        
        // 286 bytes - Unknown 13x11 map (was incorrectly classified as Driver wish)
        // Real Driver wish maps are 256 bytes (8x16) with EC/C0 axis IDs
        // Removed to prevent misclassification
        
        // 280 bytes - Boost target (10 cols x 14 rows) - Symbol display: X=IQ (10 values), Y=RPM (14 values)
        // File structure: [EC axis ID][len=14][14 RPM values (1000-4750)][C0 axis ID][len=10][10 IQ values (0-45 mg/st)][280 bytes map]
        // EC (0xEC) = RPM axis, C0 (0xC0) = IQ axis
        // Pattern: x_axis = IQ (C0, 10 values), y_axis = RPM (EC, 14 values)
        add_pattern!("Boost target map", 280, 10, 14, 0xC0, 0xEC,
            "Turbo", "Boost Target", 1.0, 0.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "Boost target", "mg/st", "rpm");
        
        // 256 bytes - Driver wish - 16 cols (Throttle %) x 8 rows (RPM)
        // Note: 16 * 8 * 2 = 256 bytes
        // Display: X axis = Throttle position (TPS %), Y axis = Engine speed (RPM)
        // Axis IDs: X=0xEC (TPS%), Y=0xC0 (RPM)
        add_pattern!("Driver wish", 256, 16, 8, 0xEC, 0xC0,
            "Misc", "Driver Request", 0.01, 0.0, 0.01, 0.0, 1.0, 0.0,
            "Throttle position", "Engine speed", "Requested IQ", "TPS %", "rpm");

        // 240 bytes - Driver wish - 10 cols (Throttle %) x 12 rows (RPM)
        // Note: 10 * 12 * 2 = 240 bytes
        add_pattern!("Driver wish", 240, 10, 12, 0xEC, 0xC0,
            "Misc", "Driver Request", 0.01, 0.0, 0.01, 0.0, 1.0, 0.0,
            "Throttle position", "Engine speed", "Requested IQ", "TPS %", "rpm");

        // 216 bytes - Driver wish - 9 cols (Throttle %) x 12 rows (RPM)
        // Note: 9 * 12 * 2 = 216 bytes
        add_pattern!("Driver wish", 216, 9, 12, 0xEC, 0xC0,
            "Misc", "Driver Request", 0.01, 0.0, 0.01, 0.0, 1.0, 0.0,
            "Throttle position", "Engine speed", "Requested IQ", "TPS %", "rpm");
        
        // 200 bytes - Multiple types (10x10)
        // Boost limit map: X=RPM (ID 0xEC), Y=Atm pressure (ID 0xC0)
        // Structure in file: [Y_ID=C0][Y_len=10][Y_data][X_ID=EC][X_len=10][X_data][Map]
        add_pattern!("Boost limit map", 200, 10, 10, 0xEC, 0xC0,
            "Limiters", "Boost", 1.0, 0.0, 1.0, 0.0, 1.0, 0.0,
            "Engine speed", "Atm pressure", "Max boost", "rpm", "mbar");
            
        add_pattern!("Boost limit map", 200, 10, 10, 0xEA, 0xC0,
            "Limiters", "Boost", 1.0, 0.0, 1.0, 0.0, 1.0, 0.0,
            "Engine speed", "Atm pressure", "Max boost", "rpm", "mbar");
            
        add_pattern!("Injector duration", 200, 10, 10, 0xC5, 0xEC,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
            
        add_pattern!("Injector duration", 200, 10, 10, 0xC4, 0xEA,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
            
        add_pattern!("Injector duration", 200, 10, 10, 0xC4, 0xEC,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
            
        // DISABLED - Now detected in mod.rs to avoid duplicates
        // add_pattern!("Fuel temp overheat protection", 200, 10, 10, 0xEC, 0x00,
        //     "Limiters", "Temperature", 0.0001, 0.0, 1.0, 0.0, 0.1, -273.1,
        //     "Engine speed", "Fuel temp", "Protection", "rpm", "°C");
        
        // 192 bytes - Driver wish - 8 cols (Throttle %) x 12 rows (RPM)
        // Note: 8 * 12 * 2 = 192 bytes
        add_pattern!("Driver wish", 192, 8, 12, 0xEC, 0xC0,
            "Misc", "Driver Request", 0.01, 0.0, 0.01, 0.0, 1.0, 0.0,
            "Throttle position", "Engine speed", "Requested IQ", "TPS %", "rpm");
        
        // 180 bytes - Start IQ (10x9) - 10 cols (Temp) x 9 rows (RPM)
        // Note: 10 * 9 * 2 = 180 bytes
        // WinOLS: X = Coolant temp (°C), Y = Engine speed (RPM)
        add_pattern!("Start IQ", 180, 10, 9, 0xC1, 0xEC,
            "Fuel", "Starting", 0.01, 0.0, 0.1, -273.1, 1.0, 0.0,
            "Coolant temp", "Engine speed", "Start IQ", "°C", "rpm");
            
        // Boost limit map 180 bytes: X=RPM (ID 0xEC), Y=Atm pressure (ID 0xC0)
        add_pattern!("Boost limit map", 180, 9, 10, 0xEC, 0xC0,
            "Limiters", "Boost", 1.0, 0.0, 1.0, 0.0, 1.0, 0.0,
            "Engine speed", "Atm pressure", "Max boost", "rpm", "mbar");
            
        add_pattern!("Injector duration", 180, 9, 10, 0xC5, 0xEC,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
            
        add_pattern!("Injector duration", 180, 10, 9, 0xC5, 0xEC,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
            
        add_pattern!("Injector duration", 180, 9, 10, 0xC4, 0xEA,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
        
        // 162 bytes - Start IQ (9x9) - 9 cols (Temp) x 9 rows (RPM)
        // Note: 9 * 9 * 2 = 162 bytes
        add_pattern!("Start IQ", 162, 9, 9, 0xC1, 0xEC,
            "Fuel", "Starting", 0.01, 0.0, 0.1, -273.1, 1.0, 0.0,
            "Coolant temp", "Engine speed", "Start IQ", "°C", "rpm");
        
        // 160 bytes - Multiple types (8x10 or 10x8)
        add_pattern!("Injector duration", 160, 8, 10, 0xC5, 0xEC,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
            
        add_pattern!("BIP SOI Correction", 160, 10, 8, 0x00, 0x00,
            "Fuel", "BIP", 0.00390625, 0.0, 0.023437, -78.0, 1.0, 0.0,
            "Crankshaft degrees", "Engine speed", "BIP calculation", "°BTDC", "rpm");
        
        // 144 bytes - Multiple types (9x8 or 8x9 or 3x24)
        add_pattern!("Fuel volume correction", 144, 9, 8, 0xEC, 0xC0,
            "Fuel", "Correction", 0.002441, 0.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "IQ correction per 100K", "mg/st", "rpm");
            
        // 144 bytes - Start IQ (8x9 or 9x8) - display as 8 cols (Temp) x 9 rows (RPM)
        // Note: 8 * 9 * 2 = 144 bytes
        // File can have either axis order - classification in mod.rs handles the swap
        add_pattern!("Start IQ", 144, 8, 9, 0xC1, 0xEC,
            "Fuel", "Starting", 0.01, 0.0, 0.1, -273.1, 1.0, 0.0,
            "Coolant temp", "Engine speed", "Start IQ", "°C", "rpm");
        // Also match swapped axis order (X=EC/Y=C1)
        add_pattern!("Start IQ", 144, 9, 8, 0xEC, 0xC1,
            "Fuel", "Starting", 0.01, 0.0, 1.0, 0.0, 0.1, -273.1,
            "Engine speed", "Coolant temp", "Start IQ", "rpm", "°C");
            
        add_pattern!("Torque limiter", 144, 3, 24, 0x00, 0x00,
            "Limiters", "Torque", 0.01, 0.0, 1.0, 0.0, 1.0, 0.0,
            "Engine speed", "Atm pressure", "Max IQ", "rpm", "mbar");
        
        // 128 bytes - Multiple types (8x8)
        // DISABLED - MAF correction by temperature is now detected in mod.rs with proper validation
        // add_pattern!("MAF correction by temp", 128, 8, 8, 0xEC, 0xC1,
        //     "Limiters", "MAF", 0.01, 0.0, 0.1, -273.1, 1.0, 0.0,
        //     "IAT", "Engine speed", "Limit", "°C", "rpm");

        // add_pattern!("MAF correction by temp", 128, 8, 8, 0xEA, 0xC1,
        //     "Limiters", "MAF", 0.01, 0.0, 0.1, -273.1, 1.0, 0.0,
        //     "IAT", "Engine speed", "Limit", "°C", "rpm");
        
        // 126 bytes - Torque limiter (3x21)
        add_pattern!("Torque limiter", 126, 3, 21, 0x00, 0x00,
            "Limiters", "Torque", 0.01, 0.0, 1.0, 0.0, 1.0, 0.0,
            "Engine speed", "Atm pressure", "Max IQ", "rpm", "mbar");
        
        // 132 bytes - Torque limiter (3x22)
        add_pattern!("Torque limiter", 132, 3, 22, 0x00, 0x00,
            "Limiters", "Torque", 0.01, 0.0, 1.0, 0.0, 1.0, 0.0,
            "Engine speed", "Atm pressure", "Max IQ", "rpm", "mbar");
        
        // 138 bytes - Torque limiter (3x23)
        add_pattern!("Torque limiter", 138, 3, 23, 0x00, 0x00,
            "Limiters", "Torque", 0.01, 0.0, 1.0, 0.0, 1.0, 0.0,
            "Engine speed", "Atm pressure", "Max IQ", "rpm", "mbar");
        
        // 120 bytes - Multiple types (3x20 or 6x10 or 10x6)
        add_pattern!("Torque limiter", 120, 3, 20, 0x00, 0x00,
            "Limiters", "Torque", 0.01, 0.0, 1.0, 0.0, 1.0, 0.0,
            "Engine speed", "Atm pressure", "Max IQ", "rpm", "mbar");
            
        // 198 bytes - Injector duration 05 (11x9)
        add_pattern!("Injector duration", 198, 11, 9, 0xC5, 0xEC,
            "Fuel", "Injection", 0.023437, 0.0, 1.0, 0.0, 0.01, 0.0,
            "Engine speed", "IQ", "Duration", "rpm", "mg/st");
        
        // 162 bytes - Start IQ (9x9) - 9 cols (Temp) x 9 rows (RPM)
        // Note: 9 * 9 * 2 = 162 bytes
        add_pattern!("Start IQ", 162, 9, 9, 0xC1, 0xEC,
            "Fuel", "Starting", 0.01, 0.0, 0.1, -273.1, 1.0, 0.0,
            "Coolant temp", "Engine speed", "Start IQ", "°C", "rpm");
        
        // 416 bytes - EGR 01 (13x16)
        add_pattern!("EGR 01", 416, 13, 16, 0xEC, 0xC0,
            "Misc", "EGR", 0.1, 0.0, 0.01, 0.0, 1.0, 0.0,
            "IQ", "Engine speed", "MAF", "mg/st", "rpm");
        
        // 416 bytes - IQ by MAF limiter 1 (13x16)
        // X=Airflow (13 values, ID 0xDA), Y=RPM (16 values, ID 0xEC)
        // File structure: [Y_ID=EC][Y_len=16][Y_data][X_ID=DA][X_len=13][X_data][Map]
        add_pattern!("IQ by MAF limiter", 416, 13, 16, 0xDA, 0xEC,
            "Limiters", "MAF", 0.01, 0.0, 0.1, 0.0, 1.0, 0.0,
            "Airflow", "Engine speed", "Max IQ", "mg/st", "rpm");
        
        // 200 bytes - Limit of overboost protection (10x10)
        // Structure in file: [Y_ID=EC][Y_len=10][Y_data][X_ID=C2][X_len=10][X_data][Map]
        // Semantic: X=VNT Duty (10 vals, ID 0xC2), Y=RPM (10 vals, ID 0xEC)
        add_pattern!("Limit of overboost protection", 200, 10, 10, 0xC2, 0xEC,
            "Limiters", "Boost", 1.0, 0.0, 0.01, 0.0, 1.0, 0.0,
            "VNT Actuator Duty Cycle", "Engine speed", "Boost limit", "%", "rpm");
        
        // 320 bytes - Boost correction by temperature (10x16)
        // Structure in file: [Y_ID=DA][Y_len=16][Y_data][X_ID=DA][X_len=10][X_data][Map]
        // Both axes have ID high byte 0xDA, use wildcards and match by dimensions
        add_pattern!("Boost correction by temperature", 320, 10, 16, 0xDA, 0xDA,
            "Turbo boost pressure", "Boost", 1.0, 0.0, 0.1, -273.1, 1.0, 0.0,
            "IAT", "Requested boost", "Boost correction", "°C", "mbar");
        
        // 320 bytes - IQ by MAF/MAP limiter (10x16) - alternate size found in some ECUs
        // X=Airflow/Boost (10 values, ID 0xDA), Y=RPM (16 values, ID 0xEC)
        // distinguish_iq_limiter_maps will rename to "IQ by MAP" if X < 2500 (boost pressure)
        add_pattern!("IQ by MAF limiter", 320, 10, 16, 0xDA, 0xEC,
            "Limiters", "MAF", 0.01, 0.0, 0.1, 0.0, 1.0, 0.0,
            "Airflow", "Engine speed", "Max IQ", "mg/st", "rpm");
        // Variant with inverted axis order
        add_pattern!("IQ by MAF limiter", 320, 10, 16, 0xEC, 0xDA,
            "Limiters", "MAF", 0.01, 0.0, 1.0, 0.0, 0.1, 0.0,
            "Engine speed", "Airflow", "Max IQ", "rpm", "mg/st");
        
        // 448 bytes - SOI maps with different temperatures (14x16) - already added above
        
        // 308 bytes - SOI limiter (temperature) (14x11) - already exists but verify
        // Already added above
        
        add_pattern!("Boost actuator upper limit curve (N75)", 32, 1, 16, 0x00, 0xC0,
            "Turbo boost pressure control", "N75", 0.01, 0.0, 1.0, 0.0, 1.0, 0.0,
            "", "Engine speed", "Upper limit", "", "rpm");
        
        // 4 bytes - MAP linearization (1x2)
        add_pattern!("MAP linearization", 4, 1, 2, 0x00, 0x00,
            "MAP sensor", "MAP", 1.0, 0.0, 1.0, 0.0, 1.0, 0.0,
            "", "", "MAP value", "", "");
        
        // 4 bytes - Idle RPM (1x2) - Y axis is coolant temp with ID 0xC1
        add_pattern!("Idle RPM", 4, 1, 2, 0x00, 0xC1,
            "Misc", "Idle", 1.0, 0.0, 1.0, 0.0, 0.1, -273.0,
            "", "Coolant temperature", "Idle RPM", "", "°C");
        
        // 12 bytes - Selector for injector duration (1x6)
        add_pattern!("Selector for injector duration", 12, 1, 6, 0x00, 0xEA,
            "Fuel", "Injection", 0.003906, 0.0, 1.0, 0.0, -0.023437, 78.0,
            "", "SOI", "Selector", "", "SOI");
        
        // 2 bytes - Single value maps (1x1)
        // REMOVED: Generic patterns for SVBL, SVRL, and switches cause too many false positives
        // These are now detected via dedicated functions:
        // - SVBL: find_svbl_sequence (hardcoded addresses)
        // - SVRL: find_svrl_sequence (byte sequence)
        // - Switches: find_left_foot_brake_switch, find_map_maf_switch (byte sequences)
        
        // 128 bytes - MAF correction by temperature (8x8)
        // DISABLED - Now detected in mod.rs with proper temperature axis validation
        // add_pattern!("MAF correction by temperature", 128, 8, 8, 0xEC, 0xC1,
        //     "Misc", "MAF", 0.01, 0.0, 0.1, -273.1, 1.0, 0.0,
        //     "Intake air temperature", "Engine speed", "Correction", "°C", "rpm");
        
        // 144 bytes - Fuel volume correction map (8x9)
        // DISABLED - Now detected in mod.rs with correct axis swap and dimensions
        // add_pattern!("Fuel volume correction map", 144, 8, 9, 0xEC, 0xC0,
        //     "Fuel", "Correction", 0.002441, 0.0, 0.01, 0.0, 1.0, 0.0,
        //     "IQ", "Engine speed", "IQ correction", "mg/st", "rpm");
        

        patterns
    }
    
}
