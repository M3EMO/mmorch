# Contratos técnicos y estándares para mmorch production-grade

> Investigación externa, 2026-08-27. Cada sección: estado del estándar (fuente oficial) + gap concreto de mmorch (estado actual asumido: rutas `~/.claude` hardcodeadas, ejecución acoplada a una sesión de Claude Code, MCP server sobre stdio).

---

## 1. Spec MCP — versión 2026-07-28 (BREAKING) y roadmap

### Estado del estándar

- **Versión vigente: `2026-07-28`** (publicada tal fecha; RC congelado el 2026-05-21). Anuncio oficial: https://blog.modelcontextprotocol.io/posts/2026-07-28/ y RC: https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/
- **El protocolo se volvió STATELESS en el core** (tema central del roadmap 2026: https://blog.modelcontextprotocol.io/posts/2026-mcp-roadmap/):
  - **Handshake `initialize`/`initialized` eliminado** (SEP-2575). Versión de protocolo, identidad y capabilities del cliente ahora viajan en `_meta` de **cada request**. Descubrimiento opcional vía RPC `server/discover`.
  - **`Mcp-Session-Id` eliminado** en Streamable HTTP (SEP-2567); requests auto-contenidos; se removió el endpoint GET de stream.
  - **Headers obligatorios `Mcp-Method` y `Mcp-Name`** en Streamable HTTP (SEP-2243) para que gateways/load-balancers ruteen sin parsear el body. Ver spec de transporte: https://modelcontextprotocol.io/specification/draft/basic/transports/streamable-http
  - **MRTR (Multi Round-Trip Requests)** reemplaza streams iniciados por el server (SEP-2322): el server responde `resultType: "input_required"` y el cliente reintenta con `inputResponses` — habilita confirmaciones mid-call sin conexiones abiertas.
  - **Cache declarativo**: `tools/list`, `prompts/list`, `resources/list`, `resources/read` llevan `ttlMs` y `cacheScope` (SEP-2549).
  - **Deprecados con ventana mínima de 12 meses** (SEP-2577): **Roots, Sampling, Logging** del core, y el transporte legacy HTTP+SSE. Tasks se movió a extensión `io.modelcontextprotocol/tasks` (SEP-2663, poll con `tasks/get`/`tasks/update`).
  - **Authorization endurecida**: validación de issuer RFC 9207 (SEP-2468), credenciales ligadas al issuer sin reuso cross-server (SEP-2352), Dynamic Client Registration deprecado a favor de CIMD.
  - **Política formal de deprecación**: features deprecadas siguen funcionando ≥12 meses; extensiones son el mecanismo de evolución. Roadmap: https://modelcontextprotocol.io/development/roadmap
- **SDKs**: python-sdk **v2** es la línea que habla 2026-07-28. `FastMCP` (in-SDK) se renombró **`MCPServer`**; opciones de transporte pasan del constructor a `run()`; la API de decoradores se mantiene. **Un MCPServer sirve ambas eras de protocolo** (clientes 2025 siguen funcionando; clientes nuevos hacen fallback al handshake contra servers viejos). La línea 1.x queda en maintenance/security-only: si no migrás ya, **pinnear `mcp>=1.28,<2`**. Fuente: https://blog.modelcontextprotocol.io/posts/sdk-betas-2026-07-28/ y resumen técnico https://www.developersdigest.tech/blog/mcp-2026-07-28-breaking-changes

### Gap mmorch

1. **Versión de SDK**: mmorch corre sobre la línea 1.x del python-sdk (era 2025). Acción inmediata barata: pinnear `mcp>=1.28,<2` en deps para no romper por un upgrade accidental. Acción production-grade: migrar a v2 (`FastMCP`→`MCPServer`, transporte en `run()`), lo que además da compat dual-era gratis.
2. **Supuestos de sesión**: cualquier lógica de mmorch que dependa del handshake `initialize` (p.ej. leer client capabilities una vez al inicio y cachearlas en estado del server) rompe bajo 2026-07-28 — las capabilities llegan per-request en `_meta`. Auditar handlers por estado per-sesión.
3. **Si mmorch usa Sampling/Roots/Logging del protocolo** (p.ej. logging MCP hacia el cliente): están deprecados; reloj de 12 meses corriendo. Migrar logging a archivo propio (ver §3) y sampling a llamadas API directas (que mmorch ya hace vía DeepSeek/Gemini).
4. **stdio sigue siendo transporte válido** y es el correcto para el uso actual (server local lanzado por Claude Code). Pero para el objetivo "loops nocturnos sin sesión de Claude" (§4), exponer además Streamable HTTP (post-migración v2) permite que un daemon local y la sesión interactiva hablen con **el mismo proceso** en vez de dos instancias con dos DuckDB locks.

---

## 2. Packaging Python moderno — PEP 621 / uv / entry points / versionado

### Estado del estándar

- **PEP 621** (metadata en `[project]` de `pyproject.toml`) es el estándar canónico: https://peps.python.org/pep-0621/ · guía oficial: https://packaging.python.org/en/latest/guides/writing-pyproject-toml/
  - Obligatorio: `name`, `version` (o `dynamic = ["version"]` para versionar desde git tag / `__version__`).
  - Recomendado: `requires-python`, `license` como **expresión SPDX** (`"MIT"`), `readme`, `dependencies`, `[project.optional-dependencies]`.
  - **CLI como entry point**: `[project.scripts]` → `mmorch = "mmorch.cli:main"` genera el ejecutable en PATH al instalar. Plugins de terceros via `[project.entry-points."mmorch.plugins"]`.
  - `[build-system]` con backend moderno: `hatchling` o `uv_build`.
- **uv es el tooling por defecto 2026**: `uv init --app` scaffoldea src-layout + `uv_build` + entry point; `uv init --lib` para librería. Docs: https://docs.astral.sh/uv/concepts/projects/config/
- **`uv tool install` reemplaza a pipx** para instalar CLIs en venvs aislados con entry points en PATH (10-100× más rápido, gestiona la versión de Python con `--python 3.13`); pipx solo conserva nicho (`inject`, `--suffix`, `install-all`). Comparación honesta del propio pipx: https://pipx.pypa.io/latest/explanation/comparisons.html y https://pydevtools.com/handbook/explanation/how-do-uv-tool-and-pipx-compare/
- **Versionado**: Python usa PEP 440 (canónico: https://peps.python.org/pep-0440/), cuyo core `MAJOR.MINOR.PATCH` es compatible con SemVer (diferencias: `2.0.0rc1` no `2.0.0-rc.1`; sin `+build` público). Adoptar SemVer dentro de PEP 440 + changelog habilita que consumidores usen `~=` (compatible-release). Ref: https://pydevtools.com/handbook/explanation/versioning-python-packages-semver-calver-and-pep-440/

### Gap mmorch

1. mmorch vive en `~/.claude/orchestration/` como árbol de scripts, no como paquete instalable. Production-grade = `pyproject.toml` PEP 621 con `[project.scripts]` (`mmorch = ...`, `mmorch-nightly = ...`) e instalación via `uv tool install --editable ~/.claude/orchestration` (o `uv pip install -e`). Eso da: CLI en PATH independiente de cwd, deps declaradas y lockeadas (`uv.lock`), y un `mmorch-nightly` invocable por Task Scheduler sin conocer rutas internas (§4).
2. Sin versión declarada no hay contrato: adoptar semver PEP 440 (`0.x` mientras la API MCP interna cambie; bump MAJOR cuando cambie el schema DuckDB o el contrato de tools) + `CHANGELOG.md`. El versionado del **schema de DuckDB** es el semver que más importa: agregar tabla de `schema_version` y migraciones idempotentes.
3. `requires-python` explícito + `uv.lock` commiteado convierte "funciona en mi máquina" en reproducible — prerequisito para el ExpertBook futuro (memoria: migración de hardware planeada).

---

## 3. Configuración — XDG/platformdirs vs `~/.claude` hardcodeado; `.env` vs keyring

### Estado del estándar

- **XDG Base Directory** es el estándar Linux (https://wiki.archlinux.org/title/XDG_Base_Directory); **Windows nunca lo adoptó** — usa `%APPDATA%` (roaming), `%LOCALAPPDATA%`, `%PROGRAMDATA%`.
- **La práctica estándar Python es `platformdirs`** (https://platformdirs.readthedocs.io/en/latest/explanation.html): `user_config_dir` / `user_data_dir` / `user_cache_dir` / `user_state_dir` / `user_log_dir` devuelven el path correcto por OS. En Windows: `AppData\Local\<App>` (data/config/state), `...\Cache`, `...\Logs`; `roaming=True` → `AppData\Roaming`. XDG env vars **no** se honran en Windows (override propio `WIN_PD_OVERRIDE_*`).
- **Separación semántica que importa**: config (editada por humano) ≠ data (DuckDB, vault) ≠ cache (regenerable, borrable) ≠ state/logs. XDG la impone; hardcodear un solo dir la pierde.
- **Secrets**: `.env` es texto plano legible por cualquier proceso/backup y propenso a commitearse; `keyring` (https://pypi.org/project/keyring/) usa el almacén nativo del OS — en Windows, **Credential Manager cifrado con DPAPI** por usuario. Refs: https://swharden.com/blog/2021-05-15-python-credentials/ · https://medium.com/@forsytheryan/securely-storing-credentials-in-python-with-keyring-d8972c3bd25f
  - Matiz real: keyring requiere sesión de usuario (los servicios como SYSTEM no ven el credential store del usuario) — relevante para §4: si el nightly corre como el usuario map12 (recomendado), keyring funciona; como SYSTEM, no.

### Gap mmorch

1. `~/.claude/orchestration/` hardcodeado mezcla código+config+data+cache en un dir que **pertenece a otra app** (Claude Code). Riesgo concreto: una limpieza/migración de `~/.claude` (ya hubo poda auditada 2026-06) puede llevarse la DuckDB. Camino incremental sin big-bang:
   - Introducir `MMORCH_HOME` (env var) con default = ruta actual → todas las rutas pasan por **una** función `paths.py` que usa `platformdirs` si `MMORCH_HOME` no está seteado.
   - Prioridad de migración: primero **data** (DuckDB episódica/semántica → `user_data_dir("mmorch")`), después cache (embeddings, respuestas), config al final. El vault Obsidian puede quedarse donde está (es contenido del usuario, no state de la app) — solo su ruta va a config.
2. API keys de DeepSeek/Gemini: si hoy viven en `.env`/JSON plano bajo `~/.claude`, mover a `keyring.set_password("mmorch", "deepseek")` con **fallback a env var** (`MMORCH_DEEPSEEK_KEY`) para contextos sin credential store. Patrón: keyring primero, env var después, error claro al final. Nunca la key en config file.
3. Config legible: un `config.toml` en `user_config_dir` (TOML: stdlib `tomllib` lo lee gratis en 3.11+), validado con schema al cargar (§5). Nada de config viva solo en constantes Python.

---

## 4. Daemon/scheduler multiplataforma — loops nocturnos en Windows

### Estado del estándar (pros/contras reales)

| Opción | Pros | Contras |
|---|---|---|
| **Task Scheduler** (nativo) | Sobrevive reboots y crashes; cero proceso residente; `Wake the computer to run this task` (Conditions tab) puede despertar de sleep; "Run whether user is logged on or not"; registrable por código (`schtasks` o API COM) | Windows-only; config vive fuera del repo (mitigable: script de registro idempotente); "run whether logged on" corre sin desktop y con quirks de permisos documentados (https://learn.microsoft.com/en-us/answers/questions/1032778/) |
| **Servicio Windows** | Siempre vivo, gestión via SCM | Setup complejo (pywin32/NSSM), admin requerido, corre como SYSTEM → **no ve keyring del usuario**, overkill para un job/noche |
| **APScheduler** (https://github.com/agronholm/apscheduler) | Cross-platform, triggers cron/interval en Python puro, jobstore persistente re-ejecuta jobs perdidos | **No es daemon ni servicio**: necesita un proceso Python vivo; si nadie lo lanza tras reboot, no corre nada. En una laptop 8GB que duerme, un proceso residente permanente es exactamente lo que no querés |

- Comando de registro (fuente: https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/schtasks-create):
  `schtasks /create /tn "mmorch-nightly" /tr "C:\...\mmorch-nightly.exe" /sc daily /st 03:00`
  (+ activar wake y "start when available" via XML o la API `win32com`/`Register-ScheduledTask` de PowerShell, que exponen `WakeToRun` y `StartWhenAvailable`).

### Veredicto para mmorch

**Task Scheduler invocando el entry point del paquete (§2), corriendo como el usuario logueado.** Razones: el trabajo es batch nocturno discreto (consolidate, evolve_nightly, distill_backlog, decay), no un servicio; sobrevive a reboots; no consume RAM de día (8GB = cuello, memoria de hardware); y como tarea del usuario mantiene acceso a keyring y a `%LOCALAPPDATA%`. APScheduler queda como **detalle interno** solo si algún día mmorch corre como server HTTP persistente (§1.4) — no como capa de scheduling del sistema. Requisitos del job para ser production-grade:
1. **Idempotente y re-entrante**: `StartWhenAvailable=true` re-lanza tras un sleep perdido; el job debe poder correr dos veces sin duplicar (marcar "última noche procesada" en DuckDB).
2. **Lock de instancia única** (lockfile o `duckdb` exclusive) — Task Scheduler puede solaparse con una sesión interactiva usando la misma DB.
3. **Exit codes reales + log a `user_log_dir`** — Task Scheduler registra el exit code; hoy un fallo nocturno sería invisible.
4. Script `mmorch install-schedule` que registre/actualice la tarea (idempotente) en vez de instrucciones manuales.

---

## 5. JSON Schema / validación en bordes

### Estado del estándar

- **Patrón canónico: "validate at the boundaries, construct internally"** — parsear/validar TODO input externo (LLM output, config, MCP params, filas de DuckDB viejas) en el borde; tipos internos ya validados no se re-validan. Refs: https://oneuptime.com/blog/post/2026-01-21-python-pydantic-v2-validation/view · https://superjson.ai/blog/2025-08-24-json-schema-validation-python-pydantic-guide/
- **Pydantic v2** (core en Rust, 5-50× v1) es el estándar de facto:
  - `Model.model_validate_json(s)` — parse+validate en un paso (más rápido y estricto que `json.loads` + validate).
  - `TypeAdapter` creado **una vez y reusado** para tipos no-modelo (`list[X]`) — cachea el schema compilado.
  - `Model.model_json_schema()` genera JSON Schema (draft 2020-12) desde el modelo — el mismo modelo sirve de contrato para tools MCP y para structured output de LLMs.
- Para validar **contra** un JSON Schema externo (p.ej. el schema de un tool MCP recibido): librería `jsonschema`; Pydantic no valida instancias contra schemas arbitrarios (https://github.com/pydantic/pydantic/discussions/5135).
- El SDK MCP ya deriva los `inputSchema` de los tools desde type hints — la fuente de verdad debe ser una: el modelo Pydantic.

### Gap mmorch

1. **El borde más peligroso de mmorch es el output de DeepSeek/Gemini** (glm-4.6 midió 34% err, memoria 2026-07). Todo JSON devuelto por un modelo barato debe pasar por `model_validate_json` con reintento-con-error-feedback (patrón Instructor, ya evaluado en libs research: robar patrón, no adoptar). Un veredicto de verificador malformado que se cuela sin validar envenena bandit + memoria.
2. **Bordes internos que hoy son de confianza implícita**: filas DuckDB escritas por versiones viejas del schema, notas del vault, `config.toml` (§3). Cada uno necesita su modelo Pydantic en el punto de carga — no validación dispersa ad-hoc.
3. Definir los contratos de tools MCP **solo** como modelos Pydantic y dejar que el SDK derive el schema: elimina drift entre docstring, schema anunciado y validación real.
4. Costo: validar en el borde una vez es barato (core Rust); lo caro es re-validar internamente — no envolver hot paths internos en modelos.

---

## Fuentes principales (leídas)

1. https://blog.modelcontextprotocol.io/posts/2026-07-28/ (spec 2026-07-28, fetch completo)
2. https://blog.modelcontextprotocol.io/posts/2026-07-28-release-candidate/
3. https://blog.modelcontextprotocol.io/posts/sdk-betas-2026-07-28/ (SDK v2, FastMCP→MCPServer)
4. https://modelcontextprotocol.io/specification/draft/basic/transports/streamable-http
5. https://modelcontextprotocol.io/development/roadmap + https://blog.modelcontextprotocol.io/posts/2026-mcp-roadmap/
6. https://www.developersdigest.tech/blog/mcp-2026-07-28-breaking-changes
7. https://peps.python.org/pep-0621/ · https://packaging.python.org/en/latest/guides/writing-pyproject-toml/ (fetch completo)
8. https://peps.python.org/pep-0440/ · https://pydevtools.com/handbook/explanation/versioning-python-packages-semver-calver-and-pep-440/
9. https://docs.astral.sh/uv/concepts/projects/config/ · https://pipx.pypa.io/latest/explanation/comparisons.html · https://pydevtools.com/handbook/explanation/how-do-uv-tool-and-pipx-compare/
10. https://platformdirs.readthedocs.io/en/latest/explanation.html (fetch completo) · https://wiki.archlinux.org/title/XDG_Base_Directory
11. https://swharden.com/blog/2021-05-15-python-credentials/ · https://medium.com/@forsytheryan/securely-storing-credentials-in-python-with-keyring-d8972c3bd25f
12. https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/schtasks-create · https://learn.microsoft.com/en-us/answers/questions/1032778/ · https://github.com/agronholm/apscheduler
13. https://oneuptime.com/blog/post/2026-01-21-python-pydantic-v2-validation/view · https://superjson.ai/blog/2025-08-24-json-schema-validation-python-pydantic-guide/ · https://github.com/pydantic/pydantic/discussions/5135
