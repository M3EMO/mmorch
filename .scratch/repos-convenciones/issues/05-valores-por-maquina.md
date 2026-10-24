# Valores que dependen de la máquina
Type: grilling
Status: open
Blocked by:
Map: ../map.md

## Question

¿Dónde viven los valores de `sdlc.toml` que solo valen en una máquina?

El `sdlc.toml` de orchestration trae `--basetemp C:/Users/map12/AppData/Local/Temp/pyt-sdlc-wt` en `suite`. Esa ruta es del escritorio Windows y no existe en la laptop Fedora. La clave `python` también puede nombrar un intérprete local. El usuario trabaja en dos máquinas desde 2026-10-08.

Opciones a decidir:

- A. Archivo local ignorado: `sdlc.local.toml` pisa claves de `sdlc.toml` y queda en `.gitignore`.
- B. Variables: `sdlc.toml` acepta `{tmp}` u otras variables que el pipeline resuelve por máquina.
- C. Prohibición: el oráculo de conformidad marca como desvío cualquier ruta absoluta en `sdlc.toml`.
- D. Nada: cada máquina corrige su copia y el mecanismo no mira esos valores.

## Test de aceptación propuesto

`tests/test_sdlc_convenciones.py::test_sin_rutas_de_maquina`: un `sdlc.toml` con `--basetemp C:/Users/x/...` falla el oráculo con el nombre de la clave. Con la opción elegida aplicada, el mismo repo pasa el oráculo y `_cfg()` devuelve una `suite` válida en la máquina del test.
