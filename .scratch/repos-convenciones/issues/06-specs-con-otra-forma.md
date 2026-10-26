# Specs y fichas que ya existen con otra forma
Type: grilling
Status: resolved
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

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-repos-convenciones-ticket-06.md`.

- Primero, `doctor` ignora los archivos sueltos de `docs/specs/` y mira solo las subcarpetas.
- Después, cada spec plana de Adepor se declara como área, y el archivo queda en su lugar.
- Una tabla opcional `[spec_de_area]` en `sdlc.toml`, con `nombre = "ruta"`, apunta a la spec de un área. `[areas]` de `sdd-fase-2` queda igual.
- Un área sin entrada en `[spec_de_area]` usa `docs/specs/<area>/spec.md`. Sus deltas van siempre a `docs/specs/<area>/changes/`.
- La etapa 6 archiva la ficha junto al delta. Con área, la ficha va a `docs/specs/<area>/changes/<slug>-<sha>.ficha.md`; sin área, a `docs/specs/<slug>/ficha.md`.
- La etapa 6 copia la ficha solo si existe, y compara los nombres después de pasarlos por `_slug()`.
- Las fichas de features ya mergeadas quedan donde están, porque `--upgrade` nunca mueve archivos.
- La etapa 6 pertenece al mapa de `sdd-fase-2`. Su sesión recibió estas decisiones el 2026-10-23.

### Tests de aceptación aprobados

`tests/test_sdlc_convenciones.py::test_specs_previas` arma repos en `tmp_path` y verifica nueve puntos:

1. Un archivo suelto como `docs/specs/01-ingesta.md` no da desvío ni aviso, y `--upgrade` no lo toca.
2. Con `[areas]` y `[spec_de_area]`, la spec base de `ingesta` es `docs/specs/01-ingesta.md`.
3. Un área sin entrada en `[spec_de_area]` usa `docs/specs/<area>/spec.md`.
4. Una entrada de `[spec_de_area]` que nombra un área ausente de `[areas]` da desvío.
5. Una entrada de `[spec_de_area]` que apunta a un archivo inexistente da desvío.
6. Una ruta absoluta en `[spec_de_area]` da desvío, por la regla del ticket 05.
7. Una ficha de `docs/sdlc/features/` ya mergeada queda en su lugar tras `--upgrade`.
8. Una ruta de `[spec_de_area]` que sale del repo, como `../x.md`, da desvío.
9. Dos áreas que apuntan a la misma spec dan desvío, contando también las rutas default.

`tests/test_sdlc_delta.py::test_archivo_de_la_ficha` cubre la etapa 6 y verifica cinco puntos:

1. Con área y ficha, la etapa 6 deja `changes/<slug>-<sha>.ficha.md` con los mismos bytes que la ficha.
2. Sin área y con ficha, la etapa 6 deja `docs/specs/<slug>/ficha.md`.
3. Sin ficha, la etapa 6 no crea ningún archivo de ficha.
4. Un área con spec fuera del default guarda sus deltas en `docs/specs/<area>/changes/`.
5. La etapa 6 encuentra la ficha `F1-escena.md` para la tarea "F1 escena", porque compara los nombres con `_slug()`.
