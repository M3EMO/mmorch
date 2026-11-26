# Spec: vault con supersesión y compresor de arranque

Esta spec es el destino del mapa wayfinder `vault-y-compresor` (2026-11-26).
ECC la construye. Cada decisión tiene su detalle en `issues/01` a `issues/09` y en las capturas de `vault/brainstorms/2026-11-26-*`.
La spec tiene dos piezas independientes. Cada pieza se construye y se acepta sola.

| Pieza | Dónde vive | Lista cuando |
|---|---|---|
| A. Supersesión en el vault | `mmorch/vault.py` y `mmorch/memory.py`, este repo | El test de pares pasa en el 100% |
| B. Compresor de arranque | Repo Root (Rust) | El arranque medido baja a 52k tokens o menos, sobre el piso del oráculo |

## Pieza A — Supersesión en el vault (mantenimiento de mmorch)

Root reemplaza a mmorch, pero el vault vive acá. Por eso esta pieza entra como mantenimiento (ticket 01).

### A1. Módulo `supersedes` (ticket 07)

- Cada mecanismo es un módulo apagable. Esta spec construye dos: `supersedes` y el atado a código.
- `mmorch_vault_write` acepta `supersedes: [<nota vieja>]`.
- Una nota vieja inexistente da error y no escribe nada.
- El tool mueve la nota vieja a `vault/archive/`.
- El tool le pone `superseded_by: <nota nueva>` en el frontmatter.
- El tool agrega la línea "Reemplazada por [[nueva]]" al tope del cuerpo de la nota vieja.
- `regenerate_moc` completa la mitad que falte cuando alguien edita una nota a mano.
- Hoy `regenerate_moc` excluye `archive/` (`mmorch/vault.py:228`). El cambio lee `archive/` solo para armar las secciones nuevas.

### A2. Módulo de atado a código (ticket 07)

- Una nota declara el módulo de código del repo que describe.
- Un commit posterior en ese módulo marca la nota "a revisar". El cálculo usa `git log`.
- La marca sale en el recall y en el MOC. La nota sigue visible.
- La marca no cuenta como reemplazo. Solo `supersedes` oculta una nota.
- Un humano o una nota nueva con `supersedes` cierra la marca.

### A3. Recall y MOC

- El recall nunca devuelve una nota reemplazada ni archivada.
- La regla cubre el gist en duckdb (`mmorch/memory.py`) y el fallback al episodio crudo.
- El MOC lista una nota reemplazada solo en la sección "Reemplazadas", con link a su nota nueva.
- El MOC lista una seed archivada en la sección "Archivadas", sin link a sucesora.

### A4. Comando de archivo de seeds (ticket 07)

- Las notas viejas no se migran. La supersesión corre solo hacia adelante.
- Un comando a mano lista las seeds candidatas. El comando mueve las notas solo con `--aplicar`.
- Una seed es candidata si nadie la leyó por nombre de archivo en los últimos 60 días de transcripts.
- Una lectura cuenta si un Read, cat, sed o grep nombra el archivo. Un glob o una regex no cuenta.
- Solo cuentan seeds con `created` de más de 60 días.
- El comando se niega a correr si los transcripts cubren menos de 60 días. Hoy eso lo frena hasta el 2026-12-11.
- Volver atrás una seed es moverla a mano a su carpeta.

### A5. Aceptación de la pieza A

Un test determinista de pares pasa en el 100%. Cada par es una nota vieja y una nota nueva con `supersedes`. El test chequea tres cosas:

1. El recall nunca devuelve la nota vieja.
2. El MOC nunca lista la nota vieja fuera de "Reemplazadas".
3. En "Reemplazadas", la nota vieja siempre sale con link a la nueva.

Un test aparte chequea el módulo A2. Un commit en el módulo atado marca la nota, y la nota sigue en el recall.

## Pieza B — Compresor de arranque (Root)

### B1. Medida del arranque

- El arranque es el input del primer mensaje del assistant con usage mayor a cero.
- El input suma `input_tokens`, `cache_creation_input_tokens` y `cache_read_input_tokens`.
- La medida es la mediana de 10 sesiones o más en `~/.claude/orchestration`, con la misma versión de Claude Code.
- La base hoy mide 66.8k tokens (11 sesiones, Claude Code 2.1.330 a 2.1.332).
- El conteo por fuente usa `count_tokens` de Anthropic, nunca una estimación por caracteres.

### B2. Reparto medido (ticket 09, corrige al ticket 02)

El ticket 02 reconstruyó las skills de ECC desde el frontmatter. Claude Code lista casi todas esas skills sin descripción. Por eso la cifra de 43k para ECC falla. La medición real del 2026-11-26 da:

| Fuente | Tokens | Palanca |
|---|---|---|
| Listado de skills y comandos (461) | 12.9k | `skillOverrides` y reescritura |
| Agentes (78, 71 de ECC) | 9.5k | Ninguna: fuera de alcance |
| CLAUDE.md ×2, AGENTS.md y MEMORY.md | 6.8k | Reescritura |
| Nombres de tools diferidos (170) | 4.5k | Apagar servidores MCP |
| Hooks de ponytail y de resumen de ECC | 3.0k | Ninguna: el usuario los deja |
| System prompt, tools nativos y contexto de sesión | ~30k | Ninguna |

### B3. Palancas, en orden de aplicación

Ninguna palanca edita un archivo del plugin ECC.

