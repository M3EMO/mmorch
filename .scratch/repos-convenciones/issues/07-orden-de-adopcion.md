# Orden de adopción y forma de aplicar en cada repo
Type: grilling
Status: resolved
Blocked by: 02, 03, 04
Map: ../map.md

## Question

¿En qué orden se actualizan los repos, y cómo llega el cambio a cada uno?

Estado de partida (sonda 2026-10-23): Portfolio, ChatBot, ssb y mf-moe tienen el árbol limpio. Estudio tiene 11 archivos sucios en la rama `init`. orchestration tiene 27 archivos sucios en `mmorch/auto`. Los prototipos F2 tienen 1 archivo sucio cada uno.

Opciones de orden a decidir:

- A. Piloto en orchestration primero, porque es el dueño del mecanismo; después los repos con árbol limpio.
- B. Piloto en un repo chico y limpio (mf-moe o ssb); orchestration al final.
- C. Todos a la vez con modo de prueba, y el usuario aprueba repo por repo.

Preguntas de forma a decidir:

- ¿El primer paso es siempre un modo de prueba que solo reporta?
- ¿Un repo con árbol sucio se salta, se aborta o se actualiza igual?
- ¿El cambio queda en una rama nueva del repo destino, o en la rama actual?
- ¿Quién commitea: el mecanismo con un mensaje fijo, o el usuario? El push queda siempre del lado del usuario.
- ¿`--upgrade --todos` escribe en todos los repos de una vez, o solo `doctor --todos` corre en lote? (ticket 02)
- ¿Cómo entra Adepor? El ticket 02 decidió adoptarlo con `init` dentro de este ticket. Falta una sonda que verifique que `tests/` corre sin red (`tests/test_ingesta_cuotas.py` usa `requests`).
- La laptop es la única máquina que corre `--upgrade` (ticket 02).
- ¿Qué cuenta como "adoptado": el oráculo verde, o además una corrida real del pipeline?

## Test de aceptación propuesto

Por repo, un registro en este ticket con tres comandos y su salida: el oráculo antes (desvíos listados), la aplicación, y el oráculo después (cero desvíos). `tests/test_sdlc_convenciones.py::test_arbol_sucio`: sobre un repo de `tmp_path` con un archivo modificado sin commitear, el mecanismo sigue la regla elegida y nunca toca ese archivo.

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-repos-convenciones-ticket-07.md`.

- `--todos` existe solo en `doctor`. `--upgrade` actúa sobre un repo por invocación.
- `--upgrade` aborta solo si un archivo que va a tocar está sucio.
- `--upgrade` commitea solo los archivos que tocó (`git commit -- <rutas>`) y pushea siempre.
- Para un agente, correr `--upgrade` equivale a pushear, así que pide OK antes (guardrail del repo: push en tier ASK).
- Casos de falla: sin remoto, el commit queda local con aviso; push rechazado, el commit queda y el reporte lo dice; pre-commit que falla, los cambios quedan sin commitear y el reporte lo dice.
- `--upgrade` commitea solo en la rama default, y aborta en otra rama o con HEAD separado.
- La rama default sale de la clave `rama` de `sdlc.toml`. Sin la clave, sale de `origin/HEAD`, o de la única rama local `main` o `master`. Si nada aplica, `--upgrade` aborta.
- `rama` es una fila nueva de la tabla del ticket 01, con default en código, así que `--upgrade` nunca la escribe. orchestration necesita `rama = "mmorch/auto"` escrita a mano.
- Adoptado = `doctor` sale con "sin desvíos" o "solo avisos". Adepor además necesita `accept_cmd` verde y sin red antes del `init`.

### Orden de adopción

1. mf-moe, como piloto y control negativo real.
2. Portfolio, ssb y ChatBot.
3. Los prototipos F2 y f2-sueldo-neto.
4. Estudio.
5. orchestration, después de escribir `rama`.
6. Adepor, con sonda sin red, `init` y checkout de `main`.

Cada repo deja abajo, en `## Comments`, tres salidas: `doctor` antes, `--upgrade` y `doctor` después.

### Test de aceptación aprobado

`tests/test_sdlc_convenciones.py::test_aplicar_en_un_repo` arma en `tmp_path` un repo con un remoto bare y verifica dieciséis puntos:

