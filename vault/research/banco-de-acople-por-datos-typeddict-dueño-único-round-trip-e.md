---
title: Banco de acople por datos: TypedDict, dueño único, round-trip, escala y trinquete (2026-09-24)
created: 2026-09-24
tags: [research, mmorch, mutation-testing, acople-por-datos, codegraph, metrics.jsonl, benchmark]
status: measured
confidence: media
sources: [worktree orch-bench rama bench/data-coupling: scripts/bench_acople/ (PREREG.md, mutants.json sha256, run_matrix.py, analyze.py, results.jsonl, results_i4_rerun.jsonl); issue bd orchestration-be8; video youtube k2qls2LiBRc]
---
## Pregunta
El video "AI gives too much code" plantea tres fallas de la IA en sistemas grandes. El pilar 3 es el acople por datos: un modulo escribe un archivo y otro lo lee sin referencia en el codigo. `codegraph_impact(log_event)` devuelve 54 simbolos, todos escritores y ningun lector de metrics.jsonl. La pregunta es que intervencion cierra ese hueco mejor por linea de codigo.

## Protocolo (pre-registrado en PREREG.md)
- deepseek-chat genero 45 mutantes sin conocer las intervenciones; 40 pasaron el filtro de validez (29 de acople clase A, 11 de rendimiento clase B).
- Oraculo: mypy + subset congelado de 32 archivos de tests, contra las fallas de la base limpia. Ningun LLM juzga.
- I1 TypedDict MetricEvent compartido (30 lineas). I2 dueño unico de metrics.jsonl + test AST (26). I3 tests round-trip por lector (75). I4 banco de escala t(N) vs t(10N) (64).
- 4 estados de codigo x 40 mutantes; 16 combinaciones por OR exacto de checks. I4 repetido con maquina quieta: 0 cambios.
- Falsos positivos en codigo limpio: ninguno.

## Resultado
| combinacion | detecta /40 | extra vs gate | lineas | extra /100 lineas | seg extra |
|---|---|---|---|---|---|
| gate actual | 26 | 0 | 0 | - | 0 |
| I1 | 30 | 4 | 30 | 13.3 | 0 |
| I2 | 27 | 1 | 26 | 3.8 | 2.6 |
| I3 | 30 | 4 | 75 | 5.3 | 3.8 |
| I4 | 28 | 2 | 64 | 3.1 | 7.8 |
| I1+I2 | 31 | 5 | 56 | 8.9 | 2.6 |
| I1+I2+I4 | 33 | 7 | 120 | 5.8 | 10.4 |
| I1+I2+I3+I4 | 34 | 8 | 195 | 4.1 | 14.2 |

Unicos por intervencion: I1 m15 (sdlc lee `timestamp`), I2 m33 (elimina el lector crudo), I3 m22 (latencia en ms), I4 m40 y m42 (cuadraticos).
Nadie detecta: m25 (separador del iso; casi equivalente), m32 y m34 (ventana cabeza vs cola), m39, m41, m44 (rendimiento lineal o fuera de la lista caliente).

## Lecturas
- El gate actual ya detecta 26/40: los tests existentes cubren gran parte del acople de metrics.jsonl.
- I1 es el mas eficiente: +4 con 30 lineas y cero segundos extra, porque mypy ya corre.
- mypy NO marca `.get("clave_inexistente")` en TypedDict; solo marca cuando el `object` resultante entra en una operacion tipada. Por eso m13 escapa.
- I3 solo agrega m22 sobre las demas. Sus tests usan 2 eventos, asi que no ven errores de ventana.
- I4 solo agrega los cuadraticos que escalan con la ventana. No ve regresiones lineales por diseño (umbral 25).
- Prediccion fallida: el gate ya detectaba los 4 mutantes de cache rota (B1).

## I5 trinquete de volumen (eje aparte)
- Replay de 375 commits en 60 dias: con umbral +50 lineas netas en mmorch/, bloquea 19% de commits; con +200, 3%.
- Proxy de beneficio: supervivencia de lineas en HEAD. Commits chicos 86%, medianos 81%, grandes 88%. No hay señal de que el codigo grande sea mas descartable.
- Costo medido, beneficio no observado. I1 (+30 lineas netas) quedaria bloqueado por un trinquete estricto.

## Limites
- n=40 sobre una sola superficie (metrics.jsonl); diferencias de 1-2 mutantes no son robustas.
- El autor de las intervenciones leyo las intenciones de los mutantes antes de escribirlas.
- Un solo generador de mutantes (deepseek-chat).

## Recomendacion
Implementar I1+I2 (56 lineas, +5 detecciones, 2.6 s de gate). I4 queda opcional para caminos calientes con ventana. I3 y I5 no pagan en esta medicion.
