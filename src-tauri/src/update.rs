// GitHub-releases update system.
//
// The app queries the latest release of GITHUB_REPO and compares its tag
// with the running version (tauri.conf.json / Cargo.toml version). Works
// unauthenticated: the repository — or at least its releases — must be
// PUBLIC on GitHub. While the repo is private the check returns a network
// error (surfaced on the manual button, silent for background checks).
//
// Publishing a release = create a GitHub release tagged `vX.Y.Z` with the
// NSIS installers attached as assets: `ZedSuite_X.Y.Z_x64-setup.exe` AND
// `ZedSuite_X.Y.Z_x86-setup.exe` (32-bit). Upload the x64 one FIRST: the
// 1.0.0 x64 clients shipped with an arch-blind picker that takes the first
// *setup* .exe of the list. The updater downloads the asset matching its
// own architecture into the temp dir, launches it and exits the app.

use serde::Serialize;
use std::io::Write;
use tauri::{Emitter, Manager};

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
fn parse_version(v: &str) -> (u64, u64, u64) {
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

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("http client: {e}"))
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

    // Pick the installer asset for THIS build's architecture — an x86 app
    // must NEVER launch the x64 installer (it cannot run on 32-bit Windows):
    //  - build x64 : ignore les assets x86, sinon logique historique
    //    (premier .exe, préférence *setup*) ;
    //  - build x86 : uniquement un .exe marqué x86/i686.
    let want_x86 = cfg!(target_arch = "x86");
    let mut download_url: Option<String> = None;
    if let Some(assets) = json["assets"].as_array() {
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
            if download_url.is_none() || name.contains("setup") {
                download_url = asset["browser_download_url"].as_str().map(String::from);
            }
            if name.contains("setup") {
                break;
            }
        }
    }

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
/// events), launches it and exits the app so NSIS can replace the files.
#[tauri::command]
pub async fn download_and_install_update(
    app: tauri::AppHandle,
    url: String,
    version: String,
) -> Result<(), String> {
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
    let path = std::env::temp_dir().join(format!("ZedSuite-setup-{safe_version}.exe"));

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

    log::warn!("[update] launching installer: {}", path.display());
    std::process::Command::new(&path)
        .spawn()
        .map_err(|e| format!("installer launch: {e}"))?;

    app.exit(0);
    Ok(())
}
