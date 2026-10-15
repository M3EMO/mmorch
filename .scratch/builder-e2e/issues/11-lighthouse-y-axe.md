# Lighthouse y axe-core en el driver
Type: task
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Corren Lighthouse CI y axe-core dentro de la imagen del driver, sin red, y con qué umbrales?

- Imagen `builder-driver` (ticket 05) más Node, `@lhci/cli` y axe-core, instalados al construirla.
- Umbrales del ticket 03: SEO y accesibilidad ≥ 0,9; desempeño solo avisa; axe sin violaciones critical ni serious.
- Aceptación: el driver del prototipo de F2 suma esos checks y su veredicto sigue verde.

## Answer

Resuelto (2026-10-15). Sí corren sin red dentro de `builder-driver`.

- Imagen (`research/05-driver/Dockerfile`): Node 22.23.1 del sitio oficial, Lighthouse 12.8.2 y axe-core 4, instalados al construir.
- axe-core: el driver lo inyecta en la página real; bloquean las violaciones critical y serious.
- Lighthouse: corre contra el mismo servidor local; bloquean SEO y accesibilidad debajo de 0,9; el desempeño solo se informa. Lighthouse ignora `--chrome-path`: hace falta la variable `CHROME_PATH` con el Chromium de Playwright.

Resultados:

- Las 10 variantes del ticket 05 siguen con exactitud balanceada 1,000; las correctas sacan 1,0 en las tres categorías. Cada variante tarda ~15 s (Lighthouse suma ~9 s).
- El sitio de F2 armado por el builder (`prototipo-06`) queda ROJO: axe encuentra una violación critical `label` (el input del bruto no tiene etiqueta). Lighthouse le da accesibilidad 0,91 y lo aprueba: axe es más estricto que el umbral de Lighthouse.

Este es el primer defecto real de un coder que atrapa la capa computable: el contrato de la interfaz no exigía la etiqueta y los tests del módulo pasaban.
