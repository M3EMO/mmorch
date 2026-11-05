# Brief de la Tarea 62 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` **después de la mezcla de la Tarea 61**. El orquestador te dice el commit. La calibración real corre al mismo tiempo.
- No uses `isolation: "worktree"` para tus subagentes.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-62`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/corre/`, la constante del aviso de dependencias en `src/oracle/descarga/python.rs` y `src/oracle/mod.rs`, los dos tests a renombrar y `.gitignore`. También sus tests y `docs/interfaces.md`.
- **No toques:** el resto de `src/oracle/`, `src/proveedores/`, `src/observe/`, `src/config.rs` y `~/.claude/.credentials.json`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- La calibración real corre mientras trabajás. Exportá `ROOT_SIN_PODMAN=1` en todo tu trabajo, también en tus commits. La suite con podman corre al mezclar.


- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Root frena la corrida real porque exige que el login de Claude cubra 2 horas por cada rol, desde el arranque. Además, la copia de la credencial trae el token de refresco: un `claude` dentro del contenedor podría rotar el login del usuario.

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-04 (noche): 58 y 60 mezcladas, corrida real cortada".
- "Desempates técnicos 32 (Opus, 2026-11-04)".
- "Tarea 62: Vigencia por rol, credencial sin refresco y limpieza".

## Tarea

Hacé un commit por punto.

1. **Copia sin `refreshToken`.** `src/corre/credencial.rs` quita `refreshToken` de `claudeAiOauth` en la copia. Test: la copia no trae `refreshToken` ni `mcpOAuth`.
2. **Vigencia por rol.** Antes de cada rol, Root vuelve a leer la credencial del host, arma otra copia y exige que el token cubra el plazo de ese rol (2 horas), no el de toda la corrida. Test: un token que vence en 3 horas deja arrancar una corrida de dos roles.
3. **Espera entre roles.** Si la vigencia no alcanza antes de un rol que no es el primero, Root registra un `Gate` y revisa el archivo cada minuto, hasta 30 minutos. Si el host lo renueva, sigue. Si no, sale con código 3. Antes del primer rol, sale con código 3 de inmediato. Los plazos son inyectables en los tests.
4. **Test real.** Sumá al test `#[ignore]` de `ROOT_TEST_BUILDER_REAL=1` la comprobación de que `claude` corre con la copia sin `refreshToken`. No lo corras vos: lo corre el orquestador.
5. **Nombres de tests.** Renombrá `oraculo_sin_dockerfile_no_llama_a_podman_y_entrega_da_veredicto` y `oraculo_con_sondas_sanas_solo_diseno_y_arquitectura_quedan_sin_veredicto`, para que digan lo que fijan hoy.
6. **Constante del aviso.** `src/oracle/mod.rs` y `src/oracle/descarga/python.rs` usan una sola constante para el texto "dependencias de runtime no verificadas".
7. **`.gitignore`.** Suma `__pycache__/`.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-62.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
