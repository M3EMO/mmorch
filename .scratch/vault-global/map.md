# Mapa: Vault global adoptado

Label: wayfinder:map

## Destination

Una **spec buildeable** para que todos los proyectos activos usen el vault global
(`~/.claude/orchestration/vault/`) como único lugar de research: lectura híbrida
(recall MCP + path directo), migración curada de lo existente, organización
tema+tag con MOC por proyecto, capa babel donde pague. La spec se entrega a
/project o GSD para implementar.

## Notes

- Dominio: mmorch (leer `README.md` y memoria `knowledge-vault-plan` primero).
- Decisiones ya tomadas en el grilling de charting (no re-litigar):
  proyectos = TODOS los activos · acceso = híbrido MCP recall + path directo ·
  migración = retroactiva CURADA (vivo sí, histórico no) · estructura =
  por tema + tag de proyecto obligatorio + MOC por proyecto en Obsidian.
- Skills a consultar por sesión: /grilling, /domain-modeling, /prototype, /research.
- Invariantes mmorch aplican: original = fuente de verdad; gates de ejecución;
  babel nunca auto-promueve lexicon.

## Decisions so far

<!-- una línea por ticket cerrado: gist + link -->

## Not yet specified

- Formato exacto del "pointer" que queda en el repo de origen tras migrar un doc
  (depende del inventario y del criterio de curación).
- Política de refresh de babels cuando el original cambia (¿nightly? ¿on-read?).
- Si el vault necesita índice embebido propio o alcanza con memory.duckdb global.
- Cómo interactúa el vault con la memoria auto de Claude Code (dos memorias).

## Out of scope

- Publicar el vault fuera de las máquinas del usuario (no hay multi-usuario).
- Reemplazar la memoria episódica/semántica de mmorch (el vault es la capa
  curada; memory.duckdb sigue siendo lo suyo).
