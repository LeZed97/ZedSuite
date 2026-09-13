// Réglages d'affichage des maps partagés par FAMILLE et mémorisés par
// CALCULATEUR (demande du 13/09/2026).
//
// • Famille : toutes les maps d'un projet qui portent le même nom une fois
//   la numérotation retirée (« Injector duration 02 » → « injector duration »,
//   « Driver wish » de chaque codeblock, « Start of injection (SOI) -10°C »
//   → « start of injection (soi) », « BIP SOI Correction [0x06136E] »…).
//   Inverser ou régler une map applique le même écart à toute sa famille.
// • Mémoire par calculateur (option « Mémoriser l'affichage des maps par
//   calculateur » du menu Paramètres de l'éditeur) : l'écart est enregistré
//   pour le type d'ECU du projet et réappliqué à l'ouverture de tout projet
//   du même type — le bouton d'inversion arrive déjà enclenché.
//
// L'écart (« overrides ») ne contient que ce qui diffère des défauts détectés
// de la map : inversion, miroirs d'axes, facteurs/décalages/diviseurs/
// décimales modifiés. Il se réapplique sur les défauts de n'importe quelle
// map de la famille, qui garde ainsi ses propres adresses et dimensions.

export interface AxisDisplayOverrides {
  mirror?: boolean;
  factor?: number;
  offset?: number;
  divisor?: number;
  precision?: number;
}

export interface MapDisplayOverrides {
  invertDisplay?: boolean;
  map?: { factor?: number; offset?: number; divisor?: number; precision?: number };
  xAxis?: AxisDisplayOverrides;
  yAxis?: AxisDisplayOverrides;
}

/** Sous-ensemble des réglages d'une map que ces helpers lisent/écrivent. */
interface AxisDisplayLike {
  mirror: boolean;
  factor: number;
  offset: number;
  divisor: number;
  precision: number;
}
interface MapDisplayLike {
  invertDisplay?: boolean;
  factor: number;
  offset: number;
  divisor: number;
  precision: number;
  xAxis: AxisDisplayLike;
  yAxis: AxisDisplayLike;
}

const NUMERIC_KEYS = ["factor", "offset", "divisor", "precision"] as const;
type NumericKey = (typeof NUMERIC_KEYS)[number];

/**
 * Clé de famille d'une map : nom en minuscules, sans étiquette d'adresse
 * « [0x…] », sans index « (2) », sans température « -10°C », sans numéro
 * final (« 02 », « 10 », « 1-2 »). Répété jusqu'à stabilité.
 */
export function mapFamilyKey(name: string | undefined | null): string {
  let s = (name || "").toLowerCase().trim();
  for (let guard = 0; guard < 6; guard++) {
    const before = s;
    s = s
      .replace(/\s*\[[^\]]*\]\s*$/, "")
      .replace(/\s*\(\s*\d+\s*\)\s*$/, "")
      .replace(/\s*[-+]?\d+(?:[.,]\d+)?\s*[°º]\s*c\s*$/, "")
      .replace(/\s*#?\d+(?:\s*-\s*\d+)?\s*$/, "")
      .trim();
    if (s === before) break;
  }
  return s;
}

/** Type d'ECU normalisé pour la mémoire (null si inconnu). */
export function normalizeEcuKey(ecuType: string | undefined | null): string | null {
  const k = (ecuType || "").trim().toUpperCase();
  if (!k || k === "UNKNOWN") return null;
  return k;
}

function numericDiff<T extends Record<NumericKey, number>>(
  current: T,
  defaults: T
): Partial<Record<NumericKey, number>> {
  const out: Partial<Record<NumericKey, number>> = {};
  for (const k of NUMERIC_KEYS) {
    if (current[k] !== defaults[k]) out[k] = current[k];
  }
  return out;
}

function axisDiff(current: AxisDisplayLike, defaults: AxisDisplayLike): AxisDisplayOverrides {
  const out: AxisDisplayOverrides = numericDiff(current, defaults);
  if (current.mirror === true) out.mirror = true;
  return out;
}

const hasKeys = (o: object | undefined): boolean => !!o && Object.keys(o).length > 0;

