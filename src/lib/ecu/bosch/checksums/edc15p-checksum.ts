/**
 * EDC15P Checksum Calculator
 *
 * Based on Bosch VAG TDI v4.1 Checksum Algorithm
 * Ported from EDCSuite (C#) to TypeScript
 *
 * This module provides checksum calculation and correction for EDC15P ECU files.
 * It supports multiple checksum variants:
 * - tdi41: Standard EDC15P checksum
 * - tdi41v2: Alternative checksum layout
 * - tdi41_2002: 2002 version checksum
 */

export enum ChecksumResult {
  ChecksumOK = 'OK',
  ChecksumFail = 'FAIL',
  ChecksumTypeError = 'TYPE_ERROR',
}

export interface ChecksumInfo {
  result: ChecksumResult;
  found: number;
  fixed: number;
  matched: number;
  variant: 'tdi41' | 'tdi41v2' | 'tdi41_2002' | 'edc16' | 'unknown';
}

/**
 * Check if a memory region is empty (filled with 0xC3)
 */
function checkEmpty(fileBuffer: Uint8Array, startAddr: number, endAddr: number): boolean {
  for (let i = startAddr; i < endAddr - 4; i++) {
    if (fileBuffer[i] !== 0xC3) return false;
  }
  return true;
}

/**
 * Calculate TDI41 checksum for a given memory region
 */
function tdi41ChecksumCalculate(
  fileBuffer: Uint8Array,
  chkStartAddr: number,
  chkEndAddr: number,
  seedA: number,
  seedB: number
): number {
  let var1: number;
  let var2: number;

  // Ensure we work with 16-bit values
  seedA = seedA & 0xFFFF;
  seedB = seedB & 0xFFFF;

  do {
    var2 = 0;
    seedA ^= ((fileBuffer[chkStartAddr + 1] << 8) + fileBuffer[chkStartAddr]) & 0xFFFF;
    chkStartAddr += 2;

    if ((seedB & 0xF) > 0) {
      var1 = (seedA >>> (16 - (seedB & 0xF))) & 0xFFFF;
      seedA = ((seedA << (seedB & 0xF)) & 0xFFFF) | var1;
      var2 = seedA & 1;
    }

    seedB = (seedB - ((fileBuffer[chkStartAddr + 1] << 8) + fileBuffer[chkStartAddr]) - var2) & 0xFFFF;
    chkStartAddr += 2;
    seedB = (seedB ^ seedA) & 0xFFFF;

    if (chkStartAddr === chkEndAddr) break;

    seedA = (seedA - ((fileBuffer[chkStartAddr + 1] << 8) + fileBuffer[chkStartAddr])) & 0xFFFF;
    chkStartAddr += 2;
    seedA = (seedA + 0xDAAC) & 0xFFFF;
    seedB = (seedB ^ ((fileBuffer[chkStartAddr + 1] << 8) + fileBuffer[chkStartAddr])) & 0xFFFF;
    chkStartAddr += 2;

    if ((seedA & 0xF) > 0) {
      var1 = ((seedB << (16 - (seedA & 0xF))) & 0xFFFF);
      seedB = ((seedB >>> (seedA & 0xF)) | var1) & 0xFFFF;
    }
  } while (chkStartAddr !== chkEndAddr);

  seedA = (seedA - 0x8631 + 0xDAAC) & 0xFFFF;
  seedB = (seedB ^ 0xDF9B) & 0xFFFF;

  return ((seedB << 16) + seedA) >>> 0;
}

/**
 * Calculate TDI41 2002 checksum for a given memory region
 */
