# Informe de impacto: Brainstorm / Discovery Notes
Date: 2026-09-25 · Goal: resolver el ticket 03 del mapa `.scratch/mapa-de-impacto` — formato, tamaño, contenido, lugar y disparo del informe de impacto que Claude recibe antes de editar.

Evidencia de entrada: ticket 09 (informe estático de literales compartidos: acopladas 42% → 96% con deepseek-v4-pro; deepseek-chat 79%, el ruido de señuelos probablemente ahoga la señal), ticket 01 (selección de tests por imports ya da recall 1.00 en mmorch), ticket 02 (codegraph no ve datos, indirectas ni ejecución; 1.6.0 corrige el reindex y resuelve alias).

## Summary / key decisions

- Alcance: cualquier repo que Claude abra, incluidos repos ajenos (Q1). Consecuencia: el informe funciona sin preparar el repo, solo con análisis estático; audit hook y contratos quedan como extras para repos con tests.
- Disparo: hook PreToolUse antes de Edit/Write; una vez por archivo por sesión; nunca bloquea; fail-open (Q2).
- Contenido: (1) lectores por literales compartidos, (2) una línea con los tests que importan el módulo (Q3).
- codegraph: MCP quitado, telemetría apagada; regla de llamadores = CLI + grep -w (Q3b, Q3c).
- Tamaño: orden por especificidad, literales en >5 módulos resumidos, tope de 10 líneas con conteo de lo cortado (Q4).
- Código: `mmorch/impacto.py` + hook fino fail-open + test de guarda de hooks (Q5).
- Lenguajes: extractor propio por lenguaje sobre tree-sitter (Q6); cada lenguaje se activa solo tras pasar el banco de tareas en ese lenguaje (Q7). Python ya pasó.
- Cursor: hook `postToolUse` global tras la primera edición de cada archivo, con gate de medición post-edición (Q8).

## Q&A log

### Q1 — Alcance
- Asked: ¿solo mmorch, todos los repos Python del usuario, o cualquier repo (incluidos ajenos)?
- Captured: **c — cualquier repo, incluidos ajenos.** Motivo (propuesta de Claude, aceptada): el error del video aparece donde no hay registro previo.
- Flags: —

### Q2 — Disparo
- Asked: ¿tool a mano o hook PreToolUse automático antes de Edit/Write?
- Captured: **b — hook PreToolUse automático**, con tres reglas: (1) una vez por archivo por sesión; (2) nunca bloquea, solo agrega contexto; (3) fail-open: si el análisis falla, la edición pasa sin informe. Motivo: en el piloto del ticket 09 los agentes revisaron lectores solo cuando alguien les avisó; la opción a depende de que Claude se acuerde.
- Flags: —

### Q3 — Contenido
- Asked: ¿qué secciones? (1) lectores por literales compartidos, (2) tests que importan el módulo, (3) llamadores de codegraph, (4) preguntas de contrato por almacén.
- Captured: **solo 1 y 2, en ese orden.** 1 es la única sección con efecto medido (ticket 09); 2 es una línea con el comando pytest (ticket 01). 3 fuera: codegraph ya está a mano. 4 fuera hasta que un banco la mida.
- Flags: el usuario preguntó si codegraph sirve, si conviene desinstalarlo o reemplazarlo -> ver Q3b.

