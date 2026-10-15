# Descomposición en módulos desde una idea
Type: prototype
Status: resolved
Blocked by: 01, 02
Map: ../map.md

## Question

¿Cómo parte el builder una idea en módulos con contratos verificables, y qué oráculo prueba la integración entre módulos?

- Probar contratos explícitos entre módulos: interfaces y tests de contrato escritos antes del código.
- Medir si cada módulo pasa el SDLC actual por separado.
- Medir si el producto integrado pasa la aceptación de punta a punta.

## Answer

Resuelto (2026-10-15) con un prototipo real: `research/04-modular/` y el repo local `~/Proyectos/F2-seo-calculadoras/prototipo-04` (sin GitHub).

Cómo se partió la idea (decisión de Opus, como fija el ticket 01):

- Bootstrap a mano: commit inicial con `src/tipos.ts` (interfaces entre módulos), `sdlc.toml`, `tsconfig` y `Dockerfile.sdlc` (Node 22 + TypeScript 5.9, imagen `f2-sdlc`).
- Tres módulos en cadena, en el orden de la rúbrica valor × esfuerzo × riesgo: m1 parámetros (la verdad), m2 cálculo (el motor), m3 interfaz.
- Un test de contrato por módulo, escrito antes del código (`research/04-modular/contratos/`).
- Encadenado a mano: el script fusiona cada rama `sdlc/<PHASE>` en `main` antes del módulo siguiente.

Resultado de la corrida final (Sonnet como planner, DeepSeek v4 pro como coder, todo en contenedor sin red):

| Módulo | Estado | Minutos | Rechazos de gates | Escaladas |
|---|---|---|---|---|
| m1 parámetros | built | 1,9 | 0 | 0 |
| m2 cálculo | built | 4,0 | 0 | 0 |
| m3 interfaz | built | 3,2 | 1 (trazabilidad-plan) | 0 |

- Integración: el driver del ticket 05 manejó el sitio armado contra los 70 casos ocultos. Veredicto verde en los 6 checks.
- Costo: US$0,14 de DeepSeek en los tres intentos; planner Sonnet ~US$0,16 equivalentes de cupo; revisión 5b de Claude sin medir.

Los dos intentos fallidos dejaron hallazgos para el builder:

1. En lenguajes que no son Python, el título de cada test debe llevar un identificador `test_R<n>_...`; si no, el gate de trazabilidad de la spec corta. El descomponedor debe escribir los contratos así.
2. El SDLC no separa un error del entorno de un error del código. Faltaban los tipos de Node para `tsc`: el coder hizo 3 vueltas y Claude 2 escaladas sobre algo que no podían arreglar. `_toolchain_missing` no reconoce `TS2307`.

Otros hallazgos:

- La revisión 5b corre con los hooks del usuario: el revisor respondió al aviso de la regla ETS dentro de su veredicto.
- `usd_run` quedó en `null` en el run-log de las tres corridas; el costo salió de `logs/metrics.jsonl`.
