# Brief de la Tarea 67 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/tarea-66` (35ce168). No esperes ninguna mezcla.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-67`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/mutacion.rs`, `src/oracle/memo.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/`, `tests/fixtures/`, `docs/interfaces.md` y sus tests.
- **No toques:** `src/oracle/juez.rs`, `src/oracle/requisitos.rs`, `src/oracle/entrega.rs`, `src/oracle/calibra/regla.rs`, `src/corre/`, `src/proveedores/`, `src/observe/`, `src/config.rs` y `src/lib.rs`. En `src/oracle/mod.rs`, solo un literal de test si un campo nuevo lo exige. El resultado de una evaluación real no cambia.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Una calibración real corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Memora pruebas dio sensibilidad 0,1. El inyector borra asserts de los tests que llaman la función por nombre, pero el mutante sigue muerto por un `.unwrap()`, por otro test o por un test de socket. El registro de `cargo-mutants` ya dice qué tests fallaron con cada mutante. El defecto debe apagar esos tests.

Leé en el plan, antes de empezar:

- "Diagnóstico de memora pruebas 0,1 (Opus, 2026-11-05)".
- "Desempates técnicos 38 (Opus, 2026-11-05)".

## Tarea

La regla que manda: **un test usa la forma real de los datos.** Sacá de un registro real de `cargo-mutants` el formato de `test <nombre> ... FAILED` y el `log_path` de `outcomes.json`. Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el punto 5.

1. **Tests que matan.** La corrida de mutación deja en `/salida` un archivo con las líneas `test ... FAILED` de cada registro, con su ruta. El comando conserva su código de salida y sigue dejando `outcomes.json`. `MutanteMuerto` suma `tests: Vec<String>`, cruzando el `log_path` de cada mutante. Rojo: hoy un muerto no sabe qué test lo mató. Un mutante con `Timeout` queda con la lista vacía.
2. **`AssertBorrado` apaga a los que matan.** El defecto agrega `#[ignore]` a cada test de `tests` del mutante elegido. El nombre `db::tests::zset` se busca por su último segmento, después de un `#[test]`, en `src/` y en `tests/`. Si la lista está vacía, o un test no aparece, prueba el siguiente muerto. El lugar del caso es el primer test apagado. Tres tests en rojo:
   - un mutante que el test mata por `.unwrap()` hoy sobrevive al borrado, y después debe vivir;
   - un mutante que mata otro test, que no lo llama por nombre, hoy no aplica o no detecta, y después debe detectar;
   - un test unitario apagado que deja un test de socket matando da un falso negativo.
3. **El caso guarda el mutante.** `.casos.jsonl` suma `mutante` y `tests_apagados`, con `#[serde(default)]`. Rojo: hoy el caso no dice qué mutante eligió. El motivo no aplicable cambia a "ningún mutante muerto tiene tests fallidos ubicables".
4. **Falla del modelo no se reusa.** `Compartidos::leer` rechaza un limpio cuyo `sin_veredicto` dice "el modelo falló". Un `sin_veredicto` por cobertura se sigue reusando. Rojo: hoy un limpio con ese motivo se reusa. Corregí también el nombre del test `leer_reusa_un_limpio_con_sin_veredicto_por_cobertura`, que hoy usa "juez caído".
5. **Guarda de `no_aplica`.** `aplicar` con una fila "no aplica" deja `calibrada` en falso y `exactitud_balanceada` en `None`. Es una guarda: pasa hoy, y el informe lo dice.

No corras la calibración real. La corre el orquestador.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/<fecha>-build-local-67.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
