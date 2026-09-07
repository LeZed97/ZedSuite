# Hoja de ruta de ZedSuite

Esta página recoge lo que está previsto, lo que han pedido los usuarios y lo que no está previsto.

Para pedir una función o informar de un fallo: abre una issue en [GitHub](https://github.com/LeZed97/ZedSuite/issues), escribe en el [hilo de ecuconnections](https://www.ecuconnections.com/forum/viewtopic.php?p=393279#p393279), o contacta conmigo en mis redes sociales: [linktr.ee/zedperf](https://linktr.ee/zedperf). Se lee cada mensaje.

## Previsto

- EDC15P de los primeros PD (1999-2002, 038906019A / 019AJ): la detección de mapas está hecha, el checksum y la tabla de DTC de estos archivos aún no están soportados, y el mapa MAP linearisation no se encuentra en los 019A.
- EDC16U31: mejora de la detección, faltan algunos archivos EDC16U31 en el banco de pruebas para terminarla correctamente (la familia 12x12 en 0x1D7xxx sigue sin nombre).
- EDC16U1: identificación de los Touareg V10, en los que hoy solo se encuentra uno de los dos números (seis archivos del banco de pruebas).
- Mejora de la detección EDC15VM: algunos archivos aún no están completamente cubiertos, sobre todo los 2.5 V6. Ya se han corregido los números de software que me han enviado los usuarios y los fallos de edición que han señalado.
- EDC15VM: comprobar en el vehículo que los mapas SVRL están realmente activos cuando la detección los encuentra.
- Detección de los mapas PID del control de presión del turbo, primero en EDC15P.

## Pedido por los usuarios, en estudio

- Vista 2D al estilo WinOLS para matrices completas: una curva por fila, fila seleccionada resaltada.
- Deshacer con Ctrl+Z en el editor.
- Importación y exportación CSV, e importación de DAMOS.
- Edición de los mapas directamente en la vista 3D.
- Edición de varias versiones del mismo proyecto lado a lado y comparación de sus mapas.

## No previsto por ahora

- Añadir nuevas centralitas (EDC15/EDC16 de BMW, PSA, etc.).
- Más datos de referencia de centralitas (marcas, motores) para la pantalla de importación.
- Compatibilidad con Windows 7.
