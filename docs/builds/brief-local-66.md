# Brief de la Tarea 66 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `laptop/tarea-65` (9699f70). No esperes ninguna mezcla.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-66`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/calibra.rs`, `src/oracle/calibra/`, `tests/fixtures/`, `docs/interfaces.md` y sus tests.
- **No toques:** `src/oracle/mod.rs`, `src/oracle/requisitos.rs`, `src/oracle/entrega.rs`, `src/oracle/mutacion.rs`, `src/oracle/juez.rs`, `src/oracle/calibra/regla.rs`, `src/corre/`, `src/proveedores/`, `src/observe/`, `src/config.rs` y `src/lib.rs`. La evaluación real no cambia.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Una calibración real corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

La calibración real dejó tres problemas. Memora completitud sigue en 0,3, porque un test inestable crea un hallazgo y ese hallazgo evita el filtro de ruidosos. Brasa funciona detecta 3 de 10, porque el defecto cae en código que la suite no ejecuta. Brasa entrega sale con error, aunque la tarea no aplica esa dimensión.

Leé en el plan, antes de empezar:

- "Diagnóstico del Grupo 1 (Opus, 2026-11-05)".
- "Desempates técnicos 37 (Opus, 2026-11-05)".

## Tarea

La regla que manda: **un test usa la forma real de los datos.** Armá cada hallazgo de un test inestable con `requisitos::hallazgos_inestables`, no a mano. Hacé un commit por punto. Cada punto empieza con su test en rojo.

1. **Hallazgo de un test con ruido.** Un hallazgo cuya `cita` es un test de `ruidosos` no cuenta, ni en `motivo_de_deteccion` ni en `estables::hallazgo_nuevo`. Traducí la cita: `contrato:<id>` queda igual, y el resto pasa a `oculto:<id>`. Rojo: un limpio con los mismos niveles que la base y un hallazgo real de un test inestable hoy detecta `hallazgo_nuevo`, y no debe detectar. Guarda: un hallazgo nuevo de un test que no está en `ruidosos` sí detecta.
2. **Motivo con su cita.** El motivo pasa a `hallazgo_nuevo:<cita>`. Rojo: hoy no dice qué hallazgo disparó.
3. **Ruidosos en la fila.** `Fila` suma `ruidosos: Vec<String>`, con `#[serde(default)]`. Rojo: hoy la lista no se guarda. Una fila vieja se lee con la lista vacía.
4. **Líneas ejercitadas por la suite.** Solo para `ComportamientoRoto`. Si un caso con defecto no cambia ningún test ni el valor frente a la base, Root prueba si la suite ejecuta esa función:
   - en una copia nueva, inserta `panic!("root-sonda");` al inicio de la función que contiene la línea;
   - evalúa esa copia con el mismo evaluador;
   - si tampoco cambia nada, la función no se ejecuta: el caso se reemplaza por el siguiente candidato y suma a `reemplazos`;
   - si cambia algo, el caso queda medido, y es un falso negativo.
   - Sin más candidatos, el caso es no aplicable con el motivo "ninguna línea candidata está en una función que la suite ejecute".

   Tests en rojo, con un evaluador falso que reacciona según el contenido de la copia:
   - una línea no ejecutada se reemplaza, y el caso mide la línea siguiente;
   - una línea ejecutada y equivalente queda como falso negativo;
   - un caso detectado no corre la sonda;
   - la función con firma de varias líneas o con `where` recibe el `panic!` dentro de su cuerpo.
5. **Entrega no aplicable.** Sin `[entrega]` en la tarea y con `elegir_dockerfile` en `SinDockerfile`, `calibrar` escribe una fila "no aplica: la tarea no define [entrega] y el repo no tiene Dockerfile", y sale con 0. No corre la base. Rojo: hoy sale con error de infraestructura. Guarda: un repo con Dockerfile mide como siempre.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-66.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
