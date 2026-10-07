# Pre-registro del ticket 06: Cursor como explorador y como borrador de plan

Estado: borrador del pre-registro. Este diseño congela todo antes de la primera corrida de un brazo.
Fecha: 2026-10-01. No se corrió nada.
Origen: panel de diseño (3 diseñadores, 3 jueces, síntesis), workflow `wf_0548f9f4-744`. Código: `orch-spike/scripts/cursor_roles/`.

## 0. Decisiones del usuario (2026-10-01)

- Los brazos de Claude usan Opus (`--model opus`).
- El cupo se gasta escalonado y con anclaje: primero el rol A con su mirada intermedia; después el usuario decide el rol B.
- El rol B es un piloto con implementación: el implementador DeepSeek restringido corre sobre cada plan.
- Los umbrales quedan como los propone la sección 9.
- La nota de `semSearch` contra grep queda reemplazada (sección 2).
- Pedido del usuario: el rol B compara además cuatro borradores sin Opus (sección 6.6).
- Prueba de humo (2026-10-01): Claude corre solo con Read, Grep y Glob y sin MCP; una corrida de Opus cuesta US$0.11-0.15 equivalentes. Cursor en `--mode plan` deja el plan en la herramienta `createPlan` y no en `result`; el código toma el plan de esa herramienta.
- El CLI de Claude se actualizó de 2.1.278 a 2.1.287: con 2.1.278, `opus` corría `claude-opus-5`; con 2.1.287 corre `claude-opus-5-5`, el modelo de la sesión.

## 0.1 Cambios de la revisión adversarial (2026-10-01, antes de congelar)

Una revisión con 4 lentes y un refutador por hallazgo confirmó 22 defectos del código (workflow `wf_74ddaf04-8b2`). El código los corrige así:

- La precisión de llamadores usa llamadas que ast resuelve: `f()` cuenta si el archivo define `f` o lo importa del módulo, y `m.f()` cuenta si `m` es un alias del módulo. El emparejado por nombre acreditaba `subprocess.run`.
- El tag (`RESPUESTA:`, `ARCHIVOS:`, `PLAN_ARCHIVOS:`) no salta de línea. Una lista debajo de un tag vacío es una falla de parseo, igual en todos los brazos.
- `plan_ok` no cuenta un archivo de test, con o sin la carpeta `tests/`.
- La regla de 5.1 cuenta llamadores sobre la verdad cruda, como `gen_questions.py`, y exige al menos un llamador en la verdad normalizada. `cal11` pasó de `paths.py::logs_dir` a `paths.py::data_dir`.
- Un borrador de llamadores es correcto si su F1 vale 1. Si la verdad no tiene un llamador de otro archivo (cal06), el plantado borra uno del mismo archivo.
- El plantado de llamadores borra también la evidencia que cita líneas del llamador borrado, y el nombre de su archivo si no queda otro llamador ahí. El plantado de ubicación reemplaza también el nombre corto del archivo.
- El plantado de omisión en B borra los lectores por nombre corto, ruta o módulo, y borra solo las frases que los nombran.
- Los plantados de control c02 y c06 cambian: c02 contradice "No cambies los otros campos"; c06 no tiene restricción explícita, y su línea amplía el alcance a un cambio que la tarea no pide. El plantado agrega además el archivo a ARCHIVOS.
- `adopted_planted` se calcula solo si la salida de Claude se parsea.
- Un DeepSeek que no llama a `done` puntúa como falla de parseo; no es una falla técnica.
- Un ítem sin borrador de Cursor sale de A1/B1 y de A2/B2, y queda en `errores_roles.jsonl`. Cada fila se escribe al terminar su corrida.
- El implementador recibe el plan completo de Opus, no la cola.
- `analyze.py` decide con valores sin redondear. Sin pares de anclaje plantados, el veredicto del rol A es "SIN DATOS DE ANCLAJE", no ADOPTAR. El rol B informa sus condiciones propias, los intervalos de Wilson y el anclaje separado por omisión y por comisión.
- `analyze.py` informa además el recall crudo, el cociente bruto, las métricas secundarias, el cupo de sesión con 50k y 150k tokens de contexto, las versiones por brazo y si el t pareado discrepa del bootstrap.
- Límite conocido: el plantado no edita los conteos en prosa. Un resumen puede decir "dos lugares" y listar uno solo.

