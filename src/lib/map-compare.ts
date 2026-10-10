/**
 * Comparaison map par map de deux versions d'un fichier (à la WinOLS).
 *
 * Chaque map est lue des deux côtés avec extractMapGrid, la lecture même du
 * MapViewer : valeurs physiques (facteur/offset, réglages Propriétés),
 * orientation et axes identiques à ce que l'éditeur affiche. Une cellule
 * signalée ici se retrouve donc à la même place dans la fenêtre de la map.
 */
import {
  extractMapGrid,
  getMapCellDecimals,
  orientMapForDisplay,
  applyDisplayOrientation,
  type ExtractMapInput,
  type ExtractMapDisplaySettings,
} from "@/lib/map-extract";
import { resolveMapCellLayout, resolveAxisSources } from "@/lib/map-cell-layout";

export interface CompareMapInput extends ExtractMapInput {
  codeblock_id?: number | null;
}

export interface MapCompareSummary {
  map: CompareMapInput;
  /** Cellules dont la valeur affichée diffère */
  changedCells: number;
  totalCells: number;
  /** Au moins un point d'axe diffère */
  axisChanged: boolean;
  /** Plus grand écart relatif (en %) sur les cellules modifiées, signé */
  maxIncreasePct: number;
  maxDecreasePct: number;
}

export interface MapCompareDetail {
  rows: number;
  cols: number;
  left: number[][];
  right: number[][];
  leftXLabels: string[];
  rightXLabels: string[];
  leftYLabels: string[];
  rightYLabels: string[];
  xAxisIsIndex: boolean;
  yAxisIsIndex: boolean;
  decimals: number;
}

/** Écart en dessous duquel deux valeurs affichées sont considérées égales
 *  (bruit flottant du facteur, pas une vraie modification). */
const EPSILON = 1e-9;

export const cellsDiffer = (a: number, b: number): boolean => Math.abs(a - b) > EPSILON;

/** Variation relative de b par rapport à a, en % (null si a = 0). */
export const percentChange = (a: number, b: number): number | null =>
  Math.abs(a) > EPSILON ? ((b - a) / Math.abs(a)) * 100 : null;

/**
 * Plages d'octets dont dépend l'affichage d'une map (données + axes lus dans
 * le binaire). Sert de pré-filtre : si aucun octet ne change, la map est
 * identique et on s'épargne sa lecture complète. Volontairement large pour
 * les axes (une plage trop large ne coûte qu'une lecture inutile).
 */
function dependencyRanges(map: CompareMapInput): Array<[number, number]> {
  const ranges: Array<[number, number]> = [[map.address, map.address + map.size]];
  const layout = resolveMapCellLayout(map);
  const axisBytes = Math.max(layout.rows, layout.cols) * 2;
  const axes = resolveAxisSources(map);
  for (const a of [axes.x.address, axes.y.address, map.x_axis_address ?? 0, map.y_axis_address ?? 0]) {
    // -2 : la garde « Boost target » lit la longueur stockée avant l'axe
    if (a > 0) ranges.push([Math.max(0, a - 2), a + axisBytes]);
  }
  return ranges;
}

function rangesDiffer(a: number[], b: number[], ranges: Array<[number, number]>): boolean {
  for (const [start, end] of ranges) {
    for (let i = start; i < end; i++) {
      if (a[i] !== b[i]) return true;
    }
  }
  return false;
}

/**
 * Lit la map des deux côtés et aligne les grilles. Certaines règles
 * d'orientation du MapViewer dépendent des valeurs (axe Y croissant ou non,
 * ligne de zéros de la Start IQ) : si une modification fait basculer l'une
 * d'elles, le côté droit est remis dans l'orientation du gauche pour que
 * chaque cellule compare le même point de fonctionnement.
 */
