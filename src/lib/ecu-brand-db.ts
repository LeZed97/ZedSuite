// ECU reference -> vehicle brand database (VAG only).
//
// ecu-brand-db.json maps normalized ECU references to the vehicle they were
// fitted in: Bosch order numbers ("0281011823") and VAG part numbers
// ("038906019NJ", "045906019AB"). Factual data compiled from publicly known
// VAG/Bosch part number listings (referenced from the VAGEDCSuite freeware
// database); ambiguous references (same number used by several brands) are
// excluded at generation time, so a hit is always safe to auto-fill.
//
// Coverage today is strong for EDC15 (Bosch numbers + 038906019xx refs) and
// thin for EDC16 (2MB dumps only expose 03G906016xx-style refs, which the
// source database barely lists). The JSON is meant to grow: add entries as
// `"REF": {"b": "Brand", "m": "Model", "e": "EngineCode"}` with REF
// uppercased and stripped of spaces.

import db from "./ecu-brand-db.json";

export interface EcuBrandInfo {
  /** Vehicle brand: Audi | Seat | Skoda | Volkswagen */
  b: string;
  /** Vehicle model (e.g. "Golf", "Ibiza") when known */
  m?: string;
  /** Engine code (e.g. "ARL", "BLT") when known */
  e?: string;
}

const REFS: Record<string, EcuBrandInfo> = (db as { refs: Record<string, EcuBrandInfo> }).refs;

/**
 * Extract candidate references from a free-form identification string.
 * Handles both raw fields ("0281011823", "03G906016KN") and composite ones
 * ("038906019 NJ SG7331" — VAG ref + suffix + SG code).
 */
function extractRefs(field: string | null | undefined): string[] {
  if (!field) return [];
  const up = field.toUpperCase();
  const out: string[] = [];

  // Whole field, normalized (covers exact keys like "0261S01002")
  const compact = up.replace(/[\s.]/g, "");
  out.push(compact);

  // Bosch/VAG numeric references: 0281/0261/0986/1037...
  const numeric = compact.match(/(?:02|09|10)\d{8}/g);
  if (numeric) out.push(...numeric);

  // VAG part numbers: XXX 906 XXX [YY] — spaces optional, suffix 0-2 letters.
  // Matched on the raw string so a following code ("SG7331") cannot bleed
  // into the suffix; both the suffixed and unsuffixed forms are candidates.
  const vagRe = /([0-9][0-9A-Z]{2})\s?(906)\s?(\d{3})\s?([A-Z]{1,2})?(?![A-Z0-9])/g;
  let m: RegExpExecArray | null;
  while ((m = vagRe.exec(up)) !== null) {
    const base = `${m[1]}${m[2]}${m[3]}`;
    if (m[4]) out.push(`${base}${m[4]}`);
    out.push(base);
  }
  return out;
}

/**
 * Look up the vehicle brand for a set of identification fields
 * (hardware number, software number, part number). Returns the first hit,
 * or null when no reference is known.
 */
export function lookupEcuBrand(
  ...fields: (string | null | undefined)[]
): EcuBrandInfo | null {
  for (const field of fields) {
    for (const ref of extractRefs(field)) {
      const hit = REFS[ref];
      if (hit) return hit;
    }
  }
  return null;
}
