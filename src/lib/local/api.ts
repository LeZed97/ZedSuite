// Local API layer — replaces the Next.js /api/* routes of the web version.
//
// The editor and dashboard were written against an HTTP API (fetch/axios on
// /api/versioning/*, /api/files/*, ...). Instead of rewriting thousands of
// call sites, the bridge (bridge.ts) intercepts those requests and hands
// them to `handleLocalApi` below, which reproduces each route's contract on
// top of the on-disk project store (store.ts) and the Rust detection
// commands (detector.ts).
//
// Everything that was a SaaS restriction in the web version (auth, quotas,
// daily limits, blue coins, mappack unlocking) is answered permissively —
// the local app has no limits.

import * as store from "./store";
import { detectMaps, SUPPORTED_ECUS } from "./detector";
import { type MappackDisplaySettings,
  buildWinolsMappack,
  serializeWinolsMappack,
  mappackFileName,
  type ExportMapData,
} from "@/lib/mappack-export";

export interface LocalApiResult {
  status: number;
  json?: any;
  /** Binary responses (mappack export) */
  body?: Uint8Array;
  headers?: Record<string, string>;
}

function ok(json: any): LocalApiResult {
  return { status: 200, json };
}

function error(status: number, message: string, extra?: any): LocalApiResult {
  return { status, json: { error: message, ...extra } };
}

function base64ToBytes(b64: string): Uint8Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes;
}