1. Un archivo sucio que `--upgrade` va a tocar aborta la corrida, sin escrituras y sin commit.
2. Un archivo sucio que `--upgrade` no toca sigue sucio y con los mismos bytes.
3. El commit contiene solo las rutas que tocó `--upgrade`, según `git show --name-only`.
4. Tras el push, la rama del remoto bare apunta al commit nuevo.
5. Un repo sin remoto conserva el commit local, y el reporte da un aviso.
6. Un push rechazado conserva el commit local, y el reporte lo dice.
7. Un hook de pre-commit que falla deja los cambios sin commitear, y el reporte lo dice.
8. En una rama que no es la default o con HEAD separado, `--upgrade` aborta sin escrituras y nombra la rama esperada.
9. La clave `rama` de `sdlc.toml` gana sobre `origin/HEAD`.
10. Sin `origin/HEAD`, la única rama local `main` es la default, y tener `main` y `master` aborta.
11. `--upgrade --todos` falla con un mensaje, porque `--todos` solo existe en `doctor`.
12. Un archivo que el usuario dejó staged sigue staged y fuera del commit de `--upgrade`.
13. Un repo sin arreglos pendientes no recibe un commit vacío ni un push.
14. El mensaje del commit sigue un formato fijo y nombra cada arreglo aplicado.
15. El push nunca usa `--force`, y un push rechazado nunca se reintenta con fuerza.
16. El push corre con `GIT_TERMINAL_PROMPT=0` y un timeout, y un timeout conserva el commit local con aviso.

## Comments

### Adopción (2026-10-24)

Cada repo corrió `doctor` antes, `init --upgrade` y `doctor` después, desde la rama `claude/youthful-bohr-f13d78`.
El OK del usuario cubre el push de cada repo. Salidas completas en el scratchpad de la sesión.

- mf-moe: antes exit 1 (claude-import); upgrade claude-import, commit `0d4b6cd`, push sin remoto: el commit queda local; después exit 3 (sin desvíos).
- Portfolio-financiero: antes exit 1 (agents, beads, claude-import, gitignore, plantillas); upgrade agents, claude-import, gitignore, plantillas, commit `35d840a`, push ok; después exit 1 (beads).
- ssb: antes exit 1 (agents, beads, gitignore); upgrade agents, gitignore, commit `81aba51`, push ok; después exit 1 (beads).
- ChatBot: antes exit 1 (agents, beads, gitignore); upgrade agents, gitignore, commit `b6d1994`, push ok; después exit 1 (beads).
- prototipo-04: antes exit 1 (agents, beads, gitignore); upgrade agents, gitignore, commit `3b3723e`, push sin remoto: el commit queda local; después exit 1 (beads).
- prototipo-06: antes exit 1 (agents, beads, gitignore); upgrade agents, gitignore, commit `e48bd0c`, push sin remoto: el commit queda local; después exit 1 (beads).
- prototipo-10: antes exit 1 (agents, beads, gitignore); upgrade agents, gitignore, commit `53bc60b`, push sin remoto: el commit queda local; después exit 1 (beads).
- f2-sueldo-neto: antes exit 1 (agents, beads, gitignore); upgrade agents, gitignore, commit `00c4ed0`, push sin remoto: el commit queda local; después exit 1 (beads).
- Estudio: antes exit 1 (agents, beads, claude-import, gitignore, plantillas); upgrade agents, claude-import, gitignore, plantillas, commit `e8a8f8b`, push ok; después exit 1 (beads).
- Adepor (`~/BetDeportivo`): sonda en un worktree de `main`, 161 tests verdes sin red (`unshare -rn`, 8,7 s); `init` commiteado en `bb56c10`; antes exit 1 (agents, claude-import, gitignore); upgrade agents, claude-import, gitignore, commit `1277290`, push ok; después exit 3 (sin desvíos). El checkout del usuario en `audit/prod-ready-2026-10` no se tocó.
- orchestration: `sdlc.toml` sin la `suite` de Windows y con `rama = "mmorch/auto"` (`da69624`). `origin/mmorch/auto` sí existía (el primer chequeo miró refs locales sin `fetch`). Antes exit 1 (gitignore, plantillas); upgrade commit `c2b6365`, push ok; después exit 3. El push llevó también el merge de `orchestration-ior` (`405fa6d`, `31cb569`) y `db223d0`, que otra sesión había hecho en `mmorch/auto`.
- `.beads/` (decisión del usuario: "solo el .beads"): `bd init --non-interactive --role maintainer --skip-agents --skip-hooks --init-if-missing` en Portfolio (`6092b8c`), ssb (`fe203b0`), ChatBot (`fcfccef`), prototipo-04 (`248aa2b`), prototipo-06 (`e102e5b`), prototipo-10 (`81e2cbf`), f2-sueldo-neto (`ea1d67e`) y Estudio (`5eb3a9e`). `bd init` commitea solo `.beads/` y siete líneas al final de `.gitignore`; los archivos sucios del usuario quedaron iguales. Push en los cuatro con remoto.
- Resultado: `doctor --todos /home/mpera` da cero desvíos en los nueve repos que encuentra (exit 3, solo avisos).
- Techo de `init`: corregido en `_globs_iniciales()` (archivos sueltos por nombre exacto, `test_init_techo_sin_comodin_en_la_raiz`). Adepor recibió el techo corregido en `2e068d9` (push ok). Ningún otro `sdlc.toml` tenía un comodín en la raíz.
- Límite de `--todos`: no encuentra orchestration, porque vive bajo `~/.claude` (carpeta con punto), ni Adepor, cuyo checkout está en `audit/prod-ready-2026-10` sin `sdlc.toml` (el archivo vive en `main`).
