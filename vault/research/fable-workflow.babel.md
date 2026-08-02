---
source: fable-workflow.md
lexicon: v1
ratio: 0.51
fidelity: 0.833
derived: true
---
Fable: reglas mmorch. Vuyyuru skill-distill: caro doc, barato exec. Fable 5 "arquitecto saliente" post F1-F4 (project-build) & mmorch quant-analysis (2026-07). Regla: cicatriz (commit/medida repo). Sin cicatriz, no entra.

1.  **Verdad=Ejecución, no opinión.** Cicatriz: 22 críticas F1/F2 (68-74% falsas). Alucinación "`except Exception` atrapa SystemExit" (3x). Regla: Gate=ejecución (pytest, etc.), no LLM veredicto. LLM crítica=HIPÓTESIS, válida si produce test fallido. NO: juicio subjetivo (estilo, diseño) → advisory.
2.  **Iterar "sin críticas válidas", no "sin críticas".** Cicatriz: refuter refuta por default (F3, 4 rondas). Perseguir cero-críticas=no-convergencia. Regla: Orquestador arbitra refutaciones (VÁLIDA→absorber, PARCIAL→extraer, INVÁLIDA→desmentir). Convergencia=ronda sin válidas nuevas. NO: delegar arbitraje a modelo barato.
3.  **Medir antes de proponer.** Cicatriz: "mmorch autoevoluciona" tesis → bandit n≤3/arm (10.674 calls, 176 eps) → 0 notas semánticas, ECE 0.456, glm-4.6 34% error. Regla: Propuesta cita número (mmorch_metrics, etc.). Si no existe, proponer instrumentar. NO: micro-decisiones reversibles → parálisis.
4.  **Scar-to-rule: falla viva→fix engine, mismo día.** Cicatriz: F4 (3 rondas) → 7 bugs engine (fixes: `b65894c`, `b09cb39`, `5a0454d`). Regla: Falla producción/live-run→triage (engine/driver/task)→fix ANTES siguiente run. NO: falla TASK (GIGO)→arreglar task, no sobre-endurecer engine.
5.  **Anti-Goodhart: acceptance=señal POSITIVA.** Cicatriz: `REGRESAN=0` dio verde con lado nuevo MUERTO (fail-open). Fix: exigir `mejoran=1` (Caso_649). Regla: Gate afirma que sistema HACE lo nuevo. Chequeo: "¿pasa si feature no existe?"→rediseñar.
6.  **Hermeticidad: tests no tocan estado real.** Cicatriz: test rojo (424/425) lee sig-bandit REAL (`93a6cb9`); record_outcome→gatear `_real_run`. Regla: Boundaries inyectados=corrida sintética=cero aprendizaje persistido. Tests usan tmp_path/fakes.
7.  **Seams inyectables + self-check `__main__` = módulo se prueba solo.** Cicatriz: F2 fakes→bug "container recursado" (antes API); graft pattern (módulo puro→wire→self-check→1 commit). Regla: Boundary→parámetro (default prod); `__main__` ejercita con fakes. Si no se self-checkea sin API→interfaz mal cortada.
8.  **Determinista sobre LLM para estructural.** Cicatriz: flat build-feature→stub (130 chars) por LLM veredicto. Rebuild→AST/DAG/topo-sort/health-floor (código puro). Regla: LLM PROPONE; código DECIDE. Si check puede ser determinista, es determinista.
9.  **Contexto completo o contrato roto.** Cicatriz: cap 6KB→regenera archivo TRUNCADO (pérdida); LAZY_SYSTEM→módulo 200 líneas→fragmento 20. Regla: Contrato "devolvé completo"→modelo VE completo; prompt no contradice (minimalidad=CAMBIO). Prompts rol→revisar como código.
10. **Verificación independiente antes de victoria.** Cicatriz: `built+integrated` F4 ronda 3→re-correr shadow-diff COMPLETO fuera engine (f4_verify). Ronda 2→"verde" por entorno roto. Regla: Claim final re-mide por camino NO compartido.
11. **Entorno es parte de la verdad.** Cicatriz: worktree fresco→3 tests rojos + shadow (pipeline mutilado PASÓ gate); repo movido mid-run (base stale). Regla: Gatear→asegurar entorno gate == entorno real (seed_globs; guard base-coherencia; baseline SIN -x).
12. **Un cambio=un commit, gates automáticos.** Cicatriz: F1→F4 + análisis ≈ 15 commits atómicos (ruff+mypy, suite verde); git-bisect viable. Regla: Nada entra sin gates lenguaje; commit msg→POR QUÉ + cicatriz.
13. **Parche vs arquitectura: fix por-caso→registry.** Cicatriz: stub_check Python-only→"if not .py: check trivial" (parche). Usuario→"parche=arreglo temporal"→`lang.py` registry (`f35ec74`). Regla: Segundo caso→seam/registry. NO construir ANTES del segundo caso (YAGNI).
14. **Presupuesto juicio: caro piensa, barato ejecuta.** Cicatriz: Arco costó $3.34 API externa (10.674 calls)→generación, verificación, review (DeepSeek/Gemini/GLM); caro→planificó, arbitró, decidió arquitectura. Regla (tesis mmorch): Rutear por valor juicio→orquestador (síntesis, tie-break, triage); externos baratos (bulk gen/verify/review). Doc=misma jugada: Fable juicio destilado→ejecutado barato.

Operacionalización mmorch:
Ya encarnado: 1→checkers/gates; 2→adversarial_verify + Opus arbitra; 8→F1/lang.py/healthy(); 7→seams; 12→hooks pre-commit/pre-push.
Faltante: 5 (LINT acceptance); 10 (post-built server job re-run); 3 (reporte periódico cron); doc→skill cargable (role-chain).
