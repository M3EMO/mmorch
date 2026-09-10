# Ticket 09 — Fix loops dirigidos y escalación en agentes de código publicados

Fecha: 2026-09-10. Fuentes primarias: papers arXiv, repos oficiales, docs oficiales.
Regla: si un dato no aparece en fuente primaria, dice "no publicado".

## Tabla comparativa

| Agente | (1) Elige archivos o reescribe todo | (2) Gate por vuelta y revert | (3) Vueltas máximo | (4) pass@1 SWE-bench (con / sin loop) | (5) Escalación a humano | Fuentes |
|---|---|---|---|---|---|---|
| **SWE-agent** (Princeton/Stanford) | El modelo elige. Abre un archivo y edita rangos de líneas: `edit n:m ... end_of_edit`. No reescribe archivos enteros. | Sí, gate de sintaxis por edición. Corre `flake8 --select=F821,F822,F831,E111,E112,E113,E999,E902` sobre la edición. Si hay error, la edición NO se aplica y el modelo recibe el error. Sin gate de tests obligatorio: el modelo decide cuándo correr `python`/tests. Sin revert por "empeorar tests". | Sin tope de turnos por default. Tope de costo: USD 4 por instancia en el paper; default actual `per_instance_cost_limit: 3.0`. `per_instance_call_limit` default 0 (ilimitado). `max_requeries: 3` errores de formato consecutivos. Al agotar cualquier límite, auto-submit del diff parcial (`exit_cost`, `exit_context`, `exit_format`). mini-SWE-agent: `step_limit: 250`, `cost_limit: 3`. | Paper (GPT-4 Turbo): 12.47% full, 18.00% Lite. Ablación del gate de lint: 18.0% → 15.0% Lite sin linter. SWE-agent 1.0 + Claude Sonnet 4: 69% Verified (345/500), single attempt. mini-SWE-agent: 65% Verified. Sin separación "con/sin loop de fix" más allá de la ablación del linter. | Sin escalación automática. Existe `HumanModel`: un humano opera la ACI en lugar del LLM. Al fallar, entrega el parche parcial. Éxito correlaciona con pocos pasos: mediana 12 pasos y USD 1.21 en resueltas, 21 pasos y USD 2.52 en fallidas. | [arXiv 2405.15793](https://arxiv.org/html/2405.15793), [model_config](https://swe-agent.com/latest/reference/model_config/), [agent_config](https://swe-agent.com/latest/reference/agent_config/), [agents.py](https://raw.githubusercontent.com/SWE-agent/SWE-agent/main/sweagent/agent/agents.py), [experiments README Claude 4](https://raw.githubusercontent.com/SWE-bench/experiments/main/evaluation/verified/20250522_sweagent_claude-4-sonnet-20250514/README.md), [swe-agent.com news](https://swe-agent.com/latest/), [mini-swe-agent swebench](https://mini-swe-agent.com/latest/usage/swebench/) |
| **OpenHands** (ex OpenDevin) | El modelo elige. Herramienta `str_replace_editor`: mismo esquema que el editor de texto de Anthropic. Edita por reemplazo de string, no reescribe todo. CodeAct v2.1 pasó a function calling. | Sin gate automático de tests o compilación. El agente corre bash cuando quiere. Sin revert automático. Hay **stuck detector** por default: misma acción-observación 4+ veces, misma acción-error 3+ veces, monólogo 3+ mensajes, patrón alternante 6+ ciclos, errores de contexto repetidos. | `max_iterations` configurable. Evaluaciones publicadas: 100 (paper SWE-bench Goes Live), 500 (submission Claude Sonnet 4 en Verified). Default del harness de benchmarks: no publicado en README; ejemplos con 100/300/500. | CodeAct v2.1 + Claude 3.5 Sonnet: 53.0% Verified (265/500). Claude Sonnet 4: 70.4% Verified (352/500), temperatura 0, 500 iteraciones, pass@1. SDK paper: Sonnet 4.5 72.8%, GPT-5 68.8%. Paper original: CodeActAgent v1.8 26.0% Lite. Separación con/sin loop: no publicado. | Confirmation policy: `AlwaysConfirm`, `NeverConfirm`, `ConfirmRisky`. El agente entra en `WAITING_FOR_CONFIRMATION` hasta que el usuario aprueba o rechaza. Security analyzer LLM clasifica LOW/MEDIUM/HIGH/UNKNOWN. Al rechazar, el usuario da feedback y el agente cambia de estrategia. El usuario puede interrumpir en cualquier momento. | [arXiv 2407.16741](https://arxiv.org/html/2407.16741), [blog CodeAct 2.1](https://www.openhands.dev/blog/openhands-codeact-21-an-open-state-of-the-art-software-development-agent), [experiments README Claude 4](https://raw.githubusercontent.com/SWE-bench/experiments/main/evaluation/verified/20250524_openhands_claude_4_sonnet/README.md), [experiments README v2.1](https://raw.githubusercontent.com/SWE-bench/experiments/main/evaluation/verified/20241029_OpenHands-CodeAct-2.1-sonnet-20241022/README.md), [stuck detector docs](https://docs.openhands.dev/sdk/guides/agent-stuck-detector), [security docs](https://docs.openhands.dev/sdk/guides/security), [SDK arXiv 2511.03690](https://arxiv.org/html/2511.03690v1), [benchmarks README](https://raw.githubusercontent.com/OpenHands/benchmarks/main/benchmarks/swebench/README.md), [openhands-aci](https://github.com/OpenHands/openhands-aci) |
| **Aider** | El usuario agrega archivos al chat. El modelo emite bloques search/replace sobre esos archivos. No reescribe todo. | Sí. Lint automático tras cada edición (`--auto-lint`, default True). Tests opcionales tras cada edición (`--auto-test`, default False, con `--test-cmd`). Si lint o test falla, el error vuelve al modelo como `reflected_message`. **No hay revert automático**: cada edición se commitea en git y el humano usa `/undo`. | `max_reflections = 3` en `base_coder.py`. Contador compartido entre error de edición, lint y test. Al agotar: `"Only 3 reflections allowed, stopping."`. | Verified: no publicado. Lite: 26.3% con 6 intentos (3 GPT-4o + 3 Opus alternados); 20.3% con solo el primer intento GPT-4o. Full: 18.9% con 2 intentos; 15.3% solo primer intento. Criterio "plausible": sin errores de edición, lint ni tests preexistentes. | Antes de cada reintento pregunta: `"Attempt to fix lint errors?"` / `"Attempt to fix test errors?"` (`confirm_ask`). `--yes-always` lo automatiza. Al agotar reflexiones, se detiene y deja al humano. | [lint-test docs](https://aider.chat/docs/usage/lint-test.html), [options](https://aider.chat/docs/config/options.html), [base_coder.py](https://raw.githubusercontent.com/Aider-AI/aider/main/aider/coders/base_coder.py), [git docs](https://aider.chat/docs/git.html), [SWE-bench Lite post](https://aider.chat/2024/05/22/swe-bench-lite.html), [SWE-bench full post](https://aider.chat/2024/06/02/main-swe-bench.html) |
| **Cursor** (Agent + Bugbot Autofix) | El modelo elige archivos y edita por diff. Mecanismo interno no publicado. | Lint: opción "Iterate on Lints"; la edición se aplica en shadow workspace y los lints vuelven al modelo. Tests por vuelta: no publicado. Revert: checkpoints por vuelta; el humano restaura. Bugbot Autofix corre en VM propia "to test your software"; detalle del gate de tests no publicado. | Agent: no publicado. Bugbot Autofix, modo "Commit to Existing Branch": máximo 3 intentos por PR "to prevent loops". | SWE-bench Verified: no publicado. Cursor lo descarta por contaminación. Composer 2: 73.7 SWE-bench Multilingual, 61.7 Terminal-Bench, 61.3 CursorBench. Bugbot Autofix: >35% de los cambios se mergean. | Run modes: Auto-review (clasificador + sandbox), Allowlist, Run Everything. Siempre requiere aprobación: browser, borrado de archivos, archivos fuera del workspace. Bugbot Autofix pushea a rama nueva (recomendado) y comenta el PR; el humano revisa. | [shadow workspace blog](https://cursor.com/blog/shadow-workspace), [Bugbot docs](https://cursor.com/docs/bugbot), [Bugbot Autofix blog](https://cursor.com/blog/bugbot-autofix), [run modes](https://cursor.com/docs/agent/security/run-modes), [agent overview](https://cursor.com/docs/agent/overview), [CursorBench](https://cursor.com/blog/cursorbench), [Composer 2 report](https://cursor.com/blog/composer-2-technical-report) |

## Lectura transversal

- Ningún agente reescribe archivos enteros. Todos usan edición dirigida: rango de líneas, string replace o search/replace.
- Solo SWE-agent y Aider tienen gate automático por vuelta. Los dos gatean lint, no tests. Aider puede gatear tests con opt-in.
- Ninguno hace revert automático "si empeora". SWE-agent rechaza la edición antes de aplicarla. Aider y Cursor delegan el revert al humano (`/undo`, checkpoints).
- El tope de vueltas cortas es 3 en dos sistemas independientes: Aider (`max_reflections`) y Bugbot Autofix (3 intentos por PR). SWE-agent usa 3 para errores de formato (`max_requeries`).
- El tope largo es por costo (USD 3-4) o por iteraciones (100-500). Al agotar, SWE-agent entrega el parche parcial en vez de fallar.
- Escalación a humano: nadie escala "porque no converge". Escalan por riesgo (OpenHands ConfirmRisky, Cursor Auto-review) o por límite agotado (Aider se detiene).
- Único dato "con/sin loop" separado es de Aider: +6 puntos Lite con reintentos, +3.6 puntos full. Y la ablación de SWE-agent: -3 puntos Lite sin gate de lint.

## 3 ideas robables

Objetivo: escalación por niveles = fix loop dirigido → diagnosticador con razonamiento → Claude → humano.

### 1. Gate barato ANTES de aplicar, con rechazo y no revert (SWE-agent)

SWE-agent corre lint sobre la edición propuesta. Si falla, la edición no toca el disco. El modelo recibe el error, la edición propuesta y el código original juntos. Esto vale 3 puntos de pass@1 en Lite.

Diseño para el nivel 1 (fix loop dirigido):
- Aplicar la edición en un worktree o copia temporal.
- Correr sintaxis + compilación + tests afectados sobre esa copia.
- Si falla: descartar la copia, devolver error + diff + original al modelo barato.
- Si pasa: promover al árbol real y commitear (Aider commitea cada edición; el commit es el checkpoint).
Nunca hay "empeorar": el árbol real solo avanza con verde.

### 2. Tope 3 por nivel, contador compartido, y auto-submit del parcial (Aider + SWE-agent + Bugbot)

Tres sistemas independientes convergen en 3 reintentos cortos. Aider comparte el contador entre error de edición, lint y test. SWE-agent al agotar costo entrega el diff parcial con un `exit_status` explícito, no crashea.

Diseño:
- Nivel 1 (modelo barato): 3 vueltas, contador único para todo tipo de fallo.
- Al agotar: empaquetar `{diff parcial, último error, historial de 3 intentos, exit_status}` y subir al nivel 2.
- Nivel 2 (diagnosticador con razonamiento, ej. deepseek-reasoner): 3 vueltas. Recibe el paquete completo, no el problema desde cero.
- Nivel 3 (Claude): 1-2 vueltas con el paquete de los dos niveles.
- Cada nivel emite un `exit_status` tipado: `submitted`, `exit_budget`, `exit_stuck`, `exit_format`. El estado dice por qué escaló.
El dato de SWE-agent respalda el tope corto: las instancias resueltas terminan en mediana 12 pasos; las fallidas gastan 21 pasos y el doble de costo. Más vueltas del mismo nivel compran poco.

### 3. Stuck detector como disparador de escalación, no solo de parada (OpenHands)

OpenHands corta cuando ve misma acción-observación 4+ veces, misma acción-error 3+ veces o ping-pong 6+ ciclos. Hoy solo detiene. La idea robable es usar esos patrones como señal de "cambiar de nivel" antes de agotar el presupuesto.

Diseño:
- Hashear (acción, observación) por vuelta.
- Mismo hash 2 veces en nivel 1 → escalar ya al nivel 2, aunque queden vueltas. El modelo barato no va a salir solo.
- Mismo error de test 2 veces en nivel 2 → escalar a Claude con la instrucción "el diagnóstico anterior fue X y no alcanzó".
- Nivel humano (OpenHands `WAITING_FOR_CONFIRMATION`, Cursor "always requires approval"): entrar por dos vías. Vía riesgo: acción destructiva o fuera del workspace, en cualquier nivel. Vía agotamiento: Claude también agotó. El humano recibe el paquete completo y el `exit_status` de cada nivel. Puede dar feedback corto que reinicia desde el nivel 2 (OpenHands: "provide feedback when rejecting to help the agent try a different approach").

## Datos no publicados (para no estimar)

- Aider en SWE-bench Verified.
- Cursor Agent: tope de vueltas, gate de tests por vuelta, pass@1 en Verified.
- OpenHands: pass@1 con vs. sin iteraciones extra; default de `max_iterations` en el harness V1.
- SWE-agent: número exacto Verified para Claude 3.7 (el metadata dice "Review Heavy", "2+ attempts"; no es pass@1 simple).
- Bugbot Autofix: si corre tests o CI antes de pushear.
