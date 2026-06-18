# mmorch aprende de sesiones de Claude — Diseño v0

Fecha: 2026-06-18
Estado: aprobado (brainstorming) — pendiente plan de implementación

## Problema

Las sesiones de Claude Code son una mina de **labels reales** (¿la tarea salió bien?
¿cómo codeó Opus? ¿qué dificultad tuvo de verdad?) que hoy mmorch no consume. El
diseño de `feedback.py` insiste en que el label venga de afuera, no auto-reportado;
las sesiones son precisamente esa fuente externa. Conectarlas cierra el lazo con datos
reales en vez de rubrics sintéticos.

## Alcance (descomposición)

El pedido completo (outcomes + razonamiento/workflow + trayectorias→skills + calibrar
router) son varios subsistemas. Todos consumen de una **capa de ingesta** común. Por
eso v0 = ingesta + UN sink end-to-end. El resto son slices separados (ver "Después").

## Decisión de arquitectura: extracción

**Parse determinista local + redacción → API externa cross-family.** Lo objetivo
(tests, commits, tool-calls, aceptación del user) se saca con parser local, cero fuga.
El razonamiento/dificultad más rico se extrae mandando segmentos **redactados** a un
modelo barato externo. Si la redacción no es confiable, degrada a solo-determinista.

## Fuente de datos

`~/.claude/projects/<proyecto>/<sesión>.jsonl`. Cada línea = un evento (user msg,
assistant msg, tool_use, tool_result).

## Componente: `mmorch/sessions.py`

Una unidad con un propósito: convertir un JSONL de sesión en señal para los sinks de
mmorch. Interfaz pública: `ingest_session(path | "latest") -> IngestReport`.

### 1. parse_session(jsonl) -> list[Segment]
Segmento ≈ un intento de tarea: `{request, assistant_reasoning, tool_calls[],
tool_results[], user_followups[]}`. Split determinista por mensajes de user / límites
de TodoWrite.

### 2. outcome_of(segment) -> Outcome | None  (el LABEL, anti-sicofancia)
`reward ∈ [0,1]` + `source` + `confidence`. Señales:
- **Positivas:** pytest/tests pasaron en un tool_result; commit exitoso; frases de
  aceptación del **USER** ("funciona", "dale", "perfecto", "gracias").
- **Negativas:** revert/undo; "no", "mal", "rehacé"; error sin resolver al cierre;
  el user re-pregunta lo mismo.
- **INVARIANTE:** nunca usar el "listo/done" del propio Claude como label (cobra
  effect / anti-reward-hacking). Solo señal externa (user o tool determinista).
- Sin señal clara → devuelve `None` (abstiene, no registra). Mejor no-dato que dato
  sucio.

### 3. redact(text) -> (texto_redactado, confianza)  (gate de privacidad)
Antes de CUALQUIER call externo: regex de API keys, valores de `.env`, tokens, emails,
paths de home, y patrones de secreto comunes. Redacta-ante-duda. Si `confianza` baja →
el segmento NO se manda a API; se usa solo la señal determinista.

### 4. estimate_difficulty(segment) -> Difficulty  (enriquecimiento, gated)
La dificultad REAL observada, no la predicha. Determinista primero (n tool-calls,
n iteraciones, hubo reverts); opcionalmente afinada por el modelo externo sobre el
segmento redactado. Mapea a un dominio Cynefin observado: 1 tool-call ≈ clear; muchas
iteraciones/reverts ≈ complex/chaotic.

### 5. Sink v0: calibrar el router (usa `feedback.py`)
Por segmento con outcome:
- `cynefin_classify(request)` → dominio **predicho** (lo que mmorch habría dicho).
- `estimate_difficulty` → dominio **observado** (lo que pasó de verdad).
- `feedback.record_outcome(arm=dominio_predicho, reward=f(acertó_dominio, outcome),
  source="claude_session")`.
- `learn.py` ya consume estos outcomes → recomienda ajustar thresholds del router.

Resultado: mmorch aprende si su clasificador barato coincide con cómo Opus realmente
resolvió la tarea, con datos reales.

### Idempotencia
Registrar el hash de cada sesión ingerida (marker file o tabla); re-correr `ingest`
sobre la misma sesión no debe doble-contar.

### Trigger (v0)
Manual: MCP `mmorch_ingest_session(path|"latest")` + opcional comando `/mmorch-learn`.
El hook SessionEnd automático se difiere hasta probar redacción + calidad de labels.

## Invariantes (resumen)
1. Label de señal externa only (anti-sicofancia).
2. Outcome determinista, nunca LLM-judge del éxito (anti-reward-hacking, cobra effect).
3. Redacción antes de cualquier call externo; degrada a determinista si dudosa.
4. Idempotente por hash de sesión.
5. Abstención > dato sucio.

## Testing
Fixture JSONL con dos segmentos (uno test-passed, uno reverted):
- el test-passed produce reward alto; el reverted reward bajo/cero.
- una API-key falsa plantada se redacta antes de cualquier salida.
- un segmento sin señal de outcome → no registra nada.
- re-ingerir la misma sesión → no doble-cuenta.

## Después (slices separados, fuera de v0)
- Trayectorias exitosas → `trajectory.distill_skill` (handbooks reusables).
- Reasoning-store: guardar el razonamiento/workflow redactado en `memory` semántica.
- Hook `SessionEnd` para ingesta automática incremental.

## No-objetivos de v0
- No sink de preferencias/correcciones (el user no lo pidió en esta tanda).
- No ingesta automática.
- No mandar transcripts crudos a API externa nunca.
