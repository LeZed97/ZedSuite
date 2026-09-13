// Échange de blocs de valeurs avec les autres programmes (Excel, EDC Suite,
// éditeur de texte) sous forme de texte : une ligne par ligne du tableau,
// cellules séparées par des tabulations — le format qu'Excel et LibreOffice
// écrivent et lisent dans le presse-papiers.
//
// La lecture est tolérante : tabulations, points-virgules ou suites d'espaces
// comme séparateurs, virgule décimale (Excel français, EDC Suite) ramenée au
// point, lignes vides ignorées. Les valeurs restent des chaînes : la map
// applique ensuite sa propre conversion (facteur, décalage) comme pour une
// saisie au clavier.

/** Texte tabulé (Excel) à partir d'un bloc de valeurs affichées. */
export function gridToText(values: string[][]): string {
  return values.map((row) => row.join("\t")).join("\n");
}

const NUMBER_RE = /^[-+]?(\d+([.,]\d*)?|[.,]\d+)([eE][-+]?\d+)?$/;

/** Normalise une cellule : virgule décimale → point ; "" si vide ; null si ce n'est pas un nombre. */
function normalizeCell(raw: string): string | null {
  const s = raw.trim().replace(/ /g, "");
  if (s === "") return "";
  // « 1 234,5 » (espaces de milliers) → « 1234,5 »
  const compact = s.replace(/(\d)[  ](?=\d{3}(\D|$))/g, "$1");
  if (!NUMBER_RE.test(compact)) return null;
  return compact.replace(",", ".").replace(/^\+/, "");
}

/**
 * Découpe un texte en bloc de valeurs numériques (chaînes normalisées).
 * Retourne null si aucune valeur numérique n'est trouvée.
 */
export function parseGridText(text: string | null | undefined): string[][] | null {
  if (!text) return null;
  const lines = text
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .filter((l) => l.trim() !== "");
  if (lines.length === 0) return null;
  const rows: string[][] = [];
  let numeric = 0;
  for (const line of lines) {
    // Tabulation d'abord (Excel), sinon point-virgule (CSV), sinon espaces
    const cells = line.includes("\t")
      ? line.split("\t")
      : line.includes(";")
        ? line.split(";")
        : line.trim().split(/\s+/);
    const row: string[] = [];
    for (const c of cells) {
      const n = normalizeCell(c);
      if (n === null) return null; // texte non numérique : pas un bloc de valeurs
      if (n !== "") numeric++;
      row.push(n);
    }
    rows.push(row);
  }
  if (numeric === 0) return null;
  const cols = Math.max(...rows.map((r) => r.length));
  // Rectangle régulier : lignes plus courtes complétées par des cases vides
  return rows.map((r) => (r.length < cols ? [...r, ...new Array(cols - r.length).fill("")] : r));
}
