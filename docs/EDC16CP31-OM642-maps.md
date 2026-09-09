# EDC16CP31 OM642 165 kW — cartographie confirmée

Dump de référence : `originecan-kess`, 2 Mo, SW Bosch **1037393817**, `99/1/EDC16CP31/001/B209/X/080000_000/`, `CR4-642-42P7-209CM-165kW-PT2R05-LR-3907x064ME`.

Labels transférés depuis le damos ASAP2 `CR4-642` (même projet B209/V080000, **autre build** : aucune adresse A2L ne tombe en face) par appariement structurel des axes + alignement d'ordre, puis relecture et vérification dans le dump.

Layout de chaque bloc : `[nx u16][ny u16][X i16×nx][Y i16×ny][Z i16×nx·ny]`, big-endian, Z en **sens colonne** (`z[x·ny + y]`). L'adresse « bloc » est le mot `nx`.

## Rail_pSetPointBase_MAP  — Rail pressure
_Basismap for the Rail pressure-Setpoint_

| | |
|---|---|
| Bloc | `0x1F1BF2` → `0x1F1E36` (580 o, alloc `0x244`) |
| Grille | 16 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1F1BF6` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1F1C16` — InjCtl_qCurr, ×0.01 [mm^3/cyc] |
| Données Z | `0x1F1C36` — Pres_Rail, ×0.1 + 0.0 [bar] |
| Plage stock | 250.0 … 1632.0 bar (raw 2500 … 16320) |
| En-tête | `00 10 00 10 01 90 01 F4 02 26 02 EE` |

Axe X : 400 500 550 750 1000 1201 1400 1600 1800 2000 2400 2800 3200 3600 4200 4400

Axe Y : 0 1 2 5 10 15 20 25 30 35 40 45 50 60 70 80

## Rail_pSetPointMin_MAP  — Rail pressure
_minimaler Rail pressure-Setpoint, dependent of the Engine speed and the Injection quantity_

| | |
|---|---|
| Bloc | `0x1F2B26` → `0x1F2C5A` (308 o, alloc `0x244`) |
| Grille | 16 × 8  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1F2B2A` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1F2B4A` — InjCtl_qCurr, ×0.01 [mm^3/cyc] |
| Données Z | `0x1F2B5A` — Pres_Rail, ×0.1 + 0.0 [bar] |
| Plage stock | 240.0 … 1122.0 bar (raw 2400 … 11220) |
| En-tête | `00 10 00 08 01 90 01 F4 02 26 02 EE` |

Axe X : 400 500 550 750 1000 1200 1400 1600 1800 2000 2400 2800 3200 3600 4200 4400

Axe Y : 0 10 20 30 40 50 60 80

## Rail_pSetPointLimN_MAP  — Rail pressure
_Basisvalue the engine speed- and injection quantityndependenten Rail pressurelimit_

| | |
|---|---|
| Bloc | `0x1F2964` → `0x1F2A50` (236 o, alloc `0x1BC`) |
| Grille | 8 × 12  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1F2968` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1F2978` — Rail_qTotFlt_mp, ×0.01 [mm^3/cyc] |
| Données Z | `0x1F2990` — Pres_Rail, ×0.1 + 0.0 [bar] |
| Plage stock | 900.0 … 1632.0 bar (raw 9000 … 16320) |
| En-tête | `00 08 00 0C 00 64 12 43 12 75 12 A7` |

Axe X : 100 4675 4725 4775 4825 4875 4900 5000

Axe Y : 0 20 30 40 50 60 65 70 75 80 95 110

## Rail_pPreCtl_MAP  — Rail pressure
_Vorsteuermap for the Pressureclosed-loop control via Pressure regulator valve_

| | |
|---|---|
| Bloc | `0x1F162C` → `0x1F16D0` (164 o, alloc `0x244`) |
| Grille | 8 × 8  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1F1630` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1F1640` — Rail_pSetPoint, ×0.1 [bar] |
| Données Z | `0x1F1650` — Pres_Rail, ×0.1 + 0.0 [bar] |
| Plage stock | 250.0 … 1600.0 bar (raw 2500 … 16000) |
| En-tête | `00 08 00 08 00 64 00 DC 00 E6 06 40` |