## 1. Pregunta

La pregunta tiene dos partes.

- Explorador: ¿Claude decide igual de bien y gasta menos cupo si Cursor explora primero y le pasa un resumen corto?
- Borrador: ¿Claude decide igual de bien y gasta menos cupo si Cursor escribe primero un borrador de plan y Claude lo aprueba o lo corrige?

El riesgo principal es el anclaje: Claude puede aceptar un resumen o un plan malo.

## 2. Cambio en la nota del ticket

La nota del ticket decide comparar "semSearch de Cursor contra grep de Claude". Esa comparación es imposible: semSearch no existe en `cursor-agent -p` (medido 2026-10-01). Este pre-registro la reemplaza por otra comparación. La comparación nueva enfrenta a Claude solo contra Cursor (grep/glob/read, pagado por el plan de Cursor) seguido de Claude. El usuario aprueba el cambio antes de congelar.

## 3. Diseño general

El diseño parte del banco "riguroso" y le injerta dos piezas de los otros. La primera es la compuerta a cero cupo del banco "mínimo". La segunda es el defecto plantado por edición quirúrgica del banco "realista".

El banco tiene tres fases.

1. Fase 0, a cero cupo: se arman los ítems, Cursor escribe sus borradores y una compuerta decide qué rol sigue.
2. Fase 1, con cupo: Claude corre pareado por ítem, sin borrador y con borrador, más un sub-experimento de anclaje.
3. Fase 2: un script congelado aplica la regla de decisión.

El rol A (explorador) corre primero. El rol B (borrador) corre después. El análisis intermedio de A puede cortar el banco por futilidad.

## 4. Hechos verificados que el diseño usa

- El snapshot de nav es el commit `4b9b33c` de orch-spike. `git diff 4b9b33c..HEAD -- mmorch/` sale vacío. El paquete mmorch/ no cambió desde el snapshot.
- `run_nav.score` usa la regex `KEY = (?:mmorch/)?([\w/]+\.py)::(\w+)`. En llamadores, el score es recall puro, sin penalidad por listas largas.
- La verdad de cal00 incluye `auto_apply.py::<lambda>`. La verdad de cal06 incluye cinco entradas `::<module>`. La regex KEY no puede capturar esos nombres. Por eso recall 1 es inalcanzable en esos dos ítems con el oráculo actual.
- Línea base DeepSeek en nav: ubicación 0.95, llamadores 0.77. Ubicación está en techo: solo loc00 y loc16 bajan de 1.0.
- Línea base DeepSeek en tareas: sin informe 10/24 en acopladas, con informe 23/24, control 12/12.
- Ticket 04: el brazo Claude (Sonnet 5) leyó unos 376k tokens de cache por tarea. Cursor grok tardó una mediana de unos 158 s por tarea.

## 5. Rol A: explorador

### 5.1 Ítems (50)

- 20 preguntas de ubicación: loc00 a loc19 de `nav/questions.json`, sin cambios.
- 10 preguntas de llamadores: cal00 a cal09, sin cambios.
- 20 preguntas de llamadores nuevas: salen del pool de `impacto/callers_truth.json`, sin LLM.

Las 20 preguntas nuevas se eligen con una regla fija.

- Entran solo funciones con 2 o más llamadores y al menos un llamador en otro archivo.
- Quedan afuera las funciones con más de 12 llamadores.
- Quedan afuera las 10 funciones de cal00 a cal09.
- Primero entran las de 3 o más llamadores, por sorteo con semilla 20261001. El resto se completa por sorteo con la misma semilla.
- El texto de la pregunta usa la plantilla de `gen_questions.py`.

El estrato de ubicación está en techo y no carga la señal de calidad. Sí carga señal de cupo y de anclaje. El estrato de llamadores carga la señal de calidad.

### 5.2 Workspace

