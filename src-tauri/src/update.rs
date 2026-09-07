// GitHub-releases update system.
//
// The app queries the latest release of GITHUB_REPO and compares its tag
// with the running version (tauri.conf.json / Cargo.toml version). Works
// unauthenticated: the repository — or at least its releases — must be
// PUBLIC on GitHub. While the repo is private the check returns a network
// error (surfaced on the manual button, silent for background checks).
//
// Publishing a release = create a GitHub release tagged `vX.Y.Z` with the
// installers attached as assets:
//  - Windows: `ZedSuite_X.Y.Z_x64-setup.exe` AND `ZedSuite_X.Y.Z_x86-setup.exe`
//    (32-bit). Upload the x64 one FIRST: the 1.0.0 x64 clients shipped with
//    an arch-blind picker that takes the first *setup* .exe of the list.
//    The updater downloads the asset matching its own architecture into the
//    temp dir, launches it and exits the app.
//  - macOS: `ZedSuite_X.Y.Z_macos-universal.dmg` for a first install and
//    `ZedSuite_X.Y.Z_macos-universal.app.tar.gz` for the updater, which
//    extracts the archive, swaps the `.app` in place and relaunches it (see
//    the `macos` module below).

use serde::Serialize;
use std::io::Write;
use tauri::Emitter;

const GITHUB_REPO: &str = "LeZed97/ZedSuite";

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_notes: String,
    pub download_url: Option<String>,
    pub release_url: String,
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// Parse "v1.2.3" / "1.2.3" into a comparable triple (missing parts = 0).
pub fn parse_version(v: &str) -> (u64, u64, u64) {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let mut parts = v
        .split(['.', '-', '+'])
        .map(|p| p.parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

/// Which release asset a build installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateTarget {
    WindowsX64,
    WindowsX86,
    MacOS,
}

impl UpdateTarget {
    /// Target of the running build.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            UpdateTarget::MacOS
        } else if cfg!(target_arch = "x86") {
            UpdateTarget::WindowsX86
        } else {
            UpdateTarget::WindowsX64
        }
    }

    /// "x64" | "x86" | "macos" (test bench and probes).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "x64" => Some(UpdateTarget::WindowsX64),
            "x86" => Some(UpdateTarget::WindowsX86),
            "macos" | "mac" | "darwin" => Some(UpdateTarget::MacOS),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            UpdateTarget::WindowsX64 => "x64",
            UpdateTarget::WindowsX86 => "x86",
            UpdateTarget::MacOS => "macos",
        }
    }
}

/// Picks the asset a build must download among the `assets` array of a
/// GitHub release (objects with `name` and `browser_download_url`).
/// Returns (asset name, download url).
pub fn pick_asset(assets: &[serde_json::Value], target: UpdateTarget) -> Option<(String, String)> {
    let mut picked: Option<(String, String)> = None;
    match target {
        // An x86 app must NEVER launch the x64 installer (it cannot run on
        // 32-bit Windows):
        //  - build x64 : ignore les assets x86, sinon logique historique
        //    (premier .exe, préférence *setup*) ;
        //  - build x86 : uniquement un .exe marqué x86/i686.
        UpdateTarget::WindowsX64 | UpdateTarget::WindowsX86 => {
            let want_x86 = target == UpdateTarget::WindowsX86;
            for asset in assets {
                let name = asset["name"].as_str().unwrap_or("").to_lowercase();
                if !name.ends_with(".exe") {
                    continue;
                }
                let is_x64 =
                    name.contains("x64") || name.contains("x86_64") || name.contains("amd64");
                let is_x86 = !is_x64 && (name.contains("x86") || name.contains("i686"));
                if want_x86 != is_x86 {
                    continue;
                }
                if picked.is_none() || name.contains("setup") {
                    picked = asset["browser_download_url"]
                        .as_str()
                        .map(|u| (name.clone(), u.to_string()));
                }
                if name.contains("setup") {
                    break;
                }
            }
        }
        // Archive de l'app (`.app.tar.gz`) : l'universelle d'abord, sinon
        // celle de l'architecture de ce build (une Intel tourne aussi sur
        // Apple Silicon via Rosetta, l'inverse non). Le `.dmg` est réservé
        // à la première installation à la main.
        UpdateTarget::MacOS => {
            let want_arm = cfg!(target_arch = "aarch64");
            let mut best_score = 0u8;
            for asset in assets {
                let name = asset["name"].as_str().unwrap_or("").to_lowercase();
                if !(name.ends_with(".tar.gz") || name.ends_with(".tgz")) {
                    continue;
                }
                if !(name.contains("macos") || name.contains("darwin") || name.contains(".app.")) {
                    continue;
                }
                let score = if name.contains("universal") {
                    3
                } else if name.contains("aarch64") || name.contains("arm64") {
                    if want_arm {
                        2
                    } else {
                        0
                    }
                } else if name.contains("x86_64") || name.contains("x64") || name.contains("intel") {
                    if want_arm {
                        1
                    } else {
                        2
                    }
                } else {
                    1
                };
                if score > best_score {
                    if let Some(url) = asset["browser_download_url"].as_str() {
                        best_score = score;
                        picked = Some((name.clone(), url.to_string()));
                    }
                }
            }
        }
    }
    picked
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client: {e}"))
}

