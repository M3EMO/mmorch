# Poda 2026-08-30 — plan por evidencia

Origen: review externo + análisis de Cursor + medición propia. Cada ítem lleva **cómo se
verificó**. Lo que no pude verificar está en T3, no en T1.

Herramientas nuevas de este esfuerzo: `tools/dead-modules.py` (ratchet por módulo) +
`tools/dormant-modules.txt` + `tests/test_dead_modules.py`.

**La vara no es líneas borradas.** `.scratch/` son 2.619 líneas y cero carga cognitiva:
nadie lo carga en la cabeza ni el modelo lo lee. Los 32 tools MCP fríos son 375 líneas y
se pagan **en cada turno**, en el listado que el orquestador lee antes de decidir. T2
vale más que T3 aunque borre menos.

---

## T1 — Borrar. Evidencia dura, consenso de 3 vías.

| Qué | Líneas | Evidencia |
|---|---|---|
| `mmorch/debate.py` | 317 | Cero importers (`dead-modules.py`). Se midió: aprueba 11/11, empata con no llamar a nadie. |
| `mmorch/refutar.py` + `tests/test_refutar.py` | 174 | Solo lo importa su test. Se midió: refuta 4/4, bloquearía todo. Marcado ADVISORY en su propio commit. |
| `mmorch/ablation.py` | 98 | Cero importers. `run_ablation` ya está en `_DEUDA_MUSEO`. Superado por `ablation_*.py` de la raíz (n=350, POWERED). |
| `docs/fable-workflow.md` | 136 | **Byte-idéntico** a la copia del vault (`diff` limpio). |
| `docs/paperclip-grafts.md` | 108 | **Byte-idéntico** a la copia del vault. |
| **Total** | **833** | |

**Por qué borrar `debate`/`refutar` no pierde el conocimiento:** las dos mediciones ya
están transcritas, con sus números, en el docstring de `mmorch/regresion.py` — el que
ganó. El código es redundante con su propio epitafio.

Los tres son brazos perdedores de experimentos medidos, no features a medio hacer. Es el
mismo movimiento que ya hiciste con los checkers y con `self_evolve`: se midió, perdió,
ganó el mecanismo determinista. Falta el último paso, que es apagar al perdedor.

Nota: la poda de los duplicados ya estaba decidida en
`.scratch/vault-global/issues/02-criterio-curacion-y-pointers.md` y nunca se ejecutó.

---

## T2 — HECHO (2026-08-31). La superficie que se paga cada turno.

**Ejecutado distinto de como estaba planeado.** El plan decía "sacar `@mcp.tool()`
de 32 tools". Al abrir `mcp_server.py` apareció que **el mecanismo ya existía**:
`MMORCH_MCP_PROFILE` (W2.2, 08-27), con un set `core` de 38 tools curado a ojo
para el techo de ~40 de Cursor, y `full` de default.

Lo que se hizo en vez de eso:

1. `_NOT_IN_CORE` pasó de 9 entradas curadas a ojo → **32 derivadas de la
   telemetría**. `core` queda en **15 tools**.
2. **Default flipeado de `full` a `core`.** Ahí está el 47 → 15 real: sin esto
   nada cambiaba para la sesión, que corría con el default.
3. Fix de borde: `MMORCH_MCP_PROFILE=""` caía en `full`. Antes daba lo mismo
   porque el default *era* full; ahora cae en `core` como corresponde.
4. `docs/cursor-setup.md` reescrito, y la tabla de 9 exclusiones reemplazada por
   un puntero a `_NOT_IN_CORE` — 32 filas duplicadas en un doc driftan seguro.
5. `test_default_es_full` → `test_default_es_core`; `test_core_curado_...` →
   `test_core_es_el_set_con_uso_medido`, que congela `core` contra la telemetría.
6. `test_mcp_contract.py` y `test_mcp_schema.py` pineados a `full`: prueban el
   contrato del **catálogo**, no del perfil. Sin eso, las 32 fuera de `core`
   dejaban de estar cubiertas por el contrato de error y por el freeze de schema.

**Riesgo asumido, explícito:** la ventana de telemetría (07-08 → 08-30) es casi
toda anterior a la integración con Cursor, que es del 08-27. `fan_out`, `cascade`
y `error_rates` se cortaron sin que su flujo haya tenido chance de correr. La
telemetría sigue loggeando: si ese flujo los usa, aparece y se revierte sacándolos
de `_NOT_IN_CORE`. Decisión del usuario tras plantearle el conflicto.

### Evidencia original

**32 tools nunca invocadas. NO se borra la función de librería.**

Telemetría `logs/mcp_calls.jsonl`, 53 días (07-08 → 08-30), 269 llamadas, 47 tools:

- **11 tools** se invocaron alguna vez.
- **5 tools = 261 de 269 llamadas (97%)**: `budget_status` (136), `record_outcome` (52),
  `review_code` (39), `adversarial_verify` (27), `vault_write` (7).
