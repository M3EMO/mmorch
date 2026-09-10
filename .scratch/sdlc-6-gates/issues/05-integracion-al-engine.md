# Integracion al engine
Type: grilling
Status: open
Blocked by: 02, 03, 04
Map: ../map.md

## Question

El pipeline reemplaza a project-build (decisión del usuario). Decidir qué se reemplaza y qué se conserva: `project_driver.py` (recursión por unidades) y `project_integrate.py` (coder, cold verifier, integrate) vs el driver por etapas; se conservan worktree aislado, checkpoints, review branch, `/run/workflow`, Lotus. Cómo migran los 4 fixes ya hechos (regla de unidad de aceptación, deps al coder, gen_model/max_fix por payload, max_tokens 32768). Cómo se retira el engine viejo sin romper `tests/test_project_*` ni el skill `/project`. Plan de migración en pasos con test cada uno.
