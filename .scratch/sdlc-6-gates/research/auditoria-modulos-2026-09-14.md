| veredicto | modulo | loc | entrada | tests | funciona | uso90d | selfcheck | importadores | proposito |
|---|---|---|---|---|---|---|---|---|---|
| borrar o cablear | context_blocks | 244 | NINGUNA | - | sin medir | 0 | si | 0 | context_blocks — the durable half of an "auto-compact to info-blocks" scheme for Claude Co |
| borrar o cablear | docgen | 239 | NINGUNA | test_docgen | si | 0 | si | 0 | docgen — vistas generadas + ratchet anti-copia. |
| borrar o cablear | adjudicate | 197 | NINGUNA | test_adjudicate | si | 0 | - | 0 | Adjudication module for matching notes to projects. |
| borrar o cablear | shadow_prior | 159 | NINGUNA | test_code_loop,test_shadow_prior | si | 0 | - | 0 | shadow_prior — Fase 5: una capa que PRIMEA al ThompsonBandit con un prior contextual, |
| borrar o cablear | predict | 118 | NINGUNA | test_predict | si | 0 | - | 0 | predict (v0.1 NN, Fase 1) — predictor de out_tokens / latencia, SIN dep pesada. |
| borrar o cablear | tournament | 109 | NINGUNA | test_tournament | si | 0 | - | 0 | tournament — elegir EL mejor de pocos candidatos por gusto/calidad (naming, |
| borrar o cablear | synth_store | 103 | NINGUNA | test_synth_store | si | 0 | - | 0 | synth_store — registro persistente de checkers SINTETIZADOS y promovidos, por tipo. |
| borrar o cablear | code_embedder | 102 | NINGUNA | - | sin medir | 0 | - | 0 | code_embedder — inferencia NUMPY PURA del encoder SimCLR del flywheel (sin torch). |
| borrar o cablear | bucketrank | 100 | NINGUNA | test_bucketrank,test_w33_w34_robustez | si | 0 | - | 0 | bucket_rank — graduar/ordenar un set GRANDE en tiers (triage por calidad, rankear |
| borrar o cablear | factory | 96 | NINGUNA | test_factory | si | 0 | - | 0 | factory — mmorch como FÁBRICA de modelos (no ES el modelo, lo CONSTRUYE/entrena). |
| borrar o cablear | cache | 84 | NINGUNA | test_innov_modules | si | 4 | - | 0 | memo (I-4) — cache content-hash de resultados/verdicts. Salta re-gen/re-verify |
| borrar o cablear | weights | 80 | NINGUNA | test_weights | si | 0 | - | 0 | weights — gestion de pesos de nodos neuronales (model-cards + verificacion). Source of |
| borrar o cablear | plugin_worker | 68 | NINGUNA | - | sin medir | 0 | si | 0 | plugin_worker — isolated subprocess host for ONE plugin invoke (graft G11). |
| borrar o cablear | schedule | 66 | NINGUNA | test_cost_savers | si | 0 | - | 0 | schedule — ADVISORY de ventana off-peak (DeepSeek descuenta fuerte fuera de hora pico). |
| borrar o cablear | megasource | 59 | NINGUNA | test_megasource | si | 0 | - | 0 | megasource (Fase 2) — megafuente autodidacta: primer hit = provider PRICING. |
| borrar o cablear | innovate | 50 | NINGUNA | test_innov_modules | si | 1 | - | 0 | innovate (I-5) — motor de innovacion productizado. mmorch se idea capacidades |
| borrar o cablear | effort | 37 | NINGUNA | test_cost_savers | si | 0 | - | 0 | effort — knob explicito de esfuerzo -> tier de modelo (patron Fable 5: 'effort' controla |
| arreglar o borrar | feedback | 212 | auto+manual+recurrente | test_bandit_unificado,test_cascade_bandit,test_code_loop,test_feedback,test_hillclimb,test_mcp_contract,test_paths,test_rubric_loop,test_shadow_prior | NO (1/165 rojos) | 0 | - | 21 | feedback — el lazo que faltaba (la 'loss' ausente). mmorch genera/verifica/ |
| arreglar o borrar | metrics | 212 | auto+manual+recurrente | test_cache_cost,test_cost_savers,test_error_rates,test_intuition_floor,test_paths,test_w33_w34_robustez,test_w6_ronda1 | NO (1/63 rojos) | 0 | - | 13 | Observability — append-only JSONL metric log (§11 backbone). |
| arreglar o borrar | paths | 61 | auto+manual+recurrente | test_paths,test_w6_ronda2 | NO (1/9 rojos) | 0 | - | 40 | Rutas de ESTADO del sistema (logs, DBs, bandits, memoria, cache). |
| manual sin uso: medir o borrar | wayfinder_prep | 155 | manual | test_wayfinder_prep | si | 0 | - | 0 | wayfinder-prep — investigación autónoma de tickets, decisión humana. |
| manual sin uso: medir o borrar | code_loop | 120 | manual | test_code_loop | si | 0 | - | 0 | code_loop — el WIRE de Fase 5 a produccion: tareas de CODIGO con lazo cerrado. |
| manual sin uso: medir o borrar | few_shot_bootstrap | 104 | manual | test_code_loop | si | 0 | si | 1 | few_shot_bootstrap — DSPy-A leído del código (bootstrap.py de stanfordnlp/dspy), robado co |
| manual sin uso: medir o borrar | decision_mining | 98 | manual | test_decision_mining | si | 0 | - | 0 | Mineria de DECISIONES humanas desde transcripts de Claude Code. |
| manual sin uso: medir o borrar | proposals | 77 | manual | test_proposals | si | 0 | - | 0 | F2 propuesta (spec .scratch/loop-cerrado/spec.md): tarjetas pre-cocinadas + pick del hook. |
| manual sin uso: medir o borrar | cli | 66 | manual | test_canary,test_w21_paquete | si | 0 | si | 0 | CLI minimo instalable (`mmorch`): status y health desde la terminal. |
| manual con uso: queda | canary | 149 | manual | test_canary | si | 120 | - | 1 | canary — set FIJO de tareas con respuesta verificable deterministicamente (W5.3). |
| manual con uso: queda | cascade | 120 | manual | test_cascade_bandit,test_code_loop,test_innov_modules | si | 67 | - | 1 | cascade — FrugalGPT-style multi-step confidence cascade (research: vault/research/ |
| queda | server | 1130 | recurrente | test_fleet,test_project_aware,test_server,test_server_smoke,test_w31_health_honesto,test_w32_server_seguro,test_w52_cobertura,test_w6_ronda1,test_w6_ronda3 | si | 0 | si | 0 | server — mmorch VISUAL nivel 3: progreso live de cada subagente + control TOTAL remoto. |
| queda | mcp_server | 1091 | recurrente | test_b2_b3,test_bandit_unificado,test_mcp_contract,test_mcp_profile,test_mcp_risk,test_mcp_schema,test_sessions_wiring,test_w21_paquete,test_w6_ronda3 | si | 0 | si | 0 | MCP wrapper — exposes mmorch patterns as tools to Claude Code. |
| queda | evolve | 893 | auto+manual+recurrente | test_automerge,test_evolve,test_evolve_branch,test_evolve_goal_guard,test_evolve_motor,test_gate_vivo,test_megasource,test_w6_ronda1,test_w6_ronda2 | si | 0 | si | 8 | evolve — subset DGM-inspirado, GATED (research: vault/research/ |
| queda | memory | 839 | auto+manual+recurrente | test_cosine_batch,test_curiosity,test_distill_backlog,test_mcp_contract,test_memory,test_nudge,test_recall_keyword,test_reconsolidation,test_retention,test_trajectory | si | 18 | - | 13 | memory — memoria episodica + semantica para mmorch (DuckDB 2 capas). |
| queda | checkers | 608 | auto+manual+recurrente | test_canary,test_checkers,test_dead_modules,test_sdlc_d2_test_compile | si | 0 | - | 10 | checkers — libreria propia de VERIFICADORES DETERMINISTAS (tool-verify). |
| queda | loop_nightly | 606 | auto+manual+recurrente | test_loop_nightly,test_w21_paquete,test_w31_health_honesto | si | 0 | - | 6 | F5 loop-cerrado: orquestador nightly del loop de ideas (spec .scratch/loop-cerrado/spec.md |
| queda | nightly | 575 | auto | test_gate_vivo,test_runtime_checkout,test_w21_paquete,test_w6_ronda2 | si | 0 | si | 0 | nightly — driver ALWAYS-ON del loop nocturno (Windows Task Scheduler, no Claude). |
| queda | project_integrate | 569 | auto+recurrente | test_project_driver,test_review_sdlc_d2,test_sdlc_d2_test_compile,test_w33_w34_robustez | si | 1824 | si | 5 | project_integrate — F3 of the /project rebuild: wire the recursive driver (F2) to REAL sea |
| queda | project_build | 526 | auto+recurrente | test_w33_w34_robustez,test_worklist_acceptance_rule | si | 175 | si | 3 | project_build — F1 of the /project rebuild: decompose a big task into a VALIDATED worklist |
| queda | auto_apply | 502 | auto | test_auto_apply | si | 0 | - | 1 | Fail-closed policy and evidence gate for autonomous code promotion. |
| queda | server_engine | 425 | recurrente | test_w52_cobertura,test_w6_ronda1 | si | 0 | - | 1 | server_engine — the in-process job execution engine: the threads that drive rubric, |
| queda | repo_mining | 421 | auto+manual | test_repo_mining | si | 0 | - | 1 | Minería de repos ajenos — aprender de cualquier repo SIN acumularlo. |
| queda | providers | 406 | auto+manual+recurrente | test_cache_cost,test_canary,test_error_rates,test_evolve,test_executor_seam,test_innov_modules,test_memory,test_patterns,test_project_driver,test_project_loop,test_providers,test_server,test_w33_w34_robustez,test_w6_ronda1,test_w6_ronda2,test_w6_ronda2b,test_w6_ronda3 | si | 0 | - | 22 | Provider layer — thin OpenAI-compatible client per external model. |
| queda | rubric_loop | 373 | recurrente | test_enrich,test_rubric_loop,test_scout,test_trajectory,test_w6_ronda1 | si | 68 | - | 3 | rubric_loop — LOOP DE AUTOCORRECCION CON VERIFICADOR INDEPENDIENTE (spec del usuario). |
| queda | babel | 358 | recurrente | - | sin medir | 234 | si | 1 | babel — capa comprimida model-native del vault (paper 2606.19857). |
| queda | workflow_store | 346 | recurrente | test_server,test_w6_ronda1 | si | 0 | si | 3 | workflow_store — durable block-context + checkpoints for cooperative workflows (Phase A). |
| queda | fuel | 323 | auto+manual+recurrente | test_fuel,test_repo_mining | si | 0 | - | 3 | Fuel module: candidate proposal lifecycle for roadmap loops. |
| queda | health | 307 | auto+manual+recurrente | test_health,test_w31_health_honesto,test_w32_server_seguro,test_w6_ronda1 | si | 6 | - | 5 | Health module for mmorch: dead-man's switch detection. |
| queda | ensemble | 292 | auto+manual+recurrente | test_b2_b3,test_ensemble_parallel,test_innov_modules | si | 1 | si | 1 | ensemble_verify (I-3) — K escepticos cross-family + voto mayoria. |
| queda | patterns | 289 | auto+manual+recurrente | test_ablation_protocol,test_b2_b3,test_checkers,test_dead_modules,test_distill_backlog,test_ensemble_parallel,test_evolve,test_goal,test_innov_modules,test_memory,test_patterns,test_server,test_w52_cobertura | si | 0 | - | 7 | Code-flow patterns (§7), migrated as deterministic Python. |
| queda | sessions | 280 | manual+recurrente | test_sessions_difficulty,test_sessions_ingest,test_sessions_outcome,test_sessions_parse,test_sessions_redact,test_w6_ronda2b | si | 0 | - | 3 | sessions — aprende de transcripts de Claude Code. Parsea el JSONL de sesion en |
| queda | regresion | 278 | auto | test_regresion | si | 0 | si | 1 | Refutacion EJECUTABLE de una branch: la objecion se prueba o no existe. |
| queda | auto_apply_nightly | 268 | auto | test_auto_apply_nightly | si | 0 | - | 1 | Nightly adapter for the isolated autonomous promotion circuit. |
| queda | intuition | 266 | auto+manual+recurrente | test_bandit_unificado,test_intuition_floor | si | 0 | si | 6 | intuition — the bandit, re-keyed by structural signature (intuition layer Phase 1). |
| queda | promotion | 263 | auto | test_auto_apply,test_promotion | si | 0 | - | 2 | Durable fail-closed state machine for autonomous code promotions. |
| queda | project_driver | 259 | auto+recurrente | test_sdlc_d3_atasco | si | 0 | si | 1 | project_driver — F2 of the /project rebuild: the RECURSIVE build orchestrator. |
| queda | self_audit | 255 | auto | test_self_audit | si | 0 | - | 1 | Auto-auditoria — el juez de mmorch se mira a si mismo, modulo por modulo. |
| queda | evolve_findings | 253 | auto+manual+recurrente | - | sin medir | 0 | si | 2 | evolve_findings — fuente automática de hallazgos para el loop nocturno de auto-evolve |
| queda | spec | 238 | recurrente | test_spec | si | 28 | si | 1 | spec — spec-builder barato que INFIERE mas alla de lo dicho, pero aplica |
| queda | vault | 238 | recurrente | test_vault_write | si | 23 | - | 2 | vault — memoria de largo plazo mmorch-legible sobre el vault Obsidian. |
| queda | project_loop | 231 | recurrente | test_executor_seam,test_project_loop | si | 7 | - | 2 | project_loop — ejecutor PROJECT-AWARE primario via mmorch (barato, cero cupo). Es la |
| queda | worktree_driver | 225 | auto+manual+recurrente | test_worktree_driver | si | 0 | si | 11 | worktree_driver — isolated execution in a throwaway git worktree (graft G3 follow-up). |
| queda | workflow_race | 224 | auto | - | sin medir | 0 | si | 2 | workflow_race — corre N VARIANTES de workflow sobre una task congelada del bench y |
| queda | config | 213 | auto+manual+recurrente | test_config_cost,test_glm_family,test_loop_nightly,test_mcp_contract | si | 0 | - | 37 | Model registry — single source of truth for models, families, endpoints, prices. |
| queda | bench | 204 | auto | - | sin medir | 11 | si | 2 | bench — benchmark CONGELADO de tasks difíciles para evolución de workflows. |
| queda | provenance | 204 | auto+manual+recurrente | test_provenance | si | 0 | si | 3 | Provenance de branches — outcomes retroactivos por verdad de ejecución. |
| queda | architecture | 202 | auto | test_architecture | si | 0 | - | 1 | Organizador de arquitectura — chequeos MECANICOS, sin juicio de LLM. |
| queda | bughunt | 201 | auto+manual | test_bughunt | si | 0 | - | 2 | BUG-HUNTER logico de mmorch: mutation-survivors como mapa de donde un bug silencioso vivir |
| queda | retention | 188 | auto+manual+recurrente | test_retention | si | 0 | si | 1 | retention — decay Ebbinghaus + Zeigarnik para la capa semantica de memory. |
| queda | trajectory | 179 | manual+recurrente | test_trajectory | si | 0 | - | 5 | trajectory — robo de Hermes: 'trajectory compression para entrenar la proxima |
| queda | pty_session | 173 | recurrente | test_w52_cobertura | si | 0 | - | 1 | pty_session — interactive PTY sessions for the Lotus terminal. |
| queda | autoresearch | 172 | auto+manual+recurrente | test_autoresearch | si | 153 | - | 2 | autoresearch (r4a) — hillclimb como JOB declarativo + resumable. |
| queda | learn | 170 | recurrente | test_learn | si | 0 | - | 1 | learn — meta-inteligencia: mmorch aprende de su propio metrics.jsonl (I-1). |
| queda | code_review | 167 | recurrente | test_mcp_contract,test_w6_ronda2b | si | 547 | si | 1 | code_review — cero-cupo senior reviewer: read code, flag where it breaks the mmorch coding |
| queda | hillclimb | 167 | auto+manual+recurrente | test_hillclimb | si | 0 | - | 2 | hillclimb — optimizacion sobre METRICA ESCALAR con feedback del entorno |
| queda | lang | 166 | auto+recurrente | - | sin medir | 0 | si | 2 | lang — capacidades deterministas POR LENGUAJE para el project-build engine. |
| queda | stuck_detector | 163 | auto+manual+recurrente | test_stuck_detector | si | 0 | si | 2 | Detector de estancamiento — tendencias sobre la historia nocturna, cero LLM. |
| queda | classify | 162 | manual+recurrente | test_classify,test_cynefin,test_mcp_contract | si | 9 | - | 2 | classify_and_act — rutear por TIPO y manejar cada rama distinto (triage, model |
| queda | plugins | 162 | recurrente | - | sin medir | 0 | si | 1 | plugins — capability-gated plugin platform (graft G11 from paperclip plugin-loader.ts). |
| queda | workflow_engine | 161 | recurrente | - | sin medir | 0 | si | 1 | workflow_engine — cooperative multi-role workflow as a pure state machine (Phase C). |
| queda | workflow_evolve | 160 | auto | - | sin medir | 0 | si | 1 | workflow_evolve — la poblacion de variantes del engine EVOLUCIONA (backlog #1 |
| queda | docs_extract | 156 | auto+manual | test_docs_extract | si | 0 | si | 1 | Extracción de texto de documentos (PDF hoy) — dos niveles, medidos en |
| queda | merge_train | 155 | auto+manual | test_merge_train | si | 0 | - | 1 | Merge train — las branches amarillas del dia se conglomeran en UN merge. |
| queda | claude_exec | 153 | recurrente | test_executor_seam,test_project_aware,test_project_loop | si | 0 | - | 2 | claude_exec — ejecutor que corre en el PLAN de Claude (cupo), no por API. Invoca el |
| queda | server_core | 152 | recurrente | test_server,test_w32_server_seguro,test_w52_cobertura,test_w6_ronda1 | si | 0 | - | 4 | server_core — shared in-process state + tiny request helpers for the server route modules. |
| queda | auto_repair | 148 | auto | test_auto_repair | si | 0 | - | 1 | Auto-reparación nocturna: los errores que el sistema DETECTA se convierten |
| queda | workflow_spec | 148 | recurrente | test_w21_paquete | si | 0 | si | 1 | workflow_spec — load + validate cooperative workflows and role personas (Phase C, Decision |
| queda | automerge | 146 | auto+manual | test_auto_apply,test_automerge | si | 0 | - | 5 | Automerge con semáforo — merges sin accionar humano SOLO en el carril verde. |
| queda | hardening | 145 | auto | test_hardening | si | 0 | - | 1 | Hardening loop: mmorch se blinda solo contra sus puntos ciegos. |
| queda | bursts | 143 | auto+manual | - | sin medir | 0 | si | 2 | Bursts de arXiv — temas recién acuñados, que ningún tag todavía nombra. |
| queda | triage | 143 | auto+manual | test_triage | si | 0 | si | 1 | Triage mecanico de branches propuestas — cero LLM, cero cupo, determinista. |
| queda | signature | 142 | auto+manual+recurrente | mut_signature | si | 0 | si | 6 | signature — project a task's TEXT onto a STRUCTURAL key (cero-cupo, deterministic). |
| queda | sandbox | 140 | auto+manual+recurrente | test_sandbox_policy | si | 0 | - | 3 | sandbox — corre codigo NO confiable aislado (la compuerta del pipeline 'git-like' |
| queda | observation | 139 | auto | test_observation | si | 0 | - | 1 | Deterministic post-merge observation policy for autonomous promotions. |
| queda | curation | 138 | manual+recurrente | test_curation | si | 0 | - | 1 | Curacion humana de propuestas — logica compartida entre scripts/veredicto.py, |
| queda | speedup | 136 | recurrente | - | sin medir | 2 | si | 1 | speedup — make a function faster, cero-cupo, kept only on MEASURED+CORRECT improvement. |
| queda | iohelpers | 134 | auto+manual+recurrente | test_error_rates,test_iohelpers,test_loop_nightly,test_outcomes,test_proposals | si | 0 | si | 22 | iohelpers — shared robustness idioms for the JSON/JSONL state files under logs/*. |
| queda | schema | 130 | auto+manual+recurrente | test_loop_nightly,test_schema,test_server | si | 9 | - | 5 | schema (§9) — structured-output gates. Hoy los parsers de mmorch son best-effort |
| queda | session_skills | 129 | manual+recurrente | test_session_skills | si | 0 | - | 1 | session_skills — mina playbooks reusables de sesiones de Claude. De segmentos con |
| queda | mcp_telemetry | 126 | recurrente | test_mcp_contract | si | 0 | si | 1 | mcp_telemetry — logger CENTRALIZADO de invocaciones MCP (audit 2026-07; hoy 46 tools, ~20 |
| queda | project_repair | 123 | auto | test_project_repair | si | 0 | - | 1 | Reparación cross-repo: mmorch arregla los proyectos del REGISTRY, no solo |
| queda | server_frontend | 119 | recurrente | test_w52_cobertura | si | 0 | - | 1 | server_frontend — the live dashboard HTML, lifted verbatim out of server.py (it is a stati |
| queda | enrich | 114 | recurrente | test_enrich | si | 0 | - | 1 | enrich — completar/especificar el prompt infiriendo intent del usuario (patron Fable 5), |
| queda | budget_policy | 113 | recurrente | test_budget_policy,test_server | si | 0 | si | 2 | budget_policy — scoped budget policies (graft G5 from paperclip). |
| queda | feedback_trace | 112 | recurrente | - | sin medir | 0 | si | 1 | feedback_trace — human vote -> trace bundle + bandit signal (graft G8 from paperclip). |
| queda | frontier | 111 | auto+manual | test_repo_mining | si | 0 | si | 1 | Frontera de temas — rompe el círculo cerrado del auto-descubrimiento. |
| queda | dataset | 110 | auto+manual | test_dataset | si | 0 | - | 1 | dataset — construye un dataset de CALIDAD DE CÓDIGO desde git history, SIN labels |
| queda | runtime_checkout | 110 | auto | test_auto_apply,test_runtime_checkout | si | 0 | - | 2 | Persistent Git worktree used as the isolated autonomous runtime. |
| queda | goal | 109 | auto+manual+recurrente | test_evolve_goal_guard,test_gate_vivo,test_goal,test_w6_ronda1 | si | 0 | - | 3 | goal — ancla anti-goal-drift, modelada sobre el `/goal` nativo de Claude Code. |
| queda | budget | 104 | auto+manual+recurrente | test_b2_b3,test_budget,test_dead_modules,test_error_rates,test_mcp_contract,test_w33_w34_robustez,test_w6_ronda2 | si | 166 | - | 6 | budget — BudgetKeeper: techo de gasto mensual (ataca el incidente +$5). |
| queda | job_graph | 103 | recurrente | - | sin medir | 0 | si | 1 | job_graph — adjacency-list ancestry over the in-memory job map (graft G1). |
| queda | server_pty | 101 | recurrente | - | sin medir | 0 | - | 1 | server_pty — interactive PTY (terminal) routes: open/stream/input/resize/close a shell |
| queda | route | 96 | manual+recurrente | test_innov_modules,test_mcp_contract | si | 47 | - | 2 | route (I-2) — confidence-gated escalation. Ahorra cupo: el modelo barato |
| queda | scout | 96 | recurrente | test_scout | si | 0 | - | 1 | scout — pre-pass ENTORNO-PRIMERO (el patron central de Fable 5: 'primero aprende el |
| queda | canal | 94 | recurrente | test_canal,test_dead_modules | si | 10 | si | 1 | canal — hilo ordenado entre Cursor, Claude Code y mmorch. |
| queda | outcomes | 94 | manual+recurrente | test_loop_nightly,test_outcomes | si | 0 | - | 1 | Outcome recording and expiry for proposals. |
| queda | arbitration | 93 | auto | - | sin medir | 0 | si | 1 | arbitration — registro auditable de los arbitrajes del orquestador (blind-spot #2, 2026-07 |
| queda | sync | 92 | recurrente | test_executor_seam,test_project_loop,test_sync,test_w52_cobertura | si | 0 | si | 3 | sync — GitHub como bus de sincronizacion entre maquinas. El host always-on (ej pc-mateo) |
| queda | fleet | 91 | recurrente | test_fleet,test_server,test_w52_cobertura | si | 0 | - | 2 | fleet — control unificado de varios hosts mmorch en el tailnet. Cada maquina corre su |
| queda | nodes | 91 | recurrente | test_nodes | si | 0 | - | 2 | nodes — el registry de la ORQUESTA: nombra a cada miembro que mmorch (el DIRECTOR) |
| queda | slim | 91 | auto | test_slim | si | 0 | - | 1 | Slim — auto-eficientización de código: menos verbose, misma conducta. |
| queda | events | 90 | auto+manual+recurrente | test_project_aware,test_server | si | 0 | - | 9 | events — bus de progreso in-process pa la UI live (nivel 3). El orquestador emite |
| queda | portability | 88 | recurrente | - | sin medir | 0 | si | 1 | portability — export/import mmorch state across devices (grafts G2 + G4). |
| queda | minds | 84 | recurrente | - | sin medir | 0 | si | 1 | minds — global federation graph across registered projects (read-only). |
| queda | chat_store | 81 | recurrente | test_server | si | 0 | si | 1 | chat_store — durable chat history for Lotus (SQLite, stdlib). |
| queda | durable_runs | 81 | recurrente | - | sin medir | 0 | si | 1 | durable_runs — heartbeat + zombie reaper for in-process jobs (graft G9 from paperclip). |
| queda | projects | 81 | auto+manual+recurrente | test_executor_seam,test_loop_nightly,test_project_aware,test_project_loop,test_project_repair,test_server,test_sync | si | 0 | - | 10 | projects — registro de proyectos que mmorch puede CONTROLAR (project-aware). Hace que |
| queda | prompts | 81 | recurrente | test_cost_savers | si | 0 | - | 2 | prompts — construccion de mensajes PREFIX-STABLE pa maximizar el cache-hit de DeepSeek. |
| queda | prices | 80 | auto+manual+recurrente | test_cache_cost,test_megasource,test_w33_w34_robustez | si | 0 | - | 2 | prices — capa de OVERRIDE de precios (datos volátiles, separados del código). |
| queda | curiosity | 78 | recurrente | test_curiosity | si | 0 | - | 1 | curiosity — deteccion de TENSION en la memoria (modulo cognitivo #3). |
| queda | nudge | 73 | auto+manual+recurrente | test_nudge | si | 0 | - | 3 | nudge — robo de Hermes 'periodic memory nudging': cada N loops cerrados, dispara |
| queda | gate_policy | 65 | recurrente | - | sin medir | 0 | si | 1 | gate_policy — staged review/approval gates per job (graft G6 from paperclip). |
| queda | loop | 61 | auto | test_loop | si | 4175 | - | 0 | loop_until_done — scope DESCONOCIDO, 'segui hasta que este limpio'. Control-flow |
| queda | server_fleet | 52 | recurrente | - | sin medir | 0 | - | 1 | server_fleet — multi-host (tailnet) routes: register/list fleet hosts, proxy a job to a |
| queda | exec_policy | 46 | recurrente | test_w52_cobertura | si | 0 | si | 2 | exec_policy — where execution is allowed to run (graft G3 from paperclip). |
| queda | transcript_store | 27 | recurrente | test_w52_cobertura | si | 0 | - | 2 | transcript_store — per-job inter-agent transcript (in-memory). |
| queda | textutil | 26 | auto+manual+recurrente | - | sin medir | 0 | si | 10 | textutil — shared text helpers. Dedups the code-fence extractor that was copy-pasted |
| queda | cost | 21 | auto+manual+recurrente | test_cache_cost,test_config_cost,test_megasource | si | 0 | - | 2 | Cost model — USD from token counts, using REGISTRY prices. |

resumen: [('queda', 108), ('borrar o cablear', 17), ('manual sin uso: medir o borrar', 6), ('arreglar o borrar', 3), ('manual con uso: queda', 2)]
