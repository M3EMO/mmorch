# Brief de la Tarea 42 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e`, que trae este archivo.
- Trabajá en una rama local nueva, por ejemplo `laptop/tarea-42`. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno

- La tarea no necesita podman, claves de API, Colab ni el `ROOT_HOME` real.
- Corré la suite con `ROOT_SIN_PODMAN=1 tools/test-seguro.sh`. Así la suite no toma el lock de podman y no choca con un Validate en curso.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`. El script las quita igual.
- El hook pre-commit corre la suite completa con podman y toma el lock. Si hay un Validate en curso, el hook espera su turno.

## Tarea

Leé en el plan, antes de empezar:

- "Tarea 42: Error de tarea por carpeta inválida, test de Arquitectura sin módulos y módulos de los pasos".
- "Desempates técnicos 20", desempate 15.
- "Vuelta del 2026-11-02: `modulos` en las fichas y P-mod1 a".
- La decisión 56 de la tabla "Decisiones del usuario".

La tarea tiene cuatro partes. Hacé un commit por parte.

1. **Carpeta inválida.** Si `oculto` o `contrato` de la tarea es un archivo y no una carpeta, `evaluar` da un error de tarea con código 2. Hoy da un error de infraestructura.
2. **Arquitectura sin módulos.** Con las rúbricas embebidas, un repo Rust sin módulos y un juez falso que mide los ítems de Arquitectura con cobertura suficiente da un valor. Con cobertura insuficiente, Arquitectura queda sin veredicto, con sus `faltantes`. Esta parte suma tests y no debería cambiar código. Si un test falla, anotá la causa en el informe.
3. **Módulos de los pasos (decisión 56).** `faltante` (`src/oracle/informe.rs`) filtra la lista `modulos` del requisito contra los módulos del repo evaluado.
   - La fuente es el conjunto `presentes` de la regla de capas (`capas_sin_modulo` en `src/oracle/perfiles/rust.rs`): los módulos del mapa más `main` y `lib` si existe su archivo.
   - Sacá ese conjunto a una sola función compartida. La regla de capas y `faltante` la usan las dos.
   - Si quedan módulos que existen, el paso dice "revisar" con esos módulos. Si no queda ninguno, dice "falta" con la lista entera.
   - Vale igual para un requisito ausente y para uno parcial.
   - Sin lista `modulos`, `faltante` sigue como hoy y nombra los tests que no pasan.

4. **ETXTBSY.** `test_seguro_misma_ruta_da_la_misma_carpeta` falló con `ETXTBSY` ("Text file busy") en un hook del 2026-11-02. El rename de la nube 8 no alcanzó. Los tests de `tests/test_seguro.rs` lanzan cada ejecutable falso con `sh <ruta>`, nunca directo: `sh` solo lee el archivo, y un archivo que se lee no da `ETXTBSY`. Probalo con 50 corridas seguidas de `cargo test --test test_seguro`.

Los criterios de `cargo test` están en la sección "Validate" de la Tarea 42.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/<fecha>-build-local-42.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con parte y commit.
3. **Qué falló o quedó incompleto**: cada parte sin hacer o sin probar, con su motivo.
4. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
