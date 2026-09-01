/**
 * EDC16 DTC Core - shared detection/modification engine
 *
 * Every EDC16 variant (U34/U31 2MB, U1 1MB) uses the same CONTROL ZONE
 * mechanism: a calibration table of [value x3][id] entries ends with a
 * known 8-byte signature, and the control zone (one control byte per DTC,
 * layout described by EDC16_DTC_CONTROL_MAP) starts right after it.
 *
 * Only the file size, the zone signature(s) and the search range differ
 * between variants — they are described by an EDC16VariantConfig and the
 * variant modules (edc16u34.ts, edc16u1.ts) are thin facades over this
 * engine.
 *
 * Zone signatures observed on the test corpus:
 * - [81 D4 81 D4 81 D4 17 48]  U34 firmwares + U31 hybrid layouts
 * - [00 EC 00 EC 00 EC 46 EC]  U31 firmwares + ALL U1 firmwares
 */

import { getDTCInfoByPCode, isDocumentedDTC } from './dtc-database';
import { EDC16_DTC_CONTROL_MAP } from './edc16-control-map';
import type { DetectedDTC, CodeblockInfo, DTCDetectionResult } from './types';

/** Control zone size (approximately 400 bytes) */
export const CONTROL_ZONE_SIZE = 400;

/** Table-end signature used by EDC16U34 firmwares (and U31 hybrid layouts) */
export const CONTROL_ZONE_SIGNATURE_U34 = new Uint8Array([
  0x81, 0xd4, 0x81, 0xd4, 0x81, 0xd4, 0x17, 0x48,
]);

/** Table-end signature used by EDC16U31 firmwares and all EDC16U1 firmwares */
export const CONTROL_ZONE_SIGNATURE_U31 = new Uint8Array([
  0x00, 0xec, 0x00, 0xec, 0x00, 0xec, 0x46, 0xec,
]);

/** Per-variant description of where and how to find the control zone */
export interface EDC16VariantConfig {
  /** Reported in results and log messages (e.g. "EDC16U34") */
  ecuType: string;
  /** Exact expected file size in bytes */
  fileSize: number;
  /** Zone signatures to try, in priority order */
  zoneSignatures: readonly Uint8Array[];
  /** Search range for the signatures */
  searchStart: number;
  searchEnd: number;
}

/**
 * Find the control zone start address by searching for the variant's
 * signatures (in order). The zone starts right after the signature.
 */
export function findControlZoneStart(
  data: Uint8Array,
  config: EDC16VariantConfig
): number | null {
  const searchEnd = Math.min(config.searchEnd, data.length);

  for (const signature of config.zoneSignatures) {
    for (let i = config.searchStart; i < searchEnd - signature.length; i++) {
      let found = true;
      for (let j = 0; j < signature.length; j++) {
        if (data[i + j] !== signature[j]) {
          found = false;
          break;
        }
      }
      if (found) {
        // Control zone starts right after the signature
        return i + signature.length;
      }
    }
  }

  return null;
}

/**
 * Detect all DTCs of an EDC16 file
 *
 * Detection is driven by the A2L-derived control map: every DTC known for
 * the EDC16 family is listed, with its enabled state read from the control
 * zone. A DTC is considered disabled only when ALL of its control bytes
 * (primary + alternate offset) are 0x00 — consistent with
 * disableEDC16FamilyDTC which zeroes both offsets.
 */
export function detectEDC16FamilyDTCs(
  data: Uint8Array,
  config: EDC16VariantConfig
): DTCDetectionResult {
  const errors: string[] = [];
  const dtcs: DetectedDTC[] = [];

  // Validate file size
  if (data.length !== config.fileSize) {
    errors.push(
      `Invalid file size for ${config.ecuType} (${data.length} bytes, expected ${config.fileSize} bytes)`
    );
    return {
      success: false,
      ecuType: config.ecuType,
      codeblocks: [],
      dtcs: [],
      errors,
    };
  }

  // Find control zone dynamically
  const controlZoneStart = findControlZoneStart(data, config);
  if (!controlZoneStart) {
    errors.push(
      `${config.ecuType}: Could not locate control zone (signature not found). DTC detection is not possible for this file.`
    );
    return {
      success: false,
      ecuType: config.ecuType,
      codeblocks: [],
      dtcs: [],
      errors,
    };
  }

  // Single virtual codeblock covering the control zone
  const codeblocks: CodeblockInfo[] = [
    {
      id: 1,
      startAddress: controlZoneStart,
      endAddress: controlZoneStart + CONTROL_ZONE_SIZE,
      metadataAddress: controlZoneStart,
      isValid: true,
    },
  ];

  // List every DTC known from the A2L control map
  for (const [vagCodeStr, mapping] of Object.entries(EDC16_DTC_CONTROL_MAP)) {
    const vagCode = Number(vagCodeStr);
    const pCode = mapping.description; // description holds the P-code label

    // Hide codes with no publicly documented description (Bosch internal
    // P8xxx codes, unidentified VAG codes) — showing them without a real
    // meaning would only confuse users.
    if (!isDocumentedDTC(pCode)) continue;

    const primaryAddress = controlZoneStart + mapping.offset;
    if (primaryAddress >= data.length) continue;

    const primaryValue = data[primaryAddress];
    const altValue =
      mapping.offsetAlt !== undefined && controlZoneStart + mapping.offsetAlt < data.length
        ? data[controlZoneStart + mapping.offsetAlt]
        : null;

    // Disabled only when every mapped control byte is zeroed
    const enabled = primaryValue !== 0x00 || (altValue !== null && altValue !== 0x00);

    const dtcInfo = getDTCInfoByPCode(pCode);

    dtcs.push({
      code: pCode,
      vagCode,
      address: primaryAddress,
      codeblockId: 1,
      enabled,
      description: dtcInfo?.description || `DTC ${pCode}`,
      system: dtcInfo?.system || 'Autre',
      canDisable: true,
    });
  }

  dtcs.sort((a, b) => a.code.localeCompare(b.code));

  const success = dtcs.length > 0;
  if (!success && errors.length === 0) {
    errors.push('No DTCs detected in the file');
  }

  return {
    success,
    ecuType: config.ecuType,
    codeblocks: codeblocks.filter((cb) => cb.isValid),
    dtcs,
    errors,
    // Store control zone info for later use
    controlZoneStart,
  } as DTCDetectionResult & { controlZoneStart?: number };
}

