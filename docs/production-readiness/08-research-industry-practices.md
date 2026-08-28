# Prácticas de industria 2025–2026 para agentes LLM en producción — aplicadas a mmorch

> Investigación externa, 2026-08-27. Cada práctica cita su fuente primaria (leída, no de memoria)
> y cierra con **→ mmorch**: cómo aplica al orquestador multi-modelo (DeepSeek/Gemini baratos +
> Claude juez, auto-evolución gateada, memoria episódica+semántica DuckDB, MCP server,
> bandit Thompson + calibración).

---

## 1. Patrones de orquestación

### 1.1 Workflows composables > frameworks (Anthropic, "Building Effective Agents")

Fuente: https://www.anthropic.com/engineering/building-effective-agents

- Las implementaciones más exitosas que Anthropic vio en clientes **no usaban frameworks
  complejos**: usaban patrones simples y composables. Cinco patrones canónicos: *prompt
  chaining* (con gates programáticos entre pasos), *routing* (clasificar y despachar a
  handlers especializados — explícitamente: mandar lo simple a un modelo barato tipo Haiku y
  lo complejo a uno caro), *parallelization* (sectioning y voting), *orchestrator-workers*
  (el orquestador descompone dinámicamente cuando los subtasks no son predecibles), y
  *evaluator-optimizer* (loop generador↔crítico, solo cuando hay criterio de evaluación claro).
- Distinción dura: **workflow** = código predefinido decide el camino; **agente** = el LLM
  dirige su propio proceso. Usar agente solo para problemas abiertos donde no se puede
  predecir el número de pasos; si el task es predecible, workflow (más barato, más predecible).
- Guardrails de producción: stopping conditions (máximo de iteraciones), checkpoints con
  feedback humano, testing extensivo en sandbox antes de deploy.
- Tool design = "ACI" (agent-computer interface) con el mismo rigor que una HCI: docstrings
  como para un dev junior, ejemplos y edge cases, y *poka-yoke* (reestructurar argumentos
  para que el error sea imposible — el caso famoso: pasar de paths relativos a absolutos
  eliminó los errores de navegación). El equipo de SWE-bench **pasó más tiempo optimizando
  tools que el prompt global**.

**→ mmorch**: valida la decisión de no adoptar framework (LangGraph/CrewAI) y construir
patrones propios (`cascade`, `fan_out`, `tournament`, `adversarial_verify` son exactamente
routing/parallelization/voting/evaluator-optimizer). Dos gaps accionables: (a) auditar cada
tool MCP de mmorch con el estándar poka-yoke — ¿algún argumento admite un valor que siempre
es un error? hacerlo imposible por firma, no por validación post-hoc; (b) el router Cynefin +
bandit ya implementa "routing barato/caro", pero conviene documentar por patrón cuál de los
5 canónicos es cada tool, y verificar que ninguno sea "agente" donde bastaría "workflow".

### 1.2 Lead–subagents en producción (Anthropic, multi-agent research system)

Fuente: https://www.anthropic.com/engineering/multi-agent-research-system

- Multi-agente (Opus lead + Sonnet subagents) superó a single-agent Opus por **90.2%** en su
  eval interno de research — pero consume **~15× los tokens de un chat** (un agente solo, ~4×).
  Regla: multi-agente solo cuando el valor del task paga ese costo, y solo cuando el task
  **se descompone en hilos paralelos independientes**. El uso de tokens explica el 80% de la
  varianza de performance en tasks de browsing.
- Delegación: cada subagente necesita **objetivo, formato de output, guía de tools, y
  fronteras claras del task** — sin eso, duplican trabajo o dejan huecos. Effort scaling
  explícito en el prompt del lead: "fact-finding simple = 1 agente con 3-10 tool calls;
  comparación directa = 2-4 subagentes con 10-15 calls cada uno".
- **Artifacts, no relay**: los subagentes guardan su trabajo en sistemas externos y devuelven
  referencias livianas al coordinador — evita pérdida de información en pipelines multi-etapa.
- Upgrade de modelo > más tokens: "pasar a Sonnet 4 rinde más que duplicar el budget de
  tokens en Sonnet 3.7".
- Un "tool-testing agent" que probaba tools y reescribía sus descripciones logró **-40% en
  tiempo de completación** para agentes futuros.

