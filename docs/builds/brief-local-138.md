# Brief de la Tarea 138 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/cadena-110-136`. Trae las Tareas 84 a 136 y la 127b.
- La 137 se construye a la vez. Toca `src/oracle/mod.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/inyectar.rs`, un módulo nuevo y los tests de `defectos.rs`. No toques esos archivos.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-138`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`.
- **El disco está al 98 %.** Antes de crear un worktree, preguntale al orquestador cuál usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la mediana y la dispersión por cuartiles | `src/oracle/eficiencia.rs` |

- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** los archivos de la 137, `src/tipos.rs`, `src/lib.rs`, el resto de `src/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
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

**La Tarea 135 corrió `tiempo_tests` 10 veces por lado, y guardó las corridas.** Con esas corridas se ve un problema nuevo: **el ruido quedó entre 0,055 y 0,162.** Un ruido de 0,162 esconde una regresión de tiempo de 16 %.

**La causa es la medida de dispersión.** Hoy es (máximo − mínimo) / mínimo. Con 10 corridas, el rango crece solo, porque hay más chances de una corrida suelta.

**En t1, la base ordenada es:** 0,263, 0,287, 0,291, 0,294, 0,294, 0,295, 0,295, 0,297, 0,304 y 0,306. El mínimo es una corrida suelta. Sola, sube el rango a 16 %.

**El mínimo también engaña en la razón.** La comparación usa el mínimo de cada lado. Con mínimos, los limpios de t1 dan 1,097 y 1,107. Con medianas, dan 1,005 y 1,006.

**Las razones con medianas, en los 12 casos de t1 a t3:**

| Tarea | Razón con mínimos | Razón con medianas | Cuartiles de la base | Cuartiles del repo |
|---|---|---|---|---|
| t1 | 1,097 a 1,113 | 1,004 a 1,010 | 0,017 | 0,007 a 0,031 |
| t2 | 1,002 a 1,020 | 1,008 a 1,024 | 0,011 | 0,012 a 0,027 |
| t3 | 1,008 a 1,034 | 1,004 a 1,024 | 0,015 | 0,005 a 0,039 |

"Cuartiles" es (tercer cuartil − primer cuartil) / mediana.

**Ningún caso cambió el tiempo a propósito.** El lastre es de memoria. Los 12 casos sirven como limpios para el tiempo.

**El margen, medido.** En los 12 casos, el exceso de la razón con medianas sobre 1 fue, como mucho, 1,04 veces la mayor dispersión por cuartiles de los dos lados. Fue en t3, caso 0: 0,024 contra 0,023. Un factor de 2 duplica ese máximo.

**La compilación no tiene este problema.** Su razón quedó entre 0,995 y 1,003 en los 12 casos, con el mínimo de sus corridas.

## Tarea

La regla que manda: **`tiempo_tests` y `memoria_pico` se comparan por la mediana, con un ruido que una corrida suelta no infla.** Hacé un commit. Empieza con su test en rojo.

### 1. La mediana y la dispersión por cuartiles

1. **Para `tiempo_tests` y `memoria_pico`,** la comparación usa la mediana de cada lado. Con el ancla, la referencia es la mediana de la serie de la base más el ancla.
2. **El ruido de esas dos medidas** es 2 veces la mayor dispersión por cuartiles de los dos lados, con el piso de 2 %. La dispersión por cuartiles es (tercer cuartil − primer cuartil) / mediana.
3. **Con menos de 4 corridas en un lado,** ese lado usa la dispersión de hoy. Los cuartiles de 3 corridas no dicen nada.
4. **Los cuartiles** se calculan con interpolación lineal entre las corridas ordenadas. Una función pura los da.
5. **`tiempo_compilacion` y `tamano_binario` no cambian.** Siguen con el mínimo y la dispersión de hoy.
6. **`referencia` y `repo` de la medida guardada** llevan el valor que se comparó: la mediana en las dos medidas que cambian.
7. **Las constantes nuevas** son el factor 2 y el mínimo de 4 corridas. Su comentario cita los datos de este brief: 12 casos de tres tareas, máximo visto de 1,04. Con más datos, se revisan.

**Tests en rojo primero, con las corridas de t1:**
- Unitario: base de t1 (las 10 corridas de arriba) y un repo de 10 corridas con mínimo 0,289, mediana 0,296 y cuartiles chicos. Hoy: razón cerca de 1,099 con mínimos, y ruido 0,162. Con el cambio: razón cerca de 1,005, y ruido cerca de 0,032. Las cuentas: mediana de la base 0,2945; primer cuartil 0,29175; tercer cuartil 0,2965; dispersión 0,0161; por 2, 0,032.
- Unitario: una base estable y un repo con la mediana 1,5 veces más alta dan regresión.
- Unitario: con 3 corridas por lado, el ruido es el de hoy.
- Unitario: los cuartiles de 1, 2, 3, 4 y 5 dan 2 y 4.
- Guarda: la compilación y el tamaño dan lo mismo que hoy.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- **La calibración de Eficiencia de t1 a t3, otra vez,** con la 137 y la 138. Los limpios deben seguir sin detectar. El ruido de `tiempo_tests` debería quedar cerca de 0,02 a 0,08.

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

Al terminar, escribí `docs/builds/informe-local-138.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **Los números del test de t1**: la razón y el ruido de hoy y los nuevos, con sus cuentas.
5. **Tests que cambiaron de valor**, con su motivo.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
