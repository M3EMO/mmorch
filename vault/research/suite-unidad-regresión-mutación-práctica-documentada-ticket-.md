---
title: Suite unidad + regresión + mutación: práctica documentada (ticket 13)
created: 2026-09-10
tags: [research, mmorch, research, mutation, ci, sdlc, ticket-13]
status: verified
confidence: 0.85
sources: [https://pitest.org/, https://pitest.org/quickstart/maven/, https://pitest.org/quickstart/incremental_analysis/, https://stryker-mutator.io/docs/stryker-js/configuration/, https://mutmut.readthedocs.io/en/latest/, https://cosmic-ray.readthedocs.io/en/latest/concepts.html, https://cosmic-ray.readthedocs.io/en/latest/reference/cli.html, https://maven.apache.org/surefire/maven-surefire-plugin/, https://maven.apache.org/surefire/maven-failsafe-plugin/, https://maven.apache.org/guides/introduction/introduction-to-the-lifecycle.html, https://docs.junit.org/6.1.3/writing-tests/tagging-and-filtering.html, https://docs.gradle.org/current/userguide/jvm_test_suite_plugin.html, https://abseil.io/resources/swe-book/html/ch23.html, https://bazel.build/docs/user-manual, https://bazel.build/run/scripts, https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-jobs]
---
## Tronco
La práctica documentada es unidad (o tests afectados) en presubmit y suite más ancha después. Nadie documenta «suite total tras cada unidad + mutación para promover».

## Qué es
Pregunta del ticket 13: ¿correr la suite total tras cada unidad y usar mutación como gate de tests es práctica publicada? Fuentes primarias: PIT, Stryker, mutmut, cosmic-ray, Maven Surefire/Failsafe, JUnit tags, Gradle test suites, Bazel, Google TAP (SWE book).

## Evidencia / mecanismo

**Respuesta.** El estándar publicado es más cercano a «unidad en PR + suite ancha tras merge». No es «suite total tras cada unidad».

Google TAP: presubmit corre tests asociados y rápidos. Si pasan, el cambio entra. Postsubmit corre todos los tests afectados, incluso lentos. Correr todo en presubmit es caro. Fuente: https://abseil.io/resources/swe-book/html/ch23.html

Maven: fase `test` = unidad (Surefire). Fase `verify` = integración (Failsafe). Un `mvn verify` corre ambos en orden. Fuente: https://maven.apache.org/guides/introduction/introduction-to-the-lifecycle.html

GitHub Actions: mismo workflow en `pull_request` y `push`. `needs` encadena jobs. Un job rojo salta a los que dependen. `fail-fast` cancela el resto de la matriz. Fuente: https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/use-jobs

Mutación: PIT pide correrla seguido **solo sobre código cambiado**, no sobre el árbol. Fuente: https://pitest.org/

### Herramientas (mide / pasa-falla / URL)

| Tool | Mide | Pasa/falla | URL |
|---|---|---|---|
| PIT | % mutantes muertos (killed/all) | Falla si score < `mutationThreshold` | https://pitest.org/quickstart/maven/ |
| Stryker | mutation score | Exit 1 si score < `thresholds.break` (default null/0 = no falla) | https://stryker-mutator.io/docs/stryker-js/configuration/ |
| mutmut | mutantes vivos/muertos | Sin umbral oficial. Badge vía `export-cicd-stats` | https://mutmut.readthedocs.io/en/latest/ |
| cosmic-ray | survival rate | `cr-rate --fail-over N` ≠ 0 si supervivencia > N% | https://cosmic-ray.readthedocs.io/en/latest/reference/cli.html |
| Surefire | tests de unidad (fase `test`) | Build para al primer fallo (salvo skip) | https://maven.apache.org/surefire/maven-surefire-plugin/ |
| Failsafe | tests de integración (`integration-test`/`verify`) | No falla en `integration-test`; falla en `verify` para permitir teardown | https://maven.apache.org/surefire/maven-failsafe-plugin/ |
| JUnit `@Tag` | filtra qué tests corren | El runner falla si un test del plan falla | https://docs.junit.org/6.1.3/writing-tests/tagging-and-filtering.html |
| Gradle jvm-test-suite | suites por propósito (`test`, `integrationTest`) | `shouldRunAfter(test)`; `failFast` corta | https://docs.gradle.org/current/userguide/jvm_test_suite_plugin.html |
| Bazel `bazel test` | targets `*_test` / `test_suite` | Exit 3 si tests fallan; `--notest_keep_going` aborta | https://bazel.build/run/scripts https://bazel.build/docs/user-manual |
| TAP (interno Google) | tests asociados (pre) y afectados (post) | Submit bloqueado si presubmit rojo; postsubmit atribuye al change (batch + split) | https://abseil.io/resources/swe-book/html/ch23.html |

Surefire fail-fast: `skipAfterFailureCount=1`. https://maven.apache.org/surefire/maven-surefire-plugin/examples/skip-after-failure.html

Blame de target: Bazel nombra el target. Maven nombra el módulo/clase. TAP parte el batch y re-corre cada change. Ningún mutator atribuye «unidad X rompió test Y del resto».

## Aplicable a mmorch
Gate de unidad = checker/comando de tests de esa unidad + mutación opcional (PIT/Stryker/cosmic-ray tienen umbral; mutmut no). Gate de regresión = suite del repo **una vez por aterrizaje/PR**, no por cada unidad interna. Atribución = nombre del job/target, no un tool de mutación.

## Objeciones
Si un repo es chico, `mvn verify` en cada PR ya es «suite total en PR». Eso no prueba «suite total tras cada unidad». TAP postsubmit es *tests afectados*, no `//...` ciego. PIT incremental es experimental y puede perder roturas por dependencias (https://pitest.org/quickstart/incremental_analysis/).

## Veredicto cross-family
- passed / 0.85 / objeción: TAP no es producto público; la fuente es el libro SWE de Google.

## Huecos (ningún tool hace X)
- Ningún producto une unidad + regresión total + mutación.
- Mutación no mide «el módulo nuevo corrompe el resto»; eso es regresión.
- mutmut no tiene gate de score oficial.
- TAP no tiene docs de producto público.
- JUnit tags no son un oráculo de pass/fail de mutación.
- Nadie documenta «suite total después de cada unidad que aterriza».

## Links
- [[sdlc-6-gates]]