- Una copia temporal sale de `git archive 4b9b33c mmorch/`, con `git init` y un commit.
- La copia no lleva scripts/, questions.json, callers_truth.json, results_*.jsonl, logs/ ni .codegraph.
- Un validador corre antes de cada corrida. El validador aborta si encuentra un id loc/cal, la cadena `callers_truth` o un archivo de tests ocultos.
- Un validador AST comprueba que cada respuesta verdadera existe en la copia.

### 5.3 Brazos

- **Acur (Cursor solo, cero cupo).** `cursor-agent -p --mode ask --trust --model grok-4.7-medium --output-format json --workspace <copia>`. El entorno no lleva SHELL, BASH, BASH_ENV, MSYSTEM ni CLAUDECODE. `<copia>/.cursor/cli.json` lleva `deny: ["Mcp(*:*)"]`. Cursor corre 3 repeticiones por ítem.
- **A0 (Claude solo).** Claude responde la pregunta con Read, Grep y Glob, sin borrador.
- **A1 (Cursor y después Claude).** Claude recibe el prompt de A0 más el resumen de la repetición 1 de Cursor.
- **A2 (anclaje plantado).** Claude recibe el prompt de A1 con un resumen plantado. Corre sobre 20 ítems sorteados con semilla 20261001.

### 5.4 Prompts congelados

Pedido a Cursor:

```
Pregunta: {q}
Respondé explorando el repo. No edites nada. Máximo 15 líneas.
RESUMEN: 2 a 4 oraciones.
EVIDENCIA:
- mmorch/archivo.py:LINEA: <línea textual> (máximo 6)
RESPUESTA: mmorch/archivo.py::funcion[, ...] (ubicación: máximo 3 candidatos)
```

Prompt de A0:

```
Estás en una sesión de trabajo sobre el paquete mmorch/ de este directorio.
Necesitás responder esta pregunta para decidir el siguiente paso: {q}
Terminá con una línea exacta: RESPUESTA: mmorch/archivo.py::funcion[, ...] (ubicación: máximo 3).
```

A1 y A2 agregan este bloque antes de la instrucción final, con el mismo texto:

```
Un trabajador (Cursor) dejó este resumen:
<<<
{borrador}
>>>
Vos sos responsable del resultado. Usalo, corregilo o ignoralo como en una sesión normal.
```

El bloque no menciona errores ni trampas.

### 5.5 Defecto plantado (A2)

`plant.py` edita un borrador natural CORRECTO del mismo ítem. La regla elige el primer borrador correcto entre las repeticiones 1 a 3.

- Ubicación: la RESPUESTA cambia por un distractor determinista. El distractor es un llamador público en otro archivo. Si no existe, es una función llamada pública. Si no existe, es la función pública vecina del mismo archivo. La línea de EVIDENCIA cambia por la línea `def` real del distractor.
- Llamadores: se borra el llamador verdadero que vive en otro archivo. Se borran también su línea de EVIDENCIA y toda mención de su nombre.
- Un grep comprueba que ningún nombre borrado sobrevive.
- Si ningún borrador del ítem es correcto, el ítem usa un borrador natural defectuoso con la etiqueta `defect_type=natural`.
- El análisis informa A2 con y sin los ítems de etiqueta natural.
- `planted.json` se congela con SHA-256 antes de correr Claude.

### 5.6 Oráculo

`cursor_roles/score.py` envuelve `run_nav.score` y lee solo la última línea `RESPUESTA:`.

- Ubicación: score 1 si acierta con 3 candidatos o menos, si no 0. Es `run_nav.score` sin cambios.
- Llamadores: score = F1.
- La verdad normalizada quita las entradas `<lambda>` y `<module>`, porque la regex KEY no las captura. Esta regla corre igual en todos los brazos. El análisis informa también el recall crudo contra la verdad completa.
- Recall: contra la traza en runtime normalizada.
- Precisión: contra la unión de la traza y las llamadas que resuelve `static_callers.py` con ast. Así un llamador estático real ausente de la traza no castiga.
- Una salida sin línea RESPUESTA vale 0 en todos los brazos. El análisis informa la tasa de fallas de parseo.
- Ningún LLM-juez.

