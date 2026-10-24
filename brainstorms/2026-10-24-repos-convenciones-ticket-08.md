# Repos-convenciones, ticket 08 (propuesta de áreas): Brainstorm / Discovery Notes
Date: 2026-10-24 · Goal: decidir dónde corre la propuesta de áreas decidida en `sdd-fase-2` y cómo convive con la regla de cero LLM del ticket 03 (`.scratch/repos-convenciones/issues/08-propuesta-de-areas.md`, bd `orchestration-4u6`).

Contexto heredado: `doctor` y `--upgrade` nunca llaman a un LLM (ticket 03, punto 4 del test); las áreas son voluntarias y el mecanismo nunca las escribe solo (ticket 01); `[spec_de_area]` apunta a specs fuera del default (ticket 06). Decisión de `sdd-fase-2` (Q15): propuesta por carpeta sin LLM, ajustes sugeridos por un LLM, aprobación del usuario antes de escribir `[areas]`.

## Summary / key decisions

- Specs planas: la propuesta sigue por carpeta; el archivo de propuesta lista las specs planas sin área; el LLM del paso 2 recibe el título de cada spec plana como contexto; el usuario asigna en el archivo de propuesta (P6).
- Archivo que calza con dos áreas: desvío de `doctor` (nombra el archivo y las dos áreas) y rechazo en `--aprobar` (P5).
- `[areas]` y `[spec_de_area]` van como tablas al final de `sdlc.toml` (las claves sueltas del ticket 03 van antes del primer encabezado).
- Paso 2: `DEFAULT_GENERATOR` (`mmorch/config.py`, hoy `deepseek-chat`) vía `mmorch.providers.call`; devuelve JSON con operaciones `unir`/`partir` y motivo; un validador determinista aplica las válidas; respuesta inválida o falla = propuesta por carpeta sin cambios; el archivo de propuesta anota cada ajuste con su motivo (P4).
- `--aprobar` sigue las reglas del ticket 07: rama default, aborta si `sdlc.toml` está sucio, commitea solo `sdlc.toml`, pushea siempre y borra la propuesta después del commit. Un agente pide OK antes de `--aprobar`; `areas <repo>` sin `--aprobar` no commitea ni pushea (P3).
- Aprobación en dos pasos: `areas <repo>` escribe `docs/sdlc/areas-propuesta.toml`; el usuario puede editarlo; `areas <repo> --aprobar` lo valida con las reglas de `doctor` y lo pasa a `sdlc.toml` (P2).
- Comando aparte `python -m sdlc areas <repo>` con los tres pasos; `doctor` y `--upgrade` siguen sin LLM (P1).

## Q&A log

### P1 — Dónde corre la propuesta
- Asked: ¿A (comando aparte `areas`), B (bandera `--areas` en `--upgrade`) o C (paso 1 en `doctor`, paso 2 aparte)?
- Captured: el usuario eligió A.
- Pendiente: Claude dijo que `doctor` agrega un aviso a cada repo sin áreas; ese aviso puede ser ruido en los diez repos -> revisar en una pregunta posterior.

### P2 — Forma de aprobar
- Asked: ¿A (archivo de propuesta + `--aprobar`), B (imprimir y copiar a mano) o C (diálogo interactivo)?
- Captured: el usuario eligió A.
- Precedente: `docs/sdlc/division.md` (propuesta de división de una tarea grande).

### P3 — Commit y push de `--aprobar`
- Asked: ¿`--aprobar` sigue las reglas del ticket 07?
- Captured: el usuario confirma.

### P4 — Modelo y contrato del paso 2
- Asked: ¿generador barato con JSON de operaciones validado, o otro modelo/contrato?
- Captured: el usuario confirma.

### P5 — Archivo en dos áreas
- Asked: ¿desvío o aviso? El usuario pidió más opciones; Claude listó A (desvío + rechazo), B (rechazo en `--aprobar`, aviso en `doctor`), C (aviso), D (precedencia, cambia la decisión de `sdd-fase-2`), E (sin chequeo, propuesta disjunta).
- Captured: el usuario respondió "Es desvio" (opción A).

### P6 — Specs planas como semilla
- Asked: ¿A (un área por spec plana, globs del LLM), B (listar specs planas sin área) o C (B + títulos como contexto del LLM)?
- Captured: el usuario eligió C.
- Motivo: A deja áreas sin globs si el LLM falla; C conserva la regla "la propuesta por carpeta funciona sola".

## Open flags (pending input)
