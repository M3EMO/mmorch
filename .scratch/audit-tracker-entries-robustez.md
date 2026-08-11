# Tracker entries — EJE=robustez (2026-08-10)

Para que el orquestador mergee al tracker local (`.scratch/<feature-slug>/issues/NN-<slug>.md`).
Feature-slug sugerido: `audit-robustez`. Rankeados: severidad primero, luego menor esfuerzo.
Informe completo: `.scratch/audit-robustez-2026-08-10.md`.

---

## 01 — budget_policy.load() falla abierto y en silencio: JSON corrupto desactiva los límites de gasto

Type: task
Status: open
Severity: IMPORTANTE
Evidence: mmorch/budget_policy.py:22-26,30 · mmorch/server_core.py:27-36

### Body

`load()` traga cualquier excepción devolviendo `[]` sin log — no distingue "no hay
políticas" de "las políticas configuradas no se pudieron leer". `blocking_incident()`
(cableado al 402 de creación de jobs en `server_core._budget_block`) con `[]` no bloquea
nada: un `budget_policies.json` truncado (que el propio `save()` no-atómico de la línea
30 puede producir en un crash mid-write) anula todos los hard-stops de gasto sin señal.

**Fix:** distinguir no-existe (`[]` legítimo) de no-parsea (log fuerte y/o incidente
hard conservador) + `save()` atómico (tmp + `os.replace`).

---

## 02 — Readers .jsonl sin tolerancia por línea: una línea corrupta tumba read_events y apaga el health floor en silencio

Type: task
Status: open
Severity: IMPORTANTE
Evidence: mmorch/metrics.py:63-72 · mmorch/intuition.py:99-125 · mmorch/feedback.py:77 · mmorch/evolve.py:257 · mmorch/trajectory.py:128

### Body

`metrics.read_events()` parsea línea por línea sin try/continue; metrics.jsonl lo
appendean varios procesos. Una línea torn hace lanzar el reader → `intuition.healthy()`
(fail-open por diseño, `except Exception: return models`) deja de filtrar modelos
enfermos sin ninguna señal — el guardrail que existe porque glm-4.6 midió 34% de error.
Mismo patrón en `feedback.read_outcomes` (rompe ECE/calibración), evolve.py:257 y
trajectory.py:128. El patrón correcto ya está en el repo (arbitration.py:44-47,
mcp_telemetry.py:74-75).

**Fix:** try/continue por línea en los 4 readers (copiar arbitration.py:46). Esfuerzo
mínimo.

---

## 03 — Lock por archivo del loop nocturno de PRs se pierde ante corrupción de evolve_open_prs.json

Type: task
Status: open
Severity: IMPORTANTE
Evidence: mmorch/evolve.py:440-448,453-458,462-464,496-511,534-539

### Body

`_load_pr_state()` → JSON corrupto → `{}` sin log: desaparecen todos los locks por
archivo y `coordinated_evolve_round()` puede abrir un branch competidor sobre un archivo
con PR abierto — exactamente la carrera que el lock existe para prevenir (comentario de
diseño líneas 440-448). `_save_pr_state()` es write_text no atómico y el loop corre
desatendido de noche: el crash nocturno produce el archivo truncado que el load traga.
Colateral: se pierden los outcomes post-merge (496-511), la señal de aprendizaje "más
valiosa que el gate".

**Fix:** write atómico + log fuerte al detectar corrupción en load.

---

## 04 — Watermark distill_upto (nudge.json): write no atómico + dos escritores sin lock → re-destilado (API repetida, notas duplicadas)

Type: task
Status: open
Severity: IMPORTANTE
Evidence: mmorch/nudge.py:20-26,50,57-58 · scripts/nightly.py:97-120

### Body

nudge.json corrupto → `_load` cae al default sin `distill_upto` → el próximo destilado
arranca en `after_id=0`. Además dos procesos (nudge.tick diurno en el MCP server;
nightly.py 02:10) hacen load→modify→write COMPLETO del mismo archivo sin lock:
last-writer-wins puede retroceder el watermark. Resultado: `distill_backlog` re-procesa
episodios ya destilados — hasta 50/noche de llamadas gen+verify repetidas + notas
semánticas duplicadas en memory.duckdb.

**Fix:** write atómico + preservar el watermark ante corrupción; mejor: mover
`distill_upto` a memory.duckdb (transaccionalidad ya disponible).

---

## 05 — Estado de bandits (bandit_state.json / sig-bandit): reset silencioso, write no atómico, carrera inter-proceso

Type: task
Status: open
Severity: IMPORTANTE
Evidence: mmorch/feedback.py:88-92,119-120 · mmorch/feedback.py:51-56→intuition.record · scripts/nightly.py (proceso 2)

### Body

`ThompsonBandit.__init__` resetea a `{}` sin log ante archivo corrupto (todo lo
aprendido se pierde); `update()` reescribe el JSON completo sin atomicidad; y el MCP
server + nightly.py escriben los mismos estados vía `record_outcome`/`intuition.record`
sin lock inter-proceso (updates concurrentes se pierden). Agravante: el bandit ya está
starved (n≤3/brazo tras 10k calls, audit 2026-07).

**Fix:** write atómico + log en corrupción; file-lock o migrar el estado a SQLite
(patrón workflow_store.py).

---

## 06 — vault.write_note sobreescribe notas en silencio ante colisión de slug (fuente de verdad)

Type: task
Status: open
Severity: IMPORTANTE
Evidence: mmorch/vault.py:16-18,31-33,81-114 · mcp_server.py:366

### Body

`_slug` normaliza y trunca a 60 chars (colisiones deterministas); `write_note` escribe
sin chequear existencia; `write_validated` (la "single validated door" del vault) delega
sin protección y `log_op` registra "write" indistinguible de un overwrite. Dos títulos
que colisionan, o re-usar un título, pierde el contenido anterior sin backup ni warning.
Git mitiga solo si se commitea seguido; el write no commitea.

**Fix:** si el path existe con contenido distinto → sufijo `-2`/`-3` o error explícito +
`log_op("overwrite", ...)`.

---

## 07 — Memo cache: reset silencioso + rewrite completo por put, lock solo-thread

Type: task
Status: open
Severity: NICE-TO-HAVE
Evidence: mmorch/cache.py:13,25-29,38

### Body

logs/memo.json corrupto → `{}` silencioso (se re-paga API por verifies ya cacheados);
cada `put` reescribe el archivo entero sin atomicidad; `_LOCK` es threading (no cubre
multi-proceso). Impacto acotado: cache regenerable.

**Fix:** write atómico; SQLite si crece.

---

## 08 — Checkpoints de resumabilidad best-effort sin señal al fallar

Type: task
Status: open
Severity: NICE-TO-HAVE
Evidence: mmorch/server_engine.py:43-48,53-56,155,343-344,384-385

### Body

Si workflow_store falla, el job sigue (deliberado) pero el checkpoint/estado resumible
no se persiste y nadie se entera: un resume posterior re-arranca de step 0 re-pagando
pasos. `emit()` ya existe en el archivo.

**Fix:** `emit("job","warn",...)` en los except de checkpoint/spec. Esfuerzo mínimo.