/// Feuille de route publique : ROADMAP.md du dépôt (anglais) et ses
/// traductions ROADMAP.<fr|es|it|de>.md, lues sur GitHub (branche master)
/// pour être à jour sans nouvelle version ; copies embarquées à la
/// compilation en repli hors ligne. Affichée dans la langue de l'app.
const ROADMAP_RAW_BASE: &str = "https://raw.githubusercontent.com/LeZed97/ZedSuite/master/";

fn roadmap_file_name(lang: &str) -> &'static str {
    match lang.to_ascii_lowercase().as_str() {
        "fr" => "ROADMAP.fr.md",
        "es" => "ROADMAP.es.md",
        "it" => "ROADMAP.it.md",
        "de" => "ROADMAP.de.md",
        _ => "ROADMAP.md",
    }
}

fn bundled_roadmap(lang: &str) -> &'static str {
    match lang.to_ascii_lowercase().as_str() {
        "fr" => include_str!("../../ROADMAP.fr.md"),
        "es" => include_str!("../../ROADMAP.es.md"),
        "it" => include_str!("../../ROADMAP.it.md"),
        "de" => include_str!("../../ROADMAP.de.md"),
        _ => include_str!("../../ROADMAP.md"),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RoadmapContent {
    pub markdown: String,
    /// "github" (à jour) ou "bundled" (copie livrée avec cette version)
    pub source: String,
}

async fn fetch_roadmap_online(lang: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let url = format!("{ROADMAP_RAW_BASE}{}", roadmap_file_name(lang));
    let res = client
        .get(&url)
        .header("User-Agent", "ZedSuite")
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?
        .error_for_status()
        .map_err(|e| format!("http: {e}"))?;
    res.text().await.map_err(|e| format!("body: {e}"))
}

#[tauri::command]
pub async fn fetch_roadmap(lang: String) -> Result<RoadmapContent, String> {
    match fetch_roadmap_online(&lang).await {
        Ok(md) if md.trim_start().starts_with('#') => Ok(RoadmapContent {
            markdown: md,
            source: "github".to_string(),
        }),
        Ok(_) | Err(_) => Ok(RoadmapContent {
            markdown: bundled_roadmap(&lang).to_string(),
            source: "bundled".to_string(),
        }),
    }
}

#[tauri::command]
pub async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let current_version = app.package_info().version.to_string();
    let releases_page = format!("https://github.com/{GITHUB_REPO}/releases");

    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let res = http_client()?
        .get(&url)
        .header("User-Agent", "ZedSuite-Updater")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("network: {e}"))?;

    // 404 = no published release yet (or repo still private): not an update
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(UpdateInfo {
            update_available: false,
            current_version: current_version.clone(),
            latest_version: current_version,
            release_notes: String::new(),
            download_url: None,
            release_url: releases_page,
        });
    }
    if !res.status().is_success() {
        return Err(format!("github api: HTTP {}", res.status()));
    }

    let json: serde_json::Value = res.json().await.map_err(|e| format!("github api: {e}"))?;

    let latest_version = json["tag_name"].as_str().unwrap_or_default().to_string();
    if latest_version.is_empty() {
        return Err("github api: release without tag_name".to_string());
    }
    let release_notes = json["body"].as_str().unwrap_or_default().to_string();
    let release_url = json["html_url"]
        .as_str()
        .map(String::from)
        .unwrap_or(releases_page);

    // Asset de CE build (architecture Windows ou archive macOS)
    let download_url = json["assets"]
        .as_array()
        .and_then(|assets| pick_asset(assets, UpdateTarget::current()))
        .map(|(_, url)| url);

    let update_available = parse_version(&latest_version) > parse_version(&current_version);
    log::warn!(
        "[update] current={current_version} latest={latest_version} available={update_available}"
    );

    Ok(UpdateInfo {
        update_available,
        current_version,
        latest_version,
        release_notes,
        download_url,
        release_url,
    })
}

