# Portage EDC16CP31 (Mercedes-Benz OM642 / OM646)

Support de la famille Bosch EDC16CP31 dans le moteur de détection de ZedSuite.

**État : calibré sur un corpus d'UN logiciel.** Onze familles de maps sont
confirmées, adresses vérifiées sur un dump réel, facteurs et sémantique d'axes
issus de la description constructeur du projet. Le module est activé
(`ecus.json` : `status: beta`, `enabled: true`), l'export mappack reste
désactivé tant que le corpus n'a qu'un fichier.

---

## Le fait qui change tout : le layout CP31 est auto-descriptif

Les détecteurs VAG cherchent un triplet `[axe][axe][données]` plausible et
**devinent** la grille dans une liste de tailles candidates. Sur CP31 c'est
inutile et strictement moins bon. Toutes les maps 2D de la famille utilisent
le record layout Bosch `Kf_Xs16_Ys16_Ws16` :

```
+0x00  u16        nx          nombre de points de l'axe X
+0x02  u16        ny          nombre de points de l'axe Y
+0x04  i16[nx]    axe X       strictement croissant
+....  i16[ny]    axe Y       strictement croissant
+....  i16[nx*ny] données Z   sens COLONNE : Y est l'index rapide, z[x*ny + y]
```

Les courbes 1D (`Kl_Xs16_Ws16`) : `[nx][X nx][Z nx]`.

Tout est big-endian (MPC5xx). Le bloc est **alloué à sa taille maximale
déclarée** : une map 12×12 dans une allocation 16×16 est suivie de padding,
qui ne fait pas partie de la map.

Conséquences concrètes :

* la grille se **lit**, elle ne se devine pas. Une mauvaise grille ne peut
  plus produire une map d'apparence plausible — le pire mode de défaillance du
  moteur axis-first disparaît ;
* le scan tombe de ~30 s à quelques ms ;
* les 4 octets d'en-tête `[nx][ny]` + la tête de l'axe X forment un
  **marqueur structurel** naturel, bien plus solide qu'un motif de code.

Comme Z est en sens colonne, la présentation naturelle est `rows = nx`
(régime) et `cols = ny` (charge) : l'ordre fichier **est** l'ordre d'affichage,
aucune transposition nulle part, et c'est aussi la présentation WinOLS.
`y_axis_address` pointe donc sur le **premier** axe du bloc (régime) et
`x_axis_address` sur le second (charge).

---

## Méthode : transférer un damos qui ne tombe pas en face

Le damos disponible (ASAP2 `CR4-642`, PROJECT `B209`, VERSION `V080000`) décrit
**le même projet** que le dump mais **un autre build de calibration**
(SW `0123456789P03_1000` contre `1037393817`). Résultat : 55 % des octets de la
zone de calibration diffèrent, et **aucune** adresse A2L ne tombe en face — il
n'y a même pas de décalage constant (test : 1 map sur 723 valide structurellement
à son adresse A2L ; le meilleur décalage constant n'en récupère que 39).

La méthode qui marche, en quatre temps :

1. **Reconstruire l'image de référence** depuis le `.hex` du damos, et lire
   chaque `CHARACTERISTIC` à son adresse A2L → 392 maps 2D exploitables avec
   leur `nx`, `ny`, leurs deux vecteurs d'axes et leurs valeurs.
2. **Scanner le dump réel** pour tous les blocs `Kf` valides (en-tête plausible
   + deux axes strictement croissants) → 482 blocs candidats.
3. **Apparier par les axes.** Les vecteurs de points de rupture changent
   beaucoup moins que les valeurs entre deux builds. Clé exacte
   `(nx, ny, axe X, axe Y)` : 122 appariements uniques, 180 ambigus, 90 sans
   correspondance. Les ambiguïtés se lèvent avec une **plus longue
   sous-séquence croissante** sur l'ordre des adresses — l'ordre des blocs est
   conservé d'un build à l'autre. → 238 maps ancrées.
4. **Combler par delta.** Les écarts damos→dump sont **constants par
   morceaux** sur de longues plages (−460, −456, −356, −152, +728, +5136…).
   Pour chaque map non appariée, essayer les deltas des ancres voisines et
   valider le bloc structurellement **et** contre les limites physiques
   déclarées dans l'A2L. → 374 maps sur 392.

Chaque map retenue a ensuite été relue dans le dump et vérifiée à la main :
valeurs physiques cohérentes pour un OM642 165 kW, monotonie, unités.

