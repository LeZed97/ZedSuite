/**
 * Mappack export - WinOLS-compatible JSON mappack generator
 *
 * Produces the exact same format as the reference mappacks
 * ("Mappack Stage 1 LEON ARL 150 EDO ZedSuite.json"):
 *  - root object { "maps": [ ... ] }
 *  - each map is a FLAT object of WinOLS keys, ALL values as strings
 *  - addresses as "$HEX" (uppercase, no padding)
 *  - factors with 6 decimals ("0.012207" -> "0.012207")
 *  - file serialized with CRLF line endings and latin-1 encoding
 *    (WinOLS does not read UTF-8; "°C" must be byte 0xB0)
 *
 * The export is built from the ORIGINAL detection data (detection_data in
 * PocketBase) so per-project user customizations (custom offsets, mirror
 * display, edited axis labels...) are NOT included — maps appear in WinOLS
 * exactly as the app displays them by default:
 *  - display value = raw * correction_factor + offset  -> Factor/Offset
 *  - EDC16 = big-endian (eHiLo), EDC15 = little-endian (eLoHi),
 *    per-map is_little_endian override respected (data only, axes follow
 *    the ECU endianness, like map-viewer.tsx)
 *  - Int16 maps -> bSigned=1, UInt8 maps -> eByte
 *  - Y axis rendered top-down like the app (AxisY.bBackwards = 1,
 *    matching every map of the reference packs)
 */

import { isBigEndianEcu } from "./ecu-endianness";

/** Superset of the editor MapData with the raw detection fields */
export interface ExportMapData {
  name?: string;
  address: number;
  size?: number;
  description?: string;
  dimensions?: {
    TwoDimensional?: { rows: number; cols: number };
    OneDimensional?: { length: number };
  };
  x_axis_address?: number | null;
  y_axis_address?: number | null;
  x_axis_correction?: number | null;
  y_axis_correction?: number | null;
  correction_factor?: number | null;
  offset?: number | null;
  x_axis_offset?: number | null;
  y_axis_offset?: number | null;
  category?: string;
  x_label?: string | null;
  y_label?: string | null;
  unit?: string | null;
  y_axis_inverted?: boolean | null;
  data_type?: string;
  is_little_endian?: boolean;
}

type WinolsMap = Record<string, string>;

/** Sous-ensemble des réglages d'affichage par map (fenêtre Propriétés) utile
 *  à l'export : le miroir de chaque axe. Clé = adresse de la map (string). */
export interface MappackDisplaySettings {
  xAxis?: { mirror?: boolean };
  yAxis?: { mirror?: boolean };
}

function hexAddr(addr: number | null | undefined): string {
  if (!addr || addr <= 0) return "$0";
  return "$" + Math.trunc(addr).toString(16).toUpperCase();
}

function fmtFactor(v: number | null | undefined): string {
  const n = typeof v === "number" && isFinite(v) ? v : 1.0;
  return n.toFixed(6);
}

function fmtOffset(v: number | null | undefined): string {
  const n = typeof v === "number" && isFinite(v) ? v : 0;
  if (Number.isInteger(n)) return String(n);
  // keep decimals but trim trailing zeros ("-273.150000" -> "-273.15")
  return String(parseFloat(n.toFixed(6)));
}

/** DataOrg of the map data cells (follows map-viewer.tsx decoding) */
function dataOrgForMap(m: ExportMapData, ecuType: string): string {
  const dt = (m.data_type || "").toLowerCase();
  if (dt === "uint8" || dt === "int8") return "eByte";
  // per-map little-endian override (e.g. SOI Selector on EDC16)
  if (m.is_little_endian === true) return "eLoHi";
  return isBigEndianEcu(ecuType) ? "eHiLo" : "eLoHi";
}

/** DataOrg of the axes: always 16-bit in the ECU endianness (axes ignore
 *  the per-map is_little_endian flag, same as map-viewer.tsx) */
function dataOrgForAxes(ecuType: string): string {
  return isBigEndianEcu(ecuType) ? "eHiLo" : "eLoHi";
}

