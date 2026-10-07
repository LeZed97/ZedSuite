"use client";

// Update-available dialog — shown by AppBootstrap when a background or
// manual check finds a newer GitHub release. Three choices:
//  - update now (downloads the installer with a progress bar, then the app
//    quits and NSIS takes over)
//  - next time (just closes; the daily check will offer it again)
//  - skip this version (never offered again until an even newer release)

import { useEffect, useMemo, useState } from "react";
import { Download, RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { openExternal, ZEDSUITE_RELEASES_URL } from "@/lib/open-external";
import { parseReleaseNotes, ZEDSUITE_ISSUE_URL, type NoteInline } from "@/lib/release-notes";
import { MODAL_GLASS } from "@/lib/modal-glass";
import { useI18n } from "@/contexts/i18n-context";
import { describeUpdateError, downloadAndInstallUpdate, type UpdateInfo } from "@/lib/update";
import { isLinux, isMacOS } from "@/lib/platform";

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
  // Notes de la release (en anglais, telles qu'écrites sur GitHub) en blocs
  // lisibles : sections, puces, gras, numéros d'issue cliquables
  const notes = useMemo(() => parseReleaseNotes(info.release_notes || ""), [info.release_notes]);
  const renderInlines = (inlines: NoteInline[]) =>
    inlines.map((part, i) => {
      if (part.kind === "bold") return <strong key={i} className="font-semibold text-white">{part.text}</strong>;
      if (part.kind === "code") return <code key={i} className="px-1 rounded bg-white/[0.08] font-mono text-[12px]">{part.text}</code>;
      if (part.kind === "issue") {
        return (
          <button
            key={i}
            type="button"
            onClick={() => void openExternal(`${ZEDSUITE_ISSUE_URL}${part.number}`)}
            className="underline underline-offset-2 text-slate-200 hover:text-white"
          >
            #{part.number}
          </button>
        );
      }
      return <span key={i}>{part.text}</span>;
    });

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
      if (isLinux()) {
        // Release without a Linux asset (or a copy that is neither an
        // AppImage nor a .deb install): the releases page instead.
        void openExternal(ZEDSUITE_RELEASES_URL);
        return;
      }
      // Release sans build pour cette plateforme (macOS publié après Windows)
      setError(isMacOS() ? t.updateDialog.noInstallerMac : t.updateDialog.noInstaller);
      return;
    }
    setError(null);
    setDownloading(true);
    try {
      await downloadAndInstallUpdate(info.download_url, info.latest_version);
      // On success the app exits by itself; nothing more to do here.
    } catch (e) {
      setDownloading(false);
      setError(describeUpdateError(String(e), t.updateDialog));
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
        className="relative w-full max-w-xl mx-4"
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

          {notes.length > 0 && (
            <div className="mb-5">
              <p className="mb-1.5 text-xs uppercase tracking-wide text-slate-400">{t.updateDialog.whatsNew}</p>
              <div className="max-h-80 overflow-y-auto rounded-lg border border-white/[0.08] bg-black/20 px-4 py-3 text-[13px] leading-relaxed text-slate-300">
                {notes.map((block, i) =>
                  block.kind === "heading" ? (
                    <p key={i} className={`text-xs uppercase tracking-wide text-slate-400 ${i === 0 ? "" : "mt-3"} mb-1`}>
                      {block.text}
                    </p>
                  ) : block.kind === "item" ? (
                    <p key={i} className="flex gap-2 mb-1.5">
                      <span className="shrink-0 text-slate-500">•</span>
                      <span>{renderInlines(block.inlines)}</span>
                    </p>
                  ) : (
                    <p key={i} className="mb-1.5">{renderInlines(block.inlines)}</p>
                  ),
                )}
              </div>
            </div>
          )}

          {/* Liste complète des mises à jour (sur demande : pour ceux que le détail intéresse) */}
          <p className="mb-5 text-xs text-slate-400">
            {t.updateDialog.allReleases}{" "}
            <button
              type="button"
              onClick={() => void openExternal(ZEDSUITE_RELEASES_URL)}
              className="underline underline-offset-2 text-slate-200 hover:text-white"
            >
              github.com/LeZed97/ZedSuite/releases
            </button>
          </p>
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
              {downloading
                ? t.updateDialog.updating
                : isLinux() && !info.download_url
                  ? t.updateDialog.viewOnGithub
                  : t.updateDialog.updateNow}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}
