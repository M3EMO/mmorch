# Brief de la Tarea 139 para el builder local

> **EN PAUSA desde la vuelta (54). No arranques.** La Prueba 0b mostró que ningún contenedor anidado corre en Colab. Este brief corre `r00t` con el modo de la Tarea 140, que no sirve. El planner lo reescribe según la respuesta del usuario a P110.

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`. Reemplaza la versión de la vuelta (52).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-136`. Trae las Tareas 84 a 136 y la 127b.
- Arrancá después de la serie 138, cuando el orquestador lo diga.
- La 137 toca `src/oracle/mod.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra/sonda_lastre.rs` y los tests de `defectos.rs`. La 138 toca `src/oracle/eficiencia.rs`. La 140 toca `src/oracle/contenedor.rs`. No toques esos archivos. Podés llamar a sus funciones públicas, como `calibra::huella_de` y `calibra::agregar`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-139`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el paquete de ida | `src/colab_calibra.rs` (nuevo), `src/lib.rs` (la línea `mod` y el despacho del comando), `src/cli.rs` |
| 2, la importación de vuelta | `src/colab_calibra.rs`, `src/cli.rs`, `src/lib.rs` |
| 3, el ciclo de varios runtimes | `src/colab_calibra.rs`, `src/oracle/colab.rs`, `src/cli.rs`, `src/lib.rs` |

- Los puntos van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **`src/oracle/colab.rs`:** podés abrir funciones de la CLI a `pub(crate)`, y cambiar `activo.json` para que guarde una lista de sesiones (punto 3). Nada más.
- **No toques:** los archivos de la 137, la 138 y la 140, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin cambio de esquema** de `Fila` ni de `Veredicto`. El manifiesto es un tipo nuevo, en tu módulo.
- **Sin dependencias nuevas** en `Cargo.toml`. Para el paquete, usá `tar` del sistema por un vector de argumentos, como `colab.rs`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a la CLI `colab` real.** Los tests usan el falso de `ROOT_COLAB_BIN`, como los de `colab.rs`.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La decisión del usuario, literal, del 2026-11-13:** "C. Colab a) Mover las mediciones a Colab, dentro de 50 unidades". **El 2026-11-14 la recordó:** "acordate de usar colab para temas de root". **Y preguntó:** "No se puede multiples en colab?"

**Lo que dijo la Prueba 0 del orquestador:**
- Un runtime de CPU de Colab es Ubuntu 24.04, con 2 núcleos y 12 GB. Corre como root, con `CAP_SYS_ADMIN` y sin `CAP_NET_ADMIN`.
- **Varios runtimes a la vez andan.** Con dos sesiones, `colab usage` dio "Active assignments: 2". Cada runtime de CPU cuesta 0,08 unidades por hora.
- podman se instala con apt, pero no puede aislar la red ni usar cgroups. La Tarea 140 suma un modo de podman para Colab, sin red aislada y sin cgroups.
- **Un proceso de `colab exec` no lee Colab Secrets.** Por eso, ninguna calibración con juez corre en Colab.
- **Sin cgroup no hay `memory.peak`.** Por eso, Eficiencia no se calibra en Colab.

**Lo que ya existe.** `src/oracle/colab.rs` corre los tests pendientes en una T4 (decisión 22). Usa la CLI `colab` con los verbos `new`, `upload`, `exec`, `download` y `stop`, siempre por vectores de argumentos. Anota una sesión en `$ROOT_HOME/colab/activo.json`, la sondea, y libera el runtime al terminar, con una señal o con un tope. `oraculo pendientes --vigilar` es la red de seguridad.

**Por qué el binario se compila en la laptop, dentro de `docker.io/library/rust:1`.** El binario de la laptop se compila en Fedora 44, con una glibc más nueva que la de Ubuntu 24.04. La imagen `rust:1` es Debian bookworm, con glibc 2.36, y Ubuntu 24.04 trae 2.39. Un binario de esa imagen corre en Colab, y el runtime no gasta tiempo compilando Root.

**Por qué la imagen del oráculo se construye en cada runtime.** `root-oraculo-rust:4` pesa varios GB. La red de Google baja sus partes rápido. Construirla desde `imagenes/oraculo-rust/Containerfile` es más barato que subirla.

## Tarea

La regla que manda: **una calibración de oráculo corre en Colab, en varios runtimes a la vez, sale de la laptop en un paquete verificable, y vuelve al `ROOT_HOME` solo si coincide con la versión de la laptop.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El paquete de ida

1. **Un comando nuevo:** `r00t colab empaquetar --binario <ruta> --trabajos <tarea>:<dimensión>,... --repo <tarea>=<ruta> --salida <archivo>`. Podés ajustar los nombres de los flags. Anotalos en el informe.
2. **Solo dimensiones de oráculo:** Seguridad, Funciona, Completitud y Pruebas. Eficiencia y toda dimensión con ítems del juez se rechazan, con el motivo: "Eficiencia necesita el cgroup del contenedor" o "el juez necesita Colab Secrets, que `exec` no lee". Usá `calibra::tiene_items_de_juez` si es público.
3. **El paquete lleva:**
   - el binario `r00t` que se le pasa. El orquestador lo compila dentro de `docker.io/library/rust:1`. El comando revisa que sea un ELF de x86-64;
   - `imagenes/oraculo-rust/` del repo de Root, para construir la imagen en el runtime;
   - de `ROOT_HOME`: `oraculos/<tarea>/` de cada tarea del paquete, la caché de dependencias de cada tarea, `calibracion.json`, `configs/` y `root.toml`;
   - el repo bueno de cada tarea;
   - un manifiesto en JSON.
4. **El paquete nunca lleva:** `eventos.jsonl`, `runs.jsonl`, `transcripts/`, `evidencia/`, `colab/`, ni ningún archivo que se llame como una clave o un token. La lista de exclusiones vive en una constante, con su motivo.
5. **El manifiesto lleva:** el commit de Root, el SHA-256 del binario, la huella de cada dimensión (`calibra::huella_de`), el `tarea_hash` de cada tarea, el SHA-256 de cada archivo del paquete, y la lista de trabajos con el comando de cada uno.
6. **El comando de cada trabajo** corre `r00t oraculo calibrar` con el modo de contenedor de Colab de la Tarea 140. Tomá el nombre del modo de una constante, y anotalo en el informe.
7. **`root.toml`** se revisa antes de entrar. Si trae un campo con nombre de clave o token con valor, el comando falla y lo nombra, sin imprimir el valor.
8. **Las funciones que arman la lista de archivos y el manifiesto son puras.** El empaquetado con `tar` es aparte.

**Tests en rojo primero, con un `ROOT_HOME` falso en una carpeta temporal:**
- Unitario: el paquete trae `oraculos/<tarea>/`, la caché de la tarea, `calibracion.json` y el binario.
- Unitario: el paquete no trae `eventos.jsonl`, `runs.jsonl` ni `transcripts/`.
- Unitario: un trabajo de Eficiencia y uno de Diseño se rechazan, cada uno con su motivo.
- Unitario: un `root.toml` con `api_key = "x"` hace fallar el comando, y el mensaje no contiene `x`.
- Unitario: el manifiesto trae el SHA-256 correcto de un archivo conocido.

### 2. La importación de vuelta

1. **Un comando nuevo:** `r00t colab importar <archivo>`.
2. **Revisa el manifiesto de vuelta** antes de escribir nada:
   - el SHA-256 de cada archivo coincide;
   - el commit de Root coincide con el de la laptop;
   - la huella y el `tarea_hash` de cada fila nueva coinciden con los que calcula la laptop.
3. **Un archivo que no coincide frena todo.** Una fila que no coincide no entra, y el comando la nombra con su motivo. Las demás filas entran.
4. **Antes de escribir,** guarda un respaldo de `calibracion.json` y de `calibracion/` en `ROOT_HOME/colab/respaldos/<fecha>/`. Usá `cp --reflink=auto` por un vector de argumentos.
5. **Agrega las filas** con `calibra::agregar`. No dupliques una fila que ya está: misma tarea, dimensión y `ts`.
6. **Copia los `.casos.jsonl` nuevos** a `ROOT_HOME/calibracion/`. Un nombre que ya existe no se pisa.
7. **El manifiesto de vuelta dice dónde se midió** cada fila: `colab`, con el tipo de runtime. La fila no cambia.
8. **Al final, imprime** cuántas filas entraron, cuántas no y por qué, y cuántos casos se copiaron.

**Tests en rojo primero:**
- Unitario: un paquete de vuelta con una fila válida la agrega a `calibracion.json`, y deja el respaldo.
- Unitario: una fila con otra huella no entra, y el comando la nombra.
- Unitario: un archivo con un SHA-256 distinto frena la importación sin escribir nada.
- Unitario: importar dos veces el mismo paquete no duplica filas.
- Unitario: dos paquetes con filas de tareas distintas se importan uno después del otro, y quedan todas.

### 3. El ciclo de varios runtimes

1. **Un comando nuevo:** `r00t colab calibrar --paquete <archivo> [--paquete <archivo> ...]`. Cada paquete va a un runtime de CPU propio, y todos corren a la vez.
2. **Un trabajo no se parte entre runtimes.** Cada paquete trae sus trabajos, y su runtime los corre en serie.
3. **El ciclo de cada runtime:** pide el runtime de CPU con `new`; sube el paquete; instala podman con apt; construye la imagen; corre los trabajos del manifiesto; arma el paquete de vuelta; lo baja; y libera el runtime con `stop`.
4. **Si la CLI necesita un valor explícito para la CPU,** usá ese valor y anotalo en el informe. Sin `--gpu`, ningún runtime pide una GPU.
5. **`activo.json` guarda una lista de sesiones.** Lee también el formato viejo, de una sola sesión. `oraculo pendientes --vigilar` y `--liberar` recorren la lista.
6. **Cada runtime se libera siempre:** al terminar, con un error, con `SIGINT` o `SIGTERM`, y al pasar un tope. El tope por defecto es de 4 horas por runtime, en una constante con su motivo.
7. **Un runtime que falla no frena a los otros.** Su paquete de vuelta no llega, el comando lo dice, y los demás siguen.
8. **Al bajar cada paquete de vuelta,** el comando llama a la importación del punto 2, de a uno.
9. **El comando nunca escribe en el `ROOT_HOME` real** si una importación falla.

**Tests en rojo primero, con el falso de `ROOT_COLAB_BIN`:**
- Unitario: dos paquetes dan dos sesiones, cada una con `new`, `upload`, `exec`, `download` y `stop`, con vectores de argumentos.
- Unitario: sin `--gpu`, `new` no lleva `--gpu`, o lleva el valor de CPU.
- Unitario: un `exec` que falla en una sesión libera esa sesión con `stop`, y la otra termina igual.
- Unitario: `activo.json` lista las dos sesiones durante el ciclo, y no existe al terminar.
- Unitario: un `activo.json` viejo, de una sola sesión, se lee.
- Unitario: ningún argumento pasado a la CLI contiene el valor de una variable de entorno con nombre de clave.

No corras la validez, `corre`, `calibrar` ni la pasada reales. No llames a la CLI `colab` real. Los corre el orquestador.

## Lo que queda sin probar

El builder no usa Colab ni podman. Quedan para el orquestador:
- **La Prueba 0b** de la vuelta (53), sección 3. Sin ella, la 140 no arranca, y esta tarea no se puede probar en Colab.
- **Una calibración real en Colab** con cuatro paquetes y cuatro runtimes, sin API: el reparto de la vuelta (53), sección 5. Después, la importación al `ROOT_HOME` de la laptop.
- Cuántas asignaciones da `colab usage` con cuatro runtimes, y las unidades que gastó todo.

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

Al terminar, escribí `docs/builds/informe-local-139.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los comandos nuevos**: el texto de `--help` de cada uno, y un ejemplo de manifiesto en JSON.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada función de `colab.rs` que abriste o cambiaste, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
