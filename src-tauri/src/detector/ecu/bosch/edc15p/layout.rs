//! Disposition mémoire d'un fichier EDC15P : génération de calibration et
//! codeblocks.
//!
//! Trois dispositions rencontrées sur des dumps de 512 Ko :
//! - **Standard** (2001+, ex. 038906019GD) : blocs de 0x10000 à 0x4C000 /
//!   0x5C000 / 0x6C000, signature V4.1 (`67 FF FF FF FF FF FF 'V4.1'`) à
//!   +0x4001 du bloc, 0x4000 octets de maps AVANT la signature ;
//! - **Compacte** (2002, 038906019AJ) : blocs de 0xC000 à 0x58000 / 0x64000 /
//!   0x70000, signature à +1 du bloc, la zone 0x4000 qui précède n'est que du
//!   remplissage C3 ;
//! - **Précoce** (1999, 038906019A) : aucune signature V4.1, quatre blocs de
//!   0x8000 (0x0000, 0x8000, 0x10000, 0x78000) terminés chacun par le marqueur
//!   `3C 3C 41 E4` suivi de la somme de contrôle.
//!
//! L'ancien modèle (plages fixes 0x4C000/0x5C000/0x6C000, métadonnées à
//! 0x50000/0x60000/0x70000) découpait les dispositions compacte et précoce
//! au mauvais endroit : identifiants de codeblock aberrants (58882), maps
//! d'un bloc dédupliquées contre celles du voisin, sélecteurs perdus.

/// Signature V4.1 placée un octet après le début du bloc de code signé.
pub const V41_SIGNATURE: [u8; 11] = [
    0x67, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x56, 0x34, 0x2E, 0x31,
];

/// Marqueur de fin de bloc des dispositions précoces (suivi de 4 octets de
/// somme de contrôle, à `fin - 8`).
const EARLY_BLOCK_MARKER: [u8; 4] = [0x3C, 0x3C, 0x41, 0xE4];
const EARLY_BLOCK_SIZE: usize = 0x8000;

/// Taille du bloc de code signé (signature → fin), commune aux dispositions
/// standard et compacte.
const SIGNED_BLOCK_SIZE: usize = 0xC000;
/// Zone de maps qui précède la signature sur la disposition standard.
const STANDARD_PRE_ZONE: usize = 0x4000;

/// Identifiants par défaut de la disposition standard (convention EDCSuite :
/// codeblocks 2, 3 et 5), quand les métadonnées du fichier sont illisibles.
const STANDARD_DEFAULT_IDS: [(u32, u32); 3] = [(0x4C000, 2), (0x5C000, 3), (0x6C000, 5)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edc15pGeneration {
    Standard,
    Compact,
    Early,
}

#[derive(Debug, Clone)]
pub struct LayoutBlock {
    pub id: u32,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone)]
pub struct Edc15pLayout {
    pub generation: Edc15pGeneration,
    pub blocks: Vec<LayoutBlock>,
}

impl Edc15pLayout {
    pub fn detect(data: &[u8]) -> Self {
        let signatures = find_signatures(data);
        if !signatures.is_empty() {
            return Self::from_signatures(data, &signatures);
        }
        let early = early_blocks(data);
        if !early.is_empty() {
            let blocks = early
                .into_iter()
                .enumerate()
                .map(|(i, (start, end))| LayoutBlock {
                    id: i as u32 + 1,
                    start: start as u32,
                    end: end as u32,
                })
                .collect();
            return Self {
                generation: Edc15pGeneration::Early,
                blocks,
            };
        }
        // Rien de reconnu : plages EDCSuite classiques, comme avant.
        Self {
            generation: Edc15pGeneration::Standard,
            blocks: STANDARD_DEFAULT_IDS
                .iter()
                .map(|&(start, id)| LayoutBlock {
                    id,
                    start,
                    end: start + 0x10000,
                })
                .collect(),
        }
    }

