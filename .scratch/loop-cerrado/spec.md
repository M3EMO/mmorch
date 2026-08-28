# Spec: loop cerrado mmorch — sistema que se mueve solo

Destino del mapa wayfinder `loop-cerrado` (2026-08-12). Todas las decisiones
tienen su detalle en `issues/01-07`; esto es la síntesis buildable para
`/project`. Idioma del sistema: español (digest, tarjetas).

## Arquitectura (el ciclo)

```
uso normal (sesiones Claude)
  → ingest            [existe] session_ingest_hook SessionEnd
  → fuel              [nuevo]  roadmap.md curado + candidatos.md derivado en batch
  → adjudicación      [nuevo]  nightly: vault notes × registry, juez+refutador
  → propuesta         [nuevo]  hook SessionStart (tarjeta pre-cocinada) + digest 09:10
  → outcome           [nuevo]  dale/no/ignorar → record_outcome (brazo=fuente)
  → trabajo           [reusa]  chip→worktree sandbox / beads→wayfinder→/project
  → aprendizaje       [existe] ThompsonBandit + doble señal al mergear
```

Todo LLM = DeepSeek/Gemini cross-family (juez propone, refutador refuta),
jamás cupo Claude. El nightly existente (`scripts/nightly.py`, 02:10) es el
host de todos los pasos batch.

## Componentes

### 1. Fuel (ticket 02)

- `roadmap.md` (raíz vault o repo — decidir en build): CURADO, solo cambia con
  OK del usuario; el archivo es la verdad; nightly registra outcomes por diff.
- `candidatos.md`: derivado. Nightly genera direcciones en batch — 4 lentes
  fijos (deuda técnica / capacidad nueva / integración entre proyectos / notas
  huérfanas), máx 5/noche, gateado por fuel nuevo (sesiones/notas/open
  loops/beads desde el último ciclo; sin fuel = 0). Dedup contra todo lo visto.
- `INNOVATION_ROADMAP_2026-06-07.md` → archivar como histórico.
- Candidatas intactas expiran a 14 días = rechazo blando al bandit.

### 2. Adjudicación (ticket 03)

- Nightly incremental: pares nuevos (nota nueva × proyectos, proyecto nuevo ×
  notas); re-juzga solo si cambió el hash de la nota. Universo = registry
  `projects.json`; codegraph enriquece (mapa de módulos al juez Y refutador,
  match cita archivo/módulo) donde hay `.codegraph/`.
- Score 0-1 con justificación; sobrevive refutación y ≥0.7 = match fuerte.
- Storage: `logs/adjudications.json` (índice keyed por proyecto, atómico via
  iohelpers, fuente de verdad) + frontmatter espejo `applies_to:` en la nota.
  Incluye estado de propuestas: `id, status (pendiente|aceptada|rechazada|
  expirada), shown_count`.

### 3. Propuesta (ticket 04)

- Hook SessionStart NUEVO (matcher startup, ~10s budget, corre en ms): lee
  `adjudications.json`, si hay match fuerte ≥0.7 con el proyecto del cwd emite
  por stdout UNA tarjeta pre-cocinada (el nightly la redactó; el hook no llama
  LLMs). Máx 1 por sesión. Incrementa `shown_count`.
- Tarjeta: 💡 + link nota + qué aplica y dónde (archivo si codegraph) + score
  + "refutado y sobrevivió" + acciones dale/no/ignorar + instrucción a la
  sesión de registrar outcome.
- Digest 09:10 (scheduled task existente `mmorch-evolve-nightly`, extender su
  SKILL.md): sección "Ideas pendientes" — 1 línea/idea (proyecto ← fuente,
  gist, score, nueva/vence en N días). "ampliá la N" rinde tarjeta completa.

### 4. Outcome (ticket 05)

- Doble vía: la sesión llama `mmorch_record_outcome` al recibir dale/no; hook
  SessionEnd barre transcript por patrón como red. Dedup por id.
- Brazo = FUENTE: `propuesta:nota` | `propuesta:roadmap-<lente>`.
- N=5 mostrada sin reacción → rechazo blando. Rewards: dale 1.0 · no explícito
  0.125 · blando (ignorada/expirada) 0.2.

### 5. Trabajo (ticket 06)

- Al "dale", la sesión clasifica con `mmorch_cynefin`: chica → task chip que
  corre en worktree sandbox (mecanismo del engine /project: worktree aislado +
  review branch, merge SOLO humano); grande/foggy → issue beads (título
  imperativo, prompt autónomo con paths + link nota + score + id) con nudge
  wayfinder.
- Nightly poda worktrees/branches abandonados a 14 días (→ blando 0.2).
- Segundo evento al terminar: mergeada 1.0 `source=merge` (patrón
  reap_merged_prs); nota origen → `status: applied`.

### 6. Guardrails (ticket 07)

- Tope USD 3/mes del loop de ideas (budget.py); al tocarlo se frena solo.
- Kill-switch: flag `logs/loop_paused` — existe = nightly saltea el loop.
- Family caído → skip corrida + aviso en digest; JAMÁS single-family.
- Nunca solo: no mergea, no pushea, no manda externo, no toca roadmap.md ni
  código fuera de worktrees.
- Digest del lunes: métricas (propuestas/sem, tasa aceptación, mergeadas vs
  abandonadas, notas huérfanas, candidatas vencidas).

## Fases de build sugeridas (para /project)

1. **F1 — adjudicación + storage**: juez/refutador nightly, adjudications.json
   + espejo frontmatter, tests con fixtures. (Cierra el eslabón 1.)
2. **F2 — propuesta**: redacción de tarjetas en nightly, hook SessionStart,
   extensión del digest. (El usuario empieza a VER el loop.)
3. **F3 — outcome**: registro en vivo + barrido SessionEnd, brazo fuente,
   rewards, N=5/14d. (El bandit empieza a comer.)
4. **F4 — fuel**: roadmap.md/candidatos.md, lentes gateados, diff-outcomes,
   archivo de junio.
5. **F5 — trabajo + guardrails**: cynefin al dale, chip/beads, poda, budget,
   kill-switch, métricas del lunes.

Cada fase = módulo puro → wire → self-check → commit (OCP por adición).
Acceptance por fase en el decompose del engine.