function mapRowsCols(m: ExportMapData): { rows: number; cols: number } {
  const two = m.dimensions?.TwoDimensional;
  if (two && two.rows > 0 && two.cols > 0) return { rows: two.rows, cols: two.cols };
  const one = m.dimensions?.OneDimensional;
  if (one && one.length > 0) return { rows: one.length, cols: 1 };
  return { rows: 1, cols: 1 };
}

/**
 * Folder names: the app groups maps by category. The reference packs use
 * numbered folders ("1-Fuel", "2-Limiters", ...), so we number the
 * categories in alphabetical order to keep a stable, WinOLS-friendly tree.
 */
function buildFolderNames(maps: ExportMapData[]): Map<string, string> {
  const categories = Array.from(
    new Set(maps.map((m) => (m.category || "Other").trim() || "Other"))
  ).sort((a, b) => a.localeCompare(b));
  const folders = new Map<string, string>();
  categories.forEach((cat, i) => {
    // Pas de préfixe numérique : WinOLS trie les dossiers par nom et le
    // « 4-… » cassait le classement (demande Enzo, tous calculateurs)
    folders.set(cat, cat);
  });
  return folders;
}

function buildWinolsMap(
  m: ExportMapData,
  ecuType: string,
  folderName: string,
  ds?: MappackDisplaySettings
): WinolsMap {
  const { rows, cols } = mapRowsCols(m);
  // Miroirs choisis par l'utilisateur : l'axe X s'exporte croissant par
  // défaut (bBackwards 0), l'axe Y de haut en bas comme l'app (bBackwards 1) ;
  // un miroir inverse le drapeau correspondant.
  const xMirror = ds?.xAxis?.mirror === true;
  const yMirror = ds?.yAxis?.mirror === true;
  const name = m.name || `Map ${hexAddr(m.address)}`;
  const dt = (m.data_type || "").toLowerCase();
  const signed = dt === "int16" || dt === "int8";

  const hasX = typeof m.x_axis_address === "number" && m.x_axis_address! > 0 && cols > 1;
  const hasY = typeof m.y_axis_address === "number" && m.y_axis_address! > 0 && rows > 1;
  const axesOrg = dataOrgForAxes(ecuType);

  return {
    "Name": name,
    "IdName": name,
    "FolderName": folderName,
    "Type": "eZweidim",
    "ViewMode": "eViewText",
    "RWin": "eBars",
    "DataOrg": dataOrgForMap(m, ecuType),
    "bReciprocal": "0",
    "bSigned": signed ? "1" : "0",
    "bDelta": "0",
    "bPercent": "0",
    "bOriginal": "0",
    "bOriginalValues": "0",
    "Columns": String(cols),
    "Rows": String(rows),
    "Radix": "10",
    "Comment": m.description || "",
    "Precision": "2",
    "SkipBytes": "0",
    "LineSkipBytes": "0",
    "ValueRangeMin": "0",
    "ValueRangeMax": "0",
    "ValueRangeLock": "0",
    "Marker": "0",
    "Fieldvalues.Name": name,
    "Fieldvalues.Unit": m.unit || "",
    "Fieldvalues.Factor": fmtFactor(m.correction_factor),
    "Fieldvalues.Offset": fmtOffset(m.offset),
    "Fieldvalues.StartAddr.Cpu": hexAddr(m.address),
    "AxisX.Name": hasX ? m.x_label || "" : "",
    "AxisX.IdName": "",
    "AxisX.Unit": hasX ? m.x_label || "" : "",
    "AxisX.Factor": hasX ? fmtFactor(m.x_axis_correction) : "1.000000",
    "AxisX.Offset": hasX ? fmtOffset(m.x_axis_offset) : "0",
    "AxisX.Radix": "10",
    "AxisX.bBackwards": xMirror ? "1" : "0",
    "AxisX.bReciprocal": "0",
    "AxisX.bSigned": "0",
    "AxisX.Precision": "0",
    "AxisX.DataSrc": "eRom",
    "AxisX.DataHeader": "0",
    "AxisX.DataAddr.Cpu": hasX ? hexAddr(m.x_axis_address) : "$0",
    "AxisX.DataOrg": axesOrg,
    "AxisY.Name": hasY ? m.y_label || "" : "",
    "AxisY.IdName": "",
    "AxisY.Unit": hasY ? m.y_label || "" : "",
    "AxisY.Factor": hasY ? fmtFactor(m.y_axis_correction) : "1.000000",
    "AxisY.Offset": hasY ? fmtOffset(m.y_axis_offset) : "0",
    "AxisY.Radix": "10",
    "AxisY.bBackwards": yMirror ? "0" : "1",
    "AxisY.bReciprocal": "0",
    "AxisY.bSigned": "0",
    "AxisY.Precision": "0",
    "AxisY.DataSrc": "eRom",
    "AxisY.DataHeader": "0",
    "AxisY.DataAddr.Cpu": hasY ? hexAddr(m.y_axis_address) : "$0",
    "AxisY.DataOrg": axesOrg,
  };
}

