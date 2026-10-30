# Brief de la build en la nube 5

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base y rama

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo. Ese commit viene después de 55e7cd2.
- Trabajá en tu propia rama de sesión. Al terminar, hacé push de esa rama.
- Las builds de la nube 1 a 4 ya entraron a la base. Leé el informe de la nube 4 antes de empezar: `docs/builds/2026-10-30-build-nube-4.md`.

## Roles

- La sesión organiza y revisa. Corre como Sonnet.
- La tarea la construye un subagente Haiku, o uno por parte de tarea.
- La sesión revisa cada entrega antes del commit. Una corrección empieza con un test en rojo.
- La sesión no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno de la nube

La sesión no tiene podman, ni `ROOT_HOME` real, ni Colab, ni red real, ni claves de API, ni la credencial de Claude.

Primeros pasos, en este orden:

1. Corré `git config core.hooksPath .githooks`.
2. Exportá `ROOT_SIN_PODMAN=1`. Con esa variable, los tests que necesitan podman se saltean y lo avisan en stderr.
3. Corré `tools/test-seguro.sh` y anotá el resultado de base: verdes, fallas e ignorados.

Si la suite falla en la base por algo que no es el entorno, detenete y escribilo en el informe.

## Tarea

| Orden | Tarea | Sección del plan |
|---|---|---|
| 1 | 33 | "Tarea 33: Capas por grupo y ajustes de la nube 4" |

Leé también, antes de empezar:

- "Mezcla de la nube 4 (2026-10-30)".
- "Hallazgo: las capas de las fichas no llegan a `tarea.toml` (2026-10-30)".
- "Desempates técnicos 13".

Notas de la tarea:

- **Capas.** Una capa de `[arquitectura].capas` es una lista de nombres de módulo. Esa lista junta los nombres de dos implementaciones distintas del mismo producto. Por eso una capa está presente si existe al menos uno de sus módulos.
- **Capas.** El hallazgo de severidad 3 sale solo cuando falta la capa entera. Su cita nombra la capa por sus módulos, por ejemplo "falta la capa [cmd, cmd_keys, cmd_strings]".
- **Capas.** No cambies la regla de orden entre capas: una capa importa solo capas posteriores.
- **Eficiencia.** La sonda corre en todo lenguaje cuando la tarea declara `[eficiencia]`. Ajustá `perfiles::items_sin_sonda` para ese caso, y mantené el test que compara esa lista con las rúbricas.
- **Juez de la ficha.** Usá el mismo límite de entrada que los otros jueces: `juez::MAX_ENTRADA_JUEZ_BYTES`.
- **Rastros.** Las listas `rastros-<id>.toml` se escriben en `$ROOT_HOME/validez/`, junto a `rastros.json`, nunca en la carpeta de los paquetes.
- **Credencial vacía.** El borrado del `.credentials.json` vacío pasa a un guardia que corre en todo camino de salida del runner. El borrado sigue pasando por `src/borrado.rs`.
- **Tests.** Sumá los tres tests que la nube 4 nombró como faltantes. Están en la sección "Tarea 33" del plan.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Hacé un commit por parte de la tarea. Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. El plan lo escribe Opus.
- **Sin red ni dinero.** No llames a ninguna API real. No uses Colab. No agregues precios.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`. Los temporales van bajo `TMPDIR`, que fija `tools/test-seguro.sh`.
- **Interfaces.** Un tipo nuevo, un campo nuevo o una variante nueva solo agrega. Los campos nuevos llevan `#[serde(default)]`, y las variantes nuevas van al final. Regenerá `docs/interfaces.md` cuando cambie un esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.
- **Dudas.** Si la tarea deja una decisión abierta, tomá la opción más cerrada, seguí y escribila en "Preguntas para el planner".

## Informe de la build

Al terminar, o cuando el tiempo se acabe, escribí `docs/builds/<fecha>-build-nube-5.md` y commitealo. Usá las mismas secciones que el informe de la nube 4:

1. **Resultado de la suite**: la base y el final de `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`, con verdes, fallas e ignorados.
2. **Tareas que entraron**: una tabla con tarea, commit y parte.
3. **Correcciones del organizador**: cada corrección con su test en rojo.
4. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
5. **Tests salteados por falta de podman**: la cantidad, y los nombres nuevos o cambiados.
6. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.

Después del commit del informe, hacé push de tu rama.
