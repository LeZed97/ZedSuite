/**
 * Bornes d'une valeur AFFICHÉE d'une map.
 *
 * - N75 (duty cycle en %) : entre 0 et 100. Sur EDC15 le facteur est
 *   négatif (100 − brut × 0,01) : une valeur saisie au-dessus de 100 donnait
 *   un brut négatif, stocké en complément à deux, et WinOLS affichait des
 *   valeurs négatives (constaté sur un EDC15VM).
 * - Sinon la plage représentable du type brut (0..65535, −32768..32767,
 *   0..255, −128..127) convertie avec le facteur et l'offset d'affichage :
 *   au-delà l'octet bouclerait de toute façon.
 */
export interface MapValueRange {
  min: number;
  max: number;
}

export function getMapValueRange(
  map: { name?: string; unit?: string | null; data_type?: string },
  factor: number,
  offset: number,
): MapValueRange | null {
  const name = (map.name || "").toLowerCase();
  if (name.includes("n75")) {
    return { min: 0, max: 100 };
  }
  const dt = String(map.data_type || "UInt16");
  const raw: [number, number] =
    dt === "Int16" ? [-32768, 32767]
    : dt === "UInt8" ? [0, 255]
    : dt === "Int8" ? [-128, 127]
    : dt === "UInt16" ? [0, 65535]
    : [Number.NEGATIVE_INFINITY, Number.POSITIVE_INFINITY];
  if (!Number.isFinite(raw[0]) || !Number.isFinite(factor) || factor === 0) {
    return null;
  }
  const a = raw[0] * factor + offset;
  const b = raw[1] * factor + offset;
  return { min: Math.min(a, b), max: Math.max(a, b) };
}

export function clampMapValue(value: number, range: MapValueRange | null): number {
  if (!range || !Number.isFinite(value)) return value;
  return Math.min(range.max, Math.max(range.min, value));
}