    fn from_signatures(data: &[u8], signatures: &[usize]) -> Self {
        // Compacte quand les blocs signés se suivent sans zone de maps devant
        // (signatures espacées de 0xC000), ou quand la zone de 0x4000 qui
        // précède le PREMIER bloc n'est que du remplissage. Les blocs
        // compacts suivants ont, eux, la fin du bloc précédent devant eux.
        let first_base = signatures[0] - 1;
        let first_pre_is_filler = first_base
            .checked_sub(STANDARD_PRE_ZONE)
            .map(|pre| is_filler(&data[pre..first_base]))
            .unwrap_or(true);
        let compact = first_pre_is_filler
            || signatures
                .windows(2)
                .any(|w| w[1] - w[0] == SIGNED_BLOCK_SIZE);
        let generation = if compact {
            Edc15pGeneration::Compact
        } else {
            Edc15pGeneration::Standard
        };

        let mut raw: Vec<(usize, usize, Option<u32>)> = Vec::new();
        for &sig in signatures {
            let base = sig - 1;
            let end = (base + SIGNED_BLOCK_SIZE).min(data.len());
            let start = if compact {
                base
            } else {
                base.saturating_sub(STANDARD_PRE_ZONE)
            };
            raw.push((start, end, read_metadata_id(data, base)));
        }
        // Deux blocs ne se chevauchent jamais.
        for i in 1..raw.len() {
            if raw[i].0 < raw[i - 1].1 {
                raw[i].0 = raw[i - 1].1;
            }
        }

        let ids: Vec<Option<u32>> = raw.iter().map(|r| r.2).collect();
        let all_valid = ids.iter().all(|id| id.is_some()) && {
            let mut seen = std::collections::HashSet::new();
            ids.iter().flatten().all(|id| seen.insert(*id))
        };
        let blocks = raw
            .iter()
            .enumerate()
            .map(|(i, &(start, end, id))| {
                let id = if all_valid {
                    id.unwrap()
                } else {
                    STANDARD_DEFAULT_IDS
                        .iter()
                        .find(|(s, _)| *s as usize == start)
                        .map(|(_, id)| *id)
                        .unwrap_or(i as u32 + 1)
                };
                LayoutBlock {
                    id,
                    start: start as u32,
                    end: end as u32,
                }
            })
            .collect();
        Self { generation, blocks }
    }

    pub fn is_standard(&self) -> bool {
        self.generation == Edc15pGeneration::Standard
    }

    /// Indice du bloc contenant l'adresse. Les adresses situées juste avant le
    /// premier bloc (jusqu'à 0xC000 en amont, comme les anciennes plages
    /// 0x40000-0x4C000) ou juste après le dernier (0x4000) lui sont rattachées.
    pub fn block_index(&self, address: u32) -> Option<usize> {
        if let Some(i) = self
            .blocks
            .iter()
            .position(|b| address >= b.start && address < b.end)
        {
            return Some(i);
        }
        let first = self.blocks.first()?;
        if address < first.start && address + 0xC000 >= first.start {
            return Some(0);
        }
        let last = self.blocks.last()?;
        if address >= last.end && address < last.end + 0x4000 {
            return Some(self.blocks.len() - 1);
        }
        None
    }

    pub fn block(&self, address: u32) -> Option<&LayoutBlock> {
        self.block_index(address).map(|i| &self.blocks[i])
    }

    pub fn block_id(&self, address: u32) -> Option<u32> {
        self.block(address).map(|b| b.id)
    }

    /// Première adresse à balayer pour les passes qui démarrent « dans les
    /// codeblocks » (0 sur la disposition précoce, ~0x4C000 sinon).
    pub fn scan_start(&self) -> usize {
        self.blocks
            .iter()
            .map(|b| b.start as usize)
            .min()
            .unwrap_or(0)
    }
}

