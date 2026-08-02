# Cómo trabaja Fable — reglas destiladas para mmorch

> Skill-distillation (metodo del articulo de Vuyyuru): el modelo caro documenta UNA VEZ su
> forma de trabajar, anclada a fallas REALES observadas (scar-to-rule, nunca consejo generico),
> y los modelos baratos / los loops de mmorch la ejecutan despues. Este doc lo escribio Fable 5
> como "arquitecto saliente" tras construir el project-build engine (F1-F4) y el analisis
> cuantizado de mmorch (2026-07). Cada regla cita su cicatriz: commit o evento medido de ESTE
> repo. Si una regla no tiene cicatriz, no entra.

## 1. La verdad es la ejecución, jamás la opinión de un modelo

**Cicatriz:** 22 críticas de reviewers cross-family sobre F1/F2 → ~68-74% falsas al arbitrarlas
por ejecución. La alucinación "`except Exception` atrapa SystemExit" apareció 3 veces; un probe
de 4 líneas la desmintió. **Regla:** un gate jamás se decide por veredicto LLM; se decide
corriendo código (pytest, node --test, checkers, self-checks). La crítica de un LLM es una
HIPÓTESIS: válida solo si produce un test que falla. **Cuándo NO:** juicio genuinamente
subjetivo (estilo, diseño) → ahí sí cross-family refutado, pero como ADVISORY, nunca gate.

## 2. Iterar "hasta que no haya críticas válidas", nunca "hasta que no haya críticas"

**Cicatriz:** el refuter refuta por default — 4 rondas sobre F3 terminaron reciclando 2 críticas
ya desmentidas (shell=True re-asertado 4x; "F2 llama commit_fn(None)" — falso por lectura del
código). Perseguir cero-críticas = no-convergencia garantizada. **Regla:** el orquestador (no el
verifier) arbitra cada refutación con evidencia: VÁLIDA → absorber y arreglar; PARCIAL → extraer
el núcleo; INVÁLIDA → desmentir con ejecución o lectura y descartar CON razón escrita.
Convergencia = una ronda sin válidas nuevas. **Cuándo NO:** nunca delegar este arbitraje a un
modelo barato — es el rol del orquestador (invariante mmorch).

## 3. Medir antes de proponer (análisis cuantizado)

**Cicatriz:** "mmorch autoevoluciona" era la tesis; los números dijeron: bandit n≤3/arm tras
10.674 calls, 176 episodios → 0 notas semánticas, ECE 0.456, glm-4.6 34% error DENTRO del pool.
Ninguna de esas 4 fallas era visible sin medir. **Regla:** toda propuesta cita su número
(mmorch_metrics/error_rates/feedback_stats/memory_stats o grep/wc del código). Si el dato no
existe, la primera propuesta es instrumentar. **Cuándo NO:** micro-decisiones reversibles —
medir todo también es parálisis.

## 4. Scar-to-rule: cada falla de un run vivo se convierte en fix del ENGINE, mismo día

**Cicatriz:** F4 = 3 rondas vivas → 7 bugs reales del engine, cada uno commiteado como fix
(`b65894c` planner sin file / cap 6KB / dup-file; `b09cb39` código unverified jamás escrito a
disco; `5a0454d` LAZY_SYSTEM vs contrato full-file). El run que "falla" es el que más enseña.
**Regla:** una falla en producción/live-run se triagea igual que una refutación (¿del engine,
del driver, de la task?) y el fix aterriza ANTES del siguiente run — el error de la ronda N no
puede reaparecer en la N+1. **Cuándo NO:** si la falla es de la TASK (GIGO), no del engine —
arreglar la task, no sobre-endurecer el engine.

## 5. Anti-Goodhart: el acceptance pinea la señal POSITIVA, no solo la ausencia de fallo

**Cicatriz:** `REGRESAN=0` dio verde con el lado nuevo MUERTO (fail-open → old==old → 0 diff).
El fix: exigir además `mejoran=1` (Caso_649 vivo). **Regla:** todo gate afirma que el sistema
HACE lo nuevo, no solo que no rompe lo viejo — un acceptance que puede pasar con el código
apagado no es acceptance. Chequeo rápido: "¿esto pasa si mi feature no existe?" → rediseñar.

## 6. Hermeticidad: los tests jamás tocan (ni envenenan) estado persistente real

**Cicatriz:** el único test rojo de la suite (424/425) era code_loop leyendo el sig-bandit REAL
de ~/.mmorch (`93a6cb9`); y al conectar record_outcome en project-build hubo que gatear con
`_real_run` para que los self-checks con fakes no envenenen el bandit. **Regla:** boundaries
inyectados = corrida sintética = cero aprendizaje persistido; tests usan tmp_path/fakes SIEMPRE.
La simetría importa: el estado real no entra a los tests, y los tests no escriben al estado real.

## 7. Seams inyectables + self-check `__main__` = el módulo se prueba solo, cero API

