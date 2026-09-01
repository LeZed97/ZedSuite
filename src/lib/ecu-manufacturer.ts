/**
 * Constructeur d'un type d'ECU, côté client (miroir du champ `manufacturer`
 * de apps/map-detector/ecus.json — préfixes de familles, pas de fs ici).
 */
export function getEcuManufacturer(ecuType?: string | null): string | null {
  if (!ecuType) return null;
  const t = ecuType.toUpperCase().replace(/[\s_-]/g, "");
  if (t.startsWith("EDC") || t.startsWith("MED") || t.startsWith("ME7")) return "Bosch";
  if (t.startsWith("MJD") || t.includes("MAREL") || t.includes("MAGNETI")) return "Magneti Marelli";
  if (t.startsWith("SID") || t.startsWith("PCR") || t.startsWith("SIMOS")) return "Siemens";
  if (t.startsWith("DCM") || t.startsWith("DDCR")) return "Delphi";
  return null;
}

/** "EDC16U34" -> "Bosch EDC16U34" (inchangé si constructeur inconnu ou déjà présent). */
export function formatEcuWithManufacturer(ecuType?: string | null): string | null {
  if (!ecuType) return null;
  const manufacturer = getEcuManufacturer(ecuType);
  if (!manufacturer) return ecuType;
  // Skip the prefix if any word of the brand is already in the type
  // ("Marelli MJD6..." must not become "Magneti Marelli Marelli MJD6...")
  const upper = ecuType.toUpperCase();
  if (manufacturer.toUpperCase().split(" ").some(w => upper.includes(w))) return ecuType;
  return `${manufacturer} ${ecuType}`;
}
