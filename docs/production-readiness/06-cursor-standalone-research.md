# 06 — mmorch en Cursor + mmorch standalone (independiente de Claude Code)

Fecha: 2026-08-27. Research externo (WebSearch/WebFetch) + inspección local del repo
`C:/Users/map12/.claude/orchestration` (paquete `mmorch` v1.2.0, MCP server con 46 tools).

---

## Parte A — Compatibilidad con Cursor

### A.1 Formato de configuración

Cursor lee MCP servers de dos lugares ([docs oficiales](https://cursor.com/docs/mcp)):

- **Global:** `~/.cursor/mcp.json`
- **Por proyecto:** `.cursor/mcp.json` en la raíz del repo

Mismo shape `mcpServers` que Claude Code. Para mmorch (stdio, igual que el registro
actual en `~/.claude.json:1544`):

```json
{
  "mcpServers": {
    "mmorch": {
      "command": "C:\\Users\\map12\\.claude\\orchestration\\.venv\\Scripts\\python.exe",
      "args": ["C:\\Users\\map12\\.claude\\orchestration\\mcp_server.py"],
      "env": { "PYTHONUTF8": "1" }
    }
  }
}
```

Soporta interpolación `${env:NAME}`, `${userHome}`, `${workspaceFolder}`; para stdio
también `envFile` (los remotos no lo soportan). El `.env` del repo lo levanta igual
`mmorch/providers.py:19` (`load_dotenv()`), así que no hace falta duplicar API keys
en el mcp.json — pero OJO: `load_dotenv()` busca cwd y padres, y Cursor lanza el
proceso con cwd del workspace, no del repo mmorch → conviene pasar `envFile` o un
`cwd` explícito vía wrapper, o hacer que providers.py resuelva el `.env` relativo a
`__file__` (hoy depende del cwd — riesgo real #1 de portabilidad a Cursor).

### A.2 Transportes soportados

Cursor soporta los tres transportes ([docs](https://cursor.com/docs/mcp)):

| Transporte | Config | Notas |
|---|---|---|
| stdio | `command` + `args` (+ `env`/`envFile`) | Cursor gestiona el proceso; es lo que mmorch usa hoy (`mcp.run()` de FastMCP = stdio por default, `mcp_server.py:9`) |
| SSE | `url` | legacy, sigue soportado |
| Streamable HTTP | `url` (+ `headers`) | recomendado para remoto/multi-cliente; OAuth vía objeto `auth` |

FastMCP (el SDK que mmorch ya usa, `mcp_server.py:18,54`) soporta streamable-http con
`mcp.run(transport="streamable-http")` — un flag, no un rewrite. Eso permitiría un solo
proceso mmorch servido a Claude Code + Cursor simultáneamente (hoy stdio = un proceso
por cliente, con locks de DuckDB/SQLite compartidos como riesgo).

Protocolo: Cursor soporta Tools, Prompts, Resources, Roots, Elicitation y MCP Apps;
imágenes base64 devueltas por tools se adjuntan al chat.

### A.3 Límite de tools — el problema concreto para mmorch

- La documentación oficial actual **ya no documenta un límite duro** ([docs](https://cursor.com/docs/mcp)).
- Históricamente Cursor tuvo un **límite/techo práctico de 40 tools activas** (algunas
  tools dejan de estar disponibles al excederlo), ampliamente reportado:
  [forum: MCP Server 40-Tool Limit](https://forum.cursor.com/t/mcp-server-40-tool-limit-in-cursor-is-this-frustrating-your-workflow/81627),
  [forum: Tools limited to 40 total](https://forum.cursor.com/t/tools-limited-to-40-total/67976),
  [análisis del workaround](https://dredyson.com/why-mcps-40-tool-limit-is-too-restrictive-and-how-to-work-around-it/).
- **mmorch expone 46 tools** (contadas: 46 `@mcp.tool()` en `mcp_server.py`) → por
  encima del techo histórico, y aun donde el límite sea blando, 46 schemas degradan la
  selección de tools y comen contexto.

Mitigaciones (en orden de menor esfuerzo):
1. **Toggle por-tool en Cursor**: la UI de Cursor permite deshabilitar tools
   individuales por server — el usuario deja ~25 esenciales.
2. **Perfil "core" del server**: env `MMORCH_MCP_PROFILE=core` que registre solo las
   ~20 tools de uso frecuente (route/fan_out/verify/recall/remember/check/classify...)
   y esconda las de mantenimiento (forget_preview, feedback_stats, budget_status...).
   Cambio chico: condicionar los `@mcp.tool()` al perfil.
3. **Facade** (patrón mcp-hub: `list_tools` + `call_tool`,
   [forum](https://forum.cursor.com/t/unlimited-mcp-tools-break-the-40-tools-limit/78040)) —
   último recurso: pierde schemas tipados, empeora la selección del modelo.

### A.4 Diferencias Cursor vs Claude Code que afectan a mmorch

| Capacidad que mmorch usa hoy | Claude Code | Cursor |
|---|---|---|
| MCP tools stdio | sí | sí — compatible tal cual (módulo el límite de tools y el cwd) |
| **Hooks** (SessionStart/SessionEnd/UserPromptSubmit/PreToolUse/Stop) | sí — mmorch registra 5+ en `~/.claude/settings.json` | **no existe un sistema de hooks equivalente** → autoregister_project, proposal_hook, session_ingest_hook, never-edit-guard, workflow/wayfinder-suggester NO corren en Cursor |
| **Skills / slash-commands** (`~/.claude/skills`, `scripts/sync_skills.py:19`) | sí | no — el equivalente son Cursor Rules (`.cursor/rules/*.mdc`), formato distinto, sin frontmatter de triggers ni Skill tool |
| **Transcripts de sesión** (`~/.claude/projects/*.jsonl`, `mmorch/sessions.py:183`) | sí — base de `mmorch_ingest_session` y del minado de playbooks | no — Cursor no escribe JSONL ahí; el flywheel de ingest queda ciego |
| **CLI headless `claude -p`** (`mmorch/claude_exec.py`) | sí — ejecutor sobre el plan | Cursor tiene `cursor-agent` CLI pero flags/stream-json distintos; `claude_exec` no aplica |
| CLAUDE.md / contexto de proyecto | sí | Cursor usa `.cursor/rules` + AGENTS.md (Cursor lee AGENTS.md — mmorch ya tiene uno en la raíz, punto a favor) |
| Aprobación de tools | permissions de Claude Code | Cursor pide aprobación por default; auto-run configurable ([docs](https://cursor.com/docs/mcp)) |

**Conclusión A:** el MCP server en sí es compatible con Cursor casi sin cambios (stdio +
FastMCP estándar). Lo que NO viaja es todo el exoesqueleto: hooks, skills, ingest de
transcripts y el ejecutor claude_exec — exactamente las mismas piezas que bloquean el
modo standalone (Parte B).

Fuentes A: [cursor.com/docs/mcp](https://cursor.com/docs/mcp) ·
[FastMCP + Cursor](https://gofastmcp.com/integrations/cursor) ·
[truefoundry guía 2026](https://www.truefoundry.com/blog/mcp-servers-in-cursor-setup-configuration-and-security-guide) ·
[forum 40-tool limit](https://forum.cursor.com/t/mcp-server-40-tool-limit-in-cursor-is-this-frustrating-your-workflow/81627)

---

## Parte B — mmorch como sistema propio (independiente de Claude Code)

### B.1 Inventario: qué asume hoy de Claude Code (grep del repo)

**Acoplamientos duros (rompen sin Claude Code):**

| Qué | Dónde | Naturaleza |
|---|---|---|
| Ejecutor `claude -p` headless (stream-json, permission-mode) | `mmorch/claude_exec.py:26-33` (busca `MMORCH_CLAUDE_BIN` o `~/AppData/Roaming/npm/claude.cmd`); usado por `mmorch/project_loop.py:192` y `mmorch/server_engine.py:282` | El coder-loop del project-build engine ejecuta SOBRE el plan de Claude. Sin Claude Code CLI no hay file-tools → habría que abstraer un `Executor` (interfaz: prompt+cwd+mode → result) con backends alternativos (cursor-agent CLI, aider, open-source agent con API propia) |
| Ingest de transcripts | `mmorch/sessions.py:183` (`Path.home()/".claude"/"projects"`, rglob de JSONL con formato interno de Claude Code: sessionId, tool-uses) | El flywheel (playbooks, decisiones, calibración) depende del formato JSONL de Claude Code. Standalone: parser por-fuente o ingestar desde el propio chat del server (`chat.db` ya existe) |
| Hooks registrados en `~/.claude/settings.json` | `scripts/autoregister_project.py` (SessionStart), `scripts/proposal_hook.py` (SessionStart), `scripts/session_ingest_hook.py` (SessionEnd), + JS: workflow-suggester, wayfinder-suggester, never-edit-guard, context-block-watch/reinject en `~/.claude/hooks/` | Todo el nervio "ambiental" (auto-registro de proyectos, inyección de propuestas, minado post-sesión, guardrails) vive en el hook-system de Claude Code. Standalone: los triggers equivalentes serían eventos del propio server (job start/end) o file-watchers |
| Skills | `scripts/sync_skills.py:19` copia repo→`~/.claude/skills/` | Los skills (autoresearch, project, perfect, verify-cross...) son la UI conversacional de mmorch. Standalone: se vuelven subcomandos del CLI o roles del server |
| Ubicación física del repo | Hardcodes `~/.claude/orchestration` en `scripts/autoregister_project.py:10`, `scripts/server_forever.ps1`, `scripts/register-autopull.ps1`, mensajes en `mmorch/providers.py:96`, `mmorch/docgen.py:7` | Mayormente cosmético/scripts, pero el repo VIVE dentro del árbol de config de Claude Code |

**Acoplamiento estructural de packaging (bloquea `pipx install`):**

- ~25 módulos anclan datos al **repo root** con `Path(__file__).resolve().parents[1]`:
  `metrics.py:22` (logs/), `cache.py:16`, `chat_store.py:16`, `memory.py:32`,
  `feedback.py:26`, `evolve.py:31`, `intuition.py:29`, `loop_nightly.py:90`
  (prompts/), `curation.py:16`, `goal.py:19`, `fleet.py:13`, etc.
  Instalado en site-packages, `parents[1]` = `site-packages/` → escribiría logs y DBs
  dentro del site-packages. **Hoy mmorch solo funciona como checkout editable.**
- `mcp_server.py`, `prompts/`, `roles/`, `weights/`, `projects.json`, `prices.json`
  están FUERA del paquete `mmorch` (pyproject: `packages = ["mmorch"]`) → un
  `pip install mmorch` no los instala.

**Lo que ya está resuelto (más de lo que parece):**

- **pyproject.toml completo** (v1.2.0, extras `mcp/memory/checkers/server/factory/host`,
  gates ruff+mypy) con 2 entry points: `mmorch-server` y `mmorch-sync`
  (`pyproject.toml:34-36`).
- **Daemon/scheduler nocturno ya NO depende de Claude Code**: `scripts/nightly.py:1-18`
  corre vía **Windows Task Scheduler** (`schtasks /Create /TN mmorch-nightly /SC DAILY
  /ST 02:10`), invoca la librería directo, cero cupo. Watchdog del server:
  `scripts/server_forever.ps1` (ONLOGON, relanza con backoff 10s — fix medido
  2026-08-14 cuando el server moría con la app de Claude). Autopull cada 15 min:
  `scripts/register-autopull.ps1`.
- **Server HTTP propio** (starlette/uvicorn, `mmorch/server.py:804-833`): ~30 rutas
  (jobs, gates, checkpoints, chat, workflows, plugins, budget, export/import, SSE
  `/events`), auth por `MMORCH_SERVER_TOKEN`, y Lotus (Tauri) como cliente nativo.
- **Config por env ya sistematizada**: 25+ vars `MMORCH_*` (SERVER_TOKEN,
  MAX_MONTHLY_USD, CHAT_DB, WORKFLOW_DB, ROLES_DIR, WORKFLOWS_DIR, CLAUDE_BIN,
  EXEC_POLICY...) — la mitad del trabajo de "relocatable" ya está: falta el default
  raíz único.

### B.2 Qué falta para el standalone (lista concreta)

1. **`MMORCH_HOME`** (default `~/.mmorch` o `%LOCALAPPDATA%/mmorch`): una sola función
   `data_dir()` en config.py que reemplace los ~25 `parents[1]`; logs/DBs/weights ahí,
   prompts/roles como package-data con override. Es EL cambio habilitante de packaging.
2. **Mover `mcp_server.py` adentro del paquete** + entry point
   `mmorch-mcp = "mmorch.mcp_server:main"` → registrable en cualquier cliente como
   `uvx mmorch-mcp` / `pipx run`. (Patrón estándar de servers MCP en PyPI.)
3. **CLI propio** (`mmorch` top-level): hoy solo hay `mmorch-server` y `mmorch-sync`.
   Subcomandos obvios mapeando skills: `mmorch route|verify|fanout|recall|nightly|
   project|status`. stdlib argparse alcanza (coherente con coding-principles).
4. **Abstraer el Executor** (claude_exec → interfaz): el único módulo que NECESITA a
   Claude; backends: claude CLI (default), cursor-agent, API-only (sin file-tools,
   degradado). Seam data-only, testeable con fake.
5. **Triggers propios en vez de hooks**: SessionEnd-ingest → comando `mmorch ingest
   --watch` o evento del server; proposal_hook → endpoint `/proposals` (el server ya
   tiene el bus de eventos SSE).
6. **Scheduler portable**: hoy schtasks/PS1 (Windows-only, ya independiente de Claude).
   Para multi-host: `mmorch nightly install` que registre schtasks en Windows y
   systemd-timer/cron en Linux (el extra `host` ya apunta a deploy multi-host).

### B.3 Prácticas 2026 para MCP servers en producción (vs estado mmorch)

| Práctica esperada | Fuente | Estado mmorch |
|---|---|---|
| Logging/telemetría de cada invocación (tool, latencia, cliente) | [Nordic APIs](https://nordicapis.com/8-tips-and-best-practices-for-mcp-server-development/), [Apigene 12 rules](https://apigene.ai/blog/mcp-best-practices) | **YA CUMPLE**: `mmorch/mcp_telemetry.py` instrumenta las 46 tools → `logs/mcp_calls.jsonl` (`mcp_server.py:55-58`); + `metrics.jsonl` |
| Health check que verifique conectividad upstream, no solo "proceso vivo" (scan abril 2026: 52% de 2181 endpoints MCP remotos muertos, 9% sanos) | [simorconsulting](https://simorconsulting.com/blog/mcp-server-ecosystem-whats-production-ready-in-2026/), [n1n checklist](https://explore.n1n.ai/blog/mcp-server-production-readiness-checklist-2026-03-07) | **FALTA endpoint**: el server HTTP no tiene `/health` (rutas en `server.py:804-833`; `/state` es snapshot de jobs). La lógica ya existe (`mmorch/health.py` probes por proveedor + `intuition.py:79` health_probes.json) — falta exponerla como `GET /health` con status por proveedor |
| Versionado de tools/API con deprecación y tests de compatibilidad | [CData 2026](https://www.cdata.com/blog/mcp-server-best-practices-2026), [Apigene](https://apigene.ai/blog/mcp-best-practices) | Parcial: versión de paquete 1.2.0 en pyproject, pero las tools MCP no declaran versión ni hay política de deprecación; ~718 tests cubren la librería, no el contrato MCP |
| Errores estructurados, credencial-isolation, control por-tool | [modelcontextprotocol.info best practices](https://modelcontextprotocol.info/docs/best-practices/), [200OK enterprise guide](https://www.200oksolutions.com/blog/mcp-servers-for-enterprise-a-practical-build-guide-2026/) | Keys vía `.env` (no viajan en config), budget cap `MMORCH_MAX_MONTHLY_USD`, breaker de salud por modelo — por encima de la media |
| Pin de dependencias / detección de schema drift upstream | [simorconsulting](https://simorconsulting.com/blog/mcp-server-ecosystem-whats-production-ready-in-2026/) | Bounds inferiores en pyproject (sin lockfile); error-rates por modelo (`mmorch_error_rates`) cubren drift funcional |

### B.4 Orden sugerido (dependencias entre pasos)

1. `MMORCH_HOME` + mover mcp_server al paquete (habilita TODO lo demás; pipx/uv/Cursor).
2. Perfil `core` de tools (≤40) + fix del `.env`-por-cwd → registrable en Cursor ya.
3. `GET /health` (exponer health.py) — 20 líneas sobre rutas existentes.
4. CLI `mmorch` (absorbe skills como subcomandos).
5. Executor abstraído (último: es el acople más profundo y el único con valor real
   en quedarse — claude CLI sigue siendo el mejor backend de coding).

---

### Fuentes (todas las URLs)

- https://cursor.com/docs/mcp — config, transportes, OAuth, imágenes, aprobación
- https://gofastmcp.com/integrations/cursor — FastMCP en Cursor
- https://forum.cursor.com/t/mcp-server-40-tool-limit-in-cursor-is-this-frustrating-your-workflow/81627 — límite 40 tools
- https://forum.cursor.com/t/tools-limited-to-40-total/67976 — límite 40 tools (feature request)
- https://forum.cursor.com/t/unlimited-mcp-tools-break-the-40-tools-limit/78040 — workaround hub
- https://dredyson.com/why-mcps-40-tool-limit-is-too-restrictive-and-how-to-work-around-it/ — análisis del límite
- https://www.truefoundry.com/blog/mcp-servers-in-cursor-setup-configuration-and-security-guide — guía setup/seguridad 2026
- https://nordicapis.com/8-tips-and-best-practices-for-mcp-server-development/ — observabilidad desde día uno
- https://www.cdata.com/blog/mcp-server-best-practices-2026 — versionado/deprecación
- https://apigene.ai/blog/mcp-best-practices — 12 reglas producción
- https://simorconsulting.com/blog/mcp-server-ecosystem-whats-production-ready-in-2026/ — scan de salud del ecosistema
- https://modelcontextprotocol.info/docs/best-practices/ — best practices del protocolo
- https://www.200oksolutions.com/blog/mcp-servers-for-enterprise-a-practical-build-guide-2026/ — guía enterprise
- https://explore.n1n.ai/blog/mcp-server-production-readiness-checklist-2026-03-07 — checklist production-readiness