export async function handleLocalApi(
  method: string,
  rawPath: string,
  body?: any
): Promise<LocalApiResult> {
  const url = new URL(rawPath, "http://localhost");
  const path = url.pathname.replace(/\/+$/, "");
  const q = url.searchParams;
  const m = method.toUpperCase();

  try {
    // ── Platform / limits stubs (no limits in the local app) ──────
    if (path === "/api/limits/status") {
      return ok({ limits: { max_files: 1000000, daily_upload: 1000000 } });
    }
    if (path === "/api/settings/public") {
      return ok({ maxFileSizeMB: 10, siteName: "ZedSuite" });
    }
    if (path === "/api/versioning/track-action") {
      return ok({ allowed: true, remaining: 1000000 });
    }
    if (path === "/api/ecu-status") {
      const ecuType = (q.get("ecu_type") || "").toUpperCase();
      return ok({ enabled: SUPPORTED_ECUS.has(ecuType) });
    }
    if (path === "/api/user/theme") {
      // Theme persistence: the theme context saves to localStorage before
      // PUTting here, so GET just mirrors localStorage back.
      if (m === "GET") {
        return ok({ theme: localStorage.getItem("userTheme") || undefined });
      }
      return ok({ success: true });
    }
    if (path.startsWith("/api/user/")) {
      // Settings persistence is handled by localStorage client-side
      return ok({});
    }

    // ── Project creation ──────────────────────────────────────────
    if (path === "/api/versioning/init" && m === "POST") {
      const b = body || {};
      if (!b.fileData) return error(400, "missing_file_data");
      const binary =
        typeof b.fileData === "string"
          ? base64ToBytes(b.fileData)
          : new Uint8Array(b.fileData);

      const { file, oriVersion } = await store.createProject({
        fileName: b.fileName || b.projectName || "file.bin",
        originalName: b.fileName,
        projectName: b.projectName,
        binary,
        ecuType: b.ecuType,
        hardwareVersion: b.hardwareVersion,
        softwareVersion: b.softwareVersion,
        detectionResults: b.detectionResults,
        vehicleBrand: b.vehicleBrand,
        vehicleModel: b.vehicleModel,
        engineType: b.engineType,
        transmissionType: b.transmissionType,
        year: b.year,
        power: b.power,
        customer: b.customer,
        stage: b.stage,
        date: b.date,
        notes: b.notes,
      });

      return ok({
        fileId: file.id,
        versions: [oriVersion],
        currentVersionId: oriVersion.id,
        projectName: b.projectName || file.file_name,
      });
    }

    // ── File data (binary + metadata for the editor) ──────────────
    let match = path.match(/^\/api\/versioning\/file-data\/([^/]+)$/);
    if (match && m === "GET") {
      const fileRecord = await store.getFile(match[1]);
      if (!fileRecord) return error(404, "not_found");
      const binary = await store.readBinary(match[1]);
      if (!binary) return error(404, "no_binary_data");

      return ok({
        file_data: Array.from(binary),
        file_name: fileRecord.file_name,
        original_name: fileRecord.original_name,
        file_size: fileRecord.file_size,
        ecu_type: fileRecord.ecu_type,
        hardware_version: fileRecord.hardware_version,
        software_version: fileRecord.software_version,
        project_name: fileRecord.project_name,
        vehicle_brand: fileRecord.vehicle_brand,
        vehicle_model: fileRecord.vehicle_model,
        engine_type: fileRecord.engine_type,
        transmission_type: fileRecord.transmission_type,
        year: fileRecord.year,
        power: fileRecord.power,
        customer: fileRecord.customer,
        stage: fileRecord.stage,
        date: fileRecord.date,
        notes: fileRecord.notes,
        detection_data: fileRecord.detection_data,
        mappack_unlocked: true,
        mappack_exported: false,
        map_display_settings: fileRecord.map_display_settings || null,
        map_sort_mode: fileRecord.map_sort_mode || null,
      });
    }

    // ── Versions ──────────────────────────────────────────────────
    if (path === "/api/versioning/versions" && m === "GET") {
      const fileId = q.get("fileId");
      if (!fileId) return error(400, "bad_request");
      const versions = await store.listVersions(fileId);
      const current = versions.find((v) => v.is_current);
      return ok({ versions, currentVersionId: current?.id || null });
    }
    if (path === "/api/versioning/versions" && m === "POST") {
      const { fileId, name, baseVersionId, setCurrent } = body || {};
      if (!fileId || !name) return error(400, "bad_request");
      const version = await store.createVersion(
        fileId,
        String(name).slice(0, 100),
        baseVersionId,
        setCurrent !== undefined ? setCurrent : true
      );
      if (!version) return error(404, "not_found");
      return ok({ version });
    }
    match = path.match(/^\/api\/versioning\/versions\/([^/]+)$/);
    if (match && m === "PATCH") {
      const { name, isCurrent } = body || {};
      const version = await store.updateVersion(match[1], {
        name: name !== undefined ? String(name).trim().slice(0, 100) : undefined,
        is_current: isCurrent,
      });
      if (!version) return error(404, "not_found");
      return ok({ version });
    }
    if (match && m === "DELETE") {
      const res = await store.deleteVersion(match[1]);
      if (!res.ok) {
        if (res.error === "not_found") return error(404, "not_found");
        return error(403, res.error || "forbidden");
      }
      return ok({ success: true });
    }

    // ── Map edits ─────────────────────────────────────────────────
    if (path === "/api/versioning/map-edits" && m === "GET") {
      const versionId = q.get("versionId");
      if (!versionId) return error(400, "bad_request");
      const edits = await store.listMapEdits(versionId);
      return ok({ edits });
    }
    if (path === "/api/versioning/map-edits" && m === "PUT") {
      const { versionId, edits } = body || {};
      if (!versionId || !Array.isArray(edits)) return error(400, "bad_request");
      const list = await store.replaceMapEdits(versionId, edits);
      if (!list) return error(404, "not_found");
      return ok({ edits: list });
    }
    if (path === "/api/versioning/map-edits" && m === "POST") {
      const { versionId, mapAddress, payload } = body || {};
      if (!versionId) return error(400, "bad_request");
      const edit = await store.addMapEdit(versionId, mapAddress ?? -1, payload);
      if (!edit) return error(404, "not_found");
      return ok({ edit });
    }

    // ── Files ─────────────────────────────────────────────────────
    match = path.match(/^\/api\/files\/([^/]+)$/);
    if (match && m === "GET") {
      const fileRecord = await store.getFile(match[1]);
      if (!fileRecord) return error(404, "not_found");
      const binary = await store.readBinary(match[1]);
      return ok({
        fileId: fileRecord.id,
        fileName: fileRecord.file_name,
        originalName: fileRecord.original_name,
        ecuType: fileRecord.ecu_type,
        detectionResults: fileRecord.detection_data,
        fileData: binary ? Array.from(binary) : [],
      });
    }
    if (match && m === "PATCH") {
      const allowed = [
        "project_name",
        "vehicle_brand",
        "vehicle_model",
        "engine_type",
        "transmission_type",
        "year",
        "power",
        "customer",
        "stage",
        "notes",
        "detection_data",
        "map_sort_mode",
      ];
      const patch: any = {};
      for (const key of allowed) {
        if (body?.[key] !== undefined) patch[key] = body[key];
      }
      const updated = await store.updateFile(match[1], patch);
      if (!updated) return error(404, "not_found");
      return ok({ success: true, file: updated });
    }
    if (match && m === "DELETE") {
      const deleted = await store.deleteFile(match[1]);
      return deleted ? ok({ success: true }) : error(404, "not_found");
    }

    match = path.match(/^\/api\/files\/([^/]+)\/display-settings$/);
    if (match && m === "PUT") {
      const settings = body?.settings;
      if (settings === undefined || typeof settings !== "object") {
        return error(400, "bad_request");
      }
      const updated = await store.updateFile(match[1], {
        map_display_settings: settings,
      });
      if (!updated) return error(404, "not_found");
      return ok({ success: true });
    }

    match = path.match(/^\/api\/files\/([^/]+)\/redetect$/);
    if (match && m === "POST") {
      const fileRecord = await store.getFile(match[1]);
      if (!fileRecord) return error(404, "not_found");
      const binary = await store.readBinary(match[1]);
      if (!binary) return error(404, "no_binary_data");

      const { bytesToBase64 } = await import("./detector");
      const results = await detectMaps({
        fileDataBase64: bytesToBase64(binary),
        fileName: fileRecord.original_name || fileRecord.file_name,
        ecuType: fileRecord.ecu_type || undefined,
      });

      await store.updateFile(match[1], {
        detection_data: results,
        maps_detected: results.total_maps || 0,
      });

      return ok({
        success: true,
        detectionResults: results,
        message: `Re-détection terminée: ${results.total_maps} maps trouvées`,
      });
    }

    // ── Mappack (free and unlimited in the local app) ─────────────
    if (path === "/api/mappack/status") {
      return ok({
        unlocked: true,
        isPro: true,
        isAdmin: false,
        canUnlock: false,
        exported: false,
        exportEnabled: true,
        mappackPrice: 0,
      });
    }
    if (path === "/api/mappack/unlock" && m === "POST") {
      return ok({ success: true, unlocked: true, creditsRemaining: 0 });
    }
    if (path === "/api/mappack/export" && m === "POST") {
      const fileId = body?.fileId;
      if (!fileId) return error(400, "bad_request");
      const fileRecord = await store.getFile(fileId);
      if (!fileRecord) return error(404, "not_found");

      let detection: { maps?: ExportMapData[] } | null = null;
      try {
        detection =
          typeof fileRecord.detection_data === "string"
            ? JSON.parse(fileRecord.detection_data)
            : fileRecord.detection_data;
      } catch {
        detection = null;
      }
      if (!detection || !Array.isArray(detection.maps) || detection.maps.length === 0) {
        return error(400, "no_maps");
      }

      // Tri : celui mémorisé avec le projet, sinon celui transmis par l'éditeur
      // (adresse/nom) — le mappack reprend l'ordre affiché dans la liste des maps
      const requested = fileRecord.map_sort_mode || body?.sortMode;
      const sortMode =
        requested === "name" || requested === "name-desc" ? requested : "address";
      // Réglages d'affichage par map (miroirs d'axes de la fenêtre Propriétés)
      // mémorisés avec le projet : appliqués au mappack (AxisX/Y.bBackwards)
      const displaySettings =
        fileRecord.map_display_settings && typeof fileRecord.map_display_settings === "object"
          ? (fileRecord.map_display_settings as Record<string, MappackDisplaySettings>)
          : undefined;
      const pack = buildWinolsMappack(
        detection.maps,
        fileRecord.ecu_type || "",
        sortMode,
        displaySettings
      );
      const bytes = serializeWinolsMappack(pack);
      const fileName = mappackFileName(
        fileRecord.project_name || fileRecord.file_name || "project"
      );

      return {
        status: 200,
        body: bytes,
        headers: {
          "Content-Type": "application/json; charset=iso-8859-1",
          "X-Mappack-Filename": encodeURIComponent(fileName),
          "X-Credits-Remaining": "0",
          "X-Maps-Count": String(pack.maps.length),
        },
      };
    }

    console.warn(`[local-api] Unhandled route: ${m} ${path}`);
    return error(404, "route_not_found");
  } catch (e: any) {
    console.error(`[local-api] ${m} ${path} failed:`, e);
    return error(500, String(e?.message || e));
  }
}