### 5.7 Métricas por fila

- Calidad: `score`, `recall`, `precision`, `f1`, `parse_ok`.
- Anclaje: `adopted_planted` (la respuesta contiene la clave plantada o omite el llamador borrado).
- Mecanismo: `verifico_evidencia` (Claude abrió el archivo citado, según los eventos tool_use de stream-json).
- Cursor: `draft_score`, `cursor_usage` (input, output, cacheRead), `cursor_secs`, `summary_tokens`, `cursor_cli_version`.

## 6. Rol B: borrador de plan (piloto)

### 6.1 Ítems (12)

- Las 12 tareas de `tareas/bench.py`: 8 acopladas (t01 a t08) y 4 de control.
- El repo sale de `bench.materialize(files, repo, seed=rid)` con los 30 señuelos, `git init` y un commit.
- El pedido NO lleva el informe de impacto. Descubrir a los lectores es el trabajo del plan.
- El repo no lleva tests ocultos ni ref_full.

Este rol es un piloto. Con 12 ítems el banco no detecta una pérdida de 10 puntos. El piloto solo puede dar "no adoptar" o "prometedor". Un "prometedor" abre un ticket ampliado con tareas nuevas.

### 6.2 Brazos

- **Bcur (Cursor solo, cero cupo).** `cursor-agent -p --mode plan` con el mismo entorno que Acur. Cursor corre 3 repeticiones por tarea.
- **B0 (Claude solo).** Claude explora el repo y escribe el plan que aprobaría.
- **B1 (Cursor y después Claude).** Claude recibe el prompt de B0 más el borrador de la repetición 1 de Cursor.
- **B2 (anclaje plantado).** Corre sobre las 12 tareas.
  - En acopladas hay un defecto de omisión. `plant.py` borra los lectores del borrador real y el plan queda igual a `ref_writer_only`.
  - En control hay un defecto de comisión. Una línea de CAMBIOS contradice la restricción explícita de la tarea. Las 4 líneas se escriben y se congelan en `planted.json` antes de correr Claude.

### 6.3 Prompts congelados

Pedido a Cursor:

```
TAREA: {prompt}
No edites nada. Escribí el plan de implementación, máximo 25 líneas.
ARCHIVOS: a.py, b.py
CAMBIOS:
- a.py: <qué cambia>
RIESGOS: <1 a 3 líneas>
```

Prompt de B0:

```
Tarea que vas a delegar en un implementador: {prompt}
Explorá el repo de esta carpeta y escribí el plan que aprobarías, con ARCHIVOS y CAMBIOS.
Terminá con una línea exacta: PLAN_ARCHIVOS: a.py, b.py
```

B1 y B2 agregan el mismo bloque neutral que A1, con la palabra "borrador de plan". También piden la línea `VEREDICTO: aprobado|corregido|reescrito`.

### 6.4 Oráculo

- Primaria, `plan_ok`, sobre `PLAN_ARCHIVOS` sin contar `tests/*`.
  - Acopladas: las claves de ref_full están incluidas en el plan, y el plan no nombra ningún `services/*`.
  - Control: las claves de ref_full están incluidas en el plan, y el plan no nombra módulos fuera de ref_full.
- Secundaria, `e2e_ok`: un implementador restringido ejecuta el plan aprobado.
  - El implementador es `tareas/agent.py` con deepseek-v4-pro y 15 pasos. Gasta cero cupo.
  - El pedido dice qué archivos puede tocar. Al terminar, el script revierte todo cambio fuera de PLAN_ARCHIVOS y de `tests/`. Así el implementador no rescata un plan malo. El campo `rescate_impl` cuenta los archivos revertidos.
  - Corre 3 repeticiones por plan. Después corre `bench.hidden_ok`.
  - Una calibración previa le da el plan oráculo (las claves de ref_full). Una tarea que pasa menos de 2 de 3 veces con el plan oráculo se marca como ruido del implementador en el análisis secundario.
- El mismo oráculo puntúa el borrador de Cursor (`draft_plan_ok`).

### 6.5 Métricas por fila

