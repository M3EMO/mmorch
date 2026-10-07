# Repos sin tests: verificar el cambio acoplado sin oráculo visible
Type: prototype
Status: resolved
Blocked by: 
Map: ../map.md

## Question

En un repo sin tests, ¿el informe sigue evitando el error del lector acoplado, o hace falta que el agente escriba primero un test de caracterización del lector?

Hoy los tests visibles del banco cubren solo al escritor, así que el banco ya mide casi este caso. Falta medir la versión extrema: sin ningún test visible y sin `run_tests`.

Método: el banco del ticket 09 sin la carpeta `tests/` visible; el oráculo oculto no cambia. Condiciones: B (informe) y B0 (informe, sin tests). Si B0 pierde más de 10 puntos contra B, se prueba T: el informe pide escribir un test de caracterización de cada lector listado antes de editar.

Criterio: si B0 queda a 10 puntos o menos de B, se cierra sin cambios. Si no, T tiene que recuperar al menos 25 puntos sobre B0 sin perder más de 10 en control.

## Answer

Cerrado sin cambios (2026-09-27). Aceptado por el usuario el 2026-09-28. Banco del ticket 09, informe instalado, deepseek-v4-pro, 72 corridas, US$0.81, cero errores de API (commit 04e7767 en `orch-spike`; resultados en `scripts/tareas/results_deepseek-v4-pro_t14.jsonl`, fuera de git como los del ticket 09): acopladas B 22/24 (92%) y B0 sin tests 21/24 (88%), -4 puntos; control 12/12 en las dos. Cumple el criterio de cierre (B0 a 10 puntos o menos de B).

- En B0 el agente nunca pidió `run_tests`: el informe, y no los tests visibles, lleva al lector acoplado.
- Única diferencia: t01 (B 1/3, B0 0/3). Con el mismo formato de informe, t01 dio 3/3 en la aceptación del ticket 05: es varianza con n = 3.
- Debilidad de formato vista en t01: la columna que cambia (`total`) aparece en 12 módulos (señuelos) y el informe la resume como literal común, sin lista. El agente llega al lector por `discount`. No se toca sin un banco que lo mida.

## Notes

**2026-09-27 — pre-registro (Claude, antes de correr):**

- Banco: los 8 repos y 12 tareas del ticket 09 (`orch-spike/scripts/tareas`), deepseek-v4-pro, 3 repeticiones, 3 workers.
- Informe: el instalado hoy (`mmorch.impacto.report`: literales + aristas indirectas en `.py`), el mismo en las dos condiciones.
- B: repo con sus tests visibles y la herramienta `run_tests`.
- B0: repo sin la carpeta `tests/` y sin `run_tests` (la herramienta no aparece en el prompt; si el agente la pide, recibe "herramienta desconocida"). El oráculo oculto no cambia.
- B y B0 se corren juntos ahora (72 corridas), no se compara con la B vieja del ticket 05, que usaba otra versión del informe.
- Criterio: si B0 queda a 10 puntos o menos de B en acopladas y en control, el ticket se cierra sin cambios. Si no, se pre-registra la condición T (test de caracterización de cada lector antes de editar).
- Errores de API: se cuentan aparte y se repiten.
