# Brief de la Tarea 148 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (60).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-147`. Trae las Tareas 84 a 147 y la 127b. El orquestador la arma cuando la 146 y la 147 terminen.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-148`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la lista de la sonda por archivo | `src/oracle/calibra/sonda_lastre.rs`, `src/oracle/mod.rs` |
| 2, la carga ajena por corrida | `src/oracle/eficiencia.rs` |

- Los dos puntos no comparten archivos. Corren en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/contenedor.rs`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**La calibración v5 de Eficiencia no midió nada útil.** Los 6 defectos quedaron no aplicables, y ningún tiempo tuvo valor. Hay dos causas.

**1. La sonda pierde el inicio de su lista.** La sonda corrió bien en el contenedor. El journal muestra `ROOT_SONDA_INICIO`, 203 funciones y `ROOT_SONDA_FIN`. Pero `resultado_de`, en `src/oracle/contenedor.rs`, guarda solo la cola de 4000 caracteres del stdout (`COLA`). El bloque mide de 4514 a 6205 caracteres. La cola pierde `ROOT_SONDA_INICIO`. Sin inicio, `salida_valida` da falso, y `medir_lastre` da "sin salida; el comando no llegó a sus marcas". El test de punta a punta de la 143 no lo vio: su ejecutor falso devuelve el stdout entero, y su repo tiene cuatro funciones.

**2. El control de carga mide el trabajo de Root.** `con_control_de_carga` lee la carga de un minuto de `/proc/loadavg` antes de las corridas y después de ellas. La segunda lectura llega justo después de las compilaciones `--release`, que usan los 8 núcleos. En v5 dio 14,08. La carga cuenta los procesos de los contenedores de Root. La base pasa su motivo a todos los casos. Así, ningún tiempo puede tener valor.

## Tarea

La regla que manda: **una medida no depende del tamaño de un stdout recortado, y la carga cuenta solo el trabajo ajeno.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La lista de la sonda por archivo

1. **`cmd_sonda` escribe la lista en un archivo.** Después de los tests, corre `ls /tmp/root-sonda > /salida/root-sonda.lista`. `/salida` es la carpeta que `medir_lastre` pasa a `ejecutar_en`.
2. **`medir_lastre` lee ese archivo** en `ruta_salida`, después de la corrida. Sin archivo, da el error de hoy: "sonda de lastre: sin salida; el comando no llegó a sus marcas".
3. **`leer_salida` lee el texto del archivo,** una función por línea. Mantiene el filtro por el mapa de funciones marcadas.
4. **Las marcas `ROOT_SONDA_INICIO` y `ROOT_SONDA_FIN` salen** del comando, junto con `tramo` y `salida_valida`, si quedan sin uso. Uní sus llamadores con codegraph y `grep -w` antes de borrar.
5. **Revisá los otros lectores del stdout.** Buscá cada código que busque una marca de inicio en un `stdout` de `ejecutar` o `ejecutar_en`. Anotá cada uno en el informe, con su tamaño posible. No los cambies si están fuera de tu tabla.

**Tests en rojo primero:**
- Unitario: un ejecutor falso que deja solo los últimos 4000 caracteres del stdout, como `resultado_de`. La sonda marca 300 funciones, y todas corren. `medir_lastre` debe dar las 300. Hoy da el error de las marcas.
- Unitario: el ejecutor falso no escribe el archivo. `medir_lastre` da "el comando no llegó a sus marcas".
- Guarda: el test de punta a punta de la 143 sigue verde. Su ejecutor falso pasa a escribir el archivo.

### 2. La carga ajena por corrida

1. **Cada comando medido imprime `ROOT_CPU_US`.** Es el valor de `usage_usec` en `/sys/fs/cgroup/cpu.stat`, al final del comando. Va en `cmd_tests` y en `cmd_compilacion`, junto a `ROOT_MEM` y `ROOT_NS`.
2. **Root lee la línea `cpu` de `/proc/stat`** justo antes y justo después de cada corrida medida: cada `correr_tests` y cada `correr_build` medido. Mide también el reloj de esa corrida.
3. **La CPU ocupada del host** es la suma de `user`, `nice`, `system`, `irq`, `softirq` y `steal`. Las unidades son de 1/100 s. Escribí ese 100 como constante con su motivo.
4. **La carga ajena de una corrida** es `(CPU ocupada del host − ROOT_CPU_US) / reloj`, en núcleos. Un valor negativo cuenta como cero.
5. **Un caso queda cargado** si alguna corrida medida supera `FRACCION_DE_NUCLEOS_CARGADA` por los núcleos. Sus tiempos quedan sin valor. El motivo dice "la máquina tenía carga ajena: X núcleos de N".
6. **Sin `ROOT_CPU_US` en una corrida,** los tiempos del caso quedan sin valor, con el motivo "sin cpu.stat del contenedor". Nunca se supone carga cero.
7. **Cada caso suma un aviso** con su mayor carga ajena, aunque no esté cargado. Con esos datos se revisa el umbral. Si el aviso no llega a la fila sin cambio de esquema, anotalo.
8. **La base sigue pasando su motivo** a los casos, como hoy.
9. **La lectura de `/proc/loadavg` sale,** con `leer_carga` y `motivo_de_carga` si quedan sin uso.
10. **`ROOT_LOADAVG` de la 147 pasa a `ROOT_PROC_STAT`.** Sin la variable, la ruta es `/proc/stat`. Los tests de `tests/` que llaman a `medir` la usan como hoy usan `ROOT_LOADAVG`.
11. **Revisá el memo y el reuso de sondas.** Una corrida de Eficiencia servida desde un memo no tiene ventana real. Anotá si alguna corrida medida puede pasar por el memo.

**Tests en rojo primero:**
- Unitario: una corrida con la CPU del host igual a la del contenedor no queda cargada, aunque use los 8 núcleos. Hoy, una carga de un minuto de 14 la deja sin valor.
- Unitario: una corrida con 6 núcleos ajenos de 8 deja sin valor los tiempos, con el motivo de carga ajena.
- Unitario: una corrida sin `ROOT_CPU_US` deja sin valor los tiempos, con "sin cpu.stat del contenedor".
- Unitario: `cmd_tests` y `cmd_compilacion` imprimen `ROOT_CPU_US`.
- Guarda: `memoria_pico` y `tamano_binario` no cambian con la carga.
- Guarda: un caso hereda el motivo de una base cargada.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 148.
- La calibración v6 de Eficiencia, de t1 a t3. Ahí se ve si `cpu.stat` existe en el contenedor de podman rootless, y si la sonda da su lista.

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

Al terminar, escribí `docs/builds/informe-local-148.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Lo que se borró**: cada función o constante, con sus llamadores antes de borrar.
5. **Los lectores del stdout**: cada uno, con su marca y su tamaño posible.
6. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
7. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
