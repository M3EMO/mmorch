# Orden de adopción y forma de aplicar en cada repo
Type: grilling
Status: open
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
