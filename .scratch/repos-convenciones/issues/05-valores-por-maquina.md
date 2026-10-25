# Valores que dependen de la máquina
Type: grilling
Status: resolved
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

## Answer

Resuelto por grilling con el usuario (2026-10-23). Registro completo: `brainstorms/2026-10-23-repos-convenciones-ticket-05.md`.

- La meta es no tener rutas absolutas en `sdlc.toml`. `doctor` marca como desvío cada ruta absoluta.
- Las variables como `{tmp}` (opción B) quedan como respaldo, solo si aparece un caso que el desvío no resuelva.
- El único caso real es el `--basetemp` de Windows en la `suite` de orchestration. Ese valor congeló la expansión Windows del default portátil de `build_feature` (`fd33b1b`).
- El arreglo de orchestration es borrar la clave `suite` a mano. `--upgrade` no borra contenido.
- Una ruta absoluta es un pedazo que empieza con letra de unidad (cualquier caso), con `/`, con `~` o con `\\`.
- `doctor` revisa cada texto de `sdlc.toml`, también dentro de listas y tablas. Parte los comandos de texto con `shlex.split`, porque esos comandos corren con shell.
- `doctor` parte en `=` solo los pedazos que empiezan con `-`, así que las URLs con parámetros no dan desvío.

### Test de aceptación aprobado

`tests/test_sdlc_convenciones.py::test_sin_rutas_de_maquina` verifica once puntos:

1. Un `sdlc.toml` con `--basetemp C:/Users/x/...` en `suite` falla `doctor` con un desvío que nombra la clave.
2. Una ruta que empieza con `/` o con `~` también da desvío.
3. Una ruta dentro de un comando, como `compile_cmd = "npx tsc -p /abs/x.json"`, da desvío.
4. Un argumento `--basetemp=C:/x` escrito junto da desvío.
5. Los globs relativos, las imágenes de `contenedor` y las URLs no dan desvío.
6. Sin la clave `suite`, `_cfg()` y `build_feature` usan el `--basetemp` de `tempfile.gettempdir()`.
7. `--upgrade` no borra ni cambia una clave con ruta absoluta, y el reporte la nombra.
8. `doctor` parte cada comando de texto con `shlex.split`, así que una ruta entre comillas también da desvío.
9. `doctor` parte en `=` solo los pedazos que empiezan con `-`, así que una URL con `?a=/b` no da desvío.
10. Una letra de unidad en minúscula, como `c:/x`, también da desvío.
11. Una ruta de red de Windows, que empieza con `\\`, también da desvío.
