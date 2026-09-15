---
title: Hosting bajo USD 15/mes para bot WhatsApp (Java + Postgres + dashboard)
created: 2026-09-09
tags: [research, quetepario-chatbot, research, hosting, vps, paas]
status: seed
sources: [https://docs.hetzner.com/general/infrastructure-and-availability/price-adjustment/, https://docs.hetzner.com/cloud/servers/overview/, https://docs.hetzner.com/general/billing-and-account-management/billing-at-hetzner/payment-overview/, https://contabo.com/en/vps/, https://help.contabo.com/en/support/solutions/articles/103000398194-why-is-there-a-setup-fee-on-my-cloud-vps-10-order-, https://donweb.com/es-ar/hosting-cloud-servers-vps, https://donweb.com/es-ar/cloud-server-vps, https://render.com/pricing, https://render.com/docs/free, https://railway.com/pricing, https://fly.io/docs/about/pricing/, https://fly.io/docs/about/discontinued-plans/, https://neon.com/pricing, https://vercel.com/pricing, https://docs.oracle.com/en-us/iaas/Content/FreeTier/freetier_topic-Always_Free_Resources.htm, https://www.ecb.europa.eu/stats/policy_and_exchange_rates/euro_reference_exchange_rates/html/eurofxref-graph-usd.en.html, https://www.bna.com.ar/Personas, https://www.boletinoficial.gob.ar/detalleAviso/primera/318447/20241219, https://askmonarca.com/guias/percepciones-exterior, https://www.infoviajera.com/2025/12/desde-enero-ya-no-regira-el-recargo-del-30-por-compras-con-tarjeta-en-dolares/]
---
---
mision: Elegir hosting bajo USD 15/mes para Spring Boot + Postgres + dashboard estático con HTTPS público estable para el webhook de WhatsApp Cloud API, pagando desde Argentina.
verifier: ninguno (sin veredicto cross-family todavía)
---

## Tronco

Un VPS Hetzner CX23 con Docker Compose cuesta ~USD 7/mes (~USD 9 con percepción AR) y deja margen para el LLM; los PaaS gratuitos no sostienen un webhook siempre encendido.

## Qué es

El ticket pide comparar VPS chico con Docker contra free tiers de PaaS. Criterios: costo real mensual, pago desde Argentina, límites del free tier, complejidad operativa. Fecha de relevamiento: 2026-09-09.

## Supuestos de carga

- Spring Boot necesita 512 MB a 1 GB de RAM en reposo. Postgres chico necesita 256 MB más.
- El webhook de Meta exige HTTPS público con certificado válido. Un servicio que "duerme" falla el webhook.
- El dashboard es estático. Cualquier servidor web lo sirve.

## Tipos de cambio usados

- EUR/USD: 1,1652 (ECB, referencia 2026-09-09).
- USD/ARS: 1.535 venta billete (Banco Nación, 2026-09-09).

## Tabla comparativa

| Opción | Specs | Costo/mes (lista) | Costo/mes USD | Pago desde AR | HTTPS público | Free tier / límites | Complejidad | Fuente |
|---|---|---|---|---|---|---|---|---|
| Hetzner CX23 (FSN/NBG/HEL) | 2 vCPU, 4 GB, 40 GB | EUR 5,49 + EUR 0,50 IPv4, sin IVA | ~7,0 | Visa/MC/Amex/PayPal. Percepción 30% | Sí, IP fija. Caddy/Traefik da TLS | No hay free tier | Media (Docker Compose, backups propios) | docs.hetzner.com price-adjustment (vigente 2026-06-15); servers/overview; payment-overview |
| Contabo Cloud VPS 4 | 4 vCPU, 8 GB, 100 GB | USD 5,28 (24 meses, impuestos incl.); lista USD 6,60 | 5,28 a 6,60 | Tarjeta/PayPal. Percepción 30% | Sí, IPv4 dedicada | No hay free tier | Media | contabo.com/en/vps. Precio a 1 mes y setup fee: NO confirmados en fuente primaria (la help page dio 404) |
| DonWeb Cloud Server (AR) | 1 vCPU, 1 GB, 10 GB | ARS 5.165 con descuento (lista ARS 8.034), IVA incl. | ~3,4 (lista ~5,2) | Pesos, sin percepción | Sí, IPv4 incluida | No hay free tier | Media | donweb.com/es-ar/hosting-cloud-servers-vps. Precio de 2 vCPU/2 GB: NO encontrado (configurador JS) |
| Render Hobby + Starter + Postgres Basic | 0,5 CPU, 512 MB + PG 256 MB, 1 GB | USD 0 + 7 + 6 | 13,0 | Tarjeta. Percepción 30% → ~16,9 | Sí, TLS gestionado | Free web duerme a los 15 min; free PG expira a los 30 días | Baja | render.com/pricing; render.com/docs/free |
| Railway Hobby | uso medido | USD 5 base con USD 5 de crédito; RAM USD 10/GB-mes; vCPU USD 20/mes | estimado 10 a 15 (sin verificar con carga real) | Tarjeta. Percepción 30% | Sí | Free: USD 1 de crédito/mes, insuficiente | Baja | railway.com/pricing |
| Fly.io | shared-cpu-1x 256 MB | USD 2,02 VM + USD 2 IPv4 + vol 0,15/GB + PG ~2 | estimado 7 a 10; VM de 1 GB NO relevada | Tarjeta. Percepción 30% | Sí | Sin free tier para cuentas nuevas desde 2024-10-07 | Media | fly.io/docs/about/pricing; discontinued-plans |
| Neon Free (solo Postgres) | 0,5 GB, 100 CU-h/proyecto | USD 0 | 0 | No requiere pago | N/A | Autosuspend a los 5 min; reanuda solo | Baja | neon.com/pricing |
| Vercel Hobby (solo dashboard) | estático, 100 GB/mes | USD 0 | 0 | No requiere pago | Sí | Solo uso no comercial. Pro USD 20 | Baja | vercel.com/pricing |
| Oracle Always Free | 2 micro AMD 1 GB o Ampere A1 (ver doc) | USD 0 | 0 | Tarjeta para verificar. Alta desde AR: NO verificada | Sí, IP pública | Oracle reclama instancias idle (<20% CPU 7 días) | Alta (OCI, red, firewall) | docs.oracle.com Always Free Resources |

## Impuestos al pagar desde Argentina

- Pago con tarjeta en moneda extranjera suma percepción 30% (RG 5617/2024, Boletín Oficial 2024-12-19). Es a cuenta de Ganancias y se puede pedir devolución.
- La nota "desde enero 2026 no rige el 30%" fue una broma del Día de los Inocentes (Info Viajera lo aclara). La percepción sigue vigente (Monarca, actualizado 2026-06-21).
- IVA 21% sobre servicios digitales del exterior: aplica según listado de ARCA. No verifiqué si Hetzner, Contabo o Render están en la lista. Tratarlo como posible +21%.
- Hetzner factura sin IVA UE a clientes fuera de la UE: NO verificado en fuente primaria.
- Costo Hetzner efectivo: ~USD 7 × 1,30 = ~USD 9,1. Con IVA extra: ~USD 10,6.
- Costo Render efectivo: USD 13 × 1,30 = ~USD 16,9. Supera el presupuesto.

## Evidencia / mecanismo

- Hetzner subió precios el 2026-06-15. CX23 pasó de EUR 3,99 a EUR 5,49. CPX22 pasó a EUR 19,49. Los planes CPX quedaron fuera de presupuesto.
- Hetzner acepta Visa, Mastercard, Amex, UnionPay y PayPal. Aclara que no todas las opciones existen en todos los países.
- Render Free no sirve para webhook: el servicio duerme a los 15 minutos. Meta reintenta pero marca fallos.
- Render pago más barato suma USD 13 antes de impuestos. Solo 512 MB para la JVM.
- Fly.io ya no ofrece free tier a cuentas nuevas. Cobra por uso.
- Railway Free da USD 1/mes. No alcanza para un contenedor siempre encendido.
- Vercel Hobby prohíbe uso comercial. El SaaS es comercial.
- Oracle Always Free es gratis pero reclama instancias ociosas. El alta desde Argentina no la verifiqué.
- Contabo da más RAM por menos plata, pero el precio publicado exige 24 meses prepagos.

## Recomendación

Elegir **Hetzner CX23** en Falkenstein o Helsinki con Docker Compose. Correr Spring Boot, Postgres y Caddy en un solo host. Caddy sirve el dashboard estático y emite TLS con Let's Encrypt.

Justificación:
1. Costo ~USD 7 lista, ~USD 9 a 11 efectivo. Quedan USD 4 a 6 para el LLM.
2. Sin compromiso de plazo. Se factura por hora.
3. 4 GB de RAM alcanzan para JVM + Postgres con margen.
4. IP fija y puerto 443 propio: el webhook no depende de un PaaS que duerme.
5. Un solo `docker-compose.yml` es la menor carga cognitiva para el próximo editor.

Alternativas:
- **DonWeb**: pedir cotización de 2 vCPU / 2 GB en el configurador. Paga en pesos sin percepción. Si sale bajo ~ARS 14.000/mes, gana en costo efectivo.
- **Contabo**: solo si se acepta prepago de 24 meses.
- **Neon Free** como Postgres externo si el VPS queda justo de RAM.

## Aplicable a mmorch

- No aplica. Esta nota es infraestructura de proyecto, no ruteo ni verificación.

## Objeciones

- La latencia Europa ↔ Argentina (~200 ms) no se midió. Para un webhook es aceptable, pero falta el dato.
- El costo de DonWeb a 2 GB es la incógnita que podría cambiar la recomendación. Requiere abrir el configurador a mano.
- El IVA 21% sobre servicios digitales no está confirmado para estos proveedores. Puede sumar ~USD 1,5.
- Los precios de Hetzner cambiaron dos veces en 2026. Re-verificar antes de contratar.

## Veredicto cross-family

- Pendiente. Sin verificación externa todavía.

## Links

- Ticket: `QueTePario/ChatBot/docs/agents/wayfinder/issues/01-hosting-15usd.md`
- Mapa: `QueTePario/ChatBot/docs/agents/wayfinder/map.md`