`plan_files`, `plan_ok`, `veredicto`, `adopted_planted`, `e2e_passes` (lista de 3), `rescate_impl`, `draft_plan_ok`, y los mismos campos de cupo y de Cursor del rol A.

### 6.6 Comparación de borradores sin Opus (pedido del usuario, 2026-10-01)

Pregunta: ¿qué modelo conviene como planner o armador de specs, por calidad y por recurso que gasta?

- Borradores: Cursor `grok-4.7-medium` (pool "Cursor Models"), DeepSeek `deepseek-v4-pro` (API en dólares), Claude Haiku y Claude Sonnet (cupo del plan de Claude).
- Cada borrador corre 3 repeticiones sobre las 12 tareas, con el pedido de la sección 6.3.
- Cursor corre con `--mode plan`. Haiku y Sonnet corren con la configuración de la sección 7. DeepSeek corre con `tareas/agent.py` sobre una copia del repo; la copia se descarta, y el plan sale del resumen de `done`.
- Oráculo: `draft_plan_ok` (sección 6.4) y `e2e_ok` con una corrida del implementador restringido por borrador.
- Recurso: dólares de DeepSeek, `total_cost_usd` equivalente de Claude, tokens de Cursor.
- Este brazo es descriptivo: el informe ordena los cuatro borradores por `draft_plan_ok`, después por `e2e_ok`, y anota el recurso. No hay umbral de adopción.
- Corre en la fase 0, junto con la compuerta, y no gasta Opus. Haiku y Sonnet gastan cupo de Claude: 72 corridas.
- Contexto medido: SDLC gastó US$13.64 de API en 82 corridas desde el 2026-09-01; el plan es una llamada de las varias de cada corrida.

## 7. Configuración de Claude

- Comando: `claude.exe -p --model <opus|sonnet|haiku> --restricted --strict-mcp-config --tools Read,Grep,Glob --no-session-persistence --output-format stream-json --verbose`.
- `--restricted` quita las herramientas que ejecutan código e ignora los settings de usuario, proyecto y local. Así no corre ningún hook. `--strict-mcp-config` deja a Claude sin MCP.
- El prompt va por stdin, porque `--tools` es variádico.
- El cwd es el workspace temporal. El entorno no lleva CLAUDECODE.
- El CLAUDE.md global sigue cargado en todos los brazos, igual que en una sesión real.
- Un timeout de 20 minutos limita cada corrida.
- Cada corrida de Claude es un proceso nuevo.
- Cursor corre con el `node.exe` y el `index.js` de la versión más nueva al empezar el lote. La versión queda fija durante todo el lote.

## 8. Métrica de cupo

- `cupo_usd` = `total_cost_usd` del evento final `result`. Es el equivalente a precio de lista y pondera input, cache_creation, cache_read y output.
- `piso` = mediana de 5 corridas de calibración con el prompt "Respondé OK." y los mismos flags.
- `net_cost` = max(`cupo_usd` − `piso`, 0.10 × `piso`). El piso del 10% evita cocientes cerca de cero o negativos.
- En A1 y B1 se suma un costo imputado: los tokens del pedido a Cursor por el precio de output del modelo. Ese texto lo escribe Claude en el flujo real.
- Ahorro por rol = 1 − media geométrica de net_cost(con Cursor) / net_cost(Claude solo), pareado por ítem.
- Secundarias: output_tokens, cache_read_input_tokens, cache_creation_input_tokens, input_tokens, num_turns, tool_calls, duration_ms. El análisis informa también el ahorro bruto, sin restar el piso.
- Extrapolación informativa, sin corte: cupo_sesion(C) = net_cost + num_turns × C × precio de cache read, con C = 50k y 150k tokens de contexto previo.
- Los tokens y segundos de Cursor se informan y no deciden.

## 9. Regla de decisión

### 9.1 Unidad y test

- La unidad de análisis es el ítem. Las repeticiones de Cursor y del implementador se promedian dentro del ítem. Nunca cuentan como n.
- Alfa unilateral 0.05.
- Cada rol se decide aparte con una prueba de intersección-unión. El rol tiene que pasar todas las condiciones. Esta prueba no necesita corrección de alfa.

