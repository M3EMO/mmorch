# Fusión del delta al archivar
Type: grilling
Status: open
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
