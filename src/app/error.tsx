'use client';

import { useEffect } from 'react';
import { Button } from '@/components/ui/button';

export default function Error({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  useEffect(() => {
    // Log the error to an error reporting service
    console.error('Application error:', error);
  }, [error]);

  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4 p-8">
      <h2 className="text-2xl font-bold text-white">Une erreur s'est produite</h2>
      <p className="text-white/70">{error.message || 'Une erreur inattendue est survenue'}</p>
      <Button onClick={reset} variant="default">
        Réessayer
      </Button>
    </div>
  );
}