/// Positions de toutes les signatures V4.1 du fichier.
pub fn find_signatures(data: &[u8]) -> Vec<usize> {
    let n = V41_SIGNATURE.len();
    if data.len() < n {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut i = 1; // la signature suit toujours un octet de bloc
    while i + n <= data.len() {
        if data[i] == 0x67 && data[i..i + n] == V41_SIGNATURE {
            out.push(i);
            i += SIGNED_BLOCK_SIZE / 2;
            continue;
        }
        i += 1;
    }
    out
}

/// Vrai si le fichier porte au moins deux blocs de calibration précoces
/// (marqueur `3C 3C 41 E4` en fin de bloc de 32 Ko) — sert à l'identification
/// des dumps sans signature V4.1 (038906019A).
pub fn has_early_block_markers(data: &[u8]) -> bool {
    early_blocks(data).len() >= 2
}

/// Blocs de 0x8000 alignés terminés par le marqueur `3C 3C 41 E4` (fin - 8).
fn early_blocks(data: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut end = EARLY_BLOCK_SIZE;
    while end <= data.len() {
        if data[end - 8..end - 4] == EARLY_BLOCK_MARKER {
            out.push((end - EARLY_BLOCK_SIZE, end));
        }
        end += EARLY_BLOCK_SIZE;
    }
    out
}

/// Vrai si la zone n'est que du remplissage (C3, FF ou 00).
fn is_filler(zone: &[u8]) -> bool {
    if zone.is_empty() {
        return true;
    }
    let filler = zone
        .iter()
        .filter(|&&b| b == 0xC3 || b == 0xFF || b == 0x00)
        .count();
    filler * 10 >= zone.len() * 9
}

/// Identifiant de codeblock lu dans la table du bloc signé : à `base + 0x1000`
/// la fin de table, à `base + 0x1002` l'offset (dans le bloc) du mot qui porte
/// l'identifiant. Même lecture que VerifyCodeBlocks/CheckCodeBlock d'EDCSuite ;
/// un identifiant hors 1..15 est rejeté (58882 lu sur un 019AJ avec l'ancien
/// modèle qui visait 0x60000, en plein milieu d'un bloc).
fn read_metadata_id(data: &[u8], base: usize) -> Option<u32> {
    if base + 0x1004 > data.len() {
        return None;
    }
    let end_of_table = u16::from_le_bytes([data[base + 0x1000], data[base + 0x1001]]);
    if end_of_table == 0xC3C3 {
        return None;
    }
    let id_offset = u16::from_le_bytes([data[base + 0x1002], data[base + 0x1003]]) as usize;
    if id_offset >= SIGNED_BLOCK_SIZE || base + id_offset + 2 > data.len() {
        return None;
    }
    let id = u16::from_le_bytes([data[base + id_offset], data[base + id_offset + 1]]) as u32;
    if (1..=15).contains(&id) {
        Some(id)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_signature(data: &mut [u8], pos: usize) {
        data[pos..pos + V41_SIGNATURE.len()].copy_from_slice(&V41_SIGNATURE);
    }

    #[test]
    fn standard_layout_two_blocks() {
        let mut data = vec![0x11u8; 0x80000];
        with_signature(&mut data, 0x50001);
        with_signature(&mut data, 0x70001);
        // métadonnées illisibles → identifiants EDCSuite par position
        for base in [0x50000usize, 0x70000] {
            data[base + 0x1000] = 0xC3;
            data[base + 0x1001] = 0xC3;
        }
        let layout = Edc15pLayout::detect(&data);
        assert_eq!(layout.generation, Edc15pGeneration::Standard);
        let ranges: Vec<(u32, u32, u32)> =
            layout.blocks.iter().map(|b| (b.id, b.start, b.end)).collect();
        assert_eq!(ranges, vec![(2, 0x4C000, 0x5C000), (5, 0x6C000, 0x7C000)]);
        assert_eq!(layout.block_id(0x4D000), Some(2));
        assert_eq!(layout.block_id(0x41000), Some(2));
        assert_eq!(layout.block_id(0x7D000), Some(5));
    }

    #[test]
    fn compact_layout_three_blocks() {
        let mut data = vec![0x11u8; 0x80000];
        for b in [0x54000usize..0x58000] {
            for x in &mut data[b] {
                *x = 0xC3;
            }
        }
        with_signature(&mut data, 0x58001);
        with_signature(&mut data, 0x64001);
        with_signature(&mut data, 0x70001);
        for (base, id) in [(0x58000usize, 2u8), (0x64000, 3), (0x70000, 1)] {
            data[base + 0x1000] = 0x00;
            data[base + 0x1001] = 0x20;
            data[base + 0x1002] = 0x50;
            data[base + 0x1003] = 0x7A;
            data[base + 0x7A50] = id;
            data[base + 0x7A51] = 0;
        }
        let layout = Edc15pLayout::detect(&data);
        assert_eq!(layout.generation, Edc15pGeneration::Compact);
        let ranges: Vec<(u32, u32, u32)> =
            layout.blocks.iter().map(|b| (b.id, b.start, b.end)).collect();
        assert_eq!(
            ranges,
            vec![(2, 0x58000, 0x64000), (3, 0x64000, 0x70000), (1, 0x70000, 0x7C000)]
        );
    }

    #[test]
    fn early_layout_marker_blocks() {
        let mut data = vec![0x11u8; 0x80000];
        for end in [0x8000usize, 0x10000, 0x18000, 0x80000] {
            data[end - 8..end - 4].copy_from_slice(&EARLY_BLOCK_MARKER);
        }
        let layout = Edc15pLayout::detect(&data);
        assert_eq!(layout.generation, Edc15pGeneration::Early);
        assert_eq!(layout.blocks.len(), 4);
        assert_eq!(layout.scan_start(), 0);
        assert_eq!(layout.block_id(0x79000), Some(4));
        assert_eq!(layout.block_id(0x30000), None);
    }
}
