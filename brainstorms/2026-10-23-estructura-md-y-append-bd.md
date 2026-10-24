# Estructura de CLAUDE.md/AGENTS.md y append de bd: Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: acordar la estructura de `CLAUDE.md` y `AGENTS.md` en los repos propios y fijar qué hace el comando de append de bd (bd `orchestration-ior`).

## Summary / key decisions
- D1: `AGENTS.md` es la fuente de las reglas comunes y del bloque de bd. `CLAUDE.md` empieza con `@AGENTS.md` y agrega solo lo propio de Claude.
- D2: el bloque de bd queda tal cual lo escribe bd. Las reglas propias de bd viven en una sección fuera de los marcadores, después del bloque, y mandan sobre él.
- D3: una plantilla en orchestration guarda la sección común de reglas propias de bd. El comando la agrega solo si falta el encabezado y nunca la reescribe.
- D4: ante un bloque de bd viejo, el comando actualiza el hash, pero antes mide las reglas propias del usuario dentro del bloque y las reintegra (mecanismo en Q4b).
- D4b: el comando mide las reglas propias con un diff contra el bloque original. El original es la revisión git cuyo cuerpo da el hash del marcador. Sin original en git, el comando sale con código 2 y no escribe.
- D5: los md de agentes pasan a estilo contrato mecánico, con menos prosa. La prosa se deriva a otro md (destino y formato en Q6+).
- D6: `AGENTS.md` tiene secciones fijas en orden fijo: título, `North-star:`, `## Comandos`, `## Reglas`, `## Mapa`, bloque de bd, `## Reglas propias de bd`, `## SDLC`. Cada ítem es una línea. Un lint determinista chequea encabezados, tope de líneas y cero párrafos. `CLAUDE.md` = `@AGENTS.md` + `## Solo Claude`, tope de 60 líneas propias (ver D11).
- D7: la prosa va a `docs/agents/contexto.md` y `docs/agents/gotchas.md`, sin @import. Cada ítem de `## Mapa` lleva un disparador ("antes de X, leé Y"). La prosa atada a un área de código vive solo en `.claude/rules/<área>.md` con `paths:`, y `## Mapa` apunta ahí.
- D8: este issue entrega el contrato escrito (`docs/agents/estructura-md.md`), un lint determinista, el comando de append con test en `tests/` y el skill new-project alineado. La migración de cada repo es una sesión asistida con OK del usuario, dentro de `orchestration-4u6`.
- D9: en `.claude/settings.json`, "no pisar" significa equivalencia semántica. Todo valor previo sigue igual y lo único nuevo es el hook de bd. El reformateo de bd se acepta.
- D10: en un repo sin `.beads/`, el comando corre `bd init --skip-agents --skip-hooks --init-if-missing` y sigue. Los archivos quedan en stage, sin commit.
- D11: topes del lint, sin contar el bloque de bd: `AGENTS.md` ≤ 100 líneas, `CLAUDE.md` ≤ 60 líneas, ítem ≤ 200 caracteres. Cero párrafos y encabezados obligatorios en orden.

## Hechos medidos antes del grilling (bd 1.3.1, repos de juguete con HOME aislado)

- `bd setup claude` ya hace append. Agrega el bloque `<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:1105d646 -->` al final de `CLAUDE.md`. Los bytes previos quedan intactos.
- Si `CLAUDE.md` importa `@AGENTS.md`, `bd setup claude` escribe el bloque en `AGENTS.md` y no toca `CLAUDE.md`.
- Una segunda corrida no cambia ningún byte (sha1 igual).
- `bd setup claude --check` compara solo el hash del marcador. Un cambio de texto dentro del bloque sigue dando "current".
- Con un hash viejo, `--check` da "stale" y falla. `bd setup claude` reemplaza entonces el bloque entero.
- orchestration falla hoy el check: su bloque tiene hash `ca08a54f` y reglas propias del usuario adentro. BetDeportivo tiene el mismo hash viejo y "Adepor Hybrid Rules" adentro.
- La plantilla actual de bd contradice reglas del usuario. Ejemplos: prohíbe TodoWrite y manda `bd remember` en lugar de MEMORY.md.
- `bd setup claude` agrega el hook `SessionStart: bd prime --hook-json` a `.claude/settings.json`. Conserva las otras claves, pero reordena las claves y reformatea el JSON.
- `bd hooks install` conserva el hook git previo del usuario, agrega su sección con marcadores y deja un `.backup`. Respeta `core.hooksPath`.
- mf-moe tiene el bloque de bd duplicado en `CLAUDE.md` y `AGENTS.md`, más un bloque de `bd setup codex`.

