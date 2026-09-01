// Tauri IPC commands — the desktop equivalent of the old map-detector HTTP API.
// Request/response JSON shapes are kept identical to the web service so the
// frontend detection code keeps working unchanged.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::detector::{ECUIdentifier, MapDetector};
use crate::models::{DetectMapsResponse, DetectedMap, ExpectedMapStatus};

fn decode_base64(data: &str) -> Result<Vec<u8>, String> {
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| format!("Invalid base64: {}", e))
}

#[derive(Debug, Serialize)]
pub struct IdentifyEcuResponse {
    pub manufacturer: String,
    pub ecu_type: String,
    pub variant: Option<String>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub part_number: Option<String>,
    pub confidence: f32,
}

/// Identify the ECU family of a binary dump.
/// Mirrors POST /api/v1/identify-ecu of the old web service.
#[tauri::command]
pub fn identify_ecu(
    file_data_base64: String,
    file_name: String,
) -> Result<IdentifyEcuResponse, String> {
    let data = decode_base64(&file_data_base64)?;
    log::warn!("🔍 [IDENTIFY-ECU] file: {} ({} bytes)", file_name, data.len());

    let ecu_id = ECUIdentifier::identify(&data);

    log::warn!(
        "✅ [IDENTIFY-ECU] Result: {:?} {:?} ({:.0}%)",
        ecu_id.manufacturer,
        ecu_id.ecu_type,
        ecu_id.confidence * 100.0
    );

    Ok(IdentifyEcuResponse {
        manufacturer: ecu_id.manufacturer.display_name().to_string(),
        ecu_type: format!("{:?}", ecu_id.ecu_type),
        variant: ecu_id.variant,
        software_version: ecu_id.software_version,
        hardware_version: ecu_id.hardware_version,
        part_number: ecu_id.part_number,
        confidence: ecu_id.confidence,
    })
}

/// Version du moteur de détection.
///
/// À INCRÉMENTER dès qu'une modification change les résultats produits :
/// nouvelles cartes reconnues, adresses, dimensions, facteurs de conversion,
/// libellés d'axes… Les projets détectés avec une version antérieure sont
/// re-scannés automatiquement à l'ouverture, ce qui évite de laisser des
/// projets sur des données périmées.
///
/// Historique :
///   1 — état initial (versions antérieures au suivi)
///   2 — EDC15VM : facteurs corrigés (EGR hysteresis, MAF, SOI, pompe N146),
///       axe Y des cartes SOI, libellés/facteurs/description de tous les axes
///   3 — EDC15VM : descriptions détaillées « grandeur | X: nom (unité) | … »
///       et réutilisation des patterns EDC15P sur identifiants d'axes communs
///   4 — EDC15VM : « MAF correction by temperature » nommée et rangée dans
///       « Other », comme sur EDC15P
///   5 — EDC15VM : une seule « EGR hysteresis » par codeblock, et scan borné
///       à la région de chaque codeblock (fichiers multimap)
///   6 — EDC15VM : orientation des cartes du balayage rétablie (lignes et
///       colonnes étaient interverties, les valeurs sortaient entrelacées)
///   7 — EDC15VM : « Inverse driver wish » retirée (absente de l'EDC15P) et
///       « EGR 01 » renommée « EGR » ; EDC15P : dossier « Engine torque
///       request » renommé « Engine fuel request »
pub const DETECTOR_VERSION: u32 = 30;

/// Version du moteur de détection, pour comparaison avec celle enregistrée
/// dans un projet.
#[tauri::command]
pub fn detector_version() -> u32 {
    DETECTOR_VERSION
}

#[derive(Debug, Deserialize)]
pub struct DetectMapsArgs {
    pub file_data_base64: String,
    pub file_name: String,
    pub ecu_type: Option<String>,
    #[serde(default)]
    pub tuned_mode: bool,
}

