---
title: Vigia - UX y presentacion de datos para un panel de confianza por token
mision: Que patrones de UX, graficos, accesibilidad, arquitectura de informacion y stack minimo necesita el panel web de Vigia para mostrar tokens de confianza baja y operar la revision humana.
status: seed
confidence:
verifier:
tags: [research, vigia]
sources:
  - https://arize.com/docs/phoenix/tracing/concepts-tracing/annotations-concepts
  - https://arize.com/docs/phoenix/tracing/how-to-tracing/filter-expressions
  - https://arize.com/docs/phoenix/release-notes/08-2026/08-17-2026-trace-filters-and-analytics-sql
  - https://github.com/Arize-ai/phoenix
  - https://langfuse.com/docs/scores/annotation
  - https://langfuse.com/docs/evaluation/evaluation-methods/annotation-queues
  - https://langfuse.com/docs/metrics/features/custom-dashboards
  - https://langfuse.com/docs/observability/overview
  - https://github.com/langfuse/langfuse
  - https://docs.langchain.com/langsmith/annotation-queues
  - https://docs.galileo.ai/galileo/gen-ai-studio-products/galileo-llm-fine-tune/using-uncertainty
  - https://help.cleanlab.ai/tlm/tutorials/tlm_advanced/
  - https://docs.patronus.ai/docs/percival/concepts
  - https://patronus.ai/products
  - https://wandb.ai/wandb_fc/product-announcements-fc/reports/Humanloop-is-Sunsetting-Migrate-to-Weights-Biases-as-an-alternative--VmlldzoxMzk4ODc1Nw
  - https://agenta.ai/blog/humanloop-sunsetting-migration-and-alternative
  - https://docs.coreweave.com/weave/guides/tracking/tracing
  - https://arxiv.org/abs/2302.07248
  - https://arxiv.org/abs/2605.28571
  - https://arxiv.org/html/2605.28571
  - https://pmc.ncbi.nlm.nih.gov/articles/PMC4349800
  - https://scikit-learn.org/stable/modules/calibration.html
  - https://matplotlib.org/stable/users/explain/colors/colormaps.html
  - https://pmc.ncbi.nlm.nih.gov/articles/PMC6070163
  - https://conceptviz.app/blog/okabe-ito-palette-hex-codes-complete-reference
  - https://www.w3.org/TR/WCAG22/
  - https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html
  - https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html
  - https://www.w3.org/WAI/ARIA/apg/patterns/grid/
  - https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/mark
  - https://data.europa.eu/apps/data-visualisation-guide/accessible-svg-and-aria
  - https://flaviocopes.com/courses/htmx/focus-after-a-swap/
  - https://htmx.org/
  - https://htmx.org/docs/
  - https://fastapi.tiangolo.com/advanced/templates/
  - https://pypi.org/project/pygal/
created: 2026-10-04
---

# Vigia: UX y presentacion de datos del panel de confianza por token

## Tronco

El panel de Vigia debe mostrar un puntaje calibrado P(error) por token con codificacion triple (fondo, subrayado y texto oculto), una cola de revision manejable por teclado y cinco graficos SVG server-side, todo con FastAPI + Jinja2 + htmx sin paso de build.

## Hallazgos

### 1. Mapa de herramientas del mercado

Ninguna herramienta revisada documenta un heatmap por token basado en senales internas de un MoE. Todas trabajan a nivel de traza, span o respuesta. Galileo es la unica con resaltado por token documentado, y usa incertidumbre de logprobs.