export function compareMapDetail(
  map: CompareMapInput,
  leftData: number[],
  rightData: number[],
  ecuType: string | undefined,
  displaySettings?: ExtractMapDisplaySettings,
): MapCompareDetail {
  const l = extractMapGrid(map, leftData, ecuType, displaySettings);
  const r = extractMapGrid(map, rightData, ecuType, displaySettings);

  let right = r.mapValues.map((row) => [...row]);
  let rightY = [...r.yAxisLabels];
  let rightX = [...r.xAxisLabels];
  if (l.rowsReversed !== r.rowsReversed) {
    right.reverse();
    rightY.reverse();
  }
  if (l.colsReversed !== r.colsReversed) {
    right = right.map((row) => [...row].reverse());
    rightX.reverse();
  }

  // Orientation d'affichage de l'éditeur (ordre des axes, miroirs, inversion),
  // décidée sur le côté gauche puis appliquée telle quelle au droit
  const oriented = orientMapForDisplay(
    map,
    l.mapValues,
    l.xAxisLabels,
    l.yAxisLabels,
    { x: l.xAxisIsIndex, y: l.yAxisIsIndex },
    {
      xMirror: !!displaySettings?.xAxis?.mirror,
      yMirror: !!displaySettings?.yAxis?.mirror,
      invert: displaySettings?.map?.invertDisplay === true,
    },
    ecuType,
  );
  const rightDisplay = applyDisplayOrientation(right, rightX, rightY, oriented);

  return {
    rows: oriented.values.length,
    cols: oriented.values[0]?.length ?? 0,
    left: oriented.values,
    right: rightDisplay.values,
    leftXLabels: oriented.xLabels,
    rightXLabels: rightDisplay.xLabels,
    leftYLabels: oriented.yLabels,
    rightYLabels: rightDisplay.yLabels,
    xAxisIsIndex: oriented.xIsIndex,
    yAxisIsIndex: oriented.yIsIndex,
    decimals: getMapCellDecimals(map.correction_factor, displaySettings?.map?.precision),
  };
}

export function summarizeDetail(map: CompareMapInput, d: MapCompareDetail): MapCompareSummary {
  let changedCells = 0;
  let maxIncreasePct = 0;
  let maxDecreasePct = 0;
  for (let row = 0; row < d.rows; row++) {
    for (let col = 0; col < d.cols; col++) {
      const a = d.left[row][col];
      const b = d.right[row]?.[col] ?? a;
      if (!cellsDiffer(a, b)) continue;
      changedCells++;
      const pct = percentChange(a, b);
      if (pct !== null) {
        if (pct > maxIncreasePct) maxIncreasePct = pct;
        if (pct < maxDecreasePct) maxDecreasePct = pct;
      }
    }
  }
  const labelsDiffer = (a: string[], b: string[]) => a.length !== b.length || a.some((v, i) => v !== b[i]);
  return {
    map,
    changedCells,
    totalCells: d.rows * d.cols,
    axisChanged: labelsDiffer(d.leftXLabels, d.rightXLabels) || labelsDiffer(d.leftYLabels, d.rightYLabels),
    maxIncreasePct,
    maxDecreasePct,
  };
}

/** Résumé de toutes les maps : nombre de cellules modifiées, axes modifiés. */
export function compareAllMaps(
  maps: CompareMapInput[],
  leftData: number[],
  rightData: number[],
  ecuType: string | undefined,
  getDisplaySettings?: (map: CompareMapInput) => ExtractMapDisplaySettings | undefined,
): MapCompareSummary[] {
  return maps.map((map) => {
    const layout = resolveMapCellLayout(map);
    if (!rangesDiffer(leftData, rightData, dependencyRanges(map))) {
      return {
        map,
        changedCells: 0,
        totalCells: layout.rows * layout.cols,
        axisChanged: false,
        maxIncreasePct: 0,
        maxDecreasePct: 0,
      };
    }
    try {
      const detail = compareMapDetail(map, leftData, rightData, ecuType, getDisplaySettings?.(map));
      return summarizeDetail(map, detail);
    } catch (error) {
      console.error("[map-compare] failed to read map", map.name, error);
      return {
        map,
        changedCells: 0,
        totalCells: layout.rows * layout.cols,
        axisChanged: false,
        maxIncreasePct: 0,
        maxDecreasePct: 0,
      };
    }
  });
}
