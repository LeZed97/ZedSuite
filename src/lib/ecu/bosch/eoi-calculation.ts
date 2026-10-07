// Fin d'injection (EOI) des EDC15P : pour chaque point régime × quantité
// d'une map d'avance (SOI), la fin d'injection vaut durée − avance, en degrés
// vilebrequin après le point mort haut. La durée vient des six « Injector
// duration », choisies par le « Selector for injector duration » selon
// l'avance (interpolation entre deux tables quand l'avance tombe entre deux
// seuils), comme le calculateur le fait. Contribution de H-ishak (PR #49),
// reprise ici avec la lecture des index du sélecteur et la map chaude par
// défaut.
import { isBigEndianEcu } from "@/lib/ecu-endianness";
import { resolveMapCellLayout, resolveAxisSources } from "@/lib/map-cell-layout";
import { DetectedMapLite, MapEditLite } from "@/lib/power-estimation";

export type EoiStatus = "early" | "ok" | "late" | "veryLate";

export interface EoiCellResult {
  rpm: number;
  iq: number;
  /** Avance (° avant PMH) */
  soi: number;
  /** Durée d'injection (° vilebrequin) */
  duration: number;
  /** Fin d'injection (° après PMH, négatif = avant) */
  eoiAtdc: number;
  status: EoiStatus;
  /** Table(s) de durée utilisée(s), ex. « 02 », « 02 / 03 (60 %) » */
  durationSource: string;
}

export interface EoiMatrixResult {
  rpmAxis: number[];
  iqAxis: number[];
  /** [index régime][index quantité] */
  cells: EoiCellResult[][];
  maxEoiAtdc: number;
  maxEoiPoint: { rpm: number; iq: number } | null;
  stats: Record<EoiStatus, number> & { total: number };
  soiMapName: string;
}

export interface EoiMapOption {
  address: number;
  name: string;
  codeblockId: number | null;
  /** Température du nom (« … 90°C »), pour l'ordre et le choix par défaut */
  temperature: number | null;
}

export interface EoiOptions {
  codeblockId: number | null;
  soiMapAddress?: number;
  durationMode: "auto" | "manual";
  /** Table de durée forcée, 0..5 */
  manualDurationIndex?: number;
  applySoiLimiter?: boolean;
}

/** Fin d'injection à partir de laquelle la cellule passe en « tardive » puis
 *  « très tardive » (° après PMH) : au-delà, la combustion finit tard dans la
 *  détente et les températures d'échappement montent. */
export const EOI_LATE_ATDC = 8;
export const EOI_VERY_LATE_ATDC = 11;

// ── Lecture du fichier ───────────────────────────────────────────────

function readU16(bytes: Uint8Array, addr: number, bigEndian: boolean): number {
  if (addr < 0 || addr + 1 >= bytes.length) return 0;
  return bigEndian ? (bytes[addr] << 8) | bytes[addr + 1] : (bytes[addr + 1] << 8) | bytes[addr];
}

function decodeCell(bytes: Uint8Array, addr: number, dataType: string, bigEndian: boolean): number {
  const dt = dataType.toLowerCase();
  if (dt === "uint8") return bytes[addr] ?? 0;
  if (dt === "int8") {
    const v = bytes[addr] ?? 0;
    return v >= 0x80 ? v - 0x100 : v;
  }
  const raw = readU16(bytes, addr, bigEndian);
  return dt === "int16" && raw >= 0x8000 ? raw - 0x10000 : raw;
}

function decodeAxis(
  bytes: Uint8Array,
  addr: number,
  count: number,
  factor: number,
  offset: number,
  bigEndian: boolean,
): number[] {
  const out: number[] = [];
  for (let i = 0; i < count; i++) out.push(readU16(bytes, addr + i * 2, bigEndian) * factor + offset);
  return out;
}

/** Cellules modifiées (coordonnées d'affichage du viewer) posées sur la grille
 *  lue dans la disposition du viewer. Les octets reçus de l'éditeur portent
 *  déjà les modifications ; ceci ne sert qu'aux édits enregistrés d'une
 *  version lue sans l'éditeur. */
