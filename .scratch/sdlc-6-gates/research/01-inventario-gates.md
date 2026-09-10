# Inventario de gates existentes en mmorch

Ticket: `../issues/01-inventario-gates-existentes.md`. Fecha: 2026-09-10.
Fuente: lectura directa del código. No se modificó nada.
Regla de esta nota: si un gate no está en el código, no aparece.

## Clases

- **Determinista**: código propio o ejecución real decide. Cero LLM.
- **Sintetizado**: un modelo escribió el checker una vez. Un sandbox lo corre después.
- **Juicio LLM**: un modelo emite el veredicto en cada corrida.
- **Juicio humano**: una persona aprueba o rechaza. El código solo guarda el estado.

## Números medidos disponibles

Fuente: `README.md` sección "Measured". Experimentos en `logs/ablation_results.jsonl`: `ablation_paired` (4 filas), `ablation_stages` (1), `ablation_stages_hard` (10), `ablation_synth_kinds` (10).

- LLM-juez sobre checkeable difícil: ~74% false-refute (`ablation_prompt`, n=120). Ese experimento NO está en el jsonl actual. Solo lo cita `checkers.py:3`.
- `ablation_paired` (n=350): `deepseek-reasoner` acc. balanceada 0.997. `deepseek-chat` sin thinking 0.77.
- `ablation_stages` (n=299): la especificidad de una cadena es la de su peor gate. `deepseek-v4-pro` 1.000.
- `ablation_stages_hard` (n=50-80): gate `code:` 0.98-1.00. Gate `synth:` 1.00. Manual 0.35-0.50.
- `ablation_synth_kinds` (57 tipos, 6 modelos): 0 bugs perdidos en 274 funciones. `deepseek-reasoner` 57/57 a $0.03.

Todos esos números miden métodos de verificación sobre aritmética con verdad computada. Ningún gate del engine `/project` tiene número propio. Eso es un hallazgo, no un descuido de esta nota.

## Tabla de gates y chequeos

