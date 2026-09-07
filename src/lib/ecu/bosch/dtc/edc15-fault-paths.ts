/**
 * EDC15 "fault path" DTC tables (compact software layout)
 *
 * Software with the V4.1 signature one byte after a 0xC000 block at 0x58000
 * / 0x64000 / 0x70000 (EDC15VM 038906012K, 012L, 012AA, 012AP, 012CP and
 * the early EDC15P PD 038906019AJ / 019AN) does not store one 8-byte entry
 * per fault code. Each record describes one fault path (a sensor, an
 * actuator...) and looks like this, in the second third of the block:
 *
 *   C3 | 8 code slots (16 bytes, LE VAG codes, 0 = unused error class)
 *      | 5 pointer words (10 bytes, RAM addresses)
 *      | K triplets of 6 bytes: [set time LE][heal time LE][flags][00]
 *      | 3 bytes (00 00 00, sometimes 05 00 00 / 05 02 14) before the next C3
 *
 * The triplets carry exactly the same values as the per-code entries of the
 * later layout ([set][heal][flags][23][code]) for the same codes, but their
 * order inside a record does not follow the slot order, so a single code
 * cannot be tied to one triplet with certainty. Switching is therefore done
 * per fault path: disabling a code disables every triplet of its record
 * (FF FF 00 00 00, the same "off" pattern as the later layout), which is what
 * a DTC off does physically anyway (the ECU stops monitoring that path).
 * A record is reported enabled while at least one of its triplets is not in
 * the "off" state.
 *
 * The bench (07/09/2026): 012K 159 records / 420 codes, 012L 106 / 280,
 * 012AP 53 / 140, 012CP 159 / 429, 019AJ 138 / 345, 019AN 92 / 230, every
 * code known to the database.
 */

import { VAG_DTC_DATABASE } from './dtc-database';
import type { CodeblockInfo, DetectedDTC, DTCDetectionResult } from './types';

const V41_SIGNATURE = [0x67, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x56, 0x34, 0x2e, 0x31];
const RECORD_MARKER = 0xc3;
const SLOT_COUNT = 8;
const POINTER_COUNT = 5;
const TRIPLET_SIZE = 6;
const HEADER_SIZE = 1 + SLOT_COUNT * 2 + POINTER_COUNT * 2; // marker + slots + pointers = 27
const RECORD_TAIL = 3;
const MAX_TRIPLETS = 16;
const VAG_CODE_MIN = 16384;
const VAG_CODE_MAX = 20000;

/** Compact 0xC000 blocks: V4.1 signature one byte after the block start. */
const COMPACT_BLOCKS: Array<{ id: number; start: number }> = [
  { id: 2, start: 0x58000 },
  { id: 3, start: 0x64000 },
  { id: 5, start: 0x70000 },
];
const COMPACT_BLOCK_SIZE = 0xc000;

function hasV41SignatureAt(data: Uint8Array, pos: number): boolean {
  if (pos + V41_SIGNATURE.length > data.length) return false;
  for (let i = 0; i < V41_SIGNATURE.length; i++) {
    if (data[pos + i] !== V41_SIGNATURE[i]) return false;
  }
  return true;
}

/**
 * True for the compact layout: a V4.1 signature at 0x58001 or 0x64001
 * (0x70001 alone is shared with the classic layout, so it does not count).
 */
export function hasCompactV41Layout(data: Uint8Array): boolean {
  return [0x58001, 0x64001].some((pos) => hasV41SignatureAt(data, pos));
}

/** The compact blocks carrying a V4.1 signature (0x70000 too when signed). */
export function detectCompactCodeblocks(data: Uint8Array): CodeblockInfo[] {
  return COMPACT_BLOCKS.map(({ id, start }) => ({
    id,
    startAddress: start,
    endAddress: start + COMPACT_BLOCK_SIZE - 1,
    metadataAddress: start + 1,
    isValid: hasV41SignatureAt(data, start + 1),
  }));
}

interface Triplet {
  address: number;
  status: number;
  flags: number;
  type: number;
}

export interface FaultPathRecord {
  /** Address of the C3 marker */
  address: number;
  codeblockId: number;
  /** Non-empty code slots: address of the word and its VAG code */
  slots: Array<{ address: number; vagCode: number }>;
  triplets: Triplet[];
}

function readWord(data: Uint8Array, pos: number): number {
  return data[pos] | (data[pos + 1] << 8);
}

function isKnownCode(vagCode: number): boolean {
  return vagCode >= VAG_CODE_MIN && vagCode <= VAG_CODE_MAX && !!VAG_DTC_DATABASE[vagCode];
}

/** A marker: C3 followed by 8 slots that are all either 0 or a known code. */
function isRecordMarker(data: Uint8Array, pos: number): boolean {
  if (data[pos] !== RECORD_MARKER || pos + HEADER_SIZE > data.length) return false;
  for (let i = 0; i < SLOT_COUNT; i++) {
    const code = readWord(data, pos + 1 + 2 * i);
    if (code !== 0 && !isKnownCode(code)) return false;
  }
  return true;
}

function tripletOff(t: Triplet): boolean {
  return t.status === 0xffff && t.flags === 0 && t.type === 0;
}

/**
 * Parse the fault path records of one block. Empty records (all slots zero)
 * are kept as delimiters: the triplet run of a record ends at the next
 * marker, three bytes before it.
 */
