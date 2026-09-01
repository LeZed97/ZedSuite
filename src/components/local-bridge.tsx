"use client";

import { installLocalApiBridge } from "@/lib/local/bridge";

// Install at module-evaluation time so the bridge is active before any
// component effect fires its first fetch/axios call.
installLocalApiBridge();

export function LocalBridge({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}
