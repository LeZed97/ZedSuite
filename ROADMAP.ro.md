# Roadmap ZedSuite

Această pagină listează ce este planificat, ce au cerut utilizatorii și ce nu este planificat.

Pentru a cere o funcție sau a raporta un bug: deschideți un issue pe [GitHub](https://github.com/LeZed97/ZedSuite/issues), sau contactați-mă pe rețelele mele sociale: [linktr.ee/zedperf](https://linktr.ee/zedperf). Fiecare raport este citit.

## Planificat

- Compararea hărților a două versiuni ale unui proiect în fereastra Compară, care astăzi face compararea binară: aceeași hartă din ambele versiuni una lângă alta, cu diferențele evidențiate.
- Vizualizare 2D în stil WinOLS pentru matricele complete: o curbă pe fiecare rând, rândul selectat evidențiat.
- Un switch de inversare a N75 pe EDC15VM, pentru mașinile trecute de la un turbo cu wastegate la un VNT sau invers: blocul care îl comandă este localizat pe majoritatea fișierelor din bancul de test, switch-ul în sine nu este încă făcut.
- EDC15P din primele PD (1999-2002, 038906019A / 019AJ): detecția hărților este făcută și tabelul DTC este citit pe 019AJ; checksum-ul nu este încă suportat, tabelul DTC al 019A are încă o altă dispunere, iar harta MAP linearisation nu este găsită pe 019A.
- Detecție EDC15VM mai bună: câteva fișiere nu sunt încă acoperite complet, în special 2.5 V6, precum și hărțile N146 și N75 ale generației 012K / 012AP. Corecțiile sunt făcute pentru numerele de software trimise de utilizatori (SOI unic și switch MAP/MAF al 012M în 1.1.7).
- EDC15VM: de verificat pe mașină că hărțile SVRL sunt cu adevărat active atunci când detectorul le găsește.
- EDC16U31: detecție mai bună, mai lipsesc câteva fișiere EDC16U31 din bancul de test pentru a o termina cum trebuie (familia 12x12 de la 0x1D7xxx nu are încă nume).
- EDC16U1: identificarea fișierelor Touareg V10, la care astăzi este găsit doar unul dintre cele două numere de ECU (șase fișiere în bancul de test).
- Compatibilitate XDF (fișiere de definiție TunerPro, pentru citirea și scrierea listelor de hărți în acest format). Planificat pentru mai târziu, când voi avea timp să mă ocup de asta.

## Cerut de utilizatori, în analiză

- Anulare cu Ctrl+Z în editor.
- Import și export CSV (mappack-ul JSON pentru WinOLS există deja), și import DAMOS.
- Hărți favorite, pentru acces rapid la cele editate cel mai des.
- O versiune de referință alta decât Ori pentru „valoarea originală” și comparare.
- Editarea hărților direct în vizualizarea 3D.
- Ajustarea estimării puterii pentru alte duze decât Firad, precum Recambo sau DSSR.
- Editarea a două versiuni ale aceluiași proiect una lângă alta.
- Mai multe funcții în fereastra Proprietăți a hărților.
- Evidențierea fiecărei valori diferite de cea de origine în ferestrele hărților.

## Neplanificat deocamdată

- Familii noi de ECU făcute de mine (EDC15/EDC16 BMW și PSA, EDC17 etc.). Un detector cere minimum două-trei luni și un corpus mare de fișiere originale și mappack-uri pentru a fi corect, iar eu întrețin ZedSuite în timpul liber: o muncă de asemenea amploare nu este ceva ce aș putea oferi gratuit. O familie poate veni oricând dintr-o contribuție care respectă ștacheta din [CONTRIBUTING.md](CONTRIBUTING.md).
- Soluții automate (EGR off, DPF off, tuning-uri dintr-un clic). ZedSuite rămâne un instrument pentru a învăța și a înțelege fișierul, în spiritul EDCSuite.
- O rutină de comutare a hărților (multimap) injectată în ECU: faceți-o în WinOLS cu rutinele care circulă pentru EDC15, apoi importați fișierul ca versiune; codeblock-ul adăugat și hărțile sale sunt afișate începând cu 1.1.6.
- Mai multe date de referință ECU (mărci, motoare) pentru ecranul de import.
- Compatibilitate cu Windows 7.
