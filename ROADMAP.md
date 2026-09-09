# ZedSuite roadmap

This page lists what is planned, what users asked for and what is not planned.

To ask for a feature or report a bug: open an issue on [GitHub](https://github.com/LeZed97/ZedSuite/issues), post in the [ecuconnections thread](https://www.ecuconnections.com/forum/viewtopic.php?p=393279#p393279), or reach me on my social networks: [linktr.ee/zedperf](https://linktr.ee/zedperf). Every report is read.

## Planned

- Comparing the maps of two versions of a project in the Compare window, which does the binary comparison today: the same map of both versions side by side, differences highlighted.
- WinOLS-style 2D view for full matrices: one curve per row, selected row highlighted.
- An N75 inversion switch on EDC15VM, for cars converted from a wastegate turbo to a VNT or the reverse: the block that controls it is located on most files of the bench, the switch itself is not built yet.
- EDC15P early PD software (1999-2002, 038906019A / 019AJ): map detection is done and the DTC table is read on the 019AJ; the checksum is not supported yet, the DTC table of the 019A uses yet another layout, and the MAP linearisation map is not found on the 019A.
- Better EDC15VM detection: a few files are still not fully covered, the 2.5 V6 in particular, and the N146 and N75 maps of the 012K / 012AP generation. Fixes are in for the software numbers users sent (012M single SOI and MAP/MAF switch in 1.1.7).
- EDC15VM: check on the car that the SVRL maps are really active when the detector finds them.
- EDC16U31: better detection, a few EDC16U31 files are still missing from the test bench to finish it properly (the 12x12 family at 0x1D7xxx is still unnamed).
- EDC16U1: identification of the Touareg V10 files, where only one of the two ECU numbers is found today (six files on the bench).
- Detection of the turbo boost control PID maps, on EDC15P first.

## Asked by users, under review

- Undo with Ctrl+Z in the editor.
- CSV import and export (the JSON mappack for WinOLS is already there), and DAMOS import.
- Favourite maps, for quick access to the ones you edit most.
- A reference version other than Ori for "original value" and the comparison.
- Inverse driver wish and MAF linearisation in the map list.
- Editing maps directly in the 3D view.
- Tuning the power estimate for nozzles other than Firad, such as Recambo or DSSR.
- Editing two versions of the same project side by side.
- More functions in the map Properties window.
- Highlighting every value that differs from stock in the map windows.
- Remembering the map orientation (axis mirror): today the display transposition is set map by map, in one project only. The plan is to apply the choice to the whole map family at once and to save it as a global setting, so every project opens the maps the same way.
- A Linux version. The interface is the same code as on Windows and macOS, only the shell side would need doing; it depends on how many people ask for it.

## Not planned for now

- Adding new ECUs (BMW and PSA EDC15/EDC16, etc.).
- A map switching routine (multimap) patched into the ECU: do it in WinOLS with the routines that circulate for EDC15, then import the file as a version, the added codeblock and its maps are shown since 1.1.6.
- More ECU reference data (brands, engines) for the import screen.
- Windows 7 compatibility.
