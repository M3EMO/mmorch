# Oráculo agente: evidencia computable por tipo de entregable
Type: research
Status: resolved
Blocked by: 01
Map: ../map.md

## Question

¿Qué evidencia puede juntar un agente al manejar cada tipo de entregable, y qué parte de esa evidencia admite una aserción determinista?

El ticket 01 fijó que el builder es general: arma productos, servicios, flujos y funcionalidades. El primer entregable es la calculadora de sueldo neto de F2 (sitio web estático).

- Web: Playwright con aserciones sobre el DOM, la red y la consola.
- CLI: código de salida, stdout y archivos producidos.
- API: tests de contrato sobre las respuestas.
- Aspecto visual o experiencia: juicio en sombra con un verificador separado.
- Flujo (automatización de varios pasos): estados intermedios, efectos y reintentos.
- Funcionalidad sobre un repo existente: regresión de la suite y contrato de la parte nueva.

Restricción del entorno: los comandos corren en un contenedor sin red (`sdlc.toml contenedor`); el preview y el agente deben convivir en ese contenedor.

Salida esperada: una tabla de evidencia por tipo de entregable, con la fracción computable estimada y el riesgo de reward hacking de cada fuente.

## Answer

Resuelto (2026-10-15) por investigación de un agente, con verificación del orquestador. Nota del vault: `vault/research/oráculo-agente-por-tipo-de-entregable-2026-10-15.md`.

Fracción computable estimada (razonada, no medida): CLI ~90 %, API ~85 %, web estática ~70 %, funcionalidad 70–85 % (depende de la suite existente), flujo ~60 % (el efecto real queda afuera por `--network none`).

Patrón común contra el reward hacking: la verdad vive FUERA del worktree, en solo lectura, y el coder nunca la ve. Se combina con una implementación de referencia que escribe otro agente, relaciones metamórficas y mutation testing en sombra.

Verificado por el orquestador:

- El loopback funciona con `--network none --cap-drop ALL`: un `http.server` en 127.0.0.1 respondió 200 y una salida a internet falló.
- Los PDF de ARCA (julio a diciembre 2026) dan: ganancia no imponible $5.585.736,93; deducción especial art. 30 inc. c) ap. 2 $26.811.537,29; escala del art. 94 con 9 tramos del 5 % al 35 %.

Evidencia mínima del MVP de F2:

- Bloquea: casos ocultos de neto cargados en la página real (±$1); relaciones metamórficas sobre ~200 brutos con semilla oculta y diff contra la referencia; consola sin errores y cero requests fuera de 127.0.0.1; Lighthouse CI con SEO y accesibilidad ≥ 0,9 (desempeño solo avisa); axe-core sin violaciones critical ni serious; metadatos (title 30–60, description 70–160, `lang="es-AR"`, canonical, un h1, JSON-LD válido); parámetros con fuente y vigencia, más el check de coherencia de la escala.
- En sombra: juez razonador con rúbricas de texto SEO y de calidad visual (anclas 1/3/5), de a pares; mutmut sobre el módulo de cálculo.

Decisiones que pasan al walkthrough de F2:

- Convención de cálculo: anualizada (12 meses, sin SAC, soltero, sin hijos) o acumulativa de la RG 4.003; dan retenciones distintas en un mes dado.
- El neto no tiene simulador oficial: la verdad sale de una referencia independiente, la convención declarada y un cruce con ejemplos publicados con tolerancia.

Sin verificar: el tope de ANSES (Res. 284/2026, solo fuentes secundarias), si el tope rige el 3 % de obra social, Playwright y Lighthouse dentro del contenedor (no se bajó la imagen, ~0,9 GB) y Lighthouse sin red.
