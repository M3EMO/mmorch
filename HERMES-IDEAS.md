# Robo de ideas — Hermes Agent (Nous) → mmorch

Hermes = framework de agente COMPLETO (chat gateways, TUI, cron, 40+ tools, 6 backends).
mmorch = lib orquestadora determinista. NO adoptar entero (forma distinta). Cherry-pick
de ideas, cada una gateada por valor/costo y por el GOAL (anti-scope-creep: métrica justifica).

## Inventario completo (toda feature de Hermes) y veredicto

| # | Idea Hermes | mmorch hoy | Veredicto |
|---|---|---|---|
| 1 | Trajectory compression → training data tool-calling | flywheel (oracle/code_loop) genera pero TIRA la trayectoria | **BUILD** (headliner) |
| 2 | Autonomous skill creation after complex tasks | rubric_loop distila regla a memoria | **BUILD** (skill = trayectoria verde destilada a template reusable) |
| 3 | Skills self-improve during use | feedback/bandit ajusta | tenemos (parcial) |
| 4 | FTS5 session search + LLM summarization | recall solo por embedding bge | **BUILD** (FTS keyword complementa semántico) |
| 5 | Periodic memory nudging | consolidate manual | **BUILD** (nudge = policy: tras N loops, auto-consolida+destila) |
| 6 | Honcho dialectic user modeling (USER.md) | — | SKIP (capa chat; Claude Code ya modela user) |
| 7 | MEMORY.md persistence | DuckDB episodic+semantic | tenemos |
| 8 | Spawn subagents paralelo | fan_out / Workflow | tenemos |
| 9 | RPC scripts colapsan pipeline a 1 turno "zero-context" | ES la tesis central de mmorch (Python conduce) | tenemos (core) |
| 10 | Cron natural-language scheduler | — | SKIP (Claude Code scheduled-tasks) |
| 11 | Batch trajectory generation | fan_out + oracle_dataset | tenemos |
| 12 | 6 backends ejecución (Docker/Modal/SSH/Daytona) | sandbox.py = subprocess (UNSAFE-vs-hostil) | **BUILD** (backend container pa zona roja; interfaz pluggable) |
| 13 | Serverless hibernación (Modal/Daytona) | — | SKIP (overkill) |
| 14 | Command approval allowlist patterns | red_content_hits escanea peligro | **BUILD** (allowlist explícita pre-exec) |
| 15 | DM pairing / auth plataforma | — | SKIP (chat) |
| 16 | MCP integration | mcp_server (22 tools) | tenemos |
| 17 | Model switching /model | registry config | tenemos |
| 18 | Personalities / context files | CLAUDE.md | SKIP |
| 19 | Multi-plataforma (Telegram/Discord/…) | — | SKIP (producto, no orquestación) |
| 20 | TUI | — | SKIP |
| 21 | Migración OpenClaw | — | SKIP |
| 22 | Per-tool API key sin lock-in | registry api_key_env | tenemos |

## Lo que se construye (5 ideas, todas barato/alto-valor o safety)
1. **trajectory.py** — captura+comprime trayectorias de rubric_loop/code_loop → dataset
   append-only etiquetado por EJECUCIÓN. Combustible directo del flywheel (code_embedder +
   ShadowPrior). Idea #1.
2. **skill distill** — trayectoria verde tras correcciones → template reusable + regla
   verificada (idea #2). Se monta sobre trajectory.
3. **FTS keyword recall** — búsqueda por término exacto junto al recall semántico bge
   (idea #4). Atrapa lo que el embedding no.
4. **nudge** — policy: cada N loops cerrados, auto-consolida memoria + destila skills
   pendientes (idea #5). No-hook (mmorch es lib): función que el caller llama.
5. **exec backends + allowlist** — sandbox.py con backend pluggable (local subprocess |
   docker) + allowlist de comandos pre-ejecución (ideas #12+#14). Safety zona roja.

## CONSTRUIDO (2026-06-12)
- `mmorch/trajectory.py` — captura+comprime trayectorias de rubric_loop/code_loop →
  `logs/trajectories.jsonl`; `trajectory_dataset()` las aplana a (code, label=ejecución)
  pal flywheel; `distill_skill()` → `logs/skills.jsonl` + nota verificada cuando hay
  corrección. Wireado en `rubric_loop._close_loop` + `code_loop`. (ideas #1+#2)
- `mmorch/memory.py::recall_keyword` (BM25-lite, cero dep, anda sin fastembed) +
  `recall_hybrid` (fusión RRF semántico+keyword). (idea #4)
- `mmorch/sandbox.py` — backend pluggable `local|docker` (docker: --network none
  --read-only --memory --pids-limit --cap-drop ALL) + `enforce_policy` (escaneo estático
  pre-exec: red/proceso/fs-write → bloquea). (ideas #12+#14)
- `mmorch/nudge.py` — cada N loops cerrados, consolida memoria auto. Wireado en ambos
  loops. (idea #5)
- 18 tests nuevos. Todo opt-in/graceful, cero dep nueva (docker opcional).

## Lo que NO (y por qué)
Gateways de chat, TUI, cron, user-modeling, migración: son la CAPA PRODUCTO de un agente
de chat. mmorch es el MOTOR. Meterlos = perder el foco (conductor determinista barato).
El GOAL lo prohíbe (no crecer complejidad sin métrica que la justifique).
