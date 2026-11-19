# Brief de la Tarea 164 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`, vuelta (81).

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `laptop/tarea-163`. Trae las Tareas 84 a 163 y la 162b.
- **Arrancá cuando termine la suite con podman de la cadena 163 y la verificación de b8d0db4058.** No corras mientras el orquestador o el usuario usen podman.
- Ningún otro builder corre a la vez.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-164`. Después de `git worktree add`, fijá `git config --worktree core.hooksPath .githooks`. Preguntale al orquestador qué worktree usar.
- La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.
- **Subagentes.** El organizador da a cada subagente la ruta absoluta de su worktree. Antes de cada commit, corre `git -C <ruta> status` y compara con la tabla de abajo. Un archivo fuera de la tabla frena el commit.
- **Temporales de subagentes.** Ningún subagente deja carpetas fuera de su worktree.

## Archivos

| Punto | Podés tocar |
|---|---|
| 1, la vuelta de instalación | `src/corre/corrida.rs`, `src/tipos.rs` |
| 2, el arreglo concreto y los cortes de Python | `src/oracle/descarga.rs`, `src/oracle/descarga/python.rs` |
| 3, la calibración y el motivo del corte | `src/oracle/calibra.rs`, `src/oracle/mod.rs` |
| 4, `apt-get` con reintentos en el runtime | `tools/colab/preparar-runtime.sh`, `tests/colab_preparar.rs` |

- Los cuatro puntos no comparten archivos. Corren en paralelo. El punto 1 necesita el texto del arreglo del punto 2 solo en sus tests: usá un hallazgo falso.
- **Tests de otros archivos.** Podés editar un test de cualquier archivo si lee un literal o una firma que esta tarea cambia. Solo el test, no código de producción. Anotá cada uno en el informe.
- **No toques:** `src/oracle/informe.rs`, `src/oracle/contenedor.rs`, `src/colab_calibra.rs`, el resto de `src/`, el resto de `tools/`, `imagenes/`, `rubricas/` y `$ROOT_HOME/`.
- **No cambies `bloqueos`, `no_listo` ni `sin_decision`.**
- **El esquema cambia solo por adición.** `Fila.vuelta_instalacion` es `Option<…>` con `#[serde(default)]`, como `vuelta_eficiencia`. Si `r00t esquema` o `docs/interfaces.md` muestran `Fila`, regeneralos.
- **Sin dependencias nuevas** en `Cargo.toml`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar, y la corre el orquestador.
- La tarea no necesita podman, claves de API reales, Colab ni el `ROOT_HOME` real.
- **No llames a podman real, a la CLI `colab` real ni a ningún builder real.** Los tests de `corre` usan sus lanzadores falsos.
- Corré la suite con `tools/test-seguro.sh`.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.
- **Codegraph.** Esta tarea se construye en Colab, donde `codegraph` no está instalado. Buscá los llamadores con `grep -w <nombre>`, y anotá en el informe que no usaste `codegraph`. Al revisar el informe, el orquestador corre `codegraph callers` en la laptop y une los dos resultados.

## Por qué

**El usuario decidió P121 (a) y P122 (a).** Un producto que no instala da `NoListo`. Un agente intenta arreglarlo, y si no puede, un humano. El agente es el builder: Root no toca el producto que evalúa.

**`corre` ya tiene un molde.** Construye, evalúa y guarda una fila. Si la evaluación marca una regresión de Eficiencia, `regresiones_de_vuelta` pide una vuelta más. `correr_vuelta_eficiencia` relanza el último rol con un prompt que trae las regresiones, y Root evalúa otra vez. La fila guarda esa vuelta en `vuelta_eficiencia`. Falta lo mismo para un producto que no instala.

**El hallazgo de la descarga no dice cómo arreglar.** Dice "el producto no instala: <línea>". El builder y el usuario necesitan el arreglo concreto.

**Los cortes de Python caen en `Corte::Otro`.** No marcan `no_compila`, así que un "no instala" de Python sigue en `SinDecision`. P121 (a) no separó lenguajes.

**`rompe_la_compilacion`, en `calibra.rs`, lee `no_compila`.** Desde la 163, un corte de la descarga puede marcarlo. Una calibración de Seguridad que inyecta una dependencia que no se baja daría un corte. Ese caso contaría como "rompe la compilación", y antes quedaba sin veredicto. Un corte no lo causa el código inyectado.