/// Downloads the installer to the temp dir (emitting `update-download-progress`
/// events), installs it and exits the app:
///  - Windows: launches the NSIS installer, which replaces the files;
///  - macOS: swaps the `.app` bundle in place and reopens it once this
///    process has exited.
#[tauri::command]
pub async fn download_and_install_update(
    app: tauri::AppHandle,
    url: String,
    version: String,
) -> Result<(), String> {
    // Avant tout téléchargement : l'app doit tourner depuis un dossier où
    // elle peut se remplacer (pas depuis l'image disque ni un dossier
    // temporaire) — sinon le message invite à la glisser dans Applications.
    #[cfg(target_os = "macos")]
    let bundle = macos::installed_bundle().map_err(|e| e.to_string())?;

    let mut res = http_client()?
        .get(&url)
        .header("User-Agent", "ZedSuite-Updater")
        .send()
        .await
        .map_err(|e| format!("download: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("download: HTTP {}", res.status()));
    }

    let total = res.content_length();
    let safe_version: String = version
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-')
        .collect();
    let file_name = if cfg!(target_os = "macos") {
        format!("ZedSuite-update-{safe_version}.app.tar.gz")
    } else {
        format!("ZedSuite-setup-{safe_version}.exe")
    };
    let path = std::env::temp_dir().join(file_name);

    let mut file =
        std::fs::File::create(&path).map_err(|e| format!("temp file: {e}"))?;
    let mut downloaded: u64 = 0;
    while let Some(chunk) = res.chunk().await.map_err(|e| format!("download: {e}"))? {
        file.write_all(&chunk).map_err(|e| format!("temp file: {e}"))?;
        downloaded += chunk.len() as u64;
        let _ = app.emit("update-download-progress", DownloadProgress { downloaded, total });
    }
    file.flush().map_err(|e| format!("temp file: {e}"))?;
    drop(file);

    #[cfg(target_os = "windows")]
    {
        log::warn!("[update] launching installer: {}", path.display());
        // Options de l'installateur NSIS de Tauri :
        //   /P       mode passif : aucune page, seulement la barre de progression,
        //            une instance encore ouverte est fermée sans question ;
        //   /UPDATE  mise à jour par-dessus la version en place, sans la page
        //            « désinstaller la version précédente » (réglages conservés) ;
        //   /R       relance l'app une fois l'installation terminée.
        std::process::Command::new(&path)
            .args(["/P", "/UPDATE", "/R"])
            .spawn()
            .map_err(|e| format!("installer launch: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        log::warn!("[update] installing {} into {}", path.display(), bundle.display());
        let installed = macos::install_from_archive(&path, &bundle, &safe_version);
        let _ = std::fs::remove_file(&path);
        installed.map_err(|e| e.to_string())?;
        // Relance par un petit script qui attend la fin de ce processus,
        // puis nettoie l'ancienne version gardée en secours.
        macos::relaunch_after_exit(&bundle, std::process::id())
            .map_err(|e| format!("relaunch: {e}"))?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = std::fs::remove_file(&path);
        return Err("update: unsupported platform".to_string());
    }

    #[allow(unreachable_code)]
    {
        app.exit(0);
        Ok(())
    }
}

/// macOS side of the updater: an app is a `.app` folder, there is no
/// installer. The archive published with each release is extracted next to
/// the running bundle (same volume, so the swap is a rename), checked, then
/// swapped in; the previous bundle is kept as `.ZedSuite-previous.app` until
/// the new one has been reopened. Files written by the app itself carry no
/// quarantine flag, so Gatekeeper does not step in on the relaunch — the
/// "Open anyway" step exists only for the first manual install.
#[cfg(target_os = "macos")]
pub mod macos {
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};

    /// Name of the app bundle inside the archive and in Applications.
    pub const BUNDLE_NAME: &str = "ZedSuite.app";
    const STAGING_DIR: &str = ".ZedSuite-update";
    const BACKUP_DIR: &str = ".ZedSuite-previous.app";

    /// Surfaced to the frontend as `macos:<code>` and translated there.
    #[derive(Debug)]
    pub enum InstallError {
        /// Running from the disk image, from a temporary folder (App
        /// Translocation) or not from a `.app`: nothing can be replaced.
        NotInApplications,
        /// Extraction or swap failed (details for the log / error line).
        InstallFailed(String),
    }

    impl std::fmt::Display for InstallError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                InstallError::NotInApplications => write!(f, "macos:not_in_applications"),
                InstallError::InstallFailed(msg) => write!(f, "macos:install_failed: {msg}"),
            }
        }
    }

    impl From<std::io::Error> for InstallError {
        fn from(e: std::io::Error) -> Self {
            InstallError::InstallFailed(e.to_string())
        }
    }

    /// The `.app` this process runs from, if it can replace itself there.
    pub fn installed_bundle() -> Result<PathBuf, InstallError> {
        let exe = std::env::current_exe()?;
        bundle_of(&exe)
    }

    /// `<Bundle>.app/Contents/MacOS/<exe>` → `<Bundle>.app`, refused when the
    /// bundle sits on a mounted image (`/Volumes/…`) or in the read-only
    /// folder macOS uses for apps never dragged into Applications.
    pub fn bundle_of(exe: &Path) -> Result<PathBuf, InstallError> {
        let bundle = exe
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .ok_or(InstallError::NotInApplications)?;
        let is_app = bundle.extension().map_or(false, |e| e == "app");
        let text = bundle.to_string_lossy();
        if !is_app || text.starts_with("/Volumes/") || text.contains("/AppTranslocation/") {
            return Err(InstallError::NotInApplications);
        }
        Ok(bundle.to_path_buf())
    }

    fn run(cmd: &mut Command, what: &str) -> Result<(), InstallError> {
        let out = cmd
            .stdin(Stdio::null())
            .output()
            .map_err(|e| InstallError::InstallFailed(format!("{what}: {e}")))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(InstallError::InstallFailed(format!("{what}: {err}")));
        }
        Ok(())
    }

    fn find_app(dir: &Path) -> Result<PathBuf, InstallError> {
        for entry in std::fs::read_dir(dir)?.flatten() {
            let p = entry.path();
            if p.is_dir() && p.extension().map_or(false, |e| e == "app") {
                return Ok(p);
            }
        }
        Err(InstallError::InstallFailed(
            "archive without a .app bundle".to_string(),
        ))
    }

    fn shell_quote(p: &Path) -> String {
        format!("'{}'", p.to_string_lossy().replace('\'', "'\\''"))
    }

    /// Replaces `bundle` with the `.app` contained in `archive` (tar.gz)
    /// whose Info.plist must carry `version`. Pure file work, no Tauri
    /// handle: exercised as is by the CI test on a real macOS runner.
    pub fn install_from_archive(
        archive: &Path,
        bundle: &Path,
        version: &str,
    ) -> Result<(), InstallError> {
        let parent = bundle.parent().ok_or(InstallError::NotInApplications)?;

        // 1. Staging folder next to the bundle (same volume: the swap is an
        //    atomic rename), or in the temp dir when Applications belongs to
        //    an administrator — the swap then goes through the system
        //    password prompt.
        let local_staging = parent.join(STAGING_DIR);
        let _ = std::fs::remove_dir_all(&local_staging);
        let parent_writable = match std::fs::create_dir_all(&local_staging) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => false,
            Err(e) => return Err(e.into()),
        };
        let staging = if parent_writable {
            local_staging
        } else {
            let tmp = std::env::temp_dir().join(STAGING_DIR);
            let _ = std::fs::remove_dir_all(&tmp);
            std::fs::create_dir_all(&tmp)?;
            tmp
        };

        // 2. Extraction by the system tar: permissions, symlinks and the
        //    executable bit are kept as they were bundled.
        run(
            Command::new("/usr/bin/tar")
                .arg("-xzf")
                .arg(archive)
                .arg("-C")
                .arg(&staging),
            "extract",
        )?;
        let new_app = find_app(&staging)?;

        // 3. Sanity: the binary is there and Info.plist carries the version
        //    the dialog announced.
        let exe = new_app.join("Contents").join("MacOS").join("ZedSuite");
        if !exe.is_file() {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(InstallError::InstallFailed(
                "archive without the app binary".to_string(),
            ));
        }
        let plist = std::fs::read_to_string(new_app.join("Contents").join("Info.plist"))?;
        let wanted = format!("<string>{}</string>", version.trim_start_matches(['v', 'V']));
        if !plist.contains(&wanted) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(InstallError::InstallFailed(format!(
                "archive is not version {version}"
            )));
        }

        // 4. Files this process wrote carry no quarantine flag; clear any
        //    just in case (best effort).
        let _ = Command::new("/usr/bin/xattr")
            .arg("-cr")
            .arg(&new_app)
            .stdin(Stdio::null())
            .output();

        // 5. Swap. The previous bundle is kept as a backup: the process still
        //    runs from it until the relaunch script has reopened the new one.
        let backup = parent.join(BACKUP_DIR);
        if parent_writable {
            let _ = std::fs::remove_dir_all(&backup);
            std::fs::rename(bundle, &backup)?;
            if let Err(e) = std::fs::rename(&new_app, bundle) {
                let _ = std::fs::rename(&backup, bundle);
                let _ = std::fs::remove_dir_all(&staging);
                return Err(InstallError::InstallFailed(format!("swap: {e}")));
            }
        } else {
            // Applications owned by an administrator: same swap, run by
            // macOS after its password prompt (the user can cancel it).
            let script = format!(
                "rm -rf {b} && mv {cur} {b} && mv {new} {cur} && rm -rf {b} {st}",
                b = shell_quote(&backup),
                cur = shell_quote(bundle),
                new = shell_quote(&new_app),
                st = shell_quote(&staging),
            );
            let osa = format!(
                "do shell script \"{}\" with administrator privileges",
                script.replace('\\', "\\\\").replace('"', "\\\"")
            );
            let escalated = run(Command::new("/usr/bin/osascript").arg("-e").arg(&osa), "swap");
            if escalated.is_err() {
                let _ = std::fs::remove_dir_all(&staging);
            }
            escalated?;
        }

        // Finder / LaunchServices notice the new bundle
        let _ = Command::new("/usr/bin/touch")
            .arg(bundle)
            .stdin(Stdio::null())
            .output();
        Ok(())
    }

    /// Reopens `bundle` once the process `pid` has exited, then removes the
    /// backup and staging folders left by `install_from_archive`.
    pub fn relaunch_after_exit(bundle: &Path, pid: u32) -> std::io::Result<()> {
        let parent = bundle.parent().unwrap_or(bundle);
        let script = format!(
            "while kill -0 {pid} 2>/dev/null; do sleep 0.2; done; \
             open {b}; sleep 3; rm -rf {backup} {staging}",
            b = shell_quote(bundle),
            backup = shell_quote(&parent.join(BACKUP_DIR)),
            staging = shell_quote(&parent.join(STAGING_DIR)),
        );
        Command::new("/bin/sh")
            .arg("-c")
            .arg(&script)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
    }
}
