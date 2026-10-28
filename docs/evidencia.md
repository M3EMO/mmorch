# Evidencia

## Evidencia heredada

Las mediciones de la sección «Measured» de mmorch valen como evidencia heredada (decisión b1).
Su método está en revisión. Se revisa con la calibración (Tarea 12) y con la validez (Tarea 12b). Ambas tareas están pendientes.
Root no recalcula estas cifras. Las cita tal como están en cada fuente.
Cada tabla dice qué midió cada fuente, qué cifras reporta y por qué su muestra es limitada.

## medido-mmorch

Fuente: `docs/contexto/medido-mmorch.md`.

| Campo | Contenido |
|---|---|
| Qué midió | Verificadores LLM sobre 350 ítems aritméticos con verdad computada. Después, bloques de compuertas, checkers sintetizados, una tabla de 57 tipos de problema, una comparación de pipeline contra motor y un pipeline de 6 etapas. Después, compuertas del SDLC con casos construidos. |
| Cifras reportadas | Verificadores (tabla de 2026-10-07): `deepseek-chat` con thinking OFF: atrapa bugs 0.94, no rechaza mal 0.56-0.61, exactitud balanceada 0.77. `deepseek-reasoner` con thinking ON: 1.00, 0.994, 0.997, $0.074. `glm-5.2`: 1.00, 1.00, 1.00, ~$0.85. Qwen3-4B en Colab T4: 1.00, 0.977, 0.989. Re-corrida 2026-10-07: `deepseek-reasoner` y `glm-5.2` dan 1.00 / 1.00. Sesgo de familia: índice 0.03, IC90 [-0.015, 0.075]. Compuertas en cadena (n=299): especificidad de K=2 a 0.593, K=3 a 0.587, K=4 a 0.587. Checkers sintetizados (n=130): `code:deepseek-v4-pro` 1.00 (130/130). Tabla de 57 tipos: `deepseek-chat` promueve 53/57, `deepseek-reasoner` 57/57, cero bugs perdidos en 274 funciones. Pipeline de 6 etapas: 6/6 verdes incluido el held-out, mediana US$0.037 por feature. Compuertas del SDLC (2026-10-23, n en la tabla): `sin-clones` 1.00, `cambio-minimo` 0.970, `docstring-intacto` 0.953. |
| Método | Verdad computada: aritmética y tipos de problema con referencia por fuerza bruta. Pares de ablación con la misma semilla, para que los brazos se comparen directo. McNemar para pares discordantes. Errores inyectados. Compuertas del SDLC con etiqueta según cómo se construye cada caso, nunca según la regla de la compuerta. |
| Advertencia de muestra | Una semilla por corrida. Un solo dominio (aritmética). Los errores son inyectados, así que mide detección, no el punto ciego del propio verificador. El set quedó saturado para verificadores con thinking: ya no ordena a `deepseek-reasoner` y `glm-5.2`. El documento dice que `deepseek-v4-pro` midió un verificador perfecto sobre ese set, y pide una segunda mirada. El pipeline de 6 etapas tiene una corrida por feature. Varios umbrales se cambiaron tras medir (`sin-clones`, de 0.8 a 0.66, el 2026-10-26). |

## veredicto-ecc-f1

Fuente: `docs/contexto/veredicto-ecc-f1.md`.

| Campo | Contenido |
|---|---|
| Qué midió | ECC v2.2.3 contra un control sin ECC, en una tarea real: el generador de contenido FREE/PREMIUM de F1 desde `portfolio.db`. El oráculo tiene 23 comprobaciones que ECC no vio. |
| Cifras reportadas | Oráculo held-out: ECC 23/23, sin ECC 23/23. Costo estimado: ~1,3 USD con ECC y una corrida cortada, 0,35 USD sin ECC. Turnos humanos: 3 con ECC, 1 sin ECC. Código y tests: 326 líneas y 38 tests con ECC, 373 líneas y 12 tests sin ECC. Mutaciones del filtro que matan sus tests: 2 de 3 en ambos. |
| Método | `claude -p` con `--plugin-dir` para ECC. El control usa el mismo commit, el mismo modelo y los mismos flags, sin el plugin. Oráculo held-out de 23 comprobaciones. Revisión de Opus sobre el mismo código. |
| Advertencia de muestra | n=1 tarea, un modelo (Sonnet 5.5), una corrida de control. El documento dice que falta medir con más tareas (n>1). Los costos de ECC son estimados por sus hooks, y una corrida se cortó sin registro de costo. |

