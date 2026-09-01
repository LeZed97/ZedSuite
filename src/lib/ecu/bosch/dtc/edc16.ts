/**
 * EDC16 DTC Detection and Modification Module - Dispatcher
 *
 * This module serves as a dispatcher for different EDC16 ECU variants:
 * - EDC16U1: 1MB files (uses edc16u1.ts)
 * - EDC16U34/C34: 2MB files (uses edc16u34.ts)
 *
 * The dispatcher automatically detects the ECU variant based on file size
 * and routes to the appropriate implementation.
 */

import type { DetectedDTC, CodeblockInfo, DTCDetectionResult } from './types';

// Import variant-specific implementations
import {
  EDC16U1_FILE_SIZE,
  detectEDC16U1DTCs,
  disableEDC16U1DTC,
  enableEDC16U1DTC,
  getEDC16U1DTCSystems,
  hasControlMapping as hasEDC16U1Mapping,
  getControlAddress as getEDC16U1ControlAddress,
} from './edc16u1';

import {
  EDC16U34_FILE_SIZE,
  detectEDC16U34DTCs,
  disableEDC16U34DTC,
  enableEDC16U34DTC,
  getEDC16U34DTCSystems,
  hasControlMapping as hasEDC16U34Mapping,
  isModificationSupported as isEDC16U34ModSupported,
} from './edc16u31-34';

// Re-export file size constants
export { EDC16U1_FILE_SIZE, EDC16U34_FILE_SIZE };

// Minimum file size for EDC16 detection
const EDC16_MIN_FILE_SIZE = 1048576; // 1MB

/**
 * Detect the ECU subtype based on file size
 */
export function detectECUSubtype(
  dataLength: number
): 'EDC16U1' | 'EDC16U34' | 'EDC16C34' | 'EDC16_UNKNOWN' {
  if (dataLength === EDC16U1_FILE_SIZE) {
    return 'EDC16U1';
  } else if (dataLength === EDC16U34_FILE_SIZE) {
    // Could be U34 or C34, both are 2MB
    // For now, treat both as U34 since they share similar structure
    return 'EDC16U34';
  }
  return 'EDC16_UNKNOWN';
}

/**
 * Detect all DTCs in an EDC16 file
 *
 * Automatically routes to the appropriate variant implementation
 */
export function detectEDC16DTCs(data: Uint8Array): DTCDetectionResult {
  // Validate minimum file size
  if (data.length < EDC16_MIN_FILE_SIZE) {
    return {
      success: false,
      ecuType: 'EDC16',
      codeblocks: [],
      dtcs: [],
      errors: [
        `File too small for EDC16 (${data.length} bytes, expected at least ${EDC16_MIN_FILE_SIZE} bytes)`,
      ],
    };
  }

  const subtype = detectECUSubtype(data.length);

  switch (subtype) {
    case 'EDC16U1':
      return detectEDC16U1DTCs(data);

    case 'EDC16U34':
    case 'EDC16C34':
      return detectEDC16U34DTCs(data);

    default:
      return {
        success: false,
        ecuType: 'EDC16',
        codeblocks: [],
        dtcs: [],
        errors: [`Unknown EDC16 variant (file size: ${data.length} bytes)`],
      };
  }
}

/**
 * Disable a DTC in EDC16 binary data
 *
 * Automatically routes to the appropriate variant implementation
 */
export function disableEDC16DTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  codeblocks: CodeblockInfo[]
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  const subtype = detectECUSubtype(data.length);


  switch (subtype) {
    case 'EDC16U1':
      return disableEDC16U1DTC(data, dtc, codeblocks);

    case 'EDC16U34':
    case 'EDC16C34':
      return disableEDC16U34DTC(data, dtc, codeblocks);

    default:
      console.warn(`[EDC16] Unknown EDC16 variant (${subtype}), cannot disable DTC`);
      return { modifiedData: new Uint8Array(data), changedAddresses: [] };
  }
}

/**
 * Enable a DTC in EDC16 binary data
 *
 * Automatically routes to the appropriate variant implementation
 */
export function enableEDC16DTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  codeblocks: CodeblockInfo[]
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  const subtype = detectECUSubtype(data.length);

  switch (subtype) {
    case 'EDC16U1':
      return enableEDC16U1DTC(data, dtc, codeblocks);

    case 'EDC16U34':
    case 'EDC16C34':
      return enableEDC16U34DTC(data, dtc, codeblocks);

    default:
      console.warn(`Unknown EDC16 variant, cannot enable DTC`);
      return { modifiedData: new Uint8Array(data), changedAddresses: [] };
  }
}

/**
 * Get unique systems from detected DTCs
 */
export function getEDC16DTCSystems(dtcs: DetectedDTC[]): string[] {
  const systems = new Set<string>();
  dtcs.forEach((dtc) => {
    if (dtc.system) {
      systems.add(dtc.system);
    }
  });
  return Array.from(systems).sort();
}

/**
 * Check if a DTC has a known control mapping for the given ECU subtype
 */
export function hasDTCControlMapping(vagCode: number, ecuSubtype: string): boolean {
  switch (ecuSubtype) {
    case 'EDC16U1':
      return hasEDC16U1Mapping(vagCode);
    case 'EDC16U34':
    case 'EDC16C34':
      return hasEDC16U34Mapping(vagCode);
    default:
      return false;
  }
}

/**
 * Get the control zone address for a DTC (for debugging/display purposes)
 */
export function getDTCControlAddress(vagCode: number, ecuSubtype: string): number | null {
  switch (ecuSubtype) {
    case 'EDC16U1':
      return getEDC16U1ControlAddress(vagCode);
    case 'EDC16U34':
    case 'EDC16C34':
      // Not implemented yet for U34
      return null;
    default:
      return null;
  }
}

/**
 * Check if DTC modification is fully supported for an ECU subtype
 */
export function isModificationSupported(ecuSubtype: string): boolean {
  switch (ecuSubtype) {
    case 'EDC16U1':
      return true; // EDC16U1 has partial mapping implemented
    case 'EDC16U34':
    case 'EDC16C34':
      return isEDC16U34ModSupported();
    default:
      return false;
  }
}

/**
 * Get supported EDC16 variants
 */
export function getSupportedEDC16Variants(): string[] {
  return ['EDC16U1', 'EDC16U34', 'EDC16C34'];
}