| Herramienta | Unidad que muestra | Granularidad del puntaje | Cola de revision | Anotaciones | Filtros | Licencia / estado |
|---|---|---|---|---|---|---|
| Arize Phoenix | Traza y span (OpenTelemetry) | Span / documento | Hotkeys para anotacion manual | Configs categorical, continuous, freeform; direccion maximize/minimize/none; annotator HUMAN, LLM, CODE; campos label, score, explanation | Expresiones booleanas Python (`span_kind == 'LLM'`); filtros de traza (`num_spans > 10 and error_count > 0`) desde 20.2-20.3 (ago 2026) | Elastic License 2.0 |
| Langfuse | Traza con observaciones anidadas, sesiones | Traza / observacion / sesion (scores) | Annotation Queues con "Complete + next" | Score configs reutilizables; tab "Scores" en el detalle | Metadata, tags, rango de score, entorno, modelo, usuario | MIT (carpeta `ee` aparte); Next.js + Postgres + ClickHouse |
| LangSmith | Run y thread | Run / thread | Single-run y pairwise (A/B) | Rubrica configurable en panel derecho; edicion de salida para crear ejemplo de referencia | [no verificado en esta nota] | Comercial |
| Galileo | Respuesta generada | Token (incertidumbre) | [no verificado] | [no verificado] | [no verificado] | Comercial; la pagina de docs citada devuelve 404 hoy |
| Patronus (Percival, Lynx) | Traza de agente | Traza / span; mas de 20 modos de falla | [no verificado] | "Generate Insights" en 3 niveles: multi-traza, span, atributo | [no verificado] | Comercial |
| Cleanlab TLM | Respuesta o campo estructurado | Respuesta (0 a 1) y por campo, con explicacion textual | No aplica | Explicaciones automaticas | No aplica | Comercial |
| Humanloop | - | - | - | - | - | Cerrado el 2025-09-08 tras la adquisicion por Anthropic |
| W&B Weave | Ops, Calls, Traces, Threads | Call | [no verificado] | [no verificado] | [no verificado] | Comercial |

Detalles relevantes por herramienta:

- **Phoenix** compila el filtro a una consulta de base de datos. El mismo texto sirve en la UI y en el SDK (`.where()`). Los literales de `span_kind` y `status_code` pasan a mayusculas solos.
- **Phoenix 20.2-20.3** muestra score, label, explanation y autor al pasar el mouse sobre un token de anotacion. Tambien permite filtrar desde ese popover.
- **Langfuse** organiza la cola con teclado completo. Flechas izquierda/derecha cambian de item. Flechas arriba/abajo cambian de campo. Las teclas `1` a `9` eligen opciones categoricas. `Cmd/Ctrl+Enter` completa y avanza. `?` abre la hoja de atajos.
- **Langfuse** agrega items a la cola por seleccion masiva ("Actions" -> "Add to queue") o desde un item individual.
- **Langfuse** ofrece dashboards con graficos de linea, barra, serie temporal y torta. La pagina no enumera percentiles disponibles [no verificado p50/p95/p99].
- **LangSmith** define estados de item: Needs Review, Needs Others' Review y Completed. Permite reservas (bloqueo temporal por revisor) y un numero minimo de revisores por run. Un revisor no ve la evaluacion de otro.
- **LangSmith pairwise** usa las teclas `A`, `B`, `E` y `Enter` para avanzar.
- **Galileo** describe incertidumbre por token: valor bajo implica confianza alta del modelo. La doc afirma correlacion con nombres, citas y URLs inventadas. La doc tambien aclara que no todo token incierto es alucinacion. Fuente leida via resultado de busqueda; la pagina hoy devuelve 404 [no verificado el detalle visual].
- **Cleanlab TLM** devuelve razones tipificadas: respuesta alternativa contradictoria casi generada, error factual o de razonamiento, tarea ambigua, prompt atipico.
- **Patronus Lynx** es un Llama 3 ajustado con 2400 ejemplos de alucinacion. Es un juez, no una senal interna.

Conclusion de mercado: el patron dominante es lista de trazas -> detalle -> anotacion -> cola. El resaltado por token es raro. Vigia puede diferenciarse con el detalle por token y con la vista capa x experto.

### 2. Evidencia HCI sobre resaltado por token

| Estudio | Diseno | Hallazgo | Implicancia |
|---|---|---|---|
| Vasconcelos et al., "Generation Probabilities Are Not Enough", ToCHI ago 2024 (arXiv 2302.07248) | 30 programadores, 3 condiciones: sin resaltado, resaltado por probabilidad baja de generacion, resaltado por probabilidad alta de edicion predicha | El resaltado por probabilidad de generacion no mejora sobre no resaltar. El resaltado por probabilidad de edicion acelera la tarea y focaliza las ediciones | Mostrar un puntaje calibrado contra la etiqueta de error, nunca la entropia cruda |
| "Not All Uncertainty Is Equal" (arXiv 2605.28571) | 192 participantes, entre sujetos; niveles salida, relacion y token | La incertidumbre por token aumenta el acuerdo con la IA. Los niveles salida y relacion bajan la confianza del usuario en su propia respuesta. El nivel relacion reduce la verificacion externa | El resaltado puede generar exceso de confianza en el texto no resaltado |

