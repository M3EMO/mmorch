# Spec: Vault global adoptado

Status: ready-for-agent
Source: [mapa wayfinder](map.md) — 10 tickets de decisión resueltos 2026-08-02/03.
Cada decisión linkea su ticket (fuente primaria); ante duda, leer el ticket.

## Problem Statement

El research (paper notes, veredictos de libs, benchmarks, brainstorms de diseño)
vive disperso y local en cada repo: se duplica sin pointer (ya driftó), no aparece
en el recall semántico, y el conocimiento de un proyecto es invisible desde los
demás. El inventario ([01](issues/01-inventario-research-local.md)) midió ~38 docs
/ 253 KB, con lo de mayor señal (ideal-vision, veredictos, el único benchmark real)
FUERA del vault.

## Solution

Un solo vault global (`~/.claude/orchestration/vault/`, Obsidian) como lugar de
todo el research, con: puerta única de escritura validada (MCP), capa babel
automática con gates de ejecución, MOCs por proyecto regenerados al escribir,
bridge al recall semántico existente, migración curada de lo existente con stubs,
sync por el bus git existente y 3 charts nightly.

## User Stories

1. Como sesión de Claude en cualquier proyecto, quiero escribir una nota de research al vault con un solo tool MCP, para que el conocimiento no quede local.
2. Como sesión de Claude, quiero que el tool valide title + tag de proyecto y autocomplete created, para que ninguna nota entre sin atribución.
3. Como sesión de Claude, quiero que el recall semántico me devuelva notas del vault aunque esté parada en otro proyecto, para reutilizar research ajeno.
4. Como modelo lector, quiero un `.babel.md` cuando pague (gates), para leer research con la mitad de tokens.
5. Como humano en Obsidian, quiero un MOC por proyecto siempre fresco, para navegar el vault sin buscar.
6. Como humano, quiero que los docs migrados dejen un stub en su path original, para que mis grep y links viejos no rompan.
7. Como usuario multi-máquina, quiero que el vault viaje solo (nightly push + auto-pull), para no perder research fresco en la otra PC.
8. Como usuario, quiero 3 charts nightly (adopción, babel, costo API) en el vault, para ver de un vistazo si el sistema paga.
9. Como sesión de Claude, quiero usar flint ad-hoc para graficar cualquier resultado, para no leer tablas a ojo.
10. Como curador del lexicon, quiero que `--mine` proponga shorthand nuevo y que nada se auto-promueva, para que el diccionario viva sin degradarse.
11. Como proyecto nuevo, quiero nacer sin carpeta de research y apuntando al vault, para no re-crear el problema.
12. Como usuario, quiero que la memoria auto de Claude linkee notas del vault sin duplicarlas, para que cada memoria haga lo suyo.

## Implementation Decisions

Todas decididas en tickets del mapa; el ticket es la fuente primaria.

- **`vault.write_validated()`** — función nueva en el módulo vault: valida frontmatter (title + tag de proyecto obligatorios, created auto, resto opcional con defaults del template), escribe la nota, regenera el MOC del proyecto, encola job babel async en el server, y bridgea `remember(gist + path)` a memory.duckdb scope global. [03](issues/03-contrato-de-escritura.md), [04](issues/04-prototipo-moc-obsidian.md), [09](issues/09-indice-del-vault.md)
- **Tool MCP `mmorch_vault_write`** — wrapper fino sobre write_validated, sin lógica propia. Puerta única de escritura para agentes; path directo queda para humanos/Obsidian. [03](issues/03-contrato-de-escritura.md)
- **Babel**: automático con pre-filtro determinista (≥3k chars, fuera de archive/ e infra); async al escribir vía cola del server; nightly como red de seguridad + refresh por hash del original en el frontmatter del babel. Gates ya existentes (ratio ≤0.7, fidelidad ≥0.8) deciden. [05](issues/05-alcance-babel.md), [03](issues/03-contrato-de-escritura.md)
- **MOC por proyecto**: formato validado por prototipo (sección por carpeta; línea = wikilink — status · conf · babel ✓); se regenera al escribir; excluye infra y archive. La versión limpia del generador reemplaza al prototipo `_gen_moc_PROTOTYPE.py`. [04](issues/04-prototipo-moc-obsidian.md)
- **Migración inicial curada**: criterio por TIPO (veredictos/benchmarks/design-rationale/brainstorms-ancestro = vivo → vault; planes ejecutados/prompts/audits auto/logs = histórico → vault/archive/); usar la clasificación del inventario; stub de 3 líneas en el path original de TODO lo migrado (incluye los 3 duplicados byte-idénticos detectados); backfill de tags de proyecto en las notas existentes (6/10 sin tag) y de gists al duckdb. [01](issues/01-inventario-research-local.md), [02](issues/02-criterio-curacion-y-pointers.md)
- **Sync**: bus git existente (sync.py) — sumar el vault al commit_push nocturno hacia la branch mmorch/auto; auto-pull ff-only; humano mergea en lote; divergencia = aviso, sin auto-merge. [06](issues/06-sync-multimaquina.md)
- **Charts**: leg nightly nuevo que genera 3 SVG vía flint a vault/charts/ + nota que los embebe: adopción (notas por proyecto en el tiempo), babel (ratio/fidelidad + % skipeados), costo API mensual por proveedor. Convención: flint-chart MCP disponible ad-hoc para cualquier sesión. [08](issues/08-charts-flint.md)
- **Dos memorias, regla por contenido**: cross-proyecto/curado → vault; sesión/preferencias/estado → memoria auto de Claude; links por path, contenido sin duplicar. [10](issues/10-vault-vs-memoria-claude.md)
- Wiring de /new-project y CLAUDE.md global: YA HECHO ([07](issues/07-wiring-new-project.md)).

## Testing Decisions

- Buenos tests = comportamiento externo con deps inyectadas, nunca detalle de implementación. Prior art: los self-checks `__main__` de babel (vault tmp + call_fn fake) y de workflow_evolve.
- **Seam 1 — `vault.write_validated()`** con vault tmp + remember/encolar inyectados: valida el contrato completo (rechaza sin tag, autocompleta created, MOC regenerado, job encolado, gist bridgeado). El grueso de la cobertura vive acá.
- **Seam 2 — smoke E2E del tool MCP**: una invocación real de `mmorch_vault_write` contra el server (nota de prueba → aparece en vault + MOC). Un solo caso, marca de integración, no matriz.
- Migración: dry-run primero; aceptación = originales convertidos a stub, archivos en vault, duplicados resueltos, cero pérdida (diff de contenido).
- Legs nightly (babel sweep, charts, sync del vault): self-check con paths inyectados, sin llamadas API en tests.

## Out of Scope

- Publicar el vault fuera de las máquinas del usuario (sin multi-usuario).
- Reemplazar la memoria episódica/semántica de mmorch (el vault es la capa curada).
- Auto-merge de conflictos por nota; índice de embeddings propio del vault.
- Auto-promoción de candidatos del lexicon (curación humana/Opus siempre).

## Further Notes

- Invariantes mmorch aplican: original = fuente de verdad; gates de ejecución, jamás LLM-judge; auto-run sí, auto-merge jamás.
- Mediciones babel relevantes en el docstring de `mmorch/babel.py` (encoder gemini / lector deepseek; símbolos del lexicon NUNCA en el prompt del encoder).
- Implementar con `/project` (project-build engine); el acceptance test natural: pytest del seam 1 + self-checks.