> Le damos et le `.bin` ne sont **pas** dans le dépôt : un damos est
> généralement sous licence, et un dump n'est pas à nous. Seules les données
> de signature dérivées sont versionnées.

---

## Les onze familles confirmées

Adresses du **bloc** (mot `nx`) sur le dump de référence, OM642 3.0 CDI 165 kW,
SW Bosch `1037393817` :

| Famille | Label damos | Bloc | Grille (nx×ny) | Facteur | Unité | Plage stock |
|---|---|---|---|---|---|---|
| Rail Pressure Target | `Rail_pSetPointBase_MAP` | `0x1F1BF2` | 16×16 | 0.1 | bar | 250 – 1632 |
| Rail Pressure Limiter | `Rail_pSetPointLimN_MAP` | `0x1F2964` | 8×12 | 0.1 | bar | 900 – 1632 |
| Boost Target | `PCR_pDesBas_MAP` | `0x1E4BCE` | 16×12 | 1.0 | hPa abs | 1010 – 2570 |
| Boost Limiter | `PCR_pBDesMaxAPGear{Hi,Lo}_MAP` | `0x1E3E36` / `0x1E407A` | 16×12 | 1.0 | hPa | 0 – 2760 |
| VNT Duty Cycle | `PCR_rCtlBas_MAP` | `0x1DB4F0` | 16×8 | 1/8192 | % | 23.5 – 88.0 |
| Smoke Limiter | `FlMng_qSmk_MAP` | `0x1A8986` | 16×16 | 0.01 | mm³/cyc | 24 – 86 |
| Driver Wish | `AccPed_trqEng_MAP` / `…2_MAP` | `0x1918DE` / `0x19169A` | 8×8 | 0.1 | Nm | 0 – 500 / 0 – 600 |
| Quantity Limiter | `FlMng_qLimBstPres_MAP` | `0x1A7FE8` | 12×12 | 0.01 | mm³/cyc | 0 – 100 |
| Torque to IQ | `FMTC_trq2qBas_MAP` | `0x1A9514` | 16×18 | 0.01 | mm³/cyc | 0 – 88 |
| Start of injection | `InjCrv_phiMI1Bas1..3` / `Max1..2` | `0x1B07EC` … | 16×16 | 3/128 | degCrS | −10.6 – +27.6 |
| EGR air mass target | `AirCtl_mDesBas_MAP` | `0x194442` | 12×16 | 0.1 | mg/cyl | 240 – 1060 |

Deux corrections importantes par rapport aux hypothèses extrapolées du VAG :

* **le limiteur de fumée CR4 n'est pas fonction du débit d'air.** `FlMng_qSmk_MAP`
  a pour axe Y la **pression de suralimentation corrigée** (`PCR_pBPSCor`,
  700–2500 hPa), pas la MAF. Le template porte le nom correct.
* **la pédale et les rapports cycliques sont en 1/8192, pas en 0.01 %/bit.**
  8192 = 100 %. C'est la conversion `Prc` du projet. `AxisType::Pedal` a été
  remplacé par `AxisType::Percent` avec le bon facteur — l'ancienne valeur VAG
  aurait mis toutes les cartes de pédale à l'échelle × 1.22.

Sont aussi apparus, gratuitement, des membres légitimes des mêmes familles :
`Rail_pSetPointMin`, `FlMng_qLimT3BPS`, `AirCtl_mDesValCor`, `AirCtl_mNSCor`,
`PCR_rCtlAtmPres`, `InjCrv_phiMI1ETSAddCor`, `InjCrv_phiMI1NSCor`. Le dump de
référence donne **25 maps** au total, toutes réelles, aucun faux positif.

---

## Zone de calibration et fenêtres — confirmées

| Fait | Valeur |
|---|---|
| Taille | exactement 2 Mo (`0x200000`) |
| Zone utile | `0x190000` → `0x1FFFFF`. **Tout en dessous de `0x190000` est `0xFF`** |
| Région checksummée | `0x190000`..`0x1FCFFB` (descripteur à `0x19003C`) |
| Segment DATA de l'A2L | `Dst190000`, `0x190000` + `0x6CF74` → cohérent au bit près |
| SW Bosch | `1037393817` à `0x190010` (offset fixe) |
| Chaîne famille | `99/1/EDC16CP31/001/B209/X/080000_000/…` à `0x1906FF` |
| Descripteur moteur | `CR4-642-42P7-209CM-165kW-PT2R05-LR-3907x064ME` à `0x1D751C` |