### Q3b — ¿Codegraph sirve?
- Asked (por el usuario): ¿codegraph sirve, conviene desinstalarlo, conviene un sustituto?
- Captured: 0.9.6 da "0 llamadores" falsos después de editar (confianza falsa); 1.6.0 corrige eso y resuelve alias; ninguna versión ve datos ni funciones como valor. Nunca se midió contra `grep -w` ni pyright. **Decisión del usuario: pausar el grilling, actualizar a 1.6.0 y medir la utilidad de codegraph antes de seguir.** Banco: llamadores reales verificados (ticket 02) como verdad; comparar recall y precisión de codegraph 1.6.0, `grep -w` y pyright.
- Flags: Q4 en adelante esperan el resultado del banco.
- **Resultado del banco (2026-09-25):** verdad = 1073 aristas reales (sys.setprofile sobre la suite completa), 604 funciones llamadas sin dunder. Recall de llamadores / "0 llamadores" falsos / ruido fuera de la verdad: codegraph 0.9.6 0.71 / 182 / 976; codegraph 1.6.0 0.74 / 62 / 1398; grep -w 0.83 / 68 / 3181; jedi 0.76 / 140 / 346; unión codegraph 1.6.0 + grep -w 0.83 / 45 / 3204. Conclusión: salir de 0.9.6; codegraph 1.6.0 complementa a grep -w (baja ceros falsos 68 → 45); jedi no suma recall a grep; ninguna herramienta sola es confiable. Regla: unir codegraph y grep -w, nunca tomar un cero como prueba. Sin medir: valor de navegación (`context`, `explore`). Código: `../orch-spike/scripts/impacto/compare_callers.py`.
- Actualización hecha (2026-09-25): el usuario eligió que Claude detenga los procesos `codegraph serve`; instalador oficial -> 1.6.0; reindex `--force` de mmorch (10 s) y de Claude, Estudio, Minecraft, Portfolio financiero y Proyecto_Adepor. Hueco conocido en 1.6.0: no resuelve imports con alias dentro de una función (`providers.call` -> `budget.check`); `grep -w` sí lo encuentra. Regla guardada en memoria (codegraph-union-grep). El usuario reinicia Claude Code para cargar el MCP nuevo.

### Q3c — Valor de navegación de codegraph 1.6.0 (pre-registro, antes de correr)
- Contexto: tras el reinicio, el MCP de 1.6.0 expone una sola tool, `codegraph_explore`; `callers`/`impact` quedan solo en la CLI. El usuario pidió medir codegraph.
- Diseño: 30 preguntas sobre mmorch, generadas por deepseek-chat (20 de ubicación desde el docstring de una función real, sin nombrarla; 10 de llamadores con verdad del trazado en runtime). Agente JSON del ticket 09, deepseek-v4-pro, 2 repeticiones. A = list_files, read_file, grep. B = A + `codegraph explore` (CLI, misma salida que la tool MCP).
- Métricas: acierto (ubicación = función exacta; llamadores = recall contra la verdad), tokens de entrada, pasos.
- Criterio: B sirve si ahorra >= 20% de tokens sin perder más de 5 puntos de acierto, o si gana >= 10 puntos de acierto. Costo estimado US$4.
- **Resultado (NEGATIVO, US$4.51, 120 corridas):** acierto A 0.89 vs B 0.88; tokens mediana A 19.8k vs B 32.2k; ubicación 95% vs 92.5%; llamadores 77% vs 78%. El agente usó explore en 26/60 corridas de B; cuando lo usó, gastó 22% más tokens con el mismo acierto. Límites: agente deepseek-v4-pro (no Claude), un solo repo, preguntas desde docstrings. Código: `../orch-spike/scripts/nav/`.
- Recomendación de Claude: mantener codegraph 1.6.0 por `callers` en la CLI (unión con grep -w); considerar quitar el servidor MCP (una sola tool sin valor medido); apagar la telemetría (hoy prendida).
- Decisión del usuario (2026-09-25): quitar el MCP de codegraph de Claude Code y apagar la telemetría. Hecho: `codegraph uninstall --target claude --location global --keep-cli` (solo quitó la entrada MCP de ~/.claude.json y 7 permisos de settings.json; Cursor conserva su MCP; la CLI queda) y `codegraph telemetry off`. Respaldos en el scratchpad de la sesión.
- Sección "CodeGraph" de ~/.claude/CLAUDE.md actualizada a pedido del usuario (2026-09-25): unir callers de la CLI con grep -w, `sync` antes de `callers` (sin MCP no hay watcher), un cero no prueba nada, navegar con grep + lectura.

