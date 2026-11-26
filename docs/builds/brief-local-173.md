# Brief de la Tarea 173 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vueltas (98) y (99).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde la punta de `laptop/cadena-172`.** Es una rama de integración: 5a4f295, más la mezcla local de `laptop/tarea-171` y `laptop/tarea-172`, sin conflictos. La arma el orquestador. No está en `claude/root-e7813e`.
- Trabajá en una rama nueva: `laptop/tarea-173`.
- **Esta tarea se construye en Colab,** en un runtime high-mem, como el usuario `builder`, sin uid 0. El hook de pre-commit corre la suite entera.
- **No uses `--no-verify` ni saltees tests.** Si un test falla sobre la base, detenete y anotalo en el informe.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Por qué

**Las Tareas 171 y 172 dejan seis huecos que frenan la prueba de aislamiento o el piloto de D3.** Salen de sus informes y de la revisión del planner:

1. **El oráculo no puede correr en el runtime de `corre`.** `preparar-corre.sh` llama a `preparar-runtime.sh`, y ese guion exige `versiones.env` y `herramientas/`. El paquete de `corre` no los trae. Además, `corre-runtime.py` no fija `ROOT_BWRAP_ENTORNO` ni `ROOT_BWRAP_IMAGEN_ID`, y el oráculo con bwrap los necesita.
2. **El token no llega como decidió el usuario.** P130 a dice que el token sube por `colab upload`, como un archivo. `corre-runtime.py` lo espera en la variable `ROOT_TOKEN_CUPO`.
3. **Con token, el builder sigue pidiendo `.credentials.json`.** En Colab ese archivo no existe.
4. **`claude` puede quedar fuera del `PATH` del builder.** El `PATH` fijo es `/usr/local/bin:/usr/bin:/bin`, y bwrap solo monta `/usr`, `/etc`, `/opt` y sus enlaces. Si `npm` instala `claude` o `node` en otra carpeta, el builder no los encuentra.
5. **La red del builder no corta IPv6.** Las reglas de `red-builder.sh` son de IPv4. Además, los logs de `slirp4netns` van a `/dev/null`.
6. **Un reintento de `colab importar --corre` puede duplicar una corrida.** Si la copia de los transcripts falla después de agregar la fila a `runs.jsonl`, el reintento agrega la fila otra vez. Una tarea contaría dos veces.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el oráculo en el paquete de `corre` | `src/colab_corre.rs`, `src/colab_calibra.rs` (solo para hacer pública y reusar la parte que arma `herramientas/`, `versiones.env` y `entorno.env`), `src/cli.rs`, `tools/colab/corre-runtime.py` |
| 2, el token por archivo | `tools/colab/corre-runtime.py` |
| 3, sin `.credentials.json` con token | `src/corre/bwrap.rs`, `src/corre/ejecutor.rs` |
| 4, `claude` y `node` bajo `/usr` | `tools/colab/preparar-corre.sh` |
| 5, IPv6 y los logs de la red | `tools/colab/red-builder.sh`, `tests/red_builder.rs` |
| 6, la importación sin duplicados | `src/colab_corre.rs` |