La interfaz del estudio 2605.28571 usa fondo rojo con intensidad proporcional. El usuario ve el valor exacto al pasar el mouse. Un umbral ajustable limita el resaltado a tokens sobre el umbral. El estudio agrega subtokens en palabras promediando logprobs.

Ambos estudios usan logprobs, no senales del router. Ninguno usa texto en espanol.

### 3. Graficos canonicos

| Grafico | Pregunta que responde | Construccion recomendada | Error comun |
|---|---|---|---|
| Curva de confiabilidad (reliability diagram) | El puntaje 0.8 significa 80% de error real? | Eje X: puntaje medio por bin. Eje Y: fraccion de positivos. Diagonal y=x. Bins uniformes o por cuantiles. Histograma de conteos debajo | Usar solo Brier: sklearn advierte que Brier mezcla calibracion y discriminacion |
| ROC | El detector separa tokens erroneos de correctos? | Curva TPR/FPR, AUROC en la leyenda, diagonal de azar | Leerla sola con clases desbalanceadas |
| Precision-Recall | Cuantos flags son errores reales al umbral elegido? | Curva P/R, linea base igual a la prevalencia, punto del umbral actual marcado | Omitir la linea base de prevalencia |
| Histograma de puntajes | Donde cae el umbral respecto de cada clase? | Dos distribuciones superpuestas (correcto vs erroneo segun juez), linea vertical del umbral | Un solo histograma sin separar clases |
| Latencia p50/p95/p99 | Vigia cumple la meta de menos de 5% de latencia agregada? | Barras agrupadas baseline vs con Vigia por percentil; linea de meta en +5% | Mostrar solo el promedio |
| Tasa de flags en el tiempo | El modelo o los datos derivaron? | Linea diaria de tokens marcados / tokens totales; marcas verticales en cada recalibracion | Mezclar versiones del calibrador sin marcarlas |
| Distribucion por capa/experto | Que capas o expertos concentran senal o flags? | Heatmap capa (filas) x experto (columnas), escala secuencial | Escala arcoiris o rojo-verde |

Saito y Rehmsmeier (PLoS ONE 2015) muestran que ROC puede inducir conclusiones enganosas con datos desbalanceados. La curva PR evalua la fraccion de verdaderos positivos entre los positivos predichos. Los errores por token son minoria, entonces PR es obligatoria junto a ROC.

### 4. Accesibilidad (WCAG 2.2 AA)

Criterios que aplican directo al panel:

| Criterio | Requisito | Aplicacion en Vigia |
|---|---|---|
| 1.4.1 Use of Color (A) | El color no es el unico medio visual de informacion | Cada token marcado lleva subrayado, nivel textual y glifo ademas del fondo |
| 1.4.3 Contrast Minimum (AA) | Texto 4.5:1 [criterio estandar, no releido hoy] | El texto sobre fondo resaltado mantiene 4.5:1 en todos los niveles |
| 1.4.11 Non-text Contrast (AA) | Componentes y graficos 3:1 [criterio estandar, no releido hoy] | Lineas de graficos, bordes de foco y marcadores con 3:1 |
| 2.4.11 Focus Not Obscured (AA, nuevo en 2.2) | El foco no queda totalmente oculto | Barras fijas y popovers no tapan el token enfocado |
| 2.5.7 Dragging Movements (AA, nuevo) | Toda accion de arrastre tiene alternativa de un puntero | El umbral usa input numerico ademas de slider |
| 2.5.8 Target Size Minimum (AA, nuevo) | Objetivo de 24x24 CSS px, con excepcion por espaciado e inline | Botones de accion 24px o mas; tokens inline quedan exentos |
| 3.3.8 Accessible Authentication (AA, nuevo) | Sin prueba cognitiva para autenticarse | Login con password manager habilitado, sin CAPTCHA cognitivo |
| 4.1.1 Parsing | Eliminado en 2.2 | No aplica |

Hallazgos de tecnica:

