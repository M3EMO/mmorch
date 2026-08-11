# Auditoría EJE=coherencia — 2026-08-10

Read-only. Gates estáticos: `ruff check .` = 0, `mypy mmorch` = 0 (100 archivos) — sin regresión.
Verificación: todo candidato pasó por `mmorch_adversarial_verify` (DeepSeek↔Gemini, refutar por
default; 2 rondas para H1/H2). Tally final: **0 BLOCKER · 2 IMPORTANTE · 2 NICE-TO-HAVE · 2 descartados.**

## Contrato estándar propuesto (proyecto ↔ mmorch)

Hoy el contrato existe pero está implícito y repartido. Definición consolidada (verificada contra
lo existente):

1. **Registro**: entrada `{name: path}` en `orchestration/projects.json` — la frontera de control
   de jobs (`mmorch/projects.py:1-6`). Escrita por el hook SessionStart
   (`scripts/autoregister_project.py`, wireado en `~/.claude/settings.json`) o por POST `/projects`
   (`mmorch/server.py:760`). `/new-project` paso 7 la ofrece opt-in.
2. **CLAUDE.md del proyecto**: índice corto (convención new-project) + línea de vault global +
   cualquier wiring específico de consumo (si consume el server HTTP: URL/puerto, esquema de token,
   endpoints usados).
3. **Hooks**: son GLOBALES (`~/.claude/hooks` + settings.json) — un proyecto no necesita hooks
   propios para consumir mmorch; el autoregister le da visibilidad automática.
4. **Tools MCP**: superficie única `mcp_server.py` (46 tools). Los skills globales referencian
   solo tools existentes (verificado: `mmorch_vault_write`, `mmorch_route`, `mmorch_cynefin`,
   `mmorch_build_spec`, `mmorch_spec_interview`, `mmorch_review_code` — todos presentes).
