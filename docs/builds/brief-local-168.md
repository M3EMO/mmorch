# Brief de la Tarea 168 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vueltas (93) y (94).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- **Arrancá desde la punta de `laptop/tarea-167`** (4939980). Trae 2020ecc y la Tarea 167.
- Trabajá en una rama nueva: `laptop/tarea-168`.
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

## Lo que esta tarea no toca

**Esta tarea no cambia lo que Root mide.** Cambia cómo la laptop importa una vuelta, cómo arma el paquete y cómo el notebook ordena los trabajos.
- **Ninguna huella cambia.** `huella_de` da el mismo valor que en 2020ecc para las nueve dimensiones.
- **No toques:** `src/oracle/`, `src/tipos.rs`, el resto de `src/`, `imagenes/`, `rubricas/`, `tools/colab/preparar-runtime.sh`, `tools/colab/versiones.env` y `$ROOT_HOME/`.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la compuerta para las dimensiones con juez | `src/colab_calibra.rs` |
| 2, el tope de gasto del paquete | `src/colab_calibra.rs`, `src/cli.rs`, `src/lib.rs`, `tools/colab/notebook-juez.py` |
| 3, la fila de cada trabajo en el notebook | `tools/colab/notebook-juez.py` |

- Los tres puntos comparten archivos. Van en serie: 1, 2 y 3.
- **`src/lib.rs` está en la lista a propósito.** Cablea la opción nueva de la línea de comandos.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe. Los tests de Python del notebook viven en `tests/colab_preparar.rs` o junto a él.
- **Sin cambio de esquema** de `Fila`. El manifiesto del paquete ya trae `tope_s2_usd`.
- **Interfaces.** Si `r00t esquema` o `docs/interfaces.md` listan las opciones de `colab empaquetar`, regeneralos.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman la corre el orquestador.
- La tarea no necesita podman, claves de API reales ni el `ROOT_HOME` real.
- **No llames a podman real, a la CLI `colab` real ni a ningún builder real.** Los tests usan el `ColabCli` falso.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En Colab, `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá que no usaste `codegraph`. El orquestador lo corre al revisar.

## Por qué

**1. La compuerta rechazó las 10 filas del juez.** `colab importar` dio "entraron 0 filas; no entraron 10". La compuerta es `decidir_fila`, en `src/colab_calibra.rs`. Compara los `valor_base` de los casos de Colab con los de una fila de la laptop.
- **Para una dimensión de oráculo, la comparación sirve.** La base es determinista. Así se encontró el defecto de bwrap, con una base de 0,111 contra 1,0.
- **Para una dimensión con juez, no sirve.** El juez da otro nivel en otra corrida, sobre el mismo código. Y la laptop no tiene fila de base para la mayoría de esas tareas: el motivo es "sin base de la laptop para comparar".
- **`valor_base` mezcla los ítems del juez y los de sonda.** No hay una parte determinista que se pueda comparar sola.

**2. El tope de S2 no se aplica al paquete entero.** `comando_de_trabajo` da a cada trabajo `--tope-usd` con el saldo de S2. Pero `oraculo calibrar --tope-usd` es el tope de **una** calibración. Con 10 trabajos y un saldo de 23,03 USD, cada trabajo podía gastar 23,03 USD. P107 a gastó 7,95 USD en total, y por eso no se notó. El usuario fijó un tope de 8 USD para la próxima medición (P125 a).

**3. El notebook no encuentra la fila de un trabajo cuando la fila sale con otro `n`.** `_coincide`, en `tools/colab/notebook-juez.py`, compara `tarea`, `dimension`, `n` y `semilla`. Pero el `n` de la fila cuenta solo los casos que cuentan. Un trabajo con `n=12` dio una fila con `n=10`. Hay dos efectos:
- el notebook dice "sin fila nueva", y la regla de parada no ve esa fila;
- al retomar una sesión cortada, `ya_calibrado` no ve el trabajo hecho, y lo corre otra vez. Eso gasta API.

Comparar sin `n` tampoco alcanza. Dos trabajos del mismo paquete pueden tener la misma tarea, dimensión y semilla, con `n` distinto.

## Tarea

La regla que manda: **la laptop acepta una medición del juez sin pedirle que se repita, el paquete nunca gasta más que su tope, y cada trabajo encuentra su propia fila.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La compuerta para las dimensiones con juez

1. **Si `tiene_items_de_juez(dimension)` es verdadero,** `decidir_fila` no exige una fila de base de la laptop y no compara `valor_base`.
2. **Las otras revisiones siguen iguales:** la huella, el `tarea_hash` y "ya estaba en la laptop".
3. **Exige los casos de Colab:** la fila trae sus `.casos.jsonl` en el manifiesto, y cada archivo está en el paquete. Si falta uno, la fila no entra, con el motivo "faltan los casos de Colab".
4. **Cada fila del juez que entra suma un aviso al resumen:** "<dimensión> de <tarea>: dimensión con juez; la base no se compara, porque el juez varía entre corridas".
5. **Para las dimensiones de oráculo, nada cambia.**

**Tests en rojo primero:**
- Unitario: una fila de Arquitectura sin base de la laptop entra, con su aviso. Hoy no entra: "sin base de la laptop para comparar".
- Unitario: una fila de Diseño con una base de la laptop distinta entra. Hoy no entra: "la base de Colab difiere".
- Unitario: una fila de Estructura sin sus casos de Colab no entra, con "faltan los casos de Colab".
- Guarda: una fila de Funciona sin base de la laptop sigue sin entrar, con el motivo de hoy.
- Guarda: una fila de Seguridad con una base distinta sigue sin entrar.
- Guarda: una fila de Arquitectura con otra huella o con otro `tarea_hash` sigue sin entrar.
- Guarda: `huella_de` da, para las nueve dimensiones, los mismos valores que en la base. Calculalos sobre la base y escribilos como literales.
- Integración, con un paquete de vuelta falso: una fila del juez y una de oráculo, las dos sin base de la laptop. Entra la del juez, y la de oráculo no.

### 2. El tope de gasto del paquete

1. **`colab empaquetar --juez` acepta `--tope-usd <X>`.** El tope del paquete, `tope_s2_usd` del manifiesto, pasa a ser el menor entre el saldo de S2 y `X`.
2. **`X` debe ser finito y mayor que 0.** Si no lo es, da un error de uso.
3. **Sin `--tope-usd`,** el tope del paquete es el saldo de S2, como hoy.
4. **La línea "gasto de S2" dice los dos números:** el saldo de S2 y el tope del paquete.
5. **El notebook aplica el tope al paquete entero.** Antes de cada trabajo con juez, calcula lo que queda. Lo que queda es el tope del paquete menos lo gastado antes en el paquete. Lo gastado incluye esta sesión y una sesión anterior que se retoma. El notebook lo toma de `gasto.jsonl`, con la misma regla que `_correr_y_medir`.
6. **El trabajo corre con ese resto como `--tope-usd`,** redondeado hacia abajo a dos decimales. El notebook cambia solo el valor que sigue a `--tope-usd` en los argumentos del trabajo.
7. **Si el resto es menor que 0,01 USD,** el trabajo no corre. El notebook imprime "tope del paquete agotado" y lo suma a los saltos.
8. **La clave no cambia de camino.** Sigue la tubería de `lanzar_con_clave`.

**Tests en rojo primero:**
- Unitario: con un saldo de 15,08 y `--tope-usd 8`, el manifiesto trae 8,00. Hoy la opción no existe.
- Unitario: con un saldo de 5 y `--tope-usd 8`, el manifiesto trae 5,00.
- Unitario: `--tope-usd 0` y `--tope-usd -1` dan un error de uso.
- Unitario de Python: con un tope de 8 y 7,50 gastados, el trabajo siguiente lleva `--tope-usd 0.50`.
- Unitario de Python: con un tope de 8 y 8 gastados, el trabajo no corre, y los saltos lo nombran.
- Unitario de Python: al retomar, lo gastado en la sesión anterior también cuenta.
- Guarda: un trabajo de una dimensión de oráculo no lleva `--tope-usd`, como hoy.
- Guarda: el test de la 167 que dice "de 30" sigue verde.

### 3. La fila de cada trabajo en el notebook

1. **Antes de cada trabajo,** el notebook guarda los `ts` de las filas que ya existen.
2. **La fila nueva del trabajo** es la de la misma `tarea`, `dimension` y `semilla`, con un `ts` nuevo. El `ts` no estaba antes de ese trabajo ni en el paquete de ida. El `n` no se compara.
3. **Al terminar cada trabajo,** el notebook escribe en `hechos.json` el índice del trabajo y el `ts` de su fila. Lo escribe en un temporal y lo renombra.
4. **`ya_calibrado` lee `hechos.json`.** Un trabajo está hecho si su índice está ahí, y su fila sigue en `calibracion.json`.
5. **`hechos.json` va en el paquete de vuelta.** Si el manifiesto de vuelta lista los archivos, lo lista.

**Tests en rojo primero:**
- Unitario de Python: un trabajo con `n=12` cuya fila trae `n=10` encuentra su fila. Hoy da "sin fila nueva".
- Unitario de Python: dos trabajos con la misma tarea, dimensión y semilla, y `n` distinto, encuentran cada uno su propia fila.
- Unitario de Python: al retomar, un trabajo con fila de `n` distinto se salta, y uno sin fila corre.
- Guarda: la regla de parada ve la fila de `n=10`.
- Guarda: los tests de Python de la 142 y de la 162 siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de `laptop/tarea-168`.
- `colab importar` real de `vuelta-p107.tar.gz`, primero en una copia del `ROOT_HOME`.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Claves.** Ninguna clave va en una línea impresa, en un argumento ni en un archivo. Los tests usan valores falsos.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-168.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La compuerta**: qué revisa ahora para una dimensión con juez y para una de oráculo.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los de `grep -w`.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