- **`<mark>`** no se anuncia en la mayoria de lectores de pantalla. MDN sugiere `::before` y `::after` con texto oculto visualmente. MDN advierte no abusar, porque algunos usuarios desactivan la verbosidad extra.
- **SVG accesible**: `role="img"`, `<title>` y `<desc>` con id y `aria-labelledby`. `<title>` solo no se expone de forma fiable. La tabla de datos alternativa va dentro de `<details>`.
- **Grid ARIA**: una grilla tiene una sola parada de tab y navega con flechas (roving tabindex). Una tabla estatica pone todos los elementos en la secuencia de tab. El APG recomienda grid solo para contenido interactivo.
- **htmx no gestiona accesibilidad solo**. Un swap puede borrar el elemento enfocado. htmx conserva el foco en inputs con id estable. El resto requiere `aria-live`, `aria-busy` y mover el foco en `htmx:afterSwap`.
- **Paleta categorica**: Okabe-Ito tiene 8 colores distinguibles con las deficiencias comunes: `#000000`, `#E69F00`, `#56B4E9`, `#009E73`, `#F0E442`, `#0072B2`, `#D55E00`, `#CC79A7`.
- **Escala secuencial**: matplotlib clasifica viridis y cividis como perceptualmente uniformes con L* monotono. Recomienda evitar rojo y verde juntos. Cividis fue optimizada para vision normal y deficiencia rojo-verde (Nunez et al., PLOS ONE 2018). La deficiencia de color afecta a mas de 4% de la poblacion segun ese paper.

### 5. Arquitectura de informacion recomendada

La estructura sigue el patron del mercado y agrega dos vistas propias (token y capa/experto).

| Pagina | Ruta sugerida | Contenido | Rol minimo |
|---|---|---|---|
| Resumen | `/` | KPIs: tasa de flags 24h, AUROC vigente, overhead p95, items pendientes en cola; sparkline de tasa de flags | viewer |
| Generaciones | `/generations` | Tabla paginada: fecha, modelo, max puntaje, cantidad de flags, accion aplicada, estado de revision; filtros en querystring | viewer |
| Detalle de generacion | `/generations/{id}` | Prompt, respuesta con tokens codificados, lista lateral de fragmentos dudosos, panel de senales (entropia, dispersion por capa), accion por fragmento | viewer |
| Cola de revision | `/review` y `/review/{item}` | Un fragmento por pantalla, contexto, decision (correcto, error, incierto), comentario, "Completar y siguiente" | reviewer |
| Calibracion | `/calibration` | Version del calibrador, tamano del set del juez, ROC, PR, confiabilidad, histograma, umbral elegido | admin para cambiar, viewer para leer |
| Rendimiento | `/performance` | Latencia p50/p95/p99 baseline vs Vigia, tokens/s, tendencia | viewer |
| Modelo | `/model` | Heatmap capa x experto: entropia media, tasa de flags por experto | viewer |
| Politicas | `/policies` | Regla por fragmento dudoso: revision humana, re-consulta RAG, abstencion; umbral por politica | admin |
| Configuracion | `/settings` | Usuarios, roles, API keys, retencion, redaccion de PII | admin |
| Auditoria | `/audit` | Registro de cambios de umbral, politica y calibrador | admin |

La navegacion principal es una barra lateral con estas paginas. En pantallas angostas pasa a un menu colapsable. Cada filtro vive en la URL para compartir enlaces. Los filtros nunca incluyen texto del prompt ni PII.

### 6. Estados vacio, carga y error

| Estado | Donde ocurre | Tratamiento |
|---|---|---|
| Sin datos todavia | Instalacion nueva | Mensaje con el siguiente paso concreto: conectar el SDK, ejemplo de codigo de 3 lineas |
| Sin calibrador | Antes de correr el juez | Banner: "Los puntajes no estan calibrados". Los tokens muestran entropia normalizada con aviso explicito |
| Filtro sin resultados | Generaciones, cola | Mensaje con boton "Limpiar filtros" |
| Cola vacia | Revision | Mensaje de cola completa y conteo revisado hoy |
| Carga | Cualquier fragmento htmx | `hx-indicator` mas `aria-busy="true"` en la region; skeleton sin animacion si `prefers-reduced-motion` [no verificado: soporte de htmx para reduced-motion] |
| Error de servidor | Fragmento htmx | Mensaje en region `role="alert"`, boton reintentar, id de error para soporte |
| Muestra chica | Graficos de calibracion | Aviso cuando un bin tiene menos de N tokens; bins vacios se omiten, no se dibujan en cero |
| Modelo no soportado | Ingesta | Error explicito con lista de arquitecturas soportadas |

### 7. Responsive