### Estado por repo

| Repo | CLAUDE.md | AGENTS.md | Bloque bd | `.beads/` |
|---|---|---|---|---|
| orchestration | 90 líneas | 92 líneas, casi espejo | en los dos, hash viejo, editado | sí |
| mf-moe | 67, índice corto | 86, puntero | en los dos + codex, hash actual | sí |
| BetDeportivo | 130 | no | hash viejo, editado | sí |
| Estudio | 150 | no | no | no |
| Portfolio-financiero | 433 | no | no | no |
| ChatBot, ssb, F2 x4 | no | no | no | no |

## Q&A log
### Q1 — Fuente única de las reglas comunes
- Asked: ¿AGENTS.md fuente, CLAUDE.md fuente o symlink?
- Captured: el usuario eligió "AGENTS.md fuente". Forma: `AGENTS.md` = north-star (3-5 líneas), punteros, bloque de bd, sección de overrides de bd. `CLAUDE.md` = `@AGENTS.md` + sección "Solo Claude" (ruteo cupo/mmorch, skills, hooks).
- Consecuencia medida: con `@AGENTS.md`, `bd setup claude` escribe solo en `AGENTS.md`. Se acaba la edición doble.
### Q2 — Dónde viven las reglas propias de bd
- Asked: ¿sección después del bloque, edición dentro del bloque o archivo aparte?
- Captured: el usuario eligió "Sección después del bloque". El bloque queda intacto y bd lo puede refrescar. La sección `## Reglas propias de bd (mandan sobre el bloque de arriba)` vive fuera de los marcadores.
- Ejemplo de contenido: TodoWrite/TaskCreate valen para planes efímeros; memoria en MEMORY.md y mmorch_remember, nunca bd remember; push = ASK.
### Q3 — Quién escribe la sección común de reglas propias de bd
- Asked: ¿el comando una vez, el usuario a mano, o el CLAUDE.md global?
- Captured: el usuario eligió "El comando, una sola vez". Plantilla en orchestration (nombre tentativo `templates/agents-bd-overrides.md`). Si `AGENTS.md` no tiene `## Reglas propias de bd`, el comando agrega la plantilla. Si ya la tiene, no toca nada, aunque difiera de la plantilla.
- Los agregados por repo (por ejemplo "Adepor Hybrid Rules") quedan como texto del usuario.
### Q4 — Qué hace el comando ante un bloque de bd viejo con reglas propias adentro
- Asked: ¿se niega, mueve y refresca, o lo deja?
- Captured: el usuario respondió con texto propio: "Actualizar el hash pero medir las reglas propias para reintegrarlas".
- Hechos medidos después de la respuesta:
  - El hash del marcador es `sha256(cuerpo.strip())[:8]`. Recalcularlo es un oráculo de "bloque editado o no".
  - orchestration `CLAUDE.md`: marcador `ca08a54f`, cuerpo `b9264fe3` → editado. `AGENTS.md`: cuerpo `c2f54b3f` → editado.
  - BetDeportivo `CLAUDE.md`: marcador `ca08a54f`, cuerpo `2190290c` → editado.
  - mf-moe: marcador y cuerpo `1105d646` → sin editar.
  - La plantilla original de `ca08a54f` existe en el historial git: orchestration `36ad2e3`, BetDeportivo `2482132`. Su cuerpo da exactamente `ca08a54f`.
### Q4b — Cómo mide y reintegra el comando las reglas propias
- Asked: ¿diff contra el original de git, diff contra la plantilla actual, o solo mostrar el diff?
- Captured: el usuario eligió "Diff contra el original". Pasos:
  1. Si `hash(cuerpo) == hash(marcador)`, el bloque está intacto y bd lo refresca.
  2. Si no, `git log -S'hash:<viejo>' -- <archivo>` busca la revisión cuyo cuerpo da el hash viejo. Esa revisión es el original.
  3. Un diff entre el original y el cuerpo actual da las líneas agregadas o cambiadas. Esas líneas son las reglas propias.
  4. El comando agrega `### Reintegradas del bloque <viejo> (<fecha>)` con esas líneas, byte a byte, a `## Reglas propias de bd`.
  5. bd escribe el bloque nuevo.
  6. Sin original en git, el comando sale con código 2 y no escribe nada.
