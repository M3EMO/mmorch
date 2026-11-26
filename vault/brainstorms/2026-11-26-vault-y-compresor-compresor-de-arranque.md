---
title: "Vault y compresor: compresor de arranque (grilling)"
created: 2026-11-26
tags: [brainstorm, orchestration, vault-y-compresor]
status: seed
---
# Vault y compresor: notas del grilling del compresor de arranque
Fecha: 2026-11-26 · Objetivo: resolver el ticket "Compresor de arranque: qué partes y con qué palanca" (`.scratch/vault-y-compresor/issues/09-compresor-de-arranque.md`).

## Contexto de entrada

- El arranque mide 66.8k tokens de mediana. La meta es 52k o menos, así que el compresor saca 14.8k o más.
- ECC aporta unos 43k: skills 29.7k, agentes 7.8k y comandos 5.6k (ticket 02).
- Los CLAUDE.md, AGENTS.md y MEMORY.md suman unos 6.4k. Solos no alcanzan la meta.
- ECC queda igual. Las palancas válidas son `skillOverrides`, settings y los `.md` propios.
- El piso es el oráculo del ticket 06: retención 0.965, guardrails al 100% y cero regresiones en tareas.

## Medición nueva antes de preguntar (2026-11-26)

El ticket 02 reconstruyó las skills de ECC desde el frontmatter, y esa reconstrucción falla. Claude Code 2.1.330–2.1.332 lista casi todas las skills de ECC sin descripción. En esta sesión, 384 de 398 entradas de ECC llegan solo con el nombre.

Se midió el texto real de cada adjunto con `count_tokens` (haiku-5-5), desde el transcript `4dbb6077`:

| Fuente | Tokens | Palanca sin editar ECC |
|---|---|---|
| Listado de skills y comandos (461 entradas) | 12.9k | `skillOverrides` por skill |
| — de eso, ECC | ~5.9k | `skillOverrides` |
| — de eso, el resto (propias, anthropic-skills, ponytail) | ~6.3k | `skillOverrides` |
| Listado de agentes (78, 71 de ECC) | 9.5k | Solo apagando el plugin: fuera |
| CLAUDE.md ×2, AGENTS.md y MEMORY.md | 6.8k | Reescribir |
| Nombres de tools diferidos (170) | 4.5k | Apagar servidores MCP sin uso |
| Hook ponytail (incluye el mapa del código) | 1.9k | Settings del plugin ponytail |
| Hook de resumen de ECC | 1.1k | `ECC_DISABLED_HOOKS` |
| Contexto de sesión (email, git status) | 1.7k | Ninguna |
| Instrucciones MCP | 0.5k | Apagar el servidor |
| System prompt y esquemas de tools (resto) | ~28k | Ninguna |

- El total sigue en 66.8k. Lo que cambia es el reparto.
- ECC pesa unos 16.5k, no 43k. Los agentes (8.5k de ECC) quedan fuera de alcance.
- `skillOverrides` acepta por skill los valores `name-only`, `user-invocable-only` y `off` (binario 2.1.332). `user-invocable-only` saca la skill del listado del modelo y deja el `/nombre` al usuario (inferido del nombre y del mensaje "disabled for model invocation"; falta probarlo).
- Settings también trae `skillListingMaxDescChars` y `skillListingBudgetFraction`.
- Techo sin tocar agentes: ~23k. La meta pide 14.8k, así que es posible pero necesita varias palancas.

## Resumen y decisiones

- El reparto medido cambia: ECC pesa ~16.5k, no 43k. Claude Code ya lista casi todas sus skills sin descripción.
- La meta queda en 52k o menos, con varias palancas (P1).
- `skillOverrides` pone en `user-invocable-only` las 392 skills de ECC sin uso en el mes (~-5.6k, P2).
- El compresor recorta las descripciones de las 12 skills propias con la receta SkillReducer. `skillOverrides` oculta las skills de Claude Code sin uso, salvo las que otra regla pide cargar (~-2.5k, P3).
- Se apagan cuatro servidores MCP sin uso: Gmail, flint-chart, scheduled-tasks y mcp-registry (~-1.3k, P4).
- Los hooks de ECC y de ponytail quedan prendidos (P5).
- Los `.md` propios se reescriben; es la única palanca que queda.
- Estimado total: ~-12.1k con un recorte de 40% en los `.md`. El arranque queda en ~54.7k, arriba de la meta.

