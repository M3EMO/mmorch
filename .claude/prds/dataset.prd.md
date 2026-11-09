# PRD: Dataset fiable para Root y Graft

**Estado:** borrador de Opus (rol planner de Root), 2026-11-06. P72: "Testear si tiene sustento y despues implementarlo en su totalidad dsps del cierre". Se implementa entero si pasa "Prueba de sustento". P70 a, P71 a, P73 d, P75, P76 y P83 están decididas. P83: "P83 a + b en paralelo".
**Dueño:** Root. Graft consume el contrato C1 a C6.
**Cuándo:** se implementa después de cerrar el hito 1 de Root. Este borrador no necesita podman, y por eso se adelanta.

## Problema

Root se calibró sobre memora y brasa, dos tareas propias. Un evaluador medido solo en dos repos no prueba que sirva en código ajeno. Graft necesita además una vara común, para saber si mejora.

Palabras del usuario, citadas en la nota del plan común: "prioricemos tener el dataset y que sea fiable y construyamos ambos graft y root sobre ella". Y: "Para root tambien deberia ser de todos los codigos de gh al igual que graft no?".

## Decisiones ya tomadas

| Tema | Decisión | Fuente |
|---|---|---|
| Piloto | Solo Rust | Usuario, relatado por la sesión de Graft |
| Presupuesto | "Presupuesto 50 de api, si hace falta mas agregaremos" | Usuario, relatado por Graft |
| Vía | "Usar api claude, el key esta en env". La clave se carga solo en los procesos del dataset | Usuario, relatado por Graft; el alcance sale del pedido del orquestador |
| Cómputo | Sin acceso a la nube: "cambiarlo a api". Corre en la laptop | Usuario, relatado por Graft |
| Borrado | Memora y brasa se borran "Cuando el dataset calibre" | Decisión 70 |
| Etiquetas | Ninguna etiqueta sale de un juez LLM | Propuesta del plan común, sin aprobar |
| Vía | U4: "Root y graft deben poder correr por cupo y api". La clave se carga en los procesos por API. Los brazos base pueden ir por cupo y no gastan los 50 USD | Usuario, citado por Graft, sin comprobar |
| ECC | Graft propuso "ECC construye las herramientas del dataset y también corre como brazo base", y el usuario respondió "Sí, las cuatro" | Citado por Graft. **Sin verificar:** no aparece en `graft.plan.md` ni en `graft.prd.md` |
| Saboteador y curva | P75 a: los dos entran | Usuario |
| Clave | P70 a: clave dedicada con límite de gasto, como variable del contenedor. El usuario la crea antes de la primera corrida de builder por API, en D2 | Orquestador: el usuario devolvió el texto con la recomendación, sin cambios |
| Juez | P71 a: solo los builders van por Claude. El juez sigue en DeepSeek | Orquestador: igual que P70 |
| Mapa | P76: "Por ahora solo medir. Despues evaluamos si genera" | Usuario |
| Recall real | P73 d: Root no evalúa un repo real hasta cumplir el recall sobre el dataset | Usuario |

**Conflicto resuelto por la cita más nueva.** P19 b decía que Graft construye en la nube. "cambiarlo a api" la reemplaza.

## Fuera del alcance

- Otros lenguajes.
- Una red neuronal de pesos. Entra solo si supera a un método simple, según el PRD de Graft.
- La construcción de Graft.

## Criterios de admisión de una tarea

Una tarea sale de un commit público con licencia permisiva. Entra solo si cumple todo. Cada criterio es un chequeo determinista.

1. **Rojo y verde.** Los tests elegidos fallan antes del commit y pasan después.
2. **Estable.** Tres corridas seguidas dan lo mismo, sin red durante los tests.
3. **Tests que matan.** La muestra de `cargo-mutants` mata al menos 1 mutante del código del commit. La muestra usa el reparto parejo de la Tarea 65.
4. **Reproducible.** La imagen fija el hash del commit, `Cargo.lock` y las dependencias, en modo sin red.
5. **Sin fuga.** La spec no menciona el arreglo ni trae el código final. El dato medido lo exige: SWE-Bench+ halló la solución en el issue en 32,67 % de los parches exitosos, y tests débiles en 31,08 % (arXiv 2410.06992).

