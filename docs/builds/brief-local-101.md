# Brief de la Tarea 101 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-100r` (4a6ef27). Trae encadenadas las Tareas 98, 99 y 100. Las Tareas 84 a 100 todavía no están mezcladas.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-101`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Grupo | Podés tocar |
|---|---|
| A, calibración | `src/oracle/calibra.rs` |
| B, mutación, aviso y regla | `src/oracle/mutacion.rs`, `src/oracle/mod.rs`, `src/oracle/informe.rs` |
| C, inyector | `src/oracle/calibra/inyectar.rs` |

- Los tres grupos no comparten archivos. Corren en paralelo. Dentro de cada grupo, los puntos van en serie.
- `docs/interfaces.md` se regenera una sola vez, al final.
- **No toques:** el resto de `src/`, `tests/` salvo tests nuevos de estos puntos o literales que hagan falta para compilar, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**Compilaciones rotas que cuentan como detección.** `rompe_la_compilacion` (`src/oracle/calibra.rs:2000`) devuelve falso para Funciona y Completitud. Una mutación que rompe el build cuenta como un caso válido de esas dos dimensiones. Con la Tarea 99, un build roto da Funciona y Completitud en 0. Esa mutación se detecta siempre, por construcción.
- El operador "agregar `!`" de la 98 puede dar `!x == 1`, que no compila.
- `modulo_borrado` borra un módulo que se usa y siempre rompe el build. Sus detecciones en memora y brasa fueron seguras por construcción.

**"Ruidoso" se dispara sin ruido.** El orquestador calibró Pruebas con el commit y sus tests (`/home/mpera/.cache/root-wt/orquestador/medicion-100.md`). En t1 y t2, los `mutantes_cambiados` son otros mutantes que mataban los mismos tests que el defecto apagó. Pasan de muerto a vivo por el defecto, no por la herramienta.

**Los 13 "delete field".** cargo-mutants 27.1.0 ignora `-F` para los mutantes "delete field ... from struct ... expression". Corren siempre.

**El aviso de la Tarea 100 no llega al veredicto.** `ResultadoMutacion.fuera_de_la_muestra` existe, pero `mod.rs` no lo lee. La 100 quitó `aviso_de_mas` porque quedó sin uso.

**El repo con tests para Pruebas** lo arma hoy el orquestador a mano, en `rep100/`.

**La revisión de la 98 y la 99** (`/home/mpera/.cache/root-wt/orquestador/revision-98-99.md`):
- `linea_limpia` (`inyectar.rs:1134`) no ve comentarios de bloque multilínea ni cadenas crudas o multilínea. Una línea dentro de `/* */` se puede mutar, y el caso no mide nada.
- `nombre_de_funcion` (`inyectar.rs:409`) no ve `const fn`, `async fn`, `unsafe fn` ni `pub(super) fn`.
- La regla de la 99 se llama "el producto no compila", sin la clave que llevan las demás. El 101 también incluye tests que no compilan.

## Tarea

La regla que manda: **ningún caso se detecta por construcción, y un cambio de estado que explica el defecto no es ruido.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### Grupo A: `src/oracle/calibra.rs`

**A1. Un caso que no compila es inválido.**
1. En Funciona y Completitud, un caso cuya copia da `no_compila` mientras la base compila es una inyección inválida. Va a reemplazo, como en las otras dimensiones.
2. Cambiá `rompe_la_compilacion` para que lo vea en esas dos dimensiones.

**A2. "Ruidoso" nuevo.**
1. Un mutante que pasa de muerto a vivo no es ruido si todos los tests que lo mataban en la base están entre los tests apagados. Los asesinos de la base salen de `MutanteMuerto.tests`.
2. Un mutante que pasa de muerto a vivo con algún asesino de la base activo es ruido.
3. Un mutante que pasa de vivo a muerto es ruido siempre.
4. En el limpio no hay tests apagados, así que todo cambio sigue siendo ruido.
5. `mutantes_cambiados` lista solo los cambios que son ruido. El caso guarda aparte los cambios explicados.

**A3. `con_tests/`.**
1. Si existe `oraculos/<tarea>/con_tests/`, `calibrar --dimension pruebas` lo usa como repo bueno, en vez de `--repo`.
2. La fila lo informa: guarda de qué carpeta salió el repo bueno.
3. Sin `con_tests/`, todo sigue como hoy.