## Registro de preguntas

### P1 — La meta con el reparto nuevo
- Pregunta: ¿la meta sigue en 52k con varias palancas, baja a lo que dan una o dos palancas, o se reabre el ticket 02?
- Respuesta del usuario: "52k, varias palancas".
- Captura: la meta queda en 52k o menos. El compresor combina `skillOverrides`, servidores MCP, hooks y la reescritura de los `.md` propios. Los agentes de ECC siguen fuera.

### P2 — Skills de ECC sin uso
- Dato medido: en un mes hubo 37 invocaciones de skills por el modelo y 93 comandos con barra, en 3205 transcripts. De ECC se usaron 6 de 398: `code-review`, `orch-build-mvp`, `orch-pipeline`, `tdd-workflow`, `plan` y `plan-prd`.
- Pregunta: ¿las 392 restantes van a `user-invocable-only`, a `off`, o solo se ocultan las de otro stack?
- Respuesta del usuario: "user-invocable-only".
- Captura: `skillOverrides` pone en `user-invocable-only` cada skill de ECC sin uso en el mes. El usuario sigue tipeando `/ecc:nombre`. Ahorro estimado: ~5.6k tokens. El oráculo de ruteo confirma que no se pierde nada.

### P3 — Skills fuera de ECC
- Dato medido: las 12 skills propias de `~/.claude/skills` pesan 1.75k en el listado. Las skills de Claude Code y las anthropic-skills pesan ~4k.
- Pregunta: ¿reescribir las propias y ocultar las de Claude Code sin uso, solo reescribir, o dejarlas igual?
- Respuesta del usuario: "Reescribir y ocultar".
- Captura: el compresor recorta las descripciones de las skills propias con la receta SkillReducer (~-0.9k). `skillOverrides` pone en `user-invocable-only` las skills de Claude Code y anthropic-skills sin uso en el mes. Quedan visibles las que otra regla pide cargar: `artifact-*`, `docs`, los dos browsers y `claude-api`. Ahorro estimado del paso: ~2.5k.
- Acumulado: ~8.1k de 14.8k.

### P4 — Servidores MCP sin uso
- Dato medido: en el mes tuvieron cero llamadas el conector de Gmail (0.9k), flint-chart, scheduled-tasks y mcp-registry. Drive (35), Calendar (21), Docs (68), Chrome (200), chrome-devtools (35) y Colab (14) sí se usaron.
- Pregunta: ¿se apagan los cuatro, solo Gmail, o ninguno?
- Respuesta del usuario: "Apagar los cuatro".
- Captura: se apagan Gmail, flint-chart, scheduled-tasks y mcp-registry. Ahorro estimado: ~1.3k.
- Acumulado: ~9.4k de 14.8k. Quedan los `.md` propios (6.8k) y los hooks (3k).

### P5 — Hooks de arranque
- Dato medido: el hook de resumen de sesión de ECC pesa 1.1k y se apaga con `ECC_DISABLED_HOOKS`. El hook de ponytail pesa 1.9k: reglas ~1.3k y mapa del código ~0.6k (`PONYTAIL_MAP`, `PONYTAIL_MAP_CHARS`).
- Pregunta: ¿se apaga el resumen de ECC y el mapa, el resumen y ponytail entero, o solo el resumen?
- Respuesta del usuario: "Ninguno, ya esta."
- Captura: ningún hook se apaga. El usuario cerró el grilling con esta respuesta.
- Consecuencia: los ~5.4k que faltan salen solo de los `.md` propios (6.8k), o sea un recorte de ~79%. SkillReducer midió -39% a -48%. Con ~40%, el arranque queda en ~54.7k, arriba de la meta de 52k.

## Pendientes

- La meta de 52k choca con P5. Con las palancas elegidas, el estimado da ~54.7k. Decide el usuario: bajar la meta, apagar un hook o aceptar el número que mida la spec → usuario.
- El valor `user-invocable-only` falta probarlo: ¿saca la skill del listado y deja el `/nombre`? → la spec, con una sesión de prueba.
- El orden de aplicación y el porcentaje de recorte de los `.md` no se preguntaron. El usuario cerró el grilling en P5 → la spec los fija, y el oráculo decide.
- El ticket 02 quedó con un desglose equivocado de ECC → corregir su nota del vault (tarea vista al pasar, no hecha).
