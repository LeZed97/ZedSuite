# Feuille de route ZedSuite

Cette page liste ce qui est prévu, ce que les utilisateurs ont demandé et ce qui n'est pas prévu.

Pour demander une fonction ou signaler un bug : ouvrez une issue sur [GitHub](https://github.com/LeZed97/ZedSuite/issues), postez dans le [fil ecuconnections](https://www.ecuconnections.com/forum/viewtopic.php?p=393279#p393279), ou contactez-moi via mes réseaux sociaux : [linktr.ee/zedperf](https://linktr.ee/zedperf). Chaque retour est lu.

## Prévu

- EDC15P des premiers PD (1999-2002, 038906019A / 019AJ) : la détection des maps est faite, le checksum et la table des DTC de ces fichiers ne sont pas encore supportés, et la map MAP linearisation n'est pas trouvée sur les 019A.
- EDC16U31 : amélioration de la détection, il manque quelques fichiers EDC16U31 dans le banc de test pour la terminer correctement (la famille 12x12 en 0x1D7xxx n'a toujours pas de nom).
- EDC16U1 : identification des Touareg V10, dont un seul des deux numéros est trouvé aujourd'hui (six fichiers du banc).
- Amélioration de la détection EDC15VM : quelques fichiers ne sont pas encore complètement couverts, notamment les 2.5 V6. Des corrections ont déjà été apportées pour les numéros de software que les utilisateurs m'ont fait remonter, ainsi que pour les bugs d'édition signalés.
- EDC15VM : vérifier sur véhicule que les maps SVRL sont bien actives lorsque la détection les trouve.
- Détection des maps PID de la régulation de suralimentation (turbo), sur EDC15P d'abord.

## Demandé par les utilisateurs, à l'étude

- Vue 2D façon WinOLS pour les matrices complètes : une courbe par ligne, ligne sélectionnée mise en avant.
- Annulation avec Ctrl+Z dans l'éditeur.
- Import et export CSV, et import de damos.
- Édition des maps directement dans la vue 3D.
- Édition de plusieurs versions du même projet côte à côte et comparaison des maps.

## Pas prévu pour l'instant

- L'ajout de nouveaux calculateurs (EDC15/EDC16 BMW, PSA, etc.).
- Plus de données de référence calculateurs (marques, moteurs) pour l'écran d'import.
- Compatibilité Windows 7.
