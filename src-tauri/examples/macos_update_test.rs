//! CI bench for the macOS side of the updater (src/update.rs, `macos`
//! module), run on a real macOS runner by .github/workflows/macos.yml:
//! the same swap and relaunch script as the app's "Update now" button,
//! without the download.
//!
//! Usage: macos_update_test <archive.app.tar.gz> <installed .app> <version> [pid of the running old app]
//!
//! With a pid, the relaunch script is armed exactly as in the app: once
//! that process exits, the new bundle is reopened and the backup removed.

#[cfg(target_os = "macos")]
fn main() {
    use std::path::{Path, PathBuf};
    use zedsuite_lib::update::macos::{
        bundle_of, install_from_archive, relaunch_after_exit, InstallError,
    };

    // Where the app can replace itself, and where it must refuse to
    assert_eq!(
        bundle_of(Path::new("/Applications/ZedSuite.app/Contents/MacOS/ZedSuite")).unwrap(),
        PathBuf::from("/Applications/ZedSuite.app")
    );
    assert_eq!(
        bundle_of(Path::new("/Users/me/Applications/ZedSuite.app/Contents/MacOS/ZedSuite")).unwrap(),
        PathBuf::from("/Users/me/Applications/ZedSuite.app")
    );
    assert!(matches!(
        bundle_of(Path::new("/Volumes/ZedSuite/ZedSuite.app/Contents/MacOS/ZedSuite")),
        Err(InstallError::NotInApplications)
    ));
    assert!(matches!(
        bundle_of(Path::new(
            "/private/var/folders/ab/T/AppTranslocation/1234/d/ZedSuite.app/Contents/MacOS/ZedSuite"
        )),
        Err(InstallError::NotInApplications)
    ));
    assert!(matches!(
        bundle_of(Path::new("/Users/me/Downloads/ZedSuite/ZedSuite")),
        Err(InstallError::NotInApplications)
    ));
    println!("bundle_of: OK (Applications accepted, disk image / translocation / bare binary refused)");

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 4 {
        eprintln!("usage: macos_update_test <archive.app.tar.gz> <installed .app> <version> [pid]");
        std::process::exit(2);
    }
    let archive = PathBuf::from(&args[1]);
    let bundle = PathBuf::from(&args[2]);
    let version = args[3].trim_start_matches(['v', 'V']).to_string();
    let pid = args.get(4).and_then(|p| p.parse::<u32>().ok());
    let binary = bundle.join("Contents").join("MacOS").join("ZedSuite");

    // An archive of another version must be refused and leave the bundle as it is
    match install_from_archive(&archive, &bundle, "0.0.0-not-this-one") {
        Err(InstallError::InstallFailed(msg)) => println!("wrong version refused: {msg}"),
        other => panic!("expected the version check to refuse, got {other:?}"),
    }
    assert!(binary.is_file(), "bundle untouched after a refusal");

    install_from_archive(&archive, &bundle, &version).expect("install_from_archive");
    let plist = std::fs::read_to_string(bundle.join("Contents").join("Info.plist")).expect("Info.plist");
    assert!(
        plist.contains(&format!("<string>{version}</string>")),
        "installed bundle is not version {version}"
    );
    assert!(binary.is_file(), "installed bundle has no binary");
    println!("install_from_archive: OK -> {} is now {version}", bundle.display());

    if let Some(pid) = pid {
        relaunch_after_exit(&bundle, pid).expect("relaunch script");
        println!("relaunch script armed: waits for pid {pid}, reopens the bundle, removes the backup");
    }
    println!("MACOS UPDATE TEST: OK");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("macos_update_test only runs on macOS");
    std::process::exit(2);
}
