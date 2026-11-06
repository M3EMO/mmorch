# PRD: Dataset fiable para Root y Graft

**Estado:** borrador de Opus (rol planner de Root), 2026-11-05. El usuario no lo aprobó.
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

## Costo

**Cifras medidas:** una evaluación con `deepseek-v4-pro` costó de 0,097 a 0,107 USD. Una evaluación de memora tardó cerca de 24 minutos, según Graft. Un mutante de memora tardó cerca de 4,5 s: 2 s de build y 2,5 s de test, en medianas.

**Estimación sin medir:**

| Parte | USD de API |
|---|---|
| Fichas de 20 tareas | Sin medir |
| 20 evaluaciones de línea base | Cerca de 2 |
| Calibración, 100 evaluaciones | Cerca de 10 |
| Corridas de builder por API | Sin medir: es la parte que puede romper el tope |

**Regla.** El piloto corre primero 3 tareas, mide y frena. Después el planner proyecta el total. Si no entra en 50 USD, va al usuario. Toda corrida lleva `--tope-usd`.

**Ruido.** Una corrida sola varía de 2,2 a 6,0 puntos de pass@1 (arXiv 2602.07150). Por eso cada brazo necesita repeticiones. Su número sale de las 3 tareas.

## Fases

| Fase | Entrega | API | Salida |
|---|---|---|---|
| D0 | Prerrequisitos | Baja | Tests verdes con podman |
| D1 | Minería de candidatos y chequeos de admisión | 0 | 30 candidatos con criterios 1 a 5 |
| D2 | 3 tareas de punta a punta | Se mide | Costo y tiempo reales |
| D3 | 20 tareas, partición y C1 a C6 | Según D2 | Chequeos C1 a C6: verde en fixture bueno, rojo en fixture malo |
| D4 | Calibración por versión | Cerca de 10 USD | Filas por dimensión |
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
5. **"usa ECC-".** El mensaje termina cortado. La lectura de Graft es que el dataset se construye con ECC. No está confirmada.
6. **Reusar benchmarks.** FeatureBench, SWE-Milestone y Commit0 se propusieron. No verifiqué si traen tareas de Rust. Se revisa en D1.
