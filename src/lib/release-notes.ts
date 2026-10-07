// Notes de version de GitHub, découpées pour la fenêtre de mise à jour.
// Le corps d'une release est écrit en Markdown léger : une ligne de titre
// « ZedSuite 1.2.6 », des sections en gras seules sur leur ligne
// (« **Editor** »), des puces « - … » avec du gras, du code et des numéros
// d'issue (#56), et parfois une règle « --- » qui sépare le reste. La fenêtre
// affichait tout cela brut dans un <pre>. Ici on en fait des blocs simples.

export type NoteInline =
  | { kind: "text"; text: string }
  | { kind: "bold"; text: string }
  | { kind: "code"; text: string }
  | { kind: "issue"; number: number };

export type NoteBlock =
  | { kind: "heading"; text: string }
  | { kind: "item"; inlines: NoteInline[] }
  | { kind: "paragraph"; inlines: NoteInline[] };

export const ZEDSUITE_ISSUE_URL = "https://github.com/LeZed97/ZedSuite/issues/";

/** Gras, code et numéros d'issue d'une ligne. */
export function parseNoteInlines(text: string): NoteInline[] {
  const out: NoteInline[] = [];
  const re = /\*\*([^*]+)\*\*|`([^`]+)`|(?<![\w/])#(\d{1,5})\b/g;
  let last = 0;
  for (const m of text.matchAll(re)) {
    const start = m.index ?? 0;
    if (start > last) out.push({ kind: "text", text: text.slice(last, start) });
    if (m[1] !== undefined) out.push({ kind: "bold", text: m[1] });
    else if (m[2] !== undefined) out.push({ kind: "code", text: m[2] });
    else out.push({ kind: "issue", number: parseInt(m[3], 10) });
    last = start + m[0].length;
  }
  if (last < text.length) out.push({ kind: "text", text: text.slice(last) });
  return out;
}

/**
 * Blocs d'une note de version. La ligne de titre « ZedSuite x.y.z » est
 * laissée de côté (la fenêtre affiche déjà le numéro), tout ce qui suit une
 * règle « --- » aussi (résumé dans une autre langue, liens de téléchargement).
 */
export function parseReleaseNotes(body: string): NoteBlock[] {
  const blocks: NoteBlock[] = [];
  const lines = body.replace(/\r\n?/g, "\n").split("\n");
  let paragraph: string[] = [];
  const flush = () => {
    if (paragraph.length === 0) return;
    blocks.push({ kind: "paragraph", inlines: parseNoteInlines(paragraph.join(" ")) });
    paragraph = [];
  };
  for (const raw of lines) {
    const line = raw.trim();
    if (line === "") {
      flush();
      continue;
    }
    if (/^-{3,}$|^\*{3,}$/.test(line)) break;
    if (blocks.length === 0 && paragraph.length === 0 && /^(#+\s*)?ZedSuite\s+\d/i.test(line)) continue;
    const heading = line.match(/^(?:#{1,6}\s+(.+)|\*\*([^*]+)\*\*:?)$/);
    if (heading) {
      flush();
      blocks.push({ kind: "heading", text: (heading[1] ?? heading[2]).trim() });
      continue;
    }
    const item = line.match(/^[-*•]\s+(.+)$/);
    if (item) {
      flush();
      blocks.push({ kind: "item", inlines: parseNoteInlines(item[1]) });
      continue;
    }
    paragraph.push(line);
  }
  flush();
  return blocks;
}
