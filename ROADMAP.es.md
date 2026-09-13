# Hoja de ruta de ZedSuite

Esta página recoge lo que está previsto, lo que han pedido los usuarios y lo que no está previsto.

Para pedir una función o informar de un error: abre una issue en [GitHub](https://github.com/LeZed97/ZedSuite/issues), o contacta conmigo en mis redes: [linktr.ee/zedperf](https://linktr.ee/zedperf). Todos los mensajes se leen.

## Previsto

- Comparar los mapas de dos versiones de un proyecto en la ventana Compare, que hoy hace la comparación binaria: el mismo mapa de ambas versiones lado a lado, con las diferencias resaltadas.
- Vista 2D al estilo WinOLS para matrices completas: una curva por fila, fila seleccionada resaltada.
- Un switch de inversión del N75 en EDC15VM, para coches que pasan de un turbo de wastegate a uno VNT o al revés: el bloque que lo controla está localizado en la mayoría de los archivos del banco, el switch en sí aún no está hecho.
- EDC15P de los primeros PD (1999-2002, 038906019A / 019AJ): la detección de mapas está hecha y la tabla de DTC se lee en los 019AJ; el checksum aún no está soportado, la tabla de DTC de los 019A usa otra disposición más, y el mapa MAP linearisation no se encuentra en los 019A.
- Mejor detección EDC15VM: algunos archivos aún no están del todo cubiertos, el 2.5 V6 en particular, y los mapas N146 y N75 de la generación 012K / 012AP. Las correcciones ya están para los números de software que enviaron los usuarios (SOI único y switch MAP/MAF del 012M en la 1.1.7).
- EDC15VM: comprobar en el coche que los mapas SVRL están realmente activos cuando el detector los encuentra.
- EDC16U31: mejor detección, aún faltan algunos archivos EDC16U31 en el banco de pruebas para terminarla bien (la familia 12x12 en 0x1D7xxx sigue sin nombre).
- EDC16U1: identificación de los archivos del Touareg V10, donde hoy solo se encuentra uno de los dos números de ECU (seis archivos en el banco).
- Detección de los mapas PID del control de turbo, primero en EDC15P.
- Compatibilidad XDF (archivos de definición de TunerPro, para leer y escribir listas de mapas en ese formato). Previsto para más adelante, cuando tenga tiempo de dedicarme a ello.

## Pedido por los usuarios, en estudio

- Deshacer con Ctrl+Z en el editor.
- Importación y exportación CSV (el mappack JSON para WinOLS ya existe), e importación DAMOS.
- Mapas favoritos, para acceder rápido a los que más se editan.
- Una versión de referencia distinta de Ori para el "valor original" y la comparación.
- Inverse driver wish y MAF linearisation en la lista de mapas.
- Editar mapas directamente en la vista 3D.
- Ajustar la estimación de potencia para toberas distintas de Firad, como Recambo o DSSR.
- Editar dos versiones del mismo proyecto lado a lado.
- Más funciones en la ventana de Propiedades de los mapas.
- Resaltar en las ventanas de mapas todos los valores distintos del original.
- Ventanas de mapas más grandes y un zoom real al 100 % en pantallas pequeñas. Hoy una ventana de mapa deja de crecer cuando sus celdas alcanzan su tamaño máximo, lo que deja espacio vacío a la derecha, y el zoom del editor está limitado por el ancho que exigen la barra de herramientas y la lista de mapas, alrededor del 75 % en una pantalla de 1024 píxeles de ancho. La idea es subir el tope del tamaño de celda y hacer que la barra de herramientas y la lista ocupen menos, para que el zoom llegue al 100 % en cualquier pantalla; también se estudia un tamaño de fuente ajustable.

## No previsto por ahora

- Nuevas ECU hechas por mí (EDC15/EDC16 de BMW y PSA, EDC17, etc.). Un detector necesita dos o tres meses como mínimo y un gran corpus de archivos originales y mappacks para ser correcto, y mantengo ZedSuite en mi tiempo libre: un trabajo de ese tamaño no es algo que pueda regalar. Una familia puede llegar igualmente por una contribución que cumpla el nivel exigido en [CONTRIBUTING.md](CONTRIBUTING.md).
- Soluciones automáticas (EGR off, DPF off, stage con un clic). ZedSuite sigue siendo una herramienta para aprender y entender el archivo, en el espíritu de EDCSuite.
- Una rutina de conmutación de mapas (multimap) parcheada en la ECU: hazla en WinOLS con las rutinas que circulan para EDC15 e importa después el archivo como versión, el codeblock añadido y sus mapas se muestran desde la 1.1.6.
- Más datos de referencia de ECU (marcas, motores) para la pantalla de importación.
- Compatibilidad con Windows 7.