function tdi412002ChecksumCalculate(
  fileBuffer: Uint8Array,
  chkStartAddr: number,
  chkEndAddr: number,
  seedA: number,
  seedB: number,
  seedC: number,
  seedD: number,
  firstPass: boolean
): number {
  let count = Math.floor(chkStartAddr / 2);
  const endCount = Math.floor(chkEndAddr / 2);
  let bufferAddr = chkStartAddr;
  let checksum: number;
  let var6: number;
  let var7 = 0;
  let var1 = 0;
  let var2 = 0;
  let var3: number;
  let var4: number;
  let var5: number;

  if (count !== endCount) {
    var1 = seedA & 0xFFFF;
    var2 = seedB & 0xFFFF;

    if (chkStartAddr === 0x8000) {
      var1 = (var1 ^ 0xD565) & 0xFFFF;
      var2 = (var2 + 0x308a) & 0xFFFF;
    }

    do {
      var1 = (var1 ^ ((fileBuffer[bufferAddr + 1] << 8) + fileBuffer[bufferAddr])) & 0xFFFF;
      var3 = var2 & 0xF;
      ++count;
      bufferAddr += 2;
      var4 = 0;

      if ((var2 & 0xF) > 0) {
        do {
          var4 = (var1 >>> 15) & 0xFFFF;
          var1 = ((var1 * 2) + var4) & 0xFFFF;
          --var3;
        } while (var3 > 0);
      }

      var2 = (var2 - (var4 + (fileBuffer[bufferAddr + 1] << 8) + fileBuffer[bufferAddr])) & 0xFFFF;
      var2 = (var1 ^ var2) & 0xFFFF;

      bufferAddr += 2;
      ++count;

      if (count > endCount) break;

      var5 = ((fileBuffer[bufferAddr + 1] << 8) + fileBuffer[bufferAddr]) & 0xFFFF;
      bufferAddr += 4;
      var1 = (var1 + (0xFFFF - var5 + 0xDAAD)) & 0xFFFF;
      var6 = (fileBuffer[bufferAddr - 1] << 8) & 0xFFFF;
      var2 = (var2 ^ (var6 + fileBuffer[bufferAddr - 2])) & 0xFFFF;
      var4 = var1 & 0xF;
      count += 2;

      if ((var1 & 0xF) > 0) {
        do {
          var6 = (var6 | 0xFFFF) & var2;
          var6 = (var6 << 15) & 0xFFFFFFFF;
          var2 = ((var2 >>> 1) + var6) & 0xFFFF;
          --var4;
        } while (var4 > 0);
      }
    } while (count <= endCount);
  }

  if (chkStartAddr === 0) {
    var1 = (var1 - 0x79CF) & 0xFFFF;
    var2 = (var2 - 0x1033) & 0xFFFF;
  }

  if (!firstPass) {
    var5 = seedD & 0xFFFF;
    var1 = (var1 - seedC) & 0xFFFF;
    var6 = ((seedC | 0xFFFF) & 0xDAAD) & 0xFFFF;
    var1 = (var1 + var6 - 1) & 0xFFFF;
    var7 = var7 & 0xFFFF;

    for (count = seedC & 0xF; count > 0; var5 = (((var5 >>> 15) + var7) & 0xFFFF)) {
      --count;
      var7 = (var7 | 0xFFFF) & var5;
      var7 = (var7 * 2) & 0xFFFF;
    }

    checksum = (var1 + ((var5 ^ var2) << 16)) >>> 0;
  } else {
    checksum = (var1 + (var2 << 16)) >>> 0;
  }

  return checksum;
}

/**
 * TDI41 checksum search and fix
 * Standard EDC15P checksum algorithm
 */
