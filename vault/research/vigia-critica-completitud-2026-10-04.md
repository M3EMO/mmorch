---
title: Vigia - critica de completitud de las 8 notas de research
mision: Que contradicciones, huecos de produccion y afirmaciones sin fuente quedan en el research de Vigia antes de pasar a Plan, y que dicen las fuentes sobre los 3 huecos mas graves.
status: seed
confidence:
verifier:
tags: [research, vigia]
sources:
  - https://arxiv.org/html/2606.22798
  - https://docs.vllm.ai/en/latest/design/plugin_system.html
  - https://docs.vllm.ai/en/latest/api/vllm/model_executor/layers/fused_moe/routed_experts_capturer/
  - https://docs.vllm.ai/en/v0.23.0/training/routed_experts_replay/
  - https://github.com/IBM/vllm-hook
  - https://arxiv.org/html/2603.06588
  - https://htmx.org/docs/
  - https://four.htmx.org/extensions/hx-csp
  - https://scikit-learn.org/stable/model_persistence.html
  - https://arxiv.org/abs/2605.07260
created: 2026-10-04
---

# Vigia - critica de completitud del research

## Tronco

El research cubre bien senales, extraccion en HF, calibracion, UX y seguridad, pero tiene cuatro contradicciones que cambian el diseno (punto de captura del router, costo de InnerExpert, composicion del tier barato, 403 contra 404), no decide la topologia de despliegue (las senales solo existen dentro del proceso que corre el modelo), y apoya la meta de menos de 5% y la tesis "router solo alcanza" en afirmaciones sin ninguna medicion.

## Notas revisadas

Lei completas las 8 notas:

- [[vigia-mercado-competencia-2026-10-04]]
- [[vigia-senales-paper-2026-10-04]]
- [[vigia-moe-internals-hf-2026-10-04]]
- [[vigia-serving-integracion-2026-10-04]]
- [[vigia-calibracion-evaluacion-2026-10-04]]
- [[vigia-ux-dashboard-2026-10-04]]
- [[vigia-seguridad-operacion-2026-10-04]]
- [[vigia-integraciones-edge-cases-2026-10-04]]

## 1. Contradicciones entre notas

Cada fila indica la nota que gana y por que. La columna "Gana" es la decision recomendada para el Plan.