### 9.2 Compuerta a cero cupo (fase 0)

Un rol no gasta cupo si cumple cualquiera de estas condiciones.

- El score medio de Cursor solo (Acur o Bcur) sobre los ítems del rol es menor que 0.60.
- La tasa de parseo del borrador de Cursor es menor que 0.90.

Razón: si Cursor falla seguido, Claude rehace el trabajo y no hay ahorro posible. El veredicto es "no apto con grok".

### 9.3 Rol A (explorador)

1. **Calidad, no inferioridad.** d_i = score(A1_i) − score(A0_i). Pasa si la cota inferior unilateral al 95% de la media de d es mayor que −0.10. La cota sale de un bootstrap pareado por ítem con 10000 remuestreos y semilla 20261001. Un t pareado sirve de sensibilidad. Si la sensibilidad discrepa, el análisis lo informa y decide el bootstrap. El margen de 0.10 es el criterio "apto" del ticket 04.
2. **Cupo.** Pasa si la media geométrica del cociente es 0.70 o menos (ahorro de 30% o más). La cota superior al 95% del cociente también debe ser 0.85 o menos. Un Wilcoxon sobre log(cociente) contra log(0.85) sirve de sensibilidad.
3. **Anclaje.** caída = media de score(A0) − media de score(A2) sobre los 20 ítems plantados. El anclaje es severo si la caída supera 0.25 y un test de signos exacto unilateral sobre los pares da p < 0.05.

### 9.4 Rol B (piloto)

1. **Calidad.** plan_ok(B1) en las 12 tareas es al menos plan_ok(B0) menos 1 tarea. El análisis informa la cota bootstrap y el intervalo de Wilson, sin test.
2. **Control.** B1 no pierde ninguna tarea de control frente a B0.
3. **Cupo.** Misma regla que el rol A, informada con su intervalo.
4. **Anclaje.** El análisis informa adopted_planted en acopladas (omisión) y e2e_ok en control (comisión), sin corte.

### 9.5 Veredictos

- **ADOPTAR (solo rol A):** pasan calidad, cupo y anclaje.
- **ADOPTAR CON REGLA (solo rol A):** pasan calidad y cupo, y hay anclaje severo. El rol entra al protocolo solo con una regla obligatoria: Claude abre la evidencia citada. Un ticket nuevo mide esa regla con su propio pre-registro.
- **NO ADOPTAR:** falla calidad o falla cupo.
- **INFERIOR DEMOSTRADO:** la cota superior de d es menor que −0.10.
- **PROMETEDOR (solo rol B):** pasan las condiciones 1 a 3 del piloto. Se abre un ticket ampliado con tareas nuevas.

### 9.6 Análisis intermedio (solo rol A)

Hay una sola mirada, a los 25 ítems en el orden sorteado. La mirada solo puede cortar por futilidad. El banco corta si el ahorro estimado es menor que 15% o si la cota superior de d es menor que −0.10. No hay corte por eficacia, así la mirada no infla el alfa.

### 9.7 Reglas de borde

- Una falla técnica (timeout, error del proveedor, salida vacía de Cursor) se reintenta una vez. Va a `errores_roles.jsonl`. No cuenta como defecto.
- Una fuga detectada en un workspace anula el rol completo.
- No se repite ninguna corrida para salir de una zona gris.
- Un cambio de versión de un CLI a mitad del lote se informa por estrato.

## 10. Riesgos de sesgo y neutralización

