/**
 * EDC16U1 DTC Module (1MB files)
 *
 * Thin facade over the shared EDC16 engine (edc16-core.ts) configured for
 * the 1MB variant.
 *
 * IMPORTANT: the control zone location VARIES by firmware — verified on the
 * 14-file U1 corpus: 0xDAA9E, 0xDAAB6, 0xDAB06, 0xDAB0A, 0xDAD16, 0xE896C...
 * A previous implementation used a FIXED zone at 0xDAB00, which was
 * misaligned for most firmwares. The zone is now located dynamically via
 * the same table-end signature as EDC16U31 ([00 EC 00 EC 00 EC 46 EC],
 * present on every U1 firmware of the corpus).
 *
 * The control zone CONTENT matches the U34/U31 layout (same leading byte
 * sequence on the whole corpus), so the shared EDC16_DTC_CONTROL_MAP
 * applies. Validated: ~498 enabled / ~13 disabled on stock files (same
 * profile as U34/U31), and a known stage-2 file shows 62 disabled DTCs
 * exactly where a DTC-off tool wrote its zeros.
 */

import {
  CONTROL_ZONE_SIGNATURE_U31,
  detectEDC16FamilyDTCs,
  disableEDC16FamilyDTC,
  enableEDC16FamilyDTC,
  getEDC16FamilyDTCSystems,
  hasControlMapping,
  type EDC16VariantConfig,
} from './edc16-core';
import { EDC16_DTC_CONTROL_MAP, type DTCControlMapping } from './edc16-control-map';
import type { DetectedDTC, CodeblockInfo, DTCDetectionResult } from './types';

// EDC16U1 specific constants
export const EDC16U1_FILE_SIZE = 1048576; // 1MB

export { hasControlMapping };

const EDC16U1_CONFIG: EDC16VariantConfig = {
  ecuType: 'EDC16U1',
  fileSize: EDC16U1_FILE_SIZE,
  zoneSignatures: [CONTROL_ZONE_SIGNATURE_U31],
  // U1 firmwares place the zone around 0xDAxxx, with one known variant
  // at 0xE8xxx — scan the whole calibration range
  searchStart: 0xd0000,
  searchEnd: 0xf0000,
};

/**
 * Detect all DTCs in an EDC16U1 file
 */
export function detectEDC16U1DTCs(data: Uint8Array): DTCDetectionResult {
  return detectEDC16FamilyDTCs(data, EDC16U1_CONFIG);
}

/**
 * Disable a DTC in EDC16U1 binary data (zeroes both control bytes)
 */
export function disableEDC16U1DTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  _codeblocks: CodeblockInfo[]
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  return disableEDC16FamilyDTC(data, dtc, EDC16U1_CONFIG);
}

/**
 * Enable a DTC in EDC16U1 binary data (restores the enabled value)
 */
export function enableEDC16U1DTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  _codeblocks: CodeblockInfo[]
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  return enableEDC16FamilyDTC(data, dtc, EDC16U1_CONFIG);
}

/**
 * Get unique systems from detected DTCs
 */
export function getEDC16U1DTCSystems(dtcs: DetectedDTC[]): string[] {
  return getEDC16FamilyDTCSystems(dtcs);
}

/**
 * Get the control zone address for a DTC (for debugging/display).
 * The control zone is located dynamically per file, so a static address
 * cannot be derived from the VAG code alone.
 */
export function getControlAddress(_vagCode: number): number | null {
  return null;
}

/**
 * Get all known DTC control mappings
 */
export function getAllControlMappings(): Record<number, DTCControlMapping> {
  return { ...EDC16_DTC_CONTROL_MAP };
}
