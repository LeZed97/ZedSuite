//! Banc d'essai du système de mise à jour (src/update.rs) SANS publier de
//! release : rejoue la même requête GitHub, le même parsing, le même choix
//! d'asset (`pick_asset`, le code livré) et le même téléchargement streamé
//! que `check_for_update` / `download_and_install_update` — mais n'installe
//! rien.
//!
//! Usage : cargo run --example update_probe -- <owner/repo> [version_courante] [x64|x86|macos]

use std::io::Write;
use zedsuite_lib::update::{parse_version, pick_asset, UpdateTarget};

fn main() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(run());
}

fn asset(name: &str) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "browser_download_url": format!("https://example.invalid/{name}")
    })
}

/// Choix d'asset sur une release type (les trois plateformes publiées).
fn self_test_pick_asset() {
    let release = [
        asset("ZedSuite_1.2.0_x64-setup.exe"),
        asset("ZedSuite_1.2.0_x86-setup.exe"),
        asset("ZedSuite_1.2.0_macos-universal.dmg"),
        asset("ZedSuite_1.2.0_macos-universal.app.tar.gz"),
    ];
    let name = |t| pick_asset(&release, t).map(|(n, _)| n).unwrap_or_default();
    assert_eq!(name(UpdateTarget::WindowsX64), "zedsuite_1.2.0_x64-setup.exe");
    assert_eq!(name(UpdateTarget::WindowsX86), "zedsuite_1.2.0_x86-setup.exe");
    assert_eq!(name(UpdateTarget::MacOS), "zedsuite_1.2.0_macos-universal.app.tar.gz");

    // Release Windows seule : rien pour macOS (fenêtre « pas encore de build macOS »)
    let windows_only = [asset("ZedSuite_1.2.0_x64-setup.exe"), asset("ZedSuite_1.2.0_x86-setup.exe")];
    assert!(pick_asset(&windows_only, UpdateTarget::MacOS).is_none());
    // Une build x86 ne prend jamais l'installateur x64, et inversement
    let x64_only = [asset("ZedSuite_1.2.0_x64-setup.exe")];
    assert!(pick_asset(&x64_only, UpdateTarget::WindowsX86).is_none());
    // Le .dmg n'est jamais choisi par l'updater
    let dmg_only = [asset("ZedSuite_1.2.0_macos-universal.dmg")];
    assert!(pick_asset(&dmg_only, UpdateTarget::MacOS).is_none());
    // Archive par architecture, sans universelle : la bonne est prise
    let per_arch = [
        asset("ZedSuite_1.2.0_macos-x86_64.app.tar.gz"),
        asset("ZedSuite_1.2.0_macos-aarch64.app.tar.gz"),
    ];
    let picked = pick_asset(&per_arch, UpdateTarget::MacOS).map(|(n, _)| n).unwrap();
    assert!(picked.contains(if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" }));
    println!("pick_asset: OK (x64 / x86 / macOS, release Windows seule, dmg ignoré, par architecture)");
}

async fn run() {
    let args: Vec<String> = std::env::args().collect();
    let repo = args.get(1).cloned().unwrap_or_else(|| "LeZed97/ZedSuite".to_string());
    let current_version = args.get(2).cloned().unwrap_or_else(|| "1.0.0".to_string());
    // Simule la cible du client (défaut : celle de la compilation)
    let target = args
        .get(3)
        .and_then(|s| UpdateTarget::parse(s))
        .unwrap_or_else(UpdateTarget::current);

    // Auto-tests parse_version (mêmes règles que update.rs)
    assert_eq!(parse_version("v1.0.0"), (1, 0, 0));
    assert_eq!(parse_version("1.0.0"), (1, 0, 0));
    assert!(parse_version("v1.0.1") > parse_version("1.0.0"));
    assert!(parse_version("v1.1.0") > parse_version("1.0.10"));
    assert!(parse_version("v2.0") > parse_version("1.9.9"));
    assert!(!(parse_version("v1.0.0") > parse_version("1.0.0")));
    println!("parse_version: OK (v1.0.0==1.0.0, 1.0.1>1.0.0, 1.1.0>1.0.10, 2.0>1.9.9)");
    self_test_pick_asset();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("http client");

    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    println!("GET {url}");
    let res = client
        .get(&url)
        .header("User-Agent", "ZedSuite-Updater")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .expect("network");

    if res.status() == reqwest::StatusCode::NOT_FOUND {
        println!("HTTP 404 -> traité « à jour » (repo privé ou aucune release) : chemin OK");
        return;
    }
    assert!(res.status().is_success(), "github api: HTTP {}", res.status());

    let json: serde_json::Value = res.json().await.expect("json");
    let latest_version = json["tag_name"].as_str().unwrap_or_default().to_string();
    assert!(!latest_version.is_empty(), "release sans tag_name");
    let release_notes = json["body"].as_str().unwrap_or_default();
    let release_url = json["html_url"].as_str().unwrap_or_default();

    // Choix de l'asset : le code livré (update.rs::pick_asset)
    let picked = json["assets"]
        .as_array()
        .and_then(|assets| pick_asset(assets, target));
    println!("cible simulée: {}", target.label());

    let update_available = parse_version(&latest_version) > parse_version(&current_version);
    println!("tag: {latest_version} | courante: {current_version} | update_available: {update_available}");
    println!("notes: {} caractères | page: {release_url}", release_notes.len());
    let (picked_name, download_url) = match picked {
        Some(p) => {
            println!("asset choisi: {}\n  -> {}", p.0, p.1);
            p
        }
        None => {
            println!("AUCUN asset pour cette cible — la fenêtre afficherait « installeur indisponible »");
            return;
        }
    };

    // Téléchargement streamé (comme download_and_install_update), SANS installer
    let mut res = client
        .get(&download_url)
        .header("User-Agent", "ZedSuite-Updater")
        .send()
        .await
        .expect("download");
    assert!(res.status().is_success(), "download: HTTP {}", res.status());
    let total = res.content_length();
    let ext = if picked_name.ends_with(".exe") { "exe" } else { "tar.gz" };
    let path = std::env::temp_dir().join(format!("zedsuite-update-probe.{ext}"));
    let mut file = std::fs::File::create(&path).expect("temp file");
    let mut downloaded: u64 = 0;
    while let Some(chunk) = res.chunk().await.expect("chunk") {
        file.write_all(&chunk).expect("write");
        downloaded += chunk.len() as u64;
    }
    file.flush().expect("flush");
    println!(
        "téléchargement streamé: {} octets (annoncés: {:?}) -> {}",
        downloaded,
        total,
        path.display()
    );
    let _ = std::fs::remove_file(&path);
    println!("PROBE COMPLETE : toute la chaîne réseau/parse/choix d'asset/téléchargement fonctionne");
}
