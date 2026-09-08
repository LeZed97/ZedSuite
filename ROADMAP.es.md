# Hoja de ruta de ZedSuite

Esta página recoge lo que está previsto, lo que han pedido los usuarios y lo que no está previsto.

Para pedir una función o informar de un error: abre una issue en [GitHub](https://github.com/LeZed97/ZedSuite/issues), escribe en el [hilo de ecuconnections](https://www.ecuconnections.com/forum/viewtopic.php?p=393279#p393279), o contacta conmigo en mis redes: [linktr.ee/zedperf](https://linktr.ee/zedperf). Todos los mensajes se leen.

## Previsto

- Comparar los mapas de dos versiones de un proyecto en la ventana Compare, que hoy hace la comparación binaria: el mismo mapa de ambas versiones lado a lado, con las diferencias resaltadas.
- Deshacer con Ctrl+Z en el editor.
- Vista 2D al estilo WinOLS para matrices completas: una curva por fila, fila seleccionada resaltada.
- Un switch de inversión del N75 en EDC15VM, para coches que pasan de un turbo de wastegate a uno VNT o al revés: el bloque que lo controla está localizado en la mayoría de los archivos del banco, el switch en sí aún no está hecho.
- DTC del software EDC15 compacto (038906012K, 012L, 012AA, 012AP, 012CP, 019AJ, 019AN): la lectura y la conmutación por ruta de fallo funcionan desde la 1.1.7, pero la conmutación aún no se ha confirmado en un coche. Los comentarios son bienvenidos.
- EDC15P de los primeros PD (1999-2002, 038906019A / 019AJ): la detección de mapas está hecha y la tabla de DTC se lee en los 019AJ; el checksum aún no está soportado, la tabla de DTC de los 019A usa otra disposición más, y el mapa MAP linearisation no se encuentra en los 019A.
- Mejor detección EDC15VM: algunos archivos aún no están del todo cubiertos, el 2.5 V6 (dumps de 1 MB) en particular, y los mapas N146 y N75 de la generación 012K / 012AP. Las correcciones ya están para los números de software que enviaron los usuarios (SOI único y switch MAP/MAF del 012M en la 1.1.7).
- EDC15VM: comprobar en el coche que los mapas SVRL están realmente activos cuando el detector los encuentra.
- EDC16U31: mejor detección, aún faltan algunos archivos EDC16U31 en el banco de pruebas para terminarla bien (la familia 12x12 en 0x1D7xxx sigue sin nombre).
- EDC16U1: identificación de los archivos del Touareg V10, donde hoy solo se encuentra uno de los dos números de ECU (seis archivos en el banco).
- Detección de los mapas PID del control de turbo, primero en EDC15P.

## Pedido por los usuarios, en estudio

- Importación y exportación CSV (el mappack JSON para WinOLS ya existe), e importación DAMOS.
- Mapas favoritos, para acceder rápido a los que más se editan.
- Una versión de referencia distinta de Ori para el "valor original" y la comparación.
- Inverse driver wish y MAF linearisation en la lista de mapas (ocultos a propósito hoy: son tablas de conversión, no mapas de tuning).
- Editar mapas directamente en la vista 3D.
- Editar dos versiones del mismo proyecto lado a lado.
- Más funciones en la ventana de Propiedades de los mapas.
- Resaltar en las ventanas de mapas todos los valores distintos del original: hoy solo se muestran en rojo las celdas que has editado.
- Una versión para Linux. La interfaz es el mismo código que en Windows y macOS, solo habría que hacer la parte del contenedor; dependerá de cuánta gente la pida.
- Ajustar de una vez las duraciones de inyección y los mapas SOI para toberas más grandes (Firad, Recambo…).

## No previsto por ahora

- Añadir nuevas ECU (EDC15/EDC16 de BMW y PSA, etc.).
- Una rutina de conmutación de mapas (multimap) parcheada en la ECU: hazla en WinOLS con las rutinas que circulan para EDC15 e importa después el archivo como versión, el codeblock añadido y sus mapas se muestran desde la 1.1.6.
- Más datos de referencia de ECU (marcas, motores) para la pantalla de importación.
- Compatibilidad con Windows 7.
