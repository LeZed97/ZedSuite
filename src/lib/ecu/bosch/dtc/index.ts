/**
 * DTC (Diagnostic Trouble Code) Module
 *
 * This module provides ECU-specific DTC detection and modification.
 * Each ECU family has its own implementation file with the correct
 * binary structure for that ECU type.
 *
 * Structure:
 * - index.ts: Common types, exports, and dispatcher
 * - edc15p.ts: EDC15P family (VAG) - 512KB files
 * - edc16.ts: EDC16 family (VAG) - 2MB files
 */

// Re-export types
export type {
  DetectedDTC,
  CodeblockInfo,
  DTCDetectionResult,
} from './types';

// Re-export implementations
export {
  detectEDC15PDTCs,
  disableEDC15PDTC,
  enableEDC15PDTC,
  getEDC15PDTCSystems,
} from './edc15p';

export {
  detectEDC16DTCs,
  disableEDC16DTC,
  enableEDC16DTC,
  getEDC16DTCSystems,
  detectECUSubtype,
  hasDTCControlMapping,
  getDTCControlAddress,
  isModificationSupported,
  getSupportedEDC16Variants,
  EDC16U1_FILE_SIZE,
  EDC16U34_FILE_SIZE,
} from './edc16';

// Re-export variant-specific implementations for direct access
export {
  detectEDC16U1DTCs,
  disableEDC16U1DTC,
  enableEDC16U1DTC,
  getEDC16U1DTCSystems,
} from './edc16u1';

export {
  detectEDC16U34DTCs,
  disableEDC16U34DTC,
  enableEDC16U34DTC,
  getEDC16U34DTCSystems,
} from './edc16u31-34';

// Re-export database (now in same folder)
export { VAG_DTC_DATABASE, convertVagToPCode } from './dtc-database';

// Re-export i18n
export {
  translateSystemName,
  translateDTCDescription,
  type DTCLanguage,
} from './dtc-i18n';

// `./edc16` only implements the VAG EDC16 variants (it picks U1 vs U34/C34
// by FILE SIZE alone once it's reached, a real, size-only heuristic, so it
// must never be reached for a non-VAG EDC16 file). `family.startsWith('EDC16')`
// alone also matches EDC16C39 (Fiat/Alfa) and any future non-VAG EDC16
// family; restricted to the variants this module actually implements.
const VAG_EDC16_VARIANTS = ['EDC16U1', 'EDC16U34', 'EDC16C34'];
function isVagEdc16(family: string): boolean {
  return VAG_EDC16_VARIANTS.some((v) => family.includes(v));
}

/**
 * Detect DTCs based on ECU type
 */
export function detectDTCs(
  data: Uint8Array,
  ecuFamily: string
): import('./types').DTCDetectionResult {
  const family = ecuFamily.toUpperCase();

  if (family.startsWith('EDC15')) {
    const { detectEDC15PDTCs } = require('./edc15p');
    return detectEDC15PDTCs(data, family);
  }

  if (isVagEdc16(family)) {
    const { detectEDC16DTCs } = require('./edc16');
    return detectEDC16DTCs(data);
  }

  // Default: try EDC15P detection
  const { detectEDC15PDTCs } = require('./edc15p');
  return detectEDC15PDTCs(data);
}

/**
 * Disable a DTC based on ECU type
 */
export function disableDTC(
  data: Uint8Array,
  dtc: import('./types').DetectedDTC,
  codeblocks: import('./types').CodeblockInfo[],
  ecuFamily: string
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  const family = ecuFamily.toUpperCase();


  if (family.startsWith('EDC15')) {
    const { disableEDC15PDTC } = require('./edc15p');
    return disableEDC15PDTC(data, dtc, codeblocks);
  }

  if (isVagEdc16(family)) {
    const { disableEDC16DTC } = require('./edc16');
    return disableEDC16DTC(data, dtc, codeblocks);
  }

  // Default: try EDC15P
  const { disableEDC15PDTC } = require('./edc15p');
  return disableEDC15PDTC(data, dtc, codeblocks);
}

/**
 * Enable a DTC based on ECU type
 */
export function enableDTC(
  data: Uint8Array,
  dtc: import('./types').DetectedDTC,
  codeblocks: import('./types').CodeblockInfo[],
  ecuFamily: string
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  const family = ecuFamily.toUpperCase();

  if (family.startsWith('EDC15')) {
    const { enableEDC15PDTC } = require('./edc15p');
    return enableEDC15PDTC(data, dtc, codeblocks);
  }

  if (isVagEdc16(family)) {
    const { enableEDC16DTC } = require('./edc16');
    return enableEDC16DTC(data, dtc, codeblocks);
  }

  // Default: try EDC15P
  const { enableEDC15PDTC } = require('./edc15p');
  return enableEDC15PDTC(data, dtc, codeblocks);
}

/**
 * Get supported ECU families for DTC operations
 */
export function getSupportedDTCFamilies(): string[] {
  return ['EDC15', ...VAG_EDC16_VARIANTS];
}

/**
 * Check if DTC operations are supported for an ECU family
 */
export function isDTCSupported(ecuFamily: string): boolean {
  const family = ecuFamily.toUpperCase();
  return family.startsWith('EDC15') || isVagEdc16(family);
}
