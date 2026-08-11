# Handoff — auditoría EJE=coherencia (2026-08-10)

## Qué se cubrió
- Gates estáticos: ruff 0 / mypy 0 (venv del repo, 100 archivos) — sin regresión.
- Contrato proyecto↔mmorch definido y verificado (informe, sección 1): projects.json (registro/
  frontera) + CLAUDE.md por proyecto + hooks globales + superficie MCP (46 tools) + consumo HTTP.
- Consumidores: Lotus (profundo — api.js/sse/store vs rutas de server.py, 1:1 sin fantasmas);
  bitterbot-desktop y caveman-upstream descartados como consumidores (cero referencias mmorch).
- Skill /new-project contra el contrato nativo (cubre registro+vault+beads; gaps menores anotados).
- Hooks globales y skills: pasada liviana; wiring en settings.json coherente; sync_skills sin drift.
- Dup cross-repo: ninguno real (hillclimb.js = espejo deliberado declarado; mock.js = fallback).
- 2 rondas secas de búsqueda extra sin hallazgos nuevos (inventario tools vs docs/skills;
  endpoints bidireccionales).
- Verificación adversarial cross-family de los 6 candidatos (2 rondas para H1/H2).

## Resultado
0 BLOCKER · 2 IMPORTANTE (drift CLAUDE.md contrato; doble tracker bd vs .scratch) ·
2 NICE (higiene projects.json; Lotus sin CLAUDE.md) · 2 descartados.

## Qué quedó fuera
- Contenido del vault, backups, .scratch (excluidos por mandato).
- Calidad interna de módulos mmorch (no es este eje).
- No se corrieron los 197 tests (gates estáticos solamente; el eje es coherencia, no robustez).
- experimentoTrabajo / Portfolio financiero / Estudio: fuera del alcance declarado
  (Desktop\Claude), aunque figuran en projects.json — si otro eje audita consumidores, empezar ahí.

## Señales para otros ejes
- **seguridad**: (1) `projects.json` registra el HOME DIR completo ("map12") y todo Desktop como
  proyectos job-controlables — la frontera de escritura de `resolve()` (`mmorch/projects.py`)
  abarca todo el perfil de usuario; el hook autoregister (`scripts/autoregister_project.py`) lo
  hace sin filtro. (2) Lotus manda el token por query param en GETs y SSE
  (`Lotus/src/lib/api.js:31,103` `?token=`) — queda en logs/history; server.py:742 declara que
  "el gate real es token + tunnel privado", validar esa premisa. (3) `/lotus` sirve estáticos del
  filesystem (`server.py:749-753`).
- **robustez**: `resolve()` tira ValueError sobre entradas muertas de projects.json sin poda —
  ver cómo degradan los handlers del server ante eso; `autoregister_project.py` traga todas las
  excepciones (data={} y sigue).
- **eficiencia**: nada material desde este eje; el espejo hillclimb.js (JS) es deliberado, no dup
  a consolidar.
