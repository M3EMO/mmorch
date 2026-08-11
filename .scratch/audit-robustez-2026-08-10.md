# Audit EJE=robustez — 2026-08-10

Read-only sobre `C:\Users\map12\.claude\orchestration` (server MCP, tools, capa de
memoria/estado, plumbing del vault) + pasada liviana sobre hooks y skills globales.
Protocolo barato→caro cumplido: gates estáticos → greps dirigidos → lectura dirigida →
verificación adversarial cross-family de cada candidato.

## Gates estáticos (baseline)

- `ruff check .` → **All checks passed** (0). `mypy mmorch --ignore-missing-imports` →
  **Success: no issues found in 100 source files**. Sin regresión.
- Import de los 100 módulos de `mmorch/` → 0 imports rotos (wiring de módulos sano).

## Nota de método — verificador adversarial

Todos los candidatos pasaron por `mmorch_adversarial_verify` (gemini-3.1-flash-lite,
refuta por default). En 3 casos (F3, F4, F10) las refutaciones **fabricaron código
inexistente** (`check_git_lock_exists` en evolve.py:542, `validate_config_integrity`/
`AUDIT_POLICY_LOAD_FAILURE` en server.py/budget_policy, "logging estándar" en
metrics.read_events) — verificado por lectura directa que esos símbolos no existen.
Conforme al invariante del repo ("Opus desempata"), esos hallazgos sobreviven por
arbitraje del orquestador, con la evidencia de la alucinación registrada. Los hallazgos
cuyas refutaciones fueron sustantivas o de calibración de severidad (F1, F2) se
respetaron y van a Descartados. Esto reconfirma la lección de MEMORY (mmorch quality
gates): el review LLM alucina; el arbitraje necesita ground-truth en código.

## Resumen

| Sev | # |
|---|---|
| BLOCKER | 0 |
| IMPORTANTE | 6 |
| NICE-TO-HAVE | 2 |
| Descartados | 2 |

Tema dominante: **estado persistido en JSON plano con el triple patrón frágil** —
(a) write no atómico (`write_text` directo, sin tmp+`os.replace`), (b) load que traga
corrupción reseteando a default vacío sin log, (c) read-modify-write completo sin lock
entre los DOS procesos que escriben (MCP server + `scripts/nightly.py` vía Task
Scheduler). Cada instancia individual "degrada graceful"; el efecto compuesto es que
varios guardrails y estados aprendidos tienen como modo de falla "desaparecer en
silencio". Las capas SQLite/DuckDB (workflow_store, chat_store, memory) están bien:
conexión+lock+commit, GC atómico documentado.

---

## Hallazgos (rankeados: severidad, luego menor esfuerzo)

### R1 · IMPORTANTE — El gate duro de presupuesto falla ABIERTO y en silencio ante archivo corrupto

**Evidencia:** `mmorch/budget_policy.py:22-26` (`load()` → `except Exception: return []`
sin log; no distingue "no hay políticas" de "no se pudieron leer"), `:30` (`save()` con
`write_text` no atómico → un crash mid-write produce el JSON truncado que `load()`
traga), `mmorch/server_core.py:27-36` (`_budget_block()` → `blocking_incident()` → 402
en creación de jobs: el gate SÍ está cableado y depende de ese load).

**Consecuencia:** un `budget_policies.json` truncado desactiva todos los límites de
gasto configurados (hard-stops incluidos) sin ninguna señal; el sistema sigue gastando
API como si no hubiera límites.

**Fix propuesto:** en `load()` distinguir no-existe (→ `[]` legítimo) de existe-pero-no-
parsea (→ log fuerte y/o incidente hard conservador); `save()` atómico (tmp +
`os.replace`). Esfuerzo bajo.

**Verificación:** refutado 2x por el verificador, ambas con símbolos fabricados
(`validate_config_integrity`, `AUDIT_POLICY_LOAD_FAILURE` — no existen; grep verificado)
y con la falsedad "componente pasivo no cableado" (server_core.py:27-36 prueba lo
contrario). Sobrevive por arbitraje.

