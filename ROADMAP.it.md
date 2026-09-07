# Roadmap di ZedSuite

Questa pagina elenca cosa è previsto, cosa hanno chiesto gli utenti e cosa non è previsto.

Per chiedere una funzione o segnalare un bug: apri una issue su [GitHub](https://github.com/LeZed97/ZedSuite/issues), scrivi nel [thread di ecuconnections](https://www.ecuconnections.com/forum/viewtopic.php?p=393279#p393279), oppure contattami sui miei social: [linktr.ee/zedperf](https://linktr.ee/zedperf). Ogni segnalazione viene letta.

## Previsto

- EDC15P dei primi PD (1999-2002, 038906019A / 019AJ): il rilevamento delle mappe è fatto, il checksum e la tabella dei DTC di questi file non sono ancora supportati, e la mappa MAP linearisation non viene trovata sui 019A.
- EDC16U31: miglioramento del rilevamento, mancano ancora alcuni file EDC16U31 nel banco di prova per completarlo correttamente (la famiglia 12x12 a 0x1D7xxx è ancora senza nome).
- EDC16U1: identificazione dei Touareg V10, per i quali oggi viene trovato solo uno dei due numeri (sei file del banco di prova).
- Miglioramento del rilevamento EDC15VM: alcuni file non sono ancora coperti del tutto, in particolare i 2.5 V6. Sono già state apportate correzioni per i numeri di software inviati dagli utenti e per i bug di modifica segnalati.
- EDC15VM: verificare sul veicolo che le mappe SVRL siano davvero attive quando il rilevamento le trova.
- Rilevamento delle mappe PID del controllo della pressione del turbo, prima su EDC15P.

## Chiesto dagli utenti, in valutazione

- Vista 2D in stile WinOLS per le matrici complete: una curva per riga, riga selezionata evidenziata.
- Annullamento con Ctrl+Z nell'editor.
- Importazione ed esportazione CSV, e importazione di DAMOS.
- Modifica delle mappe direttamente nella vista 3D.
- Modifica di più versioni dello stesso progetto affiancate e confronto delle mappe.

## Non previsto per ora

- Aggiunta di nuove centraline (EDC15/EDC16 BMW, PSA, ecc.).
- Più dati di riferimento sulle centraline (marche, motori) per la schermata di importazione.
- Compatibilità con Windows 7.
