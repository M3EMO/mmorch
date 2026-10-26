# SDD fase 2, ticket 11 (corrida real de cierre): Brainstorm / Discovery Notes
Date: 2026-10-24 · Goal: fijar qué corrida real decide que deltas y propiedades funcionan. El ticket fija la corrida; no la ejecuta. Ticket: `.scratch/sdd-fase-2/issues/11-corrida-real-de-cierre.md`.

Insumos: tickets 01 a 10 resueltos. Notas en el ticket 11: el repo necesita áreas declaradas (`orchestration-4u6` o a mano); la corrida verifica que `bd` corre en el host y deja el issue `sdlc-fuera-de-area:<area>` en el bd correcto; el número de ejemplos dentro de los mutantes (20) se mide acá. Criterio de juicio: memoria `sdlc-es-builder` (tests, gates y corridas reales; sin A/B del bench).

## Summary / key decisions

- Dos corridas de cierre: orchestration con Python y F2 con TypeScript (Q1).
- Features: `tools/ste-lint.py` (orchestration) y formato argentino en `validarBruto()` (F2) (Q2).
- Evidencia de las propiedades: al menos una mata un mutante que los tests de ejemplo no matan (Q3).
- Tope `usd_max = 3.0` por corrida; cupo de Claude anotado (Q4).
- Test aprobado: cuatro precondiciones y doce condiciones por corrida (Q5). Ticket 11 resuelto.

## Q&A log

### Q1 — Repo de la corrida
- Asked: ¿una corrida en orchestration con Python (A), una en F2 con TypeScript (B) o dos, una por lenguaje (C)?
- Captured: el usuario elige **C**. Cada lenguaje tiene su camino (TS: fast-check, JUnit de `node --test`, módulo con `--import`) y solo una corrida real prueba cada uno.
- Hechos para Q2: `tools/ste-lint.py` tiene 200 líneas y funciones chicas (`sentences`, `wc`, `lint`); F2 (`~/Proyectos/F2-seo-calculadoras/f2-sueldo-neto/src/`) tiene `calculo.ts`, `parametros.ts`, `validacion.ts`, `app.ts`, `tipos.ts`. El mapa `builder-e2e` dejó un hallazgo abierto en F2: el texto dice que los aportes se calculan sobre el bruto, pero arriba del tope de ANSES usan el tope.
- Flags: qué feature corre en cada repo (Q2).

### Q2 — Feature de cada corrida
- Asked: ¿las dos propuestas (A), F2 propuesta y otra para orchestration (B) o el usuario nombra las dos (C)? El usuario respondió "acepto ste-lint pero buscar otro" (otro para F2).
- Captured: orchestration: cambio en `tools/ste-lint.py` (cómo `sentences()` corta oraciones, más la baja de una regla vieja); propiedad: el conteo de palabras del texto entero iguala la suma por oración.
- Captured: F2: `validarBruto()` de `src/validacion.ts` (16 líneas) acepta el formato argentino `1.234.567,89`. `MODIFIED`: el punto pasa a separador de miles. `REMOVED`: el punto deja de ser separador decimal. Propiedades: ida y vuelta (todo monto válido en formato argentino vuelve igual) y `validarBruto()` nunca lanza excepción. El usuario acepta las dos features ("sí, acepto las dos").
- Flags: qué cuenta como señal de las propiedades (Q3).

### Q3 — Evidencia de que las propiedades sirven
- Asked: ¿defecto real del coder (A), mutantes del ticket 09 con una propiedad que mata un mutante que los de ejemplo no matan (B), defecto plantado a mano (C) o B con A anotada?
- Captured: el usuario elige **B**. Los mutantes salen del código, no del autor del test (lección de `builder-e2e`); el dato ya queda en el run-log por el ticket 09.
- Flags: presupuesto y paradas (Q4).

### Q4 — Gasto por corrida
- Asked: ¿tope de hoy, `usd_max = 3.0` por corrida, con cupo anotado (A), 1 dólar (B) o 5 dólares (C)?
- Captured: el usuario elige **A**. El tope de hoy multiplica por diez lo medido en `builder-e2e` (0,10 a 0,23 dólares de API). Las dos corridas suman como máximo 6 dólares. El cupo de Claude (etapas 0, 1, 2b y revisión) queda anotado en el run-log.
- Flags: test de aceptación con precondiciones y criterios de fracaso (Q5).

### Q5 — Test de aceptación
- Asked: ¿aprobás cuatro precondiciones y siete condiciones por corrida? Después: ¿falta algo?
- Captured: el usuario aprueba cuatro precondiciones y doce condiciones: las siete más el issue `sdlc-fuera-de-area:<area>` en el bd correcto si el plan intenta salir del área ("no ocurrió" no falla), tiempo de mutantes y ejemplos usados en el run-log, pre-commit verde en los dos commits de la etapa 6, mismo veredicto al repetir los gates sobre el mismo commit, y tiempo de espera en la parada anotado (lección de `builder-e2e`: `human_interventions` nunca se contaba).
- Flags: ninguno. Ticket 11 resuelto.

## Open flags (pending input)
