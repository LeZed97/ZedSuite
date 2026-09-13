# ZedSuite

**English** · [Français](README.fr.md)

![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-0078d4) ![Engine](https://img.shields.io/badge/detection%20engine-Rust-e6522c) ![License](https://img.shields.io/badge/license-GPL--3.0-2ea44f) ![Downloads](https://img.shields.io/github/downloads/LeZed97/ZedSuite/total)

**Open source map editor for VAG-group Bosch EDC15/EDC16 ECUs — 100% local, on Windows, macOS and Linux.**

Drop in an ECU dump and ZedSuite finds the maps for you — Driver Wish, Turbo Boost, N75, SOI, torque limiters and the rest. Edit them in a table, on a 2D graph or a 3D surface, or straight in the hexdump. Keep versions, compare them, disable or re-enable DTCs, fix the checksum, export your binary or a WinOLS mappack.

No account, no cloud, no limits: everything runs locally and your files stay on your computer.

![ZedSuite editor](docs/screenshot.png)

## 🚗 Supported ECUs

| ECU | Detection |
|-----|-----------|
| Bosch EDC15P | pattern + codeblock based |
| Bosch EDC15VM+ | pattern + codeblock based |
| Bosch EDC16U1 | signature based |
| Bosch EDC16U31 | signature based |
| Bosch EDC16U34 | signature based |

Identification is strict by design: a file is only opened as one of these ECUs when it carries positive evidence (Bosch hardware numbers, family strings, structural signatures). A 2 MB dump from another ECU (EDC17, Marelli, Siemens, …) is rejected instead of being misread as an EDC16.

Detection is not perfect either. Each family was calibrated on a bench made of every file I had available, but I did not have as many different EDC16U31 dumps as for the other families: on some U31 files, part of the maps may not be detected. Same thing on EDC15VM: some maps may not show up, especially on the 1 MB dumps of the V6 engines, which I deliberately left unfinished because it would have taken too much more time. In any case, when the maps are fully detected on EDC15/16, the mappacks are of unbeatable quality compared to what is available on the market.

## ⚙️ Features

- **Automatic map detection** — embedded Rust engine, per-family detectors
- **Detection completeness check** — a confidence badge shows whether every map expected for the ECU family was found, with the missing ones detailed in one click
- **Map editor** — table, 2D graph and 3D surface views, keyboard navigation and copy/paste between maps, absolute, additive or percent edits, propagation to similar maps, WinOLS-style shortcuts. The 2D graph is for single-line maps (curves, linearisations, single values); full matrices open in 3D, where a spike or a flat spot shows up at a glance, and in the table for exact values
- **Hexdump editor** — virtualized, minimap, modification highlighting vs original
- **Versioning** — "Ori" + named versions per project, compare view
- **Lean storage** — original binary + a modification file per version, rebuilt automatically at export
- **Virtual dyno** — power/torque estimation from the maps, printable PDF report
- **DTC on/off** — read the fault-code table, disable codes and re-enable them later (EDC15 and EDC16); every VAG code comes with its description in the five languages of the app
- **Solutions** — one-click patches (launch control, …); deliberately kept to a minimum so the release wouldn't take even more time, more may come later
- **Checksum correction** — EDC15 family and EDC16, implemented natively
- **Brand auto-fill** — embedded ECU reference database (Bosch/VAG part numbers)
- **Exports** — modified `.bin`, JSON mappack compatible with WinOLS 5
- **Automatic updates** — the app checks GitHub once a day for new releases; one click to install
- **3 themes** — dark, light and OLED, for every kind of screen
- **Any screen size** — resizable map panel and browser-style zoom in the editor, from laptops to ultrawides
- **Five languages** — English, French, Spanish, Italian and German, for the app and the installer; adding another is easy (a single translations file), and map names are deliberately untranslated — they stay in English

## 🔧 Working with modified files

The detection engine is deliberately built on **structure, not data**: it anchors on headers, axis layouts and signatures that survive a remap, so stage 1/2/3 files are detected fine in the vast majority of cases, a lot of bench work went specifically into that.

That said, an **extremely modified file** (rewritten axes, relocated blocks, aggressive protection patches) can still hide some maps from the scanner. The recommended workflow is:

1. Create the project from the **original (stock) file**: that's where detection finds every map.
2. Import the modified file as a **new version** of that project.

All versions of a project share the map list detected on the original, so you get the complete map set on the tuned file too, plus the compare view between versions for free.

## 🏗️ How it was built

ZedSuite started life as a web SaaS: a Next.js editor talking to a Rust (Actix) detection microservice. That version was meant to go much further: the plan was to launch with the whole VAG diesel range supported, up to the MD1, plus the older EDC15/16/17 diesels of the other brands. But I no longer had the time to finish that project, as I had to focus on other things, so I chose to release what was truly solid instead: the VAG EDC15/EDC16 scope, finished properly, as a fully local open source desktop tool.

On the design side, I tried to blend the most interesting features of the two tools I spent years with: the automatic detection and simplicity of EDCSuite, and the editing comfort of WinOLS.

Technically, the Rust engine moved into the Tauri shell as plain IPC commands, and the frontend still speaks to the old `/api/*` surface, which `src/lib/local/api.ts` reimplements on top of the on-disk store. This kept the editor code identical to the battle-tested web version while removing every server dependency.

The detection engine itself is the result of long reverse-engineering sessions on real dumps: each ECU family has its own detector, built by locating maps in WinOLS/damos-style references, extracting the structural signatures that identify them (dimension headers, axis layouts, selector blocks, inter-map spacing), then validating against a bench of real original AND tuned files until the results match the reference lists map for map. Detectors anchor on structure rather than data values precisely so that tuned files keep detecting. The same approach applies to the checksums (EDC15/EDC16 algorithms reimplemented natively, byte-validated against before/after pairs).

Rust for the detection engine was a deliberate choice. The project started as a SaaS meant to be hosted: beyond being much faster than EDCSuite's C# (no .NET runtime, no garbage collector, native machine code in a small standalone binary), I tried to optimize everything as far as possible for a web version where every detection ran server-side. As a result, a full detection takes under a second on any file: about 0.1 s on an EDC15VM (512 KB), 0.3 to 0.5 s on an EDC16 (1 to 2 MB) and around one second on an EDC15P, the heaviest scan. Against C/C++: the same speed, but a much stricter compiler that catches at build time the kind of errors that crash a tool on an unexpected file. And since Tauri is Rust too, the exact same engine that used to run on a server now runs embedded in the app, unchanged.

The interface itself is a web page: HTML, CSS and TypeScript, rendered by the browser engine the system already ships (WebView2 on Windows, WebKit on macOS, WebKitGTK on Linux). Around it, the Rust shell opens the window, reads and writes the files, and runs the detection engine, compiled natively for each platform.

That is why the macOS and Linux versions did not need a rewrite. The dashboard, the map editor, the 2D and 3D views, the hexdump, the DTC and power tools are the same code on the three systems, to the pixel. Only the shell knows where it runs: the title bar, the file dialogs, where the projects are stored and how an update is installed.

The web stack brings more than portability. No browser is bundled, so the Windows installer weighs about 6 MB and the app starts in a second. Graphs, themes and the five languages are built with tools made for that. And every fix to the interface reaches Windows, macOS and Linux at once.

## 🧭 What ZedSuite is, and what it will not become

ZedSuite stays in the spirit of EDCSuite: a tool everyone can have to learn the craft. You open a file, you see the maps, you understand what does what and you edit it yourself. That is also why there are no automatic solutions (EGR off, DPF off, one-click tunes) and why there will not be: the point is to understand the file, not to press a button.

**No new ECU family from me.** Every detector in the app took months of reverse engineering on hundreds of original and tuned files, checked against WinOLS packs and damos references, and that bench work is what makes the map list trustworthy. Doing it again for another family means two to three months minimum and a large corpus of original files and mappacks for that ECU. I maintain ZedSuite on my free time, and work of that size is not something I could give away for free. The supported list stays the VAG EDC15/EDC16 range, finished properly. A new family can still come from a contribution that meets the bar below.

## 🤝 Contributing

Contributions are welcome, new ECU detectors first, on one condition: they follow the standards set in [CONTRIBUTING.md](CONTRIBUTING.md), which explains the detector architecture, how the existing families were built and the bar a family has to meet before it ships (corpus, bench, invariants, zero false positive, right axes and units). I maintain ZedSuite on my free time. I will gladly review and merge a pull request, but I cannot end up doing half of the work afterwards in corrections, that is time I do not have. A clean contribution that covers less is worth more than a quick one that misleads the people who trust the map list; until the bar is met, a pull request stays open as a draft.

Found a bug or an undetected map? Open an issue with the ECU type and the file's software number, and attach the dump if you can: that is what gets it fixed, most detection fixes shipped so far came from a file a user sent. Files are only used to fix the detector and are never shared.

## 🙏 Thanks

- **Dilemma**, who released [VAGEDCSuite](https://github.com/Blackfrosch/VAGEDCSuite) about 14 years ago. That software is how I practiced and learned this craft: automatic map recognition and a dead-simple interface, at a time when nothing else offered that. It is an enormous piece of work for a tool born in the 2000s! (The man must be an alien) A large part of ZedSuite's EDC15 detection logic is directly inherited from the work done in EDCSuite.
- **Skalda**, who [kept VAGEDCSuite alive](https://github.com/skaldamramra/VAGEDCSuite) by updating the map detection and adding a lot of EDC15 maps. My own private build of EDCSuite started from his version, and it is what I used daily until I finally had the time to build ZedSuite.

### 👥 Contributors

Everyone whose code, report or file changed the app, with what it changed and the version it landed in: [CONTRIBUTORS.md](CONTRIBUTORS.md). Sending a dump with a report is what makes a detection fix possible; files are only used to fix the detector and are never shared.

## ⬇️ Download

Everything is in the **Assets** section of the [latest release](https://github.com/LeZed97/ZedSuite/releases/latest). Once installed, the app keeps itself up to date on its own on Windows and macOS; on Linux it tells you when a new version is out and opens the release page.

**Windows** — download `ZedSuite_x.y.z_x64-setup.exe` and run it (on a 32-bit Windows, take `ZedSuite_x.y.z_x86-setup.exe` instead). ZedSuite requires **Windows 10 or 11**: adapting it to Windows 7 would have required a lot more work.

**macOS** — download `ZedSuite_x.y.z_macos-universal.dmg`, open it and drag ZedSuite into the Applications folder. One build for Apple Silicon and Intel Macs, **macOS 12 or later**. ZedSuite is not signed with an Apple developer certificate, so the first launch takes one extra step: macOS refuses to open it, then **System Settings > Privacy & Security > Open Anyway** (on macOS 14 and earlier, right-click the app > Open). This happens once; updates installed by the app itself open directly. If you prefer Terminal, this one line installs or updates ZedSuite in Applications with no extra step:

```bash
curl -fsSL https://raw.githubusercontent.com/LeZed97/ZedSuite/master/install-macos.sh | sh
```

**Linux** — download `ZedSuite_x.y.z_linux-x86_64.AppImage`, make it executable (`chmod +x`) and run it, or install the `.deb` with `sudo apt install ./ZedSuite_x.y.z_linux-amd64.deb`. 64-bit x86, Debian 12, Ubuntu 22.04 or a newer derivative (the app needs webkit2gtk 4.1). The Linux build does not update itself yet: the app tells you when a new version is out and takes you to the release page. Linux support was contributed by [@bferd](https://github.com/bferd).

## 🗺️ Roadmap

What is being worked on, what is planned and what users asked for: [ROADMAP.md](ROADMAP.md) (also in [French](ROADMAP.fr.md), [Spanish](ROADMAP.es.md), [Italian](ROADMAP.it.md) and [German](ROADMAP.de.md)). The same page opens inside the app, in the app language, from the dashboard (roadmap button next to the help button). XDF compatibility (TunerPro definition files) is on it, planned for later, when I have the time to look into it.

## 📫 Contact

- 🌐 Website — [zedperf.com](https://zedperf.com)
- 📸 Instagram — [@zedperf](https://instagram.com/zedperf)
- ▶️ YouTube — [@ZedPerf](https://www.youtube.com/@ZedPerf)
- 👥 Facebook — [zedperf](https://www.facebook.com/zedperf.1/)
- 🔗 Everything in one place: [linktr.ee/zedperf](https://linktr.ee/zedperf)

## ☕ Buy me a coffee

ZedSuite is free and always will be. If it saved you time or a WinOLS licence, you can fuel the next reverse-engineering sessions:

- **PayPal**: [paypal.me/zedperf](https://www.paypal.com/paypalme/zedperf)
- **BTC** (Bitcoin): `bc1qj2e42vpphx73xguspqd9c6uqrs9ra0yywcq97a`
- **SOL / USDC** (Solana): `AqjSzxi7pBkwcCVkyVxBVLTk9TgPmui71bNgVgNLWrJC`
- **TRX** (Tron): `TRDgrasP7yaEKcz54r8spbmgZdRBFpNerW`

## 🚀 Getting started (development)

Prerequisites:
- [Node.js](https://nodejs.org) ≥ 18
- [Rust](https://rustup.rs) (stable) — the detection engine and the app shell are Rust/Tauri
- Windows 10/11 (WebView2 is preinstalled on Windows 11), macOS 12+ with the Xcode command line tools (`xcode-select --install`), or a Linux with the [Tauri prerequisites](https://tauri.app/start/prerequisites/) (webkit2gtk 4.1, gtk3, libayatana-appindicator, librsvg)

```bash
npm install
npm run app:dev     # launches the desktop app with hot reload
```

Build the installer:

```bash
npm run app:build   # produces the NSIS installer under src-tauri/target/release/bundle/
```

On macOS, `npx tauri build --target universal-apple-darwin` produces the `.app` and the `.dmg` for both architectures (`src-tauri/tauri.macos.conf.json` holds the macOS-only settings). On Linux, `npx tauri build` produces the AppImage and the `.deb` (`src-tauri/tauri.linux.conf.json`). The official builds are made on my own machines: Windows natively, macOS and Linux in virtual machines.

## 🧱 Architecture

```
src/                  Next.js frontend (static export, served by the Tauri webview)
  app/dashboard/      project list (opens on startup)
  app/editor/         the map editor
  lib/local/          local backend: on-disk project store + API bridge
  lib/ecu/            TypeScript ECU helpers (DTC lists, checksums)
src-tauri/            Rust desktop shell
  src/detector/       the detection engine (one folder per manufacturer)
  src/commands.rs     IPC commands exposed to the frontend
```

Projects are stored in `%APPDATA%/com.zedsuite.app/projects/` on Windows, `~/Library/Application Support/com.zedperf.zedsuite/projects/` on macOS and `~/.local/share/com.zedsuite.app/projects/` on Linux — one folder per project with the original binary, metadata and versions.

## ⚖️ License and trademarks

[GPL-3.0](LICENSE) — you are free to use, study, modify and redistribute ZedSuite, but derivative works must be released under the same license. If you improve the detection engine or add ECU support, the community gets it back.

**The license covers the code only.** The ZedSuite name, logo and mascot are trademarks of ZedPerf and are explicitly excluded from the GPL grant (GPL-3.0 §7(e)): forks are welcome, but they must ship under their own name and branding. Full policy: [TRADEMARKS.md](TRADEMARKS.md). Official builds are published exclusively on [this repository's releases page](https://github.com/LeZed97/ZedSuite/releases).

## ⚠️ Disclaimer

ZedSuite is intended for research, education and motorsport/off-road use. Modifying the ECU of a road vehicle may be illegal in your jurisdiction and can void your warranty, damage your engine, or make your vehicle non-compliant with emissions regulations. You are solely responsible for how you use this software.

**A word on security**: tuning software is a prime target for hackers, who sometimes use free tools to distribute malware. Always download the installer from the [official GitHub](https://github.com/LeZed97/ZedSuite/releases) — it is the only way to be sure you are safe.
