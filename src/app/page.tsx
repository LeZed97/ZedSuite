"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";

// The desktop app opens straight on the dashboard — no landing/login page.
export default function HomePage() {
  const router = useRouter();

  useEffect(() => {
    router.replace("/dashboard");
  }, [router]);

  return (
    <div className="min-h-screen" style={{ backgroundColor: "#0a0b0f" }} />
  );
}