- **6 tools** se invocaron exactamente 1 vez (el smoke test del día que se agregaron).
- **36 nunca**. Fechadas contra la historia real: `route`, `fan_out`, `tournament`,
  `cascade`, `classify`, `learn`, `memory_stats` son del **2026-06-07**, el día fundacional.
  No son nuevas.

**Se sacan 32.** Exentas 4:

- `mmorch_canal` — nació hoy, no tuvo ventana.
- `mmorch_build_spec`, `mmorch_route`, `mmorch_spec_interview` — los nombra
  `~/.claude/skills/perfect/SKILL.md`. La skill nunca corrió desde el 07-08, pero
  sacarlas la rompe. Decisión aparte: retirar la skill, o dejarlas.

Efecto: superficie MCP **47 → 15**, y 375 líneas menos en `mcp_server.py` (35% del
archivo). Reversible con una línea por tool el día que una haga falta.

Ninguna otra skill nombra un tool `mmorch_*`: el grep sobre `~/.claude/skills/` y
`skills/` devuelve solo 6 distintos (`review_code`, `cynefin`, `build_spec`,
`vault_write`, `spec_interview`, `route`). `mmorch_autoresearch` y `mmorch_speedup` **no**
son el backend de `/autoresearch` ni `/speedup` — esas skills manejan la librería por otro
lado. Son superficie huérfana.

---

## T3 — Revisar antes de borrar. Evidencia parcial.

| Qué | Líneas | Qué falta decidir |
|---|---|---|
| `.scratch/` (56 archivos) | 2.619 | 3 esfuerzos (`audit-2026-08` 23, `vault-global` 12, `loop-cerrado` 9) + 12 sueltos. Regla propia: lo que sobrevive al esfuerzo se **promueve a bd**, el resto se va. Hay que leer los 3 esfuerzos y ver qué quedó abierto. |
| `ablation_paired/prompt/symmetric.py` (raíz) | 1.002 | Son los que **ganaron**. El experimento §18.4 está cerrado y el resultado vive en README. ¿Archivo o borrado? Distinto de `mmorch/ablation.py` (T1), que perdió. |
| `skills/pocock/` | 876 | Copia vendorizada. **Verifiqué: ningún hook la referencia** (`~/.claude/settings.json` ni `~/.claude/hooks/`). Falta confirmar que no la use una skill. |
| `docs/superpowers/` | 838 | Cursor dice "plan ya completado". **No lo verifiqué.** |
| `SELF-EVOLUTION-PLAN.md` | 296 | ¿Plan vivo o histórico? El north star cambió a "librería con nightly gated". |
| `AUDIT_2026-06-07.md` | 103 | Auditoría de hace ~3 meses. ¿Sus hallazgos están cerrados? |
| `HANDOFF.md` | 107 | Handoff de sesión, por definición efímero. |
| `HERMES-IDEAS.md` | 64 | Brainstorm. Candidato a vault, no a raíz. |
| `ALGORITHMS-MAP.md` | 58 | ¿Lo reemplazó `docs/generated/catalog.md`? |
| `docs/intuition-layer.md` | 167 | Difiere del vault **solo en frontmatter**. Corregir las 2 referencias (ambas en `.scratch/vault-global/`) y borrar. |
| 14 funciones en `_DEUDA_MUSEO` | — | `tests/test_no_museum.py`. Decisión individual: cablear o borrar. |

---

## T4 — NO tocar. Falsos positivos verificados.

- **`mcp_server.py` en la raíz** (5 líneas). Parece resto de la mudanza W2.1. **Está vivo:**
  `~/.claude.json:1665` apunta ahí. Es el shim de compat.
- **`logs/` y `__pycache__`**. Cursor propuso "borrarlos sin versionarlos": `git ls-files`
  devuelve **0** archivos en ambos. Ya están fuera de git, no hay nada que hacer.
- **`mmorch/context_blocks.py`** (244). Sin caller Python, pero lo invoca
  `~/.claude/hooks/context-block-watch.js` con `python -m`. Ya está en `dormant-modules.txt`.
- **`mmorch/plugin_worker.py`**. `plugins.py:27` lo lanza por path, nunca lo importa — el
  aislamiento es el punto. Ya está en `dormant-modules.txt`.

---

## Orden sugerido

1. **T1** — 833 líneas, riesgo cero, consenso de tres análisis independientes.
2. **T2** — el que de verdad baja la carga por turno. 47 → 15 tools.
3. Correr `tools/dead-modules.py` + `tests/test_no_museum.py` y wirearlos a pre-commit
   junto a ruff/mypy. Recién con T1 hecho el ratchet queda en verde.
4. **T3** por ítem, cada uno con su verificación.

## Lo que este plan NO propone

Congelar el conteo de módulos, que era la recomendación del review externo. Es una regla
que se rutea: metés lo nuevo en un módulo existente y el conteo no se mueve. Los dos
ratchets (`test_no_museum.py` por función, `dead-modules.py` por módulo) miden lo que
importa — si algo tiene caller — en vez de un número que se puede gamear.

Medido: **129 de 134 módulos tienen callers reales fuera de tests.** El "campus" del review
casi no existe a nivel módulo. Existe, y fuerte, a nivel **superficie MCP**.
