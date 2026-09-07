# ZedSuite roadmap

This page lists what is planned, what users asked for and what is not planned.

To ask for a feature or report a bug: open an issue on [GitHub](https://github.com/LeZed97/ZedSuite/issues), post in the [ecuconnections thread](https://www.ecuconnections.com/forum/viewtopic.php?p=393279#p393279), or reach me on my social networks: [linktr.ee/zedperf](https://linktr.ee/zedperf). Every report is read.

## Planned

- EDC15P early PD software (1999-2002, 038906019A / 019AJ): map detection is done, the checksum and the DTC table of these dumps are not supported yet, and the MAP linearisation map is not found on the 019A.
- EDC16U31: better detection, a few EDC16U31 files are still missing from the test bench to finish it properly (the 12x12 family at 0x1D7xxx is still unnamed).
- EDC16U1: identification of the Touareg V10 files, where only one of the two ECU numbers is found today (six files on the bench).
- Better EDC15VM detection: a few files are still not fully covered, the 2.5 V6 in particular. Fixes are already in for the software numbers users sent me and for the editing bugs they reported.
- EDC15VM: check on the car that the SVRL maps are really active when the detector finds them.
- Detection of the turbo boost control PID maps, on EDC15P first.

## Asked by users, under review

- WinOLS-style 2D view for full matrices: one curve per row, selected row highlighted.
- Undo with Ctrl+Z in the editor.
- CSV import and export, and DAMOS import.
- Editing maps directly in the 3D view.
- Editing several versions of the same project side by side and comparing their maps.

## Not planned for now

- Adding new ECUs (BMW and PSA EDC15/EDC16, etc.).
- More ECU reference data (brands, engines) for the import screen.
- Windows 7 compatibility.
