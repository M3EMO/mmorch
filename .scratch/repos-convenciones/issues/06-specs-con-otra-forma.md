# Specs y fichas que ya existen con otra forma
Type: grilling
Status: open
Blocked by: 01
Map: ../map.md

## Question

¿Qué hace el mecanismo con documentos de spec que ya existen y no siguen `docs/specs/<slug>/` ni `docs/specs/<area>/`?

La sonda encontró dos formas previas:

- Adepor tiene `docs/specs/NN-<tema>.md`: diez archivos planos, uno por tema (ingesta, liquidación, fotmob). Ese formato ocupa el mismo directorio que la convención de áreas. El ticket 01 de `sdd-fase-2` buscó `docs/specs/*/spec.md` y no los vio, porque son archivos sueltos.
- Estudio, Portfolio, ChatBot y ssb tienen fichas `docs/sdlc/features/<slug>.md`. Esas fichas son el embudo wayfinder→SDLC del 2026-09-15 y se solapan con `docs/specs/<slug>/`.

Opciones a decidir:

- A. Convivencia: el mecanismo ignora archivos sueltos en `docs/specs/` y solo mira subcarpetas.
- B. Migración propuesta: el mecanismo propone mover cada spec plana a `docs/specs/<area>/spec.md` y el usuario aprueba cada movimiento.
- C. Área declarada: cada spec plana de Adepor se vuelve un área con su glob, y el archivo queda donde está con `spec = <ruta>` en `sdlc.toml`.

Preguntas derivadas:

- ¿Las fichas de `docs/sdlc/features/` siguen vivas, se archivan junto al delta o se retiran?
- ¿Adepor entra a las costumbres SDD aunque siga fuera del SDLC (ticket 02)?

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_specs_previas`: en `tmp_path` se arma un repo con `docs/specs/01-ingesta.md` plano y `docs/sdlc/features/x.md`. Tras aplicar el mecanismo, ningún archivo previo cambió ni se movió sin aprobación, y el reporte nombra cada archivo con la acción que propone la opción elegida.