| ID | Tema | Posicion A | Posicion B | Gana | Motivo |
|---|---|---|---|---|---|
| K1 | Punto de captura del router | mercado, serving e integraciones: forward hook sobre el modulo router (`mlp.router`, clase router) | moe-internals: pre-hook sobre el bloque MoE que recalcula `F.linear(x, W, b)` | moe-internals | Con MXFP4, `GptOssMLP.forward` parcheado no llama al router y el hook nunca dispara. El pre-hook funciona en v4, v5 y MXFP4. Mantener EC-10 (`HookNotFiredError`) como red de seguridad. |
| K2 | Costo de InnerExpert | mercado: "casi cero, reusa la pasada del generador" | senales-paper y calibracion: +147% solo senales MoE, +205% con el detector LR | senales-paper | La Tabla 14 del paper es la fuente primaria. La fila de mercado es falsa y no debe llegar al copy comercial. |
| K3 | Origen del costo de ~3x | calibracion: "el SVD del hidden score explica gran parte"; serving: "puede venir de atencion o hidden states" | senales-paper (Tabla 14): LLM-Check hidden solo cuesta +8.7% y atencion +4.5%; las senales MoE juntas cuestan +147% | senales-paper | El costo vive en la extraccion MoE (salidas por experto `(B,S,k,d)` y expert hidden score). Esto no prueba que el router solo sea barato: nadie aislo ese costo. |
| K4 | Composicion del tier barato del MVP | calibracion D1: el MVP incluye "similitud entre expertos activos" | moe-internals (tier 2) y serving (nivel E): la similitud exige salidas por experto, es cara y no existe con MXFP4 | moe-internals y serving | La similitud entre expertos es opt-in. El item (1) del plan ("dispersion entre expertos") queda fuera del default de produccion en gpt-oss MXFP4. |
| K5 | Que dice el paper | moe-internals H8: "gana XGBoost"; "en Gemma domina la entropia (0.884)"; "total ~3x (~1.15 s por 100 tokens)" | senales-paper Tablas 1, 3 y 14: LR gana por token en OLMoE; 0.884 es usage entropy y la router entropy es 0.327 en Gemma; 1.146 s es la vanilla | senales-paper | Leer "entropia" sin calificar lleva a apostar por la router entropy, que esta anticorrelacionada en Gemma. |
| K6 | Respuesta entre tenants | integraciones EC-48: 403 | seguridad D-06 y SEC-23: 404 | seguridad | El 404 no confirma la existencia del recurso. Corregir EC-48. |
| K7 | Streaming en el MVP | ux: el MVP muestra el puntaje al terminar | serving e integraciones: `vigia` por chunk SSE, `stream_policy: buffer|annotate_only` | Decision de producto | Propuesta: la API soporta streaming con `annotate_only`; el panel muestra trazas terminadas. El backend `vllm-routed-experts` rechaza streaming (EC-39). |
| K8 | UI sin calibrador | ux tabla 6: "los tokens muestran entropia normalizada con aviso" | ux requisito 1: "nunca la entropia cruda" (Vasconcelos) | ux requisito 1 | Sin calibrador, el panel no resalta tokens. Muestra solo el banner y el ranking relativo. |
| K9 | Juez por defecto | seguridad D-02 y calibracion D4: juez local por defecto (ej. gpt-oss-120b) | calibracion seccion 3: costo calculado con la API de DeepSeek; paper: GLM-5.1 via DeepInfra | Ninguna, falta decidir | gpt-oss-120b exige ~80 GB de GPU. Un cliente mediano no lo tiene y la laptop tampoco. Propuesta: interfaz `JudgeBackend` (endpoint compatible con OpenAI), sin modelo por defecto; un juez externo exige opt-in, redaccion y evento de auditoria; los tests usan un juez falso determinista. |
| K10 | Umbral | calibracion: un umbral por CRC con un alpha sobre FNR | mercado y ux: un umbral por accion y 3 niveles visibles; integraciones: `score >= umbral` por regla | Unificar | Un alpha da un solo tau. Varias acciones piden varios alphas o Learn-then-Test con varios riesgos. El Plan debe fijar que cada regla de politica referencia un alpha calibrado, no un numero libre. |
| K11 | Detector generico | mercado, tier Community: "detector generico pre-entrenado" | calibracion R14, integraciones EC-07: el detector se ata a (modelo, cuantizacion); otro modelo da `CalibrationMismatchError` | calibracion | Un detector generico solo existe por modelo anfitrion. Ademas, su entrenamiento exige GPU que el proyecto no tiene. El tier Community entrega el pipeline, no pesos. |
| K12 | Modelo de test en CPU | mercado: "un MoE diminuto, por ejemplo OLMoE" | moe-internals H7: OLMoE pesa 13.8 GB y no entra en 8 GB | moe-internals | Usar configs aleatorios sin red y granite-3.1-1b-a400m. |
| K13 | Licencia de Mu-SHROOM | senales-paper: CC-BY-4.0 | calibracion: HF dice CC-BY-4.0 y el paper dice CC BY-NC-SA 4.0 | calibracion (duda abierta) | Mu-SHROOM solo sirve como benchmark interno hasta confirmar la licencia. No se embebe en el producto. |
| K14 | Forma de `router_logits` | calibracion y mercado: `(batch, seq, num_experts)` | moe-internals: los routers v5 devuelven `(N, E)` aplanado | moe-internals | El extractor re-da forma con el `(B, T)` de la entrada. |
| K15 | Tipo de los IDs de vLLM | senales, serving, integraciones: int16 | doc `latest` de `RoutedExpertsCapturer` (leida hoy): buffer int32, salida uint8 (hasta 256 expertos) o uint16 | doc latest | El parser del backend reducido acepta uint8, uint16 e int16. |
| K16 | Paralelismo en vLLM | serving: el replay no soporta sequence ni pipeline parallelism | doc `latest` de `RoutedExpertsCapturer`: el buffer maneja data, tensor, sequence y expert parallelism | doc latest (parcial) | La matriz de compatibilidad debe fijarse por version de vLLM, no por la doc v0.21. |
| K17 | Persistencia del detector | calibracion: `FrozenEstimator` y `CalibratedClassifierCV` de scikit-learn | seguridad SEC-51/52: nada de pickle, hash fijado | seguridad | scikit-learn advierte que pickle y joblib ejecutan codigo al cargar y que no hay carga soportada entre versiones. El artefacto de produccion es un JSON propio (pesos LR, medias, desvios, Platt a/b, tau, metadatos) con hash. scikit-learn queda solo en entrenamiento. |

