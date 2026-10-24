# Repos-convenciones, ticket 04 (no pisar ediciones manuales): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir cómo distingue el mecanismo un archivo editado a mano de uno viejo sin tocar (`.scratch/repos-convenciones/issues/04-no-pisar-ediciones-manuales.md`, bd `orchestration-4u6`).

Contexto heredado del ticket 03 (resuelto): `--upgrade` aplica solo una lista cerrada de arreglos seguros, y ninguno modifica, mueve ni borra contenido existente. El único arreglo que espera este ticket es "actualizar una plantilla intacta y vieja". Las inserciones en `sdlc.toml` van como texto antes del primer encabezado de tabla, y `tomllib` las verifica.

Hecho (git log, 2026-10-23): `spec-template.md` tuvo una sola versión en toda la historia (`mmorch/templates/` y `sdlc/templates/`, sha1 `c83463c4`). Todas las copias sondeadas en los repos son idénticas a la versión actual.

## Summary / key decisions

- Test de aceptación aprobado: `test_respeta_ediciones` (13 puntos) y `test_lista_de_hashes` (P6). Ticket 04 resuelto.
- Hash con saltos LF; la escritura conserva el estilo de salto del archivo (P6, punto 12).
- Formato de marcas (de `orchestration-ior`, commit `0073a83`): `<!-- BEGIN SDLC <nombre> v:<n> hash:<h> -->` ... `<!-- END SDLC <nombre> -->`, con `h = sha256(interior.strip())[:8]`. Hash distinto = interior editado; `v:` menor al publicado = bloque viejo.
- La línea `@AGENTS.md` de `CLAUDE.md` va sin marcas; `doctor` chequea que exista; si falta, `--upgrade` la agrega al inicio del archivo (arreglo nuevo en la lista del ticket 03) (P5).
- Bloques con marca: inicio y cierre; `--upgrade` reescribe el interior solo si coincide con una versión publicada; interior editado = aviso con diff. El puntero actual del SDLC (sin cierre) da aviso y no se toca (P4).
- `tomlkit` no hace falta: el ticket 03 inserta texto y verifica con `tomllib`.
- Plantilla editada: aviso de `doctor` con la ruta y un diff (`difflib`) contra la plantilla actual; nunca desvío y nunca escribe archivos (P3).
- Mantenimiento de la lista: un test exige el hash de cada plantilla actual en la lista; la lista solo crece y nadie borra un hash viejo (P2).
- Detección de plantilla intacta: opción B. `sdlc/` guarda una lista con el hash de cada versión publicada de cada plantilla. Copia igual a una versión publicada = intacta, y `--upgrade` la reemplaza; copia distinta de todas = editada, y `doctor` la reporta como conflicto (P1).

## Q&A log

### P1 — Cómo detectar una plantilla intacta
- Asked: ¿A (manifiesto por repo), B (lista de hashes de versiones publicadas en `sdlc/`), C (nunca actualizar, solo reportar) o D (`<nombre>.nuevo.md` al lado)?
- Captured: el usuario eligió B.
- Hechos: Claude primero afirmó una sola versión para las tres plantillas habiendo verificado solo `spec-template.md`; después verificó las otras dos. Hashes únicos en la historia: `spec-template.md` `c83463c4`, `spec-review.md` `8848e0ef`, `refutar-tests.md` `06cd8ca2` (sha1, 8 caracteres).
- Decisión: la lista arranca con esos hashes; no agrega archivos a los repos y sirve para los repos adoptados en septiembre.

### P2 — Mantener la lista de hashes
- Asked: ¿A (test que exige el hash actual en la lista), B (generar la lista desde git en cada `doctor`) o C (a mano, sin test)?
- Captured: el usuario eligió A.
- Decisión: el test falla en el mismo commit que cambia una plantilla sin agregar su hash. Regla: la lista solo crece; un hash borrado convierte copias viejas en falsos conflictos.

### P3 — Reporte de una plantilla editada
- Asked: ¿A (solo la ruta), B (ruta + diff con `difflib`) o C (B + `<nombre>.nuevo.md` con una bandera)?
- Captured: el usuario eligió B.
- Decisión: aviso, no desvío, porque una personalización es decisión del usuario. Sin manifiesto por repo, `doctor` no sabe de qué versión partió la copia editada.

### P4 — Bloques con marca
- Asked: ¿A (nunca reescribir; aviso con diff), B (inicio y cierre; reescribir solo un interior publicado) o C (inicio y cierre; reescribir siempre)?
- Captured: el usuario eligió B.
- Decisión: misma regla de hashes que las plantillas. El puntero `## SDLC (pipeline de 6 etapas)` no tiene cierre: aviso de `doctor`, `--upgrade` no lo toca. Claude avisa a la sesión md-estructura (`orchestration-ior`) para que sus bloques de bd usen marcas de inicio y cierre.

### P5 — Marcas en la línea `@AGENTS.md`
- Contexto: la sesión md-estructura adoptó el formato de marcas de bd (hash dentro de la marca) y dejó esta pregunta al ticket 04. Claude adoptó ese formato para P4, porque cumple la opción B sin otra lista para bloques.
- Asked: ¿A (sin marcas; `doctor` chequea que la línea exista) o B (bloque con marcas)?
- Captured: el usuario eligió A, y con eso aprobó el arreglo nuevo "agregar la línea `@AGENTS.md` al inicio de `CLAUDE.md` si falta".
- Decisión: enmienda a la lista cerrada del ticket 03 (registrada en sus `## Comments`).

### P6 — Test de aceptación
- Asked: ¿apruebas el test de 10 puntos? El usuario preguntó si faltaba algo.
- Captured: Claude agregó tres casos: sin bajar de versión (11), saltos CRLF (12) y marca rota o duplicada (13). El usuario aprobó los dos tests con esos puntos.
- Decisión: test aprobado (ver el `## Answer` del ticket 04). Claude avisa a md-estructura que el hash se calcula sobre texto con saltos LF.

## Open flags (pending input)
- Rama propia o árbol actual para aplicar `--upgrade` -> ticket 07.
