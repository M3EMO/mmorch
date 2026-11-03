# Brief de las Tareas 48 y 49 para el builder local

Este brief vale solo. No necesita otra conversación. La fuente de verdad es `.claude/plans/root.plan.md`.

## Qué es Root

Root es un binario de Rust llamado `r00t`. El crate se llama `root`.
Root evalúa proyectos como un project manager, rutea trabajo entre el cupo de Claude y las APIs, observa cada corrida y aprende.
El contrato del proyecto vive en `GOAL.md`. Sus invariantes mandan sobre todo lo demás. Uno de ellos dice: "El evaluador se mide a sí mismo". La calibración (`r00t oraculo calibrar`) es esa medición.

## Base, rama y roles

- Arrancá desde la punta de `claude/root-e7813e` que trae este archivo.
- Trabajá en una rama local nueva, por ejemplo `laptop/tarea-48`. La Tarea 49 sigue en la misma rama, después de la 48. La mezcla espera la orden del usuario.
- Sonnet organiza y revisa. Haiku construye cada parte. Una corrección empieza con un test en rojo.
- El builder no decide producto. Una duda de producto va a "Preguntas para el planner" del informe.

## Entorno

- La tarea no necesita claves de API reales, Colab ni el `ROOT_HOME` real.
- Podman está libre: la calibración real se cortó. Los tests que necesitan una herramienta real, como `cargo-mutants`, corren con podman dentro de `tools/test-seguro.sh`, que toma el lock.
- No exportes `DEEPSEEK_API_KEY` ni `ANTHROPIC_API_KEY`. Los tests usan jueces y proveedores falsos.

## Por qué

La calibración real se cortó después de 15 horas y 4,19 USD, con resultados que no miden al evaluador:

- **Estructura dio sensibilidad 0.** El inyector `dios` (`src/oracle/calibra/inyectar.rs`) copia módulos dentro de `pub mod concat_<x> { ... }` en otro archivo. Las rutas `super::` y `crate::` cambian de sentido, y el repo deja de compilar. clippy da `SinVeredicto`, Estructura queda sin veredicto, y cada defecto cuenta como falso negativo.
- **Pruebas dio sensibilidad 0.** `assert_borrado` borra un assert al azar. `cargo-mutants` mide solo 60 mutantes, y un assert al azar casi nunca es el único que mata uno de ellos.
- **Eficiencia de memora dio un error.** La tarea no define `[eficiencia]`, así que nada se puede medir. Eso es un "no aplica", no un error.
- **No hay casos guardados.** Sin ellos, nadie puede ver por qué un defecto no se detectó.
- **Es lenta.** Cada caso corre todas las sondas, también la mutación de hasta 20 minutos. Los casos limpios repiten sondas deterministas. Los casos corren en serie aunque el juez espera a la red.

## Tarea

Leé en el plan, antes de empezar:

- "Vuelta del 2026-11-03: validez, calibración cortada y su rediseño".
- "Diagnóstico de la calibración (Opus, 2026-11-03, sobre el código de 4070f99)".
- "Desempates técnicos 25 (Opus, 2026-11-03): mejoras de la calibración".
- "Tarea 48" y "Tarea 49", con sus secciones "Validate".

La regla que manda sobre todas las partes: **un caso de calibración mide lo mismo que una evaluación real.** Si una parte cambia eso, detenete y escribilo en el informe.

### Parte 1: Tarea 48, una calibración que mide bien

Hacé un commit por punto.

1. **Casos guardados.** Cada calibración escribe `$ROOT_HOME/calibracion/<tarea>-<dimensión>-<ts>.casos.jsonl`, un renglón por caso: defecto, archivo y línea, detectado, motivo si quedó sin veredicto, y si se repitió por entorno.
2. **Eficiencia sin `[eficiencia]`.** La calibración escribe una fila "no aplica: la tarea no define [eficiencia]" y sale con 0.
3. **`ArchivoDios` compilable.** El defecto agranda un módulo sin romper la compilación. Le suma funciones nuevas y largas, válidas y sin uso, hasta pasar el límite de tamaño de archivo y de función de la sonda de Estructura.
4. **Inyección inválida.** Si un defecto de una dimensión que no es Completitud ni Funciona rompe la compilación, el caso se reemplaza por el siguiente candidato del mismo defecto. La fila cuenta los reemplazos.
5. **`AssertBorrado` dirigido.** El defecto elige un mutante de la muestra que la base mata. Después borra los asserts del test que lo cubre.
6. **Un test por defecto.** Sobre un fixture chico, cada uno de los defectos de `Defecto` se inyecta, y su dimensión lo detecta. Los defectos estáticos usan las sondas reales. Los que necesitan una herramienta usan la herramienta real con podman. Si la herramienta no está en la imagen, el test usa un falso que refleja el defecto, y el informe lo dice.

### Parte 2: Tarea 49, una calibración más rápida

Empezá la parte 2 solo cuando la parte 1 esté entera y en verde. Hacé un commit por punto.

1. **Mutación desde la base.** Un caso de Estructura, Arquitectura, Diseño, Seguridad, Completitud o Entrega toma la mutación de la base. Las demás sondas corren otra vez, porque el juez las lee.
2. **Casos limpios livianos.** Un caso limpio toma de la base las salidas de las sondas deterministas: estáticas, clippy, gitleaks, audit, hadolint y mutación. Corre otra vez el juez y los tests.
3. **Juez en paralelo.** Las llamadas al juez de varios casos corren de a 4 a la vez. Los contenedores siguen en serie. La reserva con lock del tope ya protege las llamadas en paralelo.
4. **Mutación acotada.** `cargo-mutants` corre solo la muestra de `mutantes_max`, en la evaluación real y en la calibración. Antes de elegir el flag, leé `cargo mutants --help` dentro de la imagen `root-oraculo-rust:2`. Si no hay una forma de elegir mutantes exactos, usá una partición fija y anotala en el informe. La muestra debe ser la misma en la evaluación y en la calibración.

## Reglas

- **Rojo primero.** Cada test nuevo se ve en rojo antes de su implementación. El mensaje del commit lista los tests vistos en rojo y los que quedaron como guardas.
- **Commits.** Usá `git commit -- <rutas>`. Un archivo nuevo necesita `git add <ruta>` antes. No uses `git add -A` ni `--no-verify`.
- **Archivos prohibidos.** No toques `GOAL.md`, `GOAL.hash`, `rubricas/`, `docs/borradores/` ni `.claude/plans/root.plan.md`. No toques nada de `~/.local/share/root/`.
- **Sin dinero.** No llames a ninguna API real.
- **Borrados.** Todo borrado de Root pasa por `src/borrado.rs`.
- **Temporales.** Ningún test crea carpetas en `/tmp`.
- **Interfaces.** Un cambio de esquema solo agrega. Los campos nuevos llevan `#[serde(default)]`. Regenerá `docs/interfaces.md` si cambia.
- **Idioma.** Los documentos van en Español Técnico Simplificado: oraciones de 20 palabras o menos, una idea por oración, voz activa.

## Informe

Al terminar, escribí `docs/builds/<fecha>-build-local-48.md` con estas secciones:

1. **Resultado de la suite**: la base y el final, con verdes, fallas e ignorados.
2. **Commits**: una tabla con parte, punto y commit.
3. **Tiempo de un caso**: cuánto tarda un caso de Diseño y un caso limpio, antes y después de la parte 2, sobre el fixture.
4. **Qué falló o quedó incompleto**: cada punto sin hacer o sin probar, con su motivo.
5. **Preguntas para el planner**: numeradas, con la decisión que tomó el organizador.