## 2. Temas que faltan para un producto listo para produccion

Ordenados por impacto en la arquitectura.

1. **Topologia de despliegue.** Las senales solo existen dentro del proceso que ejecuta el modelo. Un proxy delante de vLLM no puede calcularlas. Ninguna nota dibuja el flujo completo: quien hospeda el modelo, donde corre el detector, como llegan las features al proxy y al panel. Ver hueco H1.
2. **Validacion con un MoE real en la maquina de build.** Todas las notas dicen "la CI solo valida contratos". Ninguna propone medir AUROC real en CPU. granite-3.1-1b-a400m (2.67 GB, espanol, Apache 2.0) entra en 8 GB. Generar ~2000 respuestas de 65 tokens son ~130k tokens. Eso es una corrida nocturna en CPU [estimado; falta medir tok/s]. Ese dato seria el primer AUROC en espanol sobre senales MoE.
3. **Esquema de datos y volumen.** Falta el modelo de datos (tenants, traces, tokens, spans, policies, calibrations, review_items, audit), las migraciones (Alembic) y una estimacion de volumen. Ejemplo: 1M tokens por dia con 24 capas por F features en float16 no es despreciable. Falta decidir que se guarda por token (score y pocas features agregadas) y que se descarta.
4. **Cola de trabajos.** Webhooks con reintentos de 24 h, calibracion, purga de retencion y re-consulta RAG necesitan una cola persistente. Ninguna nota elige el mecanismo. Para self-host chico conviene una tabla de jobs en Postgres (`SELECT ... FOR UPDATE SKIP LOCKED`) y no agregar Redis [propuesta, sin fuente].
5. **Frontera core / ee.** Seguridad pone el panel y el multi-tenant en `ee/`. Mercado pone "API REST autoalojable" y el proxy OpenAI en table-stakes sin ubicarlos. Falta una tabla modulo por modulo con licencia y dependencias permitidas.
6. **Licenciamiento del tier pago.** No hay mecanismo de licencia para `ee/` en un despliegue air-gapped (clave firmada offline, verificacion sin red, comportamiento al vencer).
7. **Seguridad del panel con htmx.** La nota de seguridad exige CSP con nonce y sin `unsafe-inline`. La nota de UX elige htmx 2.x. Ninguna revisa que htmx 2.x inyecta estilos inline, evalua codigo por defecto y guarda snapshots de pagina en localStorage. Ver hueco H2.
8. **Formato del artefacto del detector.** Ver K17. Falta el schema JSON versionado y su test de ida y vuelta.
9. **CI y tooling en Windows con 8 GB.** Los tests de RLS piden Postgres en Docker. Docker Desktop en 8 GB compite con el modelo de granite. Falta decidir: Postgres nativo de Windows, o un job de CI remoto, o testcontainers solo en CI.
10. **Plan de aceptacion consolidado.** Hay 54 SEC, 12 OPS y 69 EC sueltos. Falta una matriz unica feature -> criterio de aceptacion -> test -> gate, con los gates de AUROC y de latencia definidos para CPU y marcados "pendiente GPU" para gpt-oss.
11. **Juez: prompt en espanol y reglas de etiquetado.** Las notas citan el prompt de InnerExpert (ingles). Falta el prompt en espanol con reglas para abstencion, premisa falsa, ocurrencias repetidas y numeros. Falta el protocolo de la muestra humana (quien anota, cuantos items, kappa minimo).
12. **Copy y onboarding.** Falta quickstart de 3 lineas, OpenAPI publicado, guia de despliegue y textos de UI en espanol rioplatense consistentes (glosario).
13. **Versionado y compatibilidad.** Falta politica de semver para la libreria, `schema_version` del contrato por token, y matriz soportada de transformers (>=5) y vLLM.
14. **Pruebas de carga del servicio.** Hay benchmark de latencia del modelo. No hay prueba de carga de la API de ingesta, del panel ni del export.