**→ mmorch**: (a) el `fan_out` debería imponer el template de delegación (objetivo + formato +
tools + fronteras) como schema obligatorio, no como convención; (b) adoptar effort-scaling
rules explícitas en el planner (mapear complejidad Cynefin → n° de workers y budget de calls);
(c) el patrón "artifacts + referencias livianas" es exactamente lo que conviene para los
handoffs entre roles del role-chain: escribir a disco/DuckDB y pasar paths, no pegar outputs
completos en el prompt del siguiente rol; (d) la idea del tool-testing agent es un candidato
barato para mmorch: un job nightly con DeepSeek que ejercite cada tool MCP y proponga mejoras
de descripción (gateado, como toda evolución).

### 1.3 OpenAI: primitivas y guía práctica

Fuentes: https://openai.github.io/openai-agents-python/tracing/ ·
https://cdn.openai.com/business-guides-and-resources/a-practical-guide-to-building-agents.pdf

- El Agents SDK (sucesor de Swarm) se organiza en 4 primitivas: **Agents, Tools, Handoffs,
  Guardrails**. Un handoff transfiere la *propiedad* del loop (no es un function call que
  retorna). Tracing built-in de todo: generaciones, tool calls, handoffs, guardrails.
- La guía práctica de OpenAI recomienda **guardrails en capas** (LLM-based + reglas/regex +
  moderación), y **risk-rating por tool**: clasificar cada tool por riesgo y usar ese rating
  para disparar chequeos automáticos o escalar a humano antes de ejecutar tools de alto riesgo.

**→ mmorch**: mmorch ya tiene el gate HITL para evolución; lo que falta es la **matriz de
riesgo por tool**: etiquetar cada tool MCP (read-only / mutating / outward-facing) y que el
orquestador exija confirmación solo sobre esa base — es la versión por-tool del esquema
always/ask/never que ya existe a nivel guardrails globales. Los handoffs con ownership claro
mapean al role-chain: un rol que "termina" no debería poder seguir mutando estado.

---

## 2. Contexto y memoria

### 2.1 Context engineering (Anthropic)

Fuente: https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents

- Principio guía: "el set más chico de tokens de alta señal que maximiza la probabilidad del
  resultado". Técnicas concretas: **compaction** (resumir y reiniciar ventana; primero
  maximizar recall, después precisión — lo primero que se tira: tool outputs viejos),
  **structured note-taking** (NOTES.md / to-do persistente fuera del contexto),
  **just-in-time retrieval** (mantener identificadores livianos — paths, queries — y cargar
  el dato en runtime, no precargar), y **sub-agents con contexto limpio** que devuelven
  resúmenes condensados de **1.000–2.000 tokens** al coordinador.
- "System prompt altitude": ni lógica hardcodeada frágil ni vaguedad — heurísticas.

**→ mmorch**: (a) fijar un **contrato de tamaño de retorno** para todo worker/subagente
(~1-2k tokens máx hacia el orquestador) y enforcearlo en el harness, no en el prompt;
(b) el recall de mmorch ya es just-in-time — mantener la disciplina de pasar claves/ids de
DuckDB entre etapas en vez de contenido; (c) implementar compaction explícita en los loops
largos (project-build engine): al acercarse al límite, resumir con la regla recall-primero y
descartar tool outputs viejos primero.

### 2.2 Memoria de agentes en producción

Fuentes: https://github.com/NirDiamant/Agent_Memory_Techniques ·
https://mem0.ai/blog/state-of-ai-agent-memory-2026 · https://arxiv.org/html/2604.22085v1 (Memanto)

- El paisaje 2026: Mem0 (jerarquía user/session/agent, vector+grafo+KV), Letta/MemGPT
  (memoria auto-editada, arquitectura tipo OS con recall memory en DB), Zep/Graphiti
  (knowledge graph temporal; episódico→semántico). En LongMemEval, Zep 63.8% vs Mem0 49.0%
  (la ventaja se atribuye al grafo temporal); Memanto (typed semantic memory, vector-only)
  reporta +22.9pp sobre Mem0 en LongMemEval.
- Gap señalado por la literatura: **todos los sistemas de memoria benchmarkean recall
  conversacional (LoCoMo/LongMemEval); ningún benchmark de coding mide memoria entre
  episodios** — la evidencia de que "memoria mejora agentes de código" es débil todavía.

