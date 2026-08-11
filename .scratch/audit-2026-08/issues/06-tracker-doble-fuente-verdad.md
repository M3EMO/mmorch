# Doble fuente de verdad para issues: CLAUDE.md manda beads, issue-tracker.md manda .scratch/

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: S (2 ediciones de doc)
Eje: coherencia
Evidence: orchestration/CLAUDE.md:146-148 · docs/agents/issue-tracker.md:3

Contradicción de contrato en el mismo repo: CLAUDE.md manda bd/beads para issue tracking
durable; issue-tracker.md declara que los issues viven en `.scratch/`. Ambos sistemas
vivos (`.beads/` con estado + `.scratch/` usado por wayfinder), sin regla de partición.

**Fix:** línea de partición explícita en ambos docs (bd = backlog durable;
`.scratch/<effort>/` = mapas wayfinder + tickets del esfuerzo) + regla de promoción a bd.