/** Tri des maps à l'intérieur d'un dossier — même choix que la liste de l'éditeur. */
export type MapSortMode = "address" | "name" | "name-desc";

/**
 * Build the mappack object from original detection data.
 * Maps are grouped by app category (numbered folders) and sorted inside
 * each folder by address or by name (the editor's current list order).
 */
export function buildWinolsMappack(
  maps: ExportMapData[],
  ecuType: string,
  sortMode: MapSortMode = "address",
  displaySettings?: Record<string, MappackDisplaySettings>
): { maps: WinolsMap[] } {
  // "limp" (limp-home / recovery) maps are backup limiters, not tuning
  // targets — never exported (same rule as the editor map tree).
  const valid = maps.filter(
    (m) =>
      typeof m.address === "number" &&
      m.address > 0 &&
      !(m.name || "").toLowerCase().includes("(limp)")
  );
  const folders = buildFolderNames(valid);

  const sorted = [...valid].sort((a, b) => {
    const fa = folders.get((a.category || "Other").trim() || "Other") || "";
    const fb = folders.get((b.category || "Other").trim() || "Other") || "";
    if (fa !== fb) return fa.localeCompare(fb, undefined, { numeric: true });
    if (sortMode === "name" || sortMode === "name-desc") {
      const byName = (a.name || "").localeCompare(b.name || "");
      if (byName !== 0) return sortMode === "name-desc" ? -byName : byName;
    }
    return a.address - b.address;
  });

  return {
    maps: sorted.map((m) =>
      buildWinolsMap(
        m,
        ecuType,
        folders.get((m.category || "Other").trim() || "Other") || "1-Other",
        displaySettings?.[String(m.address)]
      )
    ),
  };
}

/**
 * Serialize with the exact reference formatting: 2-space indent, CRLF line
 * endings, latin-1 bytes (so "°C" is encoded as 0xB0 0x43 like WinOLS
 * expects). Characters outside latin-1 are replaced by '?'.
 * (Manual latin-1 encoding — runs in the browser/webview, no Node Buffer.)
 */
export function serializeWinolsMappack(pack: { maps: WinolsMap[] }): Uint8Array {
  const json = JSON.stringify(pack, null, 2).replace(/\n/g, "\r\n");
  const bytes = new Uint8Array(json.length);
  for (let i = 0; i < json.length; i++) {
    const c = json.charCodeAt(i);
    bytes[i] = c <= 0xff ? c : 0x3f; // '?' for non-latin-1 characters
  }
  return bytes;
}

/** File name convention: "Mappack <project> ZedSuite.json" */
export function mappackFileName(projectName: string): string {
  const clean = (projectName || "project")
    .replace(/[\\/:*?"<>|]/g, " ")
    .replace(/\s+/g, " ")
    .trim();
  return `Mappack ${clean} ZedSuite.json`;
}
