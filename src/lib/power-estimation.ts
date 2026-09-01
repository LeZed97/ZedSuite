// Power estimation ("virtual dyno") from the injection maps.
//
// Physical model (validated against real EDC15P files and dyno anchors):
//
// 1. The Driver Wish gives the REQUESTED fuel (IQ, mg/stroke) at full pedal.
// 2. The torque limiter (IQ vs RPM × atmo pressure) caps the request.
// 3. The boost target map (always ABSOLUTE pressure, mbar, 1 bar = atmo)
//    gives the charge pressure at (IQ, RPM); clamped by SVBL and the boost
//    limit map. Air mass/stroke = rho(P) × Vcyl × VE(rpm).
// 4. Smoke caps: Smoke limiter + IQ by MAF (indexed by air mass) and IQ by
//    MAP (indexed by boost) are looked up at the ESTIMATED air/boost — plus
//    the physical smoke bound fuel ≤ air / 18 (min AFR of a diesel before
//    black smoke). This is what keeps low-RPM fueling realistic: a 4-cyl
//    can never burn 100 mg below 1500 rpm because the turbo isn't there yet.
// 5. Injector capability (PD/EDC15P): the ECU converts IQ to a duration (°)
//    through the Injector Duration maps. Real delivered fuel =
//    duration(IQ, rpm) × stock flow (derived from the ORIGINAL file's own
//    duration calibration) × nozzle factor. Bigger nozzles (+X %) deliver
//    more than requested; untouched durations cap an over-ambitious wish.
// 6. Power = fuel flow × LHV × eta(rpm); eta is a full-load efficiency
//    curve (peak ≈ 44 % near 2000 rpm, ≈ 40 % at rated speed).
//
// Calibration anchors (user dyno experience, 1.9 PD):
//   3.6-3.7 bar abs, stock nozzles, 100 mg wish  → ~290-300 hp
//   4.6-4.7 bar abs, +120 % nozzles, 100 mg wish → ~370-390 hp
//   2.5 bar abs, stock PD100, 75-80 mg wish      → ~180-190 hp
//   stock ARL 150 file                            → 144 hp / 327 Nm
//
// EDC16 (torque-based) files: the Driver Wish is in Nm and the torque chain
// carries its own limits; the air bound derived from the Boost Target maps
// is still applied so an uncapped wish cannot exceed what the turbo feeds.

import { isBigEndianEcu } from "./ecu-endianness";

export interface DetectedMapLite {
  name?: string;
  address: number;
  dimensions?: {
    TwoDimensional?: { rows: number; cols: number };
    OneDimensional?: { length: number };
  };
  data_type?: string;
  unit?: string | null;
  category?: string;
  correction_factor?: number | null;
  offset?: number | null;
  x_axis_address?: number | null;
  y_axis_address?: number | null;
  x_axis_correction?: number | null;
  y_axis_correction?: number | null;
  x_axis_offset?: number | null;
  y_axis_offset?: number | null;
  is_little_endian?: boolean;
  codeblock_id?: number | null;
}

export type MapEditLite = { map_address: number; payload?: any };

export type LimitTag = "wish" | "torque" | "smoke" | "air" | "injector";

export interface PowerPoint {
  rpm: number;
  fuel: number; // mg/stroke really burned (or Nm for torque-based files)
  powerKw: number;
  powerHp: number;
  torqueNm: number;
  boostMbar: number | null; // absolute boost used at this point
  limit: LimitTag;
}

export interface CurveSource {
  id: string;
  label: string;
  codeblockId: number | null; // EDC15 codeblock, null = per-map source (EDC16)
  mapAddresses: number[]; // driver-wish map addresses of this source
}

export interface SourceCurveResult {
  source: CurveSource;
  points: PowerPoint[];
  peakPower: PowerPoint;
  peakTorque: PowerPoint;
  torqueBased: boolean;
  hasBoostData: boolean; // air model active (boost target found)
  hasDurationData: boolean; // injector duration model active
  limitsApplied: LimitTag[]; // distinct limits that shaped the curve
}

export const ENGINE_PRESETS = [
  { id: "3cyl14", label: "1.4 TDI — 3 cyl.", cylinders: 3, displacement: 1.422 },
  { id: "4cyl19", label: "1.9 TDI — 4 cyl.", cylinders: 4, displacement: 1.896 },
  { id: "4cyl20", label: "2.0 TDI — 4 cyl.", cylinders: 4, displacement: 1.968 },
  { id: "5cyl25", label: "2.5 TDI — 5 cyl.", cylinders: 5, displacement: 2.461 },
  { id: "6cyl30", label: "3.0 TDI — 6 cyl.", cylinders: 6, displacement: 2.967 },
] as const;

// Types de nez Firad (PD 8v) — capacité d'injection PROPRE en mg/coup,
// dérivée des puissances annoncées par Firad/revendeurs pour le 1.9 PD :
// origine ≈ 230 ch, +50 % ≈ 260, +80 % ≈ 280, +100 % ≈ 300, +120 % ≈ 330,
// +160 % ≈ 350+ (1 mg/coup ≈ 2.89 ch @ 4000 tr/min dans ce modèle).
// La puissance finale = min(air disponible via le boost, capacité des nez).
export const NOZZLE_PRESETS = [
  { id: "stock", label: "Origine", factor: 1.0, ceilingMg: 80 },
  { id: "n30", label: "+30 %", factor: 1.3, ceilingMg: 85 },
  { id: "n50", label: "+50 %", factor: 1.5, ceilingMg: 90 },
  { id: "n80", label: "+80 %", factor: 1.8, ceilingMg: 97 },
  { id: "n100", label: "+100 %", factor: 2.0, ceilingMg: 104 },
  { id: "n120", label: "+120 %", factor: 2.2, ceilingMg: 114 },
  { id: "n160", label: "+160 %", factor: 2.6, ceilingMg: 135 },
  { id: "n180", label: "+180 %", factor: 2.8, ceilingMg: 145 },
  { id: "n200", label: "+200 %", factor: 3.0, ceilingMg: 155 },
] as const;

