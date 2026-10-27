---
title: "ECC (affaan-m/ECC) veredicto: prueba real recreando el generador de F1"
created: 2026-10-26
tags: [research, orchestration, ecc, claude-code-plugin, f1, data-de-la-plata, veredicto]
status: seed
confidence: "media: n=1 tarea, un modelo (Sonnet 5.5), una corrida de control sin ECC"
sources: ["https://github.com/affaan-m/ECC (commit 25278e7, v2.2.3, 2026-10-26)"]
---
## Qué se probó
ECC v2.2.3 se cargó con `claude -p --setting-sources project --strict-mcp-config --plugin-dir <ECC>`.
Ese modo carga las 311 skills y los 71 agentes de ECC. No carga el CLAUDE.md ni los hooks del usuario.
`--safe-mode` NO sirve: descarta también `--plugin-dir`.
La tarea fue el pendiente real de F1 "Data de la Plata": el generador de contenido FREE/PREMIUM desde `portfolio.db`.
La base fue el backup `Portfolio-financiero/backups/portfolio_20260825_170740_pre-fix-pk.db`, en solo lectura.
El flujo siguió el README de ECC: `/ecc:plan`, después el skill `tdd-workflow` y después `/ecc:code-review`.
El humano solo aprobó el plan y respondió 3 preguntas con la opción propuesta.

## Oráculo held-out (ECC no lo vio)
El oráculo tiene 23 comprobaciones en tres horas de corte.
- 15:15 reproduce el prototipo escrito a mano: movers, MEP, BCRA, BADLAR y GD30.
- 16:05 cae en la ráfaga falsa de las 16:03. Debe evitar el precio falso en acciones, bonos y MEP.
- 17:05 usa números que no aparecen en el prototipo, así que el agente no puede copiarlos.
El pedido solo mencionaba la contaminación en acciones, a propósito.

Resultado: **VERDE 23/23** antes del code-review y también después. En la hora 16:05, el generador cae al snapshot limpio de las 15:12.

## Qué hizo bien ECC
- El plan encontró solo que la ráfaga también contamina `mep_diario` y bonos. También encontró restos `<MagicMock>` en `moneda_cot` y ráfagas falsas casi diarias desde mayo.
- El plan detectó que `--asof 15:12` sin segundos excluye el snapshot de las 15:12:24.
- El TDD fue real: primero hubo un commit de tests en rojo y después la implementación.
- El código final tiene 326 líneas en 7 módulos y 38 tests. La base terminó con el mismo md5.
- El code-review encontró 3 bugs reales y los corrigió con test: traceback ante una DB inexistente, `-0,00%` y `ZeroDivisionError`.

## Qué no encontró (revisión de Opus sobre el mismo código)
- El MEP no tiene un control de cordura propio. Hereda el minuto del snapshot de bonos.
- Las acciones y los bonos eligen su snapshot por separado. Una contaminación parcial puede mezclar dos horas.
- Con menos de 6 acciones, la lista "bajan" se superpone con la lista "suben".
- La detección depende de la firma conocida. Un dato falso con una descripción realista pasaría porque falta un control de salto de precio.
- El code-review de ECC y la revisión de Opus no coincidieron en ningún hallazgo. Las dos se complementan.

## Costos y efectos laterales
- El costo estimado por los hooks de ECC fue unos 1,3 USD en Sonnet (cupo del plan): 0,33 del plan y 0,97 de la revisión. Una corrida de implementación se cortó sin registro de costo.
- Los hooks `post-bash-command-log` y `cost-tracker` ignoran `ECC_AGENT_DATA_HOME`. Escriben en `~/.claude/bash-commands.log` y `~/.claude/cost-tracker.log` con el texto completo de cada comando. Un secreto en un comando queda en texto plano. Esos archivos se borraron después de la prueba.
- El gate de ECC bloqueó un `rm -rf out` del agente. El bloqueo fue correcto.

## Control sin ECC (2026-10-26)
El control usó el mismo commit del pedido, el mismo modelo y los mismos flags, pero sin `--plugin-dir`.
El pedido fue uno solo: "Implementá lo que pide PEDIDO.md en este repo, con tests, y commiteá."

| Medida | ECC | Sin ECC |
|---|---|---|
| Oráculo held-out | 23/23 | 23/23 |
| Costo estimado | ~1,3 USD + una corrida cortada | 0,35 USD |
| Turnos humanos | 3 (plan, aprobación, reanudación) | 1 |
| Código / tests | 326 líneas, 38 tests | 373 líneas, 12 tests |
| Mutaciones del filtro que matan sus tests | 2 de 3 | 2 de 3 |
| `--asof 15:12` sin segundos | usa el snapshot de las 15:12:24 | cae al snapshot del 24/08 17:00 |

- El control también encontró solo la contaminación en MEP y bonos, el `MagicMock` y las ráfagas desde mayo. Ese hallazgo no se debe a ECC.
- El control exige que acciones y bonos compartan el timestamp. Eso resuelve uno de los hallazgos de Opus que ECC no resolvió.
- La única ventaja medida de ECC es el borde del minuto. Su planner lo detectó, y el control publica el dato de ayer con un aviso solo por stderr.
- Las mutaciones sobrevivientes difieren: ECC no detecta la regla `Descripcion` borrada y el control no detecta la regla `MagicMock` borrada.

## Veredicto
- El 23/23 **no se debe a ECC**. Sonnet sin ECC llegó al mismo resultado con un cuarto del costo y un solo turno humano.
- ECC como plugin entero: **no instalar** en este setup. Cuesta unas 4 veces más, sus skills chocan con mmorch/sdlc y sus hooks escriben fuera del directorio de datos declarado.
- Vale la pena una sola pieza: el paso del planner que explora los datos y lista los bordes antes de escribir código. Con n=1, ese paso explica el único bug que evitó.
- Falta medir: más tareas (n>1) para separar el valor del planner del azar de una corrida.

Artefactos: el oráculo y los dos repos quedaron en el scratchpad de la sesión 6ae748a1 (`oracle/oracle.py`, `f1-ecc/`, `f1-control/`). Ese scratchpad es temporal.