/** Écart entre les réglages d'une map et ses défauts détectés. */
export function extractDisplayOverrides<T extends MapDisplayLike>(current: T, defaults: T): MapDisplayOverrides {
  const out: MapDisplayOverrides = {};
  if (current.invertDisplay === true) out.invertDisplay = true;
  const map = numericDiff(current, defaults);
  if (hasKeys(map)) out.map = map;
  const x = axisDiff(current.xAxis, defaults.xAxis);
  if (hasKeys(x)) out.xAxis = x;
  const y = axisDiff(current.yAxis, defaults.yAxis);
  if (hasKeys(y)) out.yAxis = y;
  return out;
}

export function isEmptyOverrides(o: MapDisplayOverrides | undefined | null): boolean {
  return !o || (!o.invertDisplay && !hasKeys(o.map) && !hasKeys(o.xAxis) && !hasKeys(o.yAxis));
}

function applyAxis<A extends AxisDisplayLike>(base: A, o: AxisDisplayOverrides | undefined): A {
  return {
    ...base,
    mirror: o?.mirror === true,
    factor: o?.factor ?? base.factor,
    offset: o?.offset ?? base.offset,
    divisor: o?.divisor ?? base.divisor,
    precision: o?.precision ?? base.precision,
  };
}

/**
 * Réglages complets d'une map = ses DÉFAUTS + l'écart. Tout ce qui n'est pas
 * dans l'écart revient au défaut (retirer un miroir sur la map source le
 * retire aussi sur la famille).
 */
export function applyDisplayOverrides<T extends MapDisplayLike>(defaults: T, o: MapDisplayOverrides): T {
  return {
    ...defaults,
    invertDisplay: o.invertDisplay === true,
    factor: o.map?.factor ?? defaults.factor,
    offset: o.map?.offset ?? defaults.offset,
    divisor: o.map?.divisor ?? defaults.divisor,
    precision: o.map?.precision ?? defaults.precision,
    xAxis: applyAxis(defaults.xAxis, o.xAxis),
    yAxis: applyAxis(defaults.yAxis, o.yAxis),
  };
}

function axisDiffers(a: AxisDisplayLike, b: AxisDisplayLike): boolean {
  return (a.mirror === true) !== (b.mirror === true) || NUMERIC_KEYS.some((k) => a[k] !== b[k]);
}

/** Vrai si les deux réglages diffèrent sur un champ couvert par l'écart. */
export function displaySettingsDiffer(a: MapDisplayLike, b: MapDisplayLike): boolean {
  return (
    (a.invertDisplay === true) !== (b.invertDisplay === true) ||
    NUMERIC_KEYS.some((k) => a[k] !== b[k]) ||
    axisDiffers(a.xAxis, b.xAxis) ||
    axisDiffers(a.yAxis, b.yAxis)
  );
}

// ── Mémoire par calculateur (localStorage) ──

const KEY = "zedsuiteMapDisplayPrefs";
export const MAP_DISPLAY_PREFS_EVENT = "zedsuite-map-display-prefs";

export type MapDisplayPrefs = Record<string, Record<string, MapDisplayOverrides>>;

export function loadMapDisplayPrefs(): MapDisplayPrefs {
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" ? (parsed as MapDisplayPrefs) : {};
  } catch {
    return {};
  }
}

function storePrefs(prefs: MapDisplayPrefs): void {
  try {
    if (Object.keys(prefs).length === 0) localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    // stockage indisponible : rien à faire
  }
  window.dispatchEvent(new CustomEvent(MAP_DISPLAY_PREFS_EVENT));
}

export function getMapDisplayPref(ecuKey: string, family: string): MapDisplayOverrides | undefined {
  return loadMapDisplayPrefs()[ecuKey]?.[family];
}

/** Enregistre l'écart d'une famille pour un ECU (un écart vide efface l'entrée). */
export function saveMapDisplayPref(ecuKey: string, family: string, overrides: MapDisplayOverrides): void {
  const prefs = loadMapDisplayPrefs();
  const forEcu = { ...(prefs[ecuKey] ?? {}) };
  if (isEmptyOverrides(overrides)) delete forEcu[family];
  else forEcu[family] = overrides;
  if (Object.keys(forEcu).length === 0) delete prefs[ecuKey];
  else prefs[ecuKey] = forEcu;
  storePrefs(prefs);
}

export function hasMapDisplayPrefs(): boolean {
  return Object.keys(loadMapDisplayPrefs()).length > 0;
}

export function clearMapDisplayPrefs(): void {
  storePrefs({});
}
