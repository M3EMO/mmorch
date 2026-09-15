---
title: WhatsApp Cloud API desde Argentina — alta, costos y sandbox (2026-09)
created: 2026-09-09
tags: [research, quetepario-chatbot, whatsapp, cloud-api, pricing, argentina, bsp]
status: seed
sources: [https://developers.facebook.com/docs/whatsapp/pricing, https://developers.facebook.com/documentation/business-messaging/whatsapp/pricing, https://developers.facebook.com/docs/whatsapp/pricing/updates-to-pricing, https://developers.facebook.com/docs/whatsapp/messaging-limits, https://developers.facebook.com/docs/whatsapp/cloud-api/get-started, https://developers.facebook.com/docs/whatsapp/cloud-api/phone-numbers, https://developers.facebook.com/docs/development/release/business-verification, https://www.twilio.com/en-us/whatsapp/pricing, https://www.twilio.com/docs/whatsapp/sandbox, https://www.twilio.com/docs/whatsapp/self-sign-up, https://360dialog.com/pricing, https://ominiflow.com/whatsapp-api-pricing/argentina, https://www.basework.com.ar/blog/whatsapp-business-api-argentina, https://www.ycloud.com/blog/whatsapp-api-message-pricing-update-effective-october-1-2026, https://www.wati.io/en/blog/whatsapp-service-message-pricing/, https://help.wanotifier.com/en/article/test-phone-number-limitations-in-direct-setup-kt0ly2/]
---
## Tronco

Para 300 conversaciones/mes desde Argentina conviene Cloud API directa con un número móvil propio nuevo. El costo de Meta es cercano a cero hoy y sube a ~USD 10/mes desde octubre 2026. Un BSP agrega USD 10-55/mes sin beneficio para este volumen.

Misión: responder qué hace falta para operar un bot con WhatsApp Cloud API desde Argentina y cuánto cuesta.

Fecha de investigación: 2026-09-09. Los precios cambian cada trimestre. Revalidar antes del 2026-10-01.

## Qué es

WhatsApp Cloud API es la API oficial de Meta, hosteada por Meta. No requiere BSP. Meta cobra por mensaje de plantilla entregado desde el 2025-07-01. La tarifa depende de la categoría y del país del destinatario (fuente: developers.facebook.com/docs/whatsapp/pricing).

Categorías vigentes:
- Marketing: siempre se cobra.
- Utility: gratis dentro de la ventana de servicio de 24 h. Se cobra fuera de ella.
- Authentication: se cobra, con descuento por volumen.
- Service (mensaje libre dentro de la ventana de 24 h): gratis hoy.

La ventana de servicio abre cuando el cliente escribe. Dura 24 h. Adentro el bot manda texto libre sin plantilla (fuente: docs/whatsapp/pricing).

## Checklist de alta paso a paso

1. Crear una cuenta de Meta Business (portfolio) a nombre del comercio.
2. Registrarse como developer en developers.facebook.com y crear una app con caso de uso WhatsApp (fuente: cloud-api/get-started).
3. Meta crea un número de prueba y una WABA. Probar el envío con el token temporal.
4. Configurar el webhook HTTPS público. Verificar el token de verificación. Recibir eventos `messages` y estados.
5. Crear un System User y un token permanente con permisos `whatsapp_business_messaging`, `whatsapp_business_management`, `business_management` (fuente: get-started).
6. Conseguir el número productivo. Debe recibir SMS o llamada de voz. No puede estar registrado en WhatsApp consumer ni en WhatsApp Business App; si lo está, hay que borrar esa cuenta primero (fuente: cloud-api/phone-numbers).
7. Registrar el número: `Request Code` (SMS o VOICE) y `Verify Code`. Cargar display name (fuente: phone-numbers).
8. Cargar método de pago en la WABA. Desde 2026-04-01 Argentina factura en ARS (fuente: updates-to-pricing).
9. Iniciar Business Verification en Business Manager > Security Center. Se sube documento de razón social y documento de dirección/teléfono (fuente: docs/development/release/business-verification y help center 1095661473946872).
10. Crear plantillas utility (confirmación de pedido, envío) y esperar aprobación.
11. Pasar la app a modo Live.

Tiempos de verificación: la doc oficial no publica plazo. Twilio dice que varía por región y puede tardar semanas (fuente: twilio.com/docs/whatsapp/self-sign-up). Terceros reportan 2 a 10 días hábiles (wati.io, no verificado).

## Límites iniciales

Un portfolio nuevo puede iniciar conversaciones con 250 clientes únicos por 24 h (fuente: docs/whatsapp/messaging-limits). El límite sube a 2.000 al verificar el negocio, o al enviar 2.000 plantillas de calidad en 30 días. Después escala solo a 10K, 100K e ilimitado.

El límite aplica solo a mensajes iniciados por el negocio. Responder a clientes que escriben no consume límite. Para QTP el límite de 250 alcanza.

## Número nuevo: opciones y costo

Requisitos de Meta: número propio, con código de país y área, capaz de recibir SMS o voz. VoIP figura como "no recomendado" para OTP por SMS pero no está prohibido. Fijo es válido (fuente: cloud-api/phone-numbers).

Opciones:
- Chip prepago (Claro/Personal/Movistar) en un celular viejo. Es la opción más simple. Precio del chip: NO encontrado en fuente oficial. Movistar solo publica recargas promocionales "desde $8.000" ARS (movistar.com.ar/prepago). Listados de Mercado Libre muestran chips a ~$500 ARS, dato no confiable.
- Número virtual argentino (Zavu u otros). Zavu no publica precio; dice "depende de disponibilidad" (zavu.dev). Twilio no lista números argentinos en su página de precios; solo "international numbers desde USD 1.15/mes" (twilio.com/en-us/sms/pricing/ar). Dato para Argentina: NO encontrado.
- Número fijo del local. Funciona si atiende la llamada de verificación.

Recomendación: chip prepago móvil. Recibe SMS seguro. Una vez registrado en Cloud API, el chip solo se necesita para re-verificar.

## Tabla de precios por categoría — Argentina

Meta publica las tarifas en CSV/PDF ("USD rates", "USD volume tiers") efectivas 2026-07-01. No pude extraer el CSV por fetch. Los valores siguientes vienen de fuentes secundarias que citan ese rate card.

| Categoría | USD por mensaje | ARS por mensaje (ago-2026) | Fuente |
|---|---|---|---|
| Marketing | 0.0618 | 89,56 | ominiflow.com; basework.com.ar |
| Utility (fuera de ventana) | 0.0120 | 37,68 | ominiflow.com; basework.com.ar |
| Authentication | 0.0220 | 37,68 | ominiflow.com; basework.com.ar (discrepan) |
| Utility dentro de ventana 24 h | 0 hasta 2026-09-30 | 0 | developers.facebook.com/docs/whatsapp/pricing |
| Service (texto libre en ventana) | 0 hasta 2026-09-30 | 0 | idem |

Discrepancia: Ominiflow da authentication USD 0.0220. Basework da ARS 37,68 (igual a utility). Verificar en el CSV oficial antes de usar authentication.

Cambio crítico el 2026-10-01 (fuentes: ycloud.com, wati.io; la página oficial de updates no mostró el texto en mi fetch):
- Los service messages (texto libre saliente dentro de la ventana de 24 h) pasan a cobrarse por mensaje.
- La tarifa es igual a utility del país del destinatario (~USD 0.012 en Argentina).
- Hay 1.000 service messages gratis por número por mes, sin acumular.
- Utility dentro de la ventana también pasa a cobrarse.
- Mensajes entrantes del cliente siguen gratis.
- La ventana de 72 h de Click-to-WhatsApp sigue gratis.
- Meta Business Agent (IA de Meta) cobra USD 2 por millón de tokens desde 2026-08-01. No aplica si el bot es propio.

Confianza media en este bloque: son 3 fuentes secundarias coincidentes. Falta el texto oficial de Meta.

## Sandbox y número de prueba para demo

Opción Meta (gratis): el test number de Cloud API envía mensajes gratis a hasta 5 destinatarios cargados en el portal (fuente: developers.facebook.com/docs/whatsapp/cloud-api/get-started). Los clientes no pueden escribirle a ese número. Las plantillas creadas ahí no se migran al número productivo (fuente: help.wanotifier.com). Sirve para demo interna con Maxi y 4 personas más.

Opción Twilio Sandbox: número compartido +1 415 523 8886. Cada usuario manda `join <código>`. Sesión de 72 h. Solo 3 plantillas preaprobadas. Trial con 100 mensajes gratis; después tarifa estándar (fuente: twilio.com/docs/whatsapp/sandbox). Peor para demo: el número muestra logo de Twilio.

Recomendación: demo con test number de Meta.

## API directa vs BSP

| Opción | Costo fijo | Costo variable | Pros | Contras |
|---|---|---|---|---|
| Cloud API directa | USD 0 | Solo tarifa Meta | Sin intermediario. Factura en ARS. | Webhook y tokens propios. Soporte solo Meta. |
| Twilio | USD 0 | Meta + USD 0.005 por mensaje (in y out) | Sandbox fácil. Soporte. | Para 2.100 msgs/mes: +USD 10.5. Duplica el costo. |
| 360dialog | EUR 49/mes por número (plan Regular) | Meta sin markup | Sin markup. | EUR 49 supera el presupuesto total de USD 15. |

Fuentes: twilio.com/en-us/whatsapp/pricing; 360dialog.com/pricing.

Veredicto: API directa. El equipo ya construye API en Java; el webhook es trivial. Un BSP no reduce trabajo de alta: la verificación de Meta se hace igual (fuente: twilio.com/docs/whatsapp/self-sign-up).

## Estimación mensual para 300 conversaciones

Supuestos: 300 conversaciones iniciadas por el cliente. El bot responde 6 mensajes salientes por conversación (1.800 service messages). 100 plantillas utility fuera de ventana (confirmación de pedido, envío). 0 marketing.

Hasta 2026-09-30:
- Service: 1.800 × 0 = USD 0.
- Utility fuera de ventana: 100 × 0.0120 = USD 1.20.
- Total Meta: ~USD 1.20/mes.

Desde 2026-10-01:
- Service: 1.800 − 1.000 gratis = 800 × 0.0120 = USD 9.60.
- Utility: 100 × 0.0120 = USD 1.20.
- Total Meta: ~USD 10.80/mes (~ARS 33.900 a 37,68 ARS/mensaje).

Sensibilidad: cada mensaje saliente extra por conversación suma 300 × 0.012 = USD 3.60/mes. Con 10 mensajes por conversación el total sube a ~USD 25. Conviene que el bot responda en pocos mensajes largos, no muchos cortos.

Con Twilio sumar USD 0.005 × (300 entrantes + 1.900 salientes) = ~USD 11/mes. Con 360dialog sumar EUR 49/mes.

Costo del número: chip prepago, precio no confirmado. Estimar ARS 500 a 8.000 una vez.

## Objeciones

- El cambio de octubre 2026 se sostiene solo en fuentes secundarias. Si Meta no lo aplica, el costo queda en ~USD 1/mes.
- Los valores USD de Argentina vienen de calculadoras de terceros. El CSV oficial puede diferir en centavos.
- La verificación de negocio puede demorar semanas y exige documentos de QTP. Sin verificar, el límite es 250 iniciadas por día; no bloquea el MVP.
- El presupuesto total del proyecto es USD 15/mes. Con el escenario de octubre, WhatsApp consume USD 11 y deja USD 4 para LLM.

## Veredicto cross-family

Pendiente. Correr `/verify-cross` sobre la tabla de precios cuando el CSV oficial esté accesible.

## Links

- [[quetepario-chatbot]]
- Ticket: `ChatBot/docs/agents/wayfinder/issues/02-whatsapp-cloud-api-ar.md`