- El detalle de generacion apila texto y panel lateral bajo 768px.
- El heatmap capa x experto usa scroll horizontal propio dentro de un contenedor. La pagina no tiene scroll horizontal.
- Las tablas pasan a tarjetas bajo 640px, o mantienen 3 columnas clave con scroll horizontal acotado.
- Los graficos SVG usan `viewBox` y ancho 100%. Las etiquetas de ejes se reducen, nunca se superponen.
- La cola de revision prioriza escritorio. En movil mantiene botones de 24px o mas.

### 8. Stack minimo server-rendered

| Opcion | Ventaja | Costo | Veredicto |
|---|---|---|---|
| FastAPI + Jinja2 + htmx 2.x + SVG generado en Python | Sin Node ni paso de build; htmx pesa unos 16k min.gz y no tiene dependencias; tests con `TestClient` sobre HTML; foco y ARIA bajo control total | Hay que escribir los generadores SVG | Recomendado |
| FastAPI + Jinja2 + Chart.js o uPlot en cliente | Interactividad lista | JS en cliente, accesibilidad del canvas a resolver aparte | Opcional solo para zoom de series largas |
| SPA React/Next.js (patron de Langfuse) | Ecosistema grande | Build de Node, mas RAM en la laptop de 8 GB, dos lenguajes y dos suites de test | Rechazado para el MVP |
| pygal (SVG desde Python) | Graficos SVG listos | Licencia LGPL-3.0; menos control sobre ARIA | Rechazado |
| matplotlib a SVG | Maduro, curvas de sklearn listas | Dependencia pesada para un servidor web; SVG con poca semantica [no verificado: tamano exacto] | Solo para reportes offline |

Datos de stack verificados:

- FastAPI usa `Jinja2Templates` y `StaticFiles`. Desde FastAPI 0.108.0 la firma es `TemplateResponse(request=..., name=..., context=...)`. `url_for` funciona en templates.
- htmx ofrece `hx-get`, `hx-trigger="every 2s"`, `load`, `revealed`, `hx-swap`, `hx-indicator`, `hx-push-url`, `hx-boost` y una extension SSE. El header `HX-Request` permite devolver fragmento o pagina completa.
- htmx 4.0 salio hace poco. NPM mantiene 2.x como default. La guia rapida referencia 2.0.11.

## Implicancias para Vigia

### Requisitos de datos y puntaje

1. Mostrar en la UI el puntaje calibrado P(error) por token, nunca la entropia cruda (Vasconcelos et al.).
2. Discretizar el puntaje en 3 niveles visibles (bajo, medio, alto) con umbrales de la politica activa; el valor continuo aparece al enfocar o pasar el mouse.
3. Agregar subtokens BPE en palabras para el resaltado, con el maximo del puntaje por palabra; conservar el detalle por subtoken en el panel lateral.
4. Guardar en cada generacion la version del calibrador y del umbral; la UI muestra esa version en el detalle.
5. Agrupar tokens contiguos marcados en "fragmentos dudosos"; la accion (revision, RAG, abstencion) se aplica por fragmento, no por token.

### Codificacion visual del token

6. Usar codificacion triple: fondo con intensidad secuencial, subrayado con estilo por nivel (punteado, ondulado, doble) y texto oculto para lector de pantalla.
7. Elegir colores de fondo con texto a 4.5:1 en cada nivel y en modo oscuro; validar con un test automatico de contraste.
8. Usar una escala secuencial derivada de cividis o una rampa de un solo tono naranja de Okabe-Ito; nunca rojo-verde.
9. Envolver cada fragmento en `<mark>` con `aria-describedby` hacia un texto como "confianza baja, nivel alto, 0.83"; no anunciar inicio y fin de cada token individual.
10. Ofrecer un control de umbral con input numerico y slider; el umbral local solo cambia la vista, no la politica.
11. Mostrar un aviso fijo en el detalle: "Texto sin marcar no esta verificado" (riesgo de exceso de confianza, arXiv 2605.28571).

### Navegacion y teclado

12. Implementar atajos en el detalle: `j`/`k` para ir al fragmento siguiente o anterior, `Enter` para abrir sus senales, `?` para la hoja de atajos.
13. Implementar la cola con atajos estilo Langfuse: `1` correcto, `2` error, `3` incierto, `Ctrl+Enter` completar y siguiente, flechas para navegar.
14. Usar una tabla estatica para Generaciones; usar el patron grid de APG solo en el heatmap capa x experto.
15. Mover el foco al encabezado del nuevo contenido tras cada swap htmx de pagina; anunciar resultados de filtros en una region `aria-live="polite"`.
16. Garantizar objetivos de 24x24 px para botones de accion y que barras fijas no tapen el foco.

