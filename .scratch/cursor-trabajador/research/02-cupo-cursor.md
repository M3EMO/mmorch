# 02 — Cupo y contabilidad de uso de `cursor-agent`

Fecha: 2026-09-29. CLI local: `2026.08.25-3e8eec8`.
Método: docs oficiales de Cursor, changelog del CLI, forum con respuesta de staff.
Complemento: `--help` y `--list-models` locales, y lectura del bundle JS instalado.
No se corrió ningún prompt. No se pasó `-p`.

## Conclusión para el ruteo

- `cursor-agent` no gasta cupo del plan Claude. Gasta la cuenta Cursor del usuario.
- Los modelos `claude-*` de Cursor también cobran a Cursor, no a Anthropic.
- El CLI no soporta BYOK. Ningún modo del CLI gasta una key propia de Anthropic u OpenAI.
- El pool más barato es "Cursor Models": Composer 2.5 y Grok 4.5/4.6/4.7.
- Cada run headless reporta tokens en `usage` del evento `result`. Ese evento no reporta costo.

## 1. Modelos que acepta `--model` y pool que consumen

**Respuesta.**
El CLI local lista 246 ids con `--list-models`. El default es `auto`.
Las familias son: Composer, Grok, GPT-5.x/Codex, Claude (Opus, Sonnet, Fable), Gemini, Kimi, GLM, Muse Spark.
Muchos ids tienen variantes de esfuerzo (`-low` a `-max`) y variantes `-fast`.
Los modelos parametrizados aceptan overrides, por ejemplo `'claude-opus-4-8[context=1m,effort=high,fast=false]'`.

Los planes individuales (Pro $20, Pro Plus $60, Ultra $200) tienen dos pools mensuales.
- El pool "Cursor Models" cubre Grok 4.7, Grok 4.6, Grok 4.5 y Composer 2.5.
- El pool "Other Models" cubre todo modelo de terceros, a precio de API del proveedor.
- `auto` cobra al precio del modelo al que rutea cada request.
- Al agotar un pool, el usuario activa "on-demand" a las mismas tarifas, o sube de plan.
- El modo fast de Grok cuesta 2x. El fast con contexto largo cuesta 3x.

**Confianza.** Alta para la lista local y la regla de pools.
Media para el pool exacto de cada id: la doc nombra modelos por familia, no por id del CLI.

**No verificado.** Los montos en dólares incluidos en cada pool por plan. La doc de pricing no los da.
No verificado si `cursor-grok-*` y `grok-*` caen ambos en el pool "Cursor Models".

**Fuentes.**
- Local: `cursor-agent --list-models` (246 ids, "auto - Auto (current, default)").
- https://cursor.com/docs/account/pricing — "There are two separate usage pools, each resetting with your monthly billing cycle".
- https://cursor.com/docs/account/pricing — "usage is drawn from the Other Models pool at that model's API rate".
- https://cursor.com/docs/account/pricing — "All Auto modes bill at the list price of the model".
- https://cursor.com/docs/models — "Grok 4.7, Grok 4.6, Grok 4.5, and Composer 2.5".

## 2. Lectura del consumo por run

**Respuesta.**
El evento final `{"type":"result",...}` trae `duration_ms`, `duration_api_ms`, `session_id` y `request_id`.
El CLI local agrega un objeto opcional `usage` a ese evento en `json` y `stream-json`.
Los campos de `usage` son `inputTokens`, `outputTokens`, `cacheReadTokens` y `cacheWriteTokens`.
El CLI suma `usage` entre turnos. El CLI resta los tokens de caché a `inputTokens`.
El evento `result` no trae costo ni modelo. El modelo aparece en el evento `system`/`init` de `stream-json`.
El costo en dólares por request está solo en el dashboard web (Spending).
El comando interactivo `/usage` muestra medidores del plan y el gasto on-demand.
La Admin API tiene un endpoint de usage events con `chargedCents`, pero la Admin API es de Teams.

**Confianza.** Alta para los nombres de campo: el bundle local los construye.
Media para el comportamiento real: la doc oficial de output-format todavía no documenta `usage`.

**No verificado.** No se observó un `result` real, porque no se corrió ningún prompt.
No verificado si un plan individual accede a la Admin API.

**Fuentes.**
- Local: `versions/2026.08.25-3e8eec8/8475.index.js` arma `{type:"result",...,request_id}` más `{usage}`.
- https://cursor.com/docs/cli/changelog (feb 2026) — "Per-turn input/output/cache token totals and a request_id".
- https://forum.cursor.com/t/include-token-usage-in-stream-json-output/146980 (Alex Wettig, staff, 2026-02-27) — "this should now be part of the last json output".
- https://cursor.com/docs/cli/reference/output-format — el ejemplo de `result` no muestra `usage`.
- https://cursor.com/docs/cli/changelog (2026-07-13) — "See plan usage and spend in /usage".
- https://cursor.com/help/models-and-usage/usage-limits — "for request-level cost and pool details".
- https://cursor.com/docs/account/teams/admin-api — usage events, 60 requests por minuto por team.

