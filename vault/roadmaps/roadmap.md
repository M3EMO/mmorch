# Roadmap (curado — solo cambia con OK de Mateo)

El ARCHIVO es la fuente de verdad. Cualquier vía que lo mueva vale (edición a
mano o "dale la N" en sesión); el nightly registra outcomes comparando diffs y
detecta promociones de candidatas (`mmorch.fuel.detect_promotions`).

Consolidado 2026-08-18 desde la primera curación masiva (22 candidatas
promovidas — dedup de redundantes, agrupado por tema; trazabilidad en los
comentarios de cada línea).

## Direcciones

- **Entrenamiento de IA propia** — flywheel de datos (ya capturando: DPO,
  decisiones, traces) → router aprendido en shadow mode contra eval congelado
  (jamás toma control hasta ganarle al bandit) → reward model destilado del
  refutador → volumen mayor (sintéticos gateados + minería git con reward
  ejecutable + LoRA en GPU rentada; wayfinder antes de escalar).
  <!-- cand-2026-08-14-04, 14-05, 15-07b, 18-01, 18-02 -->

- **Calidad y anti-pseudo-progreso del loop** — SPC (CUSUM+EWMA) sobre
  outcomes; active learning en adjudicación (solo la franja incierta a juicio,
  disenso = prioridad); MAP-Elites por celda proyecto×tipo contra el
  mode-collapse; detector de divergencias engine/logs; refutaciones → tests de
  regresión semántica automáticos.
  <!-- cand-2026-08-15-05b, 15-06b, 15-08, 15-03, 15-04 -->

- **Grafts prime-agent** — rollback estructural (snapshot por edit + inversión
  mecánica) en evolve/close-loop; playbooks ejecutables (reference validado en
  session_skills); review-gate barato pre-persistencia del ingest.
  <!-- cand-2026-08-14-01, 14-02, 14-03 -->

- **Deuda estructural** — unificar los 3 ThompsonBandit (namespacing de brazos,
  evidencia compartida); feedback.jsonl → SQLite con índices cuando el volumen
  lo pida.
  <!-- cand-2026-08-15-01, 15-02 (misma idea, dedup) -->

- **Engine /project** — graft /speckit.analyze: verificación cross-family de
  consistencia spec↔decompose antes de construir.
  <!-- cand-2026-08-12-01 -->

- **Jarvis físico** — Home Assistant + sensores ESP32 vía MCP (medir el mundo,
  actuar con tool-use, percepción preentrenada); wayfinder HITL antes de
  comprar/construir.
  <!-- cand-2026-08-14-06 -->

<!-- Dedup de la curación 2026-08-18: cand-18-03 y 18-04 (variaciones
     router/refutador) absorbidas en "Entrenamiento"; 15-05 y 15-06 (variantes
     nightly de clasificadores sobre snapshots) absorbidas en "Calidad" y
     "Entrenamiento"; 15-07 (routing con feedback implícito) absorbida en
     "Entrenamiento". Nada se perdió: candidatos.md#Archivadas conserva todas
     con estado promovida. -->

<!-- Histórico pre-loop: INNOVATION_ROADMAP_2026-06-07.md (raíz del repo,
     archivado — todo LANDED, no migra nada). -->
