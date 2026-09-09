// ZedSuite desktop application
// The detection engine lives in `detector/` (one module per ECU manufacturer);
// `commands.rs` exposes it to the frontend through Tauri IPC commands.

pub mod commands;
pub mod detector;
pub mod models;
pub mod update;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Warn)
                .build(),
        )
        .setup(|app| {
            // Taille d'ouverture adaptée à l'écran : 95 % de la ZONE DE
            // TRAVAIL du moniteur, c'est-à-dire l'écran moins la barre des
            // tâches, plafonnée pour les grands écrans et centrée. La taille
            // fixe de tauri.conf.json était trop basse sur les portables 15"
            // à l'échelle 125 % ; le calcul précédent partait de l'écran
            // entier avec une marge de 14 % posée au jugé et un plancher de
            // 700 points, si bien qu'en 1024x768 la fenêtre passait sous la
            // barre des tâches. L'utilisateur reste libre de redimensionner.
            use tauri::{LogicalSize, Manager};
            if let Some(window) = app.get_webview_window("main") {
                if let Ok(Some(monitor)) = window.current_monitor() {
                    let scale = monitor.scale_factor();
                    let work = monitor.work_area().size.to_logical::<f64>(scale);
                    let width = (work.width * 0.95).min(1680.0).max(680.0);
                    let height = (work.height * 0.95).min(1120.0).max(600.0);
                    let _ = window.set_size(LogicalSize::new(width, height));
                    let _ = window.center();
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::identify_ecu,
            commands::detect_maps,
            commands::detector_version,
            commands::list_ecus,
            commands::save_binary_file,
            commands::open_project_dir,
            commands::projects_dir_size,
            commands::open_external_url,
            update::check_for_update,
            update::fetch_roadmap,
            update::download_and_install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ZedSuite");
}
