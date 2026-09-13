// Plateforme d'exécution vue depuis l'interface. Les pages sont les mêmes
// sur Windows et macOS ; seule la barre de fenêtre change : boutons
// réduire/agrandir/fermer dessinés par l'app sur Windows (fenêtre sans
// décorations), feux natifs de macOS en haut à gauche sur Mac (barre de
// titre en surimpression, voir src-tauri/tauri.macos.conf.json).

export function isMacOS(): boolean {
  if (typeof navigator === "undefined") return false;
  const hint = `${navigator.platform || ""} ${navigator.userAgent || ""}`;
  return /Mac/i.test(hint) && !/iPhone|iPad/i.test(hint);
}

export function isLinux(): boolean {
  if (typeof navigator === "undefined") return false;
  const hint = `${navigator.platform || ""} ${navigator.userAgent || ""}`;
  return /Linux/i.test(hint) && !/Android/i.test(hint);
}
