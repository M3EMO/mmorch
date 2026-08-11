# Auditoría EJE=eficiencia — mmorch — 2026-08-10

Read-only. Gates estáticos: **ruff 0, mypy 0** (sin regresiones; `.venv/Scripts/python -m ruff check` / `-m mypy mmorch`).
Protocolo: gates → greps dirigidos → lectura dirigida → verificación adversarial cross-family
(`mmorch_adversarial_verify`, gemini-3.1-flash-lite, 2 rondas) → tie-break Opus documentado por hallazgo contestado.
2 rondas secas de búsqueda extra sin hallazgos nuevos (timeouts/polling en claude_exec/worktree_driver/sync/project_loop;
paralelismo en bucketrank/tournament/cascade/code_review — limpios o secuenciales por diseño).

Conteo: **0 BLOCKER · 3 IMPORTANTE · 6 NICE-TO-HAVE**.

---

## IMPORTANTE

### E-1 · Hot-path re-parsea logs JSONL completos que crecen sin techo
**Evidencia:**
- `mmorch/providers.py:132` → `budget.check()` antes de CADA call API; con `MMORCH_MAX_MONTHLY_USD`
  seteado, `mmorch/budget.py:40` → `metrics.read_events()` (`mmorch/metrics.py:63-72`) parsea
  **todo** `logs/metrics.jsonl` (hoy 13.581 líneas / ~4 MB; +1 línea por call, append-only, nunca rota).
  En `fan_out` (max_workers=8) son hasta 8 parses completos concurrentes (GIL-bound).
- `mmorch/route.py:67` → `intuition.decide` → `healthy()` → `error_rates(window_n=200)`
  (`mmorch/intuition.py:101-102`, `mmorch/metrics.py:85-90`): parsea el archivo completo para quedarse
  con las últimas 200 líneas.
- `mmorch/route.py:87-89` (`calibrated=True` default) → `calibrate_conf` → `read_outcomes`
  (`mmorch/feedback.py:77,130-161`): parsea todo `logs/feedback.jsonl` (1.878 líneas) por call.
- Mismo costo en cada `/state` del server (ver E-6) y en `mmorch_budget_status`/`mmorch_error_rates`.

**Impacto:** costo O(historia-completa) por llamada en el camino más caliente del sistema, monótonamente
creciente (a ~100k líneas: ~0,5-1 s por call × 2-3 sitios × workers). Hoy decenas de ms; el problema es la
pendiente, no el valor actual.
**Fix propuesto (descripción):** cache módulo-level keyed por `(path, mtime, size)` en `read_events`/
`read_outcomes` + tail-read (leer últimas N líneas desde el final) para los consumidores ventaneados;
para `monthly_spend`, un acumulador mensual persistido (archivo chico `{month, total}`) actualizado en
`log_event`, con recomputo full solo al cambiar de mes.
**Verificación:** ronda 1 refutó por "despreciable hoy" (fuera de rubric: severidad); ronda 2 con el
claim de crecimiento **pasó sin refutación**.

### E-2 · `ensemble_verify` corre los K verificadores en serie
**Evidencia:** `mmorch/ensemble.py:56-58` — list comprehension de `adversarial_verify` secuencial;
cada verificador es una call API de 5-30 s (latencias observadas, `providers.py:119`). Son llamadas
independientes; el mismo repo ya paraleliza 8 calls con `ThreadPoolExecutor` en `mmorch/patterns.py:67`.
**Impacto:** latencia K× evitable en el patrón de verificación reforzada (K=2 default → ~2× esperado).
**Fix propuesto:** `ThreadPoolExecutor` (patrón idéntico a `fan_out`), preservando el orden de
`verdicts` por índice para `judge_notes`.
**Verificación:** ronda 1 refutó por riesgo de rate-limit; ronda 2 (fan_out ya tolera 8 paralelas a los
mismos proveedores, medible en `error_rates`) **pasó sin refutación**.

### E-3 · Defaults de verificador apuntan al modelo legacy más caro y fragmentan el memo-cache
**Evidencia:**
- `mmorch/cache.py:44` — `memoized_verify(..., verifier_model="gemini-2.5-flash")` (legacy,
  out $2,50/M — `config.py:84-85`) mientras `DEFAULT_VERIFIER="gemini-3.1-flash-lite"` (out $1,50/M;
  el propio comentario `config.py:161` dice "-40% out vs 2.5-flash").