function tdi41ChecksumSearch(fileBuffer: Uint8Array, fileSize: number): ChecksumInfo {
  let firstPass = true;
  let chkOldValue: number;
  let chkValue: number;
  let chkStartAddr: number;
  let chkEndAddr: number;

  const chkArray = [0x10000, 0x14000, 0x4C000, 0x50000, 0x50B80, 0x5C000, 0x60000, 0x60B80, 0x6C000, 0x70000, 0x70B80, 0x7C000];
  let seedA = 0;
  let seedB = 0;

  let chkFound = 0;
  let chkFixed = 0;
  let chkMatch = 0;

  for (; chkFound < chkArray.length - 1; chkFound++) {
    chkStartAddr = chkArray[chkFound];
    chkEndAddr = chkArray[chkFound + 1];

    if (!firstPass) {
      seedA |= 0x8631;
      seedB |= 0xEFCD;
    }

    chkOldValue = ((fileBuffer[chkEndAddr - 1] << 24) +
                   (fileBuffer[chkEndAddr - 2] << 16) +
                   (fileBuffer[chkEndAddr - 3] << 8) +
                   fileBuffer[chkEndAddr - 4]) >>> 0;

    chkValue = tdi41ChecksumCalculate(fileBuffer, chkStartAddr, chkEndAddr - 4, seedA, seedB);

    if (chkOldValue !== chkValue && chkOldValue !== 0xC3C3C3C3) {
      fileBuffer[chkEndAddr - 4] = chkValue & 0x000000FF;
      fileBuffer[chkEndAddr - 3] = (chkValue >>> 8) & 0x000000FF;
      fileBuffer[chkEndAddr - 2] = (chkValue >>> 16) & 0x000000FF;
      fileBuffer[chkEndAddr - 1] = (chkValue >>> 24) & 0x000000FF;
      chkFixed++;
    }
    if (chkOldValue === chkValue) {
      chkMatch++;
    }
    firstPass = false;
  }


  let result: ChecksumResult;
  if (chkFixed === 0) result = ChecksumResult.ChecksumOK;
  else if (chkMatch > 3) result = ChecksumResult.ChecksumFail;
  else if (chkFixed >= 6) result = ChecksumResult.ChecksumTypeError;
  else result = ChecksumResult.ChecksumFail;

  return { result, found: chkFound, fixed: chkFixed, matched: chkMatch, variant: 'tdi41' };
}

/**
 * TDI41v2 checksum search and fix
 * Alternative EDC15P checksum layout
 */
function tdi41v2ChecksumSearch(fileBuffer: Uint8Array, fileSize: number): ChecksumInfo {
  let firstPass = true;
  let chkOldValue: number;
  let chkValue: number;
  let chkStartAddr: number;
  let chkEndAddr: number;

  const chkArray = [0x10000, 0x14000, 0x58000, 0x58B80, 0x64000, 0x70000, 0x70B80, 0x7C000];
  let seedA = 0;
  let seedB = 0;

  let chkFound = 0;
  let chkFixed = 0;
  let chkMatch = 0;

  for (; chkFound < chkArray.length - 1; chkFound++) {
    chkStartAddr = chkArray[chkFound];
    chkEndAddr = chkArray[chkFound + 1];

    if (!firstPass) {
      seedA |= 0x8631;
      seedB |= 0xEFCD;
    }

    if (checkEmpty(fileBuffer, chkStartAddr, chkEndAddr)) continue;

    chkOldValue = ((fileBuffer[chkEndAddr - 1] << 24) +
                   (fileBuffer[chkEndAddr - 2] << 16) +
                   (fileBuffer[chkEndAddr - 3] << 8) +
                   fileBuffer[chkEndAddr - 4]) >>> 0;

    chkValue = tdi41ChecksumCalculate(fileBuffer, chkStartAddr, chkEndAddr - 4, seedA, seedB);

    if (chkOldValue !== chkValue && chkOldValue !== 0xC3C3C3C3) {
      fileBuffer[chkEndAddr - 4] = chkValue & 0x000000FF;
      fileBuffer[chkEndAddr - 3] = (chkValue >>> 8) & 0x000000FF;
      fileBuffer[chkEndAddr - 2] = (chkValue >>> 16) & 0x000000FF;
      fileBuffer[chkEndAddr - 1] = (chkValue >>> 24) & 0x000000FF;
      chkFixed++;
    } else if (chkValue === chkOldValue) {
      chkMatch++;
    }
    firstPass = false;
  }


  let result: ChecksumResult;
  if (chkFixed === 0) result = ChecksumResult.ChecksumOK;
  else if (chkMatch > 3) result = ChecksumResult.ChecksumFail;
  else if (chkFixed >= 4) result = ChecksumResult.ChecksumTypeError;
  else result = ChecksumResult.ChecksumFail;

  return { result, found: chkFound, fixed: chkFixed, matched: chkMatch, variant: 'tdi41v2' };
}

/**
 * TDI41 2002 checksum search and fix
 * 2002 version checksum algorithm
 */