/// Run map detection on a binary dump.
/// Mirrors POST /api/v1/detect of the old web service.
#[tauri::command]
pub fn detect_maps(request: DetectMapsArgs) -> Result<DetectMapsResponse, String> {
    let start = Instant::now();
    let data = decode_base64(&request.file_data_base64)?;

    log::warn!(
        "📥 Detection request for file: {} ({} bytes, tuned: {})",
        request.file_name,
        data.len(),
        request.tuned_mode
    );

    let detector = MapDetector::new();
    let maps: Vec<DetectedMap> =
        detector.detect_maps_with_options(&data, request.ecu_type.as_deref(), request.tuned_mode);

    let expected_maps = build_expected_report(request.ecu_type.as_deref(), &maps);

    let response = DetectMapsResponse {
        success: true,
        total_maps: maps.len(),
        maps,
        processing_time_ms: start.elapsed().as_millis(),
        file_size: data.len(),
        detector_version: DETECTOR_VERSION,
        expected_maps,
    };

    log::warn!(
        "✅ Detection complete: {} maps found in {}ms",
        response.total_maps,
        response.processing_time_ms
    );

    Ok(response)
}

/// Rapport de complétude EDC16 : familles de maps qui existent TOUJOURS dans
/// un fichier de cette famille (invariants métier). Un compte insuffisant
/// signale un fichier probablement déjà fortement modifié — l'app conseille
/// alors de charger le fichier d'origine. Les libellés reprennent les noms
/// de maps (anglais, comme partout).
fn build_expected_report(
    ecu_type: Option<&str>,
    maps: &[DetectedMap],
) -> Option<Vec<ExpectedMapStatus>> {
    let ecu = ecu_type.unwrap_or("").to_uppercase();
    if ecu.contains("EDC15P") {
        return build_expected_report_edc15p(maps);
    }
    if ecu.contains("EDC15VM") {
        return build_expected_report_edc15vm(maps);
    }
    if !ecu.contains("EDC16") {
        return None;
    }

    let count_starts = |prefix: &str| -> usize {
        maps.iter()
            .filter(|m| m.name.as_deref().map(|n| n.starts_with(prefix)).unwrap_or(false))
            .count()
    };

    // SOI numérotées ("Start of injection 00"…"Start of injection 09"…),
    // hors "(Dynamic)" : par paquet de 9, il faut un SOI Limiter et un
    // SOI Selector.
    let soi_numbered = maps
        .iter()
        .filter(|m| {
            m.name
                .as_deref()
                .map(|n| {
                    n.starts_with("Start of injection ")
                        && n["Start of injection ".len()..]
                            .chars()
                            .next()
                            .map(|c| c.is_ascii_digit())
                            .unwrap_or(false)
                })
                .unwrap_or(false)
        })
        .count();
    // Paquets de 10 (vérifié au banc : les comptes sont des multiples de 10)
    let soi_packs = if soi_numbered == 0 { 1 } else { ((soi_numbered + 5) / 10).max(1) };

    // (libellé, minimum attendu, préfixe de nom compté)
    // Calibré sur 319 fichiers (banc du 28/08) — « EGT base map » écartée :
    // absente pour de vrai sur ~1/3 des U34 sains et sur plusieurs U31.
    let rules: Vec<(&str, usize, &str)> = vec![
        // Groupe de driver wish (minimum observé sur fichiers sains : 7-8)
        ("Driver Wish (groupe)", 6, "Driver Wish"),
        ("Duration 00", 1, "Duration 00"),
        ("Duration 01", 1, "Duration 01"),
        ("Duration 02", 1, "Duration 02"),
        ("Duration 03", 1, "Duration 03"),
        ("Duration 04", 1, "Duration 04"),
        ("Duration 05", 1, "Duration 05"),
        ("Duration Selector", 1, "Duration Selector"),
        ("Torque Limiter", 1, "Torque Limiter"),
        ("Torque to IQ Conversion", 1, "Torque to IQ"),
        ("Cranking Torque Map", 2, "Cranking Torque Map"),
        ("Gearbox Torque Limiter", 5, "Gearbox Torque Limiter"),
        ("Single Value Gearbox Limiter", 1, "Single Value Gearbox Limiter"),
        ("Idle speed correction by engine temp", 1, "Idle speed correction by engine temp"),
        ("Single value idle speed", 1, "Single value idle speed"),
        ("MAP Linearisation", 1, "MAP Linearisation"),
        ("SVRL - RPM Limiter", 1, "SVRL"),
        ("Maximum Vehicle Speed", 2, "Maximum Vehicle Speed"),
        ("Smoke Limiter by MAF", 1, "Smoke Limiter by MAF"),
        ("Smoke Limiter by MAP", 1, "Smoke Limiter by MAP"),
        ("Smoke Limiter by Lambda", 1, "Smoke Limiter by Lambda"),
        ("EGR (Desired air quantity)", 1, "EGR (Desired air quantity)"),
        ("EGR hysteresis", 1, "EGR hysteresis"),
        ("Exhaust gas temp sensor linearisation", 1, "Exhaust gas temperature sensor linearisation"),
        ("Fuel correction by ambient pressure", 1, "Fuel correction by ambient pressure"),
        ("Fuel correction by fuel temperature", 1, "Fuel correction by fuel temperature"),
        ("Fuel correction by oil temperature", 1, "Fuel correction by oil temperature"),
        ("Fuel correction by exhaust gas temp", 1, "Fuel correction by exhaust gas temp"),
        ("Boost target map", 1, "Boost target map"),
        ("Boost limit map", 1, "Boost limit map"),
        ("SVBL (Single value boost limiter)", 1, "SVBL"),
        ("Boost correction by temperature", 1, "Boost correction by temperature"),
        // Corrélation « 1 N75 par Boost target » INFIRMÉE au banc (Eos et
        // G5 run trophy : 4 boost targets / 3 N75 ; l'inverse existe aussi).
        // Minimum observé sur tous les U31/U34 stock : 2.
        ("N75 duty cycle", 2, "N75 duty cycle"),
    ];

    // EDC16U1 : détecteur remis au niveau (banc du 28/08 : fenêtres 1 Mo,
    // normalisation des noms, T2IQ, smoke by MAP…). Restent exclues :
    //  - « Fuel correction by exhaust gas temp » : la famille n'existe pas
    //    en format facteur sur U1 (0/116 sur tout le corpus, structure
    //    absente vérifiée octet par octet) ;
    //  - N75 attendue à 1 (mono-instance sur les 1 Mo, pas de paire Bas/Bas2).
    let is_u1 = ecu.contains("EDC16U1") && !ecu.contains("U31") && !ecu.contains("U34");
    let u1_excluded: &[&str] = &["Fuel correction by exhaust gas temp"];
    // Moteurs SDI atmosphériques (Caddy 70ch, Golf 75ch) : pas de capteur
    // MAP ni de correction de boost par température — structures absentes,
    // vérifié octet par octet sur 03G906016N. Marqueur : la CO-absence des
    // deux sur un U1 (sur les TDI, la MAP linearization est toujours
    // détectée ; une absence isolée reste signalée).
    let sdi_marker = is_u1
        && count_starts("MAP Linearisation") == 0
        && count_starts("Boost correction by temperature") == 0;
    let sdi_excluded: &[&str] = &["MAP Linearisation", "Boost correction by temperature"];

    let mut report: Vec<ExpectedMapStatus> = rules
        .into_iter()
        .filter(|(label, _, _)| !is_u1 || !u1_excluded.contains(label))
        .filter(|(label, _, _)| !sdi_marker || !sdi_excluded.contains(label))
        .map(|(label, expected, prefix)| ExpectedMapStatus {
            label: label.to_string(),
            expected: if is_u1 && label == "N75 duty cycle" { 1 } else { expected },
            found: count_starts(prefix),
        })
        .collect();

    // Règles dynamiques : 1 SOI Limiter + 1 SOI Selector par paquet de 10
    // maps Start of injection numérotées. Les SDI atmosphériques (Caddy
    // 70ch 016GP/HN/N, Golf 75ch 016GJ/M) ont un paquet RÉDUIT de 2-6 maps
    // (vérifié : 016N = 4 maps 16x14/16x15 à 0xE89DC) — dans ce cas le
    // paquet trouvé vaut attendu, et pas de Selector (pas de bloc à 10).
    let sdi_pack = soi_numbered >= 2 && soi_numbered < 10;
    report.push(ExpectedMapStatus {
        label: if sdi_pack {
            "Start of injection (paquet réduit SDI)".to_string()
        } else {
            "Start of injection (paquet de 10)".to_string()
        },
        expected: if sdi_pack { soi_numbered } else { soi_packs * 10 },
        found: soi_numbered,
    });
    report.push(ExpectedMapStatus {
        label: "SOI Limiter (1 par paquet SOI)".to_string(),
        expected: soi_packs,
        found: count_starts("SOI Limiter"),
    });
    // Le sélecteur SOI n'existe pas sur les SDI atmosphériques (bloc de 10
    // + limiteur mais aucun sélecteur sur Golf 75ch/Caddy HN) — exemption
    // via le même marqueur SDI que MAP linearization/boost corr.
    if !sdi_pack && !sdi_marker {
        report.push(ExpectedMapStatus {
            label: "SOI Selector (1 par paquet SOI)".to_string(),
            expected: soi_packs,
            found: count_starts("SOI Selector"),
        });
    }

    Some(report)
}

