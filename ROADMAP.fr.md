# Feuille de route ZedSuite

Cette page liste ce qui est prévu, ce que les utilisateurs ont demandé et ce qui n'est pas prévu.

Pour demander une fonction ou signaler un bug : ouvrez une issue sur [GitHub](https://github.com/LeZed97/ZedSuite/issues), postez dans le [fil ecuconnections](https://www.ecuconnections.com/forum/viewtopic.php?p=393279#p393279), ou contactez-moi sur mes réseaux : [linktr.ee/zedperf](https://linktr.ee/zedperf). Chaque retour est lu.

## Prévu

- Comparer les maps de deux versions d'un projet dans la fenêtre Compare, qui fait aujourd'hui la comparaison binaire : la même map des deux versions côte à côte, différences surlignées.
- Vue 2D façon WinOLS pour les matrices complètes : une courbe par ligne, ligne sélectionnée mise en avant.
- Un switch d'inversion du N75 sur EDC15VM, pour les voitures passées d'un turbo à wastegate à un VNT ou l'inverse : le bloc qui le pilote est localisé sur la plupart des fichiers du banc, le switch lui-même n'est pas encore fait.
- EDC15P des premiers PD (1999-2002, 038906019A / 019AJ) : la détection des maps est faite et la table des DTC est lue sur les 019AJ ; le checksum n'est pas encore supporté, la table des DTC des 019A a encore une autre disposition, et la map MAP linearisation n'est pas trouvée sur les 019A.
- Meilleure détection EDC15VM : quelques fichiers ne sont pas encore entièrement couverts, le 2.5 V6 en particulier, et les maps N146 et N75 de la génération 012K / 012AP. Les corrections sont faites pour les numéros de logiciel envoyés par les utilisateurs (SOI unique et switch MAP/MAF du 012M dans la 1.1.7).
- EDC15VM : vérifier sur véhicule que les maps SVRL sont réellement actives quand le détecteur les trouve.
- EDC16U31 : meilleure détection, il manque encore quelques fichiers EDC16U31 au banc pour finir proprement (la famille 12x12 en 0x1D7xxx n'a toujours pas de nom).
- EDC16U1 : identification des fichiers Touareg V10, où un seul des deux numéros d'ECU est trouvé aujourd'hui (six fichiers au banc).
- Détection des maps PID de régulation de turbo, sur EDC15P d'abord.

## Demandé par les utilisateurs, à l'étude

- Annuler avec Ctrl+Z dans l'éditeur.
- Import et export CSV (le mappack JSON pour WinOLS existe déjà), et import DAMOS.
- Maps favorites, pour retrouver vite celles qu'on modifie le plus.
- Une version de référence autre que l'Ori pour « valeur d'origine » et la comparaison.
- Inverse driver wish et MAF linearisation dans la liste des maps.
- Modifier les maps directement dans la vue 3D.
- Ajuster l'estimation de puissance pour d'autres nez que Firad, comme Recambo ou DSSR.
- Modifier deux versions du même projet côte à côte.
- Plus de fonctions dans la fenêtre Propriétés des maps.
- Surligner dans les fenêtres de maps toutes les valeurs différentes de l'origine.
- Mémoriser l'orientation des maps (axis mirror) : aujourd'hui la transposition d'affichage se règle map par map, et seulement dans le projet ouvert. L'idée est d'appliquer le choix à toute la famille de maps d'un coup et de l'enregistrer comme réglage global, pour que tous les projets ouvrent les maps de la même façon.
- Une version Linux. L'interface est le même code que sur Windows et macOS, seule la partie coque serait à faire ; cela dépendra du nombre de demandes.

## Pas prévu pour l'instant

- Ajouter de nouveaux calculateurs (EDC15/EDC16 BMW et PSA, etc.).
- Une routine de commutation de maps (multimap) patchée dans le calculateur : à faire dans WinOLS avec les routines qui circulent pour EDC15, puis importer le fichier comme version, le codeblock ajouté et ses maps sont affichés depuis la 1.1.6.
- Plus de données de référence (marques, moteurs) pour l'écran d'import.
- Compatibilité Windows 7.
