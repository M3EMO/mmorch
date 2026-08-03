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
- [Prototipo MOC Obsidian](issues/04-prototipo-moc-obsidian.md) — formato validado (carpeta → wikilink · status · conf · babel ✓); se regenera AL ESCRIBIR desde mmorch_vault_write; infra y archive excluidos. Hallazgo: 6/10 notas sin tag de proyecto → backfill a la spec.
- [Alcance babel](issues/05-alcance-babel.md) — automático con pre-filtro determinista gratis: ≥3k chars, fuera de archive/ e infra; sin marcado manual; los gates deciden el resto (rechazo por ratio = centavos).
- [Sync multi-máquina](issues/06-sync-multimaquina.md) — sync.py tal cual (nightly commit_push del vault a mmorch/auto, auto-pull ff-only, humano mergea en lote); conflictos = ff-only avisa, sin auto-merge por nota. Spec: sumar el vault al push nocturno.
- [Charts flint](issues/08-charts-flint.md) — 3 charts nightly como SVG en vault/charts/ (adopción por proyecto, babel ratio/fidelidad, costo API por proveedor) + convención: flint ad-hoc desde cualquier sesión para graficar resultados on demand.
- [Índice del vault](issues/09-indice-del-vault.md) — bridge a duckdb: write_validated hace remember(gist+path) scope global; recall existente encuentra, path lee; backfill de las notas ya migradas.
- [Vault vs memoria auto Claude](issues/10-vault-vs-memoria-claude.md) — regla por contenido: cross-proyecto/curado → vault, sesión/preferencias → memoria auto; links por path, sin duplicar.
- [Wiring /new-project + CLAUDE.md global](issues/07-wiring-new-project.md) — HECHO: skill /new-project con sección "research → vault, NO local"; CLAUDE.md global con la convención completa. mmorch_vault_write queda como ítem de spec (fallback path directo funciona hoy).

## Not yet specified

_Vacío — toda la niebla graduada y resuelta. El mapa está completo._

## Out of scope

- Publicar el vault fuera de las máquinas del usuario (no hay multi-usuario).
- Reemplazar la memoria episódica/semántica de mmorch (el vault es la capa
  curada; memory.duckdb sigue siendo lo suyo).