1. **Skills de ECC sin uso.** `skillOverrides` pone en `user-invocable-only` cada skill y comando de ECC sin uso en el mes. Hoy son 392 de 398. Quedan visibles `code-review`, `orch-build-mvp`, `orch-pipeline`, `tdd-workflow`, `plan` y `plan-prd`. Ahorro estimado: ~5.6k.
2. **Skills de Claude Code sin uso.** `skillOverrides` pone en `user-invocable-only` las skills de Claude Code y de anthropic-skills sin uso en el mes. Quedan visibles `artifact-*`, `docs`, `built-in-browser`, `chrome-browser` y `claude-api`. Otra regla pide cargar esas skills.
3. **Skills propias.** El compresor recorta las descripciones de las 12 skills de `~/.claude/skills` con la receta SkillReducer. Los pasos 2 y 3 suman ~2.5k.
4. **Servidores MCP sin uso.** Se apagan el conector de Gmail, flint-chart, scheduled-tasks y mcp-registry. Ahorro estimado: ~1.3k.
5. **`.md` propios.** El compresor reescribe `~/.claude/CLAUDE.md`, el CLAUDE.md y el AGENTS.md del proyecto, y MEMORY.md. El bloque de bd en AGENTS.md es el primer candidato. Las "Reglas propias de bd" ya lo contradicen en parte.

- "Sin uso en el mes" se mide en los transcripts de `~/.claude/projects`. Cuentan las llamadas al tool Skill y los comandos `<command-name>`.
- El compresor recalcula la lista de uso en cada corrida. La lista de esta spec es solo la foto de hoy.
- `skillOverrides` va en `~/.claude/settings.json`, porque el uso se midió en todos los proyectos.
- El compresor escribe un diff de settings y de cada `.md`. El usuario aplica el diff. El compresor nunca lo aplica solo.

### B4. Oráculo de fidelidad (ticket 06)

El oráculo tiene dos piezas, y ninguna usa un LLM-juez.

- **Batería.** Son 200 preguntas con respuesta exacta: 120 de ruteo y 80 de reglas. Unas 25 de ruteo esperan "ningún skill".
- Una pregunta de ruteo da un pedido real y espera el nombre del skill. Una pregunta de reglas espera un dato exacto de un `.md` propio.
- Haiku 5.5 responde la batería por API, sin cupo. El original y el comprimido corren con el mismo modelo.
- Cada caso corre 3 veces y vale la mayoría. Un checker compara cada respuesta.
- Un caso entra al gold solo si el original lo acierta en 2 de 3 corridas.
- **Gate de tareas.** Son 10 tareas reales con `claude -p` y el modelo real. Corren una vez antes de aceptar una versión.
- Un checker determinista juzga cada tarea: un test que pasa, un archivo creado o un skill invocado en el transcript.
- **Piso.** La retención es aciertos del comprimido sobre aciertos del original, en los casos que el original acierta.
- La batería retiene 0.965 o más. Las 20 preguntas de guardrails y zona roja retienen el 100%.
- El comprimido pasa toda tarea que el original pasa.
- Las preguntas de ruteo cubren también las skills que pasan a `user-invocable-only`. Un pedido que antes ruteaba a una skill oculta cuenta como fallo.

### B5. Armado del gold (paso HITL)

- El ruteo sale de pedidos reales de los transcripts.
- Un LLM redacta las 80 preguntas de reglas. El usuario aprueba cada una.
- El usuario elige las 10 tareas del gate.
- El agente nunca aprueba el gold en nombre del usuario.
- El oráculo y su gold viven en Root, junto al compresor.

### B6. Aceptación de la pieza B

- El arranque medido según B1 queda en 52k tokens o menos.
- El oráculo B4 pasa su piso completo con la versión aplicada.

## Fases de build sugeridas

1. A1 y A3 con el test de pares A5.
2. A2 con su test.
3. A4, con el freno de 60 días probado sobre transcripts sintéticos.
4. B1: el script de medida del arranque y el conteo por fuente.
5. B5: el armado del gold. Esta fase espera al usuario.
6. B4: la batería y el gate sobre el original, como línea base.
7. B3, palanca por palanca, con el oráculo después de cada una.
8. B6: la medida final y el reporte al usuario.

## Riesgos y preguntas abiertas

- **La meta de 52k puede no cerrar.** Con un recorte de 40% en los `.md`, el estimado da ~54.7k. Llegar a 52k pide ~79% en los `.md`. SkillReducer midió -39% a -48%. Si B6 falla, el build se frena. Entonces el usuario decide entre bajar la meta y apagar un hook.
- **`user-invocable-only` falta probarlo.** El valor sale del binario de Claude Code 2.1.332, no de la documentación. La fase 7 prueba primero dos cosas: la skill sale del listado y `/nombre` sigue andando.
- **Dónde corre el compresor.** El mapa no decidió entre comando a mano, hook o paso nocturno. Esta spec asume un comando a mano que produce diffs. El usuario confirma antes de la fase 7.
- **Versión de Claude Code.** Un cambio de versión puede mover el reparto. B1 compara solo sesiones de la misma versión.

## Fuera de alcance

- Editar o quitar el plugin ECC, y apagar sus agentes.
- Apagar los hooks de ECC y de ponytail.
- Comprimir notas del vault: ahorra disco y no cupo (ticket 08).
- Revivir babel o un lexicon de shorthand.
- Comprimir la salida de tools.
- Mudar el vault a otro repo.
- Vencimiento, clave por hecho y chequeo LLM de supersesión: quedan como módulos futuros, sin código.
