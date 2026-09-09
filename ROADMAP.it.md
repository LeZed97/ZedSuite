# Roadmap di ZedSuite

Questa pagina elenca cosa è previsto, cosa hanno chiesto gli utenti e cosa non è previsto.

Per chiedere una funzione o segnalare un bug: apri una issue su [GitHub](https://github.com/LeZed97/ZedSuite/issues), scrivi nel [thread di ecuconnections](https://www.ecuconnections.com/forum/viewtopic.php?p=393279#p393279), oppure contattami sui miei social: [linktr.ee/zedperf](https://linktr.ee/zedperf). Ogni segnalazione viene letta.

## Previsto

- Confrontare le mappe di due versioni di un progetto nella finestra Compare, che oggi fa il confronto binario: la stessa mappa delle due versioni fianco a fianco, differenze evidenziate.
- Vista 2D in stile WinOLS per le matrici complete: una curva per riga, riga selezionata evidenziata.
- Uno switch di inversione dell'N75 su EDC15VM, per le auto passate da un turbo a wastegate a un VNT o viceversa: il blocco che lo comanda è individuato sulla maggior parte dei file del banco, lo switch in sé non è ancora fatto.
- EDC15P dei primi PD (1999-2002, 038906019A / 019AJ): il rilevamento delle mappe è fatto e la tabella dei DTC viene letta sui 019AJ; il checksum non è ancora supportato, la tabella dei DTC dei 019A usa un'altra disposizione ancora, e la mappa MAP linearisation non viene trovata sui 019A.
- Migliore rilevamento EDC15VM: alcuni file non sono ancora del tutto coperti, il 2.5 V6 in particolare, e le mappe N146 e N75 della generazione 012K / 012AP. Le correzioni ci sono per i numeri di software inviati dagli utenti (SOI singolo e switch MAP/MAF del 012M nella 1.1.7).
- EDC15VM: verificare in auto che le mappe SVRL siano davvero attive quando il rilevatore le trova.
- EDC16U31: rilevamento migliore, mancano ancora alcuni file EDC16U31 nel banco di prova per finirlo bene (la famiglia 12x12 a 0x1D7xxx non ha ancora un nome).
- EDC16U1: identificazione dei file del Touareg V10, dove oggi viene trovato solo uno dei due numeri di ECU (sei file nel banco).
- Rilevamento delle mappe PID del controllo turbo, prima su EDC15P.

## Chiesto dagli utenti, in valutazione

- Annulla con Ctrl+Z nell'editor.
- Importazione ed esportazione CSV (il mappack JSON per WinOLS c'è già), e importazione DAMOS.
- Mappe preferite, per raggiungere in fretta quelle che si modificano di più.
- Una versione di riferimento diversa da Ori per il "valore originale" e il confronto.
- Inverse driver wish e MAF linearisation nell'elenco delle mappe.
- Modificare le mappe direttamente nella vista 3D.
- Adattare la stima di potenza a polverizzatori diversi da Firad, come Recambo o DSSR.
- Modificare due versioni dello stesso progetto fianco a fianco.
- Più funzioni nella finestra Proprietà delle mappe.
- Evidenziare nelle finestre delle mappe tutti i valori diversi dall'originale.
- Ricordare l'orientamento delle mappe (axis mirror): oggi la trasposizione della vista si imposta mappa per mappa e solo nel progetto aperto. L'idea è applicare la scelta a tutta la famiglia di mappe in una volta e salvarla come impostazione globale, così tutti i progetti aprono le mappe allo stesso modo.
- Una versione Linux. L'interfaccia è lo stesso codice di Windows e macOS, resterebbe da fare solo la parte del guscio; dipenderà da quante persone la chiedono.

## Non previsto per ora

- Aggiungere nuove ECU (EDC15/EDC16 BMW e PSA, ecc.).
- Una routine di commutazione mappe (multimap) inserita nella ECU: falla in WinOLS con le routine che circolano per EDC15, poi importa il file come versione, il codeblock aggiunto e le sue mappe sono mostrati dalla 1.1.6.
- Più dati di riferimento sulle ECU (marche, motori) per la schermata di importazione.
- Compatibilità con Windows 7.