| # | Gate / chequeo | Módulo:línea | Entrada | Qué decide | Clase | Número medido | Qué pasa al fallar |
|---|---|---|---|---|---|---|---|
| 1 | `gate_policy.start` / `advance` | `mmorch/gate_policy.py:14`, `:30` | `policy {stages, comment_required}`, `action`, `actor`, `comment` | Avanza una etapa, rechaza o pide cambios. No mira contenido. | Juicio humano | No | Devuelve `error` sin cambiar estado (sin comentario, gate terminal). En `server.py:636` el actor es obligatorio. `approved` → job `done`; `rejected` → job `failed`. |
| 2 | Registro `checkers.check` | `mmorch/checkers.py:570-608` | `name` + kwargs del oráculo | Despacha a uno de 21 oráculos. `KeyError` si no existe. | Determinista | Solo el contraste: LLM-juez 74% false-refute (`checkers.py:3`) | `CheckResult.passed=False` + `detail`. El caller decide. |
| 2a | `arithmetic` | `checkers.py:98` | `expr`, `expected` | Re-evalúa en sandbox AST y compara. | Determinista | Contraste 74% | `passed=False`; expr fuera de whitelist → `UnsafeExpr` capturado. |
| 2b | `determinant` | `checkers.py:142` | `matrix`, `expected` | Bareiss entero exacto. | Determinista | No | `passed=False`. |
| 2c | `json_schema` | `checkers.py:110` | `data`, `schema` | Shape/type contra JSON-schema. | Determinista | No | `passed=False` con errores. |
| 2d | `predicate` | `checkers.py:152` | `value`, `predicate` | Corre un predicado del caller. | Determinista | No | `passed=False`. |
| 2e | `checksum` | `checkers.py:166` | `value`, `kind` (luhn/isbn10/isbn13/ean13) | Dígito verificador. | Determinista | No | `passed=False`. |
| 2f | `python_ast_valid` | `checkers.py:187` | `code` | Parsea sin ejecutar. | Determinista | No | `passed=False` con `SyntaxError`. |
| 2g | `regex_format` | `checkers.py:205` | `value`, `fmt` o `pattern` | Fullmatch de patrón nombrado o custom. | Determinista | No | `passed=False`. |
| 2h | `set_equal` | `checkers.py:216` | `a`, `b` | Igualdad de conjuntos. | Determinista | No | `passed=False`. |
| 2i | `numeric_close` | `checkers.py:222` | `a`, `b`, `tol` | Distancia ≤ tol. | Determinista | No | `passed=False`. |
| 2j | `sorted_monotonic` | `checkers.py:228` | `seq`, `direction`, `strict` | Monotonía. | Determinista | No | `passed=False`. |
| 2k | `number_theory` | `checkers.py:261` | `n`, `claim` (prime/composite) | Miller-Rabin determinista. | Determinista | No | `passed=False`. |
| 2l | `sql_valid` | `checkers.py:273` | `sql`, `dialect` | Parse con sqlglot. No ejecuta. | Determinista | No | `passed=False`. Sin dep → `passed=False`. |
| 2m | `units` | `checkers.py:286` | `quantity`, `to`, `expected` | Conversión con pint. | Determinista | No | `passed=False`. Sin dep → `passed=False`. |
| 2n | `sympy_identity` | `checkers.py:301` | `lhs`, `rhs` | `simplify(lhs-rhs)==0`. | Determinista | No | `passed=False`. Sin dep → `passed=False`. |
| 2o | `python_exec` | `checkers.py:316` | `code`, `expected_stdout`, `expect_ok`, `timeout=15` | Corre en sandbox. rc==0 y stdout. | Determinista (ejecución) | No | `passed=False` con rc/stdout/stderr. |
| 2p | `unit_test` | `checkers.py:336` | `code`, `tests` | pytest en sandbox sobre `candidate.py`. | Determinista (ejecución) | No | `passed=False` con cola de salida. |
| 2q | `code_quality` | `checkers.py:377` | `code`, `min_score=0.5` | Score 0..1: radon CC + MI + smells AST. | Determinista (heurística con umbral) | No | `passed=False`, `got=score`. |
| 2r | `mutation_score` | `checkers.py:467` | `code`, `tests`, `min_score`, `max_mutants=12` | Muta el código. Mide tests que matan mutantes. | Determinista (ejecución) | No | `passed=False`. Sin mutantes → `passed=False`. |
| 2s | `coverage` | `checkers.py:507` | `code`, `tests`, `min_cov=80` | coverage.py branch en sandbox. | Determinista (ejecución) | No | `passed=False`. |
| 2t | `deterministic` | `checkers.py:523` | `code`, `runs=3` | Corre N veces. Exige salida idéntica. | Determinista (ejecución) | No | `passed=False` con cantidad de salidas distintas. |
| 2u | `no_tell` | `checkers.py:539` | `preferred`, `alternatives` | Heurísticas anti-delación en sets generados. | Determinista (heurística) | No (cita quizzes de Estudio) | `passed=False` con violaciones. |
| 3 | `synth_store.get` / `run` | `mmorch/synth_store.py:118`, `:146` | `kind`, `params`, `timeout=20` | Corre `solve(params)` promovido en sandbox. Devuelve `int`. | Sintetizado | `ablation_synth_kinds`: 0 bugs perdidos, 57/57. `ablation_stages_hard`: synth 1.00 | `None`. No distingue "tipo nuevo" de "falló" ni de "timeout". |
| 4 | `synth_store.put` | `mmorch/synth_store.py:127` | `kind`, `src`, `model`, `evidence {n_promote, edge, n_test, n_test_ok}` | Guarda solo si la evidencia nueva supera la guardada. | Determinista (regla de evidencia) | Idem #3 | Devuelve `False`. No escribe. |
| 4a | Promoción de un checker sintetizado | `ablation_synth_kinds.py:426-481` (NO en `mmorch/`) | `src` del modelo, 3 items aleatorios + 1 de borde con verdad computada | Promueve si acierta los 4. | Determinista sobre verdad computada | Idem #3 | No promueve. El tipo entero queda refutado. |
| 5 | `validate_test_cmd` | `mmorch/project_build.py:294` | `test_cmd` | Rechaza metacaracteres de shell. Exige binario en allowlist. | Determinista | No | En plan: error → reask. En ejecución (`project_integrate.py:146`): `(False, "REJECTED")`, no ejecuta. |
| 6 | `validate_worklist` | `mmorch/project_build.py:380` | `units`, `external_test` | No vacío. Sin unidad "acceptance". Nombres y archivos únicos. Spec no vacío. Deps resuelven. `test_cmd` válido. DAG. | Determinista | No (comentarios citan casos A/B 2026-09-10) | `decompose` re-pregunta al planner hasta 3 veces (`:544`). Después `ValueError`. En el driver (`project_driver.py:108`): `escalate "invalid plan"`. |
| 7 | `stub_check` | `mmorch/project_build.py:475` → `mmorch/lang.py:48/99/124` | `code`, `file` | Stub si no hay defs, todos triviales, o syntax error. `__init__.py` solo debe parsear. | Determinista | No | `build_unit` devuelve `recurse` (`project_driver.py:66`). Descompone la unidad. Tope de profundidad → `escalate`. |
| 8 | Loop caliente del coder | `mmorch/project_integrate.py:314` | `unit`, feedback previo | Genera → corre `test_cmd` → reintenta hasta `max_fix`. | Determinista (ejecución) | No | Devuelve el último código. `stub_check` y `gate_fn` deciden. |
| 9 | Gate frío con `test_cmd` | `mmorch/project_integrate.py:344-352` | `unit`, `code` | Re-corre `test_cmd` limpio. Sin ver el razonamiento del coder. | Determinista (ejecución) | No | `(False, out)`. Guarda contraejemplo para el próximo intento. `build_unit` reintenta hasta `max_fix` → `escalate`. |
| 10 | Piso sintáctico sin `test_cmd` | `mmorch/project_integrate.py:54`, `:355` | `code`, `file` | Parsea en su lenguaje (py=AST, js=`node --check`, otro=pasa). | Determinista | No | `(False, "does not parse")`. |
| 11 | Sonda fría advisory | `mmorch/project_integrate.py:161`, `:175`, `:362-366` | `code`, `spec` | Un modelo cross-family escribe asserts. `python_exec` los corre. Solo Python. | Juicio LLM + ejecución | No | NO falla el gate. Guarda feedback. La unidad queda `unverified`. |
| 12 | Guard cross-family | `mmorch/project_integrate.py:249` | `gen_model`, `verifier_model` | Familias distintas. | Determinista | Contexto: README dice que la ganancia medida fue thinking, no familia | `ValueError`. No arranca el build. |
| 13 | Call-breaker y USD-breaker | `mmorch/project_integrate.py:276-284` | contador de llamadas, USD acumulado | `> max_gen_calls` o `> max_usd_per_run` (default $5). | Determinista | No | `RuntimeError`. `build_unit` lo cuenta como intento fallido → `escalate`. |
| 14 | `_safe_target` | `mmorch/project_integrate.py:40` | `repo`, `unit.file` | El path queda dentro del repo. | Determinista | No | `ValueError`. |
| 15 | Gate de integración | `mmorch/project_driver.py:129-135`; `mmorch/project_integrate.py:187` | `external_test` (del usuario), `results` | rc==0 del test de aceptación sobre el todo ensamblado. | Determinista (ejecución) | No | `integration_failed` con salida. Sin re-plan automático. Escala a humano/Opus. |
| 16 | Tope de recursión | `mmorch/project_driver.py:105` | `depth`, `max_depth=2` | Profundidad dentro del tope. | Determinista | No | `escalate "max recursion depth"`. |
| 17 | Re-gate del cache de unidad | `mmorch/project_driver.py:42-54` | código cacheado | El código cacheado pasa `stub_check` y `gate_fn` de nuevo. | Determinista | No | Cae al loop normal. Regenera. |
| 18 | Gate de hardening | `scripts/gate_hardening.py:17`; invocado en `mmorch/hardening.py:285` | `module`, `baseline_survived` | `survived < baseline` y suite completa verde. | Determinista (mutation testing) | No | exit 1 → build no `built`. Branch descartada (`hardening.py:312`). Módulo bloqueado 7 días (`:185`). |
| 18a | Elegibilidad de hardening | `mmorch/hardening.py:206`, `:237` | mapa `worst`, `hardening_state.json`, `loop_paused` | Peor módulo con sobrevivientes no intentado en 7 días. | Determinista | No | `skipped`. El nightly sigue. |
| 19 | `never-edit-guard` | `~/.claude/hooks/never-edit-guard.js:50` (`decide`) | Payload PreToolUse: `tool_name`, `file_path` | Deny si el path matchea `~/.claude/never-edit.txt`. Solo Write/Edit/MultiEdit/NotebookEdit. | Determinista | No | `permissionDecision: deny`. Error de infra → fail-open. Solo frena tools de Claude. No frena `write_file` del engine. |
| 20 | `ets-guard` | `~/.claude/hooks/ets-guard.js:167` (`check`) | Último texto del asistente en el transcript | R1 ≤20 palabras. R2 ≤6 oraciones por párrafo. R3 un item = una oración. | Determinista (regex) | No | `decision: block` con lista de oraciones. No loopea si `stop_hook_active`. Error de infra → fail-open. |

