# ZedSuite-Roadmap

Diese Seite zeigt, was geplant ist, was Nutzer gewünscht haben und was nicht geplant ist.

Funktionswunsch oder Fehler: ein Issue auf [GitHub](https://github.com/LeZed97/ZedSuite/issues) eröffnen, im [ecuconnections-Thread](https://www.ecuconnections.com/forum/viewtopic.php?p=393279#p393279) schreiben oder mich über meine sozialen Netzwerke erreichen: [linktr.ee/zedperf](https://linktr.ee/zedperf). Jede Meldung wird gelesen.

## Geplant

- EDC15P der ersten PD-Generation (1999-2002, 038906019A / 019AJ): die Kennfelderkennung ist fertig, Checksumme und DTC-Tabelle dieser Dateien werden noch nicht unterstützt, und das Kennfeld MAP linearisation wird bei den 019A nicht gefunden.
- EDC16U31: bessere Erkennung, für einen sauberen Abschluss fehlen noch einige EDC16U31-Dateien in der Testreihe (die 12x12-Familie bei 0x1D7xxx hat noch keinen Namen).
- EDC16U1: Identifizierung der Touareg V10, bei denen heute nur eine der beiden Nummern gefunden wird (sechs Dateien der Testreihe).
- Bessere EDC15VM-Erkennung: einige Dateien sind noch nicht vollständig abgedeckt, vor allem die 2.5 V6. Für die von Nutzern gemeldeten Softwarestände und Bearbeitungsfehler gibt es bereits Korrekturen.
- EDC15VM: am Fahrzeug prüfen, ob die SVRL-Kennfelder wirklich aktiv sind, wenn die Erkennung sie findet.

## Von Nutzern gewünscht, in Prüfung

- 2D-Ansicht im WinOLS-Stil für ganze Matrizen: eine Kurve pro Zeile, gewählte Zeile hervorgehoben.
- Rückgängig mit Strg+Z im Editor.
- CSV-Import und -Export sowie DAMOS-Import.
- Kennfelder direkt in der 3D-Ansicht bearbeiten.
- Mehrere Versionen desselben Projekts nebeneinander bearbeiten und ihre Kennfelder vergleichen.
- macOS-Kompatibilität.

## Vorerst nicht geplant

- Neue Steuergeräte hinzufügen (EDC15/EDC16 von BMW, PSA usw.).
- Mehr Referenzdaten zu Steuergeräten (Marken, Motoren) für den Importbildschirm.
- Windows-7-Kompatibilität.