function applyCellEdits(values: number[][], edits: MapEditLite[], mapAddress: number): number[][] {
  let out = values;
  let copied = false;
  for (const edit of edits) {
    if (edit.map_address !== mapAddress) continue;
    const cells = edit.payload?.changedCells;
    if (!Array.isArray(cells)) continue;
    if (!copied) {
      out = values.map((r) => [...r]);
      copied = true;
    }
    const rowsReversed = edit.payload?.flip?.rowsReversed === true;
    const colsReversed = edit.payload?.flip?.colsReversed === true;
    for (const cell of cells) {
      if (typeof cell?.row !== "number" || typeof cell?.col !== "number" || typeof cell?.value !== "number") continue;
      const r = rowsReversed ? out.length - 1 - cell.row : cell.row;
      const c = colsReversed ? (out[0]?.length ?? 0) - 1 - cell.col : cell.col;
      if (out[r] !== undefined && out[r][c] !== undefined) out[r][c] = cell.value;
    }
  }
  return out;
}

/** Map 2D orientée régime en lignes et quantité (ou autre) en colonnes, axes
 *  croissants, valeurs en unités affichées. */
interface OrientedMap {
  rpm: number[];
  other: number[];
  v: number[][];
}

function orientMap(bytes: Uint8Array, map: DetectedMapLite, ecuType: string, edits: MapEditLite[]): OrientedMap | null {
  const two = map.dimensions?.TwoDimensional;
  if (!two) return null;
  const cellBytes = (map.data_type || "UInt16").toLowerCase().includes("8") ? 1 : 2;
  const input = {
    ...map,
    rows_reversed: map.rows_reversed ?? undefined,
    size: two.rows * two.cols * cellBytes,
  };
  const layout = resolveMapCellLayout(input);
  const axes = resolveAxisSources(input);
  if (!axes.x.address || !axes.y.address) return null;

  const bigEndian = isBigEndianEcu(ecuType);
  const cellBig = map.is_little_endian === true ? false : bigEndian;
  const dt = map.data_type || "UInt16";
  const factor = map.correction_factor ?? 1;
  const offset = map.offset ?? 0;

  const x = decodeAxis(bytes, axes.x.address, layout.cols, axes.x.correction, axes.x.offset, bigEndian);
  const y = decodeAxis(bytes, axes.y.address, layout.rows, axes.y.correction, axes.y.offset, bigEndian);

  let v: number[][] = [];
  for (let r = 0; r < layout.rows; r++) {
    const row: number[] = [];
    for (let c = 0; c < layout.cols; c++) {
      const addr = map.address + layout.cellIndex(r, c) * layout.cellBytes;
      row.push(decodeCell(bytes, addr, dt, cellBig) * factor + offset);
    }
    v.push(row);
  }
  v = applyCellEdits(v, edits, map.address);

  const xMax = Math.max(...x);
  const yMax = Math.max(...y);
  let rpm: number[];
  let other: number[];
  let grid: number[][];
  if (xMax > 1000 && xMax > yMax) {
    // Régime en colonnes : on transpose
    rpm = [...x];
    other = [...y];
    grid = x.map((_, c) => v.map((row) => row[c]));
  } else {
    rpm = [...y];
    other = [...x];
    grid = v.map((row) => [...row]);
  }
  if (rpm.length > 1 && rpm[0] > rpm[rpm.length - 1]) {
    rpm.reverse();
    grid.reverse();
  }
  if (other.length > 1 && other[0] > other[other.length - 1]) {
    other.reverse();
    grid = grid.map((row) => row.slice().reverse());
  }
  return { rpm, other, v: grid };
}

function interp1(xs: number[], ys: number[], x: number): number {
  if (xs.length === 0) return 0;
  if (xs.length === 1) return ys[0] ?? 0;
  if (x <= xs[0]) return ys[0];
  if (x >= xs[xs.length - 1]) return ys[ys.length - 1];
  for (let i = 1; i < xs.length; i++) {
    if (x <= xs[i]) {
      const span = xs[i] - xs[i - 1];
      if (Math.abs(span) < 1e-9) return ys[i];
      const t = (x - xs[i - 1]) / span;
      return ys[i - 1] + t * (ys[i] - ys[i - 1]);
    }
  }
  return ys[ys.length - 1];
}

function lookup2D(map: OrientedMap, rpm: number, other: number): number {
  const column = map.other.map((_, j) => interp1(map.rpm, map.v.map((row) => row[j] ?? 0), rpm));
  return interp1(map.other, column, other);
}

// ── Sélecteur de durée ───────────────────────────────────────────────

/** Seuil d'avance (° avant PMH) → numéro de la table de durée à utiliser,
 *  triés par avance décroissante. Les valeurs du sélecteur sont des index
 *  × 256 (0, 256, …, 1280) ; un fichier préparé peut les permuter, on les
 *  lit donc au lieu de supposer l'ordre naturel. */