## Observaciones que el contrato del ticket 02 debe absorber

1. Hay dos contratos de salida distintos. `checkers` devuelve `CheckResult(passed, detail, checker, expected, got)`. El engine usa `tuple[bool, str]`. `synth_store.run` devuelve `int | None`. Un contrato nuevo tiene que unificar o adaptar los tres.
2. `synth_store` no tiene consumidor en `mmorch/`. Solo lo usan `ablation_synth_kinds.py` y su test. Ningún gate del engine lo llama hoy.
3. La promoción vive en el script de ablación, no en el paquete. `synth_store.put` confía en la evidencia que le pasan.
4. `synth_store.run` colapsa tres casos en `None`. Un gate fail-closed necesita distinguirlos.
5. El único juicio LLM del engine es la sonda fría (#11). Es advisory y no tiene número medido. Cumple el invariante del mapa por no bloquear.
6. Los checkers `coverage`, `mutation_score`, `code_quality` y `deterministic` existen pero el engine no los llama. Solo usa `python_exec` vía `run_snippet`.
7. `gate_policy` es un state machine de aprobación humana. No evalúa nada. Sirve como envoltorio de etapa, no como gate de contenido.
8. `never-edit-guard` protege al usuario de Claude. No protege al repo del engine. El engine escribe con `write_file` sin lista de archivos permitidos.

## Huecos frente a las 6 etapas (intent, spec, plan, build, test, PR)

**Intent**: no hay ningún gate. Ningún módulo leído recibe una intención como entrada. **Spec**: no hay gate en los módulos leídos. El engine recibe `task` como texto y lo pasa entero al coder (`project_integrate.py:81`). Existe una tool MCP `mmorch_build_spec` fuera del alcance de este ticket; no la leí y no afirmo que gatee. **Plan**: tiene gate estructural (#5, #6). No hay gate semántico. Nadie chequea que el plan cubra la spec. **Build**: es la etapa mejor cubierta (#7 a #14, #16, #17). **Test**: tiene el gate de integración (#15) solo si el usuario da `external_test`. Sin `external_test` la etapa no existe y el status es `built` igual (`project_driver.py:136`). Los oráculos de tests (#2p, #2r, #2s) existen pero no están cableados. El gate de hardening (#18) aplica solo al repo de mmorch en el nightly. **PR**: no hay gate de contenido. `gate_policy` (#1) registra la aprobación humana. `commit_fn` commitea por unidad sin diffstat, sin lista de archivos permitidos y sin revisión. Resumen: intent, spec y PR tienen cero gates hoy. Plan tiene solo estructura. Test depende de un comando externo opcional.