- `mmorch/ensemble.py:51` — `ensemble_verify` default `["gemini-2.5-flash","gemini-2.5-flash-lite"]`;
  `pair_verify` (`ensemble.py:195`) ya migró a 3.1-flash-lite → inconsistencia interna.
- La key del memo incluye `verifier_model` (`cache.py:51`): el mismo artefacto verificado por la vía
  memoizada vs la directa nunca comparte cache → re-verificaciones pagas evitables.
**Impacto:** $ API evitable por verify (+~67% out-price) + hit-rate del memo degradado. Esfuerzo mínimo.
**Fix propuesto:** unificar ambos defaults a `DEFAULT_VERIFIER` (import desde config); en ensemble
mantener diversidad con `[DEFAULT_VERIFIER, "gemini-2.5-flash-lite"]`.
**Verificación:** **pasó sin refutación** (ronda 1).

---

## NICE-TO-HAVE

### E-4 · `memory._connect` re-ejecuta DDL + scan de migración en cada operación
**Evidencia:** `mmorch/memory.py:83-127` — cada op (write/recall/touch/reinforce/…) abre conexión
DuckDB nueva y corre 2 `CREATE TABLE IF NOT EXISTS` + 2 `CREATE SEQUENCE` + query a
`information_schema.columns` + loop de 6 columnas. `recall_hybrid` abre 3 conexiones por llamada
(`memory.py:327, 395, 456→194`). El proceso MCP es residente.
**Fix propuesto:** flag módulo-level "schema ya migrado" (DDL/migración una sola vez por proceso),
manteniendo connect-por-op (la conexión DuckDB no es thread-safe para compartir sin lock —
refinamiento aportado por el verificador, absorbido).

### E-5 · `Memo.put` reescribe el JSON completo por inserción; `Memo()` fresco por miss
**Evidencia:** `mmorch/cache.py:34-38` — cada `put` reserializa y reescribe `logs/memo.json`
(~100 KB, sin poda, crece); `cache.py:50` — `memoized_verify(memo=None)` construye un `Memo` nuevo →
relee el archivo completo por llamada.
**Fix propuesto:** singleton módulo-level + backend dict-persistente (sqlite/shelve) o JSONL cargado
a dict una vez con append O(1) por put (refinamiento del verificador absorbido: el lookup debe seguir
siendo O(1) en memoria).

### E-6 · Handlers async del server hacen el trabajo sync pesado inline
**Evidencia:** `mmorch/server.py:42-61` (`state_snapshot`: `summary()`+`error_rates()`+`cache_stats()`
+`bstatus()` = 4 parses completos de metrics.jsonl inline) y `server.py:290-327` (`benchmarks_handler`,
2 más); el event loop queda tomado durante el parse → SSE `/events` y requests concurrentes esperan.
El mismo archivo ya usa `run_in_threadpool` en `:276` y `:337`.
**Nota honesta:** el verificador objetó que threadpool no elimina la contención de GIL (cierto: la
mitiga, no la borra). El fix de fondo es E-1 (con cache mtime estos handlers quedan baratos);
`run_in_threadpool` es complemento de consistencia.
**Fix propuesto:** aplicar E-1 y, además, envolver estos dos handlers en `run_in_threadpool` como ya
hacen `/chat` y `/minds`.

### E-7 · Hook de Stop: spawn de Python + doble parse del transcript completo en cada turno
**Evidencia:** `C:\Users\map12\.claude\hooks\context-block-watch.js` ejecuta
`execFileSync(PY, ["-m","mmorch.context_blocks","tick",...])` en **cada Stop** → startup de intérprete
del venv (~200-500 ms en Windows) + `estimate_tokens` lee y json-parsea el transcript JSONL completo
(`mmorch/context_blocks.py:40-56`; transcripts largos = decenas de MB) y, sobre el umbral, `compose`
lo re-parsea entero una segunda vez (`context_blocks.py:69-80, 93-96`). Además la tabla
`context_blocks` (sqlite 2,6 MB) no tiene índice y las queries filtran `WHERE session_id ORDER BY ts`
(`context_blocks.py:35-36, 137, 158`).
**Fix propuesto:** pre-filtro barato en `tick` por `os.stat(tp).st_size` (bytes/4 es cota superior del
estimado de tokens → si `size/4 < threshold`, salir sin parsear — válido porque `estimate_tokens` ya es
una heurística de chars); compartir una sola pasada de parse entre estimate y compose; índice
`(session_id, ts)`. (El verificador objetó el pre-filtro por "size no correlaciona con registros";
no aplica: la métrica gateada ES chars/4, y size acota chars.)