- Los puntos 1 y 6 comparten `src/colab_corre.rs`, y los puntos 1 y 2 comparten `corre-runtime.py`. Van en serie: 1, 2, 6. Los puntos 3, 4 y 5 no comparten archivos con nadie.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/`, `src/tipos.rs`, el resto de `src/`, el resto de `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Ninguna huella cambia.** Esta tarea no toca lo que mide el oráculo.
- **En `src/colab_calibra.rs`, el paquete de calibración no cambia.** Un test lo guarda.
- **Sin cambio de esquema,** y **sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- **No llames a podman real, a la CLI `colab` real, a una API real ni a `claude` real.** Los tests usan programas falsos y la `Imagen` falsa de la calibración.
- **Ningún test crea un espacio de red ni corre `iptables`, `ip6tables` o `sysctl` reales.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`.

## Tarea

La regla que manda: **el runtime de `corre` tiene el mismo oráculo que el de calibración; el token llega como un archivo; y una tarea nunca cuenta dos veces.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El oráculo en el paquete de `corre`

1. **`colab empaquetar --corre` suma `--rustup-sha256 <sha>`,** obligatorio, como en la calibración.
2. **El paquete de `corre` trae la misma parte de oráculo que un paquete de calibración:** `herramientas/`, `versiones.env`, `entorno.env` y el id de la imagen `localhost/root-oraculo-rust:4`.
3. **Esa parte sale de la misma función que usa la calibración.** No se copia el código. Si hace falta, hacé pública esa parte de `armar_y_empaquetar` en `src/colab_calibra.rs`, sin cambiar lo que hace.
4. **El manifiesto de `corre` trae el id de la imagen.**
5. **`corre-runtime.py` fija, para el `r00t` hijo:** `ROOT_BWRAP_ENTORNO` con la ruta de `entorno.env` del paquete, y `ROOT_BWRAP_IMAGEN_ID` con el id del manifiesto. Son los mismos valores que da `comando_de_trabajo` en la calibración.

**Tests en rojo primero:**
- Unitario: el paquete de `corre` trae `herramientas/`, `versiones.env` y `entorno.env`. Hoy no los trae.
- Unitario: el manifiesto de `corre` trae el id de la imagen.
- Unitario: `--corre` sin `--rustup-sha256` da un error de uso.
- Unitario de Python: el entorno del `r00t` hijo trae `ROOT_BWRAP_ENTORNO` y `ROOT_BWRAP_IMAGEN_ID`, con los valores del paquete.
- Guarda: un paquete de calibración armado con las mismas entradas da los mismos archivos y los mismos SHA que en la base.

### 2. El token por archivo

1. **`corre-runtime.py` lee el token de un archivo,** no de una variable. La ruta sale de `ROOT_TOKEN_ARCHIVO`. Su valor por defecto es `/content/root-token`.
2. **Mueve el token a `/run/root-token`,** con dueño root y modo 0600, y borra el archivo original.
3. **`ROOT_TOKEN_CUPO` deja de existir.** Si el entorno la trae, el guion termina con un error que nombra la variable, nunca su valor.
4. **La clave de DeepSeek no cambia de camino:** sigue en `DEEPSEEK_API_KEY`, que el guion saca de `os.environ` y pasa solo al `r00t` hijo.
5. **Sin el archivo del token,** el guion termina con un error, y no corre ninguna tarea.

**Tests en rojo primero, con rutas temporales del worktree:**
- Unitario de Python: con un archivo de token falso, el guion corre, y el original ya no existe. Hoy el guion pide `ROOT_TOKEN_CUPO`.
- Unitario de Python: el archivo nuevo tiene modo 0600.
- Unitario de Python: con `ROOT_TOKEN_CUPO` en el entorno, el guion frena, y su salida no trae el valor.
- Unitario de Python: sin archivo de token, no corre ninguna tarea.
- Guarda: al terminar, el token ya no existe, como hoy.

### 3. Sin `.credentials.json` con token

1. **Con `ROOT_TOKEN_BUILDER`,** `EjecutorBwrapBuilder` no exige `.credentials.json`, no la copia y no la monta.
2. **Sin `ROOT_TOKEN_BUILDER`,** exige y monta la copia, como hoy.
3. **El chequeo previo suma:** con `ROOT_TOKEN_BUILDER`, el archivo existe, no está vacío y tiene modo 0600 o más cerrado. El error nunca muestra su contenido.

**Tests en rojo primero:**
- Unitario: con `ROOT_TOKEN_BUILDER` y sin `.credentials.json`, el lanzamiento sigue. Hoy frena.
- Unitario: con `ROOT_TOKEN_BUILDER`, los argumentos de bwrap no montan ninguna credencial.
- Unitario: un archivo de token con modo 0644 frena el lanzamiento, y el mensaje no trae su contenido.
- Guarda: sin `ROOT_TOKEN_BUILDER`, el comportamiento es el de la 171.

### 4. `claude` y `node` bajo `/usr`

1. **`preparar-corre.sh` instala `nodejs` y `npm` con apt,** y usa `/usr/bin/npm` con `--prefix /usr/local` para instalar `claude`.
2. **Revisa, después de instalar:**
   - `readlink -f "$(command -v claude)"` queda bajo `/usr/`;
   - `readlink -f "$(command -v node)"` queda bajo `/usr/`;
   - con el `PATH` del builder, `/usr/local/bin:/usr/bin:/bin`, `claude --version` coincide con `versiones-builder.env`.
3. **Si una revisión falla,** el guion termina con un error que nombra la ruta encontrada.

**Tests en rojo primero, con comandos falsos:**
- Unitario: un `claude` falso que resuelve fuera de `/usr/` frena el guion, con su ruta. Hoy no se revisa.
- Unitario: un `node` falso fuera de `/usr/` frena el guion.
- Unitario: el guion llama a `/usr/bin/npm` con `--prefix /usr/local`.

### 5. IPv6 y los logs de la red

1. **Dentro del espacio `root-builder`,** `red-builder.sh` apaga IPv6 con `sysctl -w net.ipv6.conf.all.disable_ipv6=1` y `net.ipv6.conf.default.disable_ipv6=1`.
2. **Además, agrega `ip6tables -P OUTPUT DROP`** dentro del espacio. Si `ip6tables` no existe, el guion termina con un error.
3. **Los logs de `slirp4netns`** van a `/run/root-builder-slirp.log`, con modo 0600.
4. **Las reglas de IPv4 de la 171 no cambian:** 2 de DNS primero, y 6 de corte.

**Tests en rojo primero, con `ip`, `iptables`, `ip6tables` y `sysctl` falsos:**
- Unitario: el guion apaga IPv6 y pone la política de `ip6tables` en `DROP`. Hoy no lo hace.
- Unitario: sin `ip6tables`, el guion frena.
- Unitario: `slirp4netns` escribe en `/run/root-builder-slirp.log`.
- Guarda: los 5 tests de `tests/red_builder.rs` siguen verdes.

### 6. La importación sin duplicados

1. **`colab importar --corre` busca el id de la corrida en `runs.jsonl`** antes de agregar la fila. Si ya está, no la agrega otra vez.
2. **El orden de la escritura:** primero los transcripts, después los eventos, después la fila de `runs.jsonl`, y al final el registro `colab-corre.json`.
3. **Un reintento después de un fallo a mitad de camino** completa lo que falta, y no duplica nada.
4. **Los eventos tampoco se duplican:** un evento con la misma corrida, el mismo `ts` y el mismo tipo no entra dos veces.

**Tests en rojo primero:**
- Unitario: una importación que falla al copiar un transcript, y después se reintenta, deja una sola fila en `runs.jsonl`. Hoy deja dos.
- Unitario: el reintento no duplica los eventos de gasto.
- Unitario: la misma vuelta importada dos veces sigue dando "ya estaba".
- Guarda: los 11 tests `importar_corre_*` de la 172 siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Lo que verifica el orquestador

1. **La suite con podman** de `laptop/cadena-173`, que es la cadena 172 más esta tarea.
2. **`git diff laptop/cadena-172 laptop/tarea-173 --stat`** toca solo los archivos de la tabla y el informe.
3. **Un paquete real de `corre` con 1 tarea,** armado en la laptop. Trae `herramientas/`, `versiones.env` y `entorno.env`. No trae `sk-`, `.env` ni `.credentials.json`.
4. **La prueba de aislamiento en Colab,** con la receta de la sección 3 de la vuelta 99.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave ni token va en una línea impresa, en un argumento ni en un archivo del paquete. Los tests usan valores falsos.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-173.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El entorno del runtime**: cada variable que recibe el `r00t` hijo, y de dónde sale.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
