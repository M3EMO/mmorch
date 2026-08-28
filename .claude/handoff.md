# Handoff — 2026-08-03

## Goal
Vault global end-to-end vía el pipeline wayfinder → to-spec → /project. HECHO y MERGEADO.

## State (todo en branch looped-research-beads, sin pushear desde hace ~16 commits)
- **Vault OPERATIVO**: `vault.write_validated` + `regenerate_moc` mergeados (a9f018d) — construidos por el project-build engine + 2 fixes de review; acceptance `tests/test_vault_write.py` 6/6 (escrito ANTES del build, gate independiente).
- **Primer mapa wayfinder real completado**: `.scratch/vault-global/` — 10 tickets de decisión resueltos, niebla vacía, `spec.md` ready-for-agent. Cada decisión linkea su ticket fuente.
- **Wayfinder = workflow default** (3 capas): hook `wayfinder-suggester.js` (UserPromptSubmit global) + sección CLAUDE.md global + gate Cynefin en /project. Skills pocock vendorizadas en `skills/pocock/` + `scripts/sync_skills.py`.
- **signature fix** (0719bad): op_type por POSICIÓN del match — "validada" suelta ya no pisa "Crear..." (2 jobs escalados lo midieron). Casos en ambos self-checks.
- **Primera adopción cross-proyecto**: sesión de Estudio escribió nota compliance al vault siguiendo la convención (babel 0.488/fid 1.0, el mejor; MOC estudio generado con el código nuevo).
- **Corrida autoresearch babel-prompt EN CURSO** (background): A/B original vs prompt babel-comprimido contra la batería congelada, 8 rondas, journal `logs/ar_babel_prompt.qrf`, branch `mmorch/ar-babel-*` queda solo si iguala/mejora.

## Next
1. Ver resultado de la corrida babel-prompt (journal + branch).
2. Spec pendiente de implementar: tool MCP `mmorch_vault_write` (wrapper fino), legs nightly (babel sweep + charts flint + sync del vault), migración curada con stubs (inventario en ticket 01), backfill tags+gists.
3. Manual usuario: `git push` (16 commits), borrar branches `mmorch/ar-*`/`mmorch/wt-*` viejas mergeadas.

## Decisions
- Acceptance test SIEMPRE escrito por fuera del engine antes de lanzar project-build.
- Task text para el engine: fraseo GENERATE explícito (verbo inicial); verificar `signature(task).op_type` antes de lanzar si hay duda.
- Babel NO comprime instrucciones a ciegas — solo con A/B medido (corrida en curso).

## Read first
- `.scratch/vault-global/spec.md` (la spec completa con links a tickets)
- memoria `wayfinder-pipeline` + `knowledge-vault-plan`
- `logs/ar_babel_prompt.qrf` (si la corrida terminó)