interface DurationSelector {
  steps: { soi: number; index: number }[];
}

function readDurationSelector(bytes: Uint8Array, map: DetectedMapLite): DurationSelector | null {
  const axisAddr = map.y_axis_address || map.x_axis_address || (map.address >= 12 ? map.address - 12 : 0);
  if (!axisAddr || axisAddr + 12 > bytes.length || map.address + 12 > bytes.length) return null;
  const axisFactor = map.y_axis_correction ?? -0.023437;
  const axisOffset = map.y_axis_offset ?? 78.0;
  const steps: { soi: number; index: number }[] = [];
  for (let i = 0; i < 6; i++) {
    const soi = readU16(bytes, axisAddr + i * 2, false) * axisFactor + axisOffset;
    const raw = readU16(bytes, map.address + i * 2, false);
    const index = Math.max(0, Math.min(5, Math.round(raw / 256)));
    steps.push({ soi, index });
  }
  steps.sort((a, b) => b.soi - a.soi);
  return { steps };
}

// ── Maps disponibles ─────────────────────────────────────────────────

const isSoiMap = (m: DetectedMapLite): boolean => {
  const n = (m.name || "").toLowerCase();
  return (
    n.includes("start of injection") &&
    !n.includes("selector") &&
    !n.includes("limiter") &&
    !n.includes("limit") &&
    !n.includes("bip")
  );
};

const isDurationMap = (m: DetectedMapLite): boolean => {
  const n = (m.name || "").toLowerCase();
  return n.includes("injector duration") && !n.includes("selector");
};

const temperatureOf = (name: string): number | null => {
  const m = name.match(/(-?\d+)\s*°\s*C/i);
  return m ? parseInt(m[1], 10) : null;
};

/** Le calculateur a besoin d'une map d'avance et d'au moins une durée. */
export function hasEoiMaps(maps: DetectedMapLite[], ecuType: string): boolean {
  if (!(ecuType || "").toUpperCase().includes("EDC15P")) return false;
  return maps.some(isSoiMap) && maps.some(isDurationMap);
}

/** Codeblocks qui portent une map d'avance, dans l'ordre des numéros. */
export function getEoiCodeblocks(maps: DetectedMapLite[]): (number | null)[] {
  const ids = new Set<number | null>();
  for (const m of maps) if (isSoiMap(m)) ids.add(m.codeblock_id ?? null);
  return Array.from(ids).sort((a, b) => (a ?? -1) - (b ?? -1));
}

/** Maps d'avance d'un codeblock, la plus chaude en premier : c'est celle
 *  qui sert moteur chaud, donc celle qu'on regarde par défaut. */
export function getAvailableSoiMaps(maps: DetectedMapLite[], codeblockId: number | null): EoiMapOption[] {
  const result: EoiMapOption[] = [];
  for (const m of maps) {
    if (!isSoiMap(m)) continue;
    if ((m.codeblock_id ?? null) !== codeblockId) continue;
    result.push({
      address: m.address,
      name: m.name || "",
      codeblockId: m.codeblock_id ?? null,
      temperature: temperatureOf(m.name || ""),
    });
  }
  result.sort((a, b) => {
    if (a.temperature !== null && b.temperature !== null) return b.temperature - a.temperature;
    if (a.temperature !== null) return -1;
    if (b.temperature !== null) return 1;
    return a.name.localeCompare(b.name);
  });
  return result;
}

// ── Calcul ───────────────────────────────────────────────────────────

const statusOf = (eoiAtdc: number): EoiStatus => {
  if (eoiAtdc < 0) return "early";
  if (eoiAtdc <= EOI_LATE_ATDC) return "ok";
  if (eoiAtdc <= EOI_VERY_LATE_ATDC) return "late";
  return "veryLate";
};

const two = (n: number) => Math.round(n * 100) / 100;

