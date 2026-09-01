// Detection engine access — Tauri IPC wrappers.
// The Rust detection code lives in src-tauri/src/detector/ and is exposed
// through the commands in src-tauri/src/commands.rs. Response shapes are
// identical to the old map-detector HTTP microservice.

import { invoke } from "@tauri-apps/api/core";

export interface EcuIdentification {
  manufacturer: string;
  ecu_type: string;
  variant?: string;
  software_version?: string;
  hardware_version?: string;
  part_number?: string;
  confidence: number;
}

export interface DetectionResults {
  success: boolean;
  maps: any[];
  total_maps: number;
  processing_time_ms: number;
  file_size: number;
  /** Version du moteur ayant produit ce résultat (voir detectorVersion). */
  detector_version?: number;
  /** Rapport de complétude EDC16 : familles attendues vs trouvées. */
  expected_maps?: { label: string; expected: number; found: number }[];
}

/** ECU families the local app supports (must match src-tauri/ecus.json). */
export const SUPPORTED_ECUS = new Set([
  "EDC15P",
  "EDC15V",
  "EDC15VM",
  "EDC16U1",
  "EDC16U31",
  "EDC16U34",
]);

export async function identifyEcu(
  fileDataBase64: string,
  fileName: string
): Promise<EcuIdentification> {
  return invoke<EcuIdentification>("identify_ecu", {
    fileDataBase64,
    fileName,
  });
}

export async function detectMaps(args: {
  fileDataBase64: string;
  fileName: string;
  ecuType?: string;
  tunedMode?: boolean;
}): Promise<DetectionResults> {
  return invoke<DetectionResults>("detect_maps", {
    request: {
      file_data_base64: args.fileDataBase64,
      file_name: args.fileName,
      ecu_type: args.ecuType,
      tuned_mode: args.tunedMode ?? false,
    },
  });
}

export async function listEcus(): Promise<{ ecus: any[]; total: number; version: string }> {
  return invoke("list_ecus");
}

/**
 * Version courante du moteur de détection. Un projet dont les résultats
 * portent une version antérieure est re-scanné à l'ouverture : sans ça il
 * resterait indéfiniment sur des adresses, facteurs ou libellés périmés.
 */
export async function detectorVersion(): Promise<number> {
  try {
    return await invoke<number>("detector_version");
  } catch {
    // Version antérieure à l'ajout de la commande : on ne force rien.
    return 0;
  }
}

/** Encode a byte array to base64 (chunked — fast for 2MB dumps). */
export function bytesToBase64(bytes: Uint8Array): string {
  const chunks: string[] = [];
  const chunkSize = 8192;
  for (let i = 0; i < bytes.length; i += chunkSize) {
    chunks.push(String.fromCharCode(...bytes.subarray(i, i + chunkSize)));
  }
  return btoa(chunks.join(""));
}
