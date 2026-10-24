# Estructura de AGENTS.md y CLAUDE.md en los repos propios

Este documento es el contrato acordado en el grilling del 2026-10-23 (bd `orchestration-ior`).
La captura completa está en `brainstorms/2026-10-23-estructura-md-y-append-bd.md`.
El oráculo es `python -m sdlc lint-md <repo>`; ningún LLM decide si un repo cumple.

## Reparto entre archivos

- `AGENTS.md` es la fuente de las reglas comunes a todos los agentes.
- `CLAUDE.md` importa `@AGENTS.md` y agrega solo lo propio de Claude.
- bd escribe su bloque solo en `AGENTS.md` cuando `CLAUDE.md` importa `@AGENTS.md`.
- Ningún texto se repite en los dos archivos.

## Forma de AGENTS.md

Las secciones van en este orden:

```
# <repo>
North-star: <una línea>

## Comandos
- Tests: `<comando>`

## Reglas
- <una regla por línea>

## Hooks
- SessionStart: `bd prime --hook-json`
- pre-commit (git): <qué corre>

## Mapa
- Antes de <tarea>: leé `docs/agents/contexto.md#<tema>`.

<!-- BEGIN BEADS INTEGRATION ... -->
<!-- END BEADS INTEGRATION -->

<!-- BEGIN SDLC bd-propias v:1 hash:<h> -->
## Reglas propias de bd
- <reglas que mandan sobre el bloque de bd>
<!-- END SDLC bd-propias -->

## SDLC
- <puntero que escribe `python -m sdlc init`>
```

## Forma de CLAUDE.md

```
@AGENTS.md

## Solo Claude
- <ruteo cupo/mmorch, skills, hooks propios de Claude>
```

## Reglas del lint

El lint no cuenta las líneas del bloque de bd.

| Regla | Tope |
|---|---|
| Líneas no vacías de `AGENTS.md` | 100 |
| Líneas no vacías de `CLAUDE.md` | 60 |
| Caracteres por línea | 200 |

- Un párrafo es un error. Un párrafo es una línea que no es título, ítem, tabla, código, marcador, `@import` ni `North-star:`.
- `AGENTS.md` empieza con `# <repo>` y tiene una línea `North-star:`.
- `AGENTS.md` tiene `## Comandos`, `## Reglas`, `## Hooks` y `## Mapa`, en ese orden.
- `## Hooks` nombra cada comando de hook de `.claude/settings.json` y cada hook git instalado.
- `CLAUDE.md` tiene la línea `@AGENTS.md`.

## Prosa

- La prosa sale de los md de agentes.
- La prosa general va a `docs/agents/contexto.md` y las trampas conocidas van a `docs/agents/gotchas.md`.
- Ningún `@import` carga esos archivos, porque un `@import` siempre carga al inicio de la sesión.
- Cada ítem de `## Mapa` lleva un disparador: "antes de X, leé Y".
- La prosa de un área de código vive en `.claude/rules/<área>.md` con `paths:`.
- Claude carga esa regla sola cuando lee o edita un archivo del área.
- `## Mapa` apunta también a esa regla, así otros agentes la leen.

## Reglas y hooks de bd

`python -m sdlc bd <repo>` agrega las reglas y los hooks de bd por append:

1. El comando corre `bd init --skip-agents --skip-hooks --init-if-missing` si falta `.beads/`.
2. bd init commitea si git tiene identidad; el comando deshace ese commit y deja los archivos en stage.
3. El comando crea `AGENTS.md` si falta y agrega `@AGENTS.md` al inicio de `CLAUDE.md` si falta, sin marcas.
4. El comando agrega `## Reglas propias de bd` desde `sdlc/templates/agentes/bd-propias.md` si falta el encabezado.
   El bloque lleva marcas `<!-- BEGIN SDLC bd-propias v:1 hash:<h> -->` y `<!-- END SDLC bd-propias -->`.
5. El comando busca en git el original de cada bloque de bd editado. El original es la revisión cuyo cuerpo da el hash del marcador.
6. Las líneas agregadas o cambiadas respecto del original van a una sección `reintegradas`, byte a byte.
7. Un cambio que solo toca espacios no cuenta como regla propia.
8. El comando devuelve cada bloque editado a su original, y bd lo refresca en el mismo lugar.
9. El comando saca el bloque de bd de `CLAUDE.md`.
10. El comando corre `bd setup claude`, `bd hooks install` y `bd setup claude --check`.

Garantías:

- Los bytes fuera de los bloques de bd quedan intactos.
- Una segunda corrida no cambia ningún byte.
- Un bloque editado sin original en git corta la corrida con código 2, antes de escribir.
- bd administra sus propios hooks: comandos `bd ...` en `settings.json` y secciones con marcadores en `.git/hooks`.
- Todo hook que no es de bd queda igual. Si bd lo cambia, el comando restaura `settings.json` y sale con código 1.
- bd tiene el hash del marcador como `sha256(cuerpo.strip())[:8]`. `bd setup claude --check` mira solo ese hash.
- Los bloques del SDLC usan el mismo formato y el mismo hash que bd: `<!-- BEGIN SDLC <nombre> v:1 hash:<h> -->`.
- Las líneas reintegradas no llevan marcas, porque son texto del usuario.
- El hash de una marca usa el interior con saltos LF. La escritura conserva el salto del archivo, LF o CRLF.
- Un `BEGIN` sin `END` o un nombre de bloque repetido corta la corrida con código 2, antes de escribir.

## Fuera de alcance

- El bloque de `bd setup codex` no entra.
- El lint no corre en pre-commit ni en los gates del SDLC hasta migrar los repos.
- La migración de la prosa de cada repo pertenece a `orchestration-4u6`. Es una sesión asistida con OK del usuario.
