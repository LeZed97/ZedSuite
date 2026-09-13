# Contributors

ZedSuite is better because people took the time to write code, to report what was wrong, to ask for what was missing and to send the files that made it possible to fix. This page lists them with what their contribution changed, and in which version it landed.

The historical thanks, to the people whose earlier work made this project possible, are in the [README](README.md).

## Contributions

- [@COSSART-FR](https://github.com/COSSART-FR) — power estimate of a version created by importing a file (1.1.8). Also the Bosch EDC16CP31 Mercedes detector, in progress.
- [@bferd](https://github.com/bferd) — the Linux version: the AppImage and .deb builds, and the update dialog that opens the release page on Linux (1.2.1).
- [@Hitsauskone](https://github.com/Hitsauskone) — asked for the Linux version, built by @bferd (1.2.1).
- [@AlfonsoMad](https://github.com/AlfonsoMad) — copy and paste with Excel, EDC Suite and other programs (1.2.1).
- [@henry66z](https://github.com/henry66z) — EDC15VM 038906012M: the single injection timing map kept on tuned files and the MAP/MAF switch found on every VP37 software (1.1.7), then the axes of the boost correction by temperature and of the MAP linearisation (1.1.8).
- [@georgiminchev04](https://github.com/georgiminchev04) — the MAP/MAF switch on the compact EDC15P software (1.1.8), the airflow limiter of an EDC15VM whose axis has eleven points (1.1.9), and the MAP/MAF switch on EDC16, which was missing from the app entirely (1.2.0).
- [@Matt010101](https://github.com/Matt010101) — an EDC16U34 airflow limiter left at zero by its software, the report that showed why the EDC16 sensor switch had to be visible (1.2.0).
- [@Informapa](https://github.com/Informapa) — a 2.5 V6 EDC15VM taken for an EDC15P, which left it with almost no map (1.2.0).
- [@kultss](https://github.com/kultss) — the power curves named with the real codeblock numbers of the file (1.1.8), and the injector duration axes written back to the wrong axis, or without their factor, on EDC15P (1.2.1).
- [@NiwiTheFox](https://github.com/NiwiTheFox) — a 038906019FJ that turned up three problems at once: the overboost limit not found, the airflow limiter wrongly expected, and the driver wish axes swapped (1.1.9).
- [@hunterw7](https://github.com/hunterw7) — the launch control and the fault codes losing their bytes when a version was saved a second time (1.1.9).
- [@BGRibeiro00](https://github.com/BGRibeiro00) — the same loss seen from the fault-code side, on a 1.4 TDI (1.1.9), and fault codes that do not exist on the ECU listed on that 1.4 TDI (1.2.1).
- [@yovko82](https://github.com/yovko82) — the editor on a 1024 by 768 screen: the zoom ceiling, the window opening under the taskbar, and the map window that would not fill (1.1.9), then the editor zoom that dropped to 50 % after the window was minimized, and the map orientation and display settings remembered for the whole map family and per ECU type, from the editor settings (1.2.1).
- [@TechPro120](https://github.com/TechPro120) — the smoke switches of the EDC16 software that runs on lambda, missing on a 038906016K (1.2.1).
- [@Marvx1j](https://github.com/Marvx1j) — the launch control on the 038906019HJ (1.1.6).
- [@Yonifarolas](https://github.com/Yonifarolas) — multimap files and the codeblock they add, now shown on their version (1.1.6).
- [@LJ-PVD](https://github.com/LJ-PVD) — full screen.
- **mkjar**, on the ecuconnections thread — the fault codes of the compact EDC15 software (1.1.7).

## Contributing

New ECU families are the contribution I want the most. [CONTRIBUTING.md](CONTRIBUTING.md) explains how the detection engine is organized, how the existing detectors were built, and what a new family has to meet before it ships.

Reporting helps just as much: open an issue with the ECU type and the software number, and attach the dump if you can. That is what makes a detection fix possible, and every detection fix of the last releases came from a file someone sent. Files are only used to fix the detector and are never shared.