- Consecuencia para el acceptance: los bytes fuera del bloque de bd quedan intactos. Los bytes propios dentro de un bloque viejo cambian de lugar, pero sobreviven byte a byte.
### Q5 — Si el comando reparte la prosa larga entre AGENTS.md y CLAUDE.md
- Asked: ¿solo lo mecánico, también la prosa con LLM, o nada de estructura?
- Captured: el usuario respondió con texto propio: "Quiero reformatear los claude md para que sean menos verbose y menos prosa. Mas estilo contrato mecanico. Para la prosa derivar a otro md."
- Hecho medido: bd detecta `@AGENTS.md` en cualquier línea de `CLAUDE.md`, no solo en la primera.
- Pendiente: formato del contrato, destino de la prosa y quién hace el reformateo (Q6+).
### Q6 — Forma del contrato mecánico
- Asked: ¿secciones fijas con ítems de una línea, tabla clave-valor, o libre pero corto?
- Captured: el usuario eligió "Secciones fijas, ítems de 1 línea". Orden: `# <repo>`, `North-star: <una línea>`, `## Comandos`, `## Reglas`, `## Mapa`, bloque de bd, `## Reglas propias de bd`, `## SDLC`. `## Mapa` apunta a `docs/agents/contexto.md` para la prosa (destino tentativo).
- `CLAUDE.md`: `@AGENTS.md` + `## Solo Claude` (ruteo cupo/mmorch, skills). Tope tentativo: 40 líneas propias.
- Oráculo: lint determinista de encabezados, tope de líneas y cero párrafos.
### Q7 — Destino y carga de la prosa
- Asked: ¿`docs/agents/` sin @import, `docs/claude/` como new-project, o con @import?
- Captured: el usuario eligió la opción 1 (`docs/agents/contexto.md` y `docs/agents/gotchas.md`, sin @import). Preguntó: "no se puede hacer que se derive de claude md a ese contexto si se necesita?"
- Hechos de la doc (code.claude.com/docs/en/memory y /skills): un `@import` siempre carga al inicio. `CLAUDE.md` de subdirectorio carga al tocar archivos de ese subdirectorio. `.claude/rules/*.md` con `paths:` carga al leer o editar un archivo que coincide. Una skill carga su cuerpo solo al invocarse.

### Q7b — Mecanismo de carga bajo demanda
- Asked: ¿puntero con disparador más rules por área, solo puntero, o solo rules?
- Captured: el usuario eligió "Puntero + rules por área". `## Mapa` lleva ítems con disparador, por ejemplo "Antes de cambiar el motor: leé `docs/agents/contexto.md#motor`". La prosa de un área de código va también a `.claude/rules/<área>.md` con `paths:`; Claude la carga sola.
### Q8 — Alcance de este issue
- Asked: ¿contrato + lint + comando, también migrar orchestration como piloto, o sin lint?
- Captured: el usuario eligió "Contrato + lint + comando". Entregables: `docs/agents/estructura-md.md`, lint de `AGENTS.md`/`CLAUDE.md`, comando de append con test, skill new-project alineado.
- Fuera de alcance: migrar la prosa de cada repo. Eso va a `orchestration-4u6` como sesión asistida: diff, OK del usuario, lint verde.
- Aclaración de D7: la prosa de un área vive solo en `.claude/rules/<área>.md`; `## Mapa` apunta a ese archivo, así otros agentes también la leen.
### Q9 — Qué cuenta como "no pisar" en settings.json
- Asked: ¿equivalencia semántica, bytes intactos, o escribir solo si falta el archivo?
- Captured: el usuario eligió "Equivalencia semántica". Oráculo: `json.load` antes y después; después = antes + hook `SessionStart: bd prime --hook-json`. Orden de claves e indentación quedan libres. Si el hook ya existe, el comando no toca el archivo.
- Pendiente de medir en el diseño: si `bd setup claude` duplica el hook cuando el repo ya tiene `bd prime` sin `--hook-json` (caso orchestration y BetDeportivo).
### Q10 — Repo sin `.beads/`
- Asked: ¿inicializa con flags, se niega, o inicializa sin stage?
- Captured: el usuario eligió "Inicializa con flags". Medido: ese `bd init` no toca `CLAUDE.md`, no commitea y hace `git add` de `.beads/` y `.gitignore`. El usuario revisa y commitea.
### Q11 — Topes del lint
- Asked: ¿60/40/160, 100/60/200, o solo cero párrafos?
- Captured: el usuario eligió "100 / 60 / 200". `AGENTS.md` ≤ 100 líneas propias; `CLAUDE.md` ≤ 60 líneas propias; ítem ≤ 200 caracteres. Las líneas del bloque de bd no cuentan.
- Definición de párrafo (propuesta, sin objeción): línea no vacía que no es título, ítem, código, tabla, marcador ni `North-star:`. Un párrafo es error.
- Corrige el tope tentativo de 40 líneas de Q6.
### Q12 — Cierre
- Asked: ¿tocar el bloque de Codex, dónde corre el lint, o nombre y lugar del comando?
- Captured: el usuario respondió "No". Esos tres temas se deciden en la propuesta de diseño, y el usuario la aprueba.

