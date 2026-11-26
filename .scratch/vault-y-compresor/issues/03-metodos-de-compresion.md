# Métodos de compresión con piso de fidelidad
Type: research
Status: resolved
Blocked by: 
Map: ../map.md

## Question

Listá los métodos de compresión de texto para prompts que dejan el archivo más chico con un piso de fidelidad medible. Cubrí LLMLingua-2, Selective Context, SkillReducer, borrado de tramos y reescritura por LLM. Para cada método, anotá el ahorro medido, la pérdida medida, si corre local o por API y si necesita conocer la pregunta.

## Answer

| Método | Ahorro | Pérdida | Local/API | Query-aware |
|---|---|---|---|---|
| LLMLingua-2 | 2x-5x tokens | sin cifra en abstract; fragmenta el texto; 99.5% a 77.3% con GPT-3.5 en un caso de código | local (XLM-R) | no |
| Selective Context | -50% costo de contexto | BERTscore -0.023, faithfulness -0.038 | local (LM chico) | no |
| LongLLMLingua | 2x-6x; costo -94% en LooGLE | accuracy +21.4% en NQ (contexto largo) | local | sí |
| SkillReducer | -48% descripciones, -39% cuerpos | calidad +2.8%, retención 0.965 | LLM por API (inferido) | no |
| Borrado de oraciones | hasta -69.7% (FROST, trazas de razonamiento) | accuracy +26.7% en ese caso; tokens sueltos suben alucinaciones | local | según el método |
| Reescritura por LLM | -34% a -47% (Llama 3B local, 2606.03618) | "preserva", sin cifra | API o local | posible |

Todas las cifras vienen de abstracts o páginas de Lacuna, ninguna de una re-corrida propia. Ninguna se midió en español.

Recomendación, 3 candidatos:
1. SkillReducer (o su recetario): único con piso medido sobre skills y salida legible. Aplica a descripciones y cuerpos propios.
2. Reescritura por LLM con oráculo propio: el piso lo fija un checker con el contador real de Anthropic, no un juez LLM.
3. Borrado de oraciones query-aware para notas del vault, solo al recuperar con pregunta, sin tocar el archivo fuente.

Descartados: LLMLingua-2 y Selective Context dejan texto ilegible. CAVEWOMAN mide que comprimir la entrada telegráfica sube el costo neto (~1.15x), y el caché penaliza cambiar el prefijo.

Nota del vault: "Métodos de compresión de prompts con piso de fidelidad (2026-11-26)".
