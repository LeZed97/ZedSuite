// Feuille de route publique : ROADMAP.md du dépôt et ses traductions
// ROADMAP.<lang>.md, lues sur GitHub par la commande Rust dans la langue de
// l'app (repli sur la copie embarquée à la compilation hors ligne).
import { invoke } from "@tauri-apps/api/core";

export const ZEDSUITE_ROADMAP_URL = "https://github.com/LeZed97/ZedSuite/blob/master/ROADMAP.md";

export interface RoadmapContent {
  markdown: string;
  /** "github" (à jour) ou "bundled" (copie livrée avec cette version) */
  source: "github" | "bundled";
}

export async function loadRoadmap(language: string): Promise<RoadmapContent> {
  try {
    return await invoke<RoadmapContent>("fetch_roadmap", { lang: language.toLowerCase() });
  } catch {
    // Hors Tauri (navigateur de dev) : rien d'embarqué côté web
    return { markdown: "# Roadmap\n\nOpen the roadmap on GitHub.", source: "bundled" };
  }
}

// ── Rendu Markdown minimal (titres, listes, paragraphes, gras, code, liens) ──

export type RoadmapBlock =
  | { type: "heading"; level: 1 | 2 | 3; text: string }
  | { type: "paragraph"; text: string }
  | { type: "list"; items: string[] }
  | { type: "rule" };

export function parseRoadmapMarkdown(markdown: string): RoadmapBlock[] {
  const blocks: RoadmapBlock[] = [];
  let paragraph: string[] = [];
  let list: string[] = [];
  const flushParagraph = () => {
    if (paragraph.length) {
      blocks.push({ type: "paragraph", text: paragraph.join(" ") });
      paragraph = [];
    }
  };
  const flushList = () => {
    if (list.length) {
      blocks.push({ type: "list", items: list });
      list = [];
    }
  };
  for (const raw of markdown.split(/\r?\n/)) {
    const line = raw.trimEnd();
    const heading = /^(#{1,3})\s+(.*)$/.exec(line);
    if (heading) {
      flushParagraph();
      flushList();
      blocks.push({ type: "heading", level: heading[1].length as 1 | 2 | 3, text: heading[2].trim() });
      continue;
    }
    if (/^(-{3,}|\*{3,})$/.test(line.trim())) {
      flushParagraph();
      flushList();
      blocks.push({ type: "rule" });
      continue;
    }
    const item = /^\s*[-*]\s+(.*)$/.exec(line);
    if (item) {
      flushParagraph();
      list.push(item[1].trim());
      continue;
    }
    if (line.trim() === "") {
      flushParagraph();
      flushList();
      continue;
    }
    // Continuation d'un élément de liste indenté
    if (list.length && /^\s{2,}\S/.test(raw)) {
      list[list.length - 1] += " " + line.trim();
      continue;
    }
    flushList();
    paragraph.push(line.trim());
  }
  flushParagraph();
  flushList();
  return blocks;
}

export type RoadmapInline =
  | { type: "text"; text: string }
  | { type: "bold"; text: string }
  | { type: "italic"; text: string }
  | { type: "code"; text: string }
  | { type: "link"; text: string; url: string };

/** Découpe une ligne en segments : **gras**, _italique_, `code`, [texte](url). */
export function parseInline(text: string): RoadmapInline[] {
  const out: RoadmapInline[] = [];
  const re = /(\*\*([^*]+)\*\*)|(`([^`]+)`)|(\[([^\]]+)\]\(([^)\s]+)\))|(_([^_]+)_)/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) out.push({ type: "text", text: text.slice(last, m.index) });
    if (m[1]) out.push({ type: "bold", text: m[2] });
    else if (m[3]) out.push({ type: "code", text: m[4] });
    else if (m[5]) out.push({ type: "link", text: m[6], url: m[7] });
    else if (m[8]) out.push({ type: "italic", text: m[9] });
    last = m.index + m[0].length;
  }
  if (last < text.length) out.push({ type: "text", text: text.slice(last) });
  return out;
}