function tdi412002ChecksumSearch(fileBuffer: Uint8Array, fileSize: number): ChecksumInfo {
  let seed1: number;
  let seed2: number;
  let seed1Msb: number;
  let seed1Lsb: number;
  let seed2Lsb: number;
  let seed2Msb: number;

  let chkOldValue: number;
  let chkValue: number;
  let chkStartAddr: number;
  let chkEndAddr: number;
  let chkStoreAddr: number;

  let chkFound = 2;
  let chkFixed = 0;
  let chkMatch = 0;

  // Find seed 1
  seed1 = tdi412002ChecksumCalculate(fileBuffer, 0x14000, 0x4BFFE, 0x8631, 0xEFCD, 0, 0, true);
  seed1Msb = (seed1 >>> 16) & 0xFFFF;
  seed1Lsb = seed1 & 0xFFFF;

  // Find seed 2
  seed2 = tdi412002ChecksumCalculate(fileBuffer, 0, 0x7FFE, 0, 0, 0, 0, true);
  seed2Msb = (seed2 >>> 16) & 0xFFFF;
  seed2Lsb = seed2 & 0xFFFF;

  // Checksum 1
  chkOldValue = ((fileBuffer[0xFFFF] << 24) +
                 (fileBuffer[0xFFFE] << 16) +
                 (fileBuffer[0xFFFD] << 8) +
                 fileBuffer[0xFFFC]) >>> 0;

  chkValue = tdi412002ChecksumCalculate(fileBuffer, 0x8000, 0xFFFB, seed2Lsb, seed2Msb, 0x4531, 0x3550, false);

  if (chkOldValue !== chkValue) {
    fileBuffer[0xFFFC] = chkValue & 0x000000FF;
    fileBuffer[0xFFFD] = (chkValue >>> 8) & 0x000000FF;
    fileBuffer[0xFFFE] = (chkValue >>> 16) & 0x000000FF;
    fileBuffer[0xFFFF] = (chkValue >>> 24) & 0x000000FF;
    chkFixed++;
  } else {
    chkMatch++;
  }

  // Checksum 2
  chkOldValue = ((fileBuffer[0x13FFF] << 24) +
                 (fileBuffer[0x13FFE] << 16) +
                 (fileBuffer[0x13FFD] << 8) +
                 fileBuffer[0x13FFC]) >>> 0;

  chkValue = tdi412002ChecksumCalculate(fileBuffer, 0x10000, 0x13FFB, 0, 0, 0x8631, 0xEFCD, false);

  if (chkOldValue !== chkValue) {
    fileBuffer[0x13FFC] = chkValue & 0x000000FF;
    fileBuffer[0x13FFD] = (chkValue >>> 8) & 0x000000FF;
    fileBuffer[0x13FFE] = (chkValue >>> 16) & 0x000000FF;
    fileBuffer[0x13FFF] = (chkValue >>> 24) & 0x000000FF;
    chkFixed++;
  } else {
    chkMatch++;
  }

  // Checksum blocks loop
  chkStoreAddr = 0x4FFFB;
  do {
    if (fileBuffer[chkStoreAddr + 13] === 0x56 &&
        fileBuffer[chkStoreAddr + 14] === 0x34 &&
        fileBuffer[chkStoreAddr + 15] === 0x2E &&
        fileBuffer[chkStoreAddr + 16] === 0x31) {

      // Checksum block 1
      chkStartAddr = chkStoreAddr - 0x3FFB;
      chkEndAddr = chkStoreAddr;

      chkOldValue = ((fileBuffer[chkStoreAddr + 4] << 24) +
                     (fileBuffer[chkStoreAddr + 3] << 16) +
                     (fileBuffer[chkStoreAddr + 2] << 8) +
                     fileBuffer[chkStoreAddr + 1]) >>> 0;

      chkValue = tdi412002ChecksumCalculate(fileBuffer, chkStartAddr, chkEndAddr, seed1Lsb, seed1Msb, seed1Lsb, seed1Msb, false);

      if (chkOldValue !== chkValue) {
        fileBuffer[chkStoreAddr + 1] = chkValue & 0x000000FF;
        fileBuffer[chkStoreAddr + 2] = (chkValue >>> 8) & 0x000000FF;
        fileBuffer[chkStoreAddr + 3] = (chkValue >>> 16) & 0x000000FF;
        fileBuffer[chkStoreAddr + 4] = (chkValue >>> 24) & 0x000000FF;
        chkFixed++;
      } else {
        chkMatch++;
      }

      // Checksum block 2
      chkStartAddr = chkStoreAddr + 5;
      chkEndAddr = chkStoreAddr + 0xB80;

      chkOldValue = ((fileBuffer[chkStoreAddr + 2948] << 24) +
                     (fileBuffer[chkStoreAddr + 2947] << 16) +
                     (fileBuffer[chkStoreAddr + 2946] << 8) +
                     fileBuffer[chkStoreAddr + 2945]) >>> 0;

      chkValue = tdi412002ChecksumCalculate(fileBuffer, chkStartAddr, chkEndAddr, seed1Lsb, seed1Msb, seed1Lsb, seed1Msb, false);

      if (chkOldValue !== chkValue) {
        fileBuffer[chkStoreAddr + 2945] = chkValue & 0x000000FF;
        fileBuffer[chkStoreAddr + 2946] = (chkValue >>> 8) & 0x000000FF;
        fileBuffer[chkStoreAddr + 2947] = (chkValue >>> 16) & 0x000000FF;
        fileBuffer[chkStoreAddr + 2948] = (chkValue >>> 24) & 0x000000FF;
        chkFixed++;
      } else {
        chkMatch++;
      }

      // Checksum block 3
      chkStartAddr = chkStoreAddr + 0xB85;
      chkEndAddr = chkStoreAddr + 0xC000;

      chkOldValue = ((fileBuffer[chkStoreAddr + 49156] << 24) +
                     (fileBuffer[chkStoreAddr + 49155] << 16) +
                     (fileBuffer[chkStoreAddr + 49154] << 8) +
                     fileBuffer[chkStoreAddr + 49153]) >>> 0;

      chkValue = tdi412002ChecksumCalculate(fileBuffer, chkStartAddr, chkEndAddr, seed1Lsb, seed1Msb, seed1Lsb, seed1Msb, false);

      if (chkOldValue !== chkValue) {
        fileBuffer[chkStoreAddr + 49153] = chkValue & 0x000000FF;
        fileBuffer[chkStoreAddr + 49154] = (chkValue >>> 8) & 0x000000FF;
        fileBuffer[chkStoreAddr + 49155] = (chkValue >>> 16) & 0x000000FF;
        fileBuffer[chkStoreAddr + 49156] = (chkValue >>> 24) & 0x000000FF;
        chkFixed++;
      } else {
        chkMatch++;
      }

      chkFound += 3;
    }

    chkStoreAddr += 0x10000;
  } while (chkStoreAddr + 5 < fileSize);


  let result: ChecksumResult;
  if (chkFixed === 0) result = ChecksumResult.ChecksumOK;
  else if (chkMatch > 3) result = ChecksumResult.ChecksumFail;
  else if (chkFixed >= chkFound - 1) result = ChecksumResult.ChecksumTypeError;
  else result = ChecksumResult.ChecksumFail;

  return { result, found: chkFound, fixed: chkFixed, matched: chkMatch, variant: 'tdi41_2002' };
}

