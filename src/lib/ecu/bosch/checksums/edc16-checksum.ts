/**
 * EDC16 Checksum Calculator
 * (Bosch EDC16U1 / EDC16U31 / EDC16U34 / EDC16CP31)
 *
 * Implémentation native, reversée depuis l'analyse d'une librairie de
 * référence et validée octet par octet contre 6 paires de fichiers
 * (U1 1 Mo, U31 et U34 2 Mo) : 6/6 reproduits exactement. Étendue ensuite
 * au Mercedes EDC16CP31 (OM642, SW 1037393817), qui a montré que
 * l'alignement de FIN de région supposé jusque-là était une propriété du
 * corpus VAG, pas du format — voir REGION_END_ALIGNMENT.
 *
 * ── Format ────────────────────────────────────────────────────────────
 * Chaque région protégée est annoncée par une signature de 8 octets
 *
 *     FA DE CA FE CA FE AF FE
 *
 * placée à `region_start + 0x3C`. Les deux dwords big-endian qui
 * PRÉCÈDENT la signature portent les bornes de la région, exprimées en
 * adresses mémoire ECU :
 *
 *     [magic - 8] = adresse de début
 *     [magic - 4] = adresse de fin (INCLUSIVE)
 *
 * La base mémoire (0x000000 sur les dumps 2 Mo, 0x800000 sur les 1 Mo)
 * n'est pas stockée : elle se déduit de la position de la signature
 * elle-même, puisqu'on sait qu'elle vit à `start + 0x3C`.
 *
 * Le DERNIER dword de la région (à `end + 1 - 4`) porte le checksum :
 * la somme de tous les dwords big-endian de la région — le mot de
 * checksum inclus — doit valoir la constante 0xD01FE500. La correction
 * consiste donc à absorber l'écart dans ce mot.
 */

import { ChecksumResult, type ChecksumInfo } from './edc15p-checksum';

/** Signature marquant le descripteur d'une région protégée. */
const MAGIC = [0xfa, 0xde, 0xca, 0xfe, 0xca, 0xfe, 0xaf, 0xfe];

/** Décalage de la signature à l'intérieur de sa région. */
const MAGIC_OFFSET_IN_REGION = 0x3c;

/**
 * Alignement du DÉBUT de région. La signature apparaît aussi à l'intérieur
 * des données de certains fichiers ; sans cette contrainte on fabriquerait
 * des régions fantômes (bornes absurdes mais cohérentes entre elles) et la
 * "correction" écrirait n'importe où. Toutes les vraies régions observées
 * démarrent sur une frontière de 0x100 — ce qui correspond au pas de
 * recherche utilisé par le calculateur d'origine.
 */
const REGION_ALIGNMENT = 0x100;

/**
 * Alignement de la FIN de région.
 *
 * La fin était contrainte sur 0x100 elle aussi, ce qui était vrai des six
 * paires VAG du corpus d'origine mais n'est PAS une propriété du format.
 * Sur Mercedes EDC16CP31 (OM642, SW 1037393817) la région déclarée est
 * 0x190000..0x1FCFFB : début aligné sur 0x100, mais `end + 1` = 0x1FCFFC,
 * qui n'est aligné que sur 4. La région était donc rejetée, `correctEDC16-
 * Checksum` retournait `null`, et l'éditeur affichait « Checksum non
 * supporté » sur un fichier dont le checksum est parfaitement lisible —
 * la somme de la région vaut exactement 0xD01FE500, mot de checksum inclus.
 *
 * La fin n'a besoin que d'être alignée sur un dword : le checksum est le
 * dernier dword de la région. La rigueur perdue est compensée par
 * MIN_REGION_LENGTH ci-dessous ; le vrai garde-fou reste l'alignement du
 * début, qui contraint la position de la signature elle-même.
 */
const REGION_END_ALIGNMENT = 4;

/**
 * Longueur minimale d'une région protégée. Une occurrence fortuite de la
 * signature produirait des bornes quelconques ; exiger une région d'au
 * moins 4 Ko élimine les fenêtres minuscules sans exclure aucune région
 * réelle (la plus petite observée fait plusieurs centaines de kilo-octets).
 */
const MIN_REGION_LENGTH = 0x1000;

/** Somme cible : invariant vérifié par le calculateur. */
const TARGET_SUM = 0xd01fe500;

export interface Edc16Region {
  /** Offset fichier du premier octet couvert */
  start: number;
  /** Offset fichier du dernier octet couvert (inclus) */
  end: number;
  /** Offset fichier du dword de checksum (end + 1 - 4) */
  checksumOffset: number;
}

/** Lit un dword big-endian sans passer par les opérateurs 32 bits signés. */
function readU32BE(data: ArrayLike<number>, offset: number): number {
  return (
    (data[offset] * 0x1000000 +
      (data[offset + 1] << 16) +
      (data[offset + 2] << 8) +
      data[offset + 3]) >>>
    0
  );
}