const DIESEL_LHV = 42.5e6; // J/kg (gazole)
export const DEFAULT_EFFICIENCY = 0.4; // displayed value = nominal eta setting
const HP_PER_KW = 1.35962;
const R_AIR = 287; // J/(kg·K)
const T_MANIFOLD = 330; // K after intercooler at full load
const MIN_AFR = 17; // black-smoke bound: mg air per mg fuel (full-load builds)
const NM_PER_KW = 9549.3; // T[Nm] = P[kW] × 9549.3 / rpm

// Full-load thermal efficiency vs rpm (PD / common-rail), fitted so the
// stock factory files land on their announced figures (ASZ 130/310,
// ARL 150/320…) while the tuner anchors stay in range. Scaled linearly by
// the user's efficiency setting (0.40 = nominal).
// Tail past 4000 rpm falls hard: combustion window and injection quality
// degrade, which is what rolls a real dyno curve over — peak power sits
// between ~3800 and ~4100 rpm even on flat-fueled tuned files.
const ETA_CURVE: Array<[number, number]> = [
  [1000, 0.36], [1500, 0.39], [2000, 0.415], [2500, 0.412], [3000, 0.405],
  [3500, 0.395], [4000, 0.375], [4500, 0.34], [5000, 0.305], [5500, 0.275],
];

// EDC15VM (VP37 rotary pump): delivery falls off hard with rpm. Shape from
// EDCSuite's battle-tested IQToTorque correction (≈ ×0.75 at 4000+), level
// trimmed on the stock files bench: ALH 90 → 90 hp, ASV 110 → ~103 hp.
const VM_ETA_CURVE: Array<[number, number]> = [
  [1000, 0.43], [2200, 0.43], [2400, 0.4256], [2900, 0.4214],
  [3200, 0.3827], [3500, 0.358], [4000, 0.345], [5200, 0.345],
];

// Volumetric efficiency vs rpm (2-valve TDI breathing falls off up high —
// this is what rolls the power curve over past ~4500 rpm).
const VE_CURVE: Array<[number, number]> = [
  [1000, 0.88], [2000, 0.91], [3000, 0.91], [3500, 0.90], [4000, 0.88],
  [4500, 0.74], [5000, 0.60], [5500, 0.48],
];

// ── Binary decoding ─────────────────────────────────────────────────

function readU16(bytes: Uint8Array, addr: number, bigEndian: boolean): number {
  if (addr + 1 >= bytes.length) return 0;
  return bigEndian
    ? (bytes[addr] << 8) | bytes[addr + 1]
    : (bytes[addr + 1] << 8) | bytes[addr];
}

function toSigned16(v: number): number {
  return v >= 0x8000 ? v - 0x10000 : v;
}

function decodeCell(
  bytes: Uint8Array,
  addr: number,
  dataType: string,
  bigEndian: boolean
): number {
  const dt = dataType.toLowerCase();
  if (dt === "uint8") return bytes[addr] ?? 0;
  if (dt === "int8") {
    const v = bytes[addr] ?? 0;
    return v >= 0x80 ? v - 0x100 : v;
  }
  const raw = readU16(bytes, addr, bigEndian);
  return dt === "int16" ? toSigned16(raw) : raw;
}

/** Decode a 16-bit axis (axes are always 16-bit in the ECU's endianness). */
function decodeAxis(
  bytes: Uint8Array,
  addr: number,
  count: number,
  factor: number,
  offset: number,
  bigEndian: boolean
): number[] {
  const out: number[] = [];
  for (let i = 0; i < count; i++) {
    out.push(readU16(bytes, addr + i * 2, bigEndian) * factor + offset);
  }
  return out;
}

/** Overlay saved map edits (display units, source orientation) onto a grid. */
function applyCellEdits(
  values: number[][],
  edits: MapEditLite[],
  mapAddress: number
): number[][] {
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
    for (const cell of cells) {
      if (
        typeof cell?.row === "number" &&
        typeof cell?.col === "number" &&
        typeof cell?.value === "number" &&
        out[cell.row] !== undefined &&
        out[cell.row][cell.col] !== undefined
      ) {
        out[cell.row][cell.col] = cell.value;
      }
    }
  }
  return out;
}

// ── Oriented map: {rpm[], other[], v[rpmIdx][otherIdx]} ─────────────

interface OrientedMap {
  rpm: number[];
  other: number[];
  v: number[][];
}

/** Decode a 2D map and orient it so rows follow the RPM axis. The RPM axis
 *  is identified by value range (reaches > 1500), never by its labels. */