export function computeEoiMatrix(
  bytes: Uint8Array,
  maps: DetectedMapLite[],
  edits: MapEditLite[],
  ecuType: string,
  options: EoiOptions,
): EoiMatrixResult | null {
  if (!(ecuType || "").toUpperCase().includes("EDC15P")) return null;
  const { codeblockId, durationMode, manualDurationIndex = 0, applySoiLimiter = false } = options;
  const inBlock = (m: DetectedMapLite) => (m.codeblock_id ?? null) === codeblockId;

  const soiOptions = getAvailableSoiMaps(maps, codeblockId);
  if (soiOptions.length === 0) return null;
  const chosen = soiOptions.find((o) => o.address === options.soiMapAddress) ?? soiOptions[0];
  const soiMeta = maps.find((m) => m.address === chosen.address);
  if (!soiMeta) return null;
  const soiMap = orientMap(bytes, soiMeta, ecuType, edits);
  if (!soiMap) return null;

  let limiter: OrientedMap | null = null;
  if (applySoiLimiter) {
    const limMeta = maps.find((m) => inBlock(m) && (m.name || "").toLowerCase().includes("soi limiter"));
    if (limMeta) limiter = orientMap(bytes, limMeta, ecuType, edits);
  }

  // Tables de durée 00..05 du codeblock
  const durationMaps: (OrientedMap | null)[] = [null, null, null, null, null, null];
  for (const m of maps) {
    if (!inBlock(m) || !isDurationMap(m)) continue;
    const match = (m.name || "").match(/duration\s*0?(\d)/i);
    if (!match) continue;
    const idx = parseInt(match[1], 10);
    if (idx >= 0 && idx <= 5 && !durationMaps[idx]) durationMaps[idx] = orientMap(bytes, m, ecuType, edits);
  }
  const firstDuration = durationMaps.find((m) => m !== null) ?? null;
  if (!firstDuration) return null;
  const durationAt = (index: number, rpm: number, iq: number): number => {
    const m = durationMaps[index] ?? firstDuration;
    return lookup2D(m, rpm, iq);
  };

  const selectorMeta = maps.find(
    (m) => inBlock(m) && (m.name || "").toLowerCase().includes("selector for injector duration"),
  );
  const selector = selectorMeta ? readDurationSelector(bytes, selectorMeta) : null;
  const steps =
    selector && selector.steps.length >= 2
      ? selector.steps
      : [25, 20, 15, 10, 5, 0].map((soi, index) => ({ soi, index }));

  const label = (i: number) => `0${i}`;
  const cells: EoiCellResult[][] = [];
  const stats: EoiMatrixResult["stats"] = { early: 0, ok: 0, late: 0, veryLate: 0, total: 0 };
  let maxEoiAtdc = -Infinity;
  let maxEoiPoint: { rpm: number; iq: number } | null = null;

  for (let r = 0; r < soiMap.rpm.length; r++) {
    const rpm = soiMap.rpm[r];
    const row: EoiCellResult[] = [];
    for (let c = 0; c < soiMap.other.length; c++) {
      const iq = soiMap.other[c];
      let soi = soiMap.v[r]?.[c] ?? lookup2D(soiMap, rpm, iq);
      if (limiter) {
        const lim = lookup2D(limiter, rpm, 90);
        if (soi > lim) soi = lim;
      }

      let duration: number;
      let durationSource: string;
      if (durationMode === "manual") {
        const idx = Math.max(0, Math.min(5, manualDurationIndex));
        duration = durationAt(idx, rpm, iq);
        durationSource = label(idx);
      } else if (soi >= steps[0].soi) {
        duration = durationAt(steps[0].index, rpm, iq);
        durationSource = label(steps[0].index);
      } else if (soi <= steps[steps.length - 1].soi) {
        const last = steps[steps.length - 1];
        duration = durationAt(last.index, rpm, iq);
        durationSource = label(last.index);
      } else {
        let k = 0;
        for (let i = 0; i < steps.length - 1; i++) {
          if (soi <= steps[i].soi && soi >= steps[i + 1].soi) {
            k = i;
            break;
          }
        }
        const upper = steps[k];
        const lower = steps[k + 1];
        const span = upper.soi - lower.soi;
        const share = span > 0.001 ? (soi - lower.soi) / span : 0.5;
        duration = share * durationAt(upper.index, rpm, iq) + (1 - share) * durationAt(lower.index, rpm, iq);
        durationSource = `${label(upper.index)} / ${label(lower.index)}, ${Math.round(share * 100)} %`;
      }

      const eoiAtdc = duration - soi;
      const status = statusOf(eoiAtdc);
      stats[status]++;
      stats.total++;
      if (eoiAtdc > maxEoiAtdc) {
        maxEoiAtdc = eoiAtdc;
        maxEoiPoint = { rpm, iq };
      }
      row.push({ rpm, iq, soi: two(soi), duration: two(duration), eoiAtdc: two(eoiAtdc), status, durationSource });
    }
    cells.push(row);
  }

  return {
    rpmAxis: soiMap.rpm,
    iqAxis: soiMap.other,
    cells,
    maxEoiAtdc: two(maxEoiAtdc),
    maxEoiPoint,
    stats,
    soiMapName: soiMeta.name || "",
  };
}