**Tests en rojo primero:**
- Unitario: un caso de Funciona con `no_compila` y la base compilando va a reemplazo, no a detección.
- Unitario: en t2, `replace > with ==` pasa de muerto a vivo, y sus asesinos de la base son los apagados. No es ruido.
- Unitario: un mutante que pasa de vivo a muerto es ruido.
- Unitario: con `con_tests/` en la tarea, `calibrar` de Pruebas usa esa carpeta, y la fila lo dice.

### Grupo B: `mutacion.rs`, `mod.rs` e `informe.rs`

**B1. Excluir los "delete field".**
1. El listado y la corrida de cargo-mutants pasan `--exclude-re` con el patrón de "delete field ... from struct ... expression".
2. El patrón va en una constante, con un comentario que nombra cargo-mutants 27.1.0 y el motivo: esa versión ignora `-F` para ese tipo.

**B2. El aviso de `fuera_de_la_muestra`.**
1. `mod.rs` lee `ResultadoMutacion.fuera_de_la_muestra`. Si no está vacía, el veredicto suma el aviso "cargo-mutants corrió N mutantes que no estaban en la muestra".
2. Volvé a poner la función del aviso que quitó la Tarea 100.

**B3. La regla con clave.**
1. La regla de `no_listo` pasa a `no_compila: el producto o sus tests no compilan`, con la clave como las demás.
2. El motivo de Funciona y de los ocultos dice "el producto o sus tests no compilan".

**Tests en rojo primero:**
- Unitario: el comando del listado y el de la corrida traen `--exclude-re` con el patrón.
- Unitario: un resultado con dos mutantes fuera de la muestra da el aviso en el veredicto.
- Unitario: la regla sale con la clave `no_compila`.

### Grupo C: `src/oracle/calibra/inyectar.rs`

**C1. Completitud usa solo `cuerpo_vaciado`.**
1. `de_dimension(Completitud)` da solo `CuerpoVaciado`. `modulo_borrado` sale de la calibración, con o sin `bootstrap/`.
2. Si `modulo_borrado` queda sin uso, borralo después de unir los llamadores.

**C2. `cuerpo_vaciado` sin estado inicial.**
1. Sin `bootstrap/`, `cuerpo_vaciado` elige cualquier función de producción, con la semilla.
2. Con `bootstrap/`, sigue la regla de la 98: solo funciones que ya existían y que el cambio tocó.
3. El comentario de `en_cambio` explica qué significa en `cuerpo_vaciado`.

**C3. El mismo lector que las sondas.**
1. `inyectar.rs` lee las líneas con `analizar` (`src/oracle/perfiles/rust.rs:292`), en vez de `linea_limpia`.
2. Una línea dentro de un comentario de bloque, o de una cadena cruda o multilínea, no da candidato.
3. Si `linea_limpia` queda sin uso, borrala después de unir los llamadores.

**C4. "Agregar `!`" envuelve la condición.** El operador escribe `!(<condición>)`, con paréntesis.

**C5. Calificadores de `fn`.** `nombre_de_funcion` reconoce `const fn`, `async fn`, `unsafe fn`, `pub(super) fn`, `pub(crate) fn` y sus combinaciones.

**Tests en rojo primero:**
- Unitario: `de_dimension(Completitud)` no trae `ModuloBorrado`.
- Unitario: sin `bootstrap/`, `cuerpo_vaciado` aplica en un repo de fixture.
- Unitario: una línea con `a > b` dentro de un comentario `/* ... */` de varias líneas no da candidato.
- Unitario: una línea dentro de `r#"..."#` no da candidato.
- Unitario: "agregar `!`" sobre `if x == 1 {` da `if !(x == 1) {`.
- Unitario: `pub(crate) const fn f()` y `async unsafe fn g()` se reconocen.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

**Después de mezclar, lo hace el orquestador, no vos:** escribir `con_tests/` en los oráculos de s2-t1 a s2-t3, y actualizar `armar_s2.py` para que lo escriba siempre.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-101.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con grupo, punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Tests que cambiaron de valor**, con su motivo.
5. **Funciones borradas**: con sus llamadores revisados.
6. **Llamadores revisados**: por cada firma cambiada o símbolo borrado, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