## 3. Concurrencia y rate limits del CLI

**Respuesta.**
La doc oficial no publica un límite de concurrencia para el CLI headless.
Un bug viejo tiraba exit 1 al lanzar dos procesos a la vez. Staff lo atribuyó a una race condition.
Una versión de noviembre 2025 corrigió ese bug, según el usuario que lo reportó.
Staff sugirió un delay de ~100 ms entre lanzamientos como workaround.
Desde mayo 2026 el CLI muestra el mensaje real de errores de servidor como rate limits.
Un usuario reportó `resource_exhausted` con llamadas concurrentes. Ningún staff respondió ese hilo.

**Confianza.** Baja. No existe un número documentado.

**No verificado.** El límite real de runs paralelos por cuenta individual.

**Fuentes.**
- https://forum.cursor.com/t/concurrent-headless-cursor-agent-invocations-fail-without-delay/142677 (Dean Rie, staff, 2025-11-15) — "a race condition in cursor-agent when multiple instances start simultaneously".
- https://cursor.com/docs/cli/changelog (2026-05-14) — "classified server errors (like rate limits) render with their real message".
- https://forum.cursor.com/t/cursor-agent-cli-concurrent-call-limit/144782 — sin respuesta de staff.

## 4. BYOK (key propia de Anthropic u OpenAI)

**Respuesta.**
El IDE acepta keys de OpenAI, Anthropic, Google, Azure OpenAI y AWS Bedrock.
El usuario configura BYOK en Cursor Settings > Models.
El CLI no soporta BYOK. El CLI usa solo la suscripción Cursor.
El `--help` local no tiene flag de key de proveedor. `--api-key` es la key de Cursor, no la del proveedor.
Por eso `cursor-agent` nunca gasta la cuenta API de Anthropic del usuario.

**Confianza.** Media-alta. La respuesta de staff es de enero 2026.
El changelog del CLI hasta septiembre 2026 no menciona BYOK.

**No verificado.** Que BYOK siga ausente en la versión 2026.08.25. El `--help` local sugiere que sigue ausente.

**Fuentes.**
- https://forum.cursor.com/t/can-i-use-provider-api-keys-with-cursor-cli-agent/149158 (Dean Rie, staff, 2026-01-17) — "Cursor CLI doesn't support BYOK with your own provider API keys yet".
- https://cursor.com/help/models-and-usage/api-keys — "OpenAI, Anthropic, Google, Azure OpenAI, and AWS Bedrock".
- Local: `cursor-agent --help` — "--api-key <key> API key for authentication (can also use CURSOR_API_KEY env var)".

## 5. Uso desatendido en scripts y CI

**Respuesta.**
- Auth: el usuario genera una user API key en Dashboard → API Keys. El script la pasa por `CURSOR_API_KEY` o `--api-key`.
- Trust: un run headless en un workspace no confiado falla. `--trust` o `--force` evitan ese fallo.
- Escritura: sin `--force`, `-p` solo propone cambios. Con `--force`, `-p` aplica cambios y corre comandos.
- `--sandbox enabled|disabled` y `--approve-mcps` existen en el `--help` local.
- Exit codes: en falla el proceso sale con código distinto de cero y escribe el error en stderr.
- En falla, el modo `json` no emite JSON válido. El wrapper debe leer stderr.
- El bundle local usa `process.exit` con 0, 1, 2 y 130. La doc no define el significado de 2.

**Confianza.** Alta para auth, trust, `--force` y "no cero en falla". Baja para el mapa de códigos.

**No verificado.** El significado del exit code 2. El comportamiento con `CURSOR_API_KEY` en Windows.

**Fuentes.**
- https://cursor.com/docs/cli/reference/authentication — "For automation, scripts, or CI environments, use API key authentication".
- https://cursor.com/docs/cli/headless — "allows the agent to make direct file changes without confirmation".
- https://cursor.com/docs/cli/changelog (feb 2026) — "Non-interactive runs in untrusted workspaces fail with guidance".
- https://cursor.com/docs/cli/reference/output-format — "the process exits with a non-zero code and writes an error message to stderr".
- https://cursor.com/docs/cli/reference/parameters — `--trust`: "Trust the workspace without prompting".

## Lista de no verificados

- Montos incluidos por pool y por plan.
- Pool exacto de cada id del CLI (`cursor-grok-*` vs `grok-*`).
- Un evento `result` real con `usage` (requiere correr un prompt).
- Límite de concurrencia real para el CLI en plan individual.
- Acceso de un plan individual a la Admin API.
- Significado del exit code 2.