## 3. Afirmaciones sin fuente que sostienen el diseno

| ID | Afirmacion | Donde aparece | Estado | Riesgo si es falsa |
|---|---|---|---|---|
| A1 | El tier de router (R / tier 1) cumple la meta de menos de 5% de latencia | moe-internals D2 ("este tier debe cumplir la meta"), serving 11 | Sin medicion. El unico dato cercano (H3 abajo) mide ~7% para capturar solo IDs y pesos top-k en vLLM | La meta comercial central del plan cae |
| A2 | Las senales baratas (router y usage) dan un AUROC util por token | serving 14, calibracion D1 | Sin medicion. Senales sueltas: 0.44 a 0.68. El paper no publica la ablacion "solo MoE baratas" | La tesis del producto cae; un probe lineal sobre hidden state (AUC 0.85-0.90, Obeso) podria ser mejor y tambien barato |
| A3 | Las senales funcionan en espanol | todas | Sin medicion | Toda la propuesta para LatAm |
| A4 | Las senales funcionan en gpt-oss (top-4, softmax sobre k) | todas | Sin medicion; el paper usa top-8 | Signo y fuerza de cada senal desconocidos |
| A5 | El forward MXFP4 parcheado no llama al router | moe-internals H5 | Inferido del codigo, no ejecutado | Si es falso, K1 pierde urgencia pero el pre-hook sigue siendo correcto |
| A6 | Un plugin de vLLM calcula features dentro de `custom_routing_function` compatible con CUDA graphs y por debajo de 5% | serving 8 | Sin fuente ni prototipo | La ruta de produccion GPU no existe |
| A7 | Un juez local (gpt-oss-120b) etiqueta bien en espanol | seguridad D-02, mercado 7 | Sin fuente | Etiquetas ruidosas y detector sesgado |
| A8 | `ExpertsInterface.register` existe con esa firma | moe-internals H4 | No verificado | El tier 2 necesita otro mecanismo |
| A9 | Texto off-policy (teacher forcing) sirve para senales MoE | calibracion 4 | No verificado | Sin RAGTruth ni HaluEval como datos de entrenamiento |
| A10 | Precios de Vigia USD 300-600/mes y 15-30K/ano | mercado | Hipotesis declarada | Modelo de negocio |
| A11 | El 3x de InnerExpert viene de atencion o hidden states | serving objeciones | Contradicho por la Tabla 14 (K3) | Lleva a creer que quitar hidden y atencion basta |

## 4. Investigacion propia de los 3 huecos mas graves

### H1. Como sale de vLLM una senal por token hacia la API (topologia)

Pregunta: puede un plugin de vLLM calcular features del router y devolverlas al cliente por la API OpenAI?

Hallazgos:

- La doc de plugins lista cinco grupos. `general_plugins` registra modelos. `io_processor_plugins` procesa entrada y salida, pero "para pooling models". `endpoint_plugins` agrega rutas HTTP nuevas al servidor OpenAI. Ningun grupo documenta agregar campos a las respuestas estandar de chat.
- Consecuencia: el campo `vigia` dentro de `choices[]` no sale de un plugin oficial sin parchear la capa de serving. El patron viable es lateral: el plugin de modelo escribe features por `request_id` en un buffer; un endpoint plugin (o el proxy Vigia) las lee por id al terminar.
- `RoutedExpertsCapturer` en la doc `latest` captura solo IDs. Usa un buffer `(max_num_batched_tokens, num_layers, num_experts_per_tok)` int32 y entrega uint8 o uint16. No captura pesos ni logits. La pagina de replay de v0.23.0 da 404, igual que `latest`. El flag puede haberse movido o renombrado.
- vLLM-Hook (IBM, Apache 2.0, arXiv 2603.06588) subclasea el worker V1, instala forward hooks tras `load_model` y devuelve los datos por disco (`torch.save` por run) o rpc/shm. El paper no publica overhead ni trata CUDA graphs.
- Dato de costo nuevo: arXiv 2606.22798 (Chen et al., 2026) captura IDs top-k y pesos post-softmax en vLLM sobre gpt-oss-20b/120b, Qwen3-30B-A3B y Qwen3-Next. Reporta "~7% overhead at rollout collection". No da hardware ni batch. Es el unico numero publico sobre gpt-oss, y ya supera 5% sin calcular entropia sobre los N logits.