/// Rapport de complétude EDC15P : invariants PAR CODEBLOCK (règles ZedPerf,
/// vérifiées au banc sur 28 fichiers / 71 codeblocks, tous à 100 %) —
/// chaque règle est agrégée sur l'ensemble des codeblocks détectés
/// (attendu = n_codeblocks × minimum par codeblock).
fn build_expected_report_edc15p(maps: &[DetectedMap]) -> Option<Vec<ExpectedMapStatus>> {
    use std::collections::HashSet;
    let codeblocks: HashSet<u32> = maps.iter().filter_map(|m| m.codeblock_id).collect();
    let n_cb = codeblocks.len().max(1);

    // Comptes limités aux maps AVEC codeblock (les passes globales — EGR
    // temperature map, Launch control — n'entrent pas dans les règles)
    let count = |mode: u8, pat: &str| -> usize {
        maps.iter()
            .filter(|m| m.codeblock_id.is_some())
            .filter(|m| {
                m.name.as_deref().map_or(false, |n| match mode {
                    0 => n == pat,
                    1 => n.starts_with(pat),
                    _ => n.contains(pat),
                })
            })
            .count()
    };

    // (libellé, minimum PAR CODEBLOCK, mode 0=exact/1=préfixe/2=contient, motif)
    let rules: Vec<(&str, usize, u8, &str)> = vec![
        ("EGR", 1, 0, "EGR"),
        ("Driver wish", 1, 1, "Driver wish"),
        ("Start IQ", 2, 1, "Start IQ"),
        ("Torque limiter", 1, 1, "Torque limiter"),
        ("Idle RPM", 2, 1, "Idle RPM"),
        ("Injector duration 00", 1, 1, "Injector duration 00"),
        ("Injector duration 01", 1, 1, "Injector duration 01"),
        ("Injector duration 02", 1, 1, "Injector duration 02"),
        ("Injector duration 03", 1, 1, "Injector duration 03"),
        ("Injector duration 04", 1, 1, "Injector duration 04"),
        ("Injector duration 05", 1, 1, "Injector duration 05"),
        ("Selector for injector duration", 1, 1, "Selector for injector duration"),
        ("MAP linearisation", 1, 1, "MAP linearisation"),
        ("SVRL - RPM Limiter", 1, 1, "SVRL"),
        ("Smoke limiter", 1, 1, "Smoke limiter"),
        ("IQ by MAP limiter", 1, 1, "IQ by MAP limiter"),
        ("IQ by MAF limiter", 1, 1, "IQ by MAF limiter"),
        ("MAP/MAF switch", 1, 1, "MAP/MAF switch"),
        ("Start of injection (paquet de 10)", 10, 1, "Start of injection (SOI)"),
        ("SOI limiter", 1, 1, "SOI limiter"),
        ("SVBL (Single value boost limiter)", 1, 1, "SVBL"),
        ("Boost target map", 1, 1, "Boost target map"),
        ("Boost limit map", 1, 1, "Boost limit map"),
        ("Boost correction by temperature", 1, 1, "Boost correction by temperature"),
        ("Limit of overboost protection", 1, 1, "Limit of overboost"),
        ("N75 duty cycle", 1, 1, "N75 duty cycle"),
        ("Boost actuator upper limit curve", 1, 1, "Boost actuator upper limit"),
        ("Expected fuel temperature", 1, 1, "Expected fuel temperature"),
        ("Fuel volume correction", 1, 1, "Fuel volume correction"),
        ("MAF correction by temperature", 1, 1, "MAF correction by temperature"),
        ("Left foot brake switch", 1, 1, "Left foot brake switch"),
        ("VCDS Diagnostic IQ Limit (10)", 10, 1, "VCDS Diagnostic IQ Limit"),
        ("VCDS Diagnostic MAP Limit (3)", 3, 1, "VCDS Diagnostic MAP Limit"),
        ("VCDS Diagnostic Torque Limit", 1, 0, "VCDS Diagnostic Torque Limit"),
        ("VCDS Diagnostic Display offsets (3)", 3, 2, "Display offset"),
    ];

    let report: Vec<ExpectedMapStatus> = rules
        .into_iter()
        .map(|(label, per_cb, mode, pat)| ExpectedMapStatus {
            label: label.to_string(),
            expected: per_cb * n_cb,
            found: count(mode, pat),
        })
        .collect();

    Some(report)
}

