# 11 — Build plan (fase Plan del ciclo production-ready)

Fecha: 2026-08-27 · Insumos: 00-canonical-matrix (gaps 1-10), 00-acceptance-test-plan
(AT-1..34), 07-10 (research externo). Objetivo: cerrar el circuito — instalable,
honesto, seguro, auto-evolución real gateada, Cursor-compatible, independiente de
Claude Code como sistema propio.

## Principios de ejecución

- **Olas con ownership disjunto.** Cada unidad de build tiene autoría de módulos
  explícita; dos unidades nunca editan el mismo archivo en la misma ola. Conflicto
  detectado ⇒ la unidad se mueve de ola.
- **Integración continua.** Cada unidad termina con: suite verde completa + ruff+mypy
  en 0 + commit propio. Nada queda "para integrar después".
- **Sin decorativo nuevo.** Todo lo que se agrega tiene caller vivo + test. Lo
  decorativo existente se cablea o se borra (decisión por unidad, sesgo: borrar).
- **El pass final lo da ejecución** (research 07): veredicto de agente/LLM nunca es
  terminal; ATs y gates deciden.

## Contratos compartidos (todas las unidades los respetan)

- `mmorch/paths.py` (nuevo, W1): `home() -> Path` (env `MMORCH_HOME`, default
  `Path(__file__).parents[1]` para no romper el checkout actual), `data_dir()`,
  `logs_dir()`, `db_path(name)`. Único módulo autorizado a resolver rutas de estado.
  Prohibido `Path(__file__).parents[1]` fuera de paths.py (gate: grep en CI).
- Contrato de error MCP: toda tool retorna `{"error": str, "kind": str}` en fallo,
  nunca excepción cruda ni formatos mixtos (W5).
- Wrappers MCP sin lógica: mcp_server delega a la librería; una sola semántica (W5).
- `beat(component)`: solo componentes con emisor real declarados en EXPECTATIONS (W3).

## Olas

### W1 — Fundaciones (bloquea todo; secuencial)
| Unidad | Scope (archivos) | DoD | ATs |
|---|---|---|---|
| W1.1 MMORCH_HOME | `mmorch/paths.py` nuevo + ~27 módulos que anclan `parents[1]` + tests | suite verde con y sin `MMORCH_HOME` seteado; grep-gate limpio | AT-A |
| W1.2 mypy→0 + gate activo | regresion.py, health.py:162, provenance.py:38 + `.beads/hooks` (agregar mypy) + pins ruff/mypy en pyproject | `ruff check .` y `mypy mmorch` en 0 con versión pinneada; hook activo corre ambos | AT-D |
| W1.3 pin mcp | pyproject.toml: `mcp>=1.28,<2` | server MCP arranca; suite verde | AT-B |

### W2 — Standalone + Cursor
| Unidad | Scope | DoD | ATs |
|---|---|---|---|
| W2.1 Paquete instalable | mover mcp_server/prompts/roles dentro del paquete; `[project.scripts]`: `mmorch-mcp`, `mmorch-nightly`, `mmorch` (CLI status/health/check); `.env` por `__file__` no cwd | `uv pip install .` en venv limpio + `mmorch-mcp` arranca stdio | AT-A, AT-B |
| W2.2 Perfil Cursor | `MMORCH_MCP_PROFILE=core` (≤40 tools curadas); doc `docs/cursor-setup.md` con `~/.cursor/mcp.json` | server en perfil core expone ≤40 tools; smoke stdio pasa | AT-B |
| W2.3 Executor abstraído | seam `Executor` en claude_exec.py (backend claude-CLI hoy; interfaz para cursor-agent/API-only) | project_build corre con el backend actual vía la seam; test con fake Executor | AT-C |

### W3 — Honestidad operacional + robustez
| Unidad | Scope | DoD | ATs |
|---|---|---|---|
| W3.1 Health honesto | health.py EXPECTATIONS vs emisores reales; `beat("server")` en server, digest emite o sale; `GET /health`; smoke FALLA si `healthy=False` | `healthy=True` alcanzable en operación normal; rojo = señal real | AT-E |
| W3.2 Server seguro+durable | token OBLIGATORIO (sin token ⇒ refuse), solo header; jobs persistidos (DuckDB/jsonl) y resumibles post-crash; fix server_forever.ps1 bind-loop | arranque sin token falla con mensaje claro; kill -9 y restart conserva jobs | AT-F |
| W3.3 Retry/backoff + pools | providers.py: retry con backoff+jitter para 429/5xx (clase de error ya existe), half-open breaker por modelo con cooldown (defaults LiteLLM, research 09); fix `as_completed` de bucketrank (patrón H-1) | test simulando 429 transitorio: run masivo sobrevive; bucketrank no pierde pool | AT-C |
| W3.4 Budget defendible | rotación de metrics.jsonl; `price_asof` + warning por precio vencido; breaker USD por-run en builds recursivos; timeouts loggean costo estimado, no 0 | run patológico corta por breaker; summary() no revienta con línea incompleta | AT-C, AT-G |