function orientMap(
  bytes: Uint8Array,
  map: DetectedMapLite,
  ecuType: string,
  edits: MapEditLite[]
): OrientedMap | null {
  const two = map.dimensions?.TwoDimensional;
  if (!two || !map.x_axis_address || !map.y_axis_address) return null;
  const { rows, cols } = two;
  const bigEndian = isBigEndianEcu(ecuType);
  const cellBig = map.is_little_endian === true ? false : bigEndian;
  const dt = map.data_type || "UInt16";
  const cellSize = dt.toLowerCase().includes("8") ? 1 : 2;
  const factor = map.correction_factor ?? 1;
  const offset = map.offset ?? 0;

  const x = decodeAxis(
    bytes, map.x_axis_address, cols,
    map.x_axis_correction ?? 1, map.x_axis_offset ?? 0, bigEndian
  );
  const y = decodeAxis(
    bytes, map.y_axis_address, rows,
    map.y_axis_correction ?? 1, map.y_axis_offset ?? 0, bigEndian
  );

  let v: number[][] = [];
  for (let r = 0; r < rows; r++) {
    const row: number[] = [];
    for (let c = 0; c < cols; c++) {
      const addr = map.address + (r * cols + c) * cellSize;
      row.push(decodeCell(bytes, addr, dt, cellBig) * factor + offset);
    }
    v.push(row);
  }
  v = applyCellEdits(v, edits, map.address);

  const xMax = Math.max(...x);
  const yMax = Math.max(...y);
  if (yMax > 1500 && yMax >= xMax) return { rpm: y, other: x, v };
  if (xMax > 1500) {
    return { rpm: x, other: y, v: x.map((_, c) => v.map((row) => row[c])) };
  }
  return null;
}

/** EDC15 Torque limiter special storage: <rows> contiguous RPM values per
 *  atmo column (proven on real files — a row-major read interleaves it).
 *  Returns the per-RPM max across the atmo columns (best case, sea level). */
function torqueLimiterCurve(
  bytes: Uint8Array,
  map: DetectedMapLite,
  ecuType: string,
  edits: MapEditLite[]
): OrientedMap | null {
  const two = map.dimensions?.TwoDimensional;
  if (!two || !map.y_axis_address) return null;
  const { rows, cols } = two;
  const bigEndian = isBigEndianEcu(ecuType);
  const factor = map.correction_factor ?? 1;
  const rpmAxis = decodeAxis(
    bytes, map.y_axis_address, rows,
    map.y_axis_correction ?? 1, map.y_axis_offset ?? 0, bigEndian
  );
  let v: number[][] = [];
  for (let r = 0; r < rows; r++) {
    const row: number[] = [];
    for (let c = 0; c < cols; c++) {
      row.push(readU16(bytes, map.address + (c * rows + r) * 2, bigEndian) * factor);
    }
    v.push(row);
  }
  v = applyCellEdits(v, edits, map.address);
  if (Math.max(...rpmAxis) < 1500) return null;
  return { rpm: rpmAxis, other: v[0]?.map((_, i) => i) ?? [], v };
}

// ── Interpolation ───────────────────────────────────────────────────

function interp1(xs: number[], ys: number[], x: number): number {
  if (xs.length === 0) return 0;
  if (x <= xs[0]) return ys[0];
  if (x >= xs[xs.length - 1]) return ys[ys.length - 1];
  for (let i = 1; i < xs.length; i++) {
    if (x <= xs[i]) {
      const t = (x - xs[i - 1]) / (xs[i] - xs[i - 1]);
      return ys[i - 1] + t * (ys[i] - ys[i - 1]);
    }
  }
  return ys[ys.length - 1];
}

const curveInterp = (pts: Array<[number, number]>) => (x: number) =>
  interp1(pts.map((p) => p[0]), pts.map((p) => p[1]), x);

const etaStdAt = curveInterp(ETA_CURVE);
const etaVmAt = curveInterp(VM_ETA_CURVE);
const veAt = curveInterp(VE_CURVE);

const isVmEcu = (ecuType: string) =>
  ecuType.toUpperCase().includes("EDC15VM") || ecuType.toUpperCase().includes("EDC15V");

const etaFor = (ecuType: string) => (isVmEcu(ecuType) ? etaVmAt : etaStdAt);

/** Bilinear lookup in an oriented map (clamped outside the axes). */
function lookup(om: OrientedMap, rpm: number, other: number): number {
  const col = om.other.map((_, j) =>
    interp1(om.rpm, om.v.map((r) => r[j] ?? 0), rpm)
  );
  return interp1(om.other, col, other);
}

/** Per-RPM max across the non-RPM axis. */
function perRpmMax(om: OrientedMap, rpm: number): number {
  return interp1(om.rpm, om.v.map((r) => Math.max(...r)), rpm);
}

/** Full-pedal curve of a driver-wish map (max column of the pedal axis). */
function wotCurve(om: OrientedMap): { rpm: number[]; value: number[] } {
  const pedalIdx = om.other.indexOf(Math.max(...om.other));
  return { rpm: om.rpm, value: om.v.map((row) => row[pedalIdx] ?? 0) };
}

// ── Sources (codeblocks / individual maps) ──────────────────────────

function isDriverWish(m: DetectedMapLite): boolean {
  const name = (m.name || "").toLowerCase();
  return name.includes("driver") && name.includes("wish");
}

/** List the selectable curve sources of a detection result.
 *  EDC15P/VM: one source per codeblock (renumbered 1..n by ascending
 *  address — the detector's raw ids are not stable across files), each
 *  selectable in the UI. EDC16: a single codeblock with many driver-wish
 *  copies (active or not) — one single source; the per-RPM max keeps only
 *  the most demanding wish. */
export function listCurveSources(maps: DetectedMapLite[]): CurveSource[] {
  const dw = maps.filter(isDriverWish);
  if (dw.length === 0) return [];

  const withCb = dw.filter((m) => m.codeblock_id != null);
  if (withCb.length > 0) {
    const byCb = new Map<number, DetectedMapLite[]>();
    for (const m of withCb) {
      const list = byCb.get(m.codeblock_id!) ?? [];
      list.push(m);
      byCb.set(m.codeblock_id!, list);
    }
    const ids = [...byCb.keys()].sort(
      (a, b) =>
        Math.min(...byCb.get(a)!.map((m) => m.address)) -
        Math.min(...byCb.get(b)!.map((m) => m.address))
    );
    return ids.map((cbId, i) => ({
      id: `cb-${cbId}`,
      label: `Codeblock ${i + 1}`,
      codeblockId: cbId,
      mapAddresses: byCb.get(cbId)!.map((m) => m.address),
    }));
  }

  return [
    {
      id: "dw-max",
      label: "Driver Wish",
      codeblockId: null,
      mapAddresses: dw.map((m) => m.address),
    },
  ];
}

