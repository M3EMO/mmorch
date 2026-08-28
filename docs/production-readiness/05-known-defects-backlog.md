# 05 — Defectos conocidos y backlog (consolidado)

**Fecha:** 2026-08-27 · **Alcance:** todo lo YA sabido sobre defectos y deuda en mmorch,
deduplicado y rankeado por severidad para production-readiness. **No se editó código.**

## Fuentes consolidadas

| Fuente | Qué aporta | Estado de la fuente |
|---|---|---|
| `AUDIT_2026-06-07.md` | self-audit cross-family H-1..H-8 | H-1/2/3/5/6 aplicados; H-4/7/8 verificados HOY contra código |
| Blind-spot audit 2026-07-05 (memoria `mmorch-quantized-analysis`) | 9 riesgos sistémicos triageados | mayormente ABIERTOS |
| `vault/research/auditoria-mmorch_*.md` (10 notas, 2026-08-19→25) | auditoría nocturna por módulo (A→B alfabético), findings refutados 5/5-8/8 | findings sin aplicar (advisory) |
| `bd list` / `bd ready` (beads) | 6 issues durables P3-P4 | 5 open, 1 in-progress, todos blocked |
| `HANDOFF.md`, memorias `loop-cerrado-spec`, `lotus-app`, `libs-research-2026-07` | pendientes operativos | mixto |
| grep `TODO|FIXME|XXX|HACK` en `mmorch/` | **CERO marcadores reales** (los hits son "TODO"=«todo» en español) | higiene limpia |
| `ALGORITHMS-MAP.md`, `WEIGHTS.md`, `SELF-EVOLUTION-PLAN.md` §BACKLOG | seeds ⏳ gateados por problema medido — NO son deuda | informativo |

---

## Tabla maestra de defectos y deuda (dedup, rankeada)

Severidad = impacto en confiabilidad de producción (runs desatendidos, nightly, server).
Estado verificado contra el código HEAD (`90f18e9`) donde se indica.

### SEV-1 · ALTA — riesgo real en runs desatendidos

| # | Defecto / deuda | Fuente | Evidencia (file:line) | Estado |
|---|---|---|---|---|
| 1 | **Worktree = aislamiento GIT, no sandbox de OS**: `test_cmd` propuesto por LLM corre con permisos completos del usuario. Aceptable local-scale, incorrecto para autonomía escalada (nightly auto-evolve ya corre solo). | Blind-spot #3 (2026-07-05) | `evolve.py` / project-build worktrees | ABIERTO |
| 2 | **Sin circuit-breaker de dólares por-run dentro de builds recursivos** — un build patológico puede quemar presupuesto sin corte interno. | Blind-spot #6 | `project_build.py` | ABIERTO |
| 3 | **Horizonte de outcomes cerrado en build-gates**: el aprendizaje (bandit/record_outcome) nunca recibe la realidad post-merge; lo mergeado que falla después no retroalimenta. | Blind-spot #1 (TOP del triage) | flujo `record_outcome` ↔ gates | ABIERTO |
| 4 | **H-7 · Sin retry/backoff en errores transitorios de API** (429/5xx → fallo duro). Verificado hoy: `providers.py` clasifica errores (rate_limit/timeout, líneas 38-49) pero NO reintenta; el único retry existente es de JSON-parse (`schema.py:91-129`) y `propose_with_fast_retry` (`evolve.py:662`), no del provider. | AUDIT H-7 | `mmorch/providers.py` (sin `backoff`/`retry`) | ABIERTO |
| 5 | **`bucket_rank`: excepción en `f.result()` del pool aborta TODO el ranking** y pierde los resultados ya procesados (el `except` vive en `_job`, no en el loop `as_completed`). Mismo patrón que el H-1 ya fixeado en `fan_out`. | auditoria-mmorch_bucketrank (2026-08-25) [alta/bug] | `mmorch/bucketrank.py`, loop `as_completed` | ABIERTO |
| 6 | **`auto_repair`: estado se persiste ANTES del automerge** — si `try_automerge` falla/mergea, el resultado no queda en `repair_state.json`; próxima corrida decide con estado incompleto. + `keep_branch` puede retener branch con cambios sin commitear si `wt.capture` falla. | auditoria-mmorch_auto_repair (2026-08-21) [media/bug ×2, elevado: corre desatendido en nightly] | `auto_repair.py:99-119` | ABIERTO |
| 7 | **`automerge`: archivos nuevos con fixtures de test que contienen "password"/"secret" se marcan rojos** (baseline='' en `red_content_hits` para status 'A') → falsos rojos bloquean merges verdes legítimos del nightly. | auditoria-mmorch_automerge (2026-08-21) [alta/bug] | `automerge.py`, `classify_branch` | ABIERTO |
| 8 | **ZHIPU_API_KEY muerta (401 incluso en glm-4.6 control)**; glm-5.2 registrado con precios provisionales NUNCA smoke-testeado. Si sigue muerta, un nodo del `DEFAULT_INTUITION_POOL` es humo. | memoria quantized-analysis (2026-07-02, 47 días — **verificar vigencia**) | `orchestration/.env`, `config.py` pool | VERIFICAR |