### W4 — Auto-evolución real (el claim central)
| Unidad | Scope | DoD | ATs |
|---|---|---|---|
| W4.1 Gate unificado en el camino VIVO | goal_guard al arranque de nightly; `goal_aligned` sobre el diff pre-PR; tamper-halt sin re-autorización por borrado de GOAL.hash (hash faltante = HALT, no re-auth); never-edit-guard extendido al harness de eval (scorers/tests de fitness/configs de gate — research 07 #1) | nightly con GOAL adulterado NO produce PR; test de tamper | AT-F, AT-G |
| W4.2 Automerge con ledger | crear el ledger obligatorio; fix falsos rojos ("password" en archivo nuevo); auto_repair persiste DESPUÉS del automerge; primer merge verde real gateado y auditado | 1 automerge verde ejecutado end-to-end con ledger + rollback ensayado | AT-G, AT-H |
| W4.3 Museo: cablear o borrar | fitness()/evaluate(): cablear como check pre-PR del nightly; self_evolve(do_apply), promote_branch, pursue_goal, archive_variant, bandit plano zombie: BORRAR (con sus tests); feedback_stats reporta el sig-bandit real | cero funciones sin caller vivo en mmorch/ (gate: script de detección) | AT-G |
| W4.4 Watchdog | dead-man real: nightly muerto >24h ⇒ señal visible (health + digest); fix path Windows hardcodeado auto_repair.py:91 | matar el scheduled task ⇒ rojo en <24h | AT-E |

### W5 — Contratos + cobertura
| Unidad | Scope | DoD | ATs |
|---|---|---|---|
| W5.1 Una sola semántica | wrappers MCP sin lógica; doble-bandit unificado (MCP y librería entrenan lo mismo); contrato de error uniforme; docstrings sincronizados (21 checkers, no 2) | diff de comportamiento librería-vs-MCP = 0 en tools core; test de contrato | AT-B, AT-C |
| W5.2 Cobertura real | runner de self-checks (~15 líneas) en suite; tests para los 7 módulos con cero cobertura (server_*, pty_session, transcript_store); test de contrato de mcp_server (46 tools: schema in/out) | módulos con cero cobertura = 0; suite verde | AT-D |
| W5.3 Robos externos S | canary set (~20 tareas) + `model_version` en record_outcome (research 08); matriz de riesgo por tool `read\|mutate\|outward` con gate HITL (08); labels categóricos anclados en verificadores (ataca ECE 0.456) | canary corre vía comando único; tools mutantes gateadas | AT-C, AT-F |

### W6 — Verificación (fase 4 del goal; loop hasta verde)
Correr AT-1..AT-34 completos. Agentes dedicados: adversarial (inputs hostiles a tools
MCP + prompt injection en pipelines), security (token, zona roja, tamper, secretos),
data isolation (dos MMORCH_HOME simultáneos), edge cases (archivos corruptos en logs/,
DuckDB lockeada, API caída), performance (run masivo con breaker). Todo defecto →
log → fix → retest → repetir hasta: 34/34 ATs verdes, gates en 0, suite verde,
cero defectos materiales abiertos.

## Backlog explícito (NO en este ciclo; a bd)
Checkpoint-por-step estilo DBOS (robo M, research 09) · interrupt/resume HITL ·
validez temporal de hechos (Zep) · guardrails paralelos con tripwire · streamable-http
· migración python-sdk v2 de MCP · sandbox de OS para test_cmd (hoy mitigado por
allowlist anti-RCE) · outcome horizon post-merge · OTel gen_ai.* en logging.

## Secuencia

W1 secuencial (1.1 → 1.2 → 1.3, conflictos de archivo). W2 y W3 en paralelo
(ownership disjunto), W4 después de W1 (toca nightly/evolve que W3.1/W3.4 rozan —
verificar ownership al lanzar). W5 tras W2 (mcp_server ya movido). W6 al final,
en loop. Cada ola: commit por unidad + suite + gates antes de la siguiente.