Axe X : 100 220 230 1600 2000 2800 3600 4000

Axe Y : 250 400 600 700 800 1000 1200 1600

## PCR_pDesBas_MAP  — Boost
_Map for the Grundvaluedetermination of the Boost pressurebasissetpoints_

| | |
|---|---|
| Bloc | `0x1E4BCE` → `0x1E4D8A` (444 o, alloc `0x244`) |
| Grille | 16 × 12  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1E4BD2` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1E4BF2` — PCR_qDes, ×0.01 [mm^3/hub] |
| Données Z | `0x1E4C0A` — Pres_hPa, ×1.0 + 0.0 [hPa] |
| Plage stock | 1010.0 … 2570.0 hPa (raw 1010 … 2570) |
| En-tête | `00 10 00 0C 02 EF 03 E8 04 B0 05 78` |

Axe X : 751 1000 1200 1400 1600 1800 2000 2200 2400 2800 3200 3600 4000 4200 4400 4600

Axe Y : 0 5 10 15 20 25 30 40 50 60 70 80

## PCR_pBDesMaxAPGearHi_MAP  — Boost
_Maximalpressuredetermination upper Ganggruppe for the BPA Schutz_

| | |
|---|---|
| Bloc | `0x1E3E36` → `0x1E3FF2` (444 o, alloc `0x244`) |
| Grille | 16 × 12  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1E3E3A` — PCR_nPresTempCor_mp, ×1.0 [rpm] |
| Axe Y (charge) | `0x1E3E5A` — PCR_pCorPresVal_mp, ×1.0 [hPa] |
| Données Z | `0x1E3E72` — Pres_hPa, ×1.0 + 0.0 [hPa] |
| Plage stock | 0.0 … 2760.0 hPa (raw 0 … 2760) |
| En-tête | `00 10 00 0C 00 C8 03 E8 05 DC 08 98` |

Axe X : 200 1000 1500 2200 3100 3500 4000 5300 6000 7000 8000 8800 9000 9200 9400 10000

Axe Y : 550 600 650 700 750 800 850 900 950 1000 1050 1100

## PCR_pBDesMaxAPGearLo_MAP  — Boost
_Maximalpressuredetermination lower Ganggruppe for the BPA Schutz_

| | |
|---|---|
| Bloc | `0x1E407A` → `0x1E4236` (444 o, alloc `0x244`) |
| Grille | 16 × 12  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1E407E` — PCR_nPresTempCor_mp, ×1.0 [rpm] |
| Axe Y (charge) | `0x1E409E` — PCR_pCorPresVal_mp, ×1.0 [hPa] |
| Données Z | `0x1E40B6` — Pres_hPa, ×1.0 + 0.0 [hPa] |
| Plage stock | 0.0 … 2760.0 hPa (raw 0 … 2760) |
| En-tête | `00 10 00 0C 00 C8 03 E8 05 DC 08 98` |

Axe X : 200 1000 1500 2200 3100 3500 4000 5300 6000 7000 8000 8800 9000 9200 9400 10000

Axe Y : 550 600 650 700 750 800 850 900 950 1000 1050 1100

## PCR_rCtlBas_MAP  — VNT
_Basissteuermap for Boost pressure_

| | |
|---|---|
| Bloc | `0x1DB4F0` → `0x1DB624` (308 o, alloc `0x244`) |
| Grille | 16 × 8  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1DB4F4` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1DB514` — PCR_qDes, ×0.01 [mm^3/hub] |
| Données Z | `0x1DB524` — Prc, ×0.01220703125 + 0.0 [%] |
| Plage stock | 23.4985 … 88.0005 % (raw 1925 … 7209) |
| En-tête | `00 10 00 08 02 93 03 E8 04 B0 05 78` |

Axe X : 659 1000 1200 1400 1600 1800 2000 2200 2400 2600 2800 3200 3600 4000 4400 4700

Axe Y : 0 10 20 25 30 40 50 70

## PCR_rCtlLghtOffBas_MAP  — VNT
_Map for Grundvalue of the LDA-steuervalue in the State Light Off_

