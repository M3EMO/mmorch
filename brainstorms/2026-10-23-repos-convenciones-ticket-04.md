# Repos-convenciones, ticket 04 (no pisar ediciones manuales): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir cómo distingue el mecanismo un archivo editado a mano de uno viejo sin tocar (`.scratch/repos-convenciones/issues/04-no-pisar-ediciones-manuales.md`, bd `orchestration-4u6`).

Contexto heredado del ticket 03 (resuelto): `--upgrade` aplica solo una lista cerrada de arreglos seguros, y ninguno modifica, mueve ni borra contenido existente. El único arreglo que espera este ticket es "actualizar una plantilla intacta y vieja". Las inserciones en `sdlc.toml` van como texto antes del primer encabezado de tabla, y `tomllib` las verifica.

Hecho (git log, 2026-10-23): `spec-template.md` tuvo una sola versión en toda la historia (`mmorch/templates/` y `sdlc/templates/`, sha1 `c83463c4`). Todas las copias sondeadas en los repos son idénticas a la versión actual.

## Summary / key decisions

- Detección de plantilla intacta: opción B. `sdlc/` guarda una lista con el hash de cada versión publicada de cada plantilla. Copia igual a una versión publicada = intacta, y `--upgrade` la reemplaza; copia distinta de todas = editada, y `doctor` la reporta como conflicto (P1).

## Q&A log

### P1 — Cómo detectar una plantilla intacta
- Asked: ¿A (manifiesto por repo), B (lista de hashes de versiones publicadas en `sdlc/`), C (nunca actualizar, solo reportar) o D (`<nombre>.nuevo.md` al lado)?
- Captured: el usuario eligió B.
- Hechos: Claude primero afirmó una sola versión para las tres plantillas habiendo verificado solo `spec-template.md`; después verificó las otras dos. Hashes únicos en la historia: `spec-template.md` `c83463c4`, `spec-review.md` `8848e0ef`, `refutar-tests.md` `06cd8ca2` (sha1, 8 caracteres).
- Decisión: la lista arranca con esos hashes; no agrega archivos a los repos y sirve para los repos adoptados en septiembre.

## Open flags (pending input)
