# Qué repos entran y dónde vive la lista
Type: grilling
Status: open
Blocked by:
Map: ../map.md

## Question

¿Qué repos actualiza el mecanismo y qué archivo es la fuente de verdad de esa lista?

El pedido nombra cinco repos: orchestration, ChatBot, Estudio, Portfolio y Proyecto_Adepor. La sonda encontró diez repos con `sdlc.toml` en `/home/mpera`. Adepor no tiene `sdlc.toml`, y el usuario lo dejó fuera del SDLC el 2026-09-17. `projects.json` de mmorch no lista Portfolio, ChatBot ni ssb, y sí lista worktrees y carpetas `scratch-*`.

Opciones a decidir:

- A. Descubrimiento: un repo entra si tiene `sdlc.toml` bajo una raíz declarada (por ejemplo `/home/mpera`). La lista no se mantiene a mano.
- B. Registro: `projects.json` gana un campo `sdlc = true`, y el mecanismo solo recorre esas entradas. Hace falta limpiar el registro.
- C. Lista explícita: un archivo nuevo en orchestration nombra cada repo. El usuario la edita a mano.
- D. Sin lista: el mecanismo actúa sobre un repo por invocación, y el usuario elige cuál.

Preguntas derivadas:

- ¿Los prototipos de `F2-seo-calculadoras` entran, o son repos de usar y tirar?
- ¿Adepor entra a las costumbres SDD (specs) sin entrar al SDLC? Hoy tiene `docs/specs/` con otra forma (ticket 06).
- ¿El escritorio Windows cuenta? Sus clones pueden tener commits que la laptop no tiene (los commits de adopción de Adepor no están acá).

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_lista_de_repos`: en `tmp_path` se arman tres repos git, uno con `sdlc.toml`, uno sin `sdlc.toml` y uno dentro de `.claude/worktrees/`. La función que arma la lista devuelve solo los repos que la opción elegida incluye, en orden estable. Un worktree nunca entra.
