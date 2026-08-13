# Mapa wayfinder: loop cerrado mmorch (sistema que se mueve solo)

Label: wayfinder:map
Creado: 2026-08-12

## Destination

Spec de arquitectura del **loop cerrado completo** de mmorch, listo para que
`/project` lo construya por fases: fuel de ideas renovado (roadmap vivo, no el
de junio) → adjudicación de notas del vault a proyectos → propuesta inyectada
en sesión vía hook (+ digest en resumen matutino 09:10) → captura de outcome
híbrida (explícita al reaccionar, N ignoradas = rechazo blando) → aceptadas se
convierten en trabajo (chica→chip, grande→beads+wayfinder) → outcomes alimentan
bandit/evolve. Todo debe moverse a partir del uso normal, sin iniciación manual.

## Notes

- Dominio: mmorch (`~/.claude/orchestration`), hooks globales de `~/.claude`,
  vault global, beads.
- Decisiones fijadas en el grilling de charteo (2026-08-12):
  - Destino = spec (handoff a /project), no implementación en el mapa.
  - Canal de propuesta: inyección en sesión vía hook; digest matutino además.
  - Outcome: híbrido (explícito + ignorar N veces = rechazo blando).
  - Alcance: incluye renovar la fuente de ideas (roadmap/fuel), no solo los 3
    eslabones nuevos.
  - Unidad de proyecto: registry de mmorch como universo + codegraph como
    enriquecedor donde exista índice.
  - Presupuesto de propuestas: máx 1 por sesión con match fuerte + digest 09:10.
  - Idea aceptada: chica → task chip inmediato; grande/foggy → issue beads con
    nudge wayfinder.
  - Ejecución de aceptadas SIEMPRE en sandbox (worktree/branch aislado, merge
    solo con revisión humana) — nunca sobre main ni el working tree en uso.
- Skills a consultar por sesión: /grilling, /domain-modeling; generación barata
  vía mmorch (cross-family, verificador refuta).
- Guardrail: correr Workflow solo con opt-in explícito; adjudicación/narración
  siempre por modelos baratos (DeepSeek/Gemini), nunca cupo Claude.

## Decisions so far

<!-- una línea por ticket cerrado: gist + link -->
- [01 Inventario del estado actual](issues/01-inventario-estado-actual.md) — innovate no consume ningún roadmap (el de junio es output muerto, evolve_open_prs `{}`); el único hook que hoy inyecta al contexto es SessionStart:compact (reinject, 15s) — Stop solo escribe stderr; registry = 9 proyectos planos name→path (3 con codegraph); record_outcome ya tiene 7 callers y 3 bandits separados (default/sig/workflow).
- [02 Fuel: roadmap vivo](issues/02-fuel-roadmap-vivo.md) — dos artefactos: roadmap.md curado (solo cambia con OK; el archivo es la verdad, nightly registra outcomes por diff) + candidatos.md derivado en batch (lentes fijos gateados por fuel nuevo, máx 5, puede ser 0); junio se archiva; candidatas expiran a 14 días como rechazo blando.
- [03 Esquema de adjudicación](issues/03-esquema-adjudicacion.md) — adjudications.json índice + frontmatter espejo; DeepSeek juzga / Gemini refuta, ≥0.7 = fuerte; nightly incremental (hash de nota, sin re-juzgar); codegraph enriquece el citado a archivo/módulo donde hay índice.
- [04 Formato de la propuesta](issues/04-formato-propuesta.md) — hook SessionStart nuevo (texto pre-cocinado por nightly, cero API en arranque); tarjeta 2-3 líneas con dale/no/ignorar; digest = 1 línea por idea + vencimientos, "ampliá la N" la rinde como tarjeta completa.
- [05 Captura de outcome híbrida](issues/05-captura-outcome.md) — sesión registra en vivo + SessionEnd barre (dedup por id); brazo = fuente (nota vs roadmap-lente); estado en adjudications.json; N=5 sin reacción = rechazo blando; rewards dale 1.0 / no 0.125 / blando 0.2.
- [06 Pipeline de idea aceptada](issues/06-pipeline-aceptacion.md) — sesión clasifica al "dale" (cynefin); sandbox = worktree+review branch del engine /project, merge solo humano; poda a 14 días; issue beads con prompt autónomo + id; doble señal (aceptó 1.0 / mergeó 1.0 / abandonó 0.2); nota → applied al mergear.
- [07 Guardrails y presupuesto](issues/07-guardrails-presupuesto.md) — USD 3/mes tope (frena solo); kill-switch = flag logs/loop_paused; caída de un family = skip + aviso en digest (jamás single-family); nunca mergea/pushea/manda externo; métricas de salud semanales en el digest del lunes.

## Not yet specified

(vacío — el camino está claro; ver spec.md)

## Out of scope

- Ejecutar la implementación dentro de este mapa (el destino es el spec).
- Extensión a la máquina del trabajo (setup-trabajo excluye hooks y nightly
  deliberadamente) — esfuerzo aparte si algún día se quiere.
