// Dégradé de marque (rouge → orange, comme le wordmark) exposé aux icônes
// SVG via stroke="url(#zedIconGradient)". À monter une fois par page.

/** Icône fichier (tracés lucide FileText) : contour au dégradé de marque,
 *  barres de texte dans une couleur unie (blanc par défaut). */
export function ZedFileIcon({
  className,
  barColor = "#ffffff",
}: {
  className?: string;
  barColor?: string;
}) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" stroke="url(#zedIconGradient)" />
      <path d="M14 2v4a2 2 0 0 0 2 2h4" stroke="url(#zedIconGradient)" />
      <path d="M10 9H8" stroke={barColor} />
      <path d="M16 13H8" stroke={barColor} />
      <path d="M16 17H8" stroke={barColor} />
    </svg>
  );
}

/** Icône presse-papiers à liste (tracés lucide ClipboardList) : contour au
 *  dégradé de marque, lignes de la liste dans une couleur unie (blanc en
 *  thème sombre, noir en clair). Fenêtre de la feuille de route. */
export function ZedClipboardIcon({
  className,
  barColor = "#ffffff",
}: {
  className?: string;
  barColor?: string;
}) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <rect width="8" height="4" x="8" y="2" rx="1" ry="1" stroke="url(#zedIconGradient)" />
      <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" stroke="url(#zedIconGradient)" />
      <path d="M12 11h4" stroke={barColor} />
      <path d="M12 16h4" stroke={barColor} />
      <path d="M8 11h.01" stroke={barColor} />
      <path d="M8 16h.01" stroke={barColor} />
    </svg>
  );
}

export default function ZedGradientDefs() {
  return (
    <svg width="0" height="0" style={{ position: "absolute" }} aria-hidden>
      <defs>
        <linearGradient id="zedIconGradient" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stopColor="#dc2626" />
          <stop offset="50%" stopColor="#ef4444" />
          <stop offset="100%" stopColor="#f97316" />
        </linearGradient>
      </defs>
    </svg>
  );
}