**Tamaño.** Se fija un tope de archivos tocados. La corrida real de memora tardó más de 2 horas, con 25 de 57 tareas. El piloto prefiere commits chicos.

## Contrato para Graft

### C1: partición visible y oculta
- La partición es por repo, no por commit, para evitar fuga entre tareas.
- `r00t dataset listar --visible --json` da solo nombres visibles.
- Un nombre oculto da código 2.
- **Comprobación:** un fixture con un nombre oculto da 2.

### C2: tareas con el contrato de `tarea.toml`
- Cada tarea vive en `$ROOT_HOME/oraculos/<nombre>/`, con el cargador de hoy (`src/oracle/tarea.rs`).
- Graft lee `lenguaje`, `imagen`, `spec` y `contrato.cmd`. No abre `oculto/`.
- **Técnico, verificado:** `cargo test` no da junit, y la imagen no trae `cargo nextest`. Nextest emite JUnit con `[profile.ci.junit]`. Root construye una imagen nueva. `borrador_tarea` tiene comandos de pytest fijos (`src/ficha.rs:375-376`), y hay que cambiarlos. Una suite que no es pytest ya está soportada (`src/oracle/mod.rs:2212`).
- **Comprobación:** cada tarea visible evalúa sin código 2.

### C3: secuencias de commits consecutivos
- Una secuencia es una lista ordenada de pasos de un mismo repo.
- Cada paso trae el hash de su estado inicial.
- Motivo medido: SWE-Milestone cae de más de 80 % en tareas aisladas a 38,03 % en continuidad (arXiv 2603.13428).
- **Comprobación:** el hash de cada estado inicial se reproduce.

### C4: vara versionada
- La vara tiene versión, banda de aceptación, piso fijo y tareas ancla que nunca cambian.
- La banda se mueve por versión, no por build, según el PRD de Graft.
- P15 c del usuario: "P15 c despues un 10-25% mejor". Root exige más solo en una dimensión confiable.
- Banda inicial: "El 25 % superior al inicio, y se revisa después del piloto", según Graft.
- El 10-25 % se mide "Sobre la distancia al techo del puntaje de rúbrica", según Graft. **Sin verificar con ese texto:** `graft.prd.md` dice "la distancia al techo del puntaje de rúbrica".
- Graft calcula su puntaje con `Fila.minutos_pared` cuando exista, y usa el run-log hasta entonces.
- Cada fila guarda la versión.
- **Comprobación:** dos filas de versiones distintas no se comparan.

### C5: `tipo` de cada tarea
- `tarea.toml` no cambia. El `tipo` vive en `dataset.toml`, junto a la tarea.
- Los valores salen de `tipos` en los flujos de Graft. Hoy existe `mvp-spec`.
- **Comprobación:** un tipo desconocido da código 2.

### C6: `## Ficha` válida
- `evaluar` exige ficha, y un commit no la trae (`src/oracle/mod.rs`, `verificar_ficha`).
- Un modelo escribe el borrador desde el diff y la descripción del commit.
- `r00t ficha revisar` la valida con reglas deterministas.
- El criterio 5 corre sobre la ficha. La ficha no es una etiqueta: la etiqueta sale de los tests.
- **Comprobación:** cada tarea pasa `ficha revisar`.

## Calibración sobre el dataset

