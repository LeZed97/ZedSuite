# ZedSuite-Roadmap

Diese Seite listet auf, was geplant ist, was Nutzer gewünscht haben und was nicht geplant ist.

Für einen Funktionswunsch oder eine Fehlermeldung: ein Issue auf [GitHub](https://github.com/LeZed97/ZedSuite/issues) öffnen oder mich über meine Netzwerke erreichen: [linktr.ee/zedperf](https://linktr.ee/zedperf). Jede Meldung wird gelesen.

## Geplant

- Vergleich der Kennfelder zweier Versionen eines Projekts im Compare-Fenster, das heute den binären Vergleich macht: dasselbe Kennfeld beider Versionen nebeneinander, Unterschiede hervorgehoben.
- 2D-Ansicht im WinOLS-Stil für vollständige Matrizen: eine Kurve pro Zeile, gewählte Zeile hervorgehoben.
- Ein N75-Invertierungsschalter bei EDC15VM, für Fahrzeuge, die von einem Wastegate-Turbo auf VNT oder umgekehrt umgebaut wurden: der Block, der ihn steuert, ist auf den meisten Dateien des Prüfstands lokalisiert, der Schalter selbst ist noch nicht gebaut.
- EDC15P der ersten PD-Generation (1999-2002, 038906019A / 019AJ): die Kennfelderkennung ist fertig und die DTC-Tabelle wird bei den 019AJ gelesen; die Checksumme wird noch nicht unterstützt, die DTC-Tabelle der 019A hat noch ein anderes Layout, und das Kennfeld MAP linearisation wird bei den 019A nicht gefunden.
- Bessere EDC15VM-Erkennung: einige Dateien sind noch nicht vollständig abgedeckt, vor allem der 2.5 V6, sowie die Kennfelder N146 und N75 der Generation 012K / 012AP. Die Korrekturen für die von Nutzern eingesandten Softwarenummern sind drin (einzelnes SOI und MAP/MAF-Schalter des 012M in 1.1.7).
- EDC15VM: am Fahrzeug prüfen, ob die SVRL-Kennfelder wirklich aktiv sind, wenn der Detektor sie findet.
- EDC16U31: bessere Erkennung, es fehlen noch einige EDC16U31-Dateien auf dem Prüfstand, um sie sauber abzuschließen (die 12x12-Familie bei 0x1D7xxx hat noch keinen Namen).
- EDC16U1: Identifikation der Touareg-V10-Dateien, bei denen heute nur eine der beiden Steuergerätenummern gefunden wird (sechs Dateien auf dem Prüfstand).
- Erkennung der PID-Kennfelder der Ladedruckregelung, zuerst bei EDC15P.
- XDF-Kompatibilität (TunerPro-Definitionsdateien, um Kennfeldlisten in diesem Format zu lesen und zu schreiben). Für später geplant, wenn ich die Zeit finde, mich damit zu befassen.

## Von Nutzern gewünscht, in Prüfung

- Rückgängig mit Strg+Z im Editor.
- CSV-Import und -Export (das JSON-Mappack für WinOLS gibt es schon) und DAMOS-Import.
- Favoriten-Kennfelder, für schnellen Zugriff auf die am häufigsten bearbeiteten.
- Eine andere Referenzversion als Ori für „Originalwert“ und den Vergleich.
- Inverse driver wish und MAF linearisation in der Kennfeldliste.
- Kennfelder direkt in der 3D-Ansicht bearbeiten.
- Die Leistungsschätzung an andere Düsen als Firad anpassen, etwa Recambo oder DSSR.
- Zwei Versionen desselben Projekts nebeneinander bearbeiten.
- Mehr Funktionen im Eigenschaften-Fenster der Kennfelder.
- Alle vom Original abweichenden Werte in den Kennfeldfenstern hervorheben.
- Größere Map-Fenster und ein echter 100-%-Zoom auf kleinen Bildschirmen. Heute wächst ein Map-Fenster nicht mehr, sobald seine Zellen ihre Maximalgröße erreichen, was rechts Leerraum lässt, und der Zoom des Editors wird durch die Breite begrenzt, die Werkzeugleiste und Map-Liste verlangen, etwa 75 % auf einem 1024 Pixel breiten Bildschirm. Geplant ist, die Obergrenze der Zellengröße anzuheben und Werkzeugleiste und Liste sparsamer zu machen, damit der Zoom auf jedem Bildschirm 100 % erreicht; eine einstellbare Schriftgröße wird ebenfalls geprüft.

## Vorerst nicht geplant

- Neue Steuergeräte von mir (BMW- und PSA-EDC15/EDC16, EDC17 usw.). Ein Detektor braucht mindestens zwei bis drei Monate und einen großen Bestand an Originaldateien und Mappacks, um richtig zu sein, und ich pflege ZedSuite in meiner Freizeit: Arbeit in dieser Größe kann ich nicht verschenken. Eine Familie kann weiterhin über einen Beitrag kommen, der die Anforderungen aus [CONTRIBUTING.md](CONTRIBUTING.md) erfüllt.
- Automatische Lösungen (EGR off, DPF off, Tuning per Klick). ZedSuite bleibt ein Werkzeug zum Lernen und zum Verstehen der Datei, im Geist von EDCSuite.
- Eine ins Steuergerät gepatchte Kennfeld-Umschaltroutine (Multimap): in WinOLS mit den für EDC15 kursierenden Routinen erledigen und die Datei dann als Version importieren, der zusätzliche Codeblock und seine Kennfelder werden seit 1.1.6 angezeigt.
- Mehr Referenzdaten (Marken, Motoren) für den Importbildschirm.
- Windows-7-Kompatibilität.