**Cicatriz:** F2 con fakes cazó el bug "container recursado commiteado" antes de tocar la API;
el patrón graft (módulo puro → wire → self-check → 1 commit) sostuvo todo el rebuild. **Regla:**
todo boundary (modelo, disco, subprocess, git) entra por parámetro con default de producción;
el `__main__` ejercita los caminos interesantes con fakes. Si no se puede self-checkear sin API,
la interfaz está mal cortada.

## 8. Determinista sobre LLM para todo lo estructural

**Cicatriz:** el flat build-feature dejó pasar un stub de 130 chars porque el gate era un
veredicto LLM. El rebuild puso AST/DAG/topo-sort/health-floor como código puro (F1, lang.py,
healthy()). **Regla:** el LLM PROPONE (plan, código, crítica); el código DECIDE (valida, gatea,
rutea por umbral medido). Si un check puede escribirse determinista, se escribe determinista.

## 9. Contexto completo o el contrato se rompe silencioso

**Cicatriz:** cap de 6KB sobre un archivo de 25KB → el coder regeneraba el archivo TRUNCADO
(pérdida silenciosa); LAZY_SYSTEM ("código mínimo") hizo que un módulo de 200 líneas volviera
como fragmento de 20. **Regla:** si el contrato es "devolvé el archivo completo", el modelo VE
el archivo completo y el system prompt no contradice el contrato (minimalidad aplica al CAMBIO,
no al output). Los prompts de rol se revisan como código: dos instrucciones en tensión = bug.

## 10. Verificación independiente antes de declarar victoria

**Cicatriz:** el `built+integrated` de F4 ronda 3 se declaró recién tras re-correr el shadow-diff
COMPLETO fuera del engine (f4_verify: worktree limpio, sin el engine en el medio) → REGRESAN=0
mejoran=1 confirmado. La ronda 2 había dado "verde" por entorno roto — la duplicación detecta eso.
**Regla:** el claim final se re-mide por un camino que NO comparte la maquinaria que lo produjo.

## 11. El entorno es parte de la verdad

**Cicatriz:** worktree fresco sin los artefactos gitignored (caches 1.6GB, vuelco.db) → 3 tests
rojos + shadow midiendo un pipeline mutilado que PASÓ el gate (ronda 2); y el repo se movió
mid-run porque otra sesión commiteó (base stale → GIGO). **Regla:** antes de gatear, asegurar
que el entorno del gate == entorno real (seed_globs; guard de base-coherencia; baseline SIN -x
para conocer el rojo pre-existente y deseleccionarlo — el gate mide TU cambio, no deuda ajena).

## 12. Un cambio = un commit, gates automáticos siempre

**Cicatriz:** todo el arco F1→F4 + análisis = ~15 commits atómicos con ruff+mypy en pre-commit y
suite propia verde; git-bisect viable porque cada unit/fix es un commit. **Regla:** nada entra
sin pasar los gates de su lenguaje; el mensaje de commit lleva el POR QUÉ y la cicatriz que lo
motivó (los commits de este repo son el registro scar-to-rule).

## 13. Parche vs arquitectura: si el fix es por-caso, construir el registry

**Cicatriz:** stub_check era Python-only; el primer instinto fue "if not .py: check trivial"
(parche). El usuario lo marcó: "un parche es un arreglo temporal" → `lang.py` registry por
extensión (`f35ec74`): agregar lenguaje = agregar clase, sin tocar el engine. **Regla:** segundo
caso de la misma familia de fix = momento de la seam/registry. Pero al revés también: no
construir el registry ANTES del segundo caso (YAGNI — el mock/test cuenta como segundo
implementador, la especulación no).

## 14. Presupuesto de juicio: lo caro piensa, lo barato ejecuta

**Cicatriz:** todo el arco costó $3.34 de API externa en 10.674 calls porque generación,
verificación cruzada y review corrieron en DeepSeek/Gemini/GLM; el modelo caro solo planificó,
arbitró refutaciones y decidió arquitectura. **Regla (la tesis mmorch):** rutear por valor del
juicio — síntesis crítica, tie-break y triage = orquestador; bulk gen/verify/review = externos
baratos. Este doc es la misma jugada un nivel arriba: el juicio de Fable, destilado una vez,
ejecutado barato después.

---

## Cómo operacionalizar esto en mmorch (el mapa)

Ya encarnado: 1→checkers/gates de ejecución · 2→adversarial_verify refuta-por-default + Opus
arbitra · 8→F1/lang.py/healthy() · 7→seams en todo el engine · 12→hooks pre-commit/pre-push.

Faltante (candidatos a próximo graft): 5 como LINT de acceptance (¿pasa con el feature
apagado?); 10 como paso opcional post-built del server job (re-run independiente); 3 como
reporte periódico automático (quantized-check cron); este doc como skill cargable por el
role-chain (el "senior review lens" ya existe en code_review.py — mismo patrón).
