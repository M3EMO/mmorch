# Brief de la build en la nube 2

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base y rama

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo. Ese commit viene después de c5ff327.
- Trabajá en tu propia rama de sesión. Al terminar, hacé push de esa rama.
- La build de la nube 1 ya entró a la base. Su informe es `docs/builds/2026-10-28-build-nube-1.md`. Leelo antes de empezar.

## Roles

- La sesión organiza y revisa. Corre como Sonnet.
- Cada tarea la construye un subagente Haiku, uno por tarea.
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

Cada tarea está en el plan, con su `Action`, su `Validate` y la columna "Dónde". Construí solo la parte de nube de cada una.

| Orden | Tarea | Sección del plan |
|---|---|---|
| 1 | 21 | "Tarea 21: Topes y llamadas a la API" |
| 2 | 21b | "Tarea 21b: Razonamiento explícito y modelos vigentes" |
| 3 | 21c | "Tarea 21c: Respuestas en streaming" |
| 4 | 27 | "Tarea 27: Ajustes de la mezcla de la nube 1" y "Desempates técnicos 10" |
| 5 | 22 | "Tarea 22: Descarga y contenedores sin fugas" |
| 6 | 23 | "Tarea 23: El oráculo oculto no sale de `oraculos/`" |
| 7 | 24 | "Tarea 24: Sondas sin puntaje regalado" |
| 8 | 13 | "Tarea 13: Oracle completo" |
| 9 | 13b | "Tarea 13b: Informe del PM" |
| 10 | 12 | "Tarea 12: Calibración (mide al evaluador)" |
| 11 | 12b | "Tarea 12b: Validez" |
| 12 | 26 | "Tarea 26: Lectura del oráculo: aislamiento y detección" |
| 13 | 25 | "Tarea 25: Limpieza de Python y de la ficha" |

Leé también, antes de empezar:

- "Revisión de nube (2026-10-28): verificación de Opus", con la tabla de hallazgos confirmados.
- Las decisiones 39 a 46 de la tabla "Decisiones del usuario".
- "Desempates técnicos 9" y "Desempates técnicos 10".

Notas por tarea:

- **21, 21b y 21c** son dueñas de `src/config.rs`, de `src/proveedores/` y del chequeo de tope de `src/route/comando.rs`. Corrélas en orden y sin mezclar con otras tareas.
- **21b** usa solo los precios del plan: `deepseek-flash`, `deepseek-v4-pro` y `claude-haiku-5-5`. No agregues un precio para Sonnet: espera a la pregunta P55 del usuario.
- **21b** confirma con la documentación de Anthropic que Sonnet 5.5 y Opus 5.5 aceptan `thinking: {"type": "adaptive"}`. Sin acceso a esa documentación, anotalo en el informe y seguí con el test del cuerpo del pedido.
- **13, 13b, 12 y 12b** se prueban con ejecutor, proveedor y LLM falsos. Ninguna llama a una API real ni corre un contenedor.
- **12b** arma los paquetes a ciegas solo con fixtures. Los paquetes reales los arma la laptop.
- **26** construye solo el lector de `stream-json` y la búsqueda. El contenedor del builder es de la laptop.
- **25** toca la descarga de Python. Empezala solo después de cerrar la 22.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Hacé un commit por tarea, o por parte de tarea. Usá `git commit -- <rutas>`. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. El plan lo escribe Opus.
- **Sin red ni dinero.** No llames a ninguna API real. No uses Colab. No inventes precios.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Interfaces.** Un tipo nuevo o un campo nuevo solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` cuando cambie un esquema.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.
- **Dudas.** Si una tarea deja una decisión abierta, tomá la opción más cerrada, seguí y escribila en "Preguntas para el planner".

## Informe de la build

Al terminar, o cuando el tiempo se acabe, escribí `docs/builds/<fecha>-build-nube-2.md` y commitealo. Usá las mismas secciones que el informe de la build 1:

1. **Tareas que entraron**: una tabla con tarea, commit y parte.
2. **Correcciones del organizador**: cada corrección con su test en rojo.
3. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
4. **Tests salteados por falta de podman**: la cantidad, y los nombres nuevos o cambiados.
5. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.

El informe da el resultado final de `ROOT_SIN_PODMAN=1 cargo test`: verdes, fallas e ignorados.
Después del commit del informe, hacé push de tu rama.
