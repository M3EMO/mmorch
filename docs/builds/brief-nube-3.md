# Brief de la build en la nube 3

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base y rama

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo. Ese commit viene después de 1f98ad9.
- Trabajá en tu propia rama de sesión. Al terminar, hacé push de esa rama.
- Las builds de la nube 1 y 2 ya entraron a la base. Leé sus informes antes de empezar: `docs/builds/2026-10-28-build-nube-1.md` y `docs/builds/2026-10-29-build-nube-2.md`.

## Roles

- La sesión organiza y revisa. Corre como Sonnet.
- Cada tarea la construye un subagente Haiku, uno por tarea o por parte de tarea.
- La sesión revisa cada entrega antes del commit. Una corrección empieza con un test en rojo.
- La sesión no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno de la nube

La sesión no tiene podman, ni `ROOT_HOME` real, ni Colab, ni red real, ni claves de API, ni la credencial de Claude.

Primeros pasos, en este orden:

1. Corré `git config core.hooksPath .githooks`.
2. Exportá `ROOT_SIN_PODMAN=1`. Con esa variable, los tests que necesitan podman se saltean y lo avisan en stderr.
3. Corré `cargo test` y anotá el resultado de base: verdes, fallas e ignorados.

Si `cargo test` falla en la base por algo que no es el entorno, detenete y escribilo en el informe.

## Tareas, en orden

Cada tarea está en el plan, con su `Action`, su `Validate` y su parte "Dónde". Construí solo la parte de nube de cada una.

| Orden | Tarea | Sección del plan |
|---|---|---|
| 1 | 29 | "Tarea 29: Suite segura en la laptop" y "Reglas de la laptop" |
| 2 | 28 | "Tarea 28: Desempates de la nube 2" y "Desempates técnicos 11" |
| 3 | 30 | "Tarea 30: Lo que la nube 2 dejó sin hacer" |
| 4 | 16 | "Tarea 16: Runner" |
| 5 | 26 | "Tarea 26: Lectura del oráculo: aislamiento y detección", solo la capa 1 |
| 6 | 12 | "Tarea 12: Calibración (mide al evaluador)", solo la parte que no depende de P57 |

Leé también, antes de empezar:

- "Mezcla de la nube 2 (2026-10-29)".
- Las decisiones 38 a 47 de la tabla "Decisiones del usuario".
- "Desempates técnicos 10" y "Desempates técnicos 11".

Notas por tarea:

- **29** va primero. Cambia `.githooks/pre-commit` para que use `tools/test-seguro.sh`. Desde ese commit, el hook corre la suite sin claves y con un `ROOT_HOME` de prueba. En la nube, el script también respeta `ROOT_SIN_PODMAN=1`.
- **28** es larga. Partila en commits por grupo: rúbricas e informe, topes, descarga, oráculo y Colab, calibración y validez, y las tres preguntas del builder.
- **28**: los `include_str!` de las rúbricas no cambian el contenido de `rubricas/`. Esos archivos están aprobados por el usuario, y nadie los edita.
- **28**: `Decision::SinDecision` va al final del enum. Regenerá su esquema en `docs/interfaces.md`.
- **16** usa un `claude` falso: un script de fixture que emite `stream-json`. El runner pone la carpeta de cada corrida en `$HOME/.cache/root/corridas/`, fuera de `ROOT_HOME`. El runner nunca lanza `claude` real en la nube.
- **16**: el runner corre el builder dentro del contenedor de la capa 1 (decisión 43). En la nube, el ejecutor de contenedor es falso. La prueba real con memora es de la laptop.
- **26**: escribí solo el `Containerfile` del builder y los argumentos del contenedor. El contenedor no monta `ROOT_HOME` ni el resto de `$HOME`. La credencial de Claude entra en solo lectura. La imagen se construye en la laptop.
- **12**: cada caso de calibración corre solo las partes de `evaluar` que mide su dimensión. El alcance (memora, brasa y `n`) espera la pregunta P57 del usuario. No cambies los defaults de alcance.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Hacé un commit por tarea, o por parte de tarea. Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. El plan lo escribe Opus.
- **Sin red ni dinero.** No llames a ninguna API real. No uses Colab. No agregues precios: los precios por defecto ya están en el código.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Interfaces.** Un tipo nuevo, un campo nuevo o una variante nueva solo agrega. Los campos nuevos llevan `#[serde(default)]`, y las variantes nuevas van al final. Regenerá `docs/interfaces.md` cuando cambie un esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.
- **Dudas.** Si una tarea deja una decisión abierta, tomá la opción más cerrada, seguí y escribila en "Preguntas para el planner".

## Informe de la build

Al terminar, o cuando el tiempo se acabe, escribí `docs/builds/<fecha>-build-nube-3.md` y commitealo. Usá las mismas secciones que los informes de las builds 1 y 2:

1. **Resultado de la suite**: la base y el final de `ROOT_SIN_PODMAN=1 cargo test`, con verdes, fallas e ignorados.
2. **Tareas que entraron**: una tabla con tarea, commit y parte.
3. **Correcciones del organizador**: cada corrección con su test en rojo.
4. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
5. **Tests salteados por falta de podman**: la cantidad, y los nombres nuevos o cambiados.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.

Después del commit del informe, hacé push de tu rama.