Implicancias:

1. La topologia de produccion tiene dos procesos: vLLM con plugin Vigia (modelo + captura + reduccion a features en GPU) y el servicio Vigia (detector, politicas, panel, store). El plugin publica features por `request_id`. El servicio arma la respuesta `vigia`.
2. Para el MVP en CPU, el servicio Vigia hospeda el modelo con el backend HF. En ese modo no hay proxy: Vigia es el servidor OpenAI-compatible.
3. La meta de menos de 5% debe reformularse como objetivo a medir, con el 7% de 2606.22798 como referencia previa. El reporte debe mostrar la latencia por nivel aunque falle la meta.
4. El plugin vLLM queda atado a versiones. La matriz soportada se fija por version probada.

### H2. htmx 2.x frente a la CSP estricta de la nota de seguridad

Hallazgos (doc oficial de htmx 2.x):

- `allowEval` vale `true` por defecto. Habilita `new Function()` para filtros de eventos, `hx-on:`, y `hx-vals` / `hx-headers` con prefijo `js:`. Con una CSP sin `unsafe-eval` esas funciones fallan.
- htmx inyecta un `<style>` inline para `.htmx-indicator` al cargar (`includeIndicatorStyles`). Eso viola `style-src` sin `unsafe-inline`. La doc ofrece `inlineStyleNonce` e `inlineScriptNonce`.
- `historyCacheSize` vale 10 por defecto. htmx guarda snapshots de la pagina en `localStorage`. En Vigia esos snapshots contendrian texto de generaciones con PII. La doc recomienda `0` o `hx-history="false"` en paginas sensibles.
- `allowScriptTags` vale `true`: htmx ejecuta `<script>` del contenido cargado.
- `hx-disable` sobre un subarbol impide que htmx procese atributos inyectados alli, y el contenido inyectado no puede anularlo.
- htmx 4 trae una extension `hx-csp` con nonce gating y safe eval. La nota de UX fija 2.x, asi que esa extension no aplica.

Requisitos nuevos (con test):

1. Config htmx por `<meta name="htmx-config">`: `allowEval=false`, `allowScriptTags=false`, `includeIndicatorStyles=false`, `historyCacheSize=0`, `selfRequestsOnly=true`.
2. CSS del indicador en la hoja propia del panel.
3. Todo texto del modelo o del usuario se renderiza dentro de un contenedor con `hx-disable`, ademas del autoescape de Jinja.
4. Sin `hx-on` en las plantillas; un test de plantillas falla si aparece `hx-on` o `js:`.
5. Test de headers: la CSP no contiene `unsafe-inline` ni `unsafe-eval`, y la pagina funciona con esa CSP (test de humo en navegador headless en la fase de verificacion).
6. Test: tras navegar el panel, `localStorage` no contiene `htmx-history-cache`.

### H3. Formato seguro del artefacto de calibracion

Hallazgos (doc de persistencia de scikit-learn):

- pickle, joblib y cloudpickle "pueden ejecutar codigo arbitrario al cargar". Solo deben cargarse desde fuentes verificadas.
- No hay forma soportada de cargar un modelo entrenado con otra version de scikit-learn.
- skops.io carga solo tipos confiables. ONNX es la opcion mas segura y no requiere Python.

Implicancias:

1. El artefacto entregado al cliente es un JSON propio. Contiene version de schema, pesos y sesgo de la LR, medias y desvios, lista ordenada de features, parametros Platt (a, b), tau por alpha, y metadatos (modelo, cuantizacion, plantilla, juez, hash del dataset, versiones).
2. La inferencia del detector es un producto punto en numpy o torch. No importa scikit-learn en runtime.
3. Test de ida y vuelta: entrenar con scikit-learn, exportar a JSON, cargar con el runtime propio y comparar puntajes con tolerancia 1e-9.
4. El cargador rechaza campos desconocidos, hash invalido y feature list distinta de la que emite el extractor.

### Dato lateral

arXiv 2605.07260 analiza ruteo en Qwen3-30B-A3B, GPT-OSS-20B, DeepSeek-V2-Lite y OLMoE. Concluye que el router acierta en tokens confiados y es "poco informativo en los tokens fragiles". Es evidencia indirecta de que la confianza del router se relaciona con la dificultad del token en gpt-oss. No es una medicion de alucinacion.

## Requisitos derivados para el Plan

1. Captura del router por pre-hook sobre el bloque MoE, con test de MXFP4 simulado y `HookNotFiredError`.
2. Tier por defecto = logits de salida + router (F0 y F1). Similitud entre expertos solo opt-in.
3. Baselines obligatorios en todo reporte: entropia de salida, NLL y probe lineal sobre residual stream.
4. Gate de evidencia: un experimento en CPU con granite-3.1-1b-a400m, en espanol, con etiquetas computables (MKQA-es, SQAC), que mida AUROC por token de F1 contra los baselines y la latencia de F1.
5. Topologia de dos procesos para GPU (plugin vLLM + servicio Vigia, enlace por `request_id`) y de un proceso para CPU (Vigia hospeda el modelo con HF).
6. Meta de latencia tratada como medicion publicada por nivel, con 7% como referencia previa.
7. Entre tenants siempre 404.
8. Sin calibrador no hay resaltado por token.
9. `JudgeBackend` sin modelo por defecto; juez externo con opt-in, redaccion y auditoria; juez falso en tests.
10. Cada regla de politica referencia un alpha calibrado por CRC o LTT.
11. Artefacto del detector en JSON propio con hash; sin pickle en runtime.
12. Config htmx endurecida y tests de CSP y de localStorage.
13. Cola persistente de jobs en la base, sin dependencia extra.
14. Tabla core / ee por modulo con test de imports.
15. Mu-SHROOM solo como benchmark interno hasta confirmar licencia.

## Objeciones

- No medi nada. Todo lo de latencia sigue siendo una estimacion o un numero ajeno. La laptop no tiene torch instalado y no descargue dependencias.
- El 7% de arXiv 2606.22798 no trae hardware, batch ni version de vLLM. Sirve como orden de magnitud, no como baseline.
- No pude confirmar si `--enable-return-routed-experts` sigue en vLLM v0.23: la pagina dio 404 en `latest` y en v0.23.0.
- No verifique que un endpoint plugin de vLLM pueda leer un buffer escrito por un general plugin en otro proceso (workers separados). El enlace por `request_id` puede exigir memoria compartida o un store externo.
- El dato que falta para todo el producto es A2: AUROC por token de senales de router solas, en espanol, en un MoE real. Sin eso, el Plan arma un producto sobre una hipotesis.

## Fuentes

- MoE routing en vLLM sobre gpt-oss, ~7% de overhead: https://arxiv.org/html/2606.22798
- Sistema de plugins de vLLM: https://docs.vllm.ai/en/latest/design/plugin_system.html
- RoutedExpertsCapturer (latest): https://docs.vllm.ai/en/latest/api/vllm/model_executor/layers/fused_moe/routed_experts_capturer/
- Routed experts replay v0.23.0 (404): https://docs.vllm.ai/en/v0.23.0/training/routed_experts_replay/
- vLLM-Hook: https://github.com/IBM/vllm-hook y https://arxiv.org/html/2603.06588
- htmx 2.x docs (seguridad y config): https://htmx.org/docs/
- htmx 4 hx-csp: https://four.htmx.org/extensions/hx-csp
- Persistencia de modelos en scikit-learn: https://scikit-learn.org/stable/model_persistence.html
- Misrouting en MoE (incluye GPT-OSS-20B): https://arxiv.org/abs/2605.07260
