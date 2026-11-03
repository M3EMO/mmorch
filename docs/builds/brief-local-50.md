# Brief de la Tarea 50 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Uno de sus invariantes dice: "El builder nunca ve el oráculo oculto: una corrida que lo lee queda inválida."

`r00t corre` lanza un builder (`claude -p`) dentro del contenedor `root-builder:1`, construye un producto y lo evalúa. Esta tarea hace que ese builder pueda construir.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo.
- Trabajá en otro worktree, en una rama nueva, por ejemplo `laptop/tarea-50`. Otro builder construye las Tareas 48 y 49 en paralelo, en `laptop/tarea-48`.
- No toques `src/oracle/` ni sus tests: son de las Tareas 48 y 49. Tu tarea vive en `src/corre/`, sus tests y `docs/interfaces.md`.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno

- La imagen `root-builder:1` ya está construida en la laptop, con `scripts/construir-imagen-builder.sh`.
- Los tests de podman corren con `tools/test-seguro.sh`, que toma el lock. Si el otro builder corre su suite, el tuyo espera su turno.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

El orquestador probó la imagen real con los argumentos de hoy, y le pidió a Haiku crear `hola.txt`. Haiku respondió "No pude": hubo 2 denegaciones, de `Bash` y de `Write`. La causa: `args_claude` (`src/corre/args.rs`) no pasa `--permission-mode`, y el bootstrap no trae `.claude/settings.json`. El test de la Tarea 16 no lo vio, porque usa un proveedor falso.

Hay tres huecos más:

- La config `ecc-haiku` promete ECC, pero la imagen no trae el plugin y `args_claude` no pasa `--plugin-dir`.
- El banco construyó memora-ecc con `~/Proyectos/bench-ecc-sdlc/correr-ecc-rol.sh`. Leelo: muestra los plugins por rol, las variables de entorno y el modo `auto`.
- La credencial montada trae `mcpOAuth`, con los tokens de MCP del usuario. El builder corre código con red y no los necesita.

## Tarea

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-03 (tarde): objetivo del usuario, imagen del builder y huecos de la Tarea 16".
- "Desempates técnicos 26 (Opus, 2026-11-03): el builder dentro del contenedor".
- "Tarea 50: Builder que construye dentro del contenedor", con su sección "Validate".

La tarea tiene cuatro partes. Hacé un commit por parte.

1. **Modo de permisos.** `ClaudeCorrida` (`src/corre/config.rs`) suma `permission_mode`, con `auto` por defecto. `args_claude` pasa `--permission-mode <valor>`. Las reglas de `--disallowedTools` sobre `oraculos/` no cambian.
2. **ECC por rol.** La config suma, por rol, el plugin de ECC y el modelo de los subagentes. Usá la forma de `docs/builds/config-ecc-haiku.toml`, y hacé que ese archivo cargue con `ConfigCorrida::desde_texto`.
   - El runner monta el plugin de cada rol en solo lectura en `/opt/plugin`, y pasa `--plugin-dir /opt/plugin`.
   - El entorno del builder trae `CLAUDE_CODE_SUBAGENT_MODEL`, `ECC_DISABLED_HOOKS`, `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0`, y `ECC_AGENT_DATA_HOME` dentro de la carpeta de estado de la corrida.
3. **Credencial sin `mcpOAuth`.** El runner no monta el archivo de credenciales del usuario. Arma una copia con solo `claudeAiOauth`, con permisos 0600, en la carpeta de estado. La monta en solo lectura, y la borra al terminar con `src/borrado.rs`.
   - Antes de lanzar, el runner lee el vencimiento de `claudeAiOauth`. Si vence antes del plazo total de la corrida, sale con código 3 y pide renovar el login en el host.
4. **Tests con la imagen real.**
   - Un test con podman, sin `claude`: un comando de shell dentro del contenedor, con los argumentos de la corrida, escribe un archivo en el repo de la corrida y no puede listar `<ROOT_HOME>/oraculos`.
   - Un test con podman, la imagen real y `claude`, marcado `#[ignore]`: lanza `claude -p` con Haiku y le pide crear `hola.txt` y leer un archivo de `oraculos/`. `hola.txt` existe y la lectura falla. Solo corre con `ROOT_TEST_BUILDER_REAL=1`, porque gasta cupo. No lo corras vos: lo corre el orquestador.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/`, `.claude/plans/root.plan.md` ni `src/oracle/`. No toques nada de `~/.local/share/root/` ni la credencial real del usuario.
- **Sin dinero.** No llames a ninguna API real. No corras el test marcado `#[ignore]`.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/<fecha>-build-local-50.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con parte y commit.
3. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