- **Cambio de forma.** Hoy una fila es por tarea (`Calibraciones::de`). Con 20 tareas y `n = 20` por tarea son unas 860 evaluaciones, cerca de 83 USD. El piloto calibra por versión del dataset, con una fila por dimensión.
- **Muestra.** Un defecto y un limpio por tarea y dimensión, en 20 tareas: 20 defectos y 20 limpios. El trío comparte limpios: unas 100 evaluaciones con juez.
- **Defectos inyectados.** El código de GitHub no es limpio por definición, así que la sensibilidad sale de defectos conocidos.
- **Código nuevo, verificado.** `Calibraciones::de` y `aplicar` buscan por nombre de tarea (`src/oracle/calibra.rs:131` y `:141`). Sin una fila por versión, una tarea nueva queda sin calibrar.
- **Datos por tarea, verificado.** `CicloEntreCapas` necesita `[arquitectura].capas`, y `BucleCuadratico` necesita `camino_caliente`. Un commit ajeno no los trae. D3 los deriva o la fila dice no aplica.
- **Tiempo, estimación.** Una evaluación de la validez tardó 22 minutos en promedio. 100 evaluaciones son unas 37 horas en serie. El reuso baja eso, pero cada tarea paga su propia base.
- **Cuándo es confiable una dimensión.** Cumple la regla de hoy (`Calibraciones::aplicar`): exactitud de 0,9 o más, al menos 5 defectos y 5 limpios, fila completa y como mucho 20 % de limpios sin veredicto. Además, la cota inferior de Wilson de su sensibilidad supera 0,7. Es una propuesta: con 20 defectos, 18 aciertos dan cerca de 0,78.

## Prerrequisitos en Root

Sin esto, el piloto mediría mal. Todos están en el plan de Root.

0. Revisar las fórmulas de Seguridad y Estructura, y fijar `[decision.minimo]` con datos. La corrida real de memora dio Seguridad 0,556 por 2 `unsafe` normales, y un "listo" sin mínimos.
1. El juez lee más allá de los primeros 6000 caracteres de cada archivo. Diseño detectó por cita 4 de 4 dentro del corte y 0 de 6 fuera.
2. Se revisa `sin_evidencia`. En memora, 3 de 10 limpios no dieron veredicto.
3. El tope cuenta el reintento y los casos en vuelo (hallazgo 5 de la revisión de cierre).
4. Una falla del modelo no se reusa en un limpio (hallazgo 18 y arreglo de la Tarea 67).
5. `Fila` guarda la huella de prompt, rúbricas y regla (hallazgo 19, sin verificar).
6. `r00t corre` lanza por API (R6), y `track comparar` conoce la vía (R12).
7. **Recall sobre defectos reales.** El hito 1 midió 0,125, con umbral 0,8, y la pasada 0 de 10. Root no evalúa un repo real hasta cumplirlo sobre defectos que no se usaron para ajustar nada (P73 d). Ningún dato de hoy dice que se cumpla.

## Mapa de tareas y defectos (decidido: solo medir)

**Origen.** El usuario respondió con un video: "Esto. Algo asi." El orquestador vio 24 cuadros, sin audio. El video arma un mapa de significado de juegos, y mide la cercanía de cada idea. P76: "Por ahora solo medir. Despues evaluamos si genera".

**Qué mide el mapa.**
- **Cercanía.** Cada tarea tiene su distancia a lo público. Hipótesis: una tarea lejana está menos memorizada por el modelo.
- **Cobertura de defectos.** Un mapa de defectos reales e inyectados muestra si lo inyectado representa lo real. En el hito 1, los inyectados dieron 1,0 en muchas dimensiones, y el recall real dio 0,125.
- **Puntaje según cercanía, propuesta.** Con 20 tareas solo muestra una tendencia. No prueba memoria.

**Regla de Root.** La distancia es un cálculo, no un juicio. Lejos no significa bueno ni factible.

**Tamaño medido.** `gh api search/repositories`, 2026-11-05, Rust con licencia MIT o Apache-2.0: 42 127 repos con 10 estrellas o más, 9676 con 100 o más, y 1717 con 1000 o más. No verifiqué cómo une GitHub las dos licencias. Alcanza para un mapa de repos. No medí cuántos commits sirven como tareas.

**Embeddings, propuesta técnica.**