5. **Consumo HTTP (caso Lotus)**: cliente contra las rutas de `mmorch/server.py:755-792`;
   verificado que TODOS los endpoints de `Lotus/src/lib/api.js` existen en el server (state,
   events, run/*, projects, fleet*, minds, chat*, pty/*, jobs/*, transcript, benchmarks,
   approve, kill). Sin endpoints fantasma en ninguna dirección.

## Hallazgos sobrevivientes

### C-01 [IMPORTANTE] Drift del doc de contrato: orchestration/CLAUDE.md describe un server que ya no existe
- **Evidencia**: `CLAUDE.md:9-14` dice "20 tools" y lista 25 (con los cognitivos de :16-18);
  `CLAUDE.md:20` dice "tag v1.1"; `CLAUDE.md:44` da como verificador activo `gemini-2.5-flash`.
  Real: `mcp_server.py` define **46** tools (21 sin documentar: `mmorch_vault_write`,
  `mmorch_rubric_start/next/submit`, `mmorch_autoresearch`, `mmorch_cynefin`, `mmorch_build_spec`,
  `mmorch_intuition`, `mmorch_budget_status`, etc.); `pyproject.toml:3` = version 1.2.0;
  `mmorch/config.py:161` `DEFAULT_VERIFIER = "gemini-3.1-flash-lite"`.
- **Por qué importa**: es EL doc que se carga en cada sesión dentro del repo y la definición
  pública del contrato MCP; hoy miente sobre la mitad de la superficie y sobre los modelos activos.
- **Fix propuesto**: reescribir la sección de tools como puntero ("superficie = `mcp_server.py`,
  N tools, ver docstrings") en vez de lista literal que driftea; actualizar versión y modelos
  citando `config.py` como única fuente (que el CLAUDE.md ya declara en :46 — aplicar esa regla
  a su propio texto). Esfuerzo: bajo (solo doc).
- **Verificación**: SURVIVES ronda 1 ("falla crítica de mantenibilidad que invalida el contrato").

### C-02 [IMPORTANTE] Dos fuentes de verdad para issues en el mismo repo (beads vs .scratch)
- **Evidencia**: `CLAUDE.md:146-148` manda "Use bd (beads) for durable issue/backlog tracking";
  `docs/agents/issue-tracker.md:3` declara "Issues and specs for this repo live as markdown files
  in `.scratch/`". Ambos vivos: `.beads/` existe con estado (`export-state.json`) y `.scratch/`
  se usa (wayfinder escribe ahí per :21-30).
- **Por qué importa**: ambigüedad operativa real — un skill que dice "publish to the issue
  tracker" y el mandato bd divergen; tickets pueden nacer en un sistema y buscarse en el otro.
- **Fix propuesto**: una línea de partición explícita en AMBOS docs: bd = backlog durable
  cross-sesión; `.scratch/<effort>/` = mapas wayfinder + tickets efímeros de un esfuerzo, con
  regla de promoción (ticket que sobrevive al esfuerzo → bd). Esfuerzo: bajo (2 ediciones de doc).
- **Verificación**: SURVIVES ronda 1 ("ambigüedad operativa y riesgo de pérdida").

### C-03 [NICE-TO-HAVE] projects.json contaminado + autoregister sin filtro ni GC
- **Evidencia**: `projects.json` contiene `"repo"` → `AppData\Local\Temp\pytest-of-map12\
  pytest-1315\test_no_test_cmd_breaks_unveri0\repo` (path pytest muerto), `"map12"` → home dir
  completo, `"Claude"` → todo Desktop\Claude. Mecanismo: `scripts/autoregister_project.py:13`
  (`_SKIP` solo excluye orchestration; ningún filtro de temp/home) corriendo en cada SessionStart
  (`~/.claude/settings.json`). No hay GC: `mmorch/projects.py:57-65` `resolve()` tira ValueError
  sobre entradas muertas pero nada las poda. Los tests NO son el polucionador (aíslan via
  monkeypatch de resolve, `tests/test_project_loop.py:11`).
- **Fix propuesto**: agregar a `_SKIP` los prefijos temp (`%TEMP%`, `pytest-of-*`) y el home dir
  exacto; función `prune()` en projects.py (dry-run default) invocable desde `/projects` o el
  dashboard. Esfuerzo: bajo.
- **Verificación**: refutado como IMPORTANTE en 2 rondas ("mantenimiento, no seguridad" — el
  registro solo da visibilidad, editar es explícito per-call per docstring del hook); el residuo
  de higiene fue concedido → entra degradado. El ángulo "home dir como proyecto controlable" se
  deriva a EJE=seguridad (ver handoff).

### C-04 [NICE-TO-HAVE] Lotus sin ningún archivo del contrato (CLAUDE.md / AGENTS.md / docs/claude)
- **Evidencia**: `Desktop\Claude\Lotus\` no tiene CLAUDE.md ni AGENTS.md ni `docs/claude/`
  (solo README, planning/, src/). Es el único consumidor HTTP real de mmorch: el conocimiento del
  backend (base URL, token `X-Token`/`?token=`, timeout, fallback a mock) vive solo en
  `Lotus/src/lib/api.js:1-30`.
- **Fix propuesto**: CLAUDE.md mínimo según convención new-project (índice + north-star) con la
  sección de consumo: server mmorch = backend, puerto, esquema de token, "mock.js es fallback, no
  fuente de verdad", y puntero a `mmorch/server.py:755-792` como lista canónica de endpoints.
  Esfuerzo: bajo.
- **Verificación**: refutado como IMPORTANTE en 2 rondas ("deuda de documentación, no desviación
  funcional") — pero el mandato del eje define desviación de convención como hallazgo → entra
  degradado a NICE.

## /new-project vs contrato nativo (checklist)
El skill (`~/.claude/skills/new-project/SKILL.md`) SÍ cubre: CLAUDE.md+AGENTS.md, vault global
(:44-52), registro mmorch opt-in (paso 7, :90), gates por stack (:88-89), beads (`bd init`, :82).
Gaps detectados (refutados como hallazgo — ver Descartados D-01 — quedan como nota): no scaffoldea
`docs/agents/issue-tracker.md` (que el CLAUDE.md global exige por repo) ni ofrece `codegraph init`.

## Chequeos que salieron limpios
- ruff 0 / mypy 0 (gates enforced, sin regresión).
- Endpoints Lotus api.js ↔ server.py: correspondencia 1:1, sin fantasmas.
- Skills globales → tools MCP: todas las referencias `mmorch_*` existen en `mcp_server.py`.
- `sync_skills.py --check`: "sin drift" (vendorización pocock coherente, dirección repo→installed
  declarada y respetada).
- `glm-5.2` en `DEFAULT_INTUITION_POOL` sí está registrado en MODELS (`config.py:146`).
- Dup cross-repo: no hay código duplicado real mmorch↔Lotus (mock.js es fallback declarado;
  `hillclimb.js` en dynamic-workflows es espejo DELIBERADO para el lado cupo, declarado en
  `CLAUDE.md:113-114`). Ningún helper de Lotus debería migrar a mmorch ni viceversa.
- Hooks globales (pasada liviana): los 5 wireados en settings.json, fail-open documentado,
  sin contradicción con el contrato.
- bitterbot-desktop y caveman-upstream: cero referencias a mmorch → no son consumidores.

## Apéndice: Descartados
- **D-01** /new-project no scaffoldea issue-tracker.md ni codegraph init — refutado: "mejora de
  UX, no incoherencia del sistema existente" (queda como nota arriba).
- **D-02** `server.py:749` default hardcodeado `LOTUS_DIR=~\Desktop\Claude\Lotus\src` — refutado:
  fallback legítimo con override por env var, estándar para deploy local single-user.
- **D-03 (parcial)** H1/H2 como IMPORTANTE — refutados a esa severidad en 2 rondas; entran
  degradados como C-03/C-04.
