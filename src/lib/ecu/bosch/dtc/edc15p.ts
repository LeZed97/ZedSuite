/**
 * EDC15P DTC Detection and Modification Module
 *
 * Handles VAG EDC15P ECU files (typically 512KB)
 * Supports files with 1, 2, or 3 codeblocks
 *
 * DTC Structure (8 bytes per entry):
 * [2 bytes: status LE] [2 bytes: flags LE] [2 bytes: type+marker] [2 bytes: VAG code LE]
 *
 * Disabled state: status=0xFFFF, flags=0x0000, type=0x00
 * Marker byte is always 0x23
 *
 * Examples:
 *   ENABLED (standard):  30 00 30 00 11 23 66 4C
 *   ENABLED (inverted):  00 00 FF FF 00 23 66 4C  (status/flags swapped)
 *   ENABLED (type 0x41): 30 00 30 00 41 23 65 4C
 *   DISABLED:            FF FF 00 00 00 23 66 4C
 */

import { VAG_DTC_DATABASE, convertVagToPCode } from './dtc-database';
import type { DetectedDTC, CodeblockInfo, DTCDetectionResult } from './types';

// EDC15P Codeblock definitions
const EDC15P_CODEBLOCKS: CodeblockInfo[] = [
  { id: 2, startAddress: 0x4C000, endAddress: 0x5BFFF, metadataAddress: 0x50000, isValid: false },
  { id: 3, startAddress: 0x5C000, endAddress: 0x6BFFF, metadataAddress: 0x60000, isValid: false },
  { id: 5, startAddress: 0x6C000, endAddress: 0x7BFFF, metadataAddress: 0x70000, isValid: false },
];

// Invalid metadata markers (codeblock not present)
const INVALID_METADATA_MARKERS = [0xC3C3, 0xFFFF, 0x0000];

// VAG DTC code range
const VAG_CODE_MIN = 16471; // P0087 - first common VAG DTC
const VAG_CODE_MAX = 20000; // Maximum valid VAG DTC code

// DTC marker bytes
const DTC_MARKER_BYTES = [0x23, 0x42, 0x58];

// DTC search area offset within codeblock
const DTC_AREA_START_OFFSET = 0x2800;
const DTC_AREA_END_OFFSET = 0x8100; // Extended to include DTCs at 0x8016/0x8020

/**
 * Check if a byte is a valid DTC marker
 */
function isDTCMarker(byte: number): boolean {
  return DTC_MARKER_BYTES.includes(byte);
}

/**
 * Determine if a DTC entry is enabled based on status, flags, and type byte
 *
 * A DTC is considered DISABLED when:
 * - status=0xFFFF AND flags=0x0000 AND type=0x00
 *
 * A DTC is considered ENABLED in these cases:
 * - Standard: status!=0xFFFF (e.g., 0x0030)
 * - Inverted: status=0x0000 AND flags=0xFFFF (status/flags swapped)
 * - Type active: status=0xFFFF but type!=0x00 (e.g., 0x11, 0x41)
 */
function isDTCEnabled(status: number, flags: number, typeByte: number): boolean {
  // Fully disabled: status=0xFFFF, flags=0x0000, type=0x00
  if (status === 0xffff && flags === 0x0000 && typeByte === 0x00) {
    return false;
  }

  // Inverted pattern: status=0x0000, flags=0xFFFF -> this is ENABLED
  if (status === 0x0000 && flags === 0xffff) {
    return true;
  }

  // Type byte active (0x11, 0x41, etc.) means monitoring is enabled
  // even if status is 0xFFFF
  if (typeByte !== 0x00 && typeByte !== 0x23) {
    return true;
  }

  // Standard case: status != 0xFFFF means enabled
  return status !== 0xffff;
}

/**
 * Detect which codeblocks are present in an EDC15P file
 */
function detectCodeblocks(data: Uint8Array): CodeblockInfo[] {
  const codeblocks: CodeblockInfo[] = [];

  for (const cb of EDC15P_CODEBLOCKS) {
    if (data.length < cb.metadataAddress + 2) {
      continue;
    }

    const metadataValue = (data[cb.metadataAddress + 1] << 8) | data[cb.metadataAddress];
    const isValid = !INVALID_METADATA_MARKERS.includes(metadataValue);

    codeblocks.push({
      ...cb,
      isValid,
    });
  }

  return codeblocks;
}

/**
 * Scan a codeblock for DTCs
 */
