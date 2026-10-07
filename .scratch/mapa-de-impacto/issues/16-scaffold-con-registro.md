# Scaffold de /new-project con registro tipado y dueño único
Type: prototype
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿El skill `/new-project` tiene que generar desde el día uno un registro tipado de almacenes y un módulo dueño único por almacén, como `MetricEvent` en `mmorch/metrics.py`?

Evidencia previa: en este repo, registro tipado + dueño único + 19 contratos escritor-lector cerraron el acople de `logs/metrics.jsonl` (commit 0b1c4a4); los contratos generales no convergieron (ticket 06).

Método: dos versiones de un repo chico generado por el skill (con y sin registro y dueño único), y 8 tareas de cambio con lector acoplado sobre cada versión, mismo oráculo del ticket 09. Condiciones: S0 (scaffold actual) y S1 (scaffold con registro, dueño único y un test de contrato que falla si otro módulo escribe el almacén).

Criterio: S1 supera a S0 por >= 25 puntos en acopladas, sin informe de impacto, y sin perder más de 10 en control. Si el informe ya lleva S0 cerca de S1, el scaffold no agrega nada.

## Answer

No pasa (2026-09-29); aceptado por el usuario el 2026-09-29: `/new-project` no cambia. Banco de 12 tareas, deepseek-v4-pro, 72 corridas, US$0.53, cero errores de API (commits 1b34e63 y el de resultados en `orch-spike`): funciones nuevas que sobreviven a la fase 2, S0 23/24 y S1 24/24 (+4 puntos, p = 0.5); control S0 12/12, S1 11/12.

- Efecto techo: con un módulo dueño que tiene una API estable, el agente la usa aunque exista un lector que abre el archivo directo (S0). Las dos fallas son corridas donde el agente terminó sin escribir nada.
- Lo que aporta es el dueño con API estable, que está en las dos variantes. El registro tipado y el test de contrato no suman nada medible encima.
- Límite: no se midió un repo orgánico sin ningún dueño (acceso directo en todos lados). Ese es el caso del ticket 09, y ahí el informe de impacto ya lleva el acierto a 92-96%.

## Notes

**2026-09-29 — cambio de diseño (decisión del usuario): banco en dos fases, con informe.** En S1 el cambio de formato queda dentro del dueño, así que medir solo ese cambio da una victoria por construcción. La pregunta útil: ¿el código nuevo que escribe el agente sobrevive a un cambio de formato posterior?

- Fase 1 (agente): agrega una función nueva que necesita datos del almacén, en un módulo que hoy no lo toca.
- Fase 2 (fija, la aplica el banco): cambia el formato del almacén con reemplazos puntuales, como lo haría quien mantiene el repo: en S0 toca el escritor y los lectores que ya existían; en S1, solo el dueño. El código que agregó el agente no se toca.
- Oráculo: el test oculto escribe datos con la API del escritor ya cambiado y prueba la función de la fase 1.
- S0: estructura orgánica; el escritor tiene un helper de lectura que devuelve registros crudos y hay otro módulo que lee el archivo directo (mal ejemplo real). S1: `stores.py` con el registro tipado, un módulo dueño por almacén con API estable (normaliza el formato) y un test de contrato visible que falla si otro módulo nombra o abre el archivo del almacén.
- Las dos variantes llevan el informe de impacto instalado al inicio (el caso real hoy).

**2026-09-29 — pre-registro (Claude, antes de correr):**

- Banco (`orch-spike/scripts/tareas_scaffold`): 8 repos Python con 30 señuelos; cada uno con un almacén de formato distinto (CSV, JSONL, JSON, SQLite, texto) y un módulo dueño con API estable, igual en S0 y S1. S0 agrega un lector organico que abre el archivo directo. S1 agrega `stores.py` (registro tipado), `tests/test_contrato_almacenes.py` (falla si otro módulo nombra el archivo) y una línea en `CLAUDE.md`; su lector pasa por el dueño.
- 8 tareas de fase 1 (una función nueva que necesita el almacén, en un módulo que hoy no lo toca) y 4 controles (función pura). Fase 2 fija por repo: separador CSV, renombre de campo, anidar JSON, renombre de tabla, renombre de archivo, precios en centavos, orden de columnas, tiempos en ms.
- Oráculo validado en las 24 combinaciones: la referencia por API pasa después de la fase 2 en S0 y S1; la referencia que lee directo falla en S0 y S1 (en S1 además la marca el test de contrato visible); los controles pasan con las dos.
- Condiciones: S0 y S1, las dos con el informe de impacto instalado al inicio. deepseek-v4-pro, 3 repeticiones, 72 corridas, 3 workers; errores de API aparte.
- Criterio: S1 supera a S0 por >= 25 puntos en las 8 tareas sin perder más de 10 en control. Si pasa, `/new-project` genera registro, dueño y test de contrato para cada almacén que declare el proyecto.
