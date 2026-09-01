/**
 * ECU Checksum Calculator Index
 *
 * This module provides checksum calculation and correction for various ECU types.
 * Currently supported:
 * - EDC15P / EDC15V / EDC15VM / EDC15M / EDC15C (Bosch VAG TDI, algorithme v4.1)
 * - EDC16U1 / EDC16U31 / EDC16U34 (Bosch VAG)
 */

export { correctEDC15PChecksum, verifyEDC15PChecksum, ChecksumResult } from './edc15p-checksum';
export type { ChecksumInfo } from './edc15p-checksum';
export {
  correctEDC16Checksum,
  verifyEDC16Checksum,
  analyzeEDC16Checksum,
  findEdc16Regions,
} from './edc16-checksum';
export type { Edc16Region, Edc16RegionStatus } from './edc16-checksum';

import { correctEDC15PChecksum } from './edc15p-checksum';
import { correctEDC16Checksum } from './edc16-checksum';

/**
 * Correct checksums based on ECU type
 *
 * @param ecuType - The ECU type string (e.g., "EDC15P", "EDC16U34")
 * @param fileData - The file data as a number array
 * @returns The corrected data and checksum info, or null if the ECU type is
 *          not supported (or the file carries no recognizable checksum block)
 */
export function correctChecksumByEcuType(
  ecuType: string | undefined,
  fileData: number[]
): { correctedData: number[]; info: import('./edc15p-checksum').ChecksumInfo } | null {
  if (!ecuType) return null;

  const ecuTypeUpper = ecuType.toUpperCase();

  // Famille EDC15 — EDC15P, EDC15V, EDC15VM, EDC15M et EDC15C partagent
  // l'algorithme Bosch VAG TDI v4.1 (mêmes seeds, mêmes constantes et même
  // table de points de contrôle 512 Ko), d'où le routage commun.
  if (ecuTypeUpper.includes('EDC15')) {
    return correctEDC15PChecksum(fileData);
  }

  // EDC16 family (U1 / U31 / U34) — region descriptors are self-describing,
  // so an unrecognized layout returns null rather than a bogus correction.
  if (ecuTypeUpper.includes('EDC16')) {
    return correctEDC16Checksum(fileData);
  }

  // Add more ECU types here as they are implemented (EDC17, MED17, …)

  return null;
}

/**
 * Check if checksum correction is supported for an ECU type
 */
export function isChecksumSupported(ecuType: string | undefined): boolean {
  if (!ecuType) return false;

  const ecuTypeUpper = ecuType.toUpperCase();

  // Toute la famille EDC15 (v4.1) et toute la famille EDC16
  const supportedTypes = ['EDC15', 'EDC16'];

  return supportedTypes.some(type => ecuTypeUpper.includes(type));
}
