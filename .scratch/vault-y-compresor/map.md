# Mapa wayfinder — Vault rediseñado y compresor
Label: wayfinder:map · Creado: 2026-11-26 · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Destination

Confirmado con el usuario en [Destino y hogar del compresor](issues/01-destino-y-hogar.md).
Una spec que ECC construye, con dos piezas:
- Supersesión en el vault de este repo (`mmorch/vault.py`). Está lista cuando un test determinista de pares pasa en el 100%: el recall nunca devuelve una nota reemplazada.
- Compresor del contexto de arranque, en Root. Está listo con un arranque medido de 52k tokens o menos (hoy 66.8k), sobre el piso de fidelidad del oráculo.

## Notes

- Decisiones del usuario antes del charteo (2026-11-26): apagar babel y rediseñar el vault; compresor = arranque + notas, "un archivo lo más chico posible"; borrar código y datos de babel (hecho, sin commit); dejar ECC igual.
- Evidencia base: vault "Vault y compresor: babel no paga, el arranque sí (2026-11-26)" y vault "Setup de Claude Code: evidencia medida...". Leelas antes de resolver un ticket.
- Gists de la evidencia: babel más lexicon costaba 96% del original y nadie leyó un `.babel.md` en un mes. El arranque cuesta unos 67k tokens. StaleBench devuelve el hecho viejo 53% de las veces. Comprimir conociendo la pregunta supera a comprimir sin ella.
- Conflicto con un plan existente: `.scratch/vault-global/spec.md` (2026-08) describe babel como parte del vault. El código ya no tiene babel. Este mapa reemplaza esa parte de la spec.
- Root reemplaza a mmorch (decisión 2026-10-31). Por eso el compresor va a Root y la supersesión entra como mantenimiento.
- Oráculo: checkeable va a un oráculo, nunca a un LLM-juez. El conteo de tokens usa el contador real de Anthropic.
- Skills: `/grill-me` y `/domain-modeling` para grilling; `/research` para research AFK; `/prototype` para prototipos. Research al vault global con `mmorch_vault_write`, tag `orchestration`.
- Reglas: resolver un ticket de grilling es HITL; el agente nunca responde su propio grilling. Un ticket por sesión, salvo research.

## Decisions so far

<!-- una línea por ticket resuelto: gist + link -->

- [Destino y hogar del compresor](issues/01-destino-y-hogar.md) — Spec que construye ECC. La supersesión queda en este repo y el compresor va a Root. Listo = arranque ≤52k tokens y recall sin notas reemplazadas.

- [Métodos de compresión con piso de fidelidad](issues/03-metodos-de-compresion.md) — SkillReducer es el único método con piso medido sobre skills y salida legible. LLMLingua-2 y Selective Context dejan texto ilegible. Todas las cifras vienen de abstracts, ninguna se midió en español.
- [Supersesión en sistemas de memoria de agentes](issues/04-supersesion-en-memorias.md) — Un registro determinista (`valid_to`, `superseded_by`, clave sujeto-relación) supera al LLM al escribir. En StaleBench, el rerank por fecha da 0.925 contra 53% de error del coseno. Zep y mem0 no miden la supersesión aislada.
- [Lecturas y tamaños de las notas del vault](issues/05-lecturas-del-vault.md) — Comprimir notas ahorra disco, no cupo. El mes tuvo 53 lecturas, unos 21k tokens estimados (~0.009% del input) y 0 llamadas a `mmorch_recall`. Una nota p50 pesa unos 1.5k tokens, y se lee en promedio el 43% de cada nota.
- [Desglose medido del contexto de arranque](issues/02-desglose-del-arranque.md) — El arranque real mide 66.8k tokens de mediana en 11 sesiones. ECC aporta unos 43k: skills 29.7k, agentes 7.8k y comandos 5.6k. Los CLAUDE.md, AGENTS.md y MEMORY.md suman unos 6.4k. `skillOverrides` está vacío hoy, y los agentes de ECC solo se apagan con el plugin entero.
- [Supersesión en el vault](issues/07-supersesion-en-el-vault.md) — `supersedes` explícito mueve la vieja a `archive/` con aviso; el MOC la lista en "Reemplazadas" y el recall nunca. Un atado a código marca "a revisar" sin ocultar. Sin migración: un comando a mano archiva seeds sin lecturas en 60 días.
- [Oráculo de fidelidad del compresor](issues/06-oraculo-de-fidelidad.md) — Haiku responde por API una batería de 200 preguntas exactas, de ruteo y de reglas. Un gate final de 10 tareas reales corre con `claude -p`. El piso es una retención de 0.965 o más, 100% en guardrails y cero regresiones en tareas.

## Not yet specified

- Cómo se aplica el compresor al arranque sin editar archivos de ECC. La decisión "dejar ECC igual" limita las palancas a `skillOverrides`, CLAUDE.md y skills propios.
- Cómo el comando de archivo detecta una lectura por nombre en los transcripts, y si los transcripts duran 60 días (el más viejo es del 2026-10-12). Las seeds son 138, no 126; el ticket 07 decidió no migrarlas.
- Dónde corre el compresor: hook al escribir, comando a mano o paso nocturno.
- Limpieza de restos: `vault/moc/sin-proyecto.md` y `_gen_moc_PROTOTYPE.py` linkean al lexicon borrado; el README del vault nombra carpetas que no existen.

## Out of scope

- Revivir babel o un lexicon de shorthand: el usuario lo apagó y la medición dio 96% del original.
- Quitar o editar el plugin ECC: el usuario eligió dejarlo igual.
- Comprimir la salida de tools (estilo RTK): la medición propia dio solo 1.5% de ahorro.
- [Compresor de notas](issues/08-compresor-de-notas.md): las lecturas del vault son ~0.009% del input, así que ahorra disco y no cupo (usuario, 2026-11-26).
- Mudar el vault a otro repo: la supersesión se hace donde el vault vive hoy.