### R2 · IMPORTANTE — Una línea corrupta en un .jsonl compartido tumba el reader completo; en metrics.jsonl desactiva el health floor en silencio

**Evidencia:** `mmorch/metrics.py:63-72` (`read_events()` sin tolerancia por línea; el
archivo lo appendean varios procesos), `mmorch/intuition.py:99-125` (`healthy()`
fail-open: `except Exception: return models` → con `read_events` lanzando, ningún modelo
enfermo se filtra, sin señal — el floor existe porque glm-4.6 llegó a 34% de error,
medido 2026-07). Mismo patrón sin tolerancia: `feedback.py:77` (`read_outcomes` — rompe
ECE/reliability_bins/calibrate_conf), `evolve.py:257`, `trajectory.py:128`. El patrón
correcto ya existe en el repo: `arbitration.py:44-47`, `mcp_telemetry.py:74-75`
(try/continue por línea).

**Fix propuesto:** try/continue por línea en los 4 readers (copiar arbitration.py:46).
Esfuerzo mínimo.

**Verificación:** refutación fabricada ("logging estándar en read_events" — no hay
try/except ahí; "estado previo en memoria" — `healthy()` es stateless y `models` es el
pool de entrada sin filtrar). Sobrevive por arbitraje.

### R3 · IMPORTANTE — El lock por archivo del loop nocturno de PRs se pierde en silencio, recreando la carrera que existe para prevenir

**Evidencia:** `mmorch/evolve.py:440-448` (diseño: lock por archivo contra branches
competidores), `:453-458` (`_load_pr_state()` → corrupto → `{}` sin log), `:462-464`
(`_save_pr_state()` no atómico; el loop corre desatendido de noche), `:534-539`
(`coordinated_evolve_round()` decide skip/proceed con ese estado). Pérdida colateral:
los outcomes post-merge (`:496-511`, la señal "más valiosa que el gate") desaparecen
con las entradas.

**Fix propuesto:** write atómico + log fuerte al detectar corrupción en load. Esfuerzo
bajo.

**Verificación:** refutación fabricada (`check_git_lock_exists` en línea 542; la línea
real es `blocked_zone_red.append(c.target)`). Sobrevive por arbitraje.

### R4 · IMPORTANTE — Watermark de destilado (nudge.json) perdible por corrupción o carrera → re-destilado con costo API y notas duplicadas

**Evidencia:** `mmorch/nudge.py:20-26` (corrupto → default sin `distill_upto` → próximo
destilado `after_id=0`, `:50`), `:57-58` (write no atómico), `scripts/nightly.py:102-120`
(el nightly lee y REESCRIBE el mismo archivo, write_text `:120`; comentario `:97-98`
"un solo estado, dos ritmos"). Dos escritores en procesos distintos (MCP server diurno /
nightly 02:10), load→modify→write completo sin lock: last-writer-wins puede retroceder
el watermark sin señal.

**Consecuencia:** `distill_backlog` re-procesa episodios ya destilados → notas
semánticas duplicadas en memory.duckdb + llamadas gen+verify repetidas (hasta 50/noche).

**Fix propuesto:** write atómico + preservar `distill_upto` ante corrupción, o mover el
watermark a memory.duckdb (transaccionalidad ya disponible). Esfuerzo bajo.

**Verificación:** PASSED (confidence 0.9).

### R5 · IMPORTANTE — Estado de los bandits (routing aprendido): reset silencioso ante corrupción + write no atómico + carrera inter-proceso

**Evidencia:** `mmorch/feedback.py:88-92` (`ThompsonBandit.__init__`: corrupto →
`self._arms = {}` sin log — todos los posteriors Beta se pierden), `:119-120` (`update()`
reescribe `bandit_state.json` completo, no atómico). Concurrencia: MCP server y
`scripts/nightly.py` (vía `record_outcome` → `intuition.record` y `reap_merged_prs`)
escriben los mismos archivos de estado con load→modify→write sin lock inter-proceso:
updates concurrentes se pierden (last-writer-wins). Agravante: el bandit ya está starved
(n≤3/brazo tras 10k calls, audit 2026-07) — cada outcome perdido pesa.