| | |
|---|---|
| Bloc | `0x1DEB3C` → `0x1DEC70` (308 o, alloc `0x244`) |
| Grille | 16 × 8  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1DEB40` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1DEB60` — PCR_qDes, ×0.01 [mm^3/hub] |
| Données Z | `0x1DEB70` — Prc, ×0.01220703125 + 0.0 [%] |
| Plage stock | 11.499 … 71.4966 % (raw 942 … 5857) |
| En-tête | `00 10 00 08 02 94 03 E8 04 B0 05 78` |

Axe X : 660 1000 1200 1400 1600 1800 2000 2200 2400 2600 2800 3200 3600 4000 4400 4700

Axe Y : 0 10 20 25 30 40 50 70

## PCR_rCtlPFltRgnBas_MAP  — VNT
_Map for Grundvalue of the LDA-steuervalue in the State PFltRgn_

| | |
|---|---|
| Bloc | `0x1DEE28` → `0x1DEF5C` (308 o, alloc `0x244`) |
| Grille | 16 × 8  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1DEE2C` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1DEE4C` — PCR_qDes, ×0.01 [mm^3/hub] |
| Données Z | `0x1DEE5C` — Prc, ×0.01220703125 + 0.0 [%] |
| Plage stock | 13.0005 … 76.001 % (raw 1065 … 6226) |
| En-tête | `00 10 00 08 02 94 03 E8 04 B0 05 78` |

Axe X : 660 1000 1200 1400 1600 1800 2000 2200 2400 2600 2800 3200 3600 4000 4400 4700

Axe Y : 0 10 20 25 30 40 50 70

## PCR_rGvnrMax_MAP  — VNT
_Map/Curve for Determination the maximal zulässigen Manipulated variable from Engine speed and Injection quantity_

| | |
|---|---|
| Bloc | `0x1ECEF8` → `0x1ED02C` (308 o, alloc `0x244`) |
| Grille | 16 × 8  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1ECEFC` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1ECF1C` — InjCtl_qCurr, ×0.01 [mm^3/cyc] |
| Données Z | `0x1ECF2C` — Prc, ×0.01220703125 + 0.0 [%] |
| Plage stock | 31.9946 … 94.9951 % (raw 2621 … 7782) |
| En-tête | `00 10 00 08 02 94 03 E8 04 B0 05 78` |

Axe X : 660 1000 1200 1400 1600 1800 2000 2200 2400 2600 2800 3200 3600 4000 4400 4700

Axe Y : 0 10 20 25 30 40 50 70

## PCR_rGvnrMaxDyn_MAP  — VNT
_Betriebspunktdependenter Grundvalue the maximal zulässigen Manipulated variable in instat. Betriebsmode_

| | |
|---|---|
| Bloc | `0x1ECB5E` → `0x1ECC92` (308 o, alloc `0x244`) |
| Grille | 16 × 8  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1ECB62` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1ECB82` — InjCtl_qCurr, ×0.01 [mm^3/cyc] |
| Données Z | `0x1ECB92` — Prc, ×0.01220703125 + 0.0 [%] |
| Plage stock | 31.9946 … 94.9951 % (raw 2621 … 7782) |
| En-tête | `00 10 00 08 02 94 03 E8 04 B0 05 78` |

Axe X : 660 1000 1200 1400 1600 1800 2000 2200 2400 2600 2800 3200 3600 4000 4400 4700

Axe Y : 0 10 20 25 30 40 50 70

## FlMng_qSmk_MAP  — Smoke
_Map for Determination the kritischen maximum Fuel quantity the Smoke limit_

| | |
|---|---|
| Bloc | `0x1A8986` → `0x1A8BCA` (580 o, alloc `0x244`) |
| Grille | 16 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1A898A` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1A89AA` — PCR_pBPSCor, ×1.0 [hPa] |
| Données Z | `0x1A89CA` — InjMassCyc, ×0.01 + 0.0 [mm^3/cyc] |
| Plage stock | 24.0 … 86.0 mm^3/cyc (raw 2400 … 8600) |
| En-tête | `00 10 00 10 02 EE 03 20 03 E8 04 B0` |

