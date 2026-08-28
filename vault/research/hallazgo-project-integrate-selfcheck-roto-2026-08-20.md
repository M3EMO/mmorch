---
title: project_integrate.py self-check roto — pre-existente, sin cobertura pytest
status: seed
tags: [research, mmorch, bug, project-integrate]
created: 2026-08-20
---

Al restaurar `mmorch/project_integrate.py` (ver commit de esta noche sobre
el gate rojo del merge_train), su self-check interno (`python -m
mmorch.project_integrate`, bloque `__main__`) falla en el caso 1:

```
assert len(seen_fb) == 3 and seen_fb[0] == "" and "red" in seen_fb[1], seen_fb
AssertionError: ['the clean re-run failed:\nstill red', 'still red', 'still red']
```

`seen_fb[0]` deberia ser `""` (primer feedback, antes de cualquier fallo) y
llega con contenido — indica que `cold_feedback` (dict interno de
`build_project`) ya tenia algo para la unidad "u" antes de que `build_fn`
corriera por primera vez, lo cual no deberia ser posible con un dict recien
creado en esa misma llamada. Sospecha: interaccion con `run_project_build`
(mmorch/project_driver.py, el orquestador F2) — no investigado a fondo.

**Confirmado que NO es de esta noche**: `project_integrate.py` esta
restaurado byte a byte igual al commit `fbf5977` (anterior a todo el
incidente del merge accidental de `mmorch-sbx-7b8ba6cb49b3`), y ninguna de
sus dependencias (`project_driver.py`, `project_build.py`, `feedback.py`,
`intuition.py`) cambio en ese merge. Arbol identico -> mismo resultado ->
esto ya fallaba antes.

**Por que nadie lo vio**: cero tests pytest para `project_integrate.py` —
solo este self-check inline, que nadie corre salvo manualmente. Exactamente
la clase de hueco que `self_audit.py`/`architecture.py` estan pensados para
prevenir (aunque ninguno de los dos mira "self-checks inline sin pytest
wrapper" todavia).

Pendiente: investigar `run_project_build` (F2) para encontrar por que
`cold_feedback` llega poblado antes del primer build_fn. No urgente (el
modulo importa y corre bien en produccion real, esto es un self-check
desactualizado, no necesariamente el codigo de produccion) pero vale
agregar un test pytest que envuelva este caso para que deje de ser invisible.
