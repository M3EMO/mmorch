---
title: suite completa no es order/paralelo-safe
status: seed
tags: [mmorch, robustez, pendiente]
created: 2026-08-19
---

3 corridas completas de la suite hoy (2026-08-19), 3 tests DISTINTOS fallando
cada vez, ninguno relacionado entre si ni con el trabajo del dia:

1. test_server_smoke.py::test_route_table_matches_contract — real (deuda
   vieja, /pending y /verdict nunca entraron al contrato). Arreglado.
2. test_hillclimb.py::test_fixed_arm_records_outcomes_and_updates_bandit —
   PermissionError en iohelpers.py. Aislado: pasa solo.
3. test_reconsolidation.py::test_review_frozen_from_consolidate — aislado:
   pasa solo.

Patron: cada fallo desaparece en aislamiento -> no es logica rota, es la
suite corriendo en paralelo/con corridas concurrentes pisandose (probable:
temp compartido, lock de archivo, o recurso global no aislado por test).

Pendiente: identificar el recurso compartido. Sospechosos: iohelpers.py
(atomic_write_json/load_json_tolerant si dos tests tocan el mismo path por
default), o basetemp compartido entre corridas simultaneas del venv.
No bloqueante hoy (ningun test tocado por el trabajo de la sesion), pero
vale mirarlo antes de escalar merge_train/hardening a mas corridas por dia.

## Actualización — RESUELTO, causa raíz distinta a la sospechada

Corrida de control: suite completa, aislada, `--basetemp` único, nada más
corriendo en paralelo → **617 passed, 0 failed, 798.62s**.

Causa real: yo mismo reutilicé el mismo directorio `--basetemp` en corridas
de pytest que se solapaban en el tiempo (una corrida de fondo + comandos
sueltos en foreground apuntando al mismo path). Windows no soporta bien dos
procesos escribiendo/limpiando el mismo temp dir a la vez → `PermissionError`
en el que perdía la carrera, distinto test cada vez según el timing.

NO es un bug de hermeticidad de la suite (no hay tempfile.mkdtemp sin
tmp_path, no hay globals mutados entre tests — confirmado también por
architecture.py, 0 señales). Lección: nunca reusar `--basetemp` entre
invocaciones de pytest que puedan solaparse en el tiempo.