Axe X : 750 800 1000 1200 1400 1600 2000 2400 2800 3200 3600 3800 4000 4200 4400 4800

Axe Y : 700 800 900 1000 1100 1200 1300 1400 1500 1600 1700 1800 2000 2200 2400 2500

## FlMng_qLimT3BPS_MAP  — Smoke
_Map for T3(gefilterter Boost pressure)-dependente Quantity limit_

| | |
|---|---|
| Bloc | `0x1A8436` → `0x1A858A` (340 o, alloc `0x154`) |
| Grille | 12 × 12  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1A843A` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1A8452` — BPSCD_pFltVal, ×1.0 [hPa] |
| Données Z | `0x1A846A` — InjMassCyc, ×0.01 + 0.0 [mm^3/cyc] |
| Plage stock | 59.9 … 88.0 mm^3/cyc (raw 5990 … 8800) |
| En-tête | `00 0C 00 0C 03 20 04 B0 06 40 07 D0` |

Axe X : 800 1200 1600 2000 2400 2800 3200 3600 4000 4200 4400 4600

Axe Y : 1600 1700 1800 1900 2000 2050 2100 2150 2200 2250 2300 2400

## AccPed_trqEng_MAP  — Driver wish
_Drive-behavior map in Engine torque at conventional Drivetrain control_

| | |
|---|---|
| Bloc | `0x1918DE` → `0x191982` (164 o, alloc `0x244`) |
| Grille | 8 × 8  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1918E2` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1918F2` — AccPed_rChkdVal, ×0.01220703125 [%] |
| Données Z | `0x191902` — TrqPrp, ×0.1 + 0.0 [Nm] |
| Plage stock | 0.0 … 500.0 Nm (raw 0 … 5000) |
| En-tête | `00 08 00 08 00 00 02 58 04 B0 07 08` |

Axe X : 0 600 1200 1800 2400 3000 3600 4200

Axe Y : 1.19629 7.99561 15.0024 19.9951 40.0024 59.9976 80.0049 100

## AccPed_trqEng2_MAP  — Driver wish
_Drive-behavior map in Engine torque at conventional Drivetrain control_

| | |
|---|---|
| Bloc | `0x19169A` → `0x19173E` (164 o, alloc `0x244`) |
| Grille | 8 × 8  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x19169E` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1916AE` — AccPed_rChkdVal, ×0.01220703125 [%] |
| Données Z | `0x1916BE` — TrqPrp, ×0.1 + 0.0 [Nm] |
| Plage stock | 0.0 … 600.0 Nm (raw 0 … 6000) |
| En-tête | `00 08 00 08 00 00 02 58 04 B0 07 08` |

Axe X : 0 600 1200 1800 2400 3000 3600 4200

Axe Y : 0 7.99561 15.0024 19.9951 40.0024 59.9976 80.0049 100

## FlMng_qLimBstPres_MAP  — Quantity limiter
_Kf. for Quantity limit as Function of the Boost pressures_

| | |
|---|---|
| Bloc | `0x1A7FE8` → `0x1A813C` (340 o, alloc `0x154`) |
| Grille | 12 × 12  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1A7FEC` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1A8004` — FlMng_pDiff_mp, ×1.0 [hPa] |
| Données Z | `0x1A801C` — InjMassCyc, ×0.01 + 0.0 [mm^3/cyc] |
| Plage stock | 0.0 … 100.0 mm^3/cyc (raw 0 … 10000) |
| En-tête | `00 0C 00 0C 04 B0 06 40 07 D0 09 60` |

Axe X : 1200 1600 2000 2400 2800 3200 3600 3800 4000 4200 4400 4800

Axe Y : 1600 1650 1700 1750 1800 1850 1900 1950 2000 2050 2100 2200

## FMTC_trq2qBas_MAP  — Torque->IQ
_Grundmap for Moment/Quantity Conversion as Function of the Arbeitspunktes_

