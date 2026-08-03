# Charts flint: qué métricas entran a la spec

Type: grilling
Status: resolved

## Question

flint-chart-mcp ya está registrado. ¿Qué charts valen la pena en la spec v1 —
p.ej. ratio/fidelidad de babels en el tiempo, notas por proyecto, growth del
vault, curva del nightly (logs/nightly.jsonl)? ¿Y dónde viven: notas del vault
con el chart embebido, o dashboard del server mmorch? Alcance chico: 2-3 charts
que se miren de verdad, no una galería.

## Answer

Grilling 2026-08-03:

**Charts v1 (3, generados por el nightly como SVG vía flint):**
1. **Adopción del vault** — notas por proyecto en el tiempo (mide el objetivo del mapa).
2. **Babel paga o no** — ratio/fidelidad por babel + % skipeados por gate.
3. **Costo API mensual** — gasto por proveedor desde metrics.jsonl.

**Dónde:** SVG en `vault/charts/` + nota que los embebe — visibles en Obsidian,
viajan por el mismo sync del vault. Sin panel de server nuevo.

**Además (pedido explícito):** flint queda como herramienta AD-HOC de cualquier
sesión — el MCP flint-chart ya registrado se usa para graficar resultados on demand
(benchmarks, corridas, lo que se esté mirando), no solo los 3 charts fijos. La spec
debe mencionar esta convención de uso.
