/**
 * Disposition des cellules d'une map : dimensions AFFICHÉES (celles de
 * mapValues / changedCells / selectedCells du MapViewer) et index fichier de
 * chaque cellule.
 *
 * Source UNIQUE partagée par MapViewer (lecture des valeurs) et l'éditeur
 * (écriture des modifications, copie vers les maps similaires). Avant, l'éditeur
 * recalculait l'offset en ligne-major avec les dimensions API : sur les maps
 * transposées à l'affichage — torque limiter EDC15 (21x3 lu en colonne-major),
 * IQ by MAF/MAP (13x16), EGR 13x16, Drivers wish MJD6 — les cellules modifiées
 * hors de la première ligne partaient au mauvais endroit du fichier. Un
 * utilisateur l'a constaté sur un torque limiter EDC15P (019GQ) « perturbé »
 * dans VAGSuite après enregistrement.
 */
export interface MapLayoutInput {
  name?: string;
  description?: string;
  size: number;
  data_type?: string;
  /** Lignes fichier rangées dans l'ordre inverse de l'axe Y (bloc Duration de certains EDC16) */
  rows_reversed?: boolean;
  dimensions?: {
    TwoDimensional?: { rows: number; cols: number };
    OneDimensional?: { length: number };
  };
}

export interface MapCellLayout {
  /** Dimensions envoyées par le détecteur */
  apiRows: number;
  apiCols: number;
  /** Dimensions de mapValues (avant miroirs et inversion d'affichage) */
  rows: number;
  cols: number;
  /** Octets par cellule (1 pour les maps 8 bits) */
  cellBytes: number;
  /** Vrai quand la lecture transpose les dimensions API */
  axesSwapped: boolean;
  /** Index (en cellules) dans les données de la map d'une cellule (row, col) de mapValues */
  cellIndex: (row: number, col: number) => number;
}

export interface MapAxisLabelInput {
  x_label?: string | null;
  y_label?: string | null;
  description?: string | null;
}

export interface MapAxisLabels {
  /** Libellé complet, tel que l'app l'affiche (« Engine speed (rpm) ») */
  xLabel: string;
  yLabel: string;
  /** Unité seule, reprise dans le coin de la map et dans le mappack (« rpm ») */
  xUnit: string;
  yUnit: string;
}

/**
 * Libellés et unités des axes AFFICHÉS, source unique du MapViewer (coin de
 * la map, info-bulles 2D) et de l'export mappack (AxisX/Y.Name et .Unit).
 * Priorité aux champs x_label / y_label du détecteur (une chaîne vide est un
 * choix valable : pas de libellé), sinon la description « X: Grandeur (unité)
 * | Y: … ». Sans rien, rien : un « Load » ou un « mbar » par défaut serait une
 * fausse information.
 */
export function resolveAxisLabels(map: MapAxisLabelInput): MapAxisLabels {
  const extractUnit = (label: string): string => {
    const match = label.match(/\(([^)]+)\)/);
    return match ? match[1].trim() : label;
  };
  let xLabel = "";
  let yLabel = "";
  let xUnit = "";
  let yUnit = "";
  if (map.x_label !== undefined && map.x_label !== null) {
    xLabel = map.x_label;
    xUnit = extractUnit(map.x_label);
  }
  if (map.y_label !== undefined && map.y_label !== null) {
    yLabel = map.y_label;
    yUnit = extractUnit(map.y_label);
  }
  if (map.description && (!map.x_label || !map.y_label)) {
    const xMatch = map.description.match(/X:\s*([^(|]+)\s*\(([^)]+)\)/);
    const yMatch = map.description.match(/Y:\s*([^(|]+)\s*\(([^)]+)\)/);
    if (!map.x_label && xMatch && xMatch.length >= 3) {
      xLabel = xMatch[1].trim();
      xUnit = xMatch[2].trim();
    }
    if (!map.y_label && yMatch && yMatch.length >= 3) {
      yLabel = yMatch[1].trim();
      yUnit = yMatch[2].trim();
    }
  }
  return { xLabel, yLabel, xUnit, yUnit };
}

function apiDims(map: MapLayoutInput): { apiRows: number; apiCols: number } {
  if (map.dimensions?.TwoDimensional) {
    return {
      apiRows: map.dimensions.TwoDimensional.rows,
      apiCols: map.dimensions.TwoDimensional.cols,
    };
  }
  if (map.dimensions?.OneDimensional) {
    return { apiRows: 1, apiCols: map.dimensions.OneDimensional.length };
  }
  return { apiRows: 1, apiCols: Math.max(1, Math.floor(map.size / 2)) };
}

/**
 * Maps dont la vue transpose les dimensions API — même règle que le MapViewer.
 */
