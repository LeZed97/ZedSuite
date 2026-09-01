"use client";

// Update-available dialog — shown by AppBootstrap when a background or
// manual check finds a newer GitHub release. Three choices:
//  - update now (downloads the installer with a progress bar, then the app
//    quits and NSIS takes over)
//  - next time (just closes; the daily check will offer it again)
//  - skip this version (never offered again until an even newer release)

import { useEffect, useState } from "react";
import { Download, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { MODAL_GLASS } from "@/lib/modal-glass";
import { useI18n } from "@/contexts/i18n-context";
import { downloadAndInstallUpdate, type UpdateInfo } from "@/lib/update";

interface UpdateDialogProps {
  info: UpdateInfo;
  /** "Next time": close without any persistent choice. */
  onClose: () => void;
  /** "Skip this version": persist the skipped tag and close. */
  onSkip: () => void;
}

export function UpdateDialog({ info, onClose, onSkip }: UpdateDialogProps) {
  const { t } = useI18n();
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<{ downloaded: number; total: number | null }>({
    downloaded: 0,
    total: null,
  });
  const [error, setError] = useState<string | null>(null);

  // Download progress events from the Rust side
  useEffect(() => {
    if (!downloading) return;
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    void import("@tauri-apps/api/event").then(({ listen }) =>
      listen<{ downloaded: number; total: number | null }>(
        "update-download-progress",
        (e) => setProgress(e.payload)
      ).then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      })
    );
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [downloading]);

  const startUpdate = async () => {
    if (!info.download_url) {
      setError(t.updateDialog.noInstaller);
      return;
    }
    setError(null);
    setDownloading(true);
    try {
      await downloadAndInstallUpdate(info.download_url, info.latest_version);
      // On success the app exits by itself; nothing more to do here.
    } catch (e) {
      setDownloading(false);
      setError(String(e));
    }
  };

  const pct =
    progress.total && progress.total > 0
      ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
      : null;
  const mb = (n: number) => (n / (1024 * 1024)).toFixed(1);

  return (
    <div
      className="fixed inset-0 z-[90] flex items-center justify-center backdrop-blur-sm"
      style={{ backgroundColor: "#000000a2", animation: "backdropFadeIn 0.2s ease-out forwards" }}
    >
      <div
        className="relative w-full max-w-lg mx-4"
        style={{ animation: "modalExpand 0.2s ease-out forwards" }}
      >
        <div className="border rounded-lg p-6" style={MODAL_GLASS}>
          <div className="flex items-center gap-3 mb-4">
            <div className="p-2 rounded-full bg-red-500/20">
              <Download className="w-6 h-6 text-red-400" />
            </div>
            <div>
              <h3 className="text-lg font-semibold text-white">{t.updateDialog.title}</h3>
              <p className="text-sm text-slate-400">
                {t.updateDialog.versionLine
                  .replace("{latest}", info.latest_version)
                  .replace("{current}", info.current_version)}
              </p>
            </div>
          </div>

          {info.release_notes && (
            <div className="mb-5 max-h-44 overflow-y-auto rounded-lg border border-white/[0.08] bg-black/20 p-3">
              <pre className="whitespace-pre-wrap font-sans text-sm text-slate-300">
                {info.release_notes}
              </pre>
            </div>
          )}

          {downloading && (
            <div className="mb-5">
              <div className="flex justify-between text-xs text-slate-400 mb-1.5">
                <span className="flex items-center gap-1.5">
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                  {t.updateDialog.downloading}
                </span>
                <span>
                  {pct !== null
                    ? `${pct}% (${mb(progress.downloaded)} / ${mb(progress.total!)} Mo)`
                    : `${mb(progress.downloaded)} Mo`}
                </span>
              </div>
              <div className="h-2 rounded-full bg-white/[0.08] overflow-hidden">
                <div
                  className="h-full rounded-full bg-gradient-to-r from-red-600 to-orange-500 transition-all duration-200"
                  style={{ width: pct !== null ? `${pct}%` : "100%" }}
                />
              </div>
            </div>
          )}

          {error && (
            <p className="mb-4 text-sm text-red-400">
              {t.updateDialog.error} {error}
            </p>
          )}

          <div className="flex flex-wrap gap-3 justify-end">
            <Button
              variant="ghost"
              size="sm"
              className="h-9 px-4 text-slate-400 hover:text-white"
              onClick={onSkip}
              disabled={downloading}
            >
              {t.updateDialog.skipVersion}
            </Button>
            <Button
              variant="outline"
              size="sm"
              className="h-9 px-4"
              onClick={onClose}
              disabled={downloading}
            >
              {t.updateDialog.later}
            </Button>
            <Button
              size="sm"
              className="h-9 px-6 text-white bg-gradient-to-r from-red-600 via-red-500 to-orange-500 hover:from-red-500 hover:via-red-400 hover:to-orange-400"
              onClick={startUpdate}
              disabled={downloading}
            >
              {downloading ? t.updateDialog.updating : t.updateDialog.updateNow}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
