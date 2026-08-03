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

- [Inventario de research local](issues/01-inventario-research-local.md) — ~38 docs/253 KB; mmorch domina (158 KB fuera del vault); 3 duplicados sin pointer que van a driftar; lo de mayor señal (ideal-vision, verdicts, benchmark) sigue fuera; realmente son 2 repos + mmorch.
- [Criterio de curación + pointers](issues/02-criterio-curacion-y-pointers.md) — criterio por TIPO (veredictos/benchmarks/rationale/brainstorms-ancestro = vivo); pointer = stub 3 líneas en el path original (incluye los 3 duplicados); históricos → vault/archive/ con stub, fuera de MOC/recall.
- [Contrato de escritura](issues/03-contrato-de-escritura.md) — puerta única MCP `mmorch_vault_write` (valida title + tag proyecto, created auto); babel async al escribir (cola del server) + nightly como red de seguridad y refresh por hash.
- [Contrato de escritura](issues/03-contrato-de-escritura.md) — puerta única `mmorch_vault_write` (MCP); validación mínima dura (title+tag proyecto, created auto); babel async al escribir + nightly como red; colisiones = update semantics.

## Not yet specified

- Si el vault necesita índice embebido propio o alcanza con memory.duckdb global.
- Cómo interactúa el vault con la memoria auto de Claude Code (dos memorias).
<!-- graduados: pointer -> resuelto en 02; refresh de babels -> resuelto en 03 (nightly por hash) -->

## Out of scope

- Publicar el vault fuera de las máquinas del usuario (no hay multi-usuario).
- Reemplazar la memoria episódica/semántica de mmorch (el vault es la capa
  curada; memory.duckdb sigue siendo lo suyo).
