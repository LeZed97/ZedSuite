"use client";

// App-level bootstrap mounted once in the root layout:
//  - shows the first-run setup dialog when the app has never been configured
//  - runs the daily background update check (on start + hourly ticks that
//    fire once 24h have elapsed, so an app left open still checks daily)
//  - right after the first-run dialog completes, checks once in background
//  - hosts the update dialog, also opened by the settings page through the
//    UPDATE_AVAILABLE_EVENT custom event (manual "check now" button)

import { useCallback, useEffect, useState } from "react";
import { FirstRunDialog } from "@/components/first-run-dialog";
import { UpdateDialog } from "@/components/update-dialog";
import {
  backgroundUpdateCheck,
  isFirstRun,
  markFirstRunDone,
  shouldAutoCheck,
  skipVersion,
  UPDATE_AVAILABLE_EVENT,
  type UpdateInfo,
} from "@/lib/update";

export function AppBootstrap() {
  const [showFirstRun, setShowFirstRun] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);

  const runBackgroundCheck = useCallback(async () => {
    const info = await backgroundUpdateCheck();
    if (info) setUpdateInfo(info);
  }, []);

  // Startup: first-run dialog, or daily background check
  useEffect(() => {
    if (isFirstRun()) {
      setShowFirstRun(true);
      return;
    }
    if (shouldAutoCheck()) {
      void runBackgroundCheck();
    }
  }, [runBackgroundCheck]);

  // Hourly tick — performs the daily check even if the app never restarts
  useEffect(() => {
    const timer = setInterval(() => {
      if (!isFirstRun() && shouldAutoCheck()) {
        void runBackgroundCheck();
      }
    }, 60 * 60 * 1000);
    return () => clearInterval(timer);
  }, [runBackgroundCheck]);

  // Manual check from the settings page
  useEffect(() => {
    const onAvailable = (e: Event) => {
      const info = (e as CustomEvent).detail as UpdateInfo | undefined;
      if (info) setUpdateInfo(info);
    };
    window.addEventListener(UPDATE_AVAILABLE_EVENT, onAvailable);
    return () => window.removeEventListener(UPDATE_AVAILABLE_EVENT, onAvailable);
  }, []);

  const handleFirstRunDone = () => {
    markFirstRunDone();
    setShowFirstRun(false);
    // Premier lancement configuré : vérification de mise à jour en fond
    void runBackgroundCheck();
  };

  return (
    <>
      {showFirstRun && <FirstRunDialog onDone={handleFirstRunDone} />}
      {!showFirstRun && updateInfo && (
        <UpdateDialog
          info={updateInfo}
          onClose={() => setUpdateInfo(null)}
          onSkip={() => {
            skipVersion(updateInfo.latest_version);
            setUpdateInfo(null);
          }}
        />
      )}
    </>
  );
}
