# ZedSuite

[English](README.md) · **Français**

![Platform](https://img.shields.io/badge/plateforme-Windows%20%7C%20macOS-0078d4) ![Engine](https://img.shields.io/badge/moteur%20de%20d%C3%A9tection-Rust-e6522c) ![License](https://img.shields.io/badge/licence-GPL--3.0-2ea44f) ![Downloads](https://img.shields.io/github/downloads/LeZed97/ZedSuite/total)

**Éditeur de cartographies open source pour les ECU Bosch EDC15/EDC16 du groupe VAG — 100 % en local, sur Windows et macOS.**

Ouvrez un dump d'ECU et ZedSuite trouve les cartographies tout seul : Driver Wish, Turbo Boost, N75, SOI, limiteurs de couple, etc. Modifiez-les en tableau, en 2D, en 3D ou directement dans l'hexadécimal. Créez des versions, comparez-les, désactivez ou réactivez des DTC, corrigez le checksum, puis exportez votre binaire ou un mappack WinOLS.

Pas de compte, pas de cloud, pas de limites : tout est en local, vos fichiers ne quittent jamais votre ordinateur.

![Éditeur ZedSuite](docs/screenshot.png)

## 🚗 Calculateurs supportés

| ECU | Détection |
|-----|-----------|
| Bosch EDC15P | patterns + codeblocks |
| Bosch EDC15VM+ | patterns + codeblocks |
| Bosch EDC16U1 | signatures |
| Bosch EDC16U31 | signatures |
| Bosch EDC16U34 | signatures |

L'identification est volontairement stricte : un fichier n'est ouvert comme l'un de ces calculateurs que si on y trouve des preuves concrètes (numéros hardware Bosch, chaînes propres à la famille, signatures structurelles). Un dump de 2 Mo venant d'un autre calculateur (EDC17, Marelli, Siemens…) est refusé plutôt que d'être pris à tort pour un EDC16.

La détection n'est pas parfaite pour autant. Chaque famille a été calibrée sur un banc constitué de tous les fichiers que j'avais à disposition, mais je n'avais pas autant de dumps différents en EDC16U31 que pour les autres familles : sur certains fichiers U31, une partie des maps peut ne pas être détectée. Même chose côté EDC15VM : certaines maps peuvent ne pas apparaître, notamment sur les dumps de 1 Mo des moteurs V6, que je n'ai volontairement pas terminés car ça m'aurait pris encore trop de temps. Dans tous les cas, lorsque les maps sont complètement détectées sur EDC15/16, les mappacks sont d'une qualité imbattable par rapport à ce qui se fait sur le marché.

## ⚙️ Fonctionnalités

- **Détection automatique des cartographies** — moteur Rust embarqué, un détecteur par famille
- **Vérification de complétude** — un badge de confiance indique si toutes les maps attendues pour la famille d'ECU ont été trouvées, avec le détail de ce qui manque en un clic
- **Éditeur de maps** — tableau, graphe 2D et surface 3D, navigation clavier et copier/coller entre maps, modification absolue, additive ou en pourcentage, report vers les maps similaires, raccourcis façon WinOLS. Le graphe 2D est fait pour les maps à une ligne (courbes, linéarisations, valeurs uniques) ; les matrices complètes s'ouvrent en 3D, où un pic ou un plat se voit d'un coup d'œil, et dans le tableau pour les valeurs exactes
- **Éditeur hexadécimal** — virtualisé, minimap, modifications surlignées par rapport à l'origine
- **Versions** — « Ori » + versions nommées par projet, vue de comparaison
- **Stockage optimisé** — fichier d'origine + fichier de modifications par version, reconstruction automatique à l'export
- **Dyno virtuel** — estimation puissance/couple à partir des maps, rapport PDF imprimable
- **DTC on/off** — lecture de la table des codes défaut, désactivation et réactivation (EDC15 et EDC16)
- **Solutions** — patchs en un clic (launch control, …) ; volontairement limités pour ne pas repousser encore la sortie de l'application, d'autres pourront arriver plus tard
- **Correction de checksum** — famille EDC15 et EDC16, implémentée nativement
- **Marque pré-remplie** — base de références ECU embarquée (numéros Bosch/VAG)
- **Exports** — `.bin` modifié, mappack JSON compatible WinOLS 5
- **Mises à jour automatiques** — l'app vérifie une fois par jour sur GitHub si une nouvelle version existe ; installation en un clic
- **3 thèmes** — sombre, clair et OLED, pour tous les types d'écrans
- **Toutes les tailles d'écran** — menu des maps redimensionnable et zoom de l'éditeur façon navigateur, du portable à l'ultrawide
- **Cinq langues** — anglais, français, espagnol, italien et allemand, pour l'app et l'installateur ; en ajouter une autre est simple (un seul fichier de traductions), et le nom des maps n'est volontairement pas traduit — il reste en anglais

### Les maps que vous ne trouverez pas dans la liste

Deux tables que d'autres outils affichent sont volontairement laissées de côté sur EDC15P : inverse driver wish et MAF linearisation. Ce sont des tables de conversion, pas des maps de tuning, et les retirer garde une liste courte et lisible. Sur EDC15VM, un jeu de petites tables de correction d'avance est également masqué : elles apparaissaient comme une deuxième map d'avance à l'injection et induisaient en erreur, et l'outil de référence ne les liste pas non plus. Si l'une d'elles vous sert, dites-le dans une issue : quelques demandes et elle revient.

Si une map que vous attendez est vraiment absente, le badge de complétude vous le dit : il liste ce que la famille d'ECU doit contenir et ce qui n'a pas été trouvé.

## 🔧 Travailler avec des fichiers modifiés

Le moteur de détection s'appuie volontairement sur **la structure des fichiers, pas sur leurs valeurs** : il s'accroche aux headers, à la disposition des axes et à des signatures qui survivent à une reprogrammation. Les fichiers stage 1/2/3 sont donc bien détectés dans l'immense majorité des cas, énormément de travail au banc est passé là-dedans.

Malgré ça, un **fichier extrêmement modifié** (axes réécrits, blocs déplacés, protections patchées) peut encore cacher quelques maps au scanner. Le bon réflexe :

1. Créez le projet à partir du **fichier d'origine** : c'est là que la détection trouve toutes les maps.
2. Importez ensuite le fichier modifié comme **nouvelle version** de ce projet.

Toutes les versions d'un projet partagent la liste de maps détectée sur l'origine : vous retrouvez donc le jeu complet de maps sur le fichier préparé, avec en bonus la comparaison entre versions.

## 🏗️ Comment le projet a été fait

ZedSuite a d'abord été un SaaS web : un éditeur Next.js qui parlait à un microservice de détection écrit en Rust. Cette version devait aller beaucoup plus loin : l'objectif était de sortir avec tous les diesels VAG pris en charge, jusqu'aux MD1, plus les anciens EDC15/16/17 des autres marques. Mais je n'avais plus le temps de finir ce projet car je devais me concentrer sur autre chose, alors j'ai préféré publier ce qui était vraiment solide : le périmètre VAG EDC15/EDC16, terminé proprement, en application de bureau 100 % locale et open source.

Côté conception, j'ai essayé de mélanger les fonctionnalités les plus intéressantes des deux outils avec lesquels j'ai passé des années : la détection automatique et la simplicité d'EDCSuite, et le confort d'édition de WinOLS.

Techniquement, le moteur Rust a été intégré à l'application Tauri sous forme de commandes IPC, et le frontend parle toujours à l'ancienne interface `/api/*`, que `src/lib/local/api.ts` réimplémente au-dessus du stockage sur disque. L'éditeur est donc resté identique à la version web éprouvée, sans plus aucune dépendance à un serveur.

Le moteur de détection, lui, est le fruit de longues sessions de reverse engineering sur des dumps réels. Chaque famille d'ECU a son propre détecteur, construit toujours de la même façon : localiser les maps à l'aide de références type WinOLS/damos, en extraire les signatures structurelles qui permettent de les reconnaître (headers de dimensions, axes, blocs sélecteurs, espacement entre les maps), puis valider sur un banc de fichiers réels (d'origine ET modifiés) jusqu'à ce que le résultat colle aux listes de référence, map par map. Les détecteurs s'appuient sur la structure et non sur les valeurs, justement pour que les fichiers préparés restent détectés. Les checksums suivent la même logique : algorithmes EDC15/EDC16 réimplémentés nativement et validés à l'octet près sur des paires avant/après.

Le choix de Rust pour le moteur de détection n'est pas un hasard. À la base le projet était un SaaS qui devait être hébergé : au-delà d'être beaucoup plus performant que le C# d'EDCSuite (pas de runtime .NET ni de garbage collector, du code machine natif dans un binaire léger et autonome), j'ai cherché à tout optimiser au maximum pour une version web où chaque détection tournait côté serveur. Résultat, la détection complète prend moins d'une seconde quel que soit le fichier : environ 0,1 s pour un EDC15VM (512 Ko), 0,3 à 0,5 s pour un EDC16 (1 à 2 Mo) et autour d'une seconde pour un EDC15P, le balayage le plus lourd. Face au C/C++ : la même vitesse, mais un compilateur beaucoup plus strict qui attrape dès la compilation les erreurs qui font planter un outil sur un fichier inattendu. Et comme Tauri est lui-même en Rust, le moteur qui tournait hier sur un serveur tourne aujourd'hui embarqué dans l'app, sans une ligne de changée.

L'interface elle-même est une page web : HTML, CSS et TypeScript, affichés par le moteur de navigateur déjà présent sur le système (WebView2 sur Windows, WebKit sur macOS). Autour, la coque Rust ouvre la fenêtre, lit et écrit les fichiers, et fait tourner le moteur de détection, compilé nativement pour chaque plateforme.

C'est pour ça que la version macOS n'a rien demandé de réécrire. Le dashboard, l'éditeur de maps, les vues 2D et 3D, l'hexadécimal, les outils DTC et puissance sont le même code sur les deux systèmes, au pixel près. Seule la coque sait où elle tourne : barre de titre, boîtes de dialogue de fichiers, emplacement des projets, installation des mises à jour.

La pile web apporte plus que la portabilité. Aucun navigateur n'est embarqué : l'installateur Windows pèse environ 6 Mo et l'app démarre en une seconde. Les graphiques, les thèmes et les cinq langues sont faits avec des outils prévus pour ça. Et chaque correction de l'interface arrive sur Windows et macOS en même temps ; une version Linux ne demanderait que le côté coque.

## 🤝 Contribuer

Les contributions sont bienvenues : les **nouveaux détecteurs d'ECU** sont ce qui a le plus de valeur. Voir [CONTRIBUTING.md](CONTRIBUTING.md) pour l'architecture des détecteurs, la façon dont les familles existantes ont été construites et le niveau exigé d'une nouvelle famille avant sa sortie (corpus, banc, invariants, zéro faux positif). Un bug, une map non détectée ? Ouvrez une issue avec le type d'ECU et le numéro logiciel du fichier, et joignez le dump si vous le pouvez : c'est ce qui permet de corriger, la plupart des corrections de détection publiées jusqu'ici viennent d'un fichier envoyé par un utilisateur. Les fichiers servent uniquement à corriger le détecteur et ne sont jamais partagés.

## 🙏 Remerciements

- **Dilemma**, qui a publié [VAGEDCSuite](https://github.com/Blackfrosch/VAGEDCSuite) il y a environ 14 ans. C'est avec ce logiciel que j'ai pu pratiquer et apprendre facilement : la reconnaissance automatique des maps, la simplicité de l'outil… un travail énorme pour un logiciel né dans les années 2000 ! (Ce mec doit être un alien) Une grande partie de la logique de détection des EDC15 de ZedSuite est d'ailleurs directement héritée du travail fait dans EDCSuite.
- **Skalda**, qui a [fait vivre VAGEDCSuite](https://github.com/skaldamramra/VAGEDCSuite) en mettant à jour la détection et en ajoutant beaucoup de maps EDC15. Ma propre version d'EDCSuite est partie de la sienne, et c'est elle que j'ai utilisée au quotidien en attendant d'avoir le temps de faire ZedSuite.

### Contributeurs

Toutes les personnes dont du code, un signalement ou un fichier a fait évoluer l'app, avec ce que ça a changé et la version où c'est arrivé : [CONTRIBUTORS.md](CONTRIBUTORS.md). Joindre un dump à un signalement, c'est ce qui rend une correction possible ; les fichiers servent uniquement à corriger le détecteur et ne sont jamais partagés.

## ⬇️ Téléchargement

Tout est dans la section **Assets** de la [dernière release](https://github.com/LeZed97/ZedSuite/releases/latest). Une fois installée, l'application se met à jour toute seule, sur les deux systèmes.

**Windows** — téléchargez `ZedSuite_x.y.z_x64-setup.exe` et lancez-le (sur un Windows 32 bits, prenez `ZedSuite_x.y.z_x86-setup.exe`). ZedSuite demande **Windows 10 ou 11** : l'adapter à Windows 7 aurait demandé encore beaucoup de travail.

**macOS** — téléchargez `ZedSuite_x.y.z_macos-universal.dmg`, ouvrez-le et glissez ZedSuite dans le dossier Applications. Une seule build pour les Mac Apple Silicon et Intel, **macOS 12 ou plus récent**. ZedSuite n'est pas signée avec un certificat développeur Apple, le premier lancement demande donc une étape de plus : macOS refuse de l'ouvrir, puis **Réglages Système > Confidentialité et sécurité > Ouvrir quand même** (sur macOS 14 et antérieurs, clic droit sur l'app > Ouvrir). C'est une seule fois ; les mises à jour installées par l'app elle-même s'ouvrent directement. Si vous préférez le Terminal, cette ligne installe ou met à jour ZedSuite dans Applications sans aucune étape supplémentaire :

```bash
curl -fsSL https://raw.githubusercontent.com/LeZed97/ZedSuite/master/install-macos.sh | sh
```

**Linux** — pas encore de version. L'interface est le même code que sur Windows et macOS, seule la partie coque serait à faire. C'est sur la feuille de route comme demande d'utilisateurs : plus il y aura de demandes, plus vite ça arrivera.

## 🗺️ Feuille de route

Ce qui est en cours, ce qui est prévu et ce que les utilisateurs ont demandé : [ROADMAP.fr.md](ROADMAP.fr.md) (aussi en [anglais](ROADMAP.md), [espagnol](ROADMAP.es.md), [italien](ROADMAP.it.md) et [allemand](ROADMAP.de.md)). La même page s'ouvre dans l'application, dans la langue de l'app, depuis le dashboard (bouton feuille de route, à côté de l'aide).

## 📫 Contact

- 🌐 Site — [zedperf.com](https://zedperf.com)
- 📸 Instagram — [@zedperf](https://instagram.com/zedperf)
- ▶️ YouTube — [@ZedPerf](https://www.youtube.com/@ZedPerf)
- 👥 Facebook — [zedperf](https://www.facebook.com/zedperf.1/)
- 🔗 Tout au même endroit : [linktr.ee/zedperf](https://linktr.ee/zedperf)

## ☕ Buy me a coffee

ZedSuite est gratuit et le restera. S'il vous a fait gagner du temps ou une licence WinOLS, vous pouvez financer les prochaines sessions de reverse engineering :

- **PayPal** : [paypal.me/zedperf](https://www.paypal.com/paypalme/zedperf)
- **BTC** (Bitcoin) : `bc1qj2e42vpphx73xguspqd9c6uqrs9ra0yywcq97a`
- **SOL / USDC** (Solana) : `AqjSzxi7pBkwcCVkyVxBVLTk9TgPmui71bNgVgNLWrJC`
- **TRX** (Tron) : `TRDgrasP7yaEKcz54r8spbmgZdRBFpNerW`

## 🚀 Démarrer (développement)

Prérequis :
- [Node.js](https://nodejs.org) ≥ 18
- [Rust](https://rustup.rs) (stable) — le moteur de détection et l'application sont en Rust/Tauri
- Windows 10/11 (WebView2 est préinstallé sur Windows 11) ou macOS 12+ avec les outils en ligne de commande Xcode (`xcode-select --install`)

```bash
npm install
npm run app:dev     # lance l'app avec rechargement à chaud
```

Construire l'installeur :

```bash
npm run app:build   # produit l'installeur NSIS dans src-tauri/target/release/bundle/
```

Sur macOS, `npx tauri build --target universal-apple-darwin` produit le `.app` et le `.dmg` pour les deux architectures (`src-tauri/tauri.macos.conf.json` porte les réglages propres à macOS). Le [workflow macOS](.github/workflows/macos.yml) fait la même chose sur un runner GitHub, vérifie le bundle, y exerce la mise à jour et attache les fichiers à une release.

## 🧱 Architecture

```
src/                  Frontend Next.js (export statique, servi par la webview Tauri)
  app/dashboard/      liste des projets (écran d'accueil)
  app/editor/         l'éditeur de maps
  lib/local/          backend local : stockage disque des projets + pont API
  lib/ecu/            helpers ECU TypeScript (listes DTC, checksums)
src-tauri/            Application Rust
  src/detector/       le moteur de détection (un dossier par fabricant)
  src/commands.rs     commandes IPC exposées au frontend
```

Les projets sont stockés dans `%APPDATA%/com.zedsuite.app/projects/` sur Windows et `~/Library/Application Support/com.zedperf.zedsuite/projects/` sur macOS — un dossier par projet, avec le binaire d'origine, les métadonnées et les versions.

## ⚖️ Licence et marques

[GPL-3.0](LICENSE) — vous êtes libres d'utiliser, d'étudier, de modifier et de redistribuer ZedSuite, mais tout travail dérivé doit être publié sous la même licence. Si vous améliorez le moteur de détection ou ajoutez le support d'un calculateur, la communauté en profite.

**La licence couvre le code uniquement.** Le nom ZedSuite, le logo et la mascotte sont des marques de ZedPerf, explicitement exclues de la licence GPL (GPL-3.0 §7(e)) : les forks sont bienvenus, mais ils doivent être distribués sous leur propre nom et leur propre identité visuelle. Politique complète : [TRADEMARKS.md](TRADEMARKS.md). Les builds officiels sont publiés uniquement sur [la page releases de ce dépôt](https://github.com/LeZed97/ZedSuite/releases).

## ⚠️ Avertissement

ZedSuite est destiné à la recherche, à l'apprentissage et à un usage circuit/tout-terrain. Reprogrammer l'ECU d'un véhicule routier peut être illégal selon votre pays, annuler votre garantie, endommager votre moteur ou rendre le véhicule non conforme aux normes antipollution. Vous êtes seul responsable de l'usage que vous faites de ce logiciel.

**Un mot de sécurité** : les logiciels de tuning sont une cible privilégiée des hackers, qui se servent parfois des outils gratuits pour distribuer des malwares. Téléchargez toujours l'installateur depuis le [GitHub officiel](https://github.com/LeZed97/ZedSuite/releases) — c'est le seul moyen d'être sûr de n'avoir aucun problème.