**→ mmorch**: la arquitectura DuckDB episódica+semántica con decay está alineada con el
estado del arte (el patrón Graphiti "episódico destila a semántico" es exactamente
`consolidate`/`distill_backlog`). Dos ideas robables baratas: (a) **timestamps bi-temporales**
(cuándo ocurrió vs cuándo se supo — clave del grafo temporal de Zep) para invalidar hechos
semánticos sin borrarlos; (b) no invertir más en memoria sin un eval propio: como no existe
benchmark de memoria-para-código, medir con A/B interno (misma tarea con/sin recall) antes de
agregar complejidad — coherente con el hallazgo previo de mmorch de que el label era el
cuello, no la representación.

### 2.3 Harness para agentes de larga duración (Anthropic)

Fuente: https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents

- Patrón: **initializer agent** (una vez: init.sh, progress file, commit inicial) + **coder
  agent** por sesión con mandato de "progreso incremental, estado siempre mergeable".
- Estado en archivos: feature list JSON (cada feature con pasos de verificación y booleano
  `passes`; prohibido editar/borrar tests), progress file leído al arrancar, git como
  historia navegable para rollback.
- Cada sesión arranca: leer git log + progress → correr health check → **arreglar lo roto
  antes de features nuevas** (evita fallas que se componen entre sesiones). Verificación
  "como usuario humano" (dar al agente browser automation mejoró mucho la detección de bugs).

**→ mmorch**: el project-build engine (F1-F4) ya usa worktrees + gates; adoptar (a) el
**health-check de arranque de sesión** como paso obligatorio del loop por unidad — antes de
codear, correr el acceptance parcial y reparar; (b) la regla "prohibido tocar los tests del
gate" como invariante hard del engine (hoy es convención); (c) feature-list JSON con `passes`
por unidad como formato del tracker, en vez de markdown libre.

---

## 3. Observabilidad y evals

### 3.1 OpenTelemetry GenAI semantic conventions

Fuentes: https://opentelemetry.io/blog/2026/genai-observability/ ·
https://opentelemetry.io/docs/specs/semconv/registry/attributes/gen-ai/

- Esquema estándar bajo namespace `gen_ai.*`. Jerarquía de spans: **`invoke_agent`** (raíz)
  → **`chat`** (cada llamada LLM) → **`execute_tool`** (cada tool). Atributos clave:
  `gen_ai.request.model`, `gen_ai.usage.input_tokens` / `output_tokens`,
  `gen_ai.response.finish_reasons`, y contenido opcional (`gen_ai.input.messages`, etc.).
- Métricas recomendadas: **`gen_ai.client.operation.duration`** (histograma de latencias,
  filtrable por modelo) y **`gen_ai.client.token.usage`** (histograma, filtrable
  input/output). Estado: chat/embeddings estable para dashboards; convenciones de agentes
  y tool-orchestration aún "in development" (tratarlas como provisionales).

**→ mmorch**: renombrar/mapear el logging interno de mmorch a los atributos `gen_ai.*` es
barato y a prueba de futuro (cualquier backend — Jaeger, Datadog, Grafana — los entiende
nativamente). Concretamente: cada llamada a DeepSeek/Gemini/Claude emite un span `chat` con
modelo+tokens+finish_reason; cada patrón (`cascade`, `fan_out`) es un `invoke_agent`; cada
tool MCP un `execute_tool`. Aunque no se despliegue un collector hoy, loggear con esos
nombres de campo en DuckDB deja el join listo.

### 3.2 Qué monitorean los sistemas serios (Anthropic multi-agent)

- Tracing completo de producción para diagnosticar fallas sistemáticamente; monitoreo de
  **patrones de decisión y estructuras de interacción** (no contenido de conversaciones);
  los sistemas multi-agente tienen **comportamiento emergente** — un cambio chico en el lead
  cambia impredeciblemente a los subagentes — así que hay que observar más allá del agente
  individual.

**→ mmorch**: agregar a `metrics_summary` vistas de *estructura*: distribución de n° de
tool calls por task, profundidad de cascada, tasa de re-planning — no solo tasas de error
por modelo. Es lo que permite detectar regresiones emergentes tras un cambio de prompt.

### 3.3 LLM-as-judge que aguanta producción

Fuentes: https://arize.com/blog/how-to-build-llm-as-a-judge-evaluators-that-hold-up-in-production/ ·
https://www.anthropic.com/engineering/multi-agent-research-system · https://deepeval.com/blog/llm-as-a-judge