/**
 * Detect which checksum variant to use based on file structure
 */
function detectChecksumVariant(fileBuffer: Uint8Array, fileSize: number): 'tdi41' | 'tdi41v2' | 'tdi41_2002' | 'unknown' {
  // Check for 2002 variant marker (V4.1 string)
  let chkStoreAddr = 0x4FFFB;
  while (chkStoreAddr + 16 < fileSize) {
    if (fileBuffer[chkStoreAddr + 13] === 0x56 && // 'V'
        fileBuffer[chkStoreAddr + 14] === 0x34 && // '4'
        fileBuffer[chkStoreAddr + 15] === 0x2E && // '.'
        fileBuffer[chkStoreAddr + 16] === 0x31) { // '1'
      return 'tdi41_2002';
    }
    chkStoreAddr += 0x10000;
  }

  // Try tdi41 first - check if regions are not empty
  const tdi41Regions = [0x4C000, 0x5C000, 0x6C000];
  let tdi41EmptyCount = 0;
  for (const addr of tdi41Regions) {
    if (addr + 0x10000 <= fileSize && checkEmpty(fileBuffer, addr, addr + 0x10000)) {
      tdi41EmptyCount++;
    }
  }

  // Check tdi41v2 regions
  const tdi41v2Regions = [0x58000, 0x64000, 0x70000];
  let tdi41v2EmptyCount = 0;
  for (const addr of tdi41v2Regions) {
    if (addr + 0x10000 <= fileSize && checkEmpty(fileBuffer, addr, addr + 0x10000)) {
      tdi41v2EmptyCount++;
    }
  }

  // If tdi41v2 has fewer empty regions, it's probably the right variant
  if (tdi41v2EmptyCount < tdi41EmptyCount) {
    return 'tdi41v2';
  }

  return 'tdi41';
}

