# Spike: aristas de datos por audit hook y recall de impacto
Type: prototype
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Las aristas de datos extraídas con `sys.addaudithook` mejoran el recall de impacto frente a codegraph solo, sobre los mutantes del banco de acople?

Pasos:

1. **Extractor.** Correr la suite de mmorch con un audit hook (stdlib) que registre `open`, `sqlite3.connect` y `subprocess.Popen`. Cada evento guarda la ruta o el comando y la función de producción más cercana en el stack. Salida: aristas escritor→almacén→lector.
2. **Verdad computable.** Para cada mutante del banco, correr el subset SIN `-x` y guardar TODOS los tests que fallan. Los resultados de hoy usaron `-x` y guardan solo el primero.
3. **Predicción.** El corte de impacto de la función mutada: (a) codegraph solo (`codegraph_impact`); (b) codegraph + aristas de datos. Un test cuenta como predicho si llama a alguna función del corte.
4. **Métrica.** Recall = tests que fallan y fueron predichos / tests que fallan. También el tamaño del corte, como costo de contexto.

Criterio a fijar antes de medir (pre-registro): cuánto recall extra justifica seguir, y con qué tamaño máximo de corte.

Superficie: los 40 mutantes del banco + los held-out r1-r4. Worktree: `../orch-bench`.

## Comments

**2026-09-24 — pre-registro (Claude, antes de medir; el usuario puede vetarlo):**

- Código: worktree `../orch-spike` (detached en `bench/i12`), carpeta `scripts/impacto/`: `trace_plugin.py` (audit hook), `truth.py` (verdad sin `-x`), `predict.py` (cortes A y B).
- Verdad: tests que fallan en el subset de 32 archivos, sin `-x`, para los mutantes cuyo check de tests falló antes (58 de rondas bench y r1-r3, más los de r4).
- Sesgo conocido: el subset se eligió por imports estáticos. Se estima con la suite completa en 5 mutantes.
- A = codegraph solo (BFS inverso sobre calls/instantiates/decorates). B = A + lectores de los almacenes que la función mutada escribe, según el audit hook.
- Criterio: B sirve si su recall medio supera al de A por al menos 0.10 con D=3, y su corte mediano es como máximo 2x el de A. Se reportan también D=2 y D=4.
- Limitación: el audit hook solo ve lo que ejercita la suite, y solo en Python.

## Answer

Negativo para selección de tests. Con 66 mutantes y verdad sin `-x`, el recall de codegraph solo (A) y de codegraph + datos (B) es 1.00 con D=2, 3 y 4; el corte mediano no cambia (37 funciones con D=3). La suite completa en 5 mutantes no mostró ningún test roto fuera del subset. Motivo: en mmorch, todo test que falla importa el módulo mutado. Límite de la métrica: la verdad sale de tests que fallan, así que nunca incluye los escapes, que son el riesgo real. Hallazgos laterales: codegraph 0.9.6 no resuelve llamadas por alias (`B.monthly_spend()`), así que no conecta tests con producción, y su nodo de import guarda solo `mmorch`. Código: `../orch-spike/scripts/impacto/`. Siguiente: medir el caso del video con tareas de cambio (ticket 09). Aceptado por el usuario el 2026-09-25.
