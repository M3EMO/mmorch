# Brief de la Tarea 57 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` (8b417db o posterior). Otros builders trabajan en paralelo: la 55 en `src/corre/` y el git de `src/oracle/mod.rs`, y la 58 en el núcleo de `src/oracle/`.
- Trabajá en tu propio worktree, en una rama nueva: `laptop/tarea-57`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Archivos

- **Podés tocar:** `src/oracle/descarga.rs`, todo `src/oracle/descarga/`, la función `ejecutar` de `src/oracle/contenedor.rs`, la línea de `src/lib.rs` que llama a `cache_de`, sus tests y `docs/interfaces.md`.
- **No toques:** `src/oracle/mod.rs`, `src/oracle/calibra*`, `src/oracle/memo.rs`, `src/oracle/entrega.rs`, `src/corre/`, `src/proveedores/`, `src/observe/` y el resto de `src/lib.rs`.
- Si un arreglo necesita un archivo fuera de tu lista, detenete y escribilo en el informe.

## Entorno

- Si la calibración real corre al mismo tiempo, exportá `ROOT_SIN_PODMAN=1`. Así tus tests y tu hook no cargan la CPU con podman. La suite con podman corre al mezclar.

- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Corré la suite con `tools/test-seguro.sh`. Toma el lock de podman, así que espera su turno si otro builder la corre.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`.

## Por qué

Los informes de las Tareas 51 y 51-C2 dejaron huecos en la descarga, y el orquestador encontró tres guardas de secretos debilitadas. Leé `docs/builds/2026-11-03-build-local-51.md` y `docs/builds/2026-11-03-build-local-51-c2.md`.

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-04: mezclas hasta 8b417db y Tarea 54 terminada".
- "Desempates técnicos 29 (Opus, 2026-11-04)".
- La sección de tu tarea en el plan.

## Tarea

Hacé un commit por punto. Cada commit nombra el informe y la pregunta, por ejemplo `51-P4`.

1. **Clave de caché de Python (51-P4).** La clave suma una versión de formato. Una caché anterior a H5 deja de ser acierto. Test: una caché con la clave vieja no se usa.
2. **L14 en `src/lib.rs` (51-P5).** El llamador de `cache_de` pasa a `cache_de_con_error(...)?`. Test: un archivo de requisitos ilegible da error de infraestructura en la CLI.
3. **Dependencia solo-sdist.** Si una dependencia de runtime solo tiene sdist, la descarga da sin veredicto, no `no-instala`.
4. **Metadatos que Root no puede leer (51-P2 y Metadata vieja).** Un clon con `setup.py` legado, o un sdist con `PKG-INFO` anterior a Metadata 2.1, suma el aviso "dependencias de runtime no verificadas" en la confianza de la descarga. No corta.
5. **Rechazo con registro (C2-P4).** Todo rechazo del inventario registra un evento `Gate` con la clave rechazada y su línea, sin valores. Vale también para el corte por `inv.rechazos` que ya existía.
6. **Guardas de secretos (hallazgo 5).** Cada guarda busca el valor del secreto de prueba, por ejemplo una cadena única como `secreto-de-prueba-7f3a`, no el nombre del campo. Volvé a las tres guardas que la C2 cambió a `clave@`.
7. **C2 de punta a punta (hallazgo 6).** Un test con podman: un repo con `credential-provider` que crea un archivo marca, y una credencial de prueba en `root.toml`. La descarga no ejecuta el proveedor, la marca no aparece, y queda el `Gate` del rechazo.
8. **Tope de salida en `ejecutar`.** `contenedor::ejecutar` conserva solo los últimos `COLA` bytes de la salida, como `leer_en_hilo`.

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

Al terminar, escribí `docs/builds/<fecha>-build-local-57.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con hallazgo y commit.
3. **Qué falló o quedó incompleto**: cada hallazgo sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