**Fix propuesto:** write atómico + log en corrupción; para la carrera, file-lock
(msvcrt/portalocker) o mover a SQLite (patrón workflow_store.py). Esfuerzo bajo-medio.

**Verificación:** PASSED (confidence 0.95, las tres "refutaciones" confirman el
hallazgo).

### R6 · IMPORTANTE — vault.write_note pisa notas existentes en silencio ante colisión de slug (fuente de verdad sobreescribible)

**Evidencia:** `mmorch/vault.py:16-18` (`_slug`: lowercase, sin puntuación, trunca a 60
chars — colisiones deterministas posibles), `:31-33` (`write_note` hace `write_text`
directo sin chequear existencia), `:81-114` (`write_validated`, la "single validated
door" según mcp_server.py:366, delega sin protección; `log_op` registra "write" igual
para overwrite que para alta). Mitigación parcial: el vault vive en el repo git, pero el
write no commitea — el estado entre commits es volátil.

**Consecuencia:** dos títulos que normalizan al mismo slug, o re-usar un título, pierde
el contenido anterior sin backup, warning ni registro distinguible.

**Fix propuesto:** si el path existe y el contenido difiere → sufijo `-2`/`-3` o error
explícito, + `log_op("overwrite", ...)`. Esfuerzo bajo.

**Verificación:** PASSED (confidence 0.9).

### R7 · NICE-TO-HAVE — Memo cache frágil: reset silencioso + rewrite completo por put + lock solo-thread

**Evidencia:** `mmorch/cache.py:25-29` (corrupto → `{}` silencioso: se pierde el cache
de verdicts pagados → re-gasto API), `:38` (reescritura completa no atómica por cada
`put`; memo.json ~100KB), `:13` (`_LOCK` es `threading.Lock`, no cubre multi-proceso).
Impacto acotado: cache regenerable, nunca fuente de verdad.

**Fix propuesto:** write atómico; si crece, SQLite. Esfuerzo bajo.
**Verificación:** PASSED.

### R8 · NICE-TO-HAVE — Checkpoints de resumabilidad best-effort sin señal cuando fallan

**Evidencia:** `mmorch/server_engine.py:43-48, 53-56` (y réplicas en `:155, :343-344,
:384-385`): si `workflow_store` falla, el job sigue (deliberado, comentado) pero el
estado resumible no se persiste y NADIE se entera — un resume posterior re-arranca de
step 0 re-pagando pasos. El mecanismo `emit()` ya existe en el archivo.

**Fix propuesto:** `emit("job","warn",...)` en el except de checkpoint/spec. Esfuerzo
mínimo.
**Verificación:** PASSED.

---

## Apéndice: Descartados (refutados en verificación)

- **F1 — nightly sin detección de no-ejecución** (task `mmorch-nightly` con
  LastTaskResult=0x800710E0, `StartWhenAvailable=False`, último registro en
  nightly.jsonl 2026-08-06 con máquina activa el 08-10; el resumidor de 09:00 solo lee
  el log y no alerta por ausencia; el script no escribe marca de inicio). Refutado 2x:
  "problema de configuración de infra + gap de observabilidad de despliegue, no de
  robustez del código". Los HECHOS quedan como señal operativa: la pata nocturna lleva
  4 noches sin correr y nadie lo notó — ver handoff.
- **F2 — el "nightly sweep" de babel no existe** (mcp_server.py:370-371,394 lo declara
  safety net del thread daemon con `except: pass`; scripts/nightly.py no tiene esa pata
  y babel.py no tiene función de sweep). Refutado 2x: "docstring desactualizado /
  derivado regenerable, no bug de robustez" — con la premisa fáctica CONCEDIDA
  ("técnicamente correcta"). Queda como señal de wiring-drift documental para otro eje.
