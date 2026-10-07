# Implementar el informe de impacto (Python + Claude Code)
Type: task
Status: resolved
Blocked by: 03
Map: ../map.md

## Question

Implementar el diseño del ticket 03 para Python y Claude Code:

- `mmorch/impacto.py`: extractor de literales compartidos (el de `../orch-spike/scripts/tareas/extractor.py`, medido en el ticket 09), orden por especificidad, resumen de literales en más de 5 módulos, tope de 10 líneas, y una línea con los tests que importan el módulo.
- Hook `PreToolUse` en `~/.claude/hooks/` para `Edit|Write|MultiEdit` sobre archivos `.py`: `additionalContext`, una vez por archivo por `session_id`, sin `permissionDecision`, fail-open.
- Registro de cada informe en un jsonl con un solo módulo dueño.
- Test de guarda: falla si un hook de `~/.claude/hooks/` referencia un módulo de mmorch inexistente.
- Aceptación: tests del módulo verdes; el extractor reproduce en el banco del ticket 09 un acierto de B no menor al medido (96% con deepseek-v4-pro) con el recorte nuevo.

Historia: el alcance original (herramienta que une codegraph, indirectas y datos, medida por recall) se reemplazó tras el ticket 01 (recall saturado) y el grilling del ticket 03 (codegraph fuera del informe).

## Answer

Hecho (2026-09-25; commits 203e61f y 11c4b16 en mmorch/auto). `mmorch/impacto.py`: literales con forma de dato (sin espacios, o SQL), orden por especificidad, literales en más de 5 módulos resumidos, tope de 10 líneas; modos `hook` (Claude Code) y `cursor`. Aceptación en el banco del 09 (deepseek-v4-pro): 23/24 acopladas y 12/12 control, igual al extractor medido. La primera versión, con una línea de "tests que importan el módulo", dio 20/24 (p = 0.17 frente a 23/24): no mejoró y se quitó; los tests visibles no cubren a los lectores. Ablación: el filtro de forma de dato y el resumen no cambian el acierto (23/24 cada uno). Hooks instalados con OK del usuario: `PreToolUse` en `~/.claude/settings.json` y `postToolUse` en `~/.cursor/hooks.json`, ambos vía `scripts/impacto_hook.py` (el venv no instala el paquete; `-m mmorch.impacto` fallaba fuera del repo). `tests/test_hooks_guard.py` revisa hooks de Claude Code y Cursor, scripts llamados y módulos importados. Registro de uso en `logs/impacto.jsonl`. Aceptado por el usuario el 2026-09-25.