export function shouldSwapAxes(map: MapLayoutInput): boolean {
  const mapName = (map.name || "").toLowerCase();
  const description = (map.description || "").toLowerCase();

  // Marelli MJD6 "Drivers wish" : stockée [pédale][régime], transposée pour
  // se lire comme les Driver wish EDC16 (régime en lignes, pédale en colonnes).
  if (mapName.includes("drivers wish")) {
    return true;
  }

  // Torque limiter EDC15 : 21 lignes x 3 colonnes dans l'API, affichée à plat
  // (3 lignes x 21 colonnes) en lecture colonne-major.
  if (mapName.includes("torque limiter")) {
    const apiRows = map.dimensions?.TwoDimensional?.rows || 0;
    const apiCols = map.dimensions?.TwoDimensional?.cols || 0;
    if (apiRows > apiCols && apiRows >= 15 && apiCols <= 5) {
      return true;
    }
  }

  // N75 duty cycle : 13x16 annoncé avec X = IQ et Y = régime → dimensions inversées
  if (mapName.includes("n75")) {
    const apiRows = map.dimensions?.TwoDimensional?.rows || 0;
    const apiCols = map.dimensions?.TwoDimensional?.cols || 0;
    const xAxisDesc = description.match(/x:\s*([^(]+)/)?.[1]?.toLowerCase() || "";
    const yAxisDesc = description.match(/y:\s*([^(]+)/)?.[1]?.toLowerCase() || "";
    const xIsIQ = xAxisDesc.includes("iq") || xAxisDesc.includes("mg/st");
    const yIsRpm = yAxisDesc.includes("rpm") || yAxisDesc.includes("engine speed");
    if (xIsIQ && yIsRpm && apiRows === 13 && apiCols === 16) {
      return true;
    }
  }

  return false;
}

export function resolveMapCellLayout(map: MapLayoutInput): MapCellLayout {
  const name = (map.name || "").toLowerCase();
  const { apiRows, apiCols } = apiDims(map);

  const isInjectorDuration = name.includes("injector duration") && !name.includes("selector");
  const isInjectorDurationNon00 = isInjectorDuration && !name.includes("duration 00");
  const isEgrMap = name === "egr" || (name.includes("egr") && !name.includes("temperature"));
  const isIdleRpm = name.includes("idle rpm");
  const isBoostTarget = name.includes("boost target map");
  const isTorqueLimiter = name.includes("torque limiter");
  const isIQByMap = name.includes("iq by map");
  const isIQByMAF = name.includes("iq by maf");

  let needsAxisSwap = shouldSwapAxes(map);
  // Injector duration 01-05 : régime en X, IQ en Y
  if (isInjectorDurationNon00) {
    needsAxisSwap = true;
  }
  // EGR : lue dans l'ordre du fichier (pas de transposition)
  if (isEgrMap) {
    needsAxisSwap = false;
  }
  // Idle RPM : l'axe température reste en X (déjà géré par le détecteur)
  if (isIdleRpm) {
    needsAxisSwap = false;
  }

  // EGR 2D : deux conventions coexistent — 13x16 (EDC15P/EDC16, dims
  // transposées par rapport au fichier 16 lignes x 13 colonnes) et 16x13
  // (EDC15VM, déjà dans le sens du fichier). Seule la première est retournée.
  const egrDimsSwapped = isEgrMap && apiRows < apiCols;
  const rows = isEgrMap
    ? (egrDimsSwapped ? apiCols : apiRows)
    : (needsAxisSwap && isInjectorDurationNon00 ? apiRows : (needsAxisSwap ? apiCols : apiRows));
  const cols = isEgrMap
    ? (egrDimsSwapped ? apiRows : apiCols)
    : (needsAxisSwap && isInjectorDurationNon00 ? apiCols : (needsAxisSwap ? apiRows : apiCols));
  // Nombre de colonnes du fichier pour la lecture ligne-major
  const fileCols = egrDimsSwapped ? apiRows : apiCols;

  // Boost target : le détecteur oriente déjà les axes, aucune transposition côté vue
  if (isBoostTarget) {
    needsAxisSwap = false;
  }

  const dataType = String(map.data_type || "");
  const cellBytes = dataType === "UInt8" || dataType === "Int8" ? 1 : 2;

  const cellIndex = (row: number, col: number): number => {
    if (needsAxisSwap) {
      if (isTorqueLimiter || isIQByMap || isIQByMAF) {
        // Stockage colonne-major : toutes les valeurs de la première ligne
        // d'affichage se suivent dans le fichier
        return row * apiRows + col;
      }
      if (isInjectorDuration) {
        // Transposition d'affichage seulement, données en ligne-major
        return row * apiCols + col;
      }
      // Transposition standard : display[row][col] = file[col][row]
      return col * apiCols + row;
    }
    // Lignes stockées du dernier point de l'axe Y au premier : on lit à l'envers
    const fileRow = map.rows_reversed ? (rows - 1 - row) : row;
    return fileRow * fileCols + col;
  };

  return {
    apiRows,
    apiCols,
    rows,
    cols,
    cellBytes,
    axesSwapped: needsAxisSwap,
    cellIndex,
  };
}