/// Rapport de complétude EDC15VM : invariants PAR CODEBLOCK (règles ZedPerf,
/// banc 11 fichiers). Deux générations de SW : multi-SOI (10 maps par
/// sélecteur C4) et mono-SOI (1-2 maps 14x7 avant le SOI limiter) — la
/// règle SOI demande donc ≥1 par codeblock. ⚠ « IQ by MAP limiter » et le
/// 2e « Start IQ » restent incertains sur la génération mono-SOI (en
/// attente d'une référence Stage X VM).
fn build_expected_report_edc15vm(maps: &[DetectedMap]) -> Option<Vec<ExpectedMapStatus>> {
    use std::collections::HashSet;
    let codeblocks: HashSet<u32> = maps.iter().filter_map(|m| m.codeblock_id).collect();
    let n_cb = codeblocks.len().max(1);

    let count = |mode: u8, pat: &str| -> usize {
        maps.iter()
            .filter(|m| m.codeblock_id.is_some())
            .filter(|m| {
                m.name.as_deref().map_or(false, |n| match mode {
                    0 => n == pat,
                    _ => n.starts_with(pat),
                })
            })
            .count()
    };

    // (libellé, minimum PAR CODEBLOCK, mode 0=exact/1=préfixe, motif)
    let rules: Vec<(&str, usize, u8, &str)> = vec![
        ("EGR", 1, 0, "EGR"),
        ("Driver wish", 1, 1, "Driver wish"),
        ("Torque limiter", 1, 1, "Torque limiter"),
        ("Idle RPM", 2, 1, "Idle RPM"),
        ("N146 Pump voltage", 1, 1, "N146"),
        ("MAP linearisation", 1, 1, "MAP linearisation"),
        ("SVRL - RPM Limiter", 1, 1, "SVRL"),
        ("IQ by MAP limiter", 1, 1, "IQ by MAP"),
        ("IQ by MAF limiter", 1, 1, "IQ by MAF"),
        ("MAP/MAF switch", 1, 1, "MAP/MAF switch"),
        ("Start of injection (>=1)", 1, 1, "Start of injection"),
        ("SVBL (Single value boost limiter)", 1, 1, "SVBL"),
        ("Boost target map", 1, 1, "Boost target map"),
        ("Boost limit map", 1, 1, "Boost limit map"),
        ("Boost correction by temperature", 1, 1, "Boost correction by temperature"),
        ("N75 duty cycle", 1, 1, "N75 duty cycle"),
        ("MAF correction by temperature", 1, 1, "MAF correction by temperature"),
        // Même séquence que l'EDC15P, présente sur tous les VM du banc
        ("Left foot brake switch", 1, 1, "Left foot brake switch"),
    ];

    let mut report: Vec<ExpectedMapStatus> = rules
        .into_iter()
        .map(|(label, per_cb, mode, pat)| ExpectedMapStatus {
            label: label.to_string(),
            expected: per_cb * n_cb,
            found: count(mode, pat),
        })
        .collect();

    // « SOI limiter (temp) » n'existe QUE sur la génération mono-SOI (réf
    // EDCSuite : le Seat leon multi-SOI n'en a pas) — la règle ne s'applique
    // que si le fichier n'a pas ses paquets de 10 SOI.
    let soi_total = count(1, "Start of injection");
    if soi_total < 10 * n_cb {
        report.push(ExpectedMapStatus {
            label: "SOI limiter".to_string(),
            expected: n_cb,
            found: count(1, "SOI limiter"),
        });
    }

    // « Start IQ » : 1 par FICHIER au minimum — la jetta n'en a qu'un en
    // tout (réf EDCSuite : cb2 seulement), le golf 1/cb, le Seat leon 2/cb.
    report.push(ExpectedMapStatus {
        label: "Start IQ".to_string(),
        expected: 1,
        found: count(1, "Start IQ"),
    });

    Some(report)
}