`CP31_ZONES.calibration = (0x190000, 0x1FD000)`. Les fenêtres par catégorie
(`.boost`, `.rail_pressure`, `.injection`, `.torque`, `.smoke`, `.egr`) sont
renseignées, et chaque template porte en plus sa propre fenêtre serrée
(`MapTemplate::zone`), plus étroite que la fenêtre catégorie.

Le KESS ne ramène que la zone de calibration et remplit le reste en `0xFF`.
Le jour d'une lecture full flash en bench/boot, la moitié basse sera peuplée —
le test `confirmed_cp31_layout_constants` te rappellera d'élargir la fenêtre.

---

## Checksum

`src/lib/ecu/bosch/checksums/index.ts` route sur `ecuType.includes('EDC16')`
et `src/lib/ecu-endianness.ts` sur `includes("EDC16")` : le nom `EDC16CP31`
matche les deux, **rien à écrire**.

Le module cherche le descripteur Bosch `FA DE CA FE CA FE AF FE` à
`region_start + 0x3C`, lit les bornes dans les deux dwords précédents et
absorbe l'écart dans le dernier dword.

> **Vérification validée** sur le dump CP31 réel : descripteur unique à
> `0x19003C`, région `0x190000..0x1FCFFB` (446 460 octets, 111 615 dwords),
> somme des dwords big-endian = **exactement `0xD01FE500`**, mot de checksum
> `0x0375D4D9` à `0x1FCFF8`.
>
> **Reste à valider la correction** (et pas seulement la vérification) sur une
> paire stock / modifié : édite une map, corrige, vérifie que la somme retombe
> sur la constante.

---

## Tests

`cargo test --lib edc16cp31` — 14 tests, sans aucun fichier binaire :

* lecture d'un bloc `Kf` synthétique, y compris l'ordre colonne ;
* fichier à `0x00` et fichier à `0xFF` → zéro map ;
* validation d'axes (rejet des compteurs, bornes) ;
* cohérence des zones et des plages des templates ;
* **tous les marqueurs sont câblés** à un template et leur en-tête `[nx][ny]`
  est une grille plausible ;
* détection bout-en-bout d'un Boost Target planté à l'adresse confirmée ;
* **rejet** du même bloc avec une valeur hors plage physique ;
* **rejet** du même bloc planté hors de sa fenêtre.

Le test de fixture sur dump réel est séparé et `#[ignore]` (aucun binaire dans
le dépôt) :

```sh
ZEDSUITE_CP31_DUMP=/chemin/vers/original.bin \
  cargo test --test edc16cp31_dump -- --ignored --nocapture
```

Il vérifie les onze familles à leur adresse de bloc, l'adresse des deux axes
dans l'ordre CP31, la taille des données, et qu'aucune map ne déborde de la
région checksummée.

---

## Ce qu'il reste à faire

### 1. Un deuxième logiciel dans le corpus

C'est le seul vrai manque. Aujourd'hui `occurrence_rate: 1.0` veut dire
« vu dans 1 fichier sur 1 », pas une statistique. Avec un deuxième build :

* les fenêtres d'adresses se vérifient (elles sont volontairement larges) ;
* les marqueurs se confirment ou se raffinent ;
* le test de fixture échouera sur les adresses tout en trouvant les mêmes
  familles — c'est exactement le signal recherché.

### 2. OM646

Le 4 cylindres n'a **pas** été regardé. Ses zones seront différentes. Ne pas
supposer que ça marche parce que c'est aussi du CP31.

### 3. Valider la correction de checksum sur une paire stock / modifié

Voir plus haut.

### 4. Les courbes 1D

Seules les maps 2D (`Kf`) sont détectées. Le dump contient ~1685 blocs `Kl`
candidats : limiteur de régime, limiteurs de couple par rapport, courbes de
correction. Le lecteur `Kl` est trivial à ajouter (`[nx][X][Z]`), le travail
est de nouveau l'identification.

---

## Garde-fou à ne pas retirer

`detect()` retourne un vecteur vide tant qu'aucun template n'est calibré et
qu'aucune signature n'est renseignée. Il ne retombe **jamais** sur les
détecteurs VAG. C'est la règle du projet :

> *« A false "unsupported file" is annoying; a false "this is an EDC16"
> corrupts someone's ECU. »* — `CONTRIBUTING.md`

Un hit de marqueur n'est jamais suffisant à lui seul : le bloc est décodé,
ses deux axes validés et ses données bornées avant toute émission.

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