## Hechos medidos después del grilling (para el diseño)

- Un bloque viejo e intacto en `AGENTS.md` se refresca en el mismo lugar. El texto de arriba y de abajo queda igual.
- `bd hooks install` es idempotente: una segunda corrida deja los mismos sha1.
- En el `settings.json` real de orchestration, `bd setup claude` cambia sus propios hooks. Reemplaza `SessionStart: bd prime` por `bd prime --hook-json` y borra `PreCompact: bd prime`. D9 literal ("todo valor previo sigue igual") no se cumple con hooks de bd; la propuesta lo ajusta.

## Propuesta de diseño (respuestas del usuario)

- Diseño: el usuario preguntó "Le falta algo al test?". Se propusieron 8 casos más y el usuario respondió "Sí, los 8": repo sin `.beads/` sin commit nuevo; bloque intacto duplicado (mf-moe); misma edición en los dos bloques (orchestration); sección propia existente y distinta; `CLAUDE.md` sin salto final; repo sin `CLAUDE.md` o sin `AGENTS.md`; `HOME` real intacto; humo sobre clones temporales de orchestration, BetDeportivo y mf-moe.
- D9 ajustado: los hooks de bd son de bd. El usuario eligió "Ambos + Armar mejor los hooks globales":
  - El módulo define en un solo lugar qué hooks son de bd (comandos `bd ...` en `settings.json`, secciones con marcadores de bd en `.git/hooks`). El oráculo exige igualdad exacta del resto.
  - `AGENTS.md` suma `## Hooks`, una línea por hook (Claude y git). El lint exige la sección.
  - "Armar mejor los hooks globales" queda como issue de bd aparte: toca `~/.claude/settings.json`, fuera del repo, y pide su propio grilling.
- new-project: el usuario aprobó editar `~/.claude/skills/new-project/SKILL.md` en este issue.

## Implementación (2026-10-23)

- `sdlc/agentes.py`: `python -m sdlc bd <repo>` y `python -m sdlc lint-md <repo>`. Contrato escrito en `docs/agents/estructura-md.md`.
- Hallazgo: `bd init` commitea cuando git tiene identidad, y ningún flag lo evita. El comando deshace ese commit con `git reset --soft` y deja los archivos en stage (D10).
- Ajuste de D4b: un cambio que solo toca espacios no cuenta como regla propia. Sin el ajuste, orchestration reintegraba casi-duplicados de su `AGENTS.md` con espacios colapsados.
- Humo sobre clones temporales: orchestration reintegra 18 líneas, BetDeportivo 23 (incluye "Adepor Hybrid Rules") y mf-moe ninguna.
- Los tres clones quedan con prefijo intacto, segunda corrida sin cambios, `bd setup claude --check` en 0 y el bloque de bd antes de las reglas propias.
- `lint-md` sobre esos clones da 68, 26 y 19 hallazgos. Es lo esperado: la prosa se migra en `orchestration-4u6`.

## Open flags (pending input)
- Hooks globales (`~/.claude/settings.json`): qué significa "armar mejor" -> usuario, en el issue de bd aparte.