// ── Power computation ───────────────────────────────────────────────

export interface PowerModelOptions {
  cylinders: number;
  displacement: number; // litres
  efficiency: number; // eta at rated rpm (DEFAULT_EFFICIENCY = 0.40)
  nozzleFactor: number; // injector nozzle flow multiplier (1 = stock)
  nozzleCeilingMg?: number; // clean-delivery ceiling of the fitted nozzles
}

interface Ctx {
  bytes: Uint8Array;
  oriBytes: Uint8Array | null;
  maps: DetectedMapLite[];
  ecuType: string;
  edits: MapEditLite[];
}

/** Maps of a source's scope: same codeblock when the source is a codeblock,
 *  every map otherwise (EDC16 single-codeblock files). */
function scopeBy(
  ctx: Ctx,
  source: CurveSource,
  match: (name: string) => boolean
): DetectedMapLite[] {
  return ctx.maps.filter((m) => {
    if (!match((m.name || "").toLowerCase())) return false;
    return source.codeblockId == null || m.codeblock_id === source.codeblockId;
  });
}

function scopeMaps(ctx: Ctx, source: CurveSource, name: string): DetectedMapLite[] {
  return scopeBy(ctx, source, (n) => n === name.toLowerCase());
}

function readSingleValue(ctx: Ctx, m: DetectedMapLite | undefined): number | null {
  if (!m) return null;
  const bigEndian = isBigEndianEcu(ctx.ecuType);
  return readU16(ctx.bytes, m.address, bigEndian) * (m.correction_factor ?? 1);
}