### SEV-2 · MEDIA — degradación silenciosa / métricas mentirosas

| # | Defecto / deuda | Fuente | Evidencia | Estado |
|---|---|---|---|---|
| 9 | **H-4 · Guard cross-family confía en `gen_model` etiquetado por el caller**: si el caller mislabela la familia del autor, el guard OneFlow se bypassea. Verificado hoy: sigue igual (compara `family_of(gen_model)` declarado). | AUDIT H-4 | `patterns.py:152-158` | ABIERTO (limitación documentada, no validada) |
| 10 | **Bandit sin recency decay**: shift de paradigma (migración de lib) → priors viejos misrutean. Fix barato conocido: discounted Thompson. | Blind-spot #4 | `feedback.ThompsonBandit` | ABIERTO |
| 11 | **Árbitro no auditado**: la tasa de falsos-DISMISS nunca se midió; + techo common-mode (familias frontier comparten distribución → cross-family ≠ independiente en juicios subjetivos; la ejecución es el único piso real). `arbitration.py` existe como ledger pero trunca `reason` a 400 chars y `evidence` a 200 sin flag — la auditoría queda coja justo en su dato central. | Blind-spot #2 + auditoria-mmorch_arbitration [media/bug] | `arbitration.py`, `log()` | PARCIAL (ledger existe; medición y no-truncado pendientes) |
| 12 | **Gates de single-run sobre suites posiblemente flaky** — checker determinista de flakiness existe pero está SIN cablear. | Blind-spot #7 | checkers ↔ gates | ABIERTO |
| 13 | **Notas de memoria verificadas UNA vez, nunca re-verificadas** (staleness semántica). | Blind-spot #8 | `memory.py` semantic notes | ABIERTO |
| 14 | **Sin provenance de decisiones** (versión de prompt/few-shots por unidad) — prerequisito para atribuir ganancias del futuro few-shot bootstrap; sin esto, riesgo de self-training drift cuando llegue (Blind-spot #5). | Blind-spot #9 + #5 | project_build / prompts | ABIERTO |
| 15 | **H-8 · Precios hardcodeados sin staleness check** (`price_asof` no existe en `config.py`). Mitigación parcial: `megasource.py` propone updates a `prices.json` en zona amarilla, pero no hay warning automático por antigüedad. | AUDIT H-8 | `config.py` (sin `asof`), `megasource.py:46-56` | PARCIAL |
| 16 | **`autoresearch.resume_from_journal`: primer loop de `best` es código muerto mal implementado** (nunca actualiza al mayor; el 2do bloque lo pisa) + rounds cuenta rondas incompletas post-crash → discrepancia silenciosa rounds/best. | auditoria-mmorch_autoresearch (2026-08-22) [media/bug ×2] | `autoresearch.py:44-55` | ABIERTO |
| 17 | **`babel._chunks`: pérdida/duplicación del último chunk** si un párrafo completa exactamente el límite. | auditoria-mmorch_babel (2026-08-23) [media/bug] | `babel.py`, `_chunks()` | ABIERTO |
| 18 | **`architecture.co_change_pairs` cuenta commits vacíos** (commits solo-docs agregan `set()` que pasa el filtro) → ratios de co-cambio inflados; + `_mutates` no detecta `setattr`/`globals()[...]`; + `pollution_candidates` solo atrapa auto-mutación (el caso menos común). | auditoria-mmorch_architecture (2026-08-21) [alta+media+media/bug] | `architecture.py:88-96, 130-158` | ABIERTO |
| 19 | **`auto_repair`: path Windows hardcodeado** (`--basetemp=...Users/map12/AppData...`) rompe portabilidad — en Linux el gate falla y bloquea TODA reparación. | auditoria-mmorch_auto_repair [media/bug] | `auto_repair.py:91` | ABIERTO |
| 20 | **`bench.materialize`: reemplazo frágil `"python " → sys.executable`** (solo 1ra ocurrencia exacta; `python3 -m` no matchea) + test de acceptance del dedup no verifica "queda el primero" (un dedup-por-último pasa). | auditoria-mmorch_bench (2026-08-24) [alta/principio + media/bug] | `bench.py`, `materialize` | ABIERTO |
| 21 | **`adjudicate`: hash desactualizado en `state['pairs']` para notas sin strong matches** → re-juzgado innecesario en la próxima corrida; + `skipped_pairs` infla con `loop_paused`. | auditoria-mmorch_adjudicate (2026-08-20) [alta+baja/bug] | `adjudicate.py`, `run_incremental` | ABIERTO |
| 22 | **HITL sin discriminación medida**: revisión masiva del usuario = 35/35 "dale" (todo aprobado) — el gate humano hoy no aporta señal; tratado como safety, jamás reward, pero sigue siendo un control que no controla. | memoria loop-cerrado-spec | flujo /pending → /verdict | CONOCIDO/ACEPTADO |

### SEV-3 · BAJA — deuda estructural / calidad (patrón repetido en las 10 auditorías de módulo)

| # | Tema transversal | Módulos afectados (auditorías 2026-08) | Estado |
|---|---|---|---|
| 23 | **Violación ADR-0001 (releer disco con estado ya en memoria)** — el hallazgo MÁS repetido | adjudicate, arbitration, architecture (`god_module_candidates` relee lo que `import_graph` parseó), auto_repair (fallback muerto a `nightly.jsonl`), automerge, autoresearch (3-4 lecturas del mismo file), bench (`get_task`) | ABIERTO (patrón) |
| 24 | **Acoplamiento oculto a privados de otro módulo** (`_SKEPTIC_SYSTEM`/`_parse_verdict` desde ablation; `_RED_PATHS` desde automerge; firma de `build_project` en auto_repair) — rompen silencioso al refactorizar patterns/evolve | ablation, automerge, auto_repair | ABIERTO (patrón) |
| 25 | **Seams sin inyección** (call/provider no mockeable en ablation; filesystem en adjudicate; `git_runner` en architecture; `run_fn` incompleto en autoresearch; parser de tier en bucketrank) + `auto_repair` sin self-check `__main__` | ablation, adjudicate, architecture, autoresearch, bucketrank, auto_repair | ABIERTO (patrón) |
| 26 | Menores puntuales: `lat_avg=0.0` engañoso con 0 casos (ablation:76) · `fidelity` no distingue JSON inválido de fidelidad baja (babel) · `_GOD_FANIN_FRAC` cuenta `__init__.py` en denominador (architecture:12,53) · orden no determinista dentro de tier (bucketrank) | varios | ABIERTO |

> **Caveat sobre las auditorías de módulo**: son LLM cross-family (refutación 4/4-8/8) pero la
> propia memoria del proyecto documenta que el LLM-review alucina (~68-74% falsos críticos pre-gates;
> caso `_DISTILL_VERIFY_RUBRIC` 100% falso-refute). Los `bugs` de SEV-1/2 arriba conviene
> confirmarlos con repro ejecutable antes de fixear; los "estructurales/principios" son deuda
> direccional, no fuego.

---

## Backlog durable (beads, `bd list` 2026-08-27)

Todos **blocked** (●) — ninguno es defecto de producción; son tracks de investigación/features:

| ID | P | Título | Estado |
|---|---|---|---|
| orchestration-r4a | P3 | hillclimb como job automático del workflow (tool MCP + server job resumable) | in_progress, blocked |
| orchestration-ymr | P3 | Memoria de trayectoria segmentada (Block-AttnRes): drill-back selectivo en runs largos | open, ready |
| orchestration-ws6 | P4 | Bases candidatas para upcycle a looped-LM | open, ready |
| orchestration-aaf | P4 | SPIKE: tiny looped transformer reproduce resultado Physics-of-LLMs | open, ready |
| orchestration-kry | P4 | Plan de experimento: looped + Block AttnRes combinados | open, blocked |
| orchestration-qrf | P4 | Harness autoresearch (Karpathy) para el track de training | open, blocked |

## Pendientes operativos (de HANDOFF + memorias, no en beads)

- **Lotus wiring sprint**: `api.js` → endpoints chat/minds/events existentes; endpoint `/benchmarks` en server; **7 TODOs en `Lotus/src/lib`** (repo aparte `Desktop/Claude/Lotus`); remote `M3EMO/Lotus` sin crear (gh no instalado); logo diferido.
- **Poda automática de worktrees abandonados** (loop cerrado) — pendiente menor declarado.
- **`regserver.ps1` watchdog del server** — pendiente de registrar.
- **Rotación de bughunt/hardening por proyecto** — pendiente hasta semana estable.
- **Bursts de arXiv** (perfil de falla del scraper) — pendiente barato.
- **Live run del engine con recurse depth>1 + paths de escalada** — esperando una tarea real grande; opción 2do-coder cross-family sin construir.
- **Full suite (7:24 min) no wireada como ritual** — pre-push solo collection-check; el gate completo es manual/nightly.
- **PR caveman upstream sin abrir** (repo aparte; branch `fix/temp-file-leak` listo en fork desde 2026-06-15).
- **Libs research (2026-07) — grafts pendientes**: few-shot bootstrap (requiere #14 provenance primero), HNSW cuando escale la memoria, llama.cpp+Outlines cuando haya pesos locales (gateado por hardware: 8GB RAM actual).
- **Workflow-evolution**: COPRO-lite mutator cuando v1 acumule data; más bench tasks (`bench.py` congelado, held-out anti-contaminación).

## Lo que NO es deuda (para no re-abrir)

- Seeds ⏳ de `ALGORITHMS-MAP.md` y §BACKLOG de `SELF-EVOLUTION-PLAN.md` (motor físico, mutation_score checker, CodeBERT prior, GNN-AST, MoCo…): **gateados por problema medido**, entran solo si un problema lo pide.
- `WEIGHTS.md`: MoCo RECHAZADO (medido), positivos funcionales NO promovidos (no significativo), hybrid empeora a escala — decisiones cerradas con datos, no pendientes.
- Los 4 hallazgos descartados del AUDIT 2026-06-07 (refutados cross-family, ver sección "Descartados" de ese archivo).
- Higiene: **0 marcadores TODO/FIXME/XXX/HACK reales** en `mmorch/`; gates ruff+mypy enforced en 0.

## Recomendación de orden para production-readiness

1. **#4 retry/backoff en `providers.py`** + **#5 pool-abort en `bucketrank`** — mismos patrones ya fixeados en otros módulos (H-1/H-3), fixes chicos, cierran el fallo duro en runs masivos.
2. **#6/#7 nightly desatendido** (`auto_repair` orden de persistencia, `automerge` falsos rojos) — el nightly ya corre solo; sus bugs son los que nadie ve.
3. **#2 circuit-breaker de USD por-run** y **#1 sandbox OS** — antes de escalar autonomía, no después.
4. **#11 medir falsos-DISMISS del árbitro** + destruncar `reason` — barato, habilita confiar en el resto.
5. **#3 outcome horizon post-merge** y **#10 discounted Thompson** — cierran el loop de aprendizaje real.
6. Deuda estructural SEV-3 (ADR-0001, seams): via el propio nightly evolve, un módulo por vez.
