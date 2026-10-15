# Lighthouse y axe-core en el driver
Type: task
Status: open
Blocked by: 
Map: ../map.md

## Question

¿Corren Lighthouse CI y axe-core dentro de la imagen del driver, sin red, y con qué umbrales?

- Imagen `builder-driver` (ticket 05) más Node, `@lhci/cli` y axe-core, instalados al construirla.
- Umbrales del ticket 03: SEO y accesibilidad ≥ 0,9; desempeño solo avisa; axe sin violaciones critical ni serious.
- Aceptación: el driver del prototipo de F2 suma esos checks y su veredicto sigue verde.
