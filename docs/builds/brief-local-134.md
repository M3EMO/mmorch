# Brief de la Tarea 134 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-132`. Trae las Tareas 84 a 132 y la 127b.
- La 133 se construye a la vez. Toca `src/oracle/eficiencia.rs`, `src/oracle/calibra.rs` y `docs/interfaces.md`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-134`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, el lastre en una función vieja | `src/oracle/calibra/inyectar.rs`, el módulo de tests de `src/oracle/calibra/defectos.rs` |

- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/calibra.rs`, `src/oracle/eficiencia.rs`, `src/oracle/mod.rs`, `src/tipos.rs`, `src/lib.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **Si el inyector necesita un dato que `calibra.rs` no le pasa,** detenete y escribilo en el informe. La 133 toca ese archivo.
- **Sin cambio de esquema.**
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** En esta laptop el comando es `codegraph`. Corré `codegraph init .` y `codegraph sync` en tu worktree antes de buscar llamadores. Uní siempre sus llamadores con los de `grep -w <nombre>`.

## Por qué

**La calibración de Eficiencia con `LastreDeMemoria` dio esto:**
- **t1:** los defectos dieron 0,951 y 0,956. Los limpios dieron 0,950 y 0,948. Es la misma baja: la del ruido. El lastre no movió la memoria.
- **t2:** los defectos dieron 0,415 y 0,417, una razón de 2,4. El lastre sí se vio.
- **t3:** el defecto en `src/content_search/content_match.rs:17` dio 1,0. El segundo quedó "candidatos distintos agotados".

**La causa: el lastre cae en código que la medición nunca corre.**
- La medición automática de la Tarea 115 solo corre los tests que pasan en los dos lados: `bootstrap/` y el repo.
- **Los tests de una función nueva no pasan en `bootstrap/`,** porque ahí la función todavía no existe. Quedan fuera de la medición.
- El lastre de t1 cayó en `composite_pattern.rs:19` y `:170`, y el de t3 en `content_match.rs:17`. Si esas funciones son nuevas, solo las llaman los tests nuevos.
- En t2, el cambio toca una función de `content_search/mod.rs` que ya existía. Los tests viejos la llaman, y el lastre se ve.

**Hoy, `LastreDeMemoria` elige una función de un archivo del cambio** (Tarea 131), como `EstadoGlobal` de la Tarea 129. No mira si la función existía antes.

## Tarea

La regla que manda: **el lastre va a una función que ya existía en `bootstrap/`, para que los tests comunes la puedan correr.** Hacé un commit. Empieza con su test en rojo.

### 1. El lastre en una función vieja

1. **Una función cuenta como vieja** si `bootstrap/` tiene, en el mismo archivo relativo, una función con el mismo nombre.
2. **El lugar del lastre,** en orden:
   - una función vieja de un archivo del cambio;
   - si no hay, una función vieja de cualquier archivo de producción.
3. **Sin `bootstrap/`,** el lugar es el de hoy.
4. **Sin ninguna función vieja,** el defecto queda no aplicable, con el motivo "ninguna función existía en el estado inicial".
5. **Las demás reglas del lugar no cambian.** No vale en una función `const`, en un bloque `unsafe`, en código de tests ni en un `main` de ejemplo.
6. **Una función pura** dice si una función es vieja, dados el archivo del repo y el de `bootstrap/`.
7. **`EstadoGlobal` y `EfectoEnNucleo` no cambian.** Los lee el juez, que lee todo el código.

**Tests en rojo primero:**
- Unitario: en un repo de prueba con una función nueva y una vieja en el mismo archivo del cambio, el lastre va a la vieja. Hoy puede ir a la nueva.
- Unitario: con el cambio solo en funciones nuevas, el lastre va a una función vieja de otro archivo de producción.
- Unitario: sin ninguna función vieja, el defecto queda no aplicable con su motivo.
- Unitario: el repo inyectado compila con `cargo check --offline`.
- Guarda: los tests de `LastreDeMemoria` de la 131 siguen verdes, salvo los que fijaban el lugar viejo. Anotá esos en el informe.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **La calibración de Eficiencia de t1 a t3, otra vez,** con la 133 y la 134. En t1 y t3, los defectos deben dar una razón de memoria cercana a 2,4, como en t2. Si una función vieja no la llaman los tests comunes, el caso queda no detectado. Las medidas de la 133 lo muestran.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Interfaces.** Esta tarea no cambia el esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-134.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El lugar del lastre en el repo de prueba**: el archivo, la línea y la función, y por qué cuenta como vieja.
5. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
