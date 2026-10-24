# Fusión del delta al archivar
Type: grilling
Status: resolved
Blocked by: 04
Map: ../map.md

## Question

¿Cómo queda la spec viva después de una feature con delta?

OpenSpec funde el delta en `specs/` al archivar y mueve el cambio a `changes/archive/`. Hoy `_archivar_spec()` solo copia archivos en la etapa 6. La regla 6.2 del curso pide que la revisión del PR empiece por el diff de la spec.

Opciones a decidir:

- A. Fusión por código. Una función pura aplica el delta a la spec base: agrega `ADDED`, reemplaza `MODIFIED`, quita `REMOVED`. La spec fusionada y el delta quedan en el mismo commit del PR.
- B. Fusión por LLM. El writer reescribe la spec base con el delta. Un gate determinista compara los IDs del resultado contra los esperados.
- C. Sin fusión. La spec base queda fija y los deltas se acumulan en `changes/`. Leer el comportamiento actual exige aplicar todos los deltas en orden.

Preguntas derivadas:

- ¿Qué pasa con las secciones que no son requisitos (Contrato, Casos borde, Criterios de éxito, Trazabilidad)? El delta de OpenSpec solo toca requisitos.
- ¿El PR muestra el diff de la spec antes que el diff del código? Hoy el commit de la etapa 6 mezcla todo.
- ¿Una fusión que deja un ID sin test en la tabla de trazabilidad corta la etapa 6?

## Test de aceptación propuesto

`tests/test_sdlc_delta.py::test_fusion_del_delta`: una spec base con R1, R2 y R3 y un delta (`MODIFIED R1`, `REMOVED R2`, `ADDED R4`). La spec fusionada tiene R1 con el texto nuevo, R3 sin cambios, R4 y ninguna mención de R2. Aplicar el mismo delta dos veces falla con un error claro y no duplica R4. El delta queda guardado en la ruta de cambios que fija el ticket 01.

## Comments

- 2026-10-23, desde el ticket 01 (aprobado por el usuario): dos features con el mismo nombre en la misma área producen el mismo `<slug>`. La segunda pisa `docs/specs/<area>/changes/<slug>.md`. Hoy pasa lo mismo con `docs/specs/<slug>/`. Este ticket decide si el archivo del delta evita el choque.
- 2026-10-23, desde el ticket 02 (aprobado por el usuario): dos ramas en paralelo pueden tomar el mismo número libre, por ejemplo R3. La fusión chequea cada `ADDED` contra la spec base del momento de fusionar, no contra la del momento en que nació el delta.

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-sdd-fase-2-ticket-06.md`.

- La fusión es por código (decidido en el ticket 04). Una función pura aplica el delta a la spec base.
- En modo área, cada requisito es un bloque con su regla, sus casos borde y su criterio. Un `MODIFIED` arrastra el bloque entero.
- La Trazabilidad se deriva por código desde `tests/sdlc/<area>/`, un archivo por ID.
- La sección Contrato del área lista solo las rutas cubiertas, derivadas del plan y de la caracterización. Las firmas viven en el plan de cada feature.
- El pre-commit chequea las partes derivadas y falla si están viejas. El mensaje nombra `python -m sdlc specs --regenerar`. El hook nunca escribe archivos (memoria `docgen-hook-indice`).
- El pre-commit también exige IDs únicos en la spec y un archivo por ID. Un choque entre dos ramas paralelas se rehace desde la etapa 1.
- Un requisito cambia solo por delta. El pre-commit rechaza un commit que cambia un requisito de `spec.md` sin sumar un archivo a `changes/`.
- La etapa 6 hace dos commits. El primero lleva el delta, la spec fusionada y `removidos/`. El segundo lleva el código.
- El delta se archiva como `docs/specs/<area>/changes/<slug>-<sha corto del commit base>.md`, con su `.plan.md` al lado.

### Test de aceptación aprobado

`tests/test_sdlc_delta.py::test_fusion_del_delta` usa una base con R1, R2 y R3, y un delta con `MODIFIED R1`, `REMOVED R2` y `ADDED R4`:

1. La spec fusionada tiene el bloque nuevo de R1, R3 sin cambios byte a byte y R4, sin R2.
2. Aplicar el mismo delta dos veces falla con un error claro y no duplica R4.
3. La Trazabilidad fusionada sale de `tests/sdlc/<area>/` y cita `ruta::test` por ID.
4. El Contrato fusionado lista solo rutas cubiertas, sin firmas.
5. Un requisito fusionado sin archivo de test corta la etapa 6.
6. La etapa 6 hace dos commits, y el primero solo toca `docs/specs/<area>/`.
7. El delta queda en `changes/<slug>-<sha>.md`, con su `.plan.md` al lado.
8. Un test nuevo agregado a mano deja la Trazabilidad vieja, y el pre-commit falla con el comando que la regenera.
9. El pre-commit que falla no escribe ningún archivo.
10. Tras `python -m sdlc specs --regenerar`, el pre-commit pasa.
11. Dos archivos `test_R3_*` en la misma área hacen fallar el pre-commit.
12. Un R3 repetido en la spec también hace fallar el pre-commit.
13. Una corrida sin área deja la etapa 6 igual que hoy.
14. Un commit que cambia un requisito de `spec.md` sin sumar un archivo a `changes/` hace fallar el pre-commit.
15. Una etapa 6 reanudada tras una falla no vuelve a aplicar el delta, y termina igual.
16. Correr `--regenerar` dos veces seguidas no deja diferencias.

## Comments (después de resolver)

- 2026-10-23, desde el esfuerzo `orchestration-4u6` (ticket 06 de `.scratch/repos-convenciones/`, P3 y P4; el usuario confirmó los nombres; registro en `brainstorms/2026-10-23-repos-convenciones-ticket-06.md` de la rama `claude/youthful-bohr-f13d78`): la etapa 6 archiva la ficha de la feature si existe `docs/sdlc/features/<slug>.md`. Con área, la copia va a `docs/specs/<area>/changes/<slug>-<sha>.ficha.md`, con el mismo sha que el delta y el plan. Sin área, va a `docs/specs/<slug>/ficha.md`. Una feature sin ficha no cambia. Las fichas de features ya mergeadas quedan donde están. La copia entra al primer commit de la etapa 6 (el de la spec).