### E-8 · Rerank de recall: conversión numpy por par en loop Python
**Evidencia:** `mmorch/memory.py:341-358` — por cada nota, `_cosine(qvec, list(emb))` convierte ambas
listas a `np.asarray` (`memory.py:71-77`): 2N conversiones + N dots chicos. Una matriz (N×384) @ qvec
haría el mismo trabajo en una operación. El docstring ya reconoce brute-force OK a 10k; el costo
dominante es la conversión por par, no el dot.
**Fix propuesto:** apilar embeddings en una matriz float32 una vez y rankear con un solo matmul
normalizado.

### E-9 · `regenerate_moc` re-escanea y relee todo el vault en cada write
**Evidencia:** `mmorch/vault.py:99` (`write_validated` → `regenerate_moc`) y `vault.py:117-150`:
itera todas las carpetas y hace `read_text` + parse de frontmatter de **todos** los `.md` por cada
nota escrita. O(vault) por write; el vault está diseñado para crecer como memoria de largo plazo.
**Fix propuesto:** leer solo el frontmatter (primeras líneas hasta el segundo `---`) en vez del archivo
entero, y/o mantener el MOC incrementalmente (append/replace de la entrada del título escrito).

---

## Apéndice: Descartados / refutaciones desestimadas

Ningún hallazgo fue descartado por completo: ninguna refutación admisible bajo el rubric (error técnico
verificable) sobrevivió el tie-break. Refutaciones registradas y su disposición:

- **C1a/b/c (ronda 1)** — "parsear 4MB es sub-10ms/despreciable hoy": desestimada (severidad, excluida
  por rubric; además el claim es crecimiento sin techo). Ronda 2: pasó.
- **C2 (ronda 1)** — "paralelizar 2 calls satura rate-limits": desestimada; el repo ya dispara 8
  paralelas a los mismos proveedores sin 429 registrados. Ronda 2: pasó.
- **C3 (ronda 1)** — "cachear la conexión DuckDB rompe concurrencia": parcialmente válida → absorbida
  en el fix de E-4 (cachear solo la migración, no la conexión).
- **C5 (ronda 1)** — "JSONL da lookup O(n)": inexacta (se carga a dict una vez), pero absorbida:
  el fix exige lookup O(1) en memoria (sqlite/shelve o JSONL→dict).
- **C6 (ronda 2)** — "threadpool no evita contención de GIL": parcialmente válida → E-6 degradado y
  re-anclado a E-1 como fix de fondo.
- **C7 (ronda 1)** — "el hook no spawnea Python por evento": **falsa contra el código**
  (`execFileSync(PY, ...)` literal en context-block-watch.js). Descartada por evidencia.
- **C7 (ronda 2)** — "el pre-filtro por st_size es falaz": no aplica (la métrica gateada es chars/4 y
  size acota chars); nota incorporada igual como aclaración del fix.
- **C8/C9 (ronda 1)** — "no crítico / despreciable a tamaño actual": severidad, ya reflejada en el
  ranking NICE-TO-HAVE.

## Cobertura

Profundo: `mcp_server.py`, `providers.py`, `patterns.py`, `ensemble.py`, `route.py`, `cascade.py`*,
`budget.py`, `metrics.py`, `feedback.py`, `intuition.py` (health-floor), `cache.py`, `memory.py`,
`vault.py`, `context_blocks.py`, `server.py`, `config.py`, `prompts.py`, stores (`chat_store`/
`workflow_store` = conexión módulo-level, OK). Liviano: hooks globales (5 archivos; suggesters =
JS puro sin spawn, OK) y skills (markdown estático, sin costo runtime). (*solo surface-grep.)
Excluido por mandato: backups, `.scratch/`, contenido del vault, docs.
