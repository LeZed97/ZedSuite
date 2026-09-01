import Link from 'next/link';
import { Button } from '@/components/ui/button';

export default function NotFound() {
  return (
    <div className="flex min-h-screen flex-col items-center justify-center gap-4 p-8">
      <h2 className="text-2xl font-bold text-white">404 - Page non trouvée</h2>
      <p className="text-white/70">La page que vous recherchez n'existe pas.</p>
      <Link href="/">
        <Button variant="default">Retour à l'accueil</Button>
      </Link>
    </div>
  );
}

