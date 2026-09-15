---
title: SDLC 6 gates: herramientas y prácticas de industria por ticket
created: 2026-09-10
tags: [research, mmorch, sdlc, gates, mutation, ci, ticket-02, ticket-13, ticket-03, ticket-11]
status: draft
confidence: medium
sources: [https://docs.sonarsource.com/sonarqube-server/quality-standards-administration/managing-quality-gates/introduction-to-quality-gates, https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches, https://martinfowler.com/articles/practical-test-pyramid.html, https://pitest.org/quickstart/maven/, https://stryker-mutator.io/docs/stryker-js/configuration/, https://maven.apache.org/surefire/maven-failsafe-plugin/, https://aider.chat/docs/usage/lint-test.html, https://swe-agent.com/latest/usage/competitive_runs/, https://agents.md/]
---
## Pregunta

¿Qué herramientas y prácticas existen para el contrato de gate (02), testeo módulo+total (13), escalación (03) y convención por repo (11)? Fuentes primarias. Fecha: 2026-09-10.

## Hallazgo corto

El estándar de industria pone el oráculo en el **comando de CI** y bloquea el merge con un **status check**. `GATE-N.md` no es un formato conocido. Es el contrato del engine, no un reemplazo de CI.

Nadie documenta «suite total después de cada archivo». El estándar es: unidad en el PR, integración en `verify`, aceptación al final. Mutación sí es gate, con umbral numérico.

## Ticket 02 — contrato de gate

- SonarQube: un quality gate es un conjunto de **condiciones**. Si una se cumple, el gate falla. Se puede bloquear el merge. Fuente: docs.sonarsource.com quality gates.
- GitHub: `Require status checks before merging`. El check debe ser `successful` (o skipped/neutral). Contrato en ruleset / branch protection, no en un markdown de etapa. Fuente: docs.github.com protected branches.
- Fail-closed es el default de un check requerido. Advisory = check que no está en la lista requerida (Sonar «warn» vs fail).
- **Vs nuestro contrato:** `CheckResult` + `measured_ref` es más estricto que Sonar (ellos no exigen número de un experimento propio). `GATE-N.md` es extra: el mundo usa YAML de workflow + condiciones del quality gate.

## Ticket 13 — módulo + total + mutación

- Pirámide (Fowler 2018): muchos tests de unidad, menos de integración, pocos e2e. Aceptación = «¿la feature funciona?». Fuente: martinfowler.com practical-test-pyramid.
- Maven: Surefire = unidad (falla ya). Failsafe = integración; no falla en `integration-test` para poder hacer teardown; `mvn verify` mira el resultado. Fuente: maven.apache.org failsafe plugin.
- PIT (JVM): `mutationThreshold` / `coverageThreshold` / `testStrengthThreshold` fallan el build si el score queda debajo. Default del umbral de mutación es 0 (no gate hasta que lo ponés). Fuente: pitest.org/quickstart/maven.
- Stryker: `thresholds.break`; por debajo, exit 1. Default JS: `break: null` (nunca falla el build). Default.NET: `break: 0`. Hay que prenderlo. Fuente: stryker-mutator.io configuration.
- **Hueco:** no hay herramienta estándar que **sintetice** tests desde la spec. PIT/Stryker miden tests que ya existen.
- **Vs nuestra Q9:** suite total por unidad es más agresivo que CI típico (una vez por PR). Justificable si la suite es ~3 s. Mutación en cada unidad es cara; Stryker documenta `--since` / `--break-at` en pipeline, no en cada archivo.

## Ticket 03 — escalación y parches

- Aider: lint automático por default. Test automático **apagado** (`--auto-test` default False). El comando de test debe devolver rc != 0 y texto; Aider intenta arreglar. Fuente: aider.chat/docs/usage/lint-test.html.
- SWE-agent: sin tope de costo el loop no termina. Recomiendan límite de USD por instancia o ~50 turns. Fuente: swe-agent.com competitive_runs.
- **Vs nuestro diseño:** reasoner que propone parche + aviso al agotar es el patrón (oráculo → retry → humano). No hay juez en el veredicto.

## Ticket 11 — convención por repo

- El comando de test vive en `package.json` / Maven / Makefile / `AGENTS.md` («Build and test commands»). Fuente: agents.md.
- AGENTS.md es markdown libre, sin campos obligatorios. No es un gate ejecutable.
- El bloqueo real vive fuera del markdown: GitHub rulesets + jobs de CI.
- **Vs nuestro destino:** `docs/sdlc/gates/` es nuestro. El estándar mínimo de un repo robusto: comando de aceptación ejecutable + checks requeridos. Sin eso, no entra.

## Qué no afirmar

- No existe un SDLC de 6 etapas con GATE-N.md como producto de mercado.
- Mutación no reemplaza aceptación.
- Un umbral de mutación (80, 85) es convención de herramienta, no un número medido para mmorch. No copiarlo sin medir.

## Complemento (subagentes, misma fecha)

- GitHub: un job `skipped` cuenta como success en required checks (hueco fail-open). Merge queue re-corre checks sobre el grupo combinado.
- GitLab: «Pipelines must succeed». Un pipeline skipped bloquea salvo override. Merge trains = misma idea que merge queue.
- Google TAP (SWE book): presubmit (subset) bloquea; post-submit corre el resto. No es YAML de GitHub.
- Aider `max_reflections = 3`. AutoCodeRover: 3. Cursor Cloud auto-fix CI: 10. Detalle: `vault/research/loops-de-parche-y-escalación-cuando-falla-un-check-agentes-v.md`.
- Copier guarda `.copier-answers.yml` y admite update. Cookiecutter no. Scorecard no pide GATE-N.md. GNU Make: target `check` antes de instalar.

## Complemento mutación (ticket 13)

- Práctica documentada: tests afectados en presubmit; suite más ancha **después**. No existe «suite total tras cada unidad + mutación para promover».
- mutmut: sin umbral oficial. cosmic-ray: `cr-rate --fail-over N`. PIT: mutar solo código cambiado. Blame: Bazel nombra el target; Maven nombra módulo/clase.
- Mutación no mide «esta unidad corrompió el resto». Eso es regresión. Son dos gates.
- Detalle: `vault/research/suite-unidad-regresión-mutación-práctica-documentada-ticket-.md`.