function computeIqBasedCurve(
  ctx: Ctx,
  source: CurveSource,
  dwMaps: DetectedMapLite[],
  opts: PowerModelOptions
): SourceCurveResult | null {
  const { bytes, ecuType, edits } = ctx;
  const oriented = (m: DetectedMapLite) => orientMap(bytes, m, ecuType, edits);

  const dwCurves = dwMaps.map(oriented).filter((m): m is OrientedMap => !!m).map(wotCurve);
  if (dwCurves.length === 0) return null;

  // Torque limiters — EDC15P stores them as contiguous RPM runs per atmo
  // column (rows = long RPM axis → special reader); EDC15VM ("Torque
  // limiter for fuel", 3 atmo rows × N rpm cols) reads row-major.
  const tl = scopeBy(ctx, source, (n) => n.includes("torque limiter"))
    .map((m) => {
      const two = m.dimensions?.TwoDimensional;
      if (!two) return null;
      return two.rows > two.cols
        ? torqueLimiterCurve(bytes, m, ecuType, edits)
        : oriented(m);
    })
    .filter((m): m is OrientedMap => !!m);

  // Smoke-family caps: split air-indexed vs boost-indexed by the range of
  // the non-RPM axis (mbar reaches > 1600, air mass mg/stroke does not) —
  // more robust than the per-family names (EDC15P "Smoke limiter"/"IQ by
  // MAF/MAP limiter", EDC15VM "Smoke limitation by MAF/MAP").
  const smoke: OrientedMap[] = [];
  const iqByMap: OrientedMap[] = [];
  for (const m of scopeBy(
    ctx, source,
    (n) => (n.includes("smoke") || n.includes("iq by ma")) && !n.includes("air intake") && !n.includes("switch")
  )) {
    const om = oriented(m);
    if (!om) continue;
    (Math.max(...om.other) > 1600 ? iqByMap : smoke).push(om);
  }

  // Boost targets ("Boost target map" P, "Boost pressure request" VM) —
  // several copies possible: at each lookup the most permissive wins.
  const boostTargets = scopeBy(
    ctx, source,
    (n) => n.includes("boost target") || n.includes("boost pressure request")
  ).map(oriented).filter((m): m is OrientedMap => !!m);
  const boostLimits = scopeBy(
    ctx, source,
    (n) => n.includes("boost limit") || n.includes("boost pressure limiter")
  ).map(oriented).filter((m): m is OrientedMap => !!m);
  const svbl = readSingleValue(ctx, scopeBy(ctx, source, (n) => n.startsWith("svbl"))[0]);
  const svrl = readSingleValue(ctx, scopeBy(ctx, source, (n) => n.includes("svrl"))[0]);

  // Injector duration maps paired with their original (same address) to
  // derive the stock average flow (mg/°) at each rpm.
  const durationPairs: Array<{ tuned: OrientedMap; ori: OrientedMap }> = [];
  if (ctx.oriBytes) {
    for (const n of ["Injector duration 01", "Injector duration 02", "Injector duration 03", "Injector duration 04"]) {
      for (const m of scopeMaps(ctx, source, n)) {
        const tuned = oriented(m);
        const ori = orientMap(ctx.oriBytes, m, ecuType, []);
        if (tuned && ori) durationPairs.push({ tuned, ori });
      }
    }
  }

  const wotRpm = dwCurves[0].rpm;
  const wotAt = (rpm: number) =>
    Math.max(...dwCurves.map((c) => interp1(c.rpm, c.value, rpm)));

  const rMin = Math.max(1000, Math.min(...wotRpm));
  let rMax = Math.max(...wotRpm);
  if (svrl && svrl > 2000 && svrl < rMax) rMax = svrl;
  if (rMax <= rMin) return null;

  // Gros montage « déplafonné » : quand le fichier cible plus de 3 bar
  // absolus ET demande ~100 mg, les chiffres d'injection sont symboliques
  // (buses inconnues, durées recalées — l'ECU injecte ce que le matériel
  // donne). Le wish, le limiteur de couple et le chemin durées ne bornent
  // plus rien de physique : seuls l'air (boost) et les caps fumée comptent.
  const wishMax = Math.max(...dwCurves.map((c) => Math.max(...c.value)));
  const btMax = boostTargets.length
    ? Math.max(...boostTargets.flatMap((m) => m.v.map((row) => Math.max(...row))))
    : 0;
  const unlocked = wishMax >= 90 && btMax > 3000;

  const limits = new Set<LimitTag>();
  const points: PowerPoint[] = [];
  for (let rpm = rMin; rpm <= rMax; rpm += 100) {
    const req = wotAt(rpm);
    let iq = req;
    let limit: LimitTag = "wish";
    if (unlocked) {
      // Le wish/limiteur ne représentent plus le fuel réel — l'itération
      // ci-dessous ramène l'IQ sur les seules bornes physiques (air, fumée).
      iq = 1000;
      limit = "air";
    } else {
      for (const t of tl) {
        const cap = perRpmMax(t, rpm);
        if (cap < iq) { iq = cap; limit = "torque"; }
      }
    }

    // Boost → air → smoke caps; iterate because the boost target depends
    // on the IQ that the caps themselves reduce (converges downward).
    let boost: number | null = null;
    let airMg = Infinity;
    for (let it = 0; it < 3; it++) {
      if (boostTargets.length) {
        boost = Math.max(...boostTargets.map((m) => lookup(m, rpm, iq)));
        if (svbl && svbl > 500) boost = Math.min(boost, svbl);
        for (const bl of boostLimits) boost = Math.min(boost, perRpmMax(bl, rpm));
      } else if (svbl && svbl > 500) {
        boost = svbl;
      }
      if (boost == null) break;
      const rho = (boost * 100) / (R_AIR * T_MANIFOLD); // mbar → Pa
      airMg = (opts.displacement / opts.cylinders) * 1e-3 * rho * veAt(rpm) * 1e6;

      // Multimap files max their alternative smoke maps — keep the most
      // permissive of each family, then bound by physics (air / min AFR).
      // Unlocked builds: the smoke/MAP axes stop far below the real airflow
      // of 3+ bar (the lookup just saturates on a symbolic plateau) — only
      // the physical air bound means anything there.
      const smokeCaps = unlocked ? [] : smoke.map((m) => lookup(m, rpm, airMg));
      const mapCaps = unlocked ? [] : iqByMap.map((m) => lookup(m, rpm, boost!));
      let next = iq;
      let nextLimit: LimitTag = limit;
      if (smokeCaps.length) {
        const c = Math.max(...smokeCaps);
        if (c < next) { next = c; nextLimit = "smoke"; }
      }
      if (mapCaps.length) {
        const c = Math.max(...mapCaps);
        if (c < next) { next = c; nextLimit = "smoke"; }
      }
      const airCap = airMg / MIN_AFR;
      if (airCap < next) { next = airCap; nextLimit = "air"; }
      const converged = next >= iq - 0.01;
      iq = next;
      limit = nextLimit;
      if (converged) break;
    }
    // No boost data at all: legacy behavior — per-RPM max of the smoke maps
    if (boost == null && smoke.length) {
      const c = Math.max(...smoke.map((m) => perRpmMax(m, rpm)));
      if (c < iq) { iq = c; limit = "smoke"; }
    }

    // Injector duration path (reliable in the WOT region ≥ 2500 rpm; below
    // that the PD duration calibration data is not monotonic and the air
    // bound governs anyway). Skipped on unlocked builds: their durations are
    // calibrated for unknown bigger nozzles, the numbers mean nothing here.
    let fuel = iq;
    if (!unlocked && durationPairs.length && rpm >= 2500) {
      let delivered: number | null = null;
      for (const pair of durationPairs) {
        const oriIqAxis = pair.ori.other.filter((v) => v > 5 && v < 110);
        if (!oriIqAxis.length) continue;
        const iqRef = Math.max(...oriIqAxis);
        const dRef = lookup(pair.ori, rpm, iqRef);
        if (dRef < 3 || dRef > 90) continue;
        const flow = iqRef / dRef; // stock average mg per degree
        const tunedAxis = pair.tuned.other.filter((v) => v > 5 && v < 110);
        if (!tunedAxis.length) continue;
        const d = lookup(pair.tuned, rpm, Math.min(iq, Math.max(...tunedAxis)));
        const del = d * flow * opts.nozzleFactor;
        if (delivered == null || del > delivered) delivered = del;
      }
      if (delivered != null) {
        if (opts.nozzleFactor <= 1) {
          // Stock nozzles: the ECU's quantity calibration is accurate, the
          // durations only cap a wish they cannot serve.
          if (delivered < fuel) { fuel = delivered; limit = "injector"; }
        } else {
          // Bigger nozzles: real fuel follows duration × real flow, the ECU
          // does not know it is injecting more than requested.
          fuel = delivered;
          if (fuel > iq) limit = "injector";
        }
        const airCap = airMg / MIN_AFR;
        if (fuel > airCap) { fuel = airCap; limit = "air"; }
      }
    }

    // Capacité d'injection propre des nez montés (table Firad) — dernier
    // plafond : un gros boost ne sert à rien si les buses ne suivent pas.
    // La fenêtre d'injection utile se réduit avec le régime (moins de degrés
    // vilebrequin disponibles) → le plafond décroît au-delà de 4000 tr/min,
    // ce qui ramène le pic de puissance vers 3800-4200 même bridé nez.
    if (opts.nozzleCeilingMg) {
      const window = interp1(
        [4000, 4500, 5000, 5500],
        [1.0, 0.93, 0.85, 0.78],
        Math.max(4000, rpm)
      );
      const ceiling = opts.nozzleCeilingMg * window;
      if (fuel > ceiling) {
        fuel = ceiling;
        limit = "injector";
      }
    }

    const eta = etaFor(ecuType)(rpm) * (opts.efficiency / DEFAULT_EFFICIENCY);
    const flowKgS = (fuel * 1e-6 * opts.cylinders * rpm) / 120;
    const powerKw = (flowKgS * DIESEL_LHV * eta) / 1000;
    limits.add(limit);
    points.push({
      rpm,
      fuel,
      powerKw,
      powerHp: powerKw * HP_PER_KW,
      torqueNm: rpm > 0 ? (powerKw * NM_PER_KW) / rpm : 0,
      boostMbar: boost,
      limit,
    });
  }
  if (points.length < 3) return null;

  return finalizeCurve(source, points, false, boostTargets.length > 0, durationPairs.length > 0, limits);
}

