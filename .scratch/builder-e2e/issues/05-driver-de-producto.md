# Driver de producto mínimo
Type: prototype
Status: resolved
Blocked by: 03
Map: ../map.md

## Question

¿Un agente puede manejar un producto chico hecho por el SDLC y producir evidencia que sostenga un veredicto determinista?

- Producto de prueba: el que fije el ticket 01.
- El agente produce evidencia; las aserciones dictan el veredicto; el verificador opina en sombra.
- Medir contra una verdad conocida: productos con defectos plantados y productos correctos.
- Métrica: exactitud balanceada del veredicto, como en las mediciones del verificador (README, sección Measured).

## Answer

Resuelto (2026-10-15) con un prototipo en `research/05-driver/`. Sí: un agente con Playwright maneja el sitio real dentro del contenedor sin red y su evidencia sostiene un veredicto determinista.

Montaje:

- Imagen `builder-driver` (`research/05-driver/Dockerfile`): `mcr.microsoft.com/playwright/python:v1.62.0-noble` más el paquete `playwright==1.62.0`, que la imagen oficial no trae. Pesa 2,4 GB.
- El driver sirve el sitio en 127.0.0.1, abre Chromium headless y corre con el mismo `container_argv` del SDLC (`--network none`, `--cap-drop ALL`).
- Verdad: 70 casos ocultos generados por la referencia del ticket 03 (bordes del umbral, del tope y de cada tramo, ±1 peso, más 40 al azar). Los 5 ejemplos públicos de la spec quedan afuera.
- Contrato fijo del walkthrough: `#bruto`, `[data-testid=calcular]`, `[data-testid=neto]`.

Resultado sobre 10 variantes (3 correctas con formatos distintos, 7 con un defecto plantado): exactitud balanceada 1,000; ~6 s por variante.

| Variante | Check que la atrapó |
|---|---|
| bug_sin_tope, bug_tramo, bug_hardcode_publicos | solo casos ocultos |
| bug_aporte_16 | casos ocultos y neto = 0,83 × bruto bajo el umbral |
| bug_consola | consola limpia |
| bug_externo | consola limpia y requests externos |
| bug_meta | metadatos |

Hallazgo: las relaciones metamórficas solas NO atrapan los defectos de cálculo más probables. Una calculadora sin tope, con tramos corridos o con los ejemplos de la spec escritos en el código sigue siendo monótona. Los casos ocultos cargan casi todo el peso del veredicto; las metamórficas son un respaldo.

Límites:

- n = 10, y los defectos los plantó el mismo autor que escribió el driver. La prueba fuerte usa defectos de corridas reales del coder (ticket 04 o el builder).
- Lighthouse y axe-core no se probaron todavía dentro de la imagen.
- El juez en sombra con rúbricas no se armó en este prototipo.
