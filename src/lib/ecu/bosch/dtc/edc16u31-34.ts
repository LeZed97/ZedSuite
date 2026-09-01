/**
 * EDC16U34/EDC16U31 DTC Module (2MB files)
 *
 * Thin facade over the shared EDC16 engine (edc16-core.ts) configured for
 * the 2MB variants. The control zone location varies by firmware version
 * and is found dynamically through the table-end signature:
 * - EDC16U34 firmwares:  81 D4 81 D4 81 D4 17 48
 * - EDC16U31 firmwares:  00 EC 00 EC 00 EC 46 EC
 *   (verified on VW_Passat_2_0TDI_1037382052 @0x1CBDF6, Superb MK1 @0x1CBF7A,
 *    edc16u31full @0x1CBE7E — never present on the 33-file U34 corpus;
 *    some U31 firmwares like 03G906016JA/crafter use the U34 signature)
 *
 * Known control zone locations by firmware:
 * - U34: 0x1CD196, 0x1CD19A, 0x1CD21A, 0x1CEECC, 0x1CF216, 0x1CF2AC
 * - U31: 0x1CBDFE, 0x1CBF82, 0x1CBE86, 0x1D04D0 (U34-signature variants)
 *
 * The DTC layout of the zone is the shared EDC16_DTC_CONTROL_MAP
 * (edc16-control-map.ts). The VAG codes themselves are NOT modified,
 * only the control bytes change (0x00 = disabled).
 */

import {
  CONTROL_ZONE_SIGNATURE_U34,
  CONTROL_ZONE_SIGNATURE_U31,
  detectEDC16FamilyDTCs,
  disableEDC16FamilyDTC,
  enableEDC16FamilyDTC,
  getEDC16FamilyDTCSystems,
  getControlZoneInfo as getFamilyControlZoneInfo,
  hasControlMapping,
  type EDC16VariantConfig,
} from './edc16-core';
import { EDC16_DTC_CONTROL_MAP } from './edc16-control-map';
import type { DetectedDTC, CodeblockInfo, DTCDetectionResult } from './types';

// EDC16U34 specific constants
export const EDC16U34_FILE_SIZE = 2097152; // 2MB

// Re-export the shared control map under its historical home
export { EDC16_DTC_CONTROL_MAP };
export { hasControlMapping };

const EDC16U34_CONFIG: EDC16VariantConfig = {
  ecuType: 'EDC16U34',
  fileSize: EDC16U34_FILE_SIZE,
  // U34 signature first, then the U31 variant
  zoneSignatures: [CONTROL_ZONE_SIGNATURE_U34, CONTROL_ZONE_SIGNATURE_U31],
  searchStart: 0x1c0000,
  searchEnd: 0x1d5000,
};

/**
 * Detect all DTCs in an EDC16U34/U31 file
 */
export function detectEDC16U34DTCs(data: Uint8Array): DTCDetectionResult {
  return detectEDC16FamilyDTCs(data, EDC16U34_CONFIG);
}

/**
 * Disable a DTC in EDC16U34 binary data (zeroes both control bytes)
 */
export function disableEDC16U34DTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  _codeblocks: CodeblockInfo[]
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  return disableEDC16FamilyDTC(data, dtc, EDC16U34_CONFIG);
}

/**
 * Enable a DTC in EDC16U34 binary data (restores the enabled value)
 */
export function enableEDC16U34DTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  _codeblocks: CodeblockInfo[]
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  return enableEDC16FamilyDTC(data, dtc, EDC16U34_CONFIG);
}

/**
 * Get unique systems from detected DTCs
 */
export function getEDC16U34DTCSystems(dtcs: DetectedDTC[]): string[] {
  return getEDC16FamilyDTCSystems(dtcs);
}

/**
 * Check if DTC modification is supported for this ECU type
 */
export function isModificationSupported(): boolean {
  return Object.keys(EDC16_DTC_CONTROL_MAP).length > 0;
}

/**
 * Get control zone information for debugging
 */
export function getControlZoneInfo(data: Uint8Array): {
  found: boolean;
  address?: number;
  signature: string;
} {
  return getFamilyControlZoneInfo(data, EDC16U34_CONFIG);
}