/** EDC16 fuel path — the accurate model for torque-based (Nm) driver wish.
 *
 *  The EDC16 torque structure works like the EDC15 fuel path: the full-pedal
 *  wish (Nm) capped by the torque limiter is converted to an injected
 *  quantity by the ECU's own "Torque to IQ Conversion" map, then served by
 *  the Duration maps (Duration 00 = full load, sometimes 01 via the
 *  selector). Reading the fuel side is what lands stock files on their
 *  rated figures — the torque limiter alone is a protection ceiling that
 *  sits 15-25% above the real full-load curve.
 *
 *  EDC16 smoke maps are intentionally NOT used here: their rows are stored
 *  mirrored against the detected axis (the table view re-aligns them for
 *  display), so a naive lookup reads them upside down. The air bound from
 *  the boost targets covers the same physics.
 */
function computeEdc16FuelCurve(
  ctx: Ctx,
  source: CurveSource,
  dwMaps: DetectedMapLite[],
  opts: PowerModelOptions
): SourceCurveResult | null {
  const { bytes, ecuType, edits } = ctx;
  const oriented = (m: DetectedMapLite) => orientMap(bytes, m, ecuType, edits);

  const dwCurves = dwMaps.map(oriented).filter((m): m is OrientedMap => !!m).map(wotCurve);
  if (dwCurves.length === 0) return null;

  // Torque limiter ceilings (Nm) — per-RPM best case
  const tlCurves = ctx.maps
    .filter((m) => {
      const n = `${m.name || ""} ${m.category || ""}`.toLowerCase();
      return n.includes("torque") && n.includes("limit") &&
        (m.unit || "").toLowerCase().includes("nm");
    })
    .map(oriented)
    .filter((m): m is OrientedMap => !!m);

  // ECU torque -> IQ conversion (mg/stroke); fallback ratio when the map is
  // not detected on this firmware (bench average of the U31 files: ~6.1 Nm
  // per mg/stroke at full load).
  const trq2iq = ctx.maps
    .filter((m) => (m.name || "").toLowerCase().includes("torque to iq"))
    .map(oriented)
    .filter((m): m is OrientedMap => !!m)[0] ?? null;
  const NM_PER_MG = 6.1;
  // The torque limiter sits ~7% above the true full-load request (protection
  // ceiling) and the per-RPM max across the wish programs picks the most
  // aggressive one. Trim calibrated on the stock bench: KN 2.0 (140 hp) and
  // MAXENCE T5 2.5 (130 hp) both land exactly with 0.93.
  const EDC16_REQ_TRIM = 0.93;

  const boostTargets = ctx.maps
    .filter((m) => (m.name || "").toLowerCase().includes("boost target"))
    .map(oriented)
    .filter((m): m is OrientedMap => !!m);
  const boostLimits = ctx.maps
    .filter((m) => (m.name || "").toLowerCase().includes("boost limit"))
    .map(oriented)
    .filter((m): m is OrientedMap => !!m);
  const svblMap = ctx.maps.find((m) => (m.name || "").toLowerCase().startsWith("svbl"));
  const svbl = readSingleValue(ctx, svblMap);
  const svrl = readSingleValue(ctx, ctx.maps.find((m) => (m.name || "").toLowerCase().includes("svrl")));

  // Duration 00 (full load) and 01 (sometimes active via the selector),
  // anchored on the original file for the stock flow — same model as EDC15.
  const durationPairs: Array<{ tuned: OrientedMap; ori: OrientedMap }> = [];
  if (ctx.oriBytes) {
    for (const n of ["Duration 00", "Duration 01"]) {
      for (const m of ctx.maps.filter((x) => (x.name || "") === n)) {
        const tuned = oriented(m);
        const ori = orientMap(ctx.oriBytes, m, ecuType, []);
        if (tuned && ori) durationPairs.push({ tuned, ori });
      }
    }
  }

  const wotAt = (rpm: number) =>
    Math.max(...dwCurves.map((c) => interp1(c.rpm, c.value, rpm)));
  const rMin = Math.max(1000, Math.min(...dwCurves[0].rpm));
  let rMax = Math.min(Math.max(...dwCurves[0].rpm), 5100);
  if (svrl && svrl > 2000 && svrl < rMax) rMax = svrl;
  if (rMax <= rMin) return null;

  const limits = new Set<LimitTag>();
  const points: PowerPoint[] = [];
  // Monotone tail clamp: on a real engine the full-load IQ never rises
  // again once it started falling (the spikes past 4000 rpm are torque
  // limiter interpolation artifacts). Track the running floor after the
  // first significant decrease.
  let pastPeak = false;
  let prevIq: number | null = null;
  let iqFloor = Infinity;

  for (let rpm = rMin; rpm <= rMax; rpm += 100) {
    let nm = wotAt(rpm);
    let limit: LimitTag = "wish";
    for (const t of tlCurves) {
      const cap = perRpmMax(t, rpm);
      if (cap > 100 && cap < nm) { nm = cap; limit = "torque"; }
    }

    // The /6.1 fallback was bench-fitted directly (bias included), the trim
    // only applies to the exact map path.
    let iq = trq2iq
      ? lookup(trq2iq, rpm, nm) * EDC16_REQ_TRIM
      : nm / NM_PER_MG;

    // Air bound from the boost targets (boost depends on IQ — 3 fixed-point
    // rounds, converges downward like the EDC15 path).
    let boost: number | null = null;
    let airMg = Infinity;
    for (let it = 0; it < 3; it++) {
      if (boostTargets.length) {
        boost = Math.max(...boostTargets.map((m) => lookup(m, rpm, iq)));
        if (svbl && svbl > 500) boost = Math.min(boost, svbl);
        for (const bl of boostLimits) boost = Math.min(boost, perRpmMax(bl, rpm));
      } else if (svbl && svbl > 500) {
        boost = svbl;
      }
      if (boost == null) break;
      const rho = (boost * 100) / (R_AIR * T_MANIFOLD);
      airMg = (opts.displacement / opts.cylinders) * 1e-3 * rho * veAt(rpm) * 1e6;
      const airCap = airMg / MIN_AFR;
      if (airCap >= iq) break;
      iq = airCap;
      limit = "air";
    }

    if (rpm >= 3000 && prevIq != null && iq < prevIq - 0.5) pastPeak = true;
    if (pastPeak) {
      iqFloor = Math.min(iqFloor, iq);
      if (iq > iqFloor) iq = iqFloor;
    }
    prevIq = iq;

    // Duration path (Duration 00/01): stock nozzles cap a wish they cannot
    // serve; bigger nozzles deliver duration x real flow.
    let fuel = iq;
    if (durationPairs.length && rpm >= 2500) {
      let delivered: number | null = null;
      for (const pair of durationPairs) {
        const oriIqAxis = pair.ori.other.filter((v) => v > 5 && v < 110);
        if (!oriIqAxis.length) continue;
        const iqRef = Math.max(...oriIqAxis);
        const dRef = lookup(pair.ori, rpm, iqRef);
        if (dRef < 3 || dRef > 90) continue;
        const flow = iqRef / dRef;
        const tunedAxis = pair.tuned.other.filter((v) => v > 5 && v < 110);
        if (!tunedAxis.length) continue;
        const d = lookup(pair.tuned, rpm, Math.min(iq, Math.max(...tunedAxis)));
        const del = d * flow * opts.nozzleFactor;
        if (delivered == null || del > delivered) delivered = del;
      }
      if (delivered != null) {
        if (opts.nozzleFactor <= 1) {
          if (delivered < fuel) { fuel = delivered; limit = "injector"; }
        } else {
          fuel = delivered;
          if (fuel > iq) limit = "injector";
        }
        const airCap = airMg / MIN_AFR;
        if (fuel > airCap) { fuel = airCap; limit = "air"; }
      }
    }

    if (opts.nozzleCeilingMg) {
      const window = interp1(
        [4000, 4500, 5000, 5500],
        [1.0, 0.93, 0.85, 0.78],
        Math.max(4000, rpm)
      );
      const ceiling = opts.nozzleCeilingMg * window;
      if (fuel > ceiling) { fuel = ceiling; limit = "injector"; }
    }

    const eta = etaFor(ecuType)(rpm) * (opts.efficiency / DEFAULT_EFFICIENCY);
    const flowKgS = (fuel * 1e-6 * opts.cylinders * rpm) / 120;
    const powerKw = (flowKgS * DIESEL_LHV * eta) / 1000;
    limits.add(limit);
    points.push({
      rpm,
      fuel,
      powerKw,
      powerHp: powerKw * HP_PER_KW,
      torqueNm: rpm > 0 ? (powerKw * NM_PER_KW) / rpm : 0,
      boostMbar: boost,
      limit,
    });
  }
  if (points.length < 3) return null;

  return finalizeCurve(source, points, false, boostTargets.length > 0, durationPairs.length > 0, limits);
}