/// Save bytes to a user-chosen location via the native "Save as" dialog.
/// The webview cannot trigger browser downloads, so binary/mappack exports
/// go through this command. Returns false when the user cancels.
#[tauri::command]
pub async fn save_binary_file(
    app: tauri::AppHandle,
    default_name: String,
    data_base64: String,
) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;

    let data = decode_base64(&data_base64)?;

    // Extension attendue, déduite du nom proposé (.bin, .json, .pdf…). Elle
    // est garantie sur le fichier final : si l'utilisateur renomme dans la
    // boîte de dialogue sans la retaper, on la réapplique — sans ça un export
    // renommé perdait son .bin et n'était plus reconnu comme binaire.
    let expected_ext = std::path::Path::new(&default_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    let mut dialog = app.dialog().file().set_file_name(&default_name);
    if let Some(ext) = expected_ext.as_deref() {
        dialog = dialog.add_filter(format!("*.{}", ext), &[ext]);
    }
    let picked = dialog.blocking_save_file();

    match picked {
        Some(file_path) => {
            let mut path = file_path
                .into_path()
                .map_err(|e| format!("Invalid path: {}", e))?;
            if let Some(ext) = expected_ext.as_deref() {
                let already_has = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case(ext))
                    .unwrap_or(false);
                if !already_has {
                    let mut s = path.into_os_string();
                    s.push(format!(".{}", ext));
                    path = std::path::PathBuf::from(s);
                }
            }
            std::fs::write(&path, &data).map_err(|e| format!("Write failed: {}", e))?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Open a project's folder in the Windows Explorer.
#[tauri::command]
pub fn open_project_dir(app: tauri::AppHandle, file_id: String) -> Result<(), String> {
    use tauri::Manager;

    // Project ids are 15-char alphanumeric — reject anything else so a
    // crafted id can never escape the projects directory.
    if file_id.is_empty() || !file_id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Invalid project id".to_string());
    }

    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("projects")
        .join(&file_id);

    if !dir.is_dir() {
        return Err("Project folder not found".to_string());
    }

    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map_err(|e| format!("Failed to open Explorer: {}", e))?;
    Ok(())
}

/// List the supported ECUs (bundled ecus.json).
/// Mirrors GET /api/v1/ecus of the old web service.
#[tauri::command]
pub fn list_ecus() -> Result<serde_json::Value, String> {
    let ecus: serde_json::Value = serde_json::from_str(include_str!("../ecus.json"))
        .map_err(|e| format!("Invalid bundled ecus.json: {}", e))?;
    let total = ecus.as_array().map(|a| a.len()).unwrap_or(0);
    Ok(serde_json::json!({
        "ecus": ecus,
        "total": total,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