| Opción | Dato | Fuente | Dónde corre | Costo |
|---|---|---|---|---|
| `jina-embeddings-v2-base-code` | 322 MB, 768 dimensiones, Apache-2.0. Su README lista Rust. Su página lo marca como obsoleto. Pide `trust_remote_code` | Hugging Face, API y README, 2026-11-06 | Contenedor sin red ni claves, con CPU | 0 USD de API; tiempo sin medir |
| `nomic-ai/CodeRankEmbed` | 547 MB, MIT. Control de S9. No leí su lista de lenguajes | Hugging Face, API | Igual | Igual |
| **Descartados** | `jina-code-embeddings-0.5b` y `1.5b`: CC-BY-NC-4.0, y su README no nombra Rust. `nomic-embed-code`: su README no lista Rust, y pesa decenas de GB | Hugging Face | | |
| `voyage-code-3` | Cerca de 0,18 USD por millón de tokens; API cerrada | Páginas de terceros, sin verificar | API externa | Estimación: 10 000 textos de 1000 tokens, cerca de 1,8 USD |

- Usamos un modelo abierto, en la laptop, solo si S9 prueba que sirve en Rust. D1 mide el tiempo con una muestra de 100 textos.
- La decisión 33 dice que la cuenta de Colab es paga, y acepta la T4 solo para tests pendientes. Usarla aquí es otra pregunta.
- Se embebe la descripción del repo, el mensaje del commit y un resumen del diff. Los tests ocultos no entran.

**Generar tareas: se evalúa después del piloto.** No entra en ninguna fase ni en los 50 USD. Si se aprueba, una tarea generada entra solo con oráculo: solución de referencia en verde, tests ocultos y un defecto que los pone en rojo.

## Saboteador y curva de auditoría (decididos: P75 a)

- **Saboteador.** Un modelo escribe un defecto plausible en código con tests. Un test lo pone en rojo, y ese test es la etiqueta. Un defecto que ningún test pone en rojo no entra. **El test de etiqueta no entra al `oculto` de esa evaluación.** Si entra, Root encuentra el defecto por construcción.
- **Curva de auditoría.** Recall según lo que Root lee: 6000 caracteres por archivo, más, y todo. Es mi lectura del nombre, porque no vi el tuit. Decide con datos cuánto debe leer el juez.
- **El mapa no los reemplaza.** El mapa dice dónde falta muestra. El saboteador da casos con etiqueta. La curva mide cuánto cuesta encontrarlos.
- **Separación.** Los defectos que ajustan el juez no miden el recall final. Eso exige dos conjuntos.
- **Manifiesto.** El dataset guarda la semilla y la probabilidad de selección de cada tarea. Las tareas que alimentan a `aprende` no puntúan la vara. Las tareas ancla no rotan.
- **Tamaño del conjunto final: 30 defectos o más.** Con 30, 24 aciertos dan 0,8 y una cota de Wilson de 0,69. Con 8, el recall no distingue casi nada.
- **Recall por test en tareas minadas: no vale como prueba.** Si el test del commit está en `oculto`, el estado anterior siempre da rojo. El recall de D4 usa línea, sonda o juez. `recall_test` se informa aparte.

## Prueba de sustento (P72)

**Regla.** Cada umbral queda fijo antes de ver el resultado. Ningún resultado recorta una fase: "cambia" suma costo, tiempo o método. S3b se fijó antes de leer la pasada, y ya tiene resultado: **frena** (ver abajo).

### Ya probado leyendo, sin API

