# Inventario: qué del SDLC sirve por módulo y qué falta
Type: research
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Qué partes de `sdlc/pipeline.py` y de la skill `/project` sirven sin cambios para construir un módulo, y qué falta para un producto de varios módulos?

- Mapear las 6 etapas, los gates y el fix ladder contra el ciclo de universal-modder.
- Listar los supuestos de una sola feature sobre un repo existente que rompen con un repo vacío.
- Revisar si `runs/` o los logs ya guardan algo reutilizable como nota de campo.

Es AFK: lo corre el agente solo, con lectura de código.

## Answer

Resuelto (2026-10-14) por lectura de código, sin ejecutar nada. El SDLC sirve casi sin cambios para el módulo 2 en adelante. El muro está en el módulo 0 y en el encadenado entre módulos.

Huecos, del que más bloquea al que menos:

1. Bootstrap del repo vacío: `open_worktree` exige HEAD (`mmorch/worktree_driver.py:161`) y `build_feature` exige `accept` (`sdlc/pipeline.py:1839`).
2. Descomposición: `gate_tamano` escribe `division.md` y se detiene (`sdlc/pipeline.py:757`); nada consume ese archivo.
3. Encadenado: cada corrida parte del HEAD del repo (`sdlc/pipeline.py:1774`), no de la rama del módulo anterior.
4. Stack: la spec fija "Python 3.12, sin dependencias" (`sdlc/pipeline.py:1300`) y el plan prohíbe dependencias nuevas (`sdlc/pipeline.py:1362`).
5. Oráculo de integración: hoy existe solo como regresión de la suite total (`gate_suite_total`).
6. Agente que maneja el producto real: no existe.
7. Aprobación humana de la aceptación por módulo (`approve_accept`, `sdlc/pipeline.py:1287`).
8. Nota de campo: el run-log sirve como materia prima, pero no guarda la ruta del repo y nadie lo lee de vuelta.
9. Tests de aceptación que se recolecten sin el código (inferido, sin verificar).

Ciclo de universal-modder contra el SDLC: faltan la rebanada vertical, la verificación en el producto real y la nota de campo. Recon, elección de ruta, assets y empaquetado son parciales.

Sin verificar: la conducta de pytest con una suite vacía y el fallo de `worktree add` sin HEAD.