## mapa

Fuente: `docs/contexto/mapa.md`.

| Campo | Contenido |
|---|---|
| Qué midió | Punto de partida del banco del 2026-10-27: el mismo bootstrap de memora y brasa, con ECC y con el SDLC. |
| Cifras reportadas | memora con ECC y Sonnet: contrato 61/61, oráculo oculto 48/48, 0 intervenciones, ~US$5,6 de cupo. brasa con ECC y Sonnet: contrato 53/53, oculto 15/16, 0 intervenciones, ~US$6,8 de cupo. memora con el SDLC: contrato 21/61 (2 de 9 módulos), oculto 20/48, 4 intervenciones, US$2,92 de API. brasa con el SDLC: sin medir (3 de 10 módulos), 5 intervenciones, US$3,37 de API. |
| Método | Banco en `~/Proyectos/bench-ecc-sdlc/`. Oráculos y resultados en `.scratch/ecc-vs-sdlc/oraculo/`. Una corrida por celda. |
| Advertencia de muestra | Una corrida por celda, en un host. El documento dice que Haiku 5.5 dentro de ECC no está medido: el banco usó Sonnet. El costo mezcla cupo y API según la celda. La cifra de brasa con el SDLC está sin medir. |

## GOAL-mmorch

Fuente: `docs/contexto/GOAL-mmorch.md`.

| Campo | Contenido |
|---|---|
| Qué midió | Nada. Es el contrato anti-drift de mmorch: propósito, invariantes y fuera de alcance. |
| Cifras reportadas | Ninguna. |
| Método | No aplica. El hash de `GOAL.md` se conserva (decisión b2). |
| Advertencia de muestra | No es evidencia de desempeño. Fija qué invariantes no se pierden al migrar. |

## Banco ECC contra SDLC (logs)

Fuente: `~/Proyectos/bench-ecc-sdlc/logs/`. Root no escribe ni cambia estos archivos.

| Campo | Contenido |
|---|---|
| Qué midió | Corridas de memora y brasa con ECC y con el SDLC, del 2026-10-27. Y las sesiones de planeación del PRD. |
| Cifras reportadas | `memora-ecc.jsonl`: 1 línea, `total_cost_usd` 5.626639800000003, `is_error` false. `brasa-ecc.jsonl`: 2 líneas, `total_cost_usd` 2.7945993999999996 y 4.041864599999999. `memora-sdlc.jsonl`: estados `built` 2 y `escalate` 3. `brasa-sdlc.jsonl`: estados `built` 5 y `escalate` 4. `intervenciones.log`: 9 entradas. `sistema-ecc.jsonl`: 27 líneas de sesiones de planeación, sin tareas medidas. |
| Método | Salida JSON de cada sesión de `claude -p` para ECC. Registro por módulo del driver para el SDLC. Los `.err` guardan el código de salida de cada corrida. |
| Advertencia de muestra | Una corrida por banco, en el host. Los `intervenciones.log` cuentan reintentos humanos, no tiempo. Los costos vienen tal cual de `claude -p`. `sistema-ecc.jsonl` no mide construcción. |

## memora: diferencias contra el banco

Fuente: `~/.local/share/root/oraculos/memora/diferencias.md`.

