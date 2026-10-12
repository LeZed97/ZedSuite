90 hp TDI ECU code comparison — 12 October 2026

Checked all ten manufacturer ECU indexes linked from chiptuning.pw. Matching entries were found in Volkswagen, Audi, Seat and Skoda. Downloaded 120 archives: 114 explicitly labeled 90 hp / 90 PS / 66 kW, plus 6 engine-labeled candidates with no stated power rating. These contain 163 eligible payloads and 146 unique payload hashes. Every selected archive downloaded and unpacked; source labels and format qualifications are retained below.

The groups below are exact byte matches in the compared main program region. They are stronger evidence than a matching suffix or bank size. They do not establish that complete flash files, every calibration value or vehicle behavior are interchangeable.

**Exact program groups**

| Group | ECU identities and embedded software versions | Bank layout |
|---|---|---|
| P1 | 038906012FD (4102); 038906012GN (4308); 038906012GQ (4307) | 64 KiB |
| P2 | 038906012F (3313, 3331); 038906012FH (3537); 038906012FN (3358); 038906012H (3315, 3332) | 64 KiB |
| P3 | 038906012AF (2825); 038906012AJ (2829); 038906012B (2802); 038906012C (2803); 038906012CP (2839); 038906012Q (2818) | 48 KiB |
| P4 | 038906012BD (2507); 038906012M (2528) | 48 KiB |
| P5 | 038906012GK (KD34) | 64 KiB |
| P6 | 038906019AB (1071) | 64 KiB |
| P7 | 038906019LP (1596) | 64 KiB |
| P8 | 038906012AJ (2707) | 48 KiB |
| P9 | 038906012S (2512) | 48 KiB |
| P10 | 038906012BS (2804); 038906012CA (2806) | 48 KiB |
| P11 | 038906012HA (4922) | 64 KiB |

There are five groups with multiple ECU identities: P1, P2, P3, P4 and P10. CP and BD have the same 48 KiB bank arrangement but different program bytes. FH and GK differ from FD/GN/GQ; HA also has its own program. AJ 2829 belongs to P3, while AJ 2707 belongs to P8. The results apply to the listed software versions, not every possible revision of those ECU part numbers.

**Map definition and calibration comparisons**

The current ZedSuite EDC15VM detector was run on the 512 KiB 038906012 files. Its results are heuristic and do not cover every map. The comparisons below use first-bank map name, absolute address, size, dimensions and axis addresses. Map-cell and axis bytes were then compared independently. Matching detected definitions does not establish completeness or compatibility of an entire mappack.

| Compared pair | Matched detected definitions | Equal map-cell data | Equal axes |
|---|---:|---:|---:|
| 038906012FD / 038906012GN | 40 | 40 | 40 |
| 038906012GQ / 038906012GN | 40 | 40 | 40 |
| 038906012Q / 038906012CP | 25 | 21 | 22 |
| 038906012M / 038906012BD | 24 | 20 | 21 |
| 038906012BS / 038906012CA | 23 | 11 | 16 |

FD/GN/GQ share all 40 detected first-bank definitions, cell values and axes in these particular files. CP/Q differ in detected EGR, Start IQ, SOI limiter and N108 SOI map cell data; BD/M differ in the same detected categories. The complete pair comparisons and differing addresses are in TDI90_Full_Comparison.json.

**Comparison method and limitations**

- Full-payload duplicates were identified using SHA256 over every byte.
- The 64 KiB-bank program fingerprint covers [0x14000, 0x4BFFC). The 48 KiB-bank fingerprint covers [0x14000, 0x57FFC). Endpoints are exclusive; checksum storage and calibration banks are excluded.
- Lower program region [0, 0x14000) was compared separately within every same-program group. Differences were small (at most 16 bytes) and confined to identification-area/checksum locations. Exact offsets are recorded in the JSON; this is not a disassembly-based proof of equivalence.
- Bank signatures and spacing are recorded separately. Three signatures versus two means the existing factory-bank arrangement differs even when the program region matches.
- Older 256 KiB files, split chip dumps, the unknown-header file, the WinOLS container and the 1 MiB file are inventoried and checked for exact payload duplicates. Their executable/calibration boundaries were not established, so no same-program conclusion is assigned to them.
- A directory-only RAR entry is excluded from raw image comparison; it is not an independent firmware image.
- No inference of original manufacturer authenticity or engine power was made from an archive name. Six candidates lack an explicit power rating. At least one filename labels an AFN file as 90 hp; it remains marked as site-labeled rather than independently power-verified.
- The 038906019AB file is a separate program group; this local checksum implementation returned TYPE_ERROR for it. That alone does not establish a corrupt source image.
- This is offline binary analysis. No ECU was flashed, no vehicle operation was tested and no additional Tools profile was enabled.

**Filename discrepancies**

- The Seat archive named 038906021BS contains identity 038906012BS. The group table uses the embedded identity.
- The Golf4 archive named 038906012M 2813 contains software version 2528, matching the other M archive.
- The GK file identifies its software as KD34; no stock provenance is inferred from the site label.

