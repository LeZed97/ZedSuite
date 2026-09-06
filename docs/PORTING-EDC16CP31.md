# Portage EDC16CP31 (Mercedes-Benz OM642 / OM646)

Squelette de portage pour ajouter la famille Bosch EDC16CP31 au moteur de
détection de ZedSuite. **Rien ici n'est calibré** : le module compile, passe
ses tests, se câble dans l'app, et retourne volontairement **zéro map** tant
que tu n'as pas renseigné les données issues d'un corpus réel.

---

## Ce que le squelette fait déjà

| Élément | État |
|---|---|
| Module `detector/ecu/bosch/edc16cp31/` | créé, compile, 6 tests verts |
| Moteur de scan axis-first générique | **implémenté et fonctionnel** |
| Scoring (plage physique + monotonie) | implémenté |
| Validation d'axes (RPM, IQ, pédale, rail, MAF, couple, pression atmo) | implémentée |
| Lecture big-endian MPC5xx | implémentée |
| Identification Mercedes dans `ecu_identifier.rs` | implémentée (preuves ASCII) |
| Routage `smart_detector.rs` / `parse_ecu_type` | câblé |
| Entrée `ecus.json` | ajoutée, `enabled: false` |
| Checksum EDC16 | **hérité gratuitement**, à valider |
| Endianness frontend | **héritée gratuitement** |
| Zone de calibration | **CONFIRMÉE sur dump réel** |
| Checksum sur fichier CP31 réel | **VALIDÉ** |
| Base de signatures | **vide — à construire** |
| Zones par famille de maps | **vides — à construire** |
| Facteurs raw→physique | **hypothèses — à valider** |

### Les deux cadeaux

`src/lib/ecu/bosch/checksums/index.ts` route sur `ecuType.includes('EDC16')`,
et `src/lib/ecu-endianness.ts` sur `includes("EDC16")`. Le nom `EDC16CP31`
matche les deux : **la correction de checksum et le décodage big-endian
fonctionnent sans une ligne de code en plus.**

Le module checksum ne dépend d'aucune adresse VAG : il cherche le descripteur
Bosch `FA DE CA FE CA FE AF FE` à `region_start + 0x3C`, lit les bornes dans
les deux dwords précédents et absorbe l'écart dans le dernier dword pour que
la somme retombe sur `0xD01FE500`. C'est le mécanisme EDC16 générique.

> **VALIDÉ sur un dump CP31 réel.** Descripteur unique trouvé à `0x19003C`,
> aligné sur 0x100, déclarant la région `0x190000..0x1FCFFB` (446 460 octets,
> 111 615 dwords). La somme des dwords big-endian de la région vaut
> **exactement `0xD01FE500`**, mot de checksum inclus (`0x0375D4D9` à
> `0x1FCFF8`). Le module lit ce fichier sans aucune modification.
>
> Reste à valider la **correction** (et pas seulement la vérification) sur une
> paire stock / modifié : édite une map, corrige, et vérifie que la somme
> retombe sur la constante.

---

## Ce qu'il reste à faire, dans l'ordre

### Étape 1 — Constituer le corpus (c'est 80 % du travail)

Il te faut **10 à 20 dumps CP31 dont tu connais déjà l'emplacement des maps** :
mappack, damos, A2L, ou des maps que tu as localisées à la main et vérifiées.
Sans vérité terrain tu ne peux ni construire les signatures ni savoir si ton
détecteur ment.

C'est exactement ce qu'a fait l'auteur pour le VAG — l'en-tête de
`edc16u31/signatures.rs` dit : *« Generated from analysis of 20 mappack files »*.

Pour chaque map connue, note : taille de grille, adresse de la map, adresse
des deux axes, facteur raw→physique, unité, motif d'octets qui précède le
bloc, fenêtre d'adresses.

### Étape 2 — Taille de dump et zone de calibration — ✅ FAIT

Confirmé sur un dump OM642 3.0 CDI 165 kW, SW `1037393817`, lu au KESS V2 :

| Fait | Valeur |
|---|---|
| Taille | exactement 2 Mo (`0x200000`), déjà dans `SUPPORTED_SIZES` |
| Zone utile | `0x190000` → `0x1FFFFF`. **Tout ce qui est en dessous de `0x190000` est `0xFF`** |
| Région checksummée | `0x190000`..`0x1FCFFB` (descripteur à `0x19003C`) |
| SW Bosch | `1037393817` à `0x190010` (offset fixe) |
| Chaîne famille | `99/1/EDC16CP31/001/B209/X/080000_000/...` à `0x1906FF` |
| Descripteur moteur | `CR4-642-42P7-209CM-165kW-PT2R05-LR-3907x064ME` à `0x1D751C`, `3.0l` à `0x1D759A` |