| | |
|---|---|
| Bloc | `0x1A9514` → `0x1A979C` (648 o, alloc `0x2CC`) |
| Grille | 16 × 18  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1A9518` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1A9538` — CoEng_trqInrSet, ×0.1 [Nm] |
| Données Z | `0x1A955C` — InjMassCyc, ×0.01 + 0.0 [mm^3/cyc] |
| Plage stock | 0.0 … 88.0 mm^3/cyc (raw 0 … 8800) |
| En-tête | `00 10 00 12 00 00 02 BC 03 20 03 E8` |

Axe X : 0 700 800 1000 1200 1400 1600 1800 2000 2400 2800 3200 3600 4000 4200 4600

Axe Y : 0 20 40 60 80 100 150 200 250 300 350 400 450 500 525 550 575 600

## InjCrv_phiMI1Bas1_MAP  — SOI
_1. Grundvalueemap, Injector actuation start MI1_

| | |
|---|---|
| Bloc | `0x1B07EC` → `0x1B0A30` (580 o, alloc `0x244`) |
| Grille | 16 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1B07F0` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1B0810` — InjCtl_qSetUnBal, ×0.01 [mm^3/cyc] |
| Données Z | `0x1B0830` — AngleCrS, ×0.023437499999999983 + 0.0 [deg CrS] |
| Plage stock | -2.5078 … 25.2187 deg CrS (raw -107 … 1076) |
| En-tête | `00 10 00 10 01 90 02 26 02 EE 03 E8` |

Axe X : 400 550 750 1000 1200 1400 1600 1800 2000 2400 2800 3200 3600 4000 4200 4400

Axe Y : 0 1 2 5 10 15 20 25 30 35 40 45 50 60 70 80

## InjCrv_phiMI1Bas2_MAP  — SOI
_2. Grundvalueemap, Injector actuation start MI1_

| | |
|---|---|
| Bloc | `0x1B0A30` → `0x1B0C74` (580 o, alloc `0x244`) |
| Grille | 16 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1B0A34` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1B0A54` — InjCtl_qSetUnBal, ×0.01 [mm^3/cyc] |
| Données Z | `0x1B0A74` — AngleCrS, ×0.023437499999999983 + 0.0 [deg CrS] |
| Plage stock | -5.5078 … 25.2187 deg CrS (raw -235 … 1076) |
| En-tête | `00 10 00 10 01 90 02 26 02 6C 03 E8` |

Axe X : 400 550 620 1000 1200 1400 1600 1800 2000 2400 2800 3200 3600 4000 4200 4400

Axe Y : 0 1 2 5 10 15 20 25 30 35 40 45 50 60 70 80

## InjCrv_phiMI1Bas3_MAP  — SOI
_3. Grundvalueemap, Injector actuation start MI1_

| | |
|---|---|
| Bloc | `0x1B0C74` → `0x1B0EB8` (580 o, alloc `0x244`) |
| Grille | 16 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1B0C78` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1B0C98` — InjCtl_qSetUnBal, ×0.01 [mm^3/cyc] |
| Données Z | `0x1B0CB8` — AngleCrS, ×0.023437499999999983 + 0.0 [deg CrS] |
| Plage stock | -10.5937 … 25.2187 deg CrS (raw -452 … 1076) |
| En-tête | `00 10 00 10 01 90 02 26 02 EE 03 E8` |

Axe X : 400 550 750 1000 1200 1400 1600 1800 2000 2400 2800 3200 3600 4000 4200 4400

Axe Y : 0 1 2 5 10 15 20 25 30 35 40 45 50 60 70 80

## InjCrv_phiMI1Max1_MAP  — SOI
_Map -1 for frühesten Injector actuation start MI1_

| | |
|---|---|
| Bloc | `0x1B1340` → `0x1B1584` (580 o, alloc `0x244`) |
| Grille | 16 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1B1344` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1B1364` — InjCtl_qSetUnBal, ×0.01 [mm^3/cyc] |
| Données Z | `0x1B1384` — AngleCrS, ×0.023437499999999983 + 0.0 [deg CrS] |
| Plage stock | 0.4922 … 27.6328 deg CrS (raw 21 … 1179) |
| En-tête | `00 10 00 10 01 90 02 26 02 EE 03 E8` |