export function parseFaultPathRecords(data: Uint8Array, block: CodeblockInfo): FaultPathRecord[] {
  const end = Math.min(block.endAddress + 1, data.length);
  const markers: number[] = [];
  for (let p = block.startAddress; p + HEADER_SIZE <= end; p++) {
    if (isRecordMarker(data, p)) markers.push(p);
  }

  const records: FaultPathRecord[] = [];
  for (let m = 0; m < markers.length; m++) {
    const pos = markers[m];
    const slots: FaultPathRecord['slots'] = [];
    for (let i = 0; i < SLOT_COUNT; i++) {
      const address = pos + 1 + 2 * i;
      const vagCode = readWord(data, address);
      if (vagCode !== 0) slots.push({ address, vagCode });
    }

    const tripletsStart = pos + HEADER_SIZE;
    let count: number;
    if (m + 1 < markers.length) {
      const span = markers[m + 1] - RECORD_TAIL - tripletsStart;
      count = span >= 0 && span % TRIPLET_SIZE === 0 ? span / TRIPLET_SIZE : Math.floor(Math.max(0, span) / TRIPLET_SIZE);
    } else {
      // Last record of the block: read while the groups still look like
      // triplets (pad byte 0, not the 0x23 filler of the end of the table)
      count = 0;
      while (count < MAX_TRIPLETS) {
        const q = tripletsStart + count * TRIPLET_SIZE;
        if (q + TRIPLET_SIZE > end || data[q + 5] !== 0 || data[q] === 0x23) break;
        count++;
      }
    }
    count = Math.min(count, MAX_TRIPLETS);

    const triplets: Triplet[] = [];
    for (let k = 0; k < count; k++) {
      const q = tripletsStart + k * TRIPLET_SIZE;
      triplets.push({
        address: q,
        status: readWord(data, q),
        flags: readWord(data, q + 2),
        type: data[q + 4],
      });
    }

    if (slots.length > 0) {
      records.push({ address: pos, codeblockId: block.id, slots, triplets });
    }
  }
  return records;
}

function recordEnabled(record: FaultPathRecord): boolean {
  // Stock records keep some "off" triplets for unused error classes; the
  // path is off only once every triplet carries the off pattern.
  return record.triplets.length === 0 || record.triplets.some((t) => !tripletOff(t));
}

/**
 * Detect the DTCs of a compact-layout file. `family` names the ECU in the
 * result (EDC15P or EDC15VM).
 */
export function detectFaultPathDTCs(data: Uint8Array, family: string): DTCDetectionResult {
  const codeblocks = detectCompactCodeblocks(data);
  const valid = codeblocks.filter((cb) => cb.isValid);
  const errors: string[] = [];
  if (valid.length === 0) {
    errors.push('No valid codeblocks detected in the file');
    return { success: false, ecuType: family, codeblocks, dtcs: [], errors };
  }

  const dtcs: DetectedDTC[] = [];
  const seen = new Set<string>();
  for (const block of valid) {
    for (const record of parseFaultPathRecords(data, block)) {
      const enabled = recordEnabled(record);
      for (const slot of record.slots) {
        const info = VAG_DTC_DATABASE[slot.vagCode];
        if (!info || seen.has(info.code)) continue;
        seen.add(info.code);
        dtcs.push({
          code: info.code,
          vagCode: slot.vagCode,
          address: slot.address,
          codeblockId: block.id,
          enabled,
          description: info.description,
          system: info.system,
        });
      }
    }
  }
  dtcs.sort((a, b) => a.code.localeCompare(b.code));

  return { success: true, ecuType: family, codeblocks, dtcs, errors, grouped: true };
}

function writeRecords(
  data: Uint8Array,
  vagCode: number,
  apply: (modified: Uint8Array, t: Triplet, changed: number[]) => void
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  const modifiedData = new Uint8Array(data);
  const changed: number[] = [];
  for (const block of detectCompactCodeblocks(data).filter((cb) => cb.isValid)) {
    for (const record of parseFaultPathRecords(data, block)) {
      if (!record.slots.some((s) => s.vagCode === vagCode)) continue;
      for (const t of record.triplets) apply(modifiedData, t, changed);
    }
  }
  return { modifiedData, changedAddresses: [...new Set(changed)].sort((a, b) => a - b) };
}

/** Switch off every fault path carrying the code: all its triplets to FF FF 00 00 00. */
export function disableFaultPathDTC(
  data: Uint8Array,
  dtc: DetectedDTC
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  return writeRecords(data, dtc.vagCode, (m, t, changed) => {
    if (tripletOff(t)) return;
    m[t.address] = 0xff;
    m[t.address + 1] = 0xff;
    m[t.address + 2] = 0x00;
    m[t.address + 3] = 0x00;
    m[t.address + 4] = 0x00;
    changed.push(t.address, t.address + 1, t.address + 2, t.address + 3, t.address + 4);
  });
}

/**
 * Switch a fault path back on: the same 30 00 30 00 11 the later layout
 * writes (the original per-triplet values are not kept).
 */
export function enableFaultPathDTC(
  data: Uint8Array,
  dtc: DetectedDTC
): { modifiedData: Uint8Array; changedAddresses: number[] } {
  return writeRecords(data, dtc.vagCode, (m, t, changed) => {
    if (!tripletOff(t)) return;
    m[t.address] = 0x30;
    m[t.address + 1] = 0x00;
    m[t.address + 2] = 0x30;
    m[t.address + 3] = 0x00;
    m[t.address + 4] = 0x11;
    changed.push(t.address, t.address + 1, t.address + 2, t.address + 3, t.address + 4);
  });
}
