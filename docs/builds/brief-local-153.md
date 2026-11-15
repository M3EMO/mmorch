# Brief de la Tarea 153 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vueltas (63) y (64).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-152`. Trae las Tareas 84 a 152 y la 127b.
- Ningún otro builder corre a la vez. La Tarea 154 vendrá después, en `src/colab_calibra.rs`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-153`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree. La 142 dejó `~/.cache/root-tests/smoke-juez.*`.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, los registros fuera del paquete | `src/colab_calibra.rs` |
| 2, el tope agotado rechaza el paquete | `src/colab_calibra.rs` |
| 3, `--tope-usd` hacia abajo | `src/colab_calibra.rs` |
| 4, el lock de `eventos.jsonl` en la importación | `src/colab_calibra.rs` |
| 5, `..` en un destino de bwrap | `src/oracle/contenedor.rs` |
| 6, `lista_de_carpetas` nombra el guion | `tests/colab_preparar.rs` |

- Los puntos 1 a 4 comparten archivo. Van en serie. El 5 y el 6 corren en paralelo con ellos.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/lib.rs`, `src/cli.rs`, `src/tipos.rs`, `src/proveedores/`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **No toques `gastado_s2_usd`.** Su regla espera la pregunta P112 del usuario, y va en la 154.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real ni a la CLI `colab` real.**
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**1. El paquete lleva registros viejos.** `colab empaquetar` copia `oraculos/<tarea>/` entero, también `registros/`. Esa carpeta guarda el stderr de corridas viejas de la laptop. En Colab confundió el diagnóstico: un registro decía "could not find `Cargo.toml` in `/`", y no era de esa corrida. Por A2, un registro puede traer texto de los tests ocultos. El test `paquete_no_trae_registros_ni_colab_ni_evidencia` no revisa `registros`, aunque su nombre lo dice.

**2. Un tope de 0 pasa al paquete.** Si el gasto de S2 llega a 25 USD, el tope del manifiesto queda en 0. `oraculo calibrar` exige `--tope-usd` mayor que 0, así que cada trabajo con juez falla recién en Colab.

**3. El tope puede pasarse.** `--tope-usd` lleva dos decimales redondeados. Puede quedar hasta 0,005 USD por encima de lo que queda.

**4. La importación agrega líneas a `eventos.jsonl`.** Los demás escritores toman `eventos.jsonl.lock`. El informe de la 142 no dice si la importación lo toma.

**5. La revisión de destinos de bwrap acepta `/oraculo/../x`.** Empieza con `/oraculo/`, pero sale de `/oraculo`. Hoy ningún llamador lo arma.

**6. Un test entra en pánico sin decir qué archivo falló.** `lista_de_carpetas`, en `tests/colab_preparar.rs`, no nombra el guion si falta la línea `CARPETAS_RUNTIME=(`.

## Tarea

La regla que manda: **el paquete lleva solo lo que la calibración usa, un límite nunca se pasa, y una revisión revisa lo que dice.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. Los registros fuera del paquete

1. **`registros/` de cada `oraculos/<tarea>/` no entra al paquete.** Usá el mismo mecanismo de `EXCLUIDOS`, con su motivo. Si `EXCLUIDOS` aplica en todos los niveles, revisá que ningún archivo de tarea necesario se llame `registros`.
2. **El código de un repo no cambia.** `copiar_repo` no aplica `EXCLUIDOS`, y una carpeta `registros` de un repo sigue entrando.
3. **El test viejo revisa también `registros`.**

**Tests en rojo primero:**
- Unitario: con `oraculos/tarea-prueba/registros/x.stderr` en el `ROOT_HOME` falso, el paquete no lo trae. Hoy lo trae.
- Guarda: un repo con `registros/a.txt` lo lleva.

### 2. El tope agotado rechaza el paquete

1. **Con `--juez` y un tope de 0 o menos,** `colab empaquetar` falla con `ErrorOraculo::Uso`. El mensaje da el gastado y el tope de 25 USD. No arma el paquete.
2. **Sin `--juez`, el tope no importa,** y el paquete se arma como hoy.

**Tests en rojo primero:**
- Unitario: con un gasto falso de 25 USD y `--juez`, el empaquetado falla con el gastado en el mensaje. Hoy arma un paquete con tope 0.
- Guarda: con el mismo gasto y sin `--juez`, el paquete se arma.

### 3. `--tope-usd` hacia abajo

1. **El tope de cada trabajo se redondea hacia abajo, al centavo.** Nunca queda por encima de lo que queda.

**Test en rojo primero:**
- Unitario: con 18,456 USD libres, el trabajo lleva `--tope-usd 18.45`. Hoy lleva `18.46`.

### 4. El lock de `eventos.jsonl` en la importación

1. **Revisá si la importación toma `eventos.jsonl.lock`** antes de agregar líneas. Usá el mismo helper que usan los demás escritores.
2. **Si no lo toma, que lo tome.** Si ya lo toma, dejá un test de guarda.

**Test en rojo primero, si falta el lock:**
- Unitario: con el lock tomado por otro, la importación espera o falla con un mensaje claro. No agrega líneas sin el lock.

### 5. `..` en un destino de bwrap

1. **`revisar_destinos_bwrap` rechaza un destino con un componente `..`,** aunque empiece con `/oraculo/`. El error nombra el destino.

**Test en rojo primero:**
- Unitario: `/oraculo/../x` da error con su nombre. Hoy pasa.

### 6. `lista_de_carpetas` nombra el guion

1. **Si falta la línea `CARPETAS_RUNTIME=(`,** el test falla con un mensaje que nombra el guion.

**Test en rojo primero:**
- Integración: con un guion falso sin esa línea, en un temporal del test, el mensaje de falla nombra el guion. Hoy no lo nombra.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 153 y la 154.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-153.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El lock de la importación**: si ya estaba, o qué cambió.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
