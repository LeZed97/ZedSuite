// ZedSuite desktop application
// The detection engine lives in `detector/` (one module per ECU manufacturer);
// `commands.rs` exposes it to the frontend through Tauri IPC commands.

pub mod commands;
pub mod detector;
pub mod models;
pub mod update;


/// Vrai quand le DMI annonce un hyperviseur courant (VMware, VirtualBox,
/// QEMU/KVM, Hyper-V, Parallels). Lecture de /sys, sans dépendance.
#[cfg(target_os = "linux")]
fn linux_running_in_vm() -> bool {
    let read = |p: &str| std::fs::read_to_string(p).unwrap_or_default().to_lowercase();
    let id = format!(
        "{} {} {}",
        read("/sys/class/dmi/id/sys_vendor"),
        read("/sys/class/dmi/id/product_name"),
        read("/sys/class/dmi/id/board_vendor")
    );
    ["vmware", "virtualbox", "innotek", "qemu", "kvm", "microsoft corporation", "parallels", "bochs"]
        .iter()
        .any(|m| id.contains(m))
}
pub fn run() {
    // Linux : WebKitGTK (2.42 et plus) compose la page via DMA-BUF ; sur un
    // GPU sans DRI3 (machine virtuelle, pilote NVIDIA propriétaire) la
    // fenêtre reste uniformément sombre, sans aucune interface. Constaté dans
    // la VM Ubuntu de test, réglé dès que cette variable est posée (c'est le
    // contournement recommandé pour les apps Tauri). L'utilisateur garde la
    // main s'il l'a définie lui-même.
    #[cfg(target_os = "linux")]
    {
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
        // Machine virtuelle : le pilote noyau du GPU virtuel (vmwgfx sous
        // VMware) laissait le processus web de WebKit bloqué pour toujours
        // dans une attente de synchronisation GPU (dma_fence) — page affichée
        // mais sourde à la souris et au clavier. Mesa en rendu logiciel
        // (llvmpipe) contourne le pilote : WebGL et fond animé restent
        // disponibles, plus lentement. Jamais appliqué sur une machine
        // physique.
        if std::env::var_os("LIBGL_ALWAYS_SOFTWARE").is_none() && linux_running_in_vm() {
            std::env::set_var("LIBGL_ALWAYS_SOFTWARE", "1");
        }
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
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
