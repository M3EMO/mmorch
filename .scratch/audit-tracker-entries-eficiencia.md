# Entradas de tracker — auditoría EJE=eficiencia 2026-08-10

Formato del tracker local (`docs/agents/issue-tracker.md`): un ticket por archivo en
`.scratch/<feature-slug>/issues/NN-<slug>.md`, con `Type:` y `Status:` arriba.
Slug propuesto para el merge: `.scratch/audit-2026-08/issues/` (el orquestador decide numeración
final al mergear con los ejes robustez/seguridad). Rankeadas: severidad primero, a igual severidad
menor esfuerzo primero. NO apliqué ningún fix.

---

## NN-cachear-lectura-metrics-jsonl.md

# Hot-path re-parsea metrics.jsonl / feedback.jsonl completos por call

Type: task
Status: open
Severidad: IMPORTANTE
Esfuerzo: M
Origen: audit-eficiencia-2026-08-10 (E-1), verificado cross-family (2 rondas)

`providers.call()` → `budget.check()` (providers.py:132, budget.py:40) parsea todo
`logs/metrics.jsonl` (13.5k líneas, append-only, sin rotación) antes de CADA call API con budget cap;
`route()` lo re-parsea vía `intuition.healthy()`→`error_rates()` (route.py:67, intuition.py:101-102,
metrics.py:63-90) y parsea feedback.jsonl entero vía `calibrate_conf` (route.py:87-89,
feedback.py:77,130-161). Costo O(historia completa) creciente, ×8 concurrente en fan_out.
Fix: cache módulo-level por (path, mtime, size) + tail-read para consumidores ventaneados;
acumulador mensual persistido para `monthly_spend`.

## NN-unificar-default-verifier-legacy.md

# Defaults de verificador en cache.py/ensemble.py usan gemini-2.5-flash legacy (más caro)

Type: task
Status: open
Severidad: IMPORTANTE
Esfuerzo: S
Origen: audit-eficiencia-2026-08-10 (E-3), pasó verificación sin refutación

`cache.memoized_verify` default `verifier_model="gemini-2.5-flash"` (out $2.50/M) y
`ensemble_verify` default `["gemini-2.5-flash","gemini-2.5-flash-lite"]` (ensemble.py:51), mientras
`DEFAULT_VERIFIER="gemini-3.1-flash-lite"` (out $1.50/M, config.py:161) y `pair_verify`
(ensemble.py:195) ya migró. Doble costo: out-price +67% + fragmentación del memo-cache (la key
incluye verifier_model, cache.py:51). Fix: importar y usar `DEFAULT_VERIFIER` en ambos defaults.

## NN-paralelizar-ensemble-verify.md

# ensemble_verify corre K verificadores API en serie

Type: task
Status: open
Severidad: IMPORTANTE
Esfuerzo: S
Origen: audit-eficiencia-2026-08-10 (E-2), verificado cross-family (2 rondas)

ensemble.py:56-58: list comprehension secuencial de `adversarial_verify` (5-30 s c/u). Calls
independientes; el repo ya paraleliza 8 en `fan_out` (patterns.py:67). Fix: ThreadPoolExecutor
preservando orden de verdicts por índice.

## NN-migracion-duckdb-una-vez-por-proceso.md

# memory._connect re-ejecuta DDL + migración en cada operación de memoria

Type: task
Status: open
Severidad: NICE-TO-HAVE
Esfuerzo: S
Origen: audit-eficiencia-2026-08-10 (E-4)

memory.py:83-127: cada op abre conexión DuckDB + 2 CREATE TABLE + 2 CREATE SEQUENCE + scan de
information_schema + loop de 6 columnas; recall_hybrid abre 3 conexiones/llamada (327, 395, 456→194)
en un proceso MCP residente. Fix: flag módulo-level "schema migrado" (DDL una vez por proceso);
NO compartir la conexión entre threads sin lock (objeción del verificador, absorbida).

## NN-prefiltro-stat-en-context-blocks-tick.md

# Hook de Stop parsea el transcript completo (2 veces) + spawn Python por turno

Type: task
Status: open
Severidad: NICE-TO-HAVE
Esfuerzo: S
Origen: audit-eficiencia-2026-08-10 (E-7)

`hooks/context-block-watch.js` ejecuta `execFileSync(python -m mmorch.context_blocks tick ...)` en
cada Stop; `estimate_tokens` parsea el transcript JSONL entero (context_blocks.py:40-56) y sobre el
umbral `compose` lo re-parsea (69-80, 93-96); tabla `context_blocks` sin índice (35-36, 137, 158).
Fix: early-exit por `os.stat().st_size/4 < threshold` antes de parsear (size acota chars y la métrica
es chars/4), una sola pasada compartida, índice (session_id, ts).

## NN-memo-singleton-y-backend-o1.md

# Memo reescribe memo.json completo por put y se re-lee por llamada

Type: task
Status: open
Severidad: NICE-TO-HAVE
Esfuerzo: S
Origen: audit-eficiencia-2026-08-10 (E-5)

cache.py:34-38: cada `put` reserializa todo el archivo (~100 KB, sin poda); cache.py:50: `Memo()`
fresco por `memoized_verify(memo=None)` relee el archivo entero. Fix: singleton módulo-level +
backend con lookup O(1) en memoria y escritura incremental (sqlite/shelve, o JSONL cargado a dict).

## NN-threadpool-en-state-y-benchmarks.md

# state_snapshot/benchmarks_handler bloquean el event loop con parses sync

Type: task
Status: open
Severidad: NICE-TO-HAVE
Esfuerzo: S
Origen: audit-eficiencia-2026-08-10 (E-6)

server.py:42-61 y 290-327: handlers async con 2-4 parses completos de metrics.jsonl inline → SSE
/events y requests concurrentes esperan. El archivo ya usa `run_in_threadpool` (:276, :337).
Fix de fondo = ticket E-1; complemento: envolver estos handlers en run_in_threadpool. (Nota del
verificador: threadpool mitiga pero no elimina contención de GIL.)

## NN-matmul-en-recall-rerank.md

# Rerank de recall convierte embeddings a numpy por par en loop Python

Type: task
Status: open
Severidad: NICE-TO-HAVE
Esfuerzo: M
Origen: audit-eficiencia-2026-08-10 (E-8)

memory.py:341-358 + 71-77: `_cosine` por fila = 2N conversiones np.asarray + N dots chicos.
Fix: matriz (N×384) float32 una vez + un matmul normalizado contra qvec.

## NN-moc-incremental-o-frontmatter-only.md

# regenerate_moc relee todos los .md del vault en cada write

Type: task
Status: open
Severidad: NICE-TO-HAVE
Esfuerzo: M
Origen: audit-eficiencia-2026-08-10 (E-9)

vault.py:99 → 117-150: cada `write_validated` escanea todas las carpetas y hace read_text + parse
de frontmatter de todos los .md. O(vault) por write en una memoria diseñada para crecer.
Fix: leer solo el bloque frontmatter (hasta el 2do `---`) y/o actualizar el MOC incrementalmente.