### Q4 — Tamaño y recorte
- Asked: ¿cuánto mide el informe y qué se recorta primero?
- Captured: **confirmado.** (1) Orden por especificidad: primero los literales presentes en menos módulos. (2) Un literal en más de 5 módulos se resume en una línea con su conteo, sin lista. (3) Tope de 10 líneas. (4) Si el tope corta literales, la última línea dice cuántos quedaron afuera. Motivo: en el ticket 09, deepseek-chat falló t01 porque 12 señuelos junto a `total` enterraban al lector real.
- Flags: —
- Nota (pregunta del usuario): ¿valía la pena quitar el MCP si ahora hay que correr `sync`? Medido: `sync` tarda 1.3 s sin cambios y 4.2 s con un archivo cambiado; se encadena con `callers` y se usa pocas veces. El costo real del MCP era su bloque de instrucciones en cada sesión, que empujaba a `explore` (+22% tokens, mismo acierto). Decisión se mantiene.

### Q5 — Dónde vive el código
- Asked: ¿script suelto en ~/.claude/hooks/ (solo stdlib) o módulo testeado en mmorch + hook fino?
- Captured: **b — `mmorch/impacto.py` testeado + hook fino que lo llama, fail-open**, con un **test de guarda** que falla si algún hook de ~/.claude/hooks/ referencia un módulo de mmorch inexistente. Motivo: el ticket 02 encontró dos hooks que llamaban a `mmorch.context_blocks`, borrado por la poda, y fallaron en silencio 10 días.
- Flags: —

### Q6 — Lenguajes
- Asked: fuera de Python, ¿(a) solo Python v1, (b) regex de literales para todo lenguaje, (c) extractor propio por lenguaje con su parser?
- Captured: **c — extractor propio por lenguaje, con parser.** (Claude recomendaba a; el usuario eligió c.) Repos del usuario fuera de Python: Estudio (TypeScript), ChatBot (Java).
- Decisión técnica de Claude (no pregunta): un solo mecanismo de parseo para todos los lenguajes — tree-sitter — así cada lenguaje es una tabla de tipos de nodo de literal, no un extractor nuevo. Python puede quedar en `ast` (medido) o pasar a tree-sitter si da lo mismo.
- Flags: gate de medición por lenguaje -> Q7.

### Q7 — Gate por lenguaje
- Asked: ¿el informe se activa en un lenguaje apenas el extractor pasa sus tests, o recién después del banco de tareas del ticket 09 en ese lenguaje?
- Captured: **b — solo después de pasar el banco de tareas en ese lenguaje, con el mismo criterio (+25 puntos en acopladas, sin perder más de 10 en control).** Motivo: el 42% → 96% de Python no se transfiere solo; en TypeScript los literales de datos pueden vivir en tipos, constantes exportadas o esquemas, y un informe ruidoso hace fallar a modelos baratos.
- Decisiones técnicas de Claude (no preguntas): el hook registra cada informe inyectado (archivo, líneas, tiempo) en un jsonl con dueño único, para auditar el uso real; la deduplicación "una vez por archivo por sesión" usa el session_id que Claude Code pasa al hook.
- Flags: —

### Q8 — Cursor (control final)
- Asked: ¿queda algo sin tocar? El usuario: el informe también debe aparecer en Cursor.
- Verificado en docs (2026-09-25): Cursor no tiene hook que agregue contexto ANTES de editar; `postToolUse` sí agrega `additional_context` después de cualquier tool. Claude Code sí permite `additionalContext` en `PreToolUse` (Q2 se sostiene); el hook no debe devolver `permissionDecision: "allow"`, para no saltear los permisos del usuario.
- Captured: **confirmado — Cursor con un hook `postToolUse` global (`~/.cursor/hooks.json`) que agrega el informe después de la primera edición de cada archivo, reutilizando `mmorch/impacto.py`, y con gate:** entra solo tras pasar el banco del ticket 09 con el informe inyectado después del primer `write_file` (el banco midió el informe al inicio).
- Flags: —

## Cierre (2026-09-25)
- Revisión de consistencia: Q3 excluyó los llamadores de codegraph del informe "porque ya está a mano"; tras Q3b-Q3c el MCP se quitó, pero la exclusión se sostiene: la regla de llamadores (CLI + grep -w) vive en CLAUDE.md, fuera del informe.
- Trabajo que sale de este grilling (tickets propuestos): implementar `mmorch/impacto.py` + hook Claude Code + test de guarda (reemplaza el alcance del ticket 05); gate de Cursor con informe post-edición; extractor tree-sitter + banco para TypeScript y después Java.

## Open flags (pending input)
