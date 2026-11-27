# Plan de la pieza A: supersesión en el vault

El planner de ECC armó este plan el 2026-11-26 (flujo `orch-add-feature`, tier large). El usuario aprobó el Gate 1 el 2026-11-27, con los defaults P1 a P5 ("Sí, aprobado").

## Decisiones de diseño

- **D1.** El recall sabe que una nota está retirada por el disco. La regla es `vault.is_retired(path)`: el path original falta y existe `VAULT/archive/<nombre>`. No cambia el schema de duckdb.
- **D2.** El path sale del payload del episodio `vault_note`. Un helper `_vault_links(con)` en `memory.py` filtra `recall` y `recall_keyword`. `recall_hybrid` hereda el filtro. El fallback episódico filtra en SQL, antes del `LIMIT`.
- **D3.** El apagado es `VAULT_MODULES = frozenset({"supersedes", "code"})` en `mmorch/config.py`. Un `supersedes` con el módulo apagado da `ValueError`.
- **D4.** Un solo helper `_retire(p, successor)` mueve la nota, inserta `superseded_by` y agrega "Reemplazada por [[nueva]]". El helper es idempotente.
- **D5.** El nombre de nota es el stem. El código rechaza `""`, `.`, `..` y todo nombre con separador. Cero o varias coincidencias dan error.
- **D6.** `write_validated` valida todo antes de escribir la nota nueva.
- **D7.** El MCP recibe `supersedes` en CSV, igual que `sources` y `tags`.
- **D8.** El MOC suma `## Reemplazadas` (`- [[vieja]] → [[nueva]]`) y `## Archivadas` (`- [[seed]]`).
- **D9.** El atado usa el campo `code:` y el cierre `reviewed: YYYY-MM-DD`. La marca sale de `git log -1 --format=%cs`. El recall muestra `[a revisar] ` y el MOC muestra `· a revisar`.
- **D10.** A4 vive en `mmorch/vault_seeds.py` y corre con `python -m mmorch.vault_seeds [--aplicar]`.

## task_list

Fase 1 (A1, A3 y A5):

- T0: el test de aceptación `tests/test_supersesion_pares.py`, en rojo hasta T5.
- T1: la validación y el apagado, antes de escribir.
- T2: el retiro al escribir, con el parámetro nuevo del MCP.
- T3: las secciones nuevas del MOC. El test `test_moc_excluye_infra_y_archive` cambia, porque la spec pide listar `archive/`.
- T4: `regenerate_moc` completa las ediciones a mano.
- T5: el filtro del recall. Con T5, T0 pasa a verde.

Fase 2 (A2):

- T6: la marca "a revisar" en el MOC, con un repo git sintético.
- T7: la marca en el recall, con la nota visible.
- T8: el parámetro `code` al escribir, validado.

Fase 3 (A4):

- T9: el conteo de lecturas por nombre exacto en transcripts sintéticos.
- T10: las candidatas y el freno de 60 días.
- T11: la CLI, que mueve solo con `--aplicar`.

Cierre: la suite total, `python -m mmorch.docgen` y `docgen --check`.

## Defaults que el usuario aprueba en el Gate 1

- P1: un `supersedes` a mano roto hace que `regenerate_moc` loguee y saltee. El tool sí da error.
- P2: `code` ata solo paths del repo `orchestration`.
- P3: un commit del mismo día que `created` no marca la nota.
- P4: un `superseded_by` a mano no escribe `supersedes` en la nota nueva.
- P5: el MCP usa CSV para `supersedes`.

## Riesgos anotados

- `consolidate` puede fusionar un gist retirado con uno vivo. El filtro oculta entonces la fila entera.
- El MCP vivo ve el cambio solo después de recargar Claude Code.
