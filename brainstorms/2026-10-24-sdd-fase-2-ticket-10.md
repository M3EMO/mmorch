# SDD fase 2, ticket 10 (lenguajes, dependencia y determinismo): Brainstorm / Discovery Notes
Date: 2026-10-24 · Goal: decidir en qué lenguajes entran los tests de propiedades, cómo llega la dependencia al repo destino y cómo queda determinista el oráculo. Ticket: `.scratch/sdd-fase-2/issues/10-lenguajes-y-determinismo.md`.

Insumos ya decididos: propiedades en la etapa 1 (ticket 08); atribución por test y perfil de 20 ejemplos dentro de los mutantes, con el mecanismo del perfil a decidir acá (ticket 09); con `accept_cmd` de otro lenguaje no hay atribución por test (ticket 09, caso 11). Hechos medidos el 2026-10-23: Hypothesis no está en el venv ni en el extra `dev`; el contenedor no tiene red y no instala paquetes.

## Summary / key decisions

- Python con Hypothesis y TypeScript con fast-check desde el inicio; el autor elige la librería por lenguaje (Q1).
- La atribución por test sale de un reporte JUnit XML, con un solo parser para todos los runners (Q2).
- Sin librería, la corrida sigue sin propiedades y lo anota; la librería se agrega por `orchestration-4u6` con aprobación (Q3).
- Semilla fija dentro de los gates (Q4), derivada del sha del commit base y anotada en el run-log (Q5).
- Semilla y perfil llegan por un archivo auxiliar ignorado por git, cargado con `-p` (pytest) o `--import` (node) (Q6).
- El perfil apaga la base de ejemplos de Hypothesis (`database=None`); test aprobado con trece casos (Q7). Ticket 10 resuelto.

## Q&A log

### Q1 — Lenguajes
- Asked: ¿solo Python con Hypothesis (A) o Python y TypeScript con fast-check desde el inicio (B)?
- Captured: el usuario respondió "VB"; Claude pidió confirmar; el usuario respondió "B}". Elige **B**.
- Hecho: F2 (TypeScript) corre sus tests con `node --experimental-strip-types --test` (runner `node:test`, sin dependencias; `sdlc.toml` de `.scratch/builder-e2e/research/06-bootstrap/bootstrap/`). fast-check sería una dependencia npm nueva.
- Consecuencia: el caso 11 del ticket 09 (sin atribución por test fuera de pytest) queda corto para TypeScript; la atribución en TS se decide en Q2.
- Flags: atribución por test en TypeScript (Q2).

### Q2 — Atribución por test en TypeScript
- Asked: ¿reporte JUnit XML por runner con un solo parser (A), salida TAP de `node --test` (B) o sin atribución en TS (C)?
- Captured: el usuario elige **A**. `node --test` lo da con `--test-reporter=junit`; vitest también. Un parser sirve para todos los runners y para Java más adelante. Hecho: pytest también escribe JUnit (`--junitxml`), así que el mismo parser puede servir a Python. Enmienda el caso 11 del ticket 09: con JUnit hay atribución también fuera de pytest.
- Flags: cómo llega la dependencia al repo (Q3).

### Q3 — Cómo llega la librería al repo destino
- Asked: ¿agregarla con el mecanismo de `orchestration-4u6` con aprobación (A), cortar con error del entorno (B) o seguir sin propiedades y anotar (C)?
- Captured: el usuario elige **C con A**. La etapa 1 detecta la librería por código antes de escribir propiedades; si falta, la corrida sigue sin propiedades y el run-log lo anota. La adopción va por `orchestration-4u6`: `doctor` muestra la falta como aviso y el mecanismo agrega la librería con aprobación del usuario. Claude avisa a la sesión de repos.
- Flags: determinismo del oráculo (Q4).

### Q4 — Determinismo del oráculo
- Asked: ¿semilla fija dentro de los gates (A) o azar con contraejemplos guardados en el repo (B)?
- Captured: el usuario elige **A**. Mismo código, mismo veredicto (regla del oráculo). B cambiaba el estado del repo entre corridas (`.hypothesis/`). Costo de A: explorar siempre las mismas entradas; mitigación propuesta: la semilla cambia por corrida y queda en el run-log.
- Flags: de dónde sale la semilla de cada corrida (Q5).

### Q5 — De dónde sale la semilla
- Asked: ¿constante (A), derivada del sha del commit base y anotada (B) o sorteada y anotada (C)?
- Captured: el usuario elige **B**. Cada cambio explora entradas distintas; el mismo commit base siempre da la misma semilla, así que un fallo se reproduce sin buscar nada. La semilla queda en el run-log.
- Flags: cómo inyecta el SDLC semilla y perfil sin tocar el repo (Q6).

### Q6 — Cómo llegan semilla y perfil a los tests
- Asked: ¿archivo auxiliar ignorado por git en el worktree, cargado por línea de comando (A), `conftest.py` o setup en el repo vía `orchestration-4u6` (B) o cada test lee una variable de entorno (C)?
- Captured: el usuario elige **A**. pytest carga un plugin con `-p`; `node --test` carga un módulo con `--import`. El archivo vive solo durante la corrida y nunca entra a un commit; el repo no cambia.
- Flags: test de aceptación (Q7).

### Q7 — Test de aceptación
- Asked: ¿aprobás el test con diez casos? Después: ¿falta algo?
- Hallazgo de Claude: Hypothesis guarda contraejemplos en `.hypothesis/` y los repite en la corrida siguiente; eso rompe el determinismo aun con semilla fija. El perfil del archivo auxiliar apaga esa base (`database=None`).
- Captured: el usuario aprueba trece casos: los diez más sin carpeta `.hypothesis/` tras la corrida, el archivo auxiliar que carga igual dentro del contenedor sin red, y sin librería el prompt de la etapa 1 no pide propiedades y el gate de marcas del ticket 08 pasa sin marcas.
- Flags: ninguno. Ticket 10 resuelto.

## Open flags (pending input)
