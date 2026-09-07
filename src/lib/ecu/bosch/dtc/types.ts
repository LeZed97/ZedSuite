/**
 * Common types for DTC detection and modification
 */

/**
 * A detected DTC in an ECU file
 */
export interface DetectedDTC {
  /** P-code format (e.g., P0101) */
  code: string;
  /** VAG internal code (e.g., 16485) */
  vagCode: number;
  /** Address in the file where the DTC was found */
  address: number;
  /** Which codeblock contains this DTC */
  codeblockId: number;
  /** Whether the DTC is currently active */
  enabled: boolean;
  /** Description from DTC database */
  description?: string;
  /** System category (EGR, DPF, etc.) */
  system?: string;
  /** Whether this DTC can be disabled (has a control mapping) */
  canDisable?: boolean;
}

/**
 * Information about a codeblock in the ECU file
 */
export interface CodeblockInfo {
  id: number;
  startAddress: number;
  endAddress: number;
  metadataAddress: number;
  isValid: boolean;
}

/**
 * Result of DTC detection
 */
export interface DTCDetectionResult {
  success: boolean;
  ecuType: string;
  codeblocks: CodeblockInfo[];
  dtcs: DetectedDTC[];
  errors: string[];
  /**
   * Compact EDC15 layout: codes are grouped by fault path and switched
   * together (see edc15-fault-paths.ts). The DTC window says so.
   */
  grouped?: boolean;
}

/**
 * Result of DTC modification (enable/disable)
 */
export interface DTCModificationResult {
  modifiedData: Uint8Array;
  changedAddresses: number[];
}