function computeTorqueBasedCurve(
  ctx: Ctx,
  source: CurveSource,
  dwMaps: DetectedMapLite[],
  opts: PowerModelOptions
): SourceCurveResult | null {
  const { bytes, ecuType, edits } = ctx;
  const oriented = (m: DetectedMapLite) => orientMap(bytes, m, ecuType, edits);

  const dwCurves = dwMaps.map(oriented).filter((m): m is OrientedMap => !!m).map(wotCurve);
  if (dwCurves.length === 0) return null;

  // Torque limiter maps (Nm) of the file, best-case per-RPM max
  const tlCurves = ctx.maps
    .filter((m) => {
      const n = `${m.name || ""} ${m.category || ""}`.toLowerCase();
      return n.includes("torque") && n.includes("limit") &&
        (m.unit || "").toLowerCase().includes("nm");
    })
    .map(oriented)
    .filter((m): m is OrientedMap => !!m);

  // Boost targets → air bound converted to a torque bound
  const boostTargets = ctx.maps
    .filter((m) => (m.name || "").toLowerCase().includes("boost target"))
    .map(oriented)
    .filter((m): m is OrientedMap => !!m);

  const wotAt = (rpm: number) =>
    Math.max(...dwCurves.map((c) => interp1(c.rpm, c.value, rpm)));
  const allRpm = dwCurves[0].rpm;
  const rMin = Math.max(1000, Math.min(...allRpm));
  // EDC16 driver-wish axes reach 6000 rpm but a TDI cuts around 5000-5100;
  // without a readable rev limiter, stop the curve there.
  const rMax = Math.min(Math.max(...allRpm), 5100);
  if (rMax <= rMin) return null;

  const limits = new Set<LimitTag>();
  const points: PowerPoint[] = [];
  for (let rpm = rMin; rpm <= rMax; rpm += 100) {
    let nm = wotAt(rpm);
    let limit: LimitTag = "wish";
    for (const t of tlCurves) {
      const cap = perRpmMax(t, rpm);
      if (cap > 100 && cap < nm) { nm = cap; limit = "torque"; }
    }
    let boost: number | null = null;
    if (boostTargets.length) {
      boost = Math.max(...boostTargets.map((m) => perRpmMax(m, rpm)));
      const rho = (boost * 100) / (R_AIR * T_MANIFOLD);
      const airMg = (opts.displacement / opts.cylinders) * 1e-3 * rho * veAt(rpm) * 1e6;
      const eta = etaStdAt(rpm) * (opts.efficiency / DEFAULT_EFFICIENCY);
      const airKw = ((airMg / MIN_AFR) * 1e-6 * opts.cylinders * rpm) / 120 * DIESEL_LHV * eta / 1000;
      const airNm = (airKw * NM_PER_KW) / rpm;
      if (airNm < nm) { nm = airNm; limit = "air"; }
    }
    const powerKw = (nm * rpm) / NM_PER_KW;
    limits.add(limit);
    points.push({
      rpm,
      fuel: nm,
      powerKw,
      powerHp: powerKw * HP_PER_KW,
      torqueNm: nm,
      boostMbar: boost,
      limit,
    });
  }
  if (points.length < 3) return null;

  return finalizeCurve(source, points, true, boostTargets.length > 0, false, limits);
}

