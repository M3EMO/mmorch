---
title: Oráculo agente por tipo de entregable (2026-10-15)
created: 2026-10-15
tags: [research, orchestration, builder-e2e, oraculo, reward-hacking, sdlc, f2]
status: verdict
confidence: "media: fracciones razonadas, no medidas; loopback y cifras de ARCA verificados"
sources: ["https://www.afip.gob.ar/gananciasYBienes/ganancias/personas-humanas-sucesiones-indivisas/deducciones/documentos/Deducciones-personales-art-30-jul-dic-2026.pdf", "https://www.afip.gob.ar/gananciasYBienes/ganancias/personas-humanas-sucesiones-indivisas/declaracion-jurada/documentos/Tabla-Art-94-LIG-per-jul-a-dic-2026.pdf", "https://playwright.dev/python/docs/docker", "https://web.dev/articles/lighthouse-ci", .scratch/builder-e2e/issues/03-oraculo-agente.md]
---
## Pregunta

¿Qué evidencia junta un agente al manejar cada tipo de entregable, y qué parte admite una aserción determinista? Ticket 03 del mapa `.scratch/builder-e2e/`.

## Respuesta corta

- El oráculo agente separa dos roles: el agente maneja el entregable y junta evidencia; las aserciones deterministas dictan el veredicto.
- La verdad vive fuera del worktree, en solo lectura, y el coder nunca la ve. Es el freno principal contra el reward hacking.
- Frenos adicionales: implementación de referencia escrita por otro agente, relaciones metamórficas, propiedades con Hypothesis y mutation testing en sombra.

## Fracción computable estimada (razonada, no medida)

| Tipo | Computable | Lo que queda en juicio |
|---|---|---|
| CLI | ~90 % | ergonomía de la salida |
| API HTTP | ~85 % | mensajes de error |
| Web estática | ~70 % | texto SEO y aspecto visual |
| Funcionalidad en repo existente | 70–85 % | estilo; depende de la suite |
| Flujo o automatización | ~60 % | contenido de mensajes; el efecto real queda afuera sin red |

## Verificado

- Con `--network none --cap-drop ALL`, un `http.server` en 127.0.0.1 respondió 200 y una salida a internet falló. El preview y el agente pueden convivir en un contenedor.
- ARCA, julio a diciembre 2026: ganancia no imponible $5.585.736,93; deducción especial art. 30 inc. c) ap. 2 $26.811.537,29; escala art. 94 con 9 tramos del 5 % al 35 %. Leído de los PDF oficiales.

## Para la calculadora de sueldo neto (F2)

- No hay simulador oficial de neto. La verdad sale de una referencia independiente, una convención declarada (anualizada o acumulativa RG 4.003) y un cruce con ejemplos publicados con tolerancia.
- Relaciones metamórficas útiles: neto monótono en el bruto; neto = 0,83 × bruto debajo del umbral; aportes fijos arriba del tope; neto = bruto − líneas del desglose; un peso más de bruto sube el neto como mucho un peso; el fijo de cada tramo iguala el impuesto acumulado del tramo anterior; escalar bruto y tablas por k escala el neto por k.
- Bloquean: casos ocultos en la página real, metamórficas, consola limpia, cero requests externos, Lighthouse SEO y accesibilidad ≥ 0,9, axe-core sin violaciones graves, metadatos completos, parámetros con fuente y vigencia.
- En sombra: rúbricas de texto SEO y de calidad visual con anclas 1/3/5, de a pares.

## Sin verificar

- Tope de ANSES de octubre 2026 (Res. 284/2026): solo fuentes secundarias.
- Si el tope rige el 3 % de obra social.
- Playwright y Lighthouse dentro del contenedor sin red: la imagen pesa ~0,9 GB y no se probó.
