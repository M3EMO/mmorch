# Brief de la Tarea 139 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-136`. Trae las Tareas 84 a 136 y la 127b.
- La 137 y la 138 se construyen a la vez. La 137 toca `src/oracle/mod.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/inyectar.rs`, `src/oracle/calibra/sonda_lastre.rs` y los tests de `defectos.rs`. La 138 toca `src/oracle/eficiencia.rs`. No toques esos archivos. Podés llamar a sus funciones públicas, como `calibra::huella_de` y `calibra::agregar`.
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
| 3, el ciclo del runtime | `src/colab_calibra.rs`, `src/oracle/colab.rs` (solo para abrir sus funciones de la CLI a `pub(crate)`), `src/cli.rs`, `src/lib.rs` |

- Los puntos van en serie, en este orden.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** los archivos de la 137 y la 138, `src/oracle/contenedor.rs`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **`src/oracle/colab.rs`:** solo podés cambiar la visibilidad de funciones a `pub(crate)`. Si hace falta cambiar su comportamiento, detenete y escribilo en el informe.
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

**La decisión del usuario, literal, del 2026-11-13:** "C. Colab a) Mover las mediciones a Colab, dentro de 50 unidades". **El 2026-11-14 la recordó:** "acordate de usar colab para temas de root". Desde entonces, todas las calibraciones corrieron en la laptop. Tardan horas, cargan la máquina y llenaron el disco.

**Lo que ya existe.** `src/oracle/colab.rs` corre los tests pendientes en una T4 (decisión 22). Usa la CLI `colab` con los verbos `new`, `upload`, `exec`, `download` y `stop`, siempre por vectores de argumentos. Anota la sesión en `$ROOT_HOME/colab/activo.json`, la sondea, y libera el runtime al terminar, con una señal o con un tope. `oraculo pendientes --vigilar` es la red de seguridad. Prepara Rust en el runtime con `prepara-rust.py`.

**Lo que falta.** La calibración nunca corrió en Colab. Faltan tres cosas: un paquete con lo que la calibración necesita, la importación de los resultados al `ROOT_HOME` de la laptop, y el ciclo del runtime de una calibración.

**Por qué no se sube el binario.** El binario de la laptop se compila en Fedora 44. Colab usa Ubuntu 22.04, con una glibc más vieja. El binario puede no correr. Root se compila en el runtime, desde un `git bundle`, con `prepara-rust.py`.

**Lo que esta tarea no decide.** Todavía no se sabe si podman corre en Colab. Lo dice la Prueba 0 del orquestador. Si corre, la calibración usa contenedores, como en la laptop. Si no, hace falta un modo sin contenedor, la Tarea 140. Esta tarea sirve a los dos caminos: el comando de calibración que corre en el runtime viene en el manifiesto.

## Tarea

La regla que manda: **una calibración en Colab sale de la laptop en un paquete verificable, y vuelve al `ROOT_HOME` solo si coincide con la versión de la laptop.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. El paquete de ida

1. **Un comando nuevo:** `r00t colab empaquetar --tareas <t1,t2> --dimensiones <d1,d2> --repo <tarea>=<ruta> --salida <archivo>`. Podés ajustar los nombres de los flags. Anotalos en el informe.
2. **El paquete lleva:**
   - un `git bundle` del commit actual de Root;
   - de `ROOT_HOME`: `oraculos/<tarea>/` de cada tarea, la caché de dependencias que usa cada tarea, `calibracion.json`, `configs/` y `root.toml`;
   - el repo bueno de cada tarea;
   - un manifiesto en JSON.
3. **El paquete nunca lleva:** `eventos.jsonl` entero, `runs.jsonl`, `transcripts/`, `evidencia/`, `colab/`, ni ningún archivo que se llame como una clave o un token. Una lista fija de exclusiones vive en una constante, con su motivo.
4. **El manifiesto lleva:** el commit de Root, la huella de cada dimensión pedida (`calibra::huella_de`), el `tarea_hash` de cada tarea, las dimensiones, el SHA-256 de cada archivo del paquete, y el comando de calibración que corre el runtime por cada tarea y dimensión.
5. **`root.toml`** se revisa antes de entrar. Si trae un campo con nombre de clave o token con valor, el comando falla y lo nombra, sin imprimir el valor.
6. **Las funciones que arman la lista de archivos y el manifiesto son puras.** El empaquetado con `tar` es aparte.