/** Trailing rev-cut trim + 1-2-1 smoothing pass + peaks. */
function finalizeCurve(
  source: CurveSource,
  rawIn: PowerPoint[],
  torqueBased: boolean,
  hasBoostData: boolean,
  hasDurationData: boolean,
  limits: Set<LimitTag>
): SourceCurveResult {
  // The torque limiter fades to zero at the rev cut (EDC15) — drop the
  // dead tail so the curve ends where the engine actually stops pulling.
  let end = rawIn.length;
  const floor = torqueBased ? 40 : 5; // Nm / mg
  while (end > 3 && rawIn[end - 1].fuel < floor) end--;
  const raw = rawIn.slice(0, end);

  const points = raw.map((p, i) => {
    const a = raw[Math.max(0, i - 1)];
    const b = raw[Math.min(raw.length - 1, i + 1)];
    const powerKw = (a.powerKw + 2 * p.powerKw + b.powerKw) / 4;
    return {
      ...p,
      powerKw,
      powerHp: powerKw * HP_PER_KW,
      torqueNm: (a.torqueNm + 2 * p.torqueNm + b.torqueNm) / 4,
    };
  });
  const peakPower = points.reduce((x, y) => (y.powerKw > x.powerKw ? y : x));
  const peakTorque = points.reduce((x, y) => (y.torqueNm > x.torqueNm ? y : x));
  return {
    source,
    points,
    peakPower,
    peakTorque,
    torqueBased,
    hasBoostData,
    hasDurationData,
    limitsApplied: [...limits],
  };
}

/** Compute the power/torque curve of every source of the file.
 *  `oriBytes` is the untouched original binary — it anchors the injector
 *  flow model; pass null to skip the duration path (e.g. viewing "Ori"
 *  makes it an identity anyway).
 *  EDC15P/VM: one result per codeblock. EDC16: a single result whose
 *  per-RPM max keeps only the most demanding driver-wish copy. */
export function computePowerCurves(
  bytes: Uint8Array,
  oriBytes: Uint8Array | null,
  maps: DetectedMapLite[],
  ecuType: string,
  opts: PowerModelOptions,
  edits: MapEditLite[] = []
): SourceCurveResult[] {
  const sources = listCurveSources(maps);
  const ctx: Ctx = { bytes, oriBytes, maps, ecuType, edits };
  const results: SourceCurveResult[] = [];
  for (const source of sources) {
    const dwMaps = maps.filter(
      (m) => isDriverWish(m) && source.mapAddresses.includes(m.address)
    );
    if (dwMaps.length === 0) continue;
    const torqueBased = dwMaps.every((m) =>
      (m.unit || "").toLowerCase().includes("nm")
    );
    // EDC16 (Nm wish): the fuel path is the accurate model; the plain
    // torque model only remains as a fallback for exotic firmwares.
    const result = torqueBased
      ? computeEdc16FuelCurve(ctx, source, dwMaps, opts) ??
        computeTorqueBasedCurve(ctx, source, dwMaps, opts)
      : computeIqBasedCurve(ctx, source, dwMaps, opts);
    if (result) results.push(result);
  }
  return results;
}

/** Guess the engine preset from the project's metadata. */
export function guessEnginePreset(engineType?: string, ecuType?: string): string {
  const s = `${engineType || ""} ${ecuType || ""}`.toLowerCase();
  if (s.includes("1.4") || s.includes("1,4")) return "3cyl14";
  if (s.includes("2.0") || s.includes("2,0") || s.includes("edc16")) return "4cyl20";
  if (s.includes("2.5") || s.includes("2,5")) return "5cyl25";
  if (s.includes("3.0") || s.includes("3,0") || s.includes("v6")) return "6cyl30";
  return "4cyl19";
}
