/**
 * Single source of truth for ECU byte order in the web app.
 *
 * Big-endian ECUs: Bosch EDC16 family (MPC5xx) and Magneti Marelli MJD6
 * family (PowerPC) — 16-bit values are stored high byte first (eHiLo).
 * Everything else (EDC15 family, unknown) defaults to little-endian (eLoHi).
 *
 * Used by map-viewer.tsx (decode), page.tsx (write-back), compare-modal.tsx
 * and mappack-export.ts — they must all agree, otherwise the display and the
 * bytes written back to the binary diverge.
 */
export function isBigEndianEcu(ecuType: string | null | undefined): boolean {
  const t = (ecuType || "").toUpperCase();
  return t.includes("EDC16") || t.includes("MJD") || t.includes("MAREL");
}

/**
 * Marelli MJD6 axis values are UNSIGNED u16: RPM axes end with "infinity"
 * sentinels (18750/37500 raw, i.e. > 32767) and temperature axes are stored
 * as positive 0.25°C/bit values — interpreting them as signed i16 (the
 * Bosch behavior) turns the sentinels into bogus negative labels.
 */
export function hasUnsignedAxes(ecuType: string | null | undefined): boolean {
  const t = (ecuType || "").toUpperCase();
  return t.includes("MJD") || t.includes("MAREL");
}