**Tests en rojo primero, con un `ROOT_HOME` falso en una carpeta temporal:**
- Unitario: el paquete trae `oraculos/<tarea>/`, la caché de la tarea y `calibracion.json`.
- Unitario: el paquete no trae `eventos.jsonl`, `runs.jsonl` ni `transcripts/`.
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
5. **Agrega las filas** con `calibra::agregar`, que ya es público. No dupliques una fila que ya está: misma tarea, dimensión y `ts`.
6. **Copia los `.casos.jsonl` nuevos** a `ROOT_HOME/calibracion/`. Un nombre que ya existe no se pisa.
7. **Agrega las líneas de gasto** a `eventos.jsonl`, con `"origen": "colab"` en su `extra`. Así el gasto de S2 se cuenta igual.
8. **Al final, imprime** cuántas filas entraron, cuántas no y por qué, cuántos casos se copiaron, y el gasto sumado.

**Tests en rojo primero:**
- Unitario: un paquete de vuelta con una fila válida la agrega a `calibracion.json`, y deja el respaldo.
- Unitario: una fila con otra huella no entra, y el comando la nombra.
- Unitario: un archivo con un SHA-256 distinto frena la importación sin escribir nada.
- Unitario: importar dos veces el mismo paquete no duplica filas.
- Unitario: las líneas de gasto llegan a `eventos.jsonl` con `"origen": "colab"`.

### 3. El ciclo del runtime

1. **Un comando nuevo:** `r00t colab calibrar --paquete <archivo> [--gpu <tipo>]`. Sin `--gpu`, pide un runtime de CPU. Si la CLI necesita un valor explícito para la CPU, usá ese valor y anotalo en el informe.
2. **El ciclo sigue el de `colab.rs`:** anota la sesión en `activo.json` antes de pedir el runtime; sube el paquete; prepara Rust con `prepara-rust.py`; compila Root desde el bundle; corre el comando de cada tarea y dimensión del manifiesto; arma el paquete de vuelta; lo baja; y libera el runtime.
3. **El runtime se libera siempre:** al terminar, con un error, con `SIGINT` o `SIGTERM`, y al pasar un tope. El tope por defecto es de 6 horas, en una constante con su motivo.
4. **`oraculo pendientes --vigilar` y `--liberar`** deben ver también esta sesión. Si `activo.json` ya trae un campo de tipo de sesión, usalo. Si no, anotalo en el informe y no cambies su formato.
5. **Las claves.** El comando no sube ninguna clave ni la pasa por argumentos. Si una dimensión pide el juez, el comando del runtime lee la clave de Colab Secrets. Cómo leerla lo decide el planner con la Prueba 0. Dejá ese paso en una sola función, con un comentario que lo diga.
6. **Al bajar el paquete de vuelta,** el comando llama a la importación del punto 2.
7. **El comando nunca escribe en el `ROOT_HOME` real** si la importación falla.

**Tests en rojo primero, con el falso de `ROOT_COLAB_BIN`:**
- Unitario: el ciclo llama a `new`, `upload`, `exec`, `download` y `stop`, en ese orden, con vectores de argumentos.
- Unitario: sin `--gpu`, `new` no lleva `--gpu`, o lleva el valor de CPU.
- Unitario: un `exec` que falla libera el runtime con `stop`.
- Unitario: `activo.json` existe durante el ciclo y no existe al terminar.
- Unitario: ningún argumento pasado a la CLI contiene el valor de una variable de entorno con nombre de clave.

No corras la validez, `corre`, `calibrar` ni la pasada reales. No llames a la CLI `colab` real. Los corre el orquestador.

## Lo que queda sin probar

El builder no usa Colab ni podman. Quedan para el orquestador:
- **La Prueba 0** del plan, vuelta (52), sección 1, en un runtime de CPU.
- **Una calibración real en Colab,** con un paquete de una tarea y una dimensión de oráculo, sin API. Después, la importación al `ROOT_HOME` de la laptop, con respaldo.
- Las unidades que gastó, leídas antes y después.

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
6. **Llamadores revisados**: por cada función de `colab.rs` que abriste, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
