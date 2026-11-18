# Brief de la Tarea 163 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (80).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-162b`. Trae las Tareas 84 a 162 y la 162b.
- **Arrancá cuando termine la suite con podman de la cadena 162b.** No corras mientras el orquestador o el usuario usen podman.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-163`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, "no instala" y "lock inválido" cuentan como "no compila" | `src/oracle/descarga.rs`, `src/oracle/mod.rs` |
| 2, una costura para probar "no instala" sin contenedor | `src/oracle/descarga.rs` |
| 3, el informe de un corte dice que las suites no corrieron | `src/oracle/informe.rs` |

- Los puntos 1 y 2 comparten `descarga.rs`. Van en serie: primero el 2, así el 1 ya tiene su test. El punto 3 corre en paralelo.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/contenedor.rs`, `src/colab_calibra.rs`, `src/tipos.rs`, el resto de `src/`, `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **No cambies `bloqueos`, `no_listo` ni `sin_decision`.** La decisión sale de las reglas de siempre.
- **Sin cambio de esquema.** `Preparacion` es un enum interno.
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

**El usuario decidió P121 (a).** "No instala" y "lock inválido" cuentan como "no compila", y dan `NoListo`. "Fuente no aceptada" queda en `SinDecision`: es una negativa de Root por su política, no una prueba de que el producto falle.

**La regla ya existe.** `no_listo`, en `src/oracle/informe.rs`, da `NoListo` cuando `v.no_compila` es verdadero. Su disparo dice "no_compila: el producto o sus tests no compilan". Hoy, el camino del corte deja `no_compila` en falso, así que los tres cortes dan `SinDecision`.

**El camino del corte no sabe qué corte es.** `Preparacion::SinVeredicto` lleva solo los hallazgos. `descarga.rs` ya anota el motivo en `Registro::corte`: "no-instala", "lock-invalido" o "fuente-no-aceptada". Falta llevar ese tipo hasta `evaluar_con`.

**El corte "no instala" no tiene test directo.** Solo ocurre con un contenedor. `preparar_rust` no tiene una costura para reemplazar el ejecutor de `cargo vendor`.

**El informe de un corte no dice por qué no midió.** Dice "descarga sin veredicto: <detalle>" y marca las nueve dimensiones "Sin medir". No dice que las suites no corrieron.

**Un test viejo prueba otro corte del que nombra.** `oraculo_descarga_sin_veredicto_deja_las_nueve_sin_veredicto` usa `LOCK_ROTO` con `Cargo.toml`. Ese caso da "fuente no aceptada", no "lock inválido".

**La parte del agente que arregla no va aquí.** Espera la pregunta P122 del usuario.

## Tarea

La regla que manda: **un producto que no se puede construir da rojo por la regla de siempre, y el informe dice por qué no midió el resto.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 2. Una costura para probar "no instala" sin contenedor

1. **`preparar_rust` recibe el ejecutor de `cargo vendor`** por parámetro, o por el mismo camino que ya usan otras costuras de `descarga.rs`. Producción pasa el ejecutor real.
2. **Un test da una salida de `cargo vendor` con "no matching package",** y `preparar_rust` devuelve el corte "no instala".
3. **Uní los llamadores** de cada función que cambia de firma, con codegraph y `grep -w`.

**Test en rojo primero:**
- Unitario: con un ejecutor falso que falla con "no matching package named `x` found", `preparar_rust` da el corte "no instala". Hoy no existe un test así.

### 1. "No instala" y "lock inválido" cuentan como "no compila"

1. **`Preparacion::SinVeredicto` suma el tipo de corte:** no instala, lock inválido o fuente no aceptada. Un enum interno, por ejemplo `Corte`.
2. **Cada `corte(...)` de `descarga.rs` pasa su tipo,** el mismo que hoy anota en `Registro::corte`.
3. **En `evaluar_con`,** el camino del corte marca `v.no_compila = true` si el tipo es "no instala" o "lock inválido". Con "fuente no aceptada", lo deja en falso.
4. **La decisión sale de `decidir`, sin regla nueva.**
   - "No instala" y "lock inválido" dan `NoListo`, en rojo.
   - "Fuente no aceptada" da `SinDecision`.
   - Si además hay un secreto, `bloqueos` da `Bloqueado`, como hoy.
5. **El test viejo de `LOCK_ROTO`** pasa a llamarse por el corte que prueba: "fuente no aceptada". Un test nuevo arma un `Cargo.lock` que no parsea, para probar "lock inválido".

**Tests en rojo primero:**
- Unitario: el corte "no instala" da `NoListo` y estado rojo. Hoy da `SinDecision`.
- Unitario: un `Cargo.lock` que no parsea da el corte "lock inválido", y `NoListo`. Hoy da `SinDecision`.
- Unitario: "fuente no aceptada" da `SinDecision`, con informe.
- Guarda: un corte con un secreto de severidad 3 en Seguridad da `Bloqueado`.
- Guarda: las nueve dimensiones siguen sin veredicto en los tres cortes.

### 3. El informe de un corte dice que las suites no corrieron

1. **El informe de un corte suma una línea:** "Las suites no corrieron: la descarga se cortó (<tipo de corte>)."
2. **La línea va donde el informe ya explica lo que no midió,** por ejemplo junto a "Sin medir". Elegí el lugar que cambie menos.
3. **Un informe normal no cambia.**

**Tests en rojo primero:**
- Unitario: el informe de un corte "no instala" contiene "Las suites no corrieron". Hoy no lo contiene.
- Guarda: el informe de una evaluación normal no contiene esa línea.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 163.
- La verificación otra vez de b8d0db4058 con su lock real. Debe dar `NoListo`, con informe.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real ni a Colab.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-163.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **La decisión de cada corte**: lo que da cada uno, medido con su test.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
7. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