**`preparar-runtime.sh` no reintenta `apt-get`.** En la primera corrida del paso 7, `apt-get` no llegó a `archive.ubuntu.com`, por timeouts. El guion salió con 100, y `colab calibrar` dio rc 3 a los 617 segundos. La repetición anduvo. Un corte de red pasajero no debe tirar una calibración de horas.

## Tarea

La regla que manda: **un producto que no instala vuelve al builder con el arreglo concreto, y pide un humano si el builder no puede.** Hacé un commit por punto. Cada punto empieza con su test en rojo.

### 1. La vuelta de instalación

1. **Cuándo corre:** la evaluación es un corte "no instala" o "lock inválido", y todos los roles terminaron con código 0. Con "fuente no aceptada" u `Otro`, no corre.
2. **Cómo reconoce el corte:** usá el mismo camino que lee el tipo del corte en el veredicto, el de `motivo_de_corte`. No dupliques el formato.
3. **Qué corre:** el último rol, como en `correr_vuelta_eficiencia`, con un prompt nuevo. El prompt trae el texto de cada hallazgo de la descarga, con su arreglo, y el prompt del rol.
4. **El prompt no trae nada del oráculo:** ni tests, ni ids, ni evidencia. Solo los hallazgos de la fuente `descarga`.
5. **Hasta 2 vueltas.** Una constante, `MAX_VUELTAS_INSTALACION = 2`, con su motivo en el comentario: una para el arreglo, otra para un arreglo a medias.
6. **Después de cada vuelta, Root evalúa otra vez.** Si el producto instala, la corrida sigue como siempre, y esa evaluación puede pedir la vuelta de Eficiencia.
7. **Si después de 2 vueltas sigue sin instalar,** el veredicto final es el último corte, con `NoListo`. `Veredicto.avisos` suma: "pide intervención humana: el producto no instala después de 2 vueltas del builder: <hallazgo>".
8. **Una vuelta que no corre,** por falta de cupo o por un rol que sale con error, deja el motivo, como `correr_vuelta_eficiencia`. No suma más vueltas.
9. **`Fila` suma `vuelta_instalacion: Option<VueltaInstalacion>`,** con `#[serde(default)]`. Lleva cuántas vueltas corrieron, si el producto terminó instalando, el motivo si una vuelta no corrió, y `pide_humano`.

**Tests en rojo primero:**
- Unitario: con un veredicto de corte "no instala" y los roles en código 0, `corre` relanza el último rol, con un prompt que trae el hallazgo. Hoy no relanza nada.
- Unitario: si la segunda evaluación instala, la fila trae `vuelta_instalacion` con 1 vuelta, `instalo` verdadero y `pide_humano` falso.
- Unitario: si después de 2 vueltas sigue sin instalar, la fila trae `pide_humano` verdadero, y el veredicto trae el aviso.
- Unitario: con "fuente no aceptada", no hay vuelta.
- Unitario: el prompt de la vuelta no contiene ningún id de test del veredicto.
- Guarda: una fila vieja sin `vuelta_instalacion` se lee con `None`.
- Guarda: la vuelta de Eficiencia sigue igual.

### 2. El arreglo concreto y los cortes de Python

1. **El texto del hallazgo de "no instala" suma su arreglo,** según lo que dijo `cargo`:

   | Lo que dice `cargo` | El arreglo |
   |---|---|
   | "cannot update the lock file" | "`Cargo.lock` no cuadra con `Cargo.toml`: corré `cargo update --workspace` y subí el lock" |
   | "no matching package" o "failed to select a version" | "una dependencia de `Cargo.toml` no existe, o no tiene esa versión: corregí `Cargo.toml`" |
   | cualquier otro fallo de `cargo vendor` | "`cargo vendor` falló: revisá `Cargo.toml` y `Cargo.lock`" |

2. **El hallazgo de "lock inválido" suma:** "`Cargo.lock` no se puede leer: regeneralo con `cargo generate-lockfile` y subilo".
3. **La línea útil del error sigue en el texto,** como hoy.
4. **Los cortes de `python.rs` pasan a tener su tipo:**
   - "no instala" cuando pip no resuelve o no baja una dependencia;
   - "fuente no aceptada" cuando una regla de fuentes la rechaza;
   - `Otro` solo para lo que no es ninguno de los dos.
