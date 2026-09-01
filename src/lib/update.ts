// Update-system frontend helpers.
//
// The Rust side (src-tauri/src/update.rs) checks the latest GitHub release
// and downloads/launches the NSIS installer. This module wraps the IPC
// calls and keeps the check state in localStorage:
//  - updateLastCheck       timestamp of the last background check (24h cadence)
//  - updateSkippedVersion  release tag the user chose to skip
//  - zedsuiteFirstRunDone  set once the first-run dialog has been completed

import { invoke } from "@tauri-apps/api/core";

export interface UpdateInfo {
  update_available: boolean;
  current_version: string;
  latest_version: string;
  release_notes: string;
  download_url: string | null;
  release_url: string;
}

const LS_LAST_CHECK = "updateLastCheck";
const LS_SKIPPED = "updateSkippedVersion";
const LS_FIRST_RUN = "zedsuiteFirstRunDone";

export const CHECK_INTERVAL_MS = 24 * 60 * 60 * 1000; // once a day

/** Custom event dispatched to open the update dialog (detail: UpdateInfo). */
export const UPDATE_AVAILABLE_EVENT = "zedsuite-update-available";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function checkForUpdate(): Promise<UpdateInfo> {
  return invoke<UpdateInfo>("check_for_update");
}

/** Starts the download; the app quits by itself once the installer runs. */
export async function downloadAndInstallUpdate(url: string, version: string): Promise<void> {
  return invoke("download_and_install_update", { url, version });
}

export function markUpdateCheckDone(): void {
  localStorage.setItem(LS_LAST_CHECK, String(Date.now()));
}

export function shouldAutoCheck(): boolean {
  const last = Number(localStorage.getItem(LS_LAST_CHECK) || 0);
  return Date.now() - last >= CHECK_INTERVAL_MS;
}

export function getSkippedVersion(): string {
  return localStorage.getItem(LS_SKIPPED) || "";
}

export function skipVersion(version: string): void {
  localStorage.setItem(LS_SKIPPED, version);
}

export function isFirstRun(): boolean {
  return !localStorage.getItem(LS_FIRST_RUN);
}

export function markFirstRunDone(): void {
  localStorage.setItem(LS_FIRST_RUN, "1");
}

/**
 * Background check shared by the daily timer and the post-first-run check.
 * Silent on failure (offline, private repo, no release). Returns the info
 * when an update is available and not skipped by the user.
 */
export async function backgroundUpdateCheck(): Promise<UpdateInfo | null> {
  if (!isTauri()) return null;
  try {
    const info = await checkForUpdate();
    markUpdateCheckDone();
    if (info.update_available && info.latest_version !== getSkippedVersion()) {
      return info;
    }
    return null;
  } catch {
    // Silent: next attempt in 24h (do NOT mark the check as done so a
    // temporary network failure retries on the next hourly tick)
    return null;
  }
}
