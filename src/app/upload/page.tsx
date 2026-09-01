"use client";

import { useEffect } from "react";
import { useRouter } from "next/navigation";

export default function UploadPage() {
  const router = useRouter();

  useEffect(() => {
    // Redirect to dashboard - upload is now handled via modal
    router.replace("/dashboard");
  }, [router]);

  return null;
}
