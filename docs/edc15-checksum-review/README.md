# EDC15 checksum layout review



On reference 038906019AB / 1071, the existing V4.1-marker heuristic picks tdi41_2002 and attempts four corrections even on the unchanged source. The legacy single-bank structure has no populated 2002 main/prefix storage; standard tdi41 verifies its three populated entries without modifying a byte. The added rule uses one signature at 0x70001, the entire C3-padded 0x6C000–0x70000 prefix, absent main/prefix checksum storage and present legacy checksum entries. It does not match by ECU name, and remains stable after a calibration edit.



The patch also counts populated checksum entries rather than loop slots, so C3/empty compact entries no longer show misleading 6/7 results. TYPE_ERROR is preserved by verifyEDC15PChecksum; previously a no-op unsupported result with fixed=0 could be reported as OK.



Scope: one TypeScript checksum module, a standalone synthetic regression test and review evidence. No UI, conversion tool, Rust detector changes, ECU binaries or version bump.



## Validation



- `node tests/edc15-checksum-layouts.cjs`: 3 checks passed (unknown/short buffers; legacy selection before/after changes and checksum-byte isolation; compact populated-entry counts).

- The identical module in the [experimental MAP-tool branch](https://github.com/DasTDIWagon/ZedSuite/tree/experimental/map-sensor-calibration-all25) passed the 25-file/180-case real-data bench. Its test runner, source links and fixture annotations are committed there. Bench.json records exact input hashes and all three preset checksum results for each file.

- AB1071: original and corrected test outputs 3/3 tdi41 entries validate. LP1596 remains tdi41_2002, 5/5. Compact two-bank references are 6/6; three-bank references 7/7. Other classic references retain 8/8 or 11/11.

- TypeScript and production/frontend/native builds passed in that full experimental app; 66 Rust library tests passed. No Rust production code is changed by this PR.

- No private fixture paths or binary payloads are committed. Source labels do not establish untouched factory provenance. The maintainer's broader checksum corpus and independent implementation review remain outstanding; no vehicle validation is claimed.



## Bench



| ECU | Software | Populated entries | Variant |

|---|---|---|---|

| 038906012FD | 4102 | 11/11 | tdi41_2002 |

| 038906012GN | 4308 | 8/8 | tdi41_2002 |

| 038906012GQ | 4307 | 8/8 | tdi41_2002 |

| 038906012F | 3313 | 11/11 | tdi41_2002 |

| 038906012F | 3331 | 11/11 | tdi41_2002 |

| 038906012FH | 3537 | 8/8 | tdi41_2002 |

| 038906012FN | 3358 | 8/8 | tdi41_2002 |

| 038906012H | 3315 | 11/11 | tdi41_2002 |

| 038906012H | 3332 | 11/11 | tdi41_2002 |

| 038906012AF | 2825 | 6/6 | tdi41v2 |

| 038906012AJ | 2829 | 6/6 | tdi41v2 |

| 038906012B | 2802 | 6/6 | tdi41v2 |

| 038906012C | 2803 | 6/6 | tdi41v2 |

| 038906012CP | 2839 | 7/7 | tdi41v2 |

| 038906012Q | 2818 | 6/6 | tdi41v2 |

| 038906012BD | 2507 | 7/7 | tdi41v2 |

| 038906012M | 2528 | 6/6 | tdi41v2 |

| 038906012GK | KD34 | 8/8 | tdi41_2002 |

| 038906019AB | 1071 | 3/3 | tdi41 |

| 038906019LP | 1596 | 5/5 | tdi41_2002 |

| 038906012AJ | 2707 | 6/6 | tdi41v2 |

| 038906012S | 2512 | 6/6 | tdi41v2 |

| 038906012BS | 2804 | 6/6 | tdi41v2 |

| 038906012CA | 2806 | 6/6 | tdi41v2 |

| 038906012HA | 4922 | 8/8 | tdi41_2002 |