### Cola de revision

17. Modelar estados de item: pendiente, reservado, necesita otro revisor, completado (patron LangSmith).
18. Reservar un item por revisor con expiracion configurable para evitar trabajo duplicado.
19. Configurar un minimo de revisores por item; ocultar la decision de otros revisores hasta completar.
20. Exportar las decisiones humanas como etiquetas para recalibrar; mostrar el acuerdo humano vs juez en la pagina Calibracion.

### Graficos

21. Generar los 7 graficos canonicos como SVG server-side con funciones Python puras y testeables.
22. Acompanar la curva de confiabilidad con histograma de conteos y ECE; no usar Brier como unica metrica.
23. Mostrar ROC y PR juntas; la PR incluye linea base de prevalencia y el punto del umbral actual.
24. Mostrar latencia p50/p95/p99 baseline vs Vigia con la linea de meta +5%; pintar en estado de alerta si se supera.
25. Marcar cada recalibracion como linea vertical en la tendencia de tasa de flags.
26. Dar a cada SVG `role="img"`, titulo, descripcion con la conclusion principal y una tabla de datos en `<details>`.
27. Usar Okabe-Ito para series categoricas y diferenciar series tambien por estilo de linea o marcador.

### Stack e ingenieria

28. Usar FastAPI + Jinja2 con autoescape activo + htmx 2.0.x fijado y servido localmente, sin CDN en produccion.
29. Prohibir el filtro `|safe` sobre texto del modelo o del usuario; el resaltado se construye con spans escapados.
30. Devolver fragmento o pagina completa segun el header `HX-Request`; cada ruta funciona sin JavaScript.
31. Usar polling htmx de 10 s o mas en Resumen; dejar SSE para una fase posterior.
32. Testear cada pagina con `TestClient` y asserts sobre HTML; correr un chequeo axe o equivalente en la fase de verificacion [no verificado: herramienta exacta en CPU sin navegador].
33. Definir colores como tokens CSS en `:root` con variante oscura; sin framework CSS pesado.

### Seguridad y privacidad en la UI

34. Redactar PII por defecto en listas; mostrar el texto completo solo en el detalle y a roles autorizados.
35. Registrar en auditoria cada cambio de umbral, politica o calibrador con usuario y fecha.
36. Aislar datos por organizacion en cada consulta; los filtros de URL no cruzan organizaciones.

## Edge cases y riesgos

- **Tokenizacion en espanol**: tildes, `n` con virgulilla y signos de apertura pueden partir una palabra en varios subtokens. El agregado por palabra evita resaltados parciales raros.
- **Tokens de espacio y salto de linea**: un token marcado puede ser solo un espacio. La UI lo muestra con un glifo visible o lo asigna a la palabra siguiente.
- **Canal de razonamiento de gpt-oss**: el formato harmony separa razonamiento y respuesta final [no verificado en esta nota]. La UI debe separar ambos canales y no mezclar sus puntajes.
- **Textos largos**: 10 000 tokens generan 10 000 spans en el DOM. Hay que paginar o resaltar solo palabras sobre el umbral minimo.
- **Streaming**: el puntaje llega por token mientras se genera. El MVP muestra el resultado al terminar; el streaming queda para despues.
- **Recalibracion**: un cambio de calibrador altera colores historicos. Cada generacion guarda su version y la UI lo indica.
- **Muestra chica en calibracion**: pocos tokens por bin generan curvas ruidosas. Hace falta minimo por bin y, si es posible, intervalos.
- **Prevalencia muy baja de errores**: AUROC alto puede ocultar precision baja. La PR y la precision al umbral son obligatorias.
- **XSS**: la salida del modelo puede contener HTML o scripts. El autoescape de Jinja y la prohibicion de `|safe` son criticos.
- **Exceso de confianza del revisor**: el resaltado puede hacer confiar en el texto no marcado (arXiv 2605.28571).
- **Verbosidad en lector de pantalla**: anunciar cada token cansa al usuario. Solo se anuncian fragmentos.
- **Locale**: Argentina usa coma decimal y zona horaria UTC-3. La UI formatea numeros y fechas segun locale y guarda en UTC.
- **htmx 4 reciente**: atributos o defaults pueden cambiar entre 2.x y 4.x. Fijar version evita roturas.
- **Modo oscuro**: los fondos de resaltado deben recalcularse para mantener 4.5:1.