5. **El "no instala" de Python suma su arreglo:** "una dependencia de los requirements no existe, o no tiene esa versión: corregí el archivo de dependencias".

**Tests en rojo primero:**
- Unitario: el hallazgo de un lock que no cuadra trae "corré `cargo update --workspace`". Hoy no lo trae.
- Unitario: el hallazgo de "no matching package" trae "corregí `Cargo.toml`".
- Unitario: un corte de Python por una dependencia que pip no encuentra da el tipo "no instala", no `Otro`.
- Unitario: ese corte da `NoListo`.
- Guarda: los tests de `diagnosticar` de la 159 siguen verdes.

### 3. La calibración y el motivo del corte

1. **`rompe_la_compilacion` no cuenta un `no_compila` que viene de un corte de la descarga.** El caso queda sin veredicto, como antes de la 163.
2. **Reconocé el corte con el mismo lector de `motivo_de_corte`.** No dupliques el formato.
3. **Un test de ida y vuelta del motivo:** para cada tipo de corte, el texto que arma `motivo_de_corte` da el mismo tipo al leerlo.

**Tests en rojo primero:**
- Unitario: un caso de calibración con un corte "no instala", contra una base que instala, no da "rompe la compilación". Hoy da verdadero.
- Unitario: el motivo de cada tipo de corte hace ida y vuelta.
- Guarda: los tests de `rompe_la_compilacion` de hoy siguen verdes.

### 4. `apt-get` con reintentos en el runtime

1. **Si `bwrap` ya existe,** `instalar_bubblewrap` no llama a `apt-get`. Lo dice en una línea.
2. **`apt-get update` y `apt-get install` llevan** `-o Acquire::Retries=5` y `-o Acquire::http::Timeout=30`.
3. **Un bucle reintenta el par entero hasta 3 veces,** con una espera de 30 segundos entre intentos. La espera se puede cambiar con una variable, así los tests no duermen.
4. **Después del tercer fallo, el paso sale con su código,** como hoy, con un mensaje que dice cuántos intentos hizo.
5. **No cambies el espejo de Ubuntu.** Los reintentos cubren un corte pasajero, sin depender de otro servidor.

**Tests en rojo primero, en `tests/colab_preparar.rs`:**
- Integración: con un `apt-get` falso que falla dos veces y anda a la tercera, el paso `bubblewrap` sale con 0. Hoy sale con el código del primer fallo.
- Integración: con un `bwrap` falso en el PATH, el paso no llama a `apt-get`.
- Integración: con un `apt-get` que falla siempre, el paso sale distinto de 0 y dice "3 intentos".
- Guarda: los tests de `colab_preparar.rs` de hoy siguen verdes.

No corras la validez, `corre`, `calibrar` ni la pasada reales. Las corre el orquestador.

## Tests con podman que quedan sin probar

El builder no corre suites con podman. Quedan para el orquestador:
- La suite con podman de la cadena con la 164.
- Una corrida real de `corre` sobre b8d0db4058 con su lock real: el builder debe arreglar el lock en la vuelta de instalación.

## Reglas

- **Rojo primero.** Cada arreglo empieza con un test que muestra el defecto en rojo. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos siempre.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real, a Colab ni a un builder real.
- **Oráculo.** Ningún prompt al builder lleva ids de tests, evidencia ni contenido de `oraculos/`.
- **Borrados.** Todo borrado de archivos de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp` del host.
- **Idioma.** Los documentos y los mensajes van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/informe-local-164.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados, con `ROOT_SIN_PODMAN=1`.
2. **Commits**: una tabla con punto, defecto, tests vistos en rojo y commit.
3. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
4. **El prompt de la vuelta de instalación**: un ejemplo con un hallazgo falso.
5. **Los tipos de corte de Python**: cada corte de `python.rs`, y el tipo que da ahora. Además, el mensaje de `apt-get` después de 3 intentos.
6. **Tests que cambiaron de valor**, con su motivo, y cada test de otro archivo que editaste.
7. **Llamadores revisados**: por cada firma cambiada, los llamadores de codegraph y de grep.
8. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