- **Fuga del oráculo.** El workspace de A sale de git archive y lleva solo mmorch/. El repo de B no lleva tests ocultos ni ref_full. Un validador aborta ante cualquier rastro.
- **Explorador gratis.** El informe de impacto no va en ningún pedido. Claude no tiene Edit, así que el hook de impacto no dispara. Cursor corre en ask o plan, que son de solo lectura.
- **Herramientas ajenas.** Cursor corre sin MCP (modos ask y plan, más `deny Mcp(*:*)`). Claude corre con `--strict-mcp-config`. La prueba de humo verifica las dos cosas.
- **Estilo delator del defecto.** El defecto plantado es una edición quirúrgica de un borrador real, con una línea de evidencia real del repo. Ningún LLM escribe un borrador falso.
- **Encuadre.** A1, A2, B1 y B2 usan el mismo bloque neutral. El bloque no dice "verificá" ni "puede tener errores".
- **Prevalencia irreal de defectos en A2 y B2.** Cada corrida es un proceso nuevo. Claude no aprende la tasa de defectos. El anclaje medido es condicional a un borrador malo, y el informe lo dice así.
- **Selección del borrador plantado.** La regla queda congelada. El análisis informa A2 con y sin los ítems de etiqueta natural.
- **Techo en ubicación.** El estrato de ubicación es descriptivo para la calidad. La señal de calidad sale de llamadores.
- **Recall inflado en llamadores.** El score es F1 con precisión contra la unión de traza y ast.
- **Verdad inalcanzable.** Las entradas `<lambda>` y `<module>` salen de la verdad normalizada, igual para todos los brazos.
- **Rescate del implementador.** La primaria de B es plan_ok, que solo depende de la decisión de Claude. El implementador restringido no puede tocar archivos fuera del plan.
- **Pseudo-replicación.** El ítem es la unidad.
- **Cache de prompts.** Los trabajos de todos los brazos se intercalan en un orden sorteado con semilla 20261001. Cada fila guarda cache_creation y cache_read por separado.
- **Piso fijo de claude -p.** El ahorro usa net_cost, con el piso medido restado y un límite inferior.
- **Costo del pedido omitido.** El pedido a Cursor se imputa en A1 y B1.
- **Deriva de versiones.** Cada fila guarda la versión de cursor-agent, la versión de claude y el modelo efectivo.
- **Grados de libertad del analista.** `analyze.py`, los umbrales, los prompts, `items_frozen.json`, `truth_frozen.json` y `planted.json` se congelan con SHA-256 y commit antes de la fase 1. Los hashes van en la sección Notes del ticket 06.
- **Contaminación por entrenamiento.** mmorch es público en GitHub. El efecto es igual en todos los brazos de A. Los repos de B son sintéticos.
- **Alcance de la población.** Las preguntas de nav son de ubicación y de llamadores sobre un solo paquete. El resultado no cubre preguntas de diseño abiertas.

## 11. Conteo de corridas

Claude (todas de solo lectura):

- Rol A: 50 A0 + 50 A1 + 20 A2 = 120.
- Rol B: 12 B0 + 12 B1 + 12 B2 = 36.
- Piso: 5.
- Prueba de humo: 4 (A0 y A1 sobre una pregunta extra de llamadores, B0 y B1 sobre t05). Sus resultados se descartan.
- Total máximo: 165. Si A corta por futilidad a los 25 ítems: unas 70. Si las dos compuertas fallan: 0.

Cursor:

- Rol A: 50 ítems × 3 = 150.
- Rol B: 12 tareas × 3 = 36.
- Humo: 2.
- Total: 188. Con unos 158 s por corrida y concurrencia 3, el lote tarda unas 3 horas.

Comparación de borradores (sección 6.6): 36 corridas de Haiku y 36 de Sonnet (cupo de Claude, sin Opus), 36 de DeepSeek y 36 de Cursor ya contadas en el rol B.

DeepSeek (cero cupo): 36 corridas de calibración, 108 del implementador sobre los planes de Opus, 144 sobre los borradores de la sección 6.6 y 36 borradores. El costo exacto se registra y no decide.

El costo de cupo por corrida de Opus no está medido. La prueba de humo lo mide antes de lanzar. Si la proyección supera el presupuesto aprobado, el usuario decide antes de seguir.

## 12. Potencia (rol A)

- La desviación estándar de d se estima entre 0.25 y 0.35. No está medida para Claude.
- Con 50 ítems, delta 0.10 y alfa 0.05 unilateral, la potencia es de unos 0.88 con DE 0.25 y unos 0.76 con DE 0.30.
- El techo de ubicación baja la DE de esos pares y sube la potencia real.
- Las repeticiones de Claude no suman potencia, porque la varianza dominante es entre ítems.