- **Calibrar contra humanos**: etiquetar un set de validación, comparar con accuracy, Cohen's
  kappa, precision/recall y matrices de confusión; "85% de agreement puede ser inutilizable
  si falla justo en los errores que te importan" — inspeccionar slices de desacuerdo.
- **Labels booleanos/categóricos > escalas 1-10** sin anclas (los judges inventan su propia
  escala y driftea entre modelos). Incluir `insufficient_evidence` en vez de forzar binario.
- **Pinnear la versión del modelo juez** (estándar, no opcional) y re-correr un **canary set
  fijo** cuando cambia el juez; testear position bias intercambiando el orden de los pares.
- Anthropic: un solo call de juez con rúbrica (accuracy factual, accuracy de citas,
  completitud, calidad de fuentes, eficiencia de tools) con score 0-1 + pass/fail fue lo más
  consistente; **empezar con ~20 queries reales**, no esperar el eval grande; para tasks con
  estado, evaluar **end-state**, no cada paso intermedio; humanos siguen encontrando lo que
  el eval no ve (ej.: sesgo SEO en selección de fuentes).

**→ mmorch**: esto pega directo en `adversarial_verify`, `ensemble_verify` y la calibración:
(a) el verificador cross-family debería emitir **veredicto categórico + evidencia**, nunca
score numérico crudo — y la calibración (ECE 0.456 medido) mejora antes anclando labels que
ajustando curvas; (b) armar el **canary set de ~20 tareas reales** y correrlo cuando cambie
cualquier modelo de la flota (DeepSeek/Gemini rotan versiones sin aviso — es el "temporal
drift" exacto que describe Arize); (c) pinnear versiones de modelo del juez en config, y
loggear la versión en cada outcome del bandit para poder invalidar historia cuando rota.
El hallazgo previo de mmorch ("LLM code-review alucina, los gates estáticos son la capa
confiable") es consistente con esta literatura: judge para lo subjetivo, gates deterministas
para lo estructural.

---

## 4. Resiliencia

Fuentes: https://portkey.ai/blog/retries-fallbacks-and-circuit-breakers-in-llm-apps/ ·
https://tianpan.co/blog/2026-03-11-llm-api-resilience-production ·
https://www.anthropic.com/engineering/multi-agent-research-system

- Los providers LLM corren a **99–99.5% uptime** (6–14× peor que infra cloud tradicional):
  la resiliencia no es opcional. Stack mínimo viable: retry con **exponential backoff +
  jitter** (respetando `Retry-After` del provider; retriable: 429/502/503 y timeouts),
  **fallback multi-provider** para fallas persistentes (cadena típica: primario → mismo
  provider más barato → provider distinto → local), y **circuit breaker** con cooldown fijo
  para degradación sistémica (evita retry storms).
- Anthropic (multi-agente): los agentes son **stateful** — un error menor no debe reiniciar
  desde cero. Prácticas: **checkpoints regulares + resume desde el punto de falla**;
  **avisarle al modelo que una tool está fallando y dejarlo adaptarse** ("funciona
  sorprendentemente bien"); **rainbow deployments** (traffic gradual viejo→nuevo, ambos
  vivos) para no romper agentes en vuelo.
- Idempotencia: los pasos de un pipeline de agente deben poder re-ejecutarse sin duplicar
  side-effects (MightyBot: https://mightybot.ai/blog/fault-tolerant-ai-agent-pipelines/).

**→ mmorch**: (a) el half-open breaker ya está en el backlog (repo-mining 2026-07) — esta
literatura lo confirma como prioridad, con el detalle de cooldown fijo + re-test gradual;
(b) el health floor ya cubre parte del fallback, pero la **cadena explícita de degradación
por rol** (DeepSeek → Gemini → Claude solo si el task lo amerita) debería ser config
declarativa, no lógica ad-hoc; (c) robar el patrón "decile al modelo que la tool falló":
cuando un provider da timeout/429 persistente, inyectar eso como contexto y dejar que el
orquestador re-planifique, en vez de abortar; (d) checkpointing por unidad en el
project-build engine ya existe vía worktree+commits — falta el **resume**: reanudar un build
interrumpido desde el último gate verde en vez de re-decomponer.

---

## 5. Seguridad

### 5.1 Prompt injection y la "lethal trifecta"

Fuentes: https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/ ·
https://arxiv.org/pdf/2503.18813 (CaMeL, Google DeepMind) ·
https://simonwillison.net/2025/Apr/11/camel/

- **Trifecta letal**: acceso a datos privados + exposición a contenido no confiable +
  capacidad de comunicar hacia afuera. Con las tres juntas, un contenido envenenado alcanza
  para exfiltrar. Los guardrails probabilísticos ("atrapa 95% de ataques") son **nota de
  aplazo en seguridad** — la única defensa confiable es **no combinar las tres** por diseño.
- **CaMeL** (DeepMind): extraer control-flow y data-flow del query *confiable* y adjuntar
  **capabilities** (metadata de seguridad) a cada valor, de modo que datos no confiables
  jamás puedan alterar el flujo del programa — Control Flow Integrity + Information Flow
  Control aplicados a agentes. Código: github.com/google-research/camel-prompt-injection.
- En pipelines multi-agente el riesgo se propaga: el output de un agente expuesto a contenido
  no confiable **es contenido no confiable** para el siguiente (OpenAI: pasar untrusted input
  como user message, nunca a contextos privilegiados).

**→ mmorch**: mmorch hoy casi no toca contenido externo, pero `autoresearch` y cualquier tool
de web rompen eso. Reglas concretas: (a) **taint-tracking barato**: marcar en el estado del
pipeline qué outputs derivan de contenido externo (web, repos ajenos) y prohibir que esos
outputs lleguen a tools mutantes u outward-facing sin gate HITL — es CaMeL simplificado;
(b) los workers baratos (DeepSeek/Gemini) que procesan web son la superficie de inyección:
su output hacia el orquestador debe tratarse como datos (schema estricto, nunca instrucciones);
(c) verificar que ningún flujo combine la trifecta: si un pipeline lee vault/memoria (privado)
y navega web (untrusted), no debe poder escribir hacia afuera en la misma corrida.

### 5.2 Aislamiento de código generado

Fuentes: https://northflank.com/blog/how-to-sandbox-ai-agents ·
https://dev.to/aiagentengineering/how-to-sandbox-ai-agents-in-2026-firecracker-gvisor-runtimes-isolation-strategies-14pk

- Consenso 2025-2026: **contenedor con kernel compartido (Docker/runc) ya no alcanza** para
  código no confiable; tratar código LLM-generado como hostil. Escalera: microVMs
  (Firecracker: kernel dedicado por workload, boot 100-125ms) > gVisor (kernel user-space,
  intercepta syscalls, overhead I/O 10-30%) > contenedor hardened (solo código confiable).
  Google lanzó Agent Sandbox (CNCF, KubeCon NA 2025) con gVisor default.
- Defensa en profundidad: aislamiento + límites de recursos + **network egress controlado** +
  permisos mínimos + **audit trail inmutable** de cada ejecución y tool call, monitoreando
  conexiones de red inesperadas y picos de consumo.

**→ mmorch**: la regla existente "aislar código LLM en subprocess" es la versión débil del
estándar. En Windows sin KVM, lo pragmático: (a) subprocess con **working dir aislado
(worktree), sin credenciales en env, y timeout duro** como piso; (b) para el project-build
engine, negar egress de red durante la ejecución de código generado (el acceptance test no
debería necesitar internet) — es el mitigador más barato de la trifecta; (c) loggear cada
ejecución de código generado en DuckDB como audit trail (qué unidad, qué hash, exit code).

### 5.3 Least privilege para tools

Fuentes: https://developers.openai.com/api/docs/guides/agent-builder-safety ·
https://cdn.openai.com/business-guides-and-resources/a-practical-guide-to-building-agents.pdf

- Risk-rating por tool + gates automáticos sobre tools de alto riesgo + escalamiento a
  humano; guardrails en capas (reglas deterministas primero, LLM después).

**→ mmorch**: formalizar la matriz por tool MCP de mmorch (hoy el guard never-edit cubre
archivos, no tools): cada tool declara `risk: read|mutate|outward` y el server enforcea el
gate según nivel. Es una tarde de trabajo y cierra el gap más grande entre mmorch y la
práctica publicada.

---

## Síntesis: brechas priorizadas para mmorch

| # | Práctica | Fuente | Esfuerzo | Impacto |
|---|----------|--------|----------|---------|
| 1 | Canary set (~20 tasks reales) + pin/log de versión de modelo por outcome | Arize, Anthropic | Bajo | Alto — DeepSeek/Gemini rotan sin aviso; protege bandit y calibración |
| 2 | Matriz de riesgo por tool MCP (`read/mutate/outward`) con gate enforced | OpenAI guide | Bajo | Alto — cierra least-privilege |
| 3 | Contrato de retorno de workers (~1-2k tokens, schema, artifacts-por-referencia) | Anthropic ×2 | Bajo | Alto — costo y pérdida de info en handoffs |
| 4 | Half-open circuit breaker + cadena de fallback declarativa + "avisar al modelo que la tool falló" | Portkey, Anthropic | Medio | Alto — providers a 99-99.5% uptime |
| 5 | Taint-tracking de contenido externo + no-trifecta por pipeline + egress off en código generado | Willison, CaMeL | Medio | Alto — pre-requisito para autoresearch seguro |
| 6 | Campos `gen_ai.*` (OTel) en el logging DuckDB | OTel semconv | Bajo | Medio — interop futura gratis |
| 7 | Labels categóricos anclados en verificadores (nunca score 1-10 crudo) | Arize, DeepEval | Bajo | Medio — ataca el ECE 0.456 por el lado correcto |
| 8 | Resume-desde-último-gate en project-build engine + health check de arranque de sesión | Anthropic harnesses | Medio | Medio |
| 9 | Timestamps bi-temporales en memoria semántica | Zep/Graphiti | Medio | Medio — invalidación sin borrado |
| 10 | Tool-testing agent nightly (DeepSeek) que propone mejoras de descripciones, gateado | Anthropic multi-agent | Medio | Medio — el caso citado dio -40% tiempo de task |

## Fuentes (leídas)

1. Anthropic — Building Effective Agents: https://www.anthropic.com/engineering/building-effective-agents
2. Anthropic — How we built our multi-agent research system: https://www.anthropic.com/engineering/multi-agent-research-system
3. Anthropic — Effective context engineering for AI agents: https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents
4. Anthropic — Effective harnesses for long-running agents: https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents
5. OpenAI — Agents SDK tracing docs: https://openai.github.io/openai-agents-python/tracing/
6. OpenAI — A practical guide to building agents (PDF): https://cdn.openai.com/business-guides-and-resources/a-practical-guide-to-building-agents.pdf · Safety guide: https://developers.openai.com/api/docs/guides/agent-builder-safety
7. OpenTelemetry — GenAI observability blog (2026): https://opentelemetry.io/blog/2026/genai-observability/ · registry: https://opentelemetry.io/docs/specs/semconv/registry/attributes/gen-ai/
8. Simon Willison — The lethal trifecta: https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/ · sobre CaMeL: https://simonwillison.net/2025/Apr/11/camel/
9. Debenedetti et al. (Google DeepMind) — Defeating Prompt Injections by Design (CaMeL), arXiv:2503.18813: https://arxiv.org/pdf/2503.18813
10. Portkey — Retries, fallbacks, and circuit breakers in LLM apps: https://portkey.ai/blog/retries-fallbacks-and-circuit-breakers-in-llm-apps/ · TianPan — LLM API resilience in production: https://tianpan.co/blog/2026-03-11-llm-api-resilience-production
11. Arize — LLM-as-a-judge evaluators that hold up in production: https://arize.com/blog/how-to-build-llm-as-a-judge-evaluators-that-hold-up-in-production/ · DeepEval — LLM-as-a-judge 2026: https://deepeval.com/blog/llm-as-a-judge
12. Northflank — How to sandbox AI agents (microVMs/gVisor): https://northflank.com/blog/how-to-sandbox-ai-agents · dev.to — Sandboxing AI agents 2026: https://dev.to/aiagentengineering/how-to-sandbox-ai-agents-in-2026-firecracker-gvisor-runtimes-isolation-strategies-14pk
13. Memoria de agentes — Agent_Memory_Techniques (NirDiamant): https://github.com/NirDiamant/Agent_Memory_Techniques · Mem0 State of AI Agent Memory 2026: https://mem0.ai/blog/state-of-ai-agent-memory-2026 · Memanto (arXiv 2604.22085): https://arxiv.org/html/2604.22085v1
14. MightyBot — Fault-tolerant AI agent pipelines: https://mightybot.ai/blog/fault-tolerant-ai-agent-pipelines/
