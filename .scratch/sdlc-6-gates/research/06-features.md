# Ticket 06 — Set de validación del pipeline (revisado 2026-09-11)

Corrección del usuario: este mapa es SOLO del SDLC de 6 etapas en mmorch. El ChatBot es un
proyecto aparte, con su propio mapa. Sus features no son tickets de acá. La versión anterior
de este archivo (3 features del ChatBot) queda descartada.

Criterio: tests de aceptación ejecutables sin red ni credenciales, tamaño distinto, todo
dentro de mmorch o de su bench congelado. El pipeline se valida sobre lo que mmorch controla.

## Los 3 del bench congelado (`mmorch/bench.py`, `materialize(task, dst)`)

`materialize` arma un repo git nuevo con semillas + tests congelados y devuelve el `accept_cmd`.
Los tests exigen comportamiento, no ausencia de error (anti-Goodhart, lección F4).

| id | task | tamaño | rol |
|---|---|---|---|
| S1 | `lru-ttl-cache` | chica | held-out: NUNCA se usa para elegir variantes; solo para validar que el pipeline generaliza. Se corre UNA vez, al final. |
| S2 | `rate-limiter` | mediana | control histórico: el engine viejo midió 0/3 con el mismo planner. Si el pipeline la pasa, es mejora contra un número existente. |
| S3 | `etl-pipeline` | grande | 3 módulos que se importan entre sí: lo que rompía al flat build-feature. Mide interfaces entre unidades, donde el engine viejo falló. |

Comando de aceptación: el `accept_cmd` de cada task (`python -m pytest tests_accept -q`).
Punto de partida: el repo que `materialize` crea, commit inicial. Sin baseline compartido con nada.

## Los 2 de dogfood: features de mmorch construidas por el pipeline

El pipeline construye mmorch. Son las deudas del engine que el A/B dejó anotadas (ticket 10),
con test de aceptación en pytest, sin red.

- D1 chica: `gen_model` se propaga a la recursión de `project_driver` (hoy las sub-unidades caen a `DEFAULT_GENERATOR`). Aceptación: test con `plan` inyectado que fuerza una recursión y afirma el modelo recibido por el `gen` fake en el nivel 2. Toca `project_integrate.py` y `project_driver.py`.
- D2 mediana: gate `test-compile` antes del `external_test` en `project_integrate` (fail-closed, un comando declarado por el repo, `CheckResult`). Aceptación: test con `integrate` inyectado que afirma que el gate corre antes y corta con detalle cuando falla. Es el primer gate del contrato del ticket 02 escrito como código en el engine.

Comando de aceptación: `python -m pytest tests/test_project_driver.py tests/test_sdlc_gates.py -q`
(el segundo lo escribe Claude antes de lanzar). Regla: el pipeline no toca `tests/`.

## Qué se mide (propuesta para el ticket 07)

| métrica | fuente |
|---|---|
| aceptación verde / rojo | `accept_cmd` a mano, no el run-log |
| USD | `logs/metrics.jsonl` por fase (`sdlc-<id>`) |
| minutos de pared | run-log |
| rechazos por gate y vueltas | run-log `gates` / `test_rounds` |
| nivel máximo de escalación | run-log (`reasoner_rounds`, `escalated_to_claude`) |

Brazos: pipeline v3 (0 Claude) vs híbrido (Claude en escalación). El engine viejo tiene su
número histórico en S2; no se vuelve a correr.

## Orden y costo

S2 → D1 → S3 → D2 → S1 (held-out al final). Costo estimado total: menos de US$3.
Chico primero para ver el piso, held-out último para no contaminar.

## Fuera de este ticket

Las features del ChatBot. Si el pipeline resulta bueno, el ChatBot lo USA desde su propio mapa
como herramienta, con su `sdlc.toml` (ticket 11). No al revés.
