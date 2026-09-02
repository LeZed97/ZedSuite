//! Banc d'essai du système de mise à jour (src/update.rs) SANS publier de
//! release : rejoue la même requête GitHub, le même parsing, le même choix
//! d'asset et le même téléchargement streamé que `check_for_update` /
//! `download_and_install_update` — mais n'exécute PAS l'installeur.
//!
//! Usage : cargo run --example update_probe -- <owner/repo> [version_courante] [x64|x86]

use std::io::Write;

/// Copie conforme de update.rs::parse_version.
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

fn main() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
        .block_on(run());
}

async fn run() {
    let args: Vec<String> = std::env::args().collect();
    let repo = args.get(1).cloned().unwrap_or_else(|| "LeZed97/ZedSuite".to_string());
    let current_version = args.get(2).cloned().unwrap_or_else(|| "1.0.0".to_string());
    // Simule l'architecture du client (défaut : celle de la compilation)
    let want_x86 = match args.get(3).map(String::as_str) {
        Some("x86") => true,
        Some("x64") => false,
        _ => cfg!(target_arch = "x86"),
    };

    // Auto-tests parse_version (mêmes règles que update.rs)
    assert_eq!(parse_version("v1.0.0"), (1, 0, 0));
    assert_eq!(parse_version("1.0.0"), (1, 0, 0));
    assert!(parse_version("v1.0.1") > parse_version("1.0.0"));
    assert!(parse_version("v1.1.0") > parse_version("1.0.10"));
    assert!(parse_version("v2.0") > parse_version("1.9.9"));
    assert!(!(parse_version("v1.0.0") > parse_version("1.0.0")));
    println!("parse_version: OK (v1.0.0==1.0.0, 1.0.1>1.0.0, 1.1.0>1.0.10, 2.0>1.9.9)");

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

    // Choix de l'asset : même boucle (par architecture) que update.rs
    let mut download_url: Option<String> = None;
    let mut picked_name = String::new();
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
                picked_name = name.clone();
            }
            if name.contains("setup") {
                break;
            }
        }
    }
    println!("architecture simulée: {}", if want_x86 { "x86" } else { "x64" });

    let update_available = parse_version(&latest_version) > parse_version(&current_version);
    println!("tag: {latest_version} | courante: {current_version} | update_available: {update_available}");
    println!("notes: {} caractères | page: {release_url}", release_notes.len());
    match &download_url {
        Some(u) => println!("asset choisi: {picked_name}\n  -> {u}"),
        None => {
            println!("AUCUN asset .exe trouvé — la fenêtre afficherait « installeur indisponible »");
            return;
        }
    }

    // Téléchargement streamé (comme download_and_install_update), SANS lancer l'exe
    let mut res = client
        .get(download_url.as_deref().unwrap())
        .header("User-Agent", "ZedSuite-Updater")
        .send()
        .await
        .expect("download");
    assert!(res.status().is_success(), "download: HTTP {}", res.status());
    let total = res.content_length();
    let path = std::env::temp_dir().join("zedsuite-update-probe.exe");
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