function scanCodeblockForDTCs(data: Uint8Array, codeblock: CodeblockInfo): DetectedDTC[] {
  const dtcs: DetectedDTC[] = [];
  const seen = new Set<number>();

  const searchStart = codeblock.startAddress + DTC_AREA_START_OFFSET;
  const searchEnd = Math.min(
    codeblock.startAddress + DTC_AREA_END_OFFSET,
    codeblock.endAddress,
    data.length - 8
  );

  if (searchStart >= data.length || searchEnd <= searchStart) {
    return dtcs;
  }

  // Primary method: Look for marker 0x23 followed by VAG code
  for (let i = searchStart; i < searchEnd - 1; i++) {
    if (data[i] === 0x23 && i >= 5) {
      const vagCode = data[i + 1] | (data[i + 2] << 8);

      if (vagCode >= VAG_CODE_MIN && vagCode <= VAG_CODE_MAX && !seen.has(vagCode)) {
        const dtcInfo = VAG_DTC_DATABASE[vagCode];

        if (dtcInfo) {
          seen.add(vagCode);

          // Read status, flags, and type byte
          // Structure: [status 2B] [flags 2B] [type 1B] [marker 0x23] [VAG code 2B]
          const status = data[i - 5] | (data[i - 4] << 8);
          const flags = data[i - 3] | (data[i - 2] << 8);
          const typeByte = data[i - 1];
          const enabled = isDTCEnabled(status, flags, typeByte);

          dtcs.push({
            code: dtcInfo.code,
            vagCode,
            address: i + 1,
            codeblockId: codeblock.id,
            enabled,
            description: dtcInfo.description,
            system: dtcInfo.system,
          });
        }
      }
    }
  }

  // Secondary method: Direct VAG code detection with marker validation
  for (let i = searchStart; i < searchEnd - 1; i++) {
    const vagCode = data[i] | (data[i + 1] << 8);

    if (
      vagCode >= VAG_CODE_MIN &&
      vagCode <= VAG_CODE_MAX &&
      !seen.has(vagCode) &&
      VAG_DTC_DATABASE[vagCode]
    ) {
      const prevByte = i > 0 ? data[i - 1] : 0;
      const hasMarker = isDTCMarker(prevByte);

      if (hasMarker && i >= 6) {
        seen.add(vagCode);

        const dtcInfo = VAG_DTC_DATABASE[vagCode];
        // Structure: [status 2B] [flags 2B] [type 1B] [marker] [VAG code 2B]
        const status = data[i - 6] | (data[i - 5] << 8);
        const flags = data[i - 4] | (data[i - 3] << 8);
        const typeByte = data[i - 2];
        const enabled = isDTCEnabled(status, flags, typeByte);

        dtcs.push({
          code: dtcInfo.code,
          vagCode,
          address: i,
          codeblockId: codeblock.id,
          enabled,
          description: dtcInfo.description,
          system: dtcInfo.system,
        });
      }
    }
  }

  return dtcs;
}

/**
 * Detect all DTCs in an EDC15P file
 */
export function detectEDC15PDTCs(data: Uint8Array): DTCDetectionResult {
  const errors: string[] = [];

  // Validate file size (EDC15P files are typically 512KB)
  if (data.length < 0x70000) {
    errors.push(`File too small for EDC15P (${data.length} bytes, expected at least 458752 bytes)`);
    return {
      success: false,
      ecuType: 'EDC15P',
      codeblocks: [],
      dtcs: [],
      errors,
    };
  }

  const codeblocks = detectCodeblocks(data);
  const validCodeblocks = codeblocks.filter((cb) => cb.isValid);

  if (validCodeblocks.length === 0) {
    errors.push('No valid codeblocks detected in the file');
    return {
      success: false,
      ecuType: 'EDC15P',
      codeblocks,
      dtcs: [],
      errors,
    };
  }

  const allDTCs: DetectedDTC[] = [];
  const seenCodes = new Set<string>();

  for (const codeblock of validCodeblocks) {
    const dtcs = scanCodeblockForDTCs(data, codeblock);

    for (const dtc of dtcs) {
      if (!seenCodes.has(dtc.code)) {
        seenCodes.add(dtc.code);
        allDTCs.push(dtc);
      }
    }
  }

  allDTCs.sort((a, b) => a.code.localeCompare(b.code));

  return {
    success: true,
    ecuType: 'EDC15P',
    codeblocks,
    dtcs: allDTCs,
    errors,
  };
}

/**
 * Disable a DTC in EDC15P binary data
 *
 * Sets the DTC entry to disabled state:
 * - Status: 0xFFFF (bytes -6, -5)
 * - Flags: 0x0000 (bytes -4, -3)
 * - Type: 0x00 (byte -2)
 * - Marker 0x23 unchanged (byte -1)
 *
 * This handles all cases including:
 * - Standard: [30 00] [30 00] [11 23] -> [FF FF] [00 00] [00 23]
 * - Inverted: [00 00] [FF FF] [00 23] -> [FF FF] [00 00] [00 23]
 * - Type 0x41: [30 00] [30 00] [41 23] -> [FF FF] [00 00] [00 23]
 * - Already 0xFFFF status but active type: [FF FF] [30 00] [11 23] -> [FF FF] [00 00] [00 23]
 */