/**
 * Main function to correct EDC15P checksums
 * This modifies the fileBuffer in place and returns information about what was fixed
 *
 * @param fileData - The file data as a number array
 * @returns ChecksumInfo with result, counts, and the corrected data
 */
/**
 * Taille minimale exploitable : les tables de points de contrôle v4.1 vont
 * jusqu'à 0x7C000. Sur un fichier plus court, les lectures sortiraient du
 * buffer et l'écriture retomberait sur des zéros — donc un binaire corrompu.
 * On refuse explicitement plutôt que de produire un fichier invalide.
 */
const MIN_V41_FILE_SIZE = 0x80000; // 512 Ko

export function correctEDC15PChecksum(fileData: number[]): { info: ChecksumInfo; correctedData: number[] } {
  // Convert to Uint8Array for easier manipulation
  const fileBuffer = new Uint8Array(fileData);
  const fileSize = fileBuffer.length;

  if (fileSize < MIN_V41_FILE_SIZE) {
    return {
      info: {
        result: ChecksumResult.ChecksumTypeError,
        found: 0,
        fixed: 0,
        matched: 0,
        variant: 'unknown',
      },
      correctedData: [...fileData],
    };
  }

  // Detect which variant to use
  const variant = detectChecksumVariant(fileBuffer, fileSize);

  let info: ChecksumInfo;

  // Try the detected variant first
  switch (variant) {
    case 'tdi41_2002':
      info = tdi412002ChecksumSearch(fileBuffer, fileSize);
      break;
    case 'tdi41v2':
      info = tdi41v2ChecksumSearch(fileBuffer, fileSize);
      // If v2 fails, try standard tdi41
      if (info.result === ChecksumResult.ChecksumTypeError) {
        const fileBuffer2 = new Uint8Array(fileData); // Reset buffer
        info = tdi41ChecksumSearch(fileBuffer2, fileSize);
        if (info.result !== ChecksumResult.ChecksumTypeError) {
          // Copy corrected data back
          for (let i = 0; i < fileSize; i++) {
            fileBuffer[i] = fileBuffer2[i];
          }
        }
      }
      break;
    case 'tdi41':
    default:
      info = tdi41ChecksumSearch(fileBuffer, fileSize);
      // If standard fails, try v2
      if (info.result === ChecksumResult.ChecksumTypeError) {
        const fileBuffer2 = new Uint8Array(fileData); // Reset buffer
        info = tdi41v2ChecksumSearch(fileBuffer2, fileSize);
        if (info.result !== ChecksumResult.ChecksumTypeError) {
          // Copy corrected data back
          for (let i = 0; i < fileSize; i++) {
            fileBuffer[i] = fileBuffer2[i];
          }
        }
      }
      break;
  }

  // Convert back to number array
  const correctedData = Array.from(fileBuffer);

  return { info, correctedData };
}

/**
 * Verify if EDC15P checksums are correct without modifying the data
 *
 * @param fileData - The file data as a number array
 * @returns ChecksumInfo with verification results
 */
export function verifyEDC15PChecksum(fileData: number[]): ChecksumInfo {
  // Create a copy to avoid modifying original
  const fileDataCopy = [...fileData];
  const { info } = correctEDC15PChecksum(fileDataCopy);

  // If no fixes were needed, checksums are valid
  return {
    ...info,
    result: info.fixed === 0 ? ChecksumResult.ChecksumOK : ChecksumResult.ChecksumFail,
  };
}
