//! Banc d'essai du remplacement d'AppImage (src/update.rs, module `linux`),
//! sans réseau ni release : deux faux AppImage (en-tête ELF) dans un dossier
//! temporaire, le « nouveau » remplace le « courant », l'ancien est gardé en
//! `.old`, puis le refus d'un fichier qui n'est pas un ELF et d'un dossier
//! non accessible en écriture.
//!
//! Usage (Linux) : cargo run --example linux_update_test
//! Sur un autre système le programme ne fait rien.

#[cfg(target_os = "linux")]
fn main() {
    use std::os::unix::fs::PermissionsExt;
    use zedsuite_lib::update::linux::{backup_path, dir_writable, replace_appimage, InstallError};

    let dir = std::env::temp_dir().join(format!("zedsuite-linux-update-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let current = dir.join("ZedSuite_1.2.1_linux-x86_64.AppImage");
    let downloaded = dir.join("download").join("ZedSuite-update-1.2.2.AppImage");
    std::fs::create_dir_all(downloaded.parent().unwrap()).unwrap();

    let elf = |tag: &str| {
        let mut v = vec![0x7F, b'E', b'L', b'F'];
        v.extend_from_slice(tag.as_bytes());
        v
    };
    std::fs::write(&current, elf("current 1.2.1")).unwrap();
    std::fs::set_permissions(&current, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(&downloaded, elf("new 1.2.2")).unwrap();

    // 1. remplacement : le chemin courant porte le nouveau contenu, exécutable,
    //    l'ancien est en .old
    replace_appimage(&downloaded, &current).expect("replace");
    assert_eq!(std::fs::read(&current).unwrap(), elf("new 1.2.2"));
    assert_ne!(std::fs::metadata(&current).unwrap().permissions().mode() & 0o111, 0, "exécutable");
    assert_eq!(std::fs::read(backup_path(&current)).unwrap(), elf("current 1.2.1"));
    assert!(!downloaded.exists(), "le téléchargement a été déplacé");
    println!("remplacement AppImage : OK ({})", current.display());

    // 2. un téléchargement qui n'est pas un ELF (page d'erreur) est refusé,
    //    le fichier courant reste intact
    std::fs::write(&downloaded, b"<html>not found</html>").unwrap();
    match replace_appimage(&downloaded, &current) {
        Err(InstallError::InstallFailed(msg)) => assert!(msg.contains("not an AppImage"), "{msg}"),
        other => panic!("attendu un refus, obtenu {other:?}"),
    }
    assert_eq!(std::fs::read(&current).unwrap(), elf("new 1.2.2"));
    println!("refus d'un fichier non ELF : OK");

    // 3. dossier non accessible en écriture détecté avant tout téléchargement
    let ro = dir.join("readonly");
    std::fs::create_dir_all(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    let writable = dir_writable(&ro);
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
    // root écrit partout : le test ne vaut que pour un utilisateur normal
    if unsafe { libc_geteuid() } != 0 {
        assert!(!writable, "dossier 0555 vu comme accessible en écriture");
        println!("dossier non accessible en écriture : OK");
    } else {
        println!("dossier non accessible en écriture : ignoré (root)");
    }

    let _ = std::fs::remove_dir_all(&dir);
    println!("LINUX UPDATE TEST : OK");
}

#[cfg(target_os = "linux")]
extern "C" {
    #[link_name = "geteuid"]
    fn libc_geteuid() -> u32;
}

#[cfg(not(target_os = "linux"))]
fn main() {
    println!("linux_update_test : Linux seulement");
}
