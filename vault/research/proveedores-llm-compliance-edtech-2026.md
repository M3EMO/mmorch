---
title: Proveedores de LLM — compliance/residencia de datos para juez cross-family EdTech
status: verified
confidence: 0.8
verifier: gemini-2.5-flash (WebSearch, síntesis manual)
tags: [research, compliance, llm-providers, edtech, estudio, estudio-saas, data-residency, gdpr]
sources:
  - https://xorabyte.com/blog/aws-bedrock-gdpr-soc2-compliance/
  - https://aws.amazon.com/bedrock/faqs/
  - https://compound.law/en-DE/tools/azure-openai/
  - https://azure.microsoft.com/en-us/blog/enterprise-trust-in-azure-openai-service-strengthened-with-data-zones/
  - https://www.digitalapplied.com/blog/self-hosting-open-weight-llms-2026-deployment-decision-guide
  - https://www.promptquorum.com/local-llms/deepseek-local-china-data-privacy-2026
  - https://skywork.ai/skypage/en/deepseek-data-privacy-security-guide/2047585299882700800
created: 2026-08-03
---

## Qué es

Investigación de mercado (research de precios/compliance publicados, NO pruebas propias de costo/calidad) para el TODO abierto en el proyecto "Plataforma educativa SaaS": el stack de verificación cruzada (DeepSeek↔Gemini↔GLM) tiene 2 de 3 familias con sede en China, lo cual es un riesgo de compliance real para clientes institucionales (universidades privadas tipo UdeSA). Pregunta: ¿hay alternativas viables que preserven el principio de verificación cruzada (familias de modelo genuinamente distintas) sin ese riesgo?

## Evidencia / mecanismo

**DeepSeek — riesgo confirmado, no solo teórico.** La API hosteada de DeepSeek almacena datos en servidores en China, sin opción de residencia US/UE, sin DPA, sin BAA, sin mecanismo de transferencia legal a la UE. Esto puede violar HIPAA/SOC2/leyes de residencia de datos para empresas reguladas — no es una precaución excesiva, es un gap de compliance documentado por múltiples fuentes independientes en 2026.

**Pero el modelo (los pesos) es distinto del servicio hosteado.** DeepSeek publica sus pesos bajo licencia permisiva — un despliegue self-hosted (VPC propia, corriendo el modelo vía vLLM/SGLang) mantiene la diversidad de familia de modelo (sigue siendo "DeepSeek" como arquitectura/entrenamiento, genuinamente distinto de GPT/Claude/Gemini) SIN que los datos salgan de la infraestructura del operador. Esto resuelve el problema de raíz sin sacrificar el principio de verificación cruzada del proyecto (familias de modelo realmente distintas, no la misma familia dos veces).

**AWS Bedrock — mejor opción "managed" para el juez.**
- Un solo DPA cubre TODOS los modelos del catálogo (Claude, Llama, Mistral, Titan) — AWS es el procesador único, los proveedores de modelo son sub-procesadores bajo los términos de AWS. Simplifica la negociación institucional a 1 contrato en vez de 1 por proveedor.
- No usa inputs/outputs para entrenar modelos base; no comparte contenido con los proveedores de modelo.
- Cubre SOC, ISO, HIPAA-eligible, GDPR; control de residencia regional; endpoint VPC para aislamiento de red; audit log vía CloudTrail.
- AWS European Sovereign Cloud (Brandenburg, Alemania) disponible desde enero 2026 — relevante si el mercado se expande a UE.
- **Límite**: el catálogo de Bedrock no incluye Gemini (es de Google) ni GLM — para mantener 3 familias hay que combinar Bedrock (Claude/Llama) + Vertex AI de Google (Gemini, con su propio DPA, generalmente sólido) + un 3er origen genuinamente distinto (self-hosted DeepSeek/Qwen en VPC, ver abajo) en vez de las 3 vía DeepSeek/Gemini/GLM directo.

**Azure OpenAI — comparable a Bedrock, ecosistema Microsoft.**
- DPA vía Online Services Terms + Data Protection Addendum (relación de procesador Art. 28 GDPR).
- SOC2, HIPAA (vía BAA incluido en el DPA para clientes elegibles), FedRAMP.
- "Data Zones" (EU/US) para residencia regional; prompts/outputs no se usan para entrenar sin permiso explícito.
- Aporta principalmente modelos GPT — mismo límite que Bedrock: no diversifica familia por sí solo, es un canal de compliance para UNA familia (OpenAI), no una solución de 3 familias.

**Self-hosted (Llama/Qwen/DeepSeek abiertos) — la opción de compliance más fuerte, con umbral de costo.**
- Para cargas HIPAA/GDPR/SOC2, el despliegue self-hosted en VPC propia es "el único camino directo sin necesitar BAA de terceros" — evita la cadena de sub-procesadores por completo.
- Costo: ~$0.15–0.18 por millón de tokens de salida en H100 con vLLM a batch alto — muy competitivo, PERO el punto de equilibrio real está en volumen: self-hosting le gana a las APIs manejadas recién arriba de ~600M tokens/mes (código) o ~1.2B tokens/mes (chat). Un cluster 8×H100 reservado cuesta ~$22-28K/mes — esto es infraestructura de volumen alto, no algo que tenga sentido para un piloto de una sola universidad.
- **Conclusión de costo**: self-hosting NO es la opción de arranque — es la opción de madurez, cuando el volumen de evaluaciones ya justifica la inversión fija en GPU. Para el piloto 1 y clientes tempranos, Bedrock/Vertex/Azure (pago por token, sin compromiso de infraestructura) son el camino de menor fricción.

## Aplicable al proyecto (Plataforma educativa SaaS)

Reconciliación recomendada para `05-arquitectura-api.md` §6 / `08-privacidad-y-cumplimiento.md`:
1. **Corto plazo (piloto 1 → primeros clientes institucionales)**: migrar el juez cruzado de DeepSeek/Gemini/GLM directo a **Claude o Llama vía Bedrock + Gemini vía Vertex AI + un 3er modelo** — mantiene 3 familias genuinamente distintas, un DPA por canal (2 DPAs: AWS + Google, en vez de 3 con exposición China), sin inversión de infraestructura.
2. **Mediano plazo (volumen alto, >600M tokens/mes)**: evaluar self-host de un modelo abierto (Qwen o el propio DeepSeek-open-weights) en VPC propia como 3er miembro del panel cross-family — recién cuando el volumen justifique el costo fijo de GPU reservada.
3. Esto no es solo mitigación de riesgo — es también una simplificación contractual: 1-2 DPAs (Bedrock cubre varios modelos bajo un solo contrato) es más fácil de vender a una universidad que 3 DPAs separados con 3 proveedores de distinto origen.

## Veredicto cross-family
- No corrido por mmorch_ensemble_verify (esto es research de mercado, no una afirmación técnica verificable por cross-family) — verificado por consistencia entre 4 búsquedas independientes que coinciden en los mismos hechos (DPA único de Bedrock, gap de compliance de DeepSeek hosteado, umbral de costo de self-hosting).
- Confianza 0.8: los datos de pricing/compliance son de fuentes de terceros (blogs, guías), no de la documentación oficial primaria de cada proveedor leída directamente — recomendable confirmar contra la documentación oficial de AWS/Azure/Google antes de firmar un DPA real.

## Links
- [[estudio-saas-plataforma-educativa]]