| N.º | Afirmación | Resultado | Lectura |
|---|---|---|---|
| S4 | Las citas dicen lo que el PRD dice | **Sostiene** | SWE-Bench+, SWE-Milestone y el ruido de pass@1 coinciden con sus abstracts. SWE-Bench+ dice "issue report or the comments". |
| S5 | Una tarea de Rust cumple el contrato | **Sostiene, con código** | Nextest emite JUnit, y Root soporta suites que no son pytest. Faltan la imagen y `borrador_tarea`. |
| S6 | Se puede filtrar por mutación del commit | **Sostiene, sin probar la versión** | La documentación de `cargo-mutants` describe `--in-diff`. La imagen trae 27.1.0, y no vi su `--help`. |
| S7 | Una calibración sirve para tareas nuevas | **Cambia el PRD** | Hoy las filas son por tarea, y dos defectos piden datos que un commit no trae. Es código de D0 y D3. |
| S8 | El recall es medible | **Cambia el PRD** | Un test del commit en `oculto` da recall 1 por construcción, y 8 defectos no alcanzan. Ahora: test de etiqueta aparte y 30 defectos o más. |
| S10 | Los 50 USD alcanzan | **No alcanzan por poco, por estimación** | Cerca de 52 USD con el costo medido de la pasada. Los brazos de builder por API no entran. S2 lo mide. |
| S11 | Las fichas salen sin el usuario | **Choca con la decisión 12** | La decisión 12 dice: "el usuario aprueba los requisitos; la spec sale de una sesión `/grill-me`". C6 usa un modelo. Va a P81. |
| S12 | Los benchmarks externos se reusan | **Sin sostener** | Sus abstracts no nombran Rust. Falta lectura. |

### Propuesto, con costo

| N.º | Afirmación | Cómo se prueba | Sostiene | Cambia | Frena | Costo | Tiempo, estimación |
|---|---|---|---|---|---|---|---|
| S1 | Hay tareas suficientes de Rust | Minar 50 commits de 10 repos elegidos con semilla, con los criterios 1 a 5. **Medido: 3 de 50, 6 %, y da "cambia". Ampliado: 16 admitidos de 109 commits, y da "frena" (P94).** Regla nueva: 40 o más admitidos, en 5 repos o más, sostiene | 10 % o más admitidos, de 3 repos o más | De 2 % a 10 %: más repos o minería más larga | Menos de 2 % | 0 USD; GitHub, P79 | 1 a 2 días de CPU |
| S2 | El costo entra en 50 USD | 3 tareas de punta a punta: 15 evaluaciones y 6 defectos del saboteador | Proyección de Root de 50 USD o menos | De 50 a 75 USD: el usuario amplía | Más de 75 USD | Hasta 5 USD de los 50, P80, después del cierre | 1 día |
| S3a | El tiempo es viable | Lo mide S2 | 60 minutos o menos por evaluación, y 7 días o menos en total | Hasta 14 días: se elige por tamaño de repo | Más de 14 días | Incluido en S2 | Incluido |
| S3b | El recall de 0,8 es alcanzable | La pasada sobre los 10 defectos frescos | 7 o más de 10 | 4 a 6: el juez y la pasada se amplían antes de D4 | 3 o menos: el usuario decide | **Medido: 0 de 10, 0,4665 USD. Frena** | Hecho |
| S9 (medido: 66 de 100, sostiene) | El mapa sirve en Rust | 100 crates de categorías conocidas: ¿el vecino más cercano comparte categoría? | 60 % o más | 30 % a 60 %: otro modelo | Menos de 30 % en dos modelos: el mapa no mide | 0 USD; bajar un modelo, P79 | Horas |
| S13 | El saboteador da casos válidos | 10 intentos en las 3 tareas de S2 | 5 o más compilan, pasan limpios y su test da rojo | 2 a 4: mejorar el prompt | 1 o menos | Incluido en S2 | Incluido |

Los umbrales de S1, S3a, S9 y S13 son criterio mío, no una medición. No tienen otra base.

### Resultado de S3b: frena

**Medido.** La pasada leyó los 8 archivos, sin corte, y encontró 0 de 10. Con 0 de 10, la cota superior de Wilson al 90 % es 0,14. De sus 6 hallazgos, 4 son reales según mi lectura, 1 dudoso y 1 falso. Dos reales no estaban en la revisión de cierre.

**Lo que significa.** Esta pasada no sirve como camino al recall de 0,8. Hoy ninguna medición apoya esa afirmación del PRD. La pasada sí halla defectos de otra clase.

