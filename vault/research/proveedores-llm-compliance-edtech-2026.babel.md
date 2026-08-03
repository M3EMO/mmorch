---
source: proveedores-llm-compliance-edtech-2026.md
lexicon: v2
ratio: 0.488
fidelity: 1.0
derived: true
---
LLM-PROV: EdTech cross-family judge compliance/data residency. Research: price/compliance public, NOT cost/quality tests. SaaS Edu Platform: DeepSeek↔Gemini↔GLM stack has 2/3 China-based families = institutional risk (e.g., UdeSA). Alt viable? Preserves cross-verify (distinct families) w/o risk?

EVID:
DeepSeek: API host = China data, NO US/EU residency, NO DPA/BAA/legal EU transfer. Violates HIPAA/SOC2/data residency for regulated firms. Documented compliance gap (2026).
BUT: Model weights ≠ hosted service. Self-hosted (VPC, vLLM/SGLang) = distinct family (DeepSeek arch/train) ≠ GPT/Claude/Gemini, data stays in operator infra. Solves root issue, preserves cross-verify principle.

AWS Bedrock: Best "managed" for judge.
- 1 DPA for ALL catalog models (Claude, Llama, Mistral, Titan). AWS = sole processor, model providers = sub-processors. Simplifies institutional negotiation.
- No input/output use for base model training; no content sharing w/ model providers.
- SOC, ISO, HIPAA-eligible, GDPR compliant. Regional residency control. VPC endpoint. Audit log (CloudTrail).
- AWS European Sovereign Cloud (Brandenburg, Jan 2026) for EU expansion.
- LIMIT: Catalog lacks Gemini (Google) & GLM. For 3 families: Bedrock (Claude/Llama) + Google Vertex AI (Gemini, solid DPA) + 3rd distinct origin (self-hosted DeepSeek/Qwen in VPC).

Azure OpenAI: Comparable to Bedrock, MS ecosystem.
- DPA via Online Services Terms + Data Protection Addendum (Art. 28 GDPR processor).
- SOC2, HIPAA (BAA in DPA for eligible clients), FedRAMP.
- "Data Zones" (EU/US) for regional residency. Prompts/outputs not used for training w/o explicit permission.
- Primarily GPT models. LIMIT: Not a 3-family solution, compliance channel for ONE family (OpenAI).

Self-hosted (Llama/Qwen/DeepSeek open): Strongest compliance, cost threshold.
- HIPAA/GDPR/SOC2: Self-hosted in own VPC = "only direct path w/o 3rd party BAA". Avoids sub-processor chain.
- Cost: ~$0.15–0.18/M output tokens (H100, vLLM, high batch). Competitive, BUT break-even at volume: >~600M tokens/mo (code) or ~1.2B tokens/mo (chat) vs managed APIs. 8×H100 cluster ~$22-28K/mo = high-volume infra, not for single university pilot.
- CONCLUSION: Self-hosting NOT for startup. Maturity option, when volume justifies fixed GPU investment. For pilot/early clients: Bedrock/Vertex/Azure (pay-per-token, no infra commitment) = lowest friction.

Reconciliación `05-arquitectura-api.md` §6 / `08-privacidad-y-cumplimiento.md`:
1. **Corto plazo (piloto 1 → clientes inst.)**: Juez cruzado DeepSeek/Gemini/GLM → Claude/Llama (Bedrock) + Gemini (Vertex AI) + 3er modelo. Mantiene 3 familias. 2 DPAs (AWS+Google) vs 3 (exposición China). Sin inversión infra.
2. **Mediano plazo (>600M tokens/mes)**: Evaluar self-host modelo abierto (Qwen/DeepSeek-open-weights) en VPC. Costo fijo GPU reservada justifica volumen.
3. Simplifica contrato: 1-2 DPAs (Bedrock unifica) vs 3.

Veredicto cross-family:
- No `mmorch_ensemble_verify`. Investigación mercado. Consistencia 4 búsquedas: DPA Bedrock único, gap compliance DeepSeek hosteado, umbral costo self-hosting.
- Confianza 0.8: Pricing/compliance de terceros (blogs, guías). Confirmar docs oficiales AWS/Azure/Google antes de DPA.

Links:
- [[estudio-saas-plataforma-educativa]]