| Campo | Contenido |
|---|---|
| Qué midió | Cada test de memora que difiere del banco: `test_blpop_fifo_entre_clientes` y `test_redis_py`. Contrato 61/61 en contenedor, igual que el banco. |
| Cifras reportadas | `test_blpop_fifo_entre_clientes`: banco pasa (1 corrida). Host: 9/30 pasan. Contenedor: 9/30 pasan. Variante de diagnóstico: contenedor 11 de 30, host 15 de 30. `test_redis_py`: banco falla (NOPROTO). Hoy 3/3 en host y 3/3 en contenedor. Oráculo oculto en contenedor: 47 pasan y 1 falla. El documento corrige el brief: el banco dio 47/48, no 48/48. |
| Método | 30 corridas del test inestable por entorno. Contenedor con `podman run --network none`. Host con el venv del banco. `test_redis_py`: 3 corridas por entorno. |
| Advertencia de muestra | `test_blpop_fifo_entre_clientes` es inestable: falla 21 de 30 en ambos entornos. El origen (`producto` u `oraculo`) queda como pregunta al usuario. Según la decisión 16, los conteos exactos no se exigen. El estado no sale de un conteo solo. |

## brasa: diferencias contra el banco

Fuente: `~/.local/share/root/oraculos/brasa/diferencias.md`.

| Campo | Contenido |
|---|---|
| Qué midió | Cada test de brasa que difiere del banco: `test_spin_despues_de_rotacion`, `test_rendimiento[gpu-10000-res1]` y el contrato. |
| Cifras reportadas | `test_spin_despues_de_rotacion`: falla en el banco y 3 de 3 en host y en contenedor. `test_rendimiento[gpu-10000-res1]`: el banco pasa (frame_ms 178.161). Host 3 de 3. Contenedor 3 de 3 fallan, sin adaptador de hardware. Oculto en el banco: 15 pasan, 1 falla, 5 se saltan. Host igual. Contenedor: 14 pasan, 2 fallan, 5 se saltan. Contrato: una evaluación real del Oracle (Tarea 7, primera corrida, 1 repetición, `r00t oraculo evaluar --tarea brasa`) dio 52/53, y oculto 14/16. El reporte de esa corrida no se conservó: el Oracle no guardaba el junit. No se reprodujo: 53/53 en 3 corridas de contenedor y 1 de host. |
| Método | Repeticiones con `-k` sobre los tests en disputa. Corridas completas en contenedor con `podman run --network none`. Host con el venv del banco. El contrato 52/53 de la evaluación de la Tarea 7 no se reprodujo en 3 corridas de contenedor ni en 1 de host (53/53). Queda sin determinar. |
| Advertencia de muestra | La diferencia de `gpu` viene del entorno: el contenedor tiene solo llvmpipe, que es CPU. Los 5 tests `test_modelo_khronos` se saltan porque faltan las muestras `.glb`. Las repeticiones son 3 por entorno. El contrato 52/53 queda sin determinar: es un test del contrato inestable que una sola corrida no detecta. Con 5 repeticiones del contrato (Tarea 7f) se detectaría. |

## Qué cambió desde el banco original

- Repeticiones: cada `tarea.toml` declara `repeticiones` y `repeticiones_inestable`. El default de `repeticiones_inestable` es 10. Los `tarea.toml` actuales de memora y brasa dicen `repeticiones = 5`. El registro de diferencias de brasa dice 3. Esta diferencia no se resuelve aquí. Nota: los `diferencias.md` de brasa y memora hablan de 3 repeticiones, y los `tarea.toml` de hoy usan 5 (`repeticiones = 5`, `repeticiones_inestable = 10`).
- Causas por diferencia: cada test diferente recibe una causa (`producto`, `oraculo`, `entorno`, `inestable` o `sin determinar`) y su origen. Solo `[[entorno]]` y `[[oraculo_dudoso]]` excluyen tests, y cada exclusión lleva un motivo. Los tests de `producto` no se excluyen.
- Cambio de test: `test_redis_py` fija `protocol=2` en el cliente. Antes dependía del default de redis-py. La causa era `oraculo`, y ya está corregida.
- Fichas: los SPEC de memora y de brasa todavía no tienen sección `## Ficha`. Su redacción y aprobación son de la Tarea 9f, pendiente.
- Preguntas al usuario sobre el origen de dos diferencias: el orden FIFO de BLPOP en memora y el espacio "local" de brasa. Siguen abiertas.