**Lo que no significa.** No prueba que el dataset sea inútil: es el lugar donde se puede medir otro buscador sobre defectos nuevos. Tampoco prueba que el 0,8 sea imposible.

**Este conjunto quedó gastado.** Los 10 ya no miden un buscador ajustado por quien vio este resultado. El umbral de S3b no cambió.

**Decisión del usuario.** P83: "P83 a + b en paralelo". El dataset sigue, y la Línea Buscador corre a la vez. La regla sigue: Root no evalúa repos reales sin recall de 0,8. Nadie promete llegar.

### Criterio de salida

- **Implementar el PRD entero tras el cierre:** S1, S2, S3a, S9 y S13 sostienen, P79 a P82 y P84 tienen respuesta. S3b ya no frena el PRD: pasa a la Línea Buscador.
- **Cambiar el PRD y seguir:** alguna da "cambia". El planner escribe el cambio, con costo, y el usuario lo ve. Ninguna fase sale.
- **Frenar y preguntar:** alguna da "frena". El usuario decide. S9 solo frena el mapa, y el usuario decide si eso cambia su pedido.

## Línea Buscador (P83)

**Objetivo.** Hallar un buscador que cumpla el recall de 0,8 sobre defectos que nadie usó para ajustar. El detalle está en el plan, en "Línea Buscador". No hay evidencia de que exista a costo razonable.

**Conjuntos.**
- **Ajuste:** 20 defectos o más. Se permite mirarlos.
- **Final, sellado:** 30 sembrados y 15 fallas reales, o más. Solo el orquestador los ve. Cada variante elegida se mide una sola vez.
- **Gastados:** los 10 frescos y los 8 de la validez no entran en ninguno.
- **Partición.** Fallas reales: ajuste y final por repo. Un repo puede dar tareas al dataset y fallas al buscador, con commits distintos. Sembrados: por archivo, porque salen de 2 repos. La regla exacta está en el plan.

**Reporte.** Recall por línea, por clase y por origen (real o sembrado), además de candidatos por 1000 líneas. El recall sembrado solo no basta: un saboteador y un detector LLM pueden compartir puntos ciegos. Es un argumento, sin medir.

**Primera medición, sobre 24 sembrados de ajuste.** Clippy: 0,833, con 530,8 candidatos por 1000 líneas, y no sirve. Sonnet por archivo: 0,708, con 1,3. Sonnet con 4 focos: 0,833, con 11,3. En 18 defectos reales ya gastados, solo informativo, los 4 focos dieron 7 de 18. Los sembrados pueden sobrestimar el recall real. Por eso deciden los dos umbrales, y antes del final se exigen 3 o más de 5 reales de ajuste.

**Cuenta el oráculo.** Un defecto real cuenta si un hallazgo cae a 3 líneas o menos de una línea que cambió el arreglo. Un sembrado cuenta igual sobre la línea alterada. Un LLM nunca decide el acierto.

## Costo

**Cifras medidas:** una evaluación con `deepseek-v4-pro` costó de 0,097 a 0,107 USD. Una evaluación de memora tardó cerca de 24 minutos, según Graft. Un mutante de memora tardó cerca de 4,5 s: 2 s de build y 2,5 s de test, en medianas.

**Estimación sin medir:**

| Parte | USD de API |
|---|---|
| Fichas de 20 tareas | Cerca de 1 |
| Calibración, 100 evaluaciones a 0,19 USD con la lectura ampliada | Cerca de 19 |
| Curva: 40 defectos en 3 lecturas | Cerca de 23 |
| Conjunto final: 30 defectos | Cerca de 5,7 |
| Saboteador | 0 USD de API: va por cupo |
| Línea Buscador, variantes por API | **Tope 12** (P84). Estimación: 9,2, con V0, V1 y Vd |
| S2, 3 tareas | **Tope 5** (P80) |
| **Subtotal de Root** | **Cerca de 65,7: 17 son topes y 48,7 son estimación** |
| Línea Buscador por cupo y sondas | 0 USD de API |
| Corridas de builder por API | No entran en este subtotal. Sin medir. Una sola corrida de memora midió 2,76 USD de planificación, y se estimó en cerca de 18 USD por 2 horas de organización |

