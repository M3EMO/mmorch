# Brief de la Tarea 141 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-136`. Trae las Tareas 84 a 136 y la 127b.
- **Arrancá cuando la serie 138 termine de medir.** El orquestador lo dice.
- La 139 y la 140 se construyen a la vez. La 139 toca `src/oracle/contenedor.rs`, `src/oracle/descarga.rs` y `src/oracle/calibra/target.rs`. La 140 toca `tools/colab/` y `tests/colab_preparar.rs`. No toques esos archivos.
- La 137 y la 138 tocan `src/oracle/mod.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra/sonda_lastre.rs`, los tests de `defectos.rs` y `src/oracle/eficiencia.rs`. No toques esos archivos. Podés llamar a sus funciones públicas, como `calibra::huella_de`, `calibra::agregar` y `calibra::tiene_items_de_juez`.
- **Constantes de la 139.** Esta tarea usa el nombre y el valor de `ROOT_CONTENEDOR_MODO`, `ROOT_BWRAP_ENTORNO` y `ROOT_BWRAP_IMAGEN_ID`. La 139 los define en `contenedor.rs`. Mientras tanto, definí las tuyas en tu módulo, con el mismo texto y un comentario que nombre la 139. Al encadenar, el orquestador deja una sola definición.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-141`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el paquete de ida | `src/colab_calibra.rs` (nuevo), `src/lib.rs` (la línea `mod` y el despacho del comando), `src/cli.rs` |
| 2, la importación con el control de la base | `src/colab_calibra.rs`, `src/cli.rs`, `src/lib.rs` |
| 3, los runtimes a la vez | `src/colab_calibra.rs`, `src/oracle/colab.rs`, `src/cli.rs`, `src/lib.rs` |

- Los puntos van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **`src/oracle/colab.rs`:** podés abrir funciones de la CLI a `pub(crate)`, y cambiar `activo.json` para que guarde una lista de sesiones. Nada más.
- **No toques:** los archivos de la 137, la 138, la 139 y la 140, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema** de `Fila` ni de `Veredicto`. El manifiesto es un tipo nuevo, en tu módulo.
- **Sin dependencias nuevas** en `Cargo.toml`. Para el paquete, usá `tar` del sistema por un vector de argumentos, como `colab.rs`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a la CLI `colab` real ni a podman real.** Los tests usan el falso de `ROOT_COLAB_BIN`, y un podman falso por una variable, como la auditoría de la Tarea 118.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Las decisiones del usuario, literales:** "C. Colab a) Mover las mediciones a Colab, dentro de 50 unidades", y P110 "b) Bubblewrap".

**Lo que ya se sabe de Colab, por las pruebas del orquestador:**
- Un runtime de CPU es Ubuntu 24.04, con 2 núcleos y 12 GB. Cuesta 0,08 unidades por hora.
- **Varios runtimes a la vez andan.** Con dos sesiones, `colab usage` dio "Active assignments: 2".
- Ningún contenedor anidado corre. bubblewrap sí, con la Tarea 139.
- **`colab exec` no lee Colab Secrets.** Por eso ninguna calibración con juez va a Colab.
- **Sin cgroup no hay `memory.peak`.** Por eso Eficiencia no va a Colab.

**Lo que ya existe.** `src/oracle/colab.rs` corre los tests pendientes en una T4. Usa la CLI `colab` con `new`, `upload`, `exec`, `download` y `stop`, por vectores de argumentos. Anota una sesión en `$ROOT_HOME/colab/activo.json`, la sondea, y libera el runtime al terminar, con una señal o con un tope.

**Por qué el binario de Root se compila dentro de `docker.io/library/rust:1`.** El de la laptop sale de Fedora 44, con una glibc más nueva que la de Ubuntu 24.04. La imagen `rust:1` es Debian bookworm, con glibc 2.36. Su binario corre en Colab.

**Por qué las herramientas salen de la imagen.** Así son los mismos binarios que usa la laptop. La Tarea 140 las instala en el runtime y compara su SHA-256.

**Por qué hace falta un control de la base.** El runtime tiene otro sistema, otro compilador de C y 2 núcleos en vez de 8. Un test o un mutante puede vencer su plazo, y un mutante vencido cuenta como muerto. Si eso cambia la base, la fila no mide lo mismo que en la laptop.

## Tarea

La regla que manda: **una calibración de oráculo corre en Colab, en varios runtimes a la vez, y vuelve al `ROOT_HOME` solo si su versión y su base coinciden con las de la laptop.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El paquete de ida

1. **Un comando nuevo:** `r00t colab empaquetar --binario <ruta> --trabajos <tarea>:<dimensión>,... --repo <tarea>=<ruta> --salida <archivo>`. Podés ajustar los nombres de los flags. Anotalos en el informe.
2. **Solo dimensiones de oráculo:** Seguridad, Funciona, Completitud y Pruebas. Eficiencia y toda dimensión con ítems del juez se rechazan, con su motivo: "Eficiencia necesita el cgroup del contenedor", o "el juez necesita Colab Secrets, que `exec` no lee".
3. **Una tarea cuyo contrato u oculto usa pytest** se rechaza, con el motivo "Colab usa otro Python". Las tareas de S2 usan nextest.
4. **Lo que saca de la imagen, en la laptop.** Con podman, de la imagen de la tarea (`tarea.imagen`):
   - los cinco binarios de `/usr/local/bin`: `cargo-audit`, `cargo-mutants`, `cargo-nextest`, `gitleaks` y `hadolint`;
   - la versión de `rustc`;
   - el entorno de la imagen, con `podman image inspect`;
   - el id de la imagen.
5. **El paquete lleva:**
   - el binario `r00t` que se le pasa. El comando revisa que sea un ELF de x86-64;
   - `herramientas/` con los cinco binarios;
   - `versiones.env`, con el formato de la Tarea 140;
   - `entorno.env`, con una línea `CLAVE=valor` por variable de la imagen, sin claves ni tokens;
   - `tools/colab/` del repo de Root;
   - de `ROOT_HOME`: `oraculos/<tarea>/`, la caché de dependencias de cada tarea, `cache/rustsec/`, `calibracion.json`, `configs/` y `root.toml`;
   - el repo bueno de cada tarea;
   - el manifiesto.
6. **El paquete nunca lleva:** `eventos.jsonl`, `runs.jsonl`, `transcripts/`, `evidencia/`, `colab/`, ni ningún archivo o variable con nombre de clave o token. La lista de exclusiones vive en una constante, con su motivo.
7. **El manifiesto lleva:** el commit de Root, el SHA-256 del binario y de cada archivo, la huella de cada dimensión (`calibra::huella_de`), el `tarea_hash` de cada tarea, el id de la imagen, y la lista de trabajos.
8. **El comando de cada trabajo** corre `r00t oraculo calibrar --json`, con `ROOT_CONTENEDOR_MODO=bwrap`, `ROOT_BWRAP_ENTORNO` apuntando a `entorno.env`, y `ROOT_BWRAP_IMAGEN_ID` con el id de la imagen.
9. **`root.toml`** se revisa antes de entrar. Si trae un campo con nombre de clave o token con valor, el comando falla y lo nombra, sin imprimir el valor.
10. **Las funciones que arman la lista de archivos y el manifiesto son puras.** El empaquetado con `tar` y las llamadas a podman son aparte.

**Tests en rojo primero, con un `ROOT_HOME` falso y un podman falso:**
- Unitario: el paquete trae `oraculos/<tarea>/`, la caché, `cache/rustsec/`, `calibracion.json`, el binario y las cinco herramientas.
- Unitario: el paquete no trae `eventos.jsonl`, `runs.jsonl` ni `transcripts/`.
- Unitario: un trabajo de Eficiencia y uno de Diseño se rechazan, cada uno con su motivo.
- Unitario: un `root.toml` con `api_key = "x"` hace fallar el comando, y el mensaje no contiene `x`.
- Unitario: una variable de la imagen con nombre de token no entra a `entorno.env`.
- Unitario: el manifiesto trae el SHA-256 correcto de un archivo conocido.

### 2. La importación con el control de la base

1. **Un comando nuevo:** `r00t colab importar <archivo>`.
2. **Revisa el manifiesto de vuelta** antes de escribir nada:
   - el SHA-256 de cada archivo coincide;
   - el commit de Root coincide con el de la laptop;
   - la huella y el `tarea_hash` de cada fila nueva coinciden con los de la laptop;
   - la verificación de la Tarea 140 dio todo `ok`. El paquete de vuelta trae su salida.
3. **El control de la base.** Para cada fila nueva, busca la última fila de la laptop con la misma tarea, dimensión y huella.
   - Compara el `valor_base` de los renglones de casos de las dos filas.
   - En Pruebas, compara también el estado de cada mutante de la muestra de la base, si ese dato está guardado. Si no lo está, compara solo el valor, y lo anota.
   - **Si difieren,** la fila no entra, con el motivo "la base de Colab difiere de la base de la laptop", y los dos valores.
   - **Sin fila de la laptop para comparar,** la fila no entra, con el motivo "sin base de la laptop para comparar".
4. **Un archivo que no coincide frena todo.** Una fila que no pasa no entra, y el comando la nombra con su motivo. Las demás filas entran.
5. **Antes de escribir,** guarda un respaldo de `calibracion.json` y de `calibracion/` en `ROOT_HOME/colab/respaldos/<fecha>/`. Usá `cp --reflink=auto` por un vector de argumentos.
6. **Agrega las filas** con `calibra::agregar`. No dupliques una fila que ya está: misma tarea, dimensión y `ts`.
7. **Copia los `.casos.jsonl` nuevos** a `ROOT_HOME/calibracion/`. Un nombre que ya existe no se pisa.
8. **El manifiesto de vuelta dice dónde se midió** cada fila: `colab`, con el tipo de runtime. La fila no cambia.
9. **Al final, imprime** cuántas filas entraron, cuántas no y por qué, y cuántos casos se copiaron.

**Tests en rojo primero:**
- Unitario: un paquete de vuelta con una fila válida, cuya base coincide con la de la laptop, la agrega y deja el respaldo.
- Unitario: una fila con otro `valor_base` que la de la laptop no entra, con su motivo y los dos valores.
- Unitario: una fila sin fila de la laptop para comparar no entra.
- Unitario: una verificación con una línea `mal` frena la importación sin escribir nada.
- Unitario: un archivo con un SHA-256 distinto frena la importación sin escribir nada.
- Unitario: importar dos veces el mismo paquete no duplica filas.

### 3. Los runtimes a la vez

1. **Un comando nuevo:** `r00t colab calibrar --paquete <archivo> [--paquete <archivo> ...]`. Cada paquete va a un runtime de CPU propio, y todos corren a la vez.
2. **Un trabajo no se parte entre runtimes.** Cada paquete trae sus trabajos, y su runtime los corre en serie.
3. **El ciclo de cada runtime:** pide el runtime de CPU con `new`; sube el paquete; corre `tools/colab/preparar-runtime.sh`; corre los trabajos del manifiesto; arma el paquete de vuelta con la salida de la verificación, las filas nuevas, los casos nuevos y su manifiesto; lo baja; y libera el runtime con `stop`.
4. **Si la CLI necesita un valor explícito para la CPU,** usá ese valor y anotalo en el informe. Ningún runtime pide una GPU.
5. **`activo.json` guarda una lista de sesiones.** Lee también el formato viejo, de una sola. `oraculo pendientes --vigilar` y `--liberar` recorren la lista.
6. **Cada runtime se libera siempre:** al terminar, con un error, con `SIGINT` o `SIGTERM`, y al pasar un tope. El tope por defecto es de 4 horas por runtime, en una constante con su motivo.
7. **Un runtime que falla no frena a los otros.** Su paquete de vuelta no llega, el comando lo dice, y los demás siguen.
8. **Al bajar cada paquete de vuelta,** el comando llama a la importación del punto 2, de a uno.
9. **El comando nunca escribe en el `ROOT_HOME` real** si una importación falla.

**Tests en rojo primero, con el falso de `ROOT_COLAB_BIN`:**
- Unitario: dos paquetes dan dos sesiones, cada una con `new`, `upload`, `exec`, `download` y `stop`, con vectores de argumentos.
- Unitario: ningún `new` lleva `--gpu`, o lleva el valor de CPU.
- Unitario: un `exec` que falla en una sesión libera esa sesión, y la otra termina igual.
- Unitario: `activo.json` lista las dos sesiones durante el ciclo, y no existe al terminar.
- Unitario: un `activo.json` viejo, de una sola sesión, se lee.
- Unitario: ningún argumento pasado a la CLI contiene el valor de una variable de entorno con nombre de clave.

No corras la validez, `corre`, `calibrar` ni la pasada reales. No llames a la CLI `colab` real ni a podman real. Los corre el orquestador.

## Lo que queda sin probar

El builder no usa Colab ni podman. Quedan para el orquestador, después de las pruebas de la 140 y la 139:
- **Un paquete de Seguridad de s2-t1 en un runtime,** y su importación a una copia del `ROOT_HOME` de la laptop.
- **Las doce calibraciones de oráculo en cuatro runtimes,** con el reparto de la vuelta (53), sección 5. Cuántas asignaciones da `colab usage`, y las unidades que gastó todo.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Claves.** Ninguna clave va en el paquete, en un argumento ni en un log.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Si `r00t esquema` o `docs/interfaces.md` listan los comandos, regeneralos.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-141.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los comandos nuevos**: el texto de `--help` de cada uno, y un ejemplo de manifiesto en JSON.
5. **El control de la base**: qué se compara, de qué campos sale, y qué falta guardar para comparar más.
6. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
7. **Llamadores revisados**: por cada función de `colab.rs` que abriste o cambiaste, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
