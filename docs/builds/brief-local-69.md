# Brief de la Tarea 69 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde `e665a5f`, la punta de `claude/root-e7813e`.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-69`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/corre/`, `Fila` en `src/tipos.rs` (solo campos nuevos), `docs/interfaces.md` y sus tests. En `src/cli.rs` y `src/lib.rs`, solo el flag `--reanuda`.
- **No toques:** `src/oracle/`, `src/route/`, `src/observe/`, `src/proveedores/` y `src/config.rs`. La evaluación no cambia, y el contenedor sigue sin `refreshToken`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La validez automática corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.
- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

La corrida real de memora se cortó a las 2 horas, con 25 de 57 tareas hechas, y no llegó al juez final. `r00t corre` no puede retomar. El corte dejó además un costo estimado de 115,13 USD que, por lectura del código, frena la próxima corrida hasta que venza el bloque de cupo.

Leé en el plan, antes de empezar:

- "Verificación de los hechos de la corrida (Opus, sobre e665a5f)".
- "Cómo llegar al juez final (Opus, 2026-11-05)".

## Tarea

La regla que manda: **un corte por plazo no pierde trabajo, y toda corrida que no aborta llega al juez final.** Hacé un commit por punto. Cada punto empieza con su test en rojo, salvo el punto 7. No imprimas tokens en ningún mensaje ni test.

1. **Costo del corte.** En `costo_de_uso`, la lectura de caché cuesta 0,1 de la entrada, y la escritura de caché 1,25. El evento suma `extra.cache_read_tokens` y `extra.cache_write_tokens`. Rojo: un transcript con 54 732 360 de lectura, 2 797 230 de escritura, 1154 de entrada y 7181 de salida, a 2 y 10 USD, hoy da más de 100 USD. Debe dar cerca de 18 USD.
2. **Mensaje del corte.** Un `ErrorCorre::Cortado` dice "el builder se cortó por plazo", y su `Gate` dice "rol <rol> cortado por plazo". Rojo: hoy dice "no se pudo lanzar" y "no arrancó".
3. **Sesión al terminar cada rol.** La sesión de un rol con `result` exitoso se registra apenas termina. Rojo: con `planifica` bien y `organiza` con error, hoy `planifica` no deja fila.
4. **Plazo por lanzamiento.** `Lanzamiento` suma `plazo`: el menor de `TIMEOUT_BUILDER` y la vida restante del login menos 5 minutos. Antes de lanzar se exige que queden 20 minutos o más. Con menos, rige `esperar_vigencia_del_rol`. Rojo:
   - con 1 hora de login, el contenedor recibe un plazo de 55 minutos, y hoy el rol ni arranca;
   - con 10 minutos, espera, y sale con error si no se renueva.
5. **Vueltas.** Un rol cortado no aborta. Root guarda su transcript, registra el corte y deja un `Gate`, con `MAX_VUELTAS = 4`. Después lanza otra vez, con la credencial leída de nuevo, con `--resume <sesión>` y este prompt: "La vuelta anterior se cortó por tiempo. Revisá `git status` y `git log`. Terminá o descartá lo que quedó sin commit. Seguí esta instrucción:" más el prompt del rol. La sesión sale de la primera línea del transcript parcial que traiga `session_id`. Sin sesión, va sin `--resume`. Tests en rojo:
   - dos cortes y un `result` exitoso llegan a evaluar, con 3 transcripts distintos;
   - 4 cortes evalúan el repo parcial y escriben la fila;
   - la lectura del oráculo en una vuelta cortada deja su evento;
   - una vuelta sin `result` exitoso que no fue cortada sigue abortando, como hoy.
6. **`r00t corre --reanuda <id>`.** Exige `--tarea` y `--config`, y que `<id>` empiece con `corre-<tarea>-<config>-`. Usa la carpeta y el repo existentes, sin `git init` ni copia nueva. Un rol con `result` exitoso en su transcript no se lanza otra vez, y su sesión se registra si falta. El primer rol sin `result` sigue como vuelta nueva. `Fila` suma `reanudada: bool` y `vueltas: u32`, con `#[serde(default)]`. Tests en rojo:
   - un `planifica` terminado no se relanza;
   - un `organiza` parcial sigue con `--resume`;
   - otro `<id>`, o una carpeta que no existe, da error de uso;
   - la fila queda marcada.
7. **Prueba real.** Un test con `#[ignore]`, que corre solo con `ROOT_TEST_BUILDER_REAL=1`, lanza `claude` en el contenedor, lo corta por plazo y lo retoma con `--resume`. Comprueba que la segunda vuelta da `result` exitoso y sigue la misma sesión. Vos no lo corrés: lo corre el orquestador. El informe lo dice.

No corras la corrida real. La corre el orquestador.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-69.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