/**
 * Disable a DTC: set its control byte(s) to 0x00 (primary + alternate)
 */
export function disableEDC16FamilyDTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  config: EDC16VariantConfig
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  const modifiedData = new Uint8Array(data);
  const changedAddresses: number[] = [];

  // Find control zone
  const controlZoneStart = findControlZoneStart(data, config);
  if (!controlZoneStart) {
    console.warn(`[${config.ecuType}] Could not locate control zone. DTC disable failed.`);
    return { modifiedData, changedAddresses };
  }

  const mapping = EDC16_DTC_CONTROL_MAP[dtc.vagCode];

  if (mapping) {
    // Primary offset
    const controlAddress = controlZoneStart + mapping.offset;
    if (controlAddress < data.length && modifiedData[controlAddress] !== 0x00) {
      modifiedData[controlAddress] = 0x00;
      changedAddresses.push(controlAddress);
    }

    // Alternate offset if exists
    if (mapping.offsetAlt !== undefined) {
      const altAddress = controlZoneStart + mapping.offsetAlt;
      if (altAddress < data.length && modifiedData[altAddress] !== 0x00) {
        modifiedData[altAddress] = 0x00;
        changedAddresses.push(altAddress);
      }
    }
  } else {
    console.warn(
      `${config.ecuType}: No control mapping found for VAG code ${dtc.vagCode} (${dtc.code}). ` +
        `This DTC requires manual mapping.`
    );
  }

  return { modifiedData, changedAddresses };
}

/**
 * Enable a DTC: restore its control byte(s) to the enabled value
 */
export function enableEDC16FamilyDTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  config: EDC16VariantConfig
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  const modifiedData = new Uint8Array(data);
  const changedAddresses: number[] = [];

  // Find control zone
  const controlZoneStart = findControlZoneStart(data, config);
  if (!controlZoneStart) {
    console.warn(`[${config.ecuType}] Could not locate control zone. DTC enable failed.`);
    return { modifiedData, changedAddresses };
  }

  const mapping = EDC16_DTC_CONTROL_MAP[dtc.vagCode];

  if (mapping) {
    // Restore primary offset
    const controlAddress = controlZoneStart + mapping.offset;
    if (controlAddress < data.length) {
      modifiedData[controlAddress] = mapping.enabledValue;
      changedAddresses.push(controlAddress);
    }

    // Restore alternate offset if exists
    if (mapping.offsetAlt !== undefined) {
      const altAddress = controlZoneStart + mapping.offsetAlt;
      if (altAddress < data.length) {
        modifiedData[altAddress] = mapping.enabledValue;
        changedAddresses.push(altAddress);
      }
    }
  } else {
    console.warn(
      `${config.ecuType}: No control mapping found for VAG code ${dtc.vagCode} (${dtc.code}).`
    );
  }

  return { modifiedData, changedAddresses };
}

/**
 * Get unique systems from detected DTCs
 */
export function getEDC16FamilyDTCSystems(dtcs: DetectedDTC[]): string[] {
  const systems = new Set<string>();
  dtcs.forEach((dtc) => {
    if (dtc.system) {
      systems.add(dtc.system);
    }
  });
  return Array.from(systems).sort();
}

/**
 * Check if a DTC has a known control mapping
 */
export function hasControlMapping(vagCode: number): boolean {
  return vagCode in EDC16_DTC_CONTROL_MAP;
}

/**
 * Get control zone information for debugging
 */
export function getControlZoneInfo(
  data: Uint8Array,
  config: EDC16VariantConfig
): {
  found: boolean;
  address?: number;
  signature: string;
} {
  const start = findControlZoneStart(data, config);
  return {
    found: start !== null,
    address: start ?? undefined,
    signature: Array.from(config.zoneSignatures[0])
      .map((b) => b.toString(16).padStart(2, '0'))
      .join(' '),
  };
}
