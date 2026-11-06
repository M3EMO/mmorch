# PRD: Dataset fiable para Root y Graft

**Estado:** borrador de Opus (rol planner de Root), 2026-11-06. El usuario no lo aprobó entero (P72). P73 d, P75 y P76 ya están decididas.
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
| ECC | Graft propuso "ECC construye las herramientas del dataset y también corre como brazo base", y el usuario respondió "Sí, las cuatro" | Usuario, citado por Graft, sin comprobar |
| Saboteador y curva | P75 a: los dos entran | Usuario |
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
- **Falta decidir, técnico:** `cargo test` no da junit. Root usa `cargo nextest` o un conversor, en una imagen nueva. Hoy las tareas usan pytest.
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
- El 10-25 % se mide "Sobre la distancia al techo del puntaje de rúbrica", según Graft.
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
- **Cuándo es confiable una dimensión.** Cumple la regla de hoy (`Calibraciones::aplicar`): exactitud de 0,9 o más, al menos 5 defectos y 5 limpios, fila completa y como mucho 20 % de limpios sin veredicto. Además, la cota inferior de Wilson de su sensibilidad supera 0,7. Es una propuesta: con 20 defectos, 18 aciertos dan cerca de 0,78.

## Prerrequisitos en Root

Sin esto, el piloto mediría mal. Todos están en el plan de Root.

1. El juez lee más allá de los primeros 6000 caracteres de cada archivo. Diseño detectó por cita 4 de 4 dentro del corte y 0 de 6 fuera.
2. Se revisa `sin_evidencia`. En memora, 3 de 10 limpios no dieron veredicto.
3. El tope cuenta el reintento y los casos en vuelo (hallazgo 5 de la revisión de cierre).
4. Una falla del modelo no se reusa en un limpio (hallazgo 18 y arreglo de la Tarea 67).
5. `Fila` guarda la huella de prompt, rúbricas y regla (hallazgo 19, sin verificar).
6. `r00t corre` lanza por API (R6), y `track comparar` conoce la vía (R12).
7. **Recall sobre defectos reales.** El hito 1 midió 0,125, con umbral 0,8. Root no evalúa un repo real hasta cumplirlo sobre defectos que no se usaron para ajustar nada (P73 d).

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
| `jina-embeddings-v2-base-code` o su sucesor | 768 dimensiones, 8192 tokens, Apache 2.0 | Búsqueda web, sin leer el modelo | Laptop con CPU | 0 USD de API; tiempo sin medir |
| `voyage-code-3` | Cerca de 0,18 USD por millón de tokens; API cerrada | Páginas de terceros, sin verificar | API externa | Estimación: 10 000 textos de 1000 tokens, cerca de 1,8 USD |

- Propongo el modelo abierto, en la laptop. D1 mide el tiempo con una muestra de 100 textos.
- La decisión 33 dice que la cuenta de Colab es paga, y acepta la T4 solo para tests pendientes. Usarla aquí es otra pregunta.
- Se embebe la descripción del repo, el mensaje del commit y un resumen del diff. Los tests ocultos no entran.

**Generar tareas: se evalúa después del piloto.** No entra en ninguna fase ni en los 50 USD. Si se aprueba, una tarea generada entra solo con oráculo: solución de referencia en verde, tests ocultos y un defecto que los pone en rojo.

## Saboteador y curva de auditoría (decididos: P75 a)

- **Saboteador.** Un modelo escribe un defecto plausible en código con tests. Un test oculto lo pone en rojo, y ese test es la etiqueta. Un defecto que ningún test pone en rojo no entra. Ataca lo medido: hoy los inyectores cubren pocas clases.
- **Curva de auditoría.** Recall según lo que Root lee: 6000 caracteres por archivo, más, y todo. Es mi lectura del nombre, porque no vi el tuit. Decide con datos cuánto debe leer el juez.
- **El mapa no los reemplaza.** El mapa dice dónde falta muestra. El saboteador da casos con etiqueta. La curva mide cuánto cuesta encontrarlos.
- **Separación.** Los defectos que ajustan el juez no miden el recall final. Eso exige dos conjuntos.

## Costo

**Cifras medidas:** una evaluación con `deepseek-v4-pro` costó de 0,097 a 0,107 USD. Una evaluación de memora tardó cerca de 24 minutos, según Graft. Un mutante de memora tardó cerca de 4,5 s: 2 s de build y 2,5 s de test, en medianas.

**Estimación sin medir:**

| Parte | USD de API |
|---|---|
| Fichas de 20 tareas | Sin medir |
| 20 evaluaciones de línea base | Cerca de 2 |
| Calibración, 100 evaluaciones | Cerca de 10 |
| Corridas de builder por API | Sin medir. Se achica si los brazos base van por cupo (U4) |
| Saboteador y curva | Sin medir: se mide en D2 |

**Regla.** El piloto corre primero 3 tareas, mide y frena. Después el planner proyecta el total. Si no entra en 50 USD, va al usuario. Toda corrida lleva `--tope-usd`.

**Ruido.** Una corrida sola varía de 2,2 a 6,0 puntos de pass@1 (arXiv 2602.07150). Por eso cada brazo necesita repeticiones. Su número sale de las 3 tareas.

## Fases

| Fase | Entrega | API | Salida |
|---|---|---|---|
| D0 | Prerrequisitos | Baja | Tests verdes con podman |
| D1 | Minería, chequeos de admisión y mapa | 0 | 30 candidatos con criterios 1 a 5, y su cercanía |
| D2 | 3 tareas de punta a punta | Se mide | Costo y tiempo reales |
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
5. **Reusar benchmarks.** FeatureBench, SWE-Milestone y Commit0 se propusieron. No verifiqué si traen tareas de Rust. Se revisa en D1.