Le KESS ne ramène que la zone de calibration et remplit le reste en `0xFF` :
`CP31_ZONES.calibration` est donc passée de l'hypothèse VAG `(0x180000,
0x200000)` à `(0x190000, 0x1FD000)`. Un jour où tu feras une lecture full
flash en bench/boot, la moitié basse sera peuplée — élargis la fenêtre à ce
moment-là, le test `confirmed_cp31_layout_constants` te le rappellera.

L'identification exploite maintenant deux marqueurs confirmés : le SW `1037`
**à l'offset fixe** `0x190010` (positionnel, pas une recherche de sous-chaîne)
et le descripteur moteur `CR<n>-<groupe>-`. Confiance 0.95 quand la chaîne
famille et un marqueur structurel sont tous deux présents.

### Étape 3 — Calibrer les templates un par un

Dans `MAP_TEMPLATES` (fichier `edc16cp31/mod.rs`), chaque entrée porte
`calibrated: false`. Pour une map donnée :

1. Lance le détecteur en mode exploratoire :
   `EDC16CP31Detector::exploratory()` — il ignore le garde-fou `calibrated`.
2. Compare ce qu'il propose avec l'adresse que tu connais.
3. Ajuste `grids`, `z_factor`, `z_offset`, `z_range_stock` / `z_range_tuned`,
   et le type d'axe jusqu'à ce que la map tombe juste sur tout le corpus.
4. **Alors seulement** passe cette entrée à `calibrated: true`.

Les valeurs actuelles sont des points de départ extrapolés de la forme des
templates VAG et de ce qu'un V6 CR fait plausiblement. Ne les prends pas pour
des valeurs CP31 : un facteur faux ne plante pas, il mets juste ta map à la
mauvaise échelle — c'est le mode de défaillance le plus dangereux du moteur.

### Étape 4 — Renseigner les signatures

`CP31_SIGNATURES` et `CP31_MARKERS` sont vides. Une signature ne se rajoute
que quand elle a matché **tous** les fichiers du corpus et produit **zéro**
hit sur des fichiers d'autres familles de même taille.

Les marqueurs VAG (`[8A 00 00 06]`, `[00 00 00 3C 00 64]`) sont des marqueurs
de softs VAG. Les recopier produirait des résultats faux mais confiants.

Puis implémente `detect_by_signatures()` — la référence est
`detect_n75_by_signature()` dans `edc16u34/mod.rs`.

### Étape 5 — Renforcer l'identification

`identify_mercedes_edc16()` s'appuie aujourd'hui sur des preuves ASCII :
chaîne `EDC16CP31` explicite, ou référence Daimler (`A` + 10 chiffres,
groupes 642/646/628/611/629) combinée aux caractéristiques EDC16, avec veto
si une référence VAG traîne dans le fichier. Confiance 0.65 à 0.90.

C'est plus faible que ce dont disposent les variantes VAG (signatures
structurelles à offset fixe). Dès que tu as des dumps, ajoute une signature
structurelle et monte la confiance.

**Ajoute le test négatif obligatoire** (règle du `CONTRIBUTING.md`) : un
fichier quelconque de 2 Mo ne doit PAS être identifié comme CP31. Modèle :
`test_foreign_2mb_file_is_not_edc16`.

### Étape 6 — Activer côté frontend

Décommente `"EDC16CP31"` dans `SUPPORTED_ECUS` (`src/lib/local/detector.ts`)
et passe `enabled: true` dans `src-tauri/ecus.json`. **Uniquement après**
qu'au moins un template soit calibré et validé sur des fichiers réels.

---

## Performance

Le scan axis-first en mode exploratoire parcourt toute la moitié calibration
d'un 2 Mo pour chaque grille et chaque ordre d'axes. Sur un fichier de test
rempli de zéros, la passe complète prend ~30 s.

Dès que tu connais les fenêtres d'adresses par famille, renseigne
`CP31_ZONES.boost`, `.rail_pressure`, etc. : le détecteur les utilise
automatiquement via `zone_for()` et tombe à quelques centaines de ms — c'est
d'ailleurs pour ça que les modules VAG codent leurs zones en dur (84 adresses
distinctes rien que dans `edc16u31/mod.rs`).

---

## Garde-fou à ne pas retirer

`detect()` retourne un vecteur vide tant qu'aucun template n'est calibré et
qu'aucune signature n'est renseignée. Il ne retombe **jamais** sur les
détecteurs VAG. C'est la règle du projet :

> *« A false "unsupported file" is annoying; a false "this is an EDC16"
> corrupts someone's ECU. »* — `CONTRIBUTING.md`

Le test `uncalibrated_detector_reports_no_maps` verrouille ce contrat.

---

## Licence et amont

Le projet est en **GPL-3.0** : un fork distribué reste GPL. Le nom
« ZedSuite » est exclu de la licence (`TRADEMARKS.md`) — il faut renommer.

Le dépôt amont n'a que 2 commits (dumps de code écrasés). Si l'auteur continue
comme ça, chaque mise à jour amont donnera des conflits massifs sur des
fichiers de 9 000 lignes. Deux options saines : soit tu diverges pour de bon,
soit tu proposes la famille CP31 en PR upstream — `CONTRIBUTING.md` dit
explicitement que le support de nouvelles familles d'ECU est la contribution
la plus utile.
