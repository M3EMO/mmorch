# 09 — Landscape competitivo: frameworks de orquestación multi-agente (2026)

> Investigación externa para mmorch. Fecha: 2026-08-27.
> Premisa fija: **mmorch NO adopta ningún framework** — solo roba patrones.
> Contexto mmorch: orquestador Python multi-modelo (DeepSeek/Gemini baratos + Claude juez),
> auto-evolución gateada, memoria episódica+semántica en DuckDB, MCP server,
> bandit Thompson + calibración.

---

## 0. Mapa del terreno 2026 (resumen)

Los frameworks se agrupan en tres familias ([LangChain "best AI agent frameworks 2026"](https://www.langchain.com/resources/ai-agent-frameworks), [Arize agent handbook](https://arize.com/guides/ai-agent-handbook/agent-frameworks/)):

1. **Graph-based / workflow-first**: LangGraph, Mastra, Microsoft Agent Framework (workflows), Google ADK.
2. **Role-based / conversacional**: CrewAI, AutoGen (legado).
3. **SDK-nativos de proveedor**: OpenAI Agents SDK, Claude Agent SDK, Pydantic AI (agnóstico pero SDK-style), smolagents.

Cambios estructurales 2025→2026:
- **AutoGen entró en maintenance mode**; su sucesor directo es **Microsoft Agent Framework** (equipos AutoGen + Semantic Kernel), GA 1.0 en abril 2026 ([devblogs.microsoft.com](https://devblogs.microsoft.com/agent-framework/microsoft-agent-framework-version-1-0/), [migration guide](https://learn.microsoft.com/en-us/agent-framework/migration-guide/from-autogen/)). AG2 (fork comunitario de AutoGen) sigue vivo pero con tracción decreciente frente a MAF.
- **La durabilidad se volvió el eje de diferenciación**: la crítica de Diagrid ("checkpoints ≠ durable execution") aplica a LangGraph, CrewAI, Google ADK ([diagrid.io](https://www.diagrid.io/blog/checkpoints-are-not-durable-execution-why-langgraph-crewai-google-adk-and-others-fall-short-for-production-agent-workflows)) y también a MS Agent Framework y Strands ([diagrid.io #2](https://www.diagrid.io/blog/still-not-durable-how-microsoft-agent-framework-and-strands-agents-repeat-the-same-mistake)). La respuesta de la industria fue delegar durabilidad a motores externos: **Temporal, DBOS, Restate, Dapr Workflows, Inngest** ([Pydantic AI durable execution](https://pydantic.dev/docs/ai/capabilities/durable_execution/overview/), [Temporal + OpenAI SDK](https://temporal.io/blog/announcing-openai-agents-sdk-integration)).
- **La memoria de agentes se profesionalizó como capa separada**: Mem0, Zep/Graphiti, Letta, LangMem, con benchmarks head-to-head (LoCoMo) ([Mem0 state of agent memory 2026](https://mem0.ai/blog/state-of-ai-agent-memory-2026), [comparativa 5 sistemas](https://medium.com/@wasowski.jarek/i-compared-5-ai-agent-memory-systems-across-6-dimensions-none-wins-6a658335ed0a)).
- **Routing por costo se volvió commodity de gateway** (LiteLLM, OpenRouter, Portkey) + línea de investigación activa (RouteLLM, surveys de cascading) ([Braintrust best LLM routers 2026](https://www.braintrust.dev/articles/best-llm-routers-2026), [survey arXiv 2603.04445](https://arxiv.org/pdf/2603.04445)).

---

## 1. LangGraph (LangChain)

**Arquitectura central.** Grafo de estado explícito (StateGraph): nodos = funciones, aristas = control flow, estado tipado compartido. Persistencia vía **checkpointers** (`InMemorySaver`, `SqliteSaver`, `PostgresSaver`) que guardan un snapshot del estado del thread en cada super-step; sobre eso montan durable execution, time-travel y HITL ([docs durable execution](https://docs.langchain.com/oss/python/langgraph/durable-execution), [repo](https://github.com/langchain-ai/langgraph)). Dos capas: checkpointers (estado por thread) + **Stores** (datos cross-thread: preferencias, hechos).

**(a) Durabilidad/resume.** Con checkpointer activo, cualquier run se puede reanudar pasando `thread_id`; el grafo re-ejecuta desde el último checkpoint. Modos de durabilidad configurables (sync/async/exit). **Límite real** (Diagrid): la detección del fallo y el re-invoke son responsabilidad del operador — no hay recovery automático ni coordinación distribuida; el nodo entero se re-ejecuta, así que los side effects pre-checkpoint necesitan idempotencia manual ([diagrid.io](https://www.diagrid.io/blog/checkpoints-are-not-durable-execution-why-langgraph-crewai-google-adk-and-others-fall-short-for-production-agent-workflows), [ZenML sobre runtime durable](https://www.zenml.io/blog/langgraph-durable-runtime)).

**(b) HITL.** El patrón más limpio del mercado: `interrupt(payload)` dentro de un nodo pausa el grafo, persiste estado, y espera indefinidamente; se reanuda con `Command(resume=value)`, donde `value` se convierte en el return de `interrupt()`. Patrones documentados: approve/reject, review-and-edit, interrupt dentro de la tool misma. Caveat documentado: el código ANTES del `interrupt()` se re-ejecuta al reanudar (side effects duplicados) ([docs interrupts](https://docs.langchain.com/oss/python/langgraph/interrupts)).

**(c) Routing multi-provider por costo.** No nativo — delegado a `init_chat_model` + gateways externos. Nada que robar acá.

**(d) Memoria.** Checkpointer = memoria corta por thread; Store = memoria larga cross-thread; LangMem como paquete aparte. Menos sofisticado que la memoria de mmorch (sin decay, sin episódica/semántica diferenciada nativa).

**Mejor que mmorch:** HITL resumible con contrato limpio (interrupt/resume como valor de retorno); time-travel/fork de estado desde cualquier checkpoint; ecosistema de observabilidad (LangSmith).
**Peor que mmorch:** cero routing por costo; sin calibración ni bandit; memoria larga primitiva; grafo estático vs. la descomposición recursiva de mmorch.

**Robable:**
- El **contrato interrupt-as-return-value** (`interrupt()` → `Command(resume=v)` → `v` es el return): mapea directo a los tickets HITL de wayfinder/mmorch. URL: <https://docs.langchain.com/oss/python/langgraph/interrupts>
- **Checkpoint por super-step con thread_id + time-travel** (fork desde checkpoint N para explorar contrafactuales — útil para el tournament/evolution de mmorch). URL: <https://docs.langchain.com/oss/python/langgraph/durable-execution>

---

## 2. AutoGen / AG2 → Microsoft Agent Framework (MAF)

**Estado.** AutoGen (Microsoft) está en **maintenance mode**; MAF 1.0 (GA abril 2026) es el sucesor oficial, fusionando AutoGen (patrones multi-agente) + Semantic Kernel (enterprise: estado por sesión, type safety, filters, telemetría) ([overview](https://learn.microsoft.com/en-us/agent-framework/overview/), [1.0 announcement](https://devblogs.microsoft.com/agent-framework/microsoft-agent-framework-version-1-0/)). AG2 (fork) mantiene el API conversacional clásico de AutoGen; para un análisis 2026, MAF es el player relevante.

**Arquitectura central.** Dos planos: **Agents** (loop LLM+tools) y **Workflows** (motor de grafo determinista que compone agentes y funciones). Patrones de orquestación estables heredados de MSR/AutoGen: sequential, concurrent, handoff, group chat, **Magentic-One** (orquestador con task-ledger + progress-ledger que re-planifica cuando detecta estancamiento).

**(a) Durabilidad.** Checkpointing + "hydration" de workflows largos; pero Diagrid señala que repite el mismo patrón: snapshot sí, recovery automático no ([diagrid.io #2](https://www.diagrid.io/blog/still-not-durable-how-microsoft-agent-framework-and-strands-agents-repeat-the-same-mistake)).

**(b) HITL.** Novedad vs AutoGen: el patrón **request/response** — un workflow puede pausar la ejecución y esperar input externo antes de continuar, cosa que la abstracción Team de AutoGen no tenía; todos los patrones de orquestación soportan streaming, checkpointing, aprobaciones HITL y pause/resume ([migration guide](https://learn.microsoft.com/en-us/agent-framework/migration-guide/from-autogen/)).

**(c) Routing por costo.** No nativo (Azure-céntrico; hay model router en Azure AI Foundry como servicio aparte).

**(d) Memoria.** Estado por sesión heredado de Semantic Kernel; sin capa de memoria semántica comparable a DuckDB de mmorch.

**Mejor que mmorch:** catálogo formalizado de patrones de orquestación (especialmente **Magentic-One**: doble ledger de tareas/progreso con re-planning ante stall); type safety y telemetría integradas.
**Peor que mmorch:** sin routing cross-family, sin auto-evolución, memoria más pobre, lock-in cultural Azure.

**Robable:**
- **Magentic-One progress ledger**: el orquestador mantiene un ledger de progreso y detecta loops/estancamiento → re-planifica. mmorch ya tiene close_loop/open_loops; el ledger de progreso explícito con stall-detection es el delta. URL: <https://learn.microsoft.com/en-us/agent-framework/overview/>
- **Request/response como primitiva de pausa** en workflows (vs. bloquear un thread). URL: <https://learn.microsoft.com/en-us/agent-framework/migration-guide/from-autogen/>

---

## 3. CrewAI

**Arquitectura central.** Dos modos: **Crews** (agentes con rol/goal/backstory + tasks, proceso sequential o hierarchical) y **Flows** (orquestación event-driven con decoradores `@start`, `@listen`, `@router`, `@persist`, `@human_feedback`). Standalone (ya no depende de LangChain). Fortaleza reconocida: velocidad de prototipado role-based ([Arize](https://arize.com/guides/ai-agent-handbook/agent-frameworks/), [gurusup comparison](https://gurusup.com/blog/best-multi-agent-frameworks-2026)).

**(a) Durabilidad.** `@persist` en Flows guarda estado por método, pero la lógica de recovery es manual en cada método y el replay solo captura la última ejecución ([diagrid.io](https://www.diagrid.io/blog/checkpoints-are-not-durable-execution-why-langgraph-crewai-google-adk-and-others-fall-short-for-production-agent-workflows)).

**(b) HITL.** Doble vía: `@human_feedback` decorator para flows locales síncronos, y **HITL por webhook** para producción — el flow pausa, notifica (Slack/Teams), y un endpoint de resume acepta `execution_id`, `task_id`, `human_feedback`, `is_approve` ([docs HITL](https://docs.crewai.com/en/learn/human-in-the-loop)). Enterprise agrega asignación de responsables, escalation, SLA.

**(c) Routing por costo.** No nativo (usa LiteLLM por debajo para multi-provider, sin política de costo propia).

**(d) Memoria.** Rediseñada 2026 como **memoria unificada**: una clase `Memory` donde un LLM analiza el contenido al guardar (infiere scope, categorías, importancia), organiza en un **árbol de scopes tipo filesystem** (`/project/alpha`, `/agent/researcher`), y el recall rankea por fórmula compuesta: similitud semántica + decay exponencial de recencia + importancia. Backend default LanceDB; `remember_many()` no-bloqueante con read-barrier en `recall()` ([docs memory](https://docs.crewai.com/en/concepts/memory)).

**Mejor que mmorch:** el scoring compuesto de recall (semántica+recencia+importancia) está más integrado que el ranking+MMR de mmorch (que ya tiene decay pero no importancia inferida por LLM al encode); HITL por webhook production-grade.
**Peor que mmorch:** durabilidad débil, sin verificación adversarial, sin calibración, proceso hierarchical famoso por ser poco controlable.

**Robable:**
- **Importance-at-encode**: que el modelo barato infiera importancia/scope al momento de `remember`, y que ese score entre en el ranking de recall junto al decay que mmorch ya tiene. URL: <https://docs.crewai.com/en/concepts/memory>
- **Resume-endpoint por webhook** con `execution_id` + `is_approve` para los tickets HITL. URL: <https://docs.crewai.com/en/learn/human-in-the-loop>

---

## 4. OpenAI Agents SDK

**Arquitectura central.** Minimalista deliberado: `Agent` (instructions+tools+guardrails+handoffs) + `Runner` con el loop (LLM → final output | handoff → switch de agente | tool calls → ejecutar y reinyectar). **Handoffs** = transferencia de control entre agentes especializados. **Guardrails** = validaciones de input corriendo EN PARALELO al agente (tripwire aborta temprano) y de output al final ([docs guardrails](https://openai.github.io/openai-agents-python/guardrails/), [docs running agents](https://openai.github.io/openai-agents-python/running_agents/)).

**(a) Durabilidad.** Nativo: **`RunState` serializable** — al pausar (tool approval o `cancel(mode="after_turn")`), el runner captura todo el contexto de ejecución en un objeto serializable; se reanuda pasándolo a `Runner.run()` sin re-ejecutar pasos previos. Para durabilidad real: partnership oficial con **Temporal** ([temporal.io](https://temporal.io/blog/announcing-openai-agents-sdk-integration)).

**(b) HITL.** Tool-approval nativo: tools marcadas `needs_approval` interrumpen el run; el `RunState` se serializa, el humano aprueba/rechaza, y se reanuda. Mismo contrato que interrupt de LangGraph pero a nivel de tool y sin replay del nodo ([docs](https://openai.github.io/openai-agents-python/running_agents/)).

**(c) Routing por costo.** Ninguno (mono-provider por diseño; soporta otros modelos vía LiteLLM adapter pero sin política).

**(d) Memoria.** **Sessions** con backends intercambiables (SQLite, Redis, SQLAlchemy, Dapr, OpenAI Conversations, encrypted): recuperan historial antes de cada run y persisten después. Es historial conversacional, no memoria semántica.

**Mejor que mmorch:** guardrails-en-paralelo (validar input mientras el modelo caro ya corre; el tripwire cancela y ahorra tokens); RunState serializable como valor único que encapsula un run pausado.
**Peor que mmorch:** mono-provider, sin routing/calibración/bandit, sin memoria semántica, sin verificación cross-family.

**Robable:**
- **Guardrails paralelos con tripwire**: correr el clasificador barato de mmorch (cynefin/route) EN PARALELO a la generación cara y abortar temprano si dispara. URL: <https://openai.github.io/openai-agents-python/guardrails/>
- **RunState serializable único** para pausar/reanudar un run sin re-ejecutar. URL: <https://openai.github.io/openai-agents-python/running_agents/>

---

## 5. smolagents (Hugging Face)

**Arquitectura central.** Minimalismo radical (~1000 líneas de lógica core): `MultiStepAgent` base + **`CodeAgent`** — el agente escribe sus ACCIONES como código Python (no JSON tool calls); cada paso ReAct (Thought→Action→Observation) emite un bloque Python, el intérprete lo ejecuta, el output es la observación siguiente. Composabilidad natural (loops, condicionales, anidamiento de funciones). Sandboxing vía E2B/Docker/Modal/Blaxel ([docs](https://huggingface.co/docs/smolagents/index), [agents course](https://huggingface.co/learn/agents-course/en/unit2/smolagents/code_agents)). ~28k stars a mediados de 2026 ([news.lesbass.com](https://news.lesbass.com/articles/smolagents-hugging-face-agenti-codice-python/)).

**(a) Durabilidad.** Prácticamente nula (memoria de pasos en RAM, replay básico). **(b) HITL.** Nada serio. **(c) Routing.** Multi-provider vía LiteLLM/InferenceClient, sin política de costo. **(d) Memoria.** Solo memoria de pasos del run.

**Mejor que mmorch:** el paradigma code-as-action — HF reporta ~30% menos pasos que JSON tool-calling para tareas componibles (claim del paper Executable Code Actions / docs smolagents); alineado con la filosofía anti-framework de mmorch.
**Peor que mmorch:** en todo lo demás (sin durabilidad, memoria, routing, verificación).

**Robable:**
- **Code-as-action para los workers de mmorch**: cuando un subtask requiere componer 3+ tools, pedir un bloque Python sandboxeado en vez de N tool-calls JSON — menos round-trips = menos costo API. mmorch ya aísla código LLM en subprocess (principio existente), así que el sandbox ya está. URL: <https://huggingface.co/docs/smolagents/index>

---

## 6. Mastra (TypeScript)

**Arquitectura central.** Stack TS integrado: agents + workflows (step graph con `.then()/.branch()/.parallel()`) + memoria + evals + observabilidad. Workflow engine con **snapshots persistidos** ([mastra.ai](https://mastra.ai/ai-agent-framework)).

**(a) Durabilidad.** `suspend()`/`resume()` con async/await: al suspender, el snapshot completo del workflow se persiste (LibSQL default) y **sobrevive deploys y restarts**; se reanuda desde un step ID específico con `resumeData` tipado ([docs suspend-resume](https://mastra.ai/docs/workflows/suspend-and-resume)). Hay gente reemplazando Temporal con esto para casos simples ([substack](https://sumantthakur.substack.com/p/part-3-orchestrating-agents-with)).

**(b) HITL.** El mismo suspend/resume ES el mecanismo HITL: el step declara qué necesita (schema de `resumeData`), suspende, y el humano reanuda con datos tipados/validados.

**(c) Routing.** Model routing multi-provider (formato `"provider/model"`) sin política de costo automática.

**(d) Memoria.** La capa más completa de los frameworks generalistas: message history + **working memory** (datos estructurados persistentes del usuario) + **semantic recall** + **observational memory** (agentes background comprimen mensajes viejos en observaciones densas — análogo directo al consolidate/distill de mmorch), con scoping por `resource` (usuario) y `thread` (conversación) ([docs memory](https://mastra.ai/docs/memory/overview)).

**Mejor que mmorch:** suspend/resume tipado con schema de resumeData (contrato de datos del gate HITL); observational memory como proceso background formalizado.
**Peor que mmorch:** TS (no aplica a mmorch), sin routing por costo, sin verificación cross-family, sin evolución.

**Robable:**
- **`resumeData` con schema**: cada gate HITL de mmorch declara el schema JSON de lo que espera del humano; el resume valida contra el schema antes de reanudar. URL: <https://mastra.ai/docs/workflows/suspend-and-resume>
- **Scoping resource/thread** para la memoria: separar "memoria del proyecto" vs "memoria de la sesión" con dos claves explícitas. URL: <https://mastra.ai/docs/memory/overview>

---

## 7. Players 2026 adicionales relevantes

### 7.1 Google ADK
Runtime de **event loop**: el Runner recibe eventos del agente, los persiste vía `SessionService` (historial como Events + State scratchpad), y el agente reanuda donde pausó. Patrón production: agente duerme DÍAS (dormancy event-driven, sin polling), un webhook hidrata la sesión desde SQLite y reanuda el razonamiento exacto ([Google Developers Blog](https://developers.googleblog.com/build-long-running-ai-agents-that-pause-resume-and-never-lose-context-with-adk/), [sessions/state](https://ravichaganti.com/blog/google-adk-sessions-state-and-memory/)). Separación limpia SessionService (conversación) / MemoryService (archivo de largo plazo). Diagrid le critica lo mismo: event sourcing sí, auto-recovery no.
**Robable:** **dormancy gates event-driven** — el workflow largo no es un proceso vivo esperando, es estado en disco + un trigger que lo despierta. mmorch como MCP server ya es request-driven; formalizar "workflow dormido = fila en DuckDB + condición de despertar".

### 7.2 Pydantic AI
El movimiento arquitectónico más citado de 2026: **no construir durabilidad propia** sino integraciones first-party co-mantenidas con **Temporal, DBOS, Prefect** (y Restate en camino) — agentes que sobreviven restarts y corren días sobre el motor que ya operás, con HITL incluido ([docs durable execution](https://pydantic.dev/docs/ai/capabilities/durable_execution/overview/), [DBOS integration](https://docs.dbos.dev/integrations/pydantic-ai)). **DBOS** es el dato clave para mmorch: durabilidad como LIBRERÍA que checkpointea en una DB existente (Postgres… o SQLite/DuckDB conceptualmente) — cero infra nueva, los workflows reanudan automáticamente desde el último step completado ([temporal.io blog](https://temporal.io/blog/build-durable-ai-agents-pydantic-ai-and-temporal), [Reactify 2026 overview](https://www.reactify-solutions.com/articles/durable-ai-agents-2026)).
**Robable:** el patrón DBOS — decorar cada step del project-build engine con un checkpoint-en-DuckDB (step_id, inputs hash, output) y al reanudar, saltar steps ya completados. Es durable execution "de biblioteca", no de plataforma.

### 7.3 Dapr Agents / Dapr Workflows
La alternativa "durabilidad real" que Diagrid empuja (co-mantenida con NVIDIA): workflow-as-code sobre un runtime que garantiza completion con replay + idempotencia ([diagrid.io](https://www.diagrid.io/blog/checkpoints-are-not-durable-execution-why-langgraph-crewai-google-adk-and-others-fall-short-for-production-agent-workflows)). Para mmorch es overkill de infra (sidecar model), pero su checklist define el estándar: persistencia automática por step, recovery sin intervención, replay que restaura variables locales, no-duplicación de trabajo.

### 7.4 Capa de memoria especializada (Mem0 / Zep / Letta)
No son orquestadores pero compiten con la memoria de mmorch:
- **Mem0**: extracción jerárquica single-pass + retrieval multi-señal; en 2026 reportó +29.6 pts en queries temporales y +23.1 en multi-hop vs su algoritmo anterior (LoCoMo) ([mem0.ai](https://mem0.ai/blog/state-of-ai-agent-memory-2026)).
- **Zep/Graphiti**: **knowledge graph temporal** — cada hecho tiene validez temporal, resolviendo contradicciones ("vivía en NY, ahora en Londres") por invalidación, no por sobrescritura ([comparativa Medium](https://medium.com/@wasowski.jarek/i-compared-5-ai-agent-memory-systems-across-6-dimensions-none-wins-6a658335ed0a)).
- **Letta (ex MemGPT)**: memoria auto-editada por el agente, con tiering core/archival explícito.
**Robable:** validez temporal de hechos en la memoria semántica de mmorch (columnas `valid_from`/`invalidated_by` en DuckDB) — resuelve contradicciones sin borrar historia, y alimenta el `flag_contradiction` existente.

### 7.5 Routing por costo (capa gateway + research)
- **LiteLLM Router**: 5 estrategias incl. `cost-based-routing` (menor precio del cost map respetando límites RPM/TPM), latency-based con TTL, cooldowns automáticos (`allowed_fails`=3/min → cooldown 5s), retry policies por tipo de excepción, tiers de prioridad (`order`) y weighted failover que agota réplicas del mismo grupo antes de cruzar de modelo ([docs routing](https://docs.litellm.ai/docs/routing)).
- **RouteLLM** (LMSYS): router aprendido de preference data — 85% ahorro en MT-Bench manteniendo 95% de calidad GPT-4, usando el modelo fuerte solo en 14% de queries (benchmark-specific) ([Braintrust](https://www.braintrust.dev/articles/best-llm-routers-2026)).
- **OpenRouter Auto Router**: dial `cost_quality_tradeoff` 0-10 ([wavect comparison](https://wavect.io/blog/llm-gateway-router-comparison-2026/)).
- Survey académico de routing/cascading dinámico: [arXiv 2603.04445](https://arxiv.org/pdf/2603.04445); benchmark unificado [LLMRouterBench, arXiv 2601.07206](https://arxiv.org/pdf/2601.07206).
**Posición de mmorch:** el bandit Thompson + calibración de mmorch es MÁS sofisticado que cualquier framework de orquestación (ninguno tiene routing aprendido nativo) pero menos robusto operacionalmente que LiteLLM. **Robable:** cooldown/half-open por deployment + retry policy por clase de excepción + weighted failover (mmorch ya tiene health floor y half-open breaker en backlog — LiteLLM da los defaults numéricos probados en producción).

---

## 8. Tabla: feature × framework × mmorch

| Feature | LangGraph | MAF (ex-AutoGen) | CrewAI | OpenAI SDK | smolagents | Mastra | Google ADK | Pydantic AI | **mmorch hoy** |
|---|---|---|---|---|---|---|---|---|---|
| Durabilidad/resume | Checkpointer por super-step, resume manual por thread_id | Checkpoint+hydration, recovery manual | `@persist` débil, recovery manual | `RunState` serializable; Temporal para durabilidad real | No | `suspend()` snapshot sobrevive deploys | Event sourcing + hidratación por webhook | Delegado: Temporal/DBOS/Prefect first-party | Parcial (worktrees + branches; sin checkpoint por step) |
| HITL gates | `interrupt()`/`Command(resume=)` — el mejor contrato | request/response primitive | `@human_feedback` + webhook resume enterprise | Tool `needs_approval` + RunState | No | suspend con `resumeData` tipado | Pausa event-driven días | Vía motor durable | Tickets HITL wayfinder (manual, sin resume automático) |
| Routing por costo | No | No (Azure aparte) | No (LiteLLM passthrough) | No | No (passthrough) | No | No | No | **Sí — bandit Thompson + calibración (único)** |
| Memoria persistente | Store cross-thread + LangMem | Sesión (SK) | Unificada: LLM-scored, árbol de scopes, LanceDB | Sessions (historial) | No | working+semantic+observational, resource/thread | SessionService + MemoryService | Básica | **Sí — episódica+semántica DuckDB, decay, ranking+MMR** |
| Verificación adversarial | No | No | No | Guardrails (validación, no refutación) | No | Evals | No | Output validators | **Sí — cross-family refute (único)** |
| Auto-evolución | No | No | No | No | No | No | No | No | **Sí — gateada (único)** |
| Multi-modelo cross-family | Agnóstico sin política | Agnóstico | Agnóstico | Mono-OpenAI | Agnóstico | Agnóstico | Gemini-first | Agnóstico | **Sí, con política** |
| Time-travel/fork de estado | Sí | No | No | No | No | No | Parcial (events) | Vía Temporal replay | No |
| Sandboxing de código | No nativo | No | No | Sí (hosted) | Sí (E2B/Docker/Modal) | No | Sí (Cloud) | No | Subprocess propio |

Lectura honesta: **mmorch gana en las 3 features que ningún framework tiene** (routing aprendido, verificación cross-family, evolución gateada) y **pierde en las 2 que todos convergieron a resolver** (durabilidad por step y HITL con resume automático).

---

## 9. Los 5 robos de mayor valor (ranked, con esfuerzo)

1. **Checkpoint-por-step estilo DBOS en DuckDB** — decorar cada unidad del project-build engine con `(workflow_id, step_id, inputs_hash) → output` persistido; al reanudar, los steps completados se saltan (memoización durable). Cubre el gap #1 de la tabla sin adoptar Temporal ni infra nueva: DuckDB ya está ahí. Regla de idempotencia de LangGraph aplica: side effects después del checkpoint. **Esfuerzo: M (2-3 sesiones: decorator + tabla + replay-skip + tests de kill-resume).** Refs: <https://docs.dbos.dev/integrations/pydantic-ai>, <https://docs.langchain.com/oss/python/langgraph/durable-execution>
2. **Contrato interrupt/resume para los tickets HITL** — `interrupt(payload)` persiste el workflow dormido (fila DuckDB + condición de despertar, estilo dormancy de ADK); el humano responde y `resume(ticket_id, data)` valida `data` contra un schema declarado (estilo `resumeData` de Mastra) y reanuda desde el checkpoint del robo #1. Convierte los tickets wayfinder de "bloqueo de sesión" a "workflow dormido". **Esfuerzo: M (depende del robo #1; 1-2 sesiones encima).** Refs: <https://docs.langchain.com/oss/python/langgraph/interrupts>, <https://mastra.ai/docs/workflows/suspend-and-resume>, <https://developers.googleblog.com/build-long-running-ai-agents-that-pause-resume-and-never-lose-context-with-adk/>
3. **Cooldown + retry policy + weighted failover de LiteLLM en el router** — defaults probados: `allowed_fails` por minuto → cooldown por deployment, retry inmediato vs backoff según clase de excepción, tiers `order` que agotan réplicas antes de cruzar de familia. Complementa (no reemplaza) el bandit: el bandit elige por calidad esperada, esta capa lo protege operacionalmente (ataca directo el err 34% de glm-4.6). **Esfuerzo: S (1 sesión; el half-open breaker ya está en backlog — esto lo especifica).** Ref: <https://docs.litellm.ai/docs/routing>
4. **Validez temporal de hechos en memoria semántica (patrón Zep/Graphiti)** — `valid_from`/`invalidated_by` en las notas DuckDB: una contradicción nueva invalida (no borra) el hecho anterior; `recall` filtra por vigencia; `flag_contradiction` pasa de detectar a RESOLVER. Mem0 muestra que lo temporal es donde más se gana (+29.6 pts). **Esfuerzo: S-M (1-2 sesiones: migración de schema + lógica en remember/recall).** Refs: <https://mem0.ai/blog/state-of-ai-agent-memory-2026>, <https://medium.com/@wasowski.jarek/i-compared-5-ai-agent-memory-systems-across-6-dimensions-none-wins-6a658335ed0a>
5. **Guardrails paralelos con tripwire (OpenAI SDK)** — correr el check barato (cynefin/classify, presupuesto, injection-scan con DeepSeek) EN PARALELO a la generación cara y cancelar si dispara: hoy mmorch clasifica ANTES (latencia serial) o verifica DESPUÉS (tokens ya gastados). **Esfuerzo: S (1 sesión: asyncio.gather + cancel en el camino caliente de orchestra/cascade).** Ref: <https://openai.github.io/openai-agents-python/guardrails/>

**Menciones que quedaron fuera del top-5** (valor real, menor ROI inmediato): code-as-action de smolagents para workers multi-tool (~menos round-trips; el subprocess sandbox ya existe) — <https://huggingface.co/docs/smolagents/index>; progress-ledger de Magentic-One con stall-detection para el project-build engine — <https://learn.microsoft.com/en-us/agent-framework/overview/>; importance-at-encode de CrewAI para el ranking de recall — <https://docs.crewai.com/en/concepts/memory>; scoping resource/thread de Mastra — <https://mastra.ai/docs/memory/overview>.

---

## 10. Fuentes (leídas)

1. https://docs.langchain.com/oss/python/langgraph/durable-execution — docs oficiales, fetched
2. https://docs.langchain.com/oss/python/langgraph/interrupts — docs oficiales, fetched
3. https://www.diagrid.io/blog/checkpoints-are-not-durable-execution-why-langgraph-crewai-google-adk-and-others-fall-short-for-production-agent-workflows — engineering blog, fetched
4. https://www.diagrid.io/blog/still-not-durable-how-microsoft-agent-framework-and-strands-agents-repeat-the-same-mistake — engineering blog
5. https://devblogs.microsoft.com/agent-framework/microsoft-agent-framework-version-1-0/ — anuncio oficial MAF 1.0
6. https://learn.microsoft.com/en-us/agent-framework/migration-guide/from-autogen/ — docs oficiales
7. https://docs.crewai.com/en/concepts/memory — docs oficiales, fetched
8. https://docs.crewai.com/en/learn/human-in-the-loop — docs oficiales
9. https://openai.github.io/openai-agents-python/running_agents/ — docs oficiales, fetched
10. https://openai.github.io/openai-agents-python/guardrails/ — docs oficiales
11. https://temporal.io/blog/announcing-openai-agents-sdk-integration — anuncio oficial Temporal
12. https://huggingface.co/docs/smolagents/index — docs oficiales
13. https://huggingface.co/learn/agents-course/en/unit2/smolagents/code_agents — curso oficial HF
14. https://mastra.ai/docs/workflows/suspend-and-resume — docs oficiales
15. https://mastra.ai/docs/memory/overview — docs oficiales, fetched
16. https://developers.googleblog.com/build-long-running-ai-agents-that-pause-resume-and-never-lose-context-with-adk/ — blog oficial Google
17. https://pydantic.dev/docs/ai/capabilities/durable_execution/overview/ — docs oficiales
18. https://docs.dbos.dev/integrations/pydantic-ai — docs oficiales DBOS
19. https://docs.litellm.ai/docs/routing — docs oficiales, fetched
20. https://www.braintrust.dev/articles/best-llm-routers-2026 — comparativa (RouteLLM numbers)
21. https://mem0.ai/blog/state-of-ai-agent-memory-2026 — benchmark report primera mano
22. https://medium.com/@wasowski.jarek/i-compared-5-ai-agent-memory-systems-across-6-dimensions-none-wins-6a658335ed0a — comparativa técnica
23. https://arxiv.org/pdf/2603.04445 — survey routing/cascading
24. https://arxiv.org/pdf/2601.07206 — LLMRouterBench
25. https://www.langchain.com/resources/ai-agent-frameworks — landscape 2026
26. https://arize.com/guides/ai-agent-handbook/agent-frameworks/ — comparativa frameworks
