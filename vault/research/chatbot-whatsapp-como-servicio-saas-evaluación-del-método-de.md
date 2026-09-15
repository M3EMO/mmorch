---
title: Chatbot WhatsApp como servicio SaaS (evaluación del método del video)
created: 2026-09-09
tags: [research, quetepario, chatbot, saas, modelo-negocio, whatsapp]
status: seed
sources: [https://youtu.be/3JX0frxLi78]
---
## El modelo del video (Agustín Medina, canal Aink)

Vende agentes de WhatsApp con IA a negocios locales (clínicas, talleres).
Cobra instalación (800 a 5000 €) más cuota mensual (150 a 300 €).
Prospecta con Google Maps Scraper (Apify) más mensajes personalizados
generados con Claude Code. El bot agenda citas, responde preguntas
frecuentes y deriva a un humano si hace falta.

## Lectura honesta de los números

- 250 €/día es la meta final, no el mes 1. El video lo aclara: mes 1 da 0.
- De la cuota mensual, ~50 € se van en costos de API (WhatsApp, LLM).
- Los precios son "públicos" del propio curso del autor: son un piso
  optimista de venta, no garantía de cierre.
- El modelo en sí es real: agencias de automatización con IA ya venden
  esto. No es una fantasía, pero tampoco es tan fácil como "3 minutos".

## Riesgos técnicos a tener en cuenta

- WhatsApp Cloud API oficial (Meta) exige verificación de negocio.
- Baileys es una librería no oficial: riesgo de baneo de número.
- Twilio es oficial pero cobra por mensaje enviado.
- El LLM detrás (OpenAI u otro) es un costo variable por conversación.

## Sobre "hacerlo en TypeScript"

El stack no es lo que determina si el negocio funciona.
TypeScript sirve igual que Python o cualquier otro lenguaje del stdlib
de cada uno. Ponytail: usar el lenguaje en que el usuario sea más
rápido, no el del video.

## Recomendación

Separar dos negocios distintos:
1. QueTePario: cliente real, caso de uso concreto, ya tiene contacto.
2. Vender el chatbot como servicio a OTROS comercios: negocio nuevo,
   necesita validación propia (primer cliente pagando, no una demo).

Usar QueTePario como el primer caso real construido. Después decidir
si conviene empaquetarlo y salir a vender a otros negocios locales.
