# Industria vs tickets 02 / 13 / 03 / 11

Fecha: 2026-09-10. Nota canónica: vault `research` tag `mmorch` (título: SDLC 6 gates: herramientas y prácticas de industria por ticket).

Este archivo es el puntero local del esfuerzo. No duplicar el cuerpo si diverge: leer el vault.

## Gist

- Gates de merge: Sonar quality gate + GitHub required status checks. Fail-closed.
- Módulo + total: pirámide Fowler; Maven Surefire vs Failsafe. Mutación: PIT / Stryker con umbral explícito.
- Escalación: Aider (lint on, test off por default); SWE-agent tope de USD/turns.
- Repo: comando de test en el repo + CI. `GATE-N.md` no es estándar; `AGENTS.md` es puntero, no oráculo.

## Implicación para el grilling

- 13 extra: mutmut sin umbral; cosmic-ray sí. PIT incremental = código cambiado. Nadie une unidad + total + mutación en un producto. Mutación ≠ regresión.
- 03: tope de vueltas + USD es estándar. Avisar al agotar, no en cada rojo.
- 11: obligatorio = comando de aceptación + checks. `GATE-N.md` convive con CI, no lo reemplaza.
- 03 (números): Aider/AutoCodeRover tope **3**; Cursor Cloud CI **10**; SWE-agent tope **USD**, no vueltas. Ver vault `loops-de-parche-y-escalación-cuando-falla-un-check-agentes-v.md`.
- 02 extra: job `skipped` en GitHub = success. TAP = subset en presubmit, resto después.
