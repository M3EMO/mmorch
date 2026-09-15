---
title: Convenciones de arquitectura de repo: plantilla, gates y CI
created: 2026-09-10
tags: [research, mmorch, sdlc, architecture, gates, templates, cookiecutter, copier, scorecard, slsa, agents.md]
status: seed
confidence: alta en fuentes oficiales; GATE-N.md no aparece en ellas
sources: [https://cookiecutter.readthedocs.io/en/stable/overview.html, https://copier.readthedocs.io/en/stable/creating/, https://docs.github.com/en/repositories/creating-and-managing-repositories/creating-a-template-repository, https://peps.python.org/pep-0621/, https://packaging.python.org/en/latest/specifications/pyproject-toml/, https://maven.apache.org/guides/introduction/introduction-to-the-pom.html, https://maven.apache.org/guides/introduction/introduction-to-the-standard-directory-layout, https://go.dev/ref/mod#go-mod-file, https://12factor.net/codebase, https://12factor.net/config, https://github.com/ossf/scorecard/blob/main/docs/checks.md, https://slsa.dev/spec/v1.2/build-requirements, https://agents.md/, https://docs.anthropic.com/en/docs/claude-code/memory, https://docs.npmjs.com/cli/v10/using-npm/scripts, https://www.gnu.org/prep/standards/html_node/Standard-Targets.html, https://just.systems/man/en/introduction.html, https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches, https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions/creating-a-default-community-health-file, https://sdlc.md/]
---
## Veredicto

`GATE-N.md` / `sdlc.toml` no aparece en docs oficiales. La industria pone gates en CI YAML. El comando vive en el repo (`Makefile`, `package.json`, `justfile`, `pom.xml`). El CI llama ese comando. Branch protection exige el check.

## Plantilla vs proyecto generado

- Cookiecutter: plantilla = `cookiecutter.json` + `{{ cookiecutter.project_name }}/`. El output es otro directorio. Sin metadata de update. https://cookiecutter.readthedocs.io/en/stable/overview.html
- Copier: plantilla = `copier.yml`. El proyecto guarda `.copier-answers.yml` y admite `copier update`. https://copier.readthedocs.io/en/stable/creating/
- GitHub template: copia el branch default. Sin Jinja. Historias no relacionadas. https://docs.github.com/en/repositories/creating-and-managing-repositories/creating-a-template-repository

## Contrato de build por lenguaje

- Python: `pyproject.toml` con `[build-system]` (PEP 518) y `[project]` (PEP 621; `name` obligatorio). https://peps.python.org/pep-0621/ https://packaging.python.org/en/latest/specifications/pyproject-toml/
- Maven: `pom.xml` mínimo `modelVersion`, `groupId`, `artifactId`, `version`. Layout `src/main/java`, `src/test/java`. https://maven.apache.org/guides/introduction/introduction-to-the-pom.html https://maven.apache.org/guides/introduction/introduction-to-the-standard-directory-layout
- Go: `go.mod` + `go.sum` en la raíz. `go test ./...`. https://go.dev/ref/mod#go-mod-file

## 12-factor

- Un codebase, muchos deploys. https://12factor.net/codebase
- Config en env, no en el repo. https://12factor.net/config
- Setup declarativo para unir desarrolladores. No define GATE-N.md.

## Robustez día 1 (Scorecard / SLSA)

- Scorecard mira LICENSE, SECURITY.md, CI, branch protection, SAST, Dependabot, Code-Review. No exige GATE-N.md. https://github.com/ossf/scorecard/blob/main/docs/checks.md
- SLSA L1: el *build* genera provenance (digest + cómo se produjo). No es un archivo del repo. L2 pide plataforma hosted que firme. https://slsa.dev/spec/v1.2/build-requirements

## Agentes

- AGENTS.md: estándar AAIF/Linux Foundation. Markdown libre. Suele listar build/test. https://agents.md/
- CLAUDE.md: Anthropic. `./CLAUDE.md` o `.claude/CLAUDE.md`. Contexto, no enforce. https://docs.anthropic.com/en/docs/claude-code/memory

## Dónde se declara el test de aceptación

- GNU Make: target `check` (antes de instalar); `installcheck` (después). https://www.gnu.org/prep/standards/html_node/Standard-Targets.html
- npm: `scripts.test` → `npm test`. No hay script oficial `acceptance`. https://docs.npmjs.com/cli/v10/using-npm/scripts
- just: recetas en `justfile`. Sin nombre obligatorio. https://just.systems/man/en/introduction.html
- Maven: `mvn test` (unidad); `mvn verify` (IT/failsafe).

## Gates: CI vs contrato en el repo

- **Gates en CI:** patrón conocido. `.github/workflows/*.yml` + required status checks. Enforce al merge. Opaco si el YAML duplica comandos. https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches
- **Gates en archivos de contrato:** `Makefile` / `package.json` / `justfile` / `pom` declaran el comando. Local y CI corren lo mismo (parity 12-factor). No bloquean merge salvo que CI los invoque.
- `GATE-N.md` / `sdlc.toml`: no es estándar. Cercanos no oficiales: https://sdlc.md/ (criterio en AGENTS.md) y `gates.md` por issue en blogs de agentes.

## Obligatorio vs opcional (repo bien armado)

**De facto obligatorio:** VCS; archivo de build del lenguaje; README; LICENSE; CI que corre tests.

**Esperado, no mandatorio del lenguaje:** SECURITY.md, CONTRIBUTING.md, CODE_OF_CONDUCT; branch protection; Dependabot; AGENTS.md.

**No exigido por ninguna spec oficial:** GATE-N.md, sdlc.toml, `docs/sdlc/gates/`.

GitHub community health: https://docs.github.com/en/communities/setting-up-your-project-for-healthy-contributions/creating-a-default-community-health-file