## Objeciones

- La evidencia HCI usa logprobs y codigo o tareas en ingles. Falta un estudio con senales MoE y texto en espanol de finanzas, salud o legal.
- No hay dato de que 3 niveles discretos superen a la intensidad continua. La decision se basa en reducir carga visual y en WCAG 1.4.1, no en un experimento.
- No pude verificar la UI concreta de Galileo, W&B Weave ni Patronus: las paginas no cargaron o no describen la interfaz.
- No verifique precios de ninguna herramienta.
- No verifique los criterios 1.4.3 y 1.4.11 en su pagina propia; los valores 4.5:1 y 3:1 son los estandar conocidos.
- No mida el costo de renderizar 10 000 spans en un navegador de gama baja.
- Falta un test de usabilidad con revisores reales de un cliente objetivo.

## Fuentes

- Phoenix, conceptos de anotacion: https://arize.com/docs/phoenix/tracing/concepts-tracing/annotations-concepts
- Phoenix, expresiones de filtro: https://arize.com/docs/phoenix/tracing/how-to-tracing/filter-expressions
- Phoenix, release 20.2-20.3: https://arize.com/docs/phoenix/release-notes/08-2026/08-17-2026-trace-filters-and-analytics-sql
- Phoenix, repositorio: https://github.com/Arize-ai/phoenix
- Langfuse, anotacion: https://langfuse.com/docs/scores/annotation
- Langfuse, colas de anotacion: https://langfuse.com/docs/evaluation/evaluation-methods/annotation-queues
- Langfuse, dashboards: https://langfuse.com/docs/metrics/features/custom-dashboards
- Langfuse, observabilidad: https://langfuse.com/docs/observability/overview
- Langfuse, repositorio: https://github.com/langfuse/langfuse
- LangSmith, colas de anotacion: https://docs.langchain.com/langsmith/annotation-queues
- Galileo, incertidumbre (via resultado de busqueda, hoy 404): https://docs.galileo.ai/galileo/gen-ai-studio-products/galileo-llm-fine-tune/using-uncertainty
- Cleanlab TLM: https://help.cleanlab.ai/tlm/tutorials/tlm_advanced/
- Patronus Percival: https://docs.patronus.ai/docs/percival/concepts
- Patronus productos: https://patronus.ai/products
- Humanloop cierre: https://wandb.ai/wandb_fc/product-announcements-fc/reports/Humanloop-is-Sunsetting-Migrate-to-Weights-Biases-as-an-alternative--VmlldzoxMzk4ODc1Nw
- Humanloop cierre: https://agenta.ai/blog/humanloop-sunsetting-migration-and-alternative
- Weave, tracing: https://docs.coreweave.com/weave/guides/tracking/tracing
- Vasconcelos et al.: https://arxiv.org/abs/2302.07248
- Granularidad de incertidumbre: https://arxiv.org/abs/2605.28571 y https://arxiv.org/html/2605.28571
- Saito y Rehmsmeier 2015: https://pmc.ncbi.nlm.nih.gov/articles/PMC4349800
- scikit-learn, calibracion: https://scikit-learn.org/stable/modules/calibration.html
- matplotlib, colormaps: https://matplotlib.org/stable/users/explain/colors/colormaps.html
- Nunez et al. 2018, cividis: https://pmc.ncbi.nlm.nih.gov/articles/PMC6070163
- Okabe-Ito hex: https://conceptviz.app/blog/okabe-ito-palette-hex-codes-complete-reference
- WCAG 2.2: https://www.w3.org/TR/WCAG22/
- WCAG 1.4.1: https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html
- WCAG 2.5.8: https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html
- ARIA APG grid: https://www.w3.org/WAI/ARIA/apg/patterns/grid/
- MDN mark: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/mark
- SVG accesible: https://data.europa.eu/apps/data-visualisation-guide/accessible-svg-and-aria
- htmx foco tras swap: https://flaviocopes.com/courses/htmx/focus-after-a-swap/
- htmx: https://htmx.org/ y https://htmx.org/docs/
- FastAPI templates: https://fastapi.tiangolo.com/advanced/templates/
- pygal: https://pypi.org/project/pygal/