**Lectura.** El subtotal pasa de 50 USD por cerca de 16, por estimación. No recorto fases. Los brazos de builder van por cupo (P82). S2 mide la parte estimada. El usuario dijo: "si hace falta mas agregaremos". S2 lo mide antes de pedir. Los brazos de builder por API no entran: dependen de P82. Los 0,19 USD salen de suponer 52 000 tokens de entrada y 30 000 de salida. La pasada midió de 20 000 a 30 000 de salida, y yo había estimado 16 000: costó 0,4665 USD contra 0,25 estimados.

**Regla.** El piloto corre primero 3 tareas, mide y frena. Después el planner proyecta el total. Si no entra en 50 USD, va al usuario. Toda corrida lleva `--tope-usd`.

**Ruido.** Una corrida sola varía de 2,2 a 6,0 puntos de pass@1 (arXiv 2602.07150). Por eso cada brazo necesita repeticiones. Su número sale de las 3 tareas.

## Fases

| Fase | Entrega | API | Salida |
|---|---|---|---|
| D0 | Prerrequisitos | Baja | Tests verdes con podman |
| D1 | Minería, chequeos de admisión y mapa | 0 | 30 candidatos con criterios 1 a 5, y su cercanía |
| D2 | 3 tareas de punta a punta, con predicción registrada como intervalo antes de cada corrida | Se mide | Costo y tiempo reales, y error de la predicción |
| D3 | 20 tareas estratificadas también por cercanía, partición y C1 a C6 | Según D2 | Chequeos C1 a C6: verde en fixture bueno, rojo en fixture malo |
| D4 | Calibración por versión, saboteador y curva | Cerca de 10 USD, más lo que mida D2 | Filas por dimensión y recall sobre un conjunto separado |
| D5 | Vara v1 y `oraculo carencias` | Baja | Graft la consume |
| D6 | Borrado de memora y brasa | 0 | Decisión 70, con orden del usuario |

## Métricas de éxito

- Cada tarea admitida pasa los 5 criterios, y un fixture defectuoso cae en cada uno.
- Una segunda corrida del minero da la misma lista de tareas.
- La partición oculta nunca aparece en una salida visible. Un test lo prueba.
- Al menos 5 dimensiones quedan confiables, igual que hoy en memora y brasa.
- El gasto queda dentro de 50 USD, o el usuario lo amplió.

## Riesgos

- **Pocos commits de Rust cumplen todo.** La minería lo mide en D1, sin API.
- **Una clave en el contenedor del builder.** El dataset corre tests ajenos. Ver P70.
- **Un sesgo por repos con muchos tests.** Se estratifica por tamaño y módulos.

## Lo que espera al usuario

1. **Aprobar este PRD**, con su reparto: Root administra dataset, vara y oráculo oculto. Graft consume y mide. El usuario no aprobó ese reparto todavía.
2. **Lista de fuentes de GitHub.** Es red fuera del repo: zona roja del GOAL. El planner trae la lista después de D1, con licencias.
3. **P70:** cómo entra la clave al builder.
4. **P71:** si "Usar api claude" incluye al juez.
5. **Reusar benchmarks.** Los abstracts de FeatureBench, SWE-Milestone y Commit0 no nombran Rust. Falta leer sus artículos o repos. Es lectura pública, sin costo.
6. **Respondidas:** P79 con la lista a la vista, P80 hasta 5 USD, P81 con las 3 primeras fichas y el método, P82 por cupo, P84 hasta 12 USD. El método está en `docs/dataset-metodo-fichas.md`.
7. **La clave dedicada (P70 a):** el usuario la crea al empezar D2, si algo va por API.