export function disableEDC15PDTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  codeblocks: CodeblockInfo[]
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  const modifiedData = new Uint8Array(data);
  const changedAddresses: number[] = [];

  for (const codeblock of codeblocks.filter((cb) => cb.isValid)) {
    const searchStart = codeblock.startAddress + DTC_AREA_START_OFFSET;
    const searchEnd = Math.min(
      codeblock.startAddress + DTC_AREA_END_OFFSET,
      codeblock.endAddress,
      data.length - 8
    );

    for (let i = searchStart; i < searchEnd - 1; i++) {
      const vagCode = modifiedData[i] | (modifiedData[i + 1] << 8);

      if (vagCode === dtc.vagCode) {
        // Check if marker 0x23 is at i-1
        if (i >= 1 && modifiedData[i - 1] === 0x23) {
          if (i >= 6) {
            // Read current values to check if already disabled
            const currentStatus = modifiedData[i - 6] | (modifiedData[i - 5] << 8);
            const currentFlags = modifiedData[i - 4] | (modifiedData[i - 3] << 8);
            const currentType = modifiedData[i - 2];

            // Check if already fully disabled
            const isAlreadyDisabled =
              currentStatus === 0xffff && currentFlags === 0x0000 && currentType === 0x00;

            if (!isAlreadyDisabled) {
              // Set status to 0xFFFF (disabled)
              modifiedData[i - 6] = 0xff;
              modifiedData[i - 5] = 0xff;
              changedAddresses.push(i - 6, i - 5);

              // Set flags to 0x0000
              modifiedData[i - 4] = 0x00;
              modifiedData[i - 3] = 0x00;
              changedAddresses.push(i - 4, i - 3);

              // Set type to 0x00 (keep marker 0x23)
              modifiedData[i - 2] = 0x00;
              changedAddresses.push(i - 2);
            }
          }
        }
      }
    }
  }

  const uniqueAddresses = [...new Set(changedAddresses)].sort((a, b) => a - b);
  return { modifiedData, changedAddresses: uniqueAddresses };
}

/**
 * Enable a DTC in EDC15P binary data
 *
 * Sets the DTC entry to enabled state:
 * - Status: 0x0030 (bytes -6, -5)
 * - Flags: 0x0030 (bytes -4, -3)
 * - Type: 0x11 (byte -2)
 * - Marker 0x23 unchanged (byte -1)
 */
export function enableEDC15PDTC(
  data: Uint8Array,
  dtc: DetectedDTC,
  codeblocks: CodeblockInfo[]
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  const modifiedData = new Uint8Array(data);
  const changedAddresses: number[] = [];

  for (const codeblock of codeblocks.filter((cb) => cb.isValid)) {
    const searchStart = codeblock.startAddress + DTC_AREA_START_OFFSET;
    const searchEnd = Math.min(
      codeblock.startAddress + DTC_AREA_END_OFFSET,
      codeblock.endAddress,
      data.length - 8
    );

    for (let i = searchStart; i < searchEnd - 1; i++) {
      const vagCode = modifiedData[i] | (modifiedData[i + 1] << 8);

      if (vagCode === dtc.vagCode) {
        if (i >= 1 && modifiedData[i - 1] === 0x23) {
          if (i >= 6) {
            // Read current values to check if disabled
            const currentStatus = modifiedData[i - 6] | (modifiedData[i - 5] << 8);
            const currentFlags = modifiedData[i - 4] | (modifiedData[i - 3] << 8);
            const currentType = modifiedData[i - 2];

            // Only enable if currently disabled (status=0xFFFF, flags=0x0000, type=0x00)
            const isDisabled =
              currentStatus === 0xffff && currentFlags === 0x0000 && currentType === 0x00;

            if (isDisabled) {
              // Set status to 0x0030 (enabled)
              modifiedData[i - 6] = 0x30;
              modifiedData[i - 5] = 0x00;
              changedAddresses.push(i - 6, i - 5);

              // Set flags to 0x0030
              modifiedData[i - 4] = 0x30;
              modifiedData[i - 3] = 0x00;
              changedAddresses.push(i - 4, i - 3);

              // Set type to 0x11 (enabled)
              modifiedData[i - 2] = 0x11;
              changedAddresses.push(i - 2);
            }
          }
        }
      }
    }
  }

  const uniqueAddresses = [...new Set(changedAddresses)].sort((a, b) => a - b);
  return { modifiedData, changedAddresses: uniqueAddresses };
}

/**
 * Get unique systems from detected DTCs
 */
export function getEDC15PDTCSystems(dtcs: DetectedDTC[]): string[] {
  const systems = new Set<string>();
  dtcs.forEach((dtc) => {
    if (dtc.system) {
      systems.add(dtc.system);
    }
  });
  return Array.from(systems).sort();
}