Axe X : 400 550 750 1000 1200 1400 1600 1800 2000 2400 2800 3200 3600 4000 4200 4400

Axe Y : 0 1 2 5 10 15 20 25 30 35 40 45 50 60 70 80

## InjCrv_phiMI1Max2_MAP  — SOI
_Map -2 for frühesten Injector actuation start MI1_

| | |
|---|---|
| Bloc | `0x1B1584` → `0x1B17C8` (580 o, alloc `0x244`) |
| Grille | 16 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x1B1588` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x1B15A8` — InjCtl_qSetUnBal, ×0.01 [mm^3/cyc] |
| Données Z | `0x1B15C8` — AngleCrS, ×0.023437499999999983 + 0.0 [deg CrS] |
| Plage stock | 0.4922 … 25.5937 deg CrS (raw 21 … 1092) |
| En-tête | `00 10 00 10 01 90 02 26 02 EE 03 E8` |

Axe X : 400 550 750 1000 1200 1400 1600 1800 2000 2400 2800 3200 3600 4000 4200 4400

Axe Y : 0 1 2 5 10 15 20 25 30 35 40 45 50 60 70 80

## AirCtl_mDesBas_MAP  — EGR
_Map for Determination of the stationären Basissetpoints for ARF without VE_

| | |
|---|---|
| Bloc | `0x194442` → `0x1945FE` (444 o, alloc `0x1BC`) |
| Grille | 12 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x194446` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x19445E` — InjCtl_qRaw, ×0.01 [mm^3/cyc] |
| Données Z | `0x19447E` — AirMassPerCyl, ×0.1 + 0.0 [mg/Hub] |
| Plage stock | 240.0 … 1060.0 mg/Hub (raw 2400 … 10600) |
| En-tête | `00 0C 00 10 02 6C 03 20 03 E8 04 B0` |

Axe X : 620 800 1000 1200 1400 1600 1800 2000 2400 2800 3200 3300

Axe Y : 0 2 5 7.5 10 12.5 15 17.5 20 22.5 25 27.5 30 35 40 50

## AirCtl_mDesValCor_MAP  — EGR
_Map for Correctionvaluebildung = f(Engine speed, Injection quantity)_

| | |
|---|---|
| Bloc | `0x1949FE` → `0x194BBA` (444 o, alloc `0x1BC`) |
| Grille | 12 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x194A02` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x194A1A` — InjCtl_qRaw, ×0.01 [mm^3/cyc] |
| Données Z | `0x194A3A` — AirMassPerCyl, ×0.1 + 0.0 [mg/Hub] |
| Plage stock | 0.0 … 75.0 mg/Hub (raw 0 … 750) |
| En-tête | `00 0C 00 10 02 A8 03 20 03 E8 04 B0` |

Axe X : 680 800 1000 1200 1400 1600 2000 2400 2800 3200 3600 4000

Axe Y : 0 5 10 12.5 15 17.5 20 22.5 25 27.5 30 35 36 37 38 39

## AirCtl_mNSCor_MAP  — EGR
_Grundmap for Abschätzung the Gang- and Engine temperature dependenten Correction of the EGR-Setpoints_

| | |
|---|---|
| Bloc | `0x194D0E` → `0x194F52` (580 o, alloc `0x244`) |
| Grille | 16 × 16  (lignes = régime, colonnes = charge) |
| Axe X (régime) | `0x194D12` — Eng_nAvrg, ×1.0 [rpm] |
| Axe Y (charge) | `0x194D32` — InjCtl_qRaw, ×0.01 [mm^3/cyc] |
| Données Z | `0x194D52` — AirMassPerCyl, ×0.1 + 0.0 [mg/Hub] |
| Plage stock | 0.0 … 125.0 mg/Hub (raw 0 … 1250) |
| En-tête | `00 10 00 10 02 58 03 20 03 E8 04 B0` |

Axe X : 600 800 1000 1200 1600 2000 2200 2400 2600 2800 3000 3200 3400 3600 3800 4000

Axe Y : 0 2 4 6 8 10 12 14 16 18 20 24 26 28 30 35