function writeU32BE(data: number[], offset: number, value: number): void {
  data[offset] = (value >>> 24) & 0xff;
  data[offset + 1] = (value >>> 16) & 0xff;
  data[offset + 2] = (value >>> 8) & 0xff;
  data[offset + 3] = value & 0xff;
}

/**
 * Localise toutes les régions protégées d'un fichier EDC16.
 * Une région n'est retenue que si ses bornes sont cohérentes avec la
 * taille du fichier et alignées sur 4 octets — ce qui écarte d'office
 * les occurrences fortuites de la signature dans les données.
 */
export function findEdc16Regions(fileData: ArrayLike<number>): Edc16Region[] {
  const size = fileData.length;
  const regions: Edc16Region[] = [];

  for (let i = 8; i + MAGIC.length <= size; i++) {
    if (fileData[i] !== MAGIC[0]) continue;
    let matched = true;
    for (let k = 1; k < MAGIC.length; k++) {
      if (fileData[i + k] !== MAGIC[k]) {
        matched = false;
        break;
      }
    }
    if (!matched) continue;

    const startAddr = readU32BE(fileData, i - 8);
    const endAddr = readU32BE(fileData, i - 4);

    // La signature vit à start + 0x3C : on en déduit la base mémoire.
    const base = startAddr - (i - MAGIC_OFFSET_IN_REGION);
    const start = startAddr - base;
    const end = endAddr - base;

    if (!(start >= 0 && start < end && end < size)) continue;
    if (start % REGION_ALIGNMENT !== 0) continue;
    if ((end + 1) % REGION_END_ALIGNMENT !== 0) continue;
    const length = end + 1 - start;
    if (length % 4 !== 0) continue;
    if (length < MIN_REGION_LENGTH) continue;
    // La signature doit vivre DANS sa propre région : sans ça, deux dwords
    // quelconques précédant une occurrence fortuite suffisent à fabriquer
    // une fenêtre cohérente ailleurs dans le fichier.
    if (i < start || i >= end) continue;

    regions.push({ start, end, checksumOffset: end + 1 - 4 });
  }

  return regions;
}

/** Somme (mod 2^32) des dwords big-endian sur [start, end]. */
function sumRegion(data: ArrayLike<number>, start: number, end: number): number {
  let sum = 0;
  for (let i = start; i + 3 <= end; i += 4) {
    sum = (sum + readU32BE(data, i)) >>> 0;
  }
  return sum;
}

export interface Edc16RegionStatus extends Edc16Region {
  /** Somme courante de la région */
  sum: number;
  /** Valeur du checksum actuellement stockée */
  stored: number;
  /** Valeur attendue pour satisfaire l'invariant */
  expected: number;
  valid: boolean;
}

/** Analyse chaque région sans rien modifier. */
export function analyzeEDC16Checksum(
  fileData: ArrayLike<number>
): Edc16RegionStatus[] {
  return findEdc16Regions(fileData).map((region) => {
    const sum = sumRegion(fileData, region.start, region.end);
    const stored = readU32BE(fileData, region.checksumOffset);
    const expected = (stored + (TARGET_SUM - sum)) >>> 0;
    return { ...region, sum, stored, expected, valid: sum === TARGET_SUM };
  });
}

/**
 * Corrige les checksums EDC16 d'un fichier.
 *
 * Retourne `null` si aucune région protégée n'est reconnue : le fichier
 * n'est alors pas un EDC16 de ce format et il n'y a rien à corriger
 * (l'appelant l'interprète comme « non supporté »).
 */
export function correctEDC16Checksum(
  fileData: number[]
): { info: ChecksumInfo; correctedData: number[] } | null {
  const statuses = analyzeEDC16Checksum(fileData);
  if (statuses.length === 0) return null;

  const correctedData = [...fileData];
  let fixed = 0;
  let matched = 0;

  for (const status of statuses) {
    if (status.valid) {
      matched++;
      continue;
    }
    writeU32BE(correctedData, status.checksumOffset, status.expected);
    fixed++;
  }

  return {
    correctedData,
    info: {
      result: fixed === 0 ? ChecksumResult.ChecksumOK : ChecksumResult.ChecksumFail,
      found: statuses.length,
      fixed,
      matched,
      variant: 'edc16',
    },
  };
}

/** Vérifie les checksums sans modifier les données. */
export function verifyEDC16Checksum(
  fileData: ArrayLike<number>
): ChecksumInfo | null {
  const statuses = analyzeEDC16Checksum(fileData);
  if (statuses.length === 0) return null;
  const matched = statuses.filter((s) => s.valid).length;
  return {
    result:
      matched === statuses.length
        ? ChecksumResult.ChecksumOK
        : ChecksumResult.ChecksumFail,
    found: statuses.length,
    fixed: statuses.length - matched,
    matched,
    variant: 'edc16',
  };
}
