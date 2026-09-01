// API bridge — routes the app's HTTP calls to the local backend.
//
// The editor/dashboard code still performs fetch()/axios calls on /api/*
// (inherited from the web version). This module patches both so those
// requests never leave the app: they are answered synchronously by
// handleLocalApi (api.ts) on top of the local project store.
//
// Installed once from LocalBridgeProvider (imported by the root layout).

import axios from "axios";
import { handleLocalApi, type LocalApiResult } from "./api";

let installed = false;

function isLocalApiUrl(url: string): boolean {
  return url.startsWith("/api/");
}

function toResponse(result: LocalApiResult): Response {
  if (result.body) {
    return new Response(new Blob([result.body as BlobPart]), {
      status: result.status,
      headers: result.headers,
    });
  }
  return new Response(JSON.stringify(result.json ?? {}), {
    status: result.status,
    headers: { "Content-Type": "application/json", ...(result.headers || {}) },
  });
}

export function installLocalApiBridge(): void {
  if (installed || typeof window === "undefined") return;
  installed = true;

  // ── fetch ────────────────────────────────────────────────────────
  const originalFetch = window.fetch.bind(window);
  window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.href
          : input.url;

    if (!isLocalApiUrl(url)) {
      return originalFetch(input as any, init);
    }

    const method = init?.method || (typeof input === "object" && "method" in input ? input.method : "GET");
    let body: any = undefined;
    if (init?.body && typeof init.body === "string") {
      try {
        body = JSON.parse(init.body);
      } catch {
        body = init.body;
      }
    }
    const result = await handleLocalApi(method || "GET", url, body);
    return toResponse(result);
  };

  // ── axios ────────────────────────────────────────────────────────
  const originalAdapter = axios.defaults.adapter;
  axios.defaults.adapter = async (config) => {
    const url = config.url || "";
    if (!isLocalApiUrl(url)) {
      const fallback = axios.getAdapter(originalAdapter || "xhr");
      return fallback(config);
    }

    let body: any = config.data;
    if (typeof body === "string") {
      try {
        body = JSON.parse(body);
      } catch {
        // keep as string
      }
    }

    const result = await handleLocalApi(config.method || "get", url, body);

    if (result.status >= 400) {
      const err: any = new Error(`Request failed with status code ${result.status}`);
      err.response = {
        data: result.json,
        status: result.status,
        statusText: String(result.status),
        headers: result.headers || {},
        config,
      };
      err.isAxiosError = true;
      err.config = config;
      throw err;
    }

    return {
      data: result.body ?? result.json,
      status: result.status,
      statusText: "OK",
      headers: result.headers || {},
      config,
      request: {},
    };
  };
}
