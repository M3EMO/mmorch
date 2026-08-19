---
title: Docling completo bloqueado por RAM — pypdfium2 acoplado hoy
status: applied
tags: [research, mmorch, docling, hardware, repo-mining]
created: 2026-08-19
---

Candidata: acoplar Docling (github.com/docling-project/docling) para
enriquecer el contexto que ve el juez de `repo_mining` con documentos, no
solo README/código.

## Medido en vivo, no supuesto

`docling` estándar (aunque se seleccione el extra liviano
`format-pdf-pypdfium2`) importa sin `torch`, pero **revienta en runtime**: el
layout model hace `import torch` recién al inicializarse
(`ModuleNotFoundError` reproducido con el PDF real de Estudio). No existe
pipeline liviano para PDF en esta versión — `SimplePipeline` solo sirve para
formatos declarativos (DOCX/HTML), PDF necesita sí o sí el pipeline con
modelos de layout.

`torch` solo (wheel) pesa 527MB en disco, más pesos de modelos aparte —
choca directo con la limitante de hardware ya documentada (memoria
`hardware-plan`: 8GB RAM = cuello, esperando ExpertBook 64GB).

## Decisión

Docling completo (layout + tablas + reading-order multi-columna) queda
**bloqueado hasta la mejora de hardware** — candidata, no descartada.

Hoy se acopló la mitad liviana: `mmorch/docs_extract.py` usa `pypdfium2`
solo (4MB, sin ML) para texto plano de PDFs. Sin estructura de tablas, sin
reading-order — pero mejor que la nada de hoy (ningún PDF entraba al
contexto del juez de `repo_mining`). Interfaz estable a propósito
(`extract_text(path) -> str`): cuando el hardware lo permita, el upgrade es
cambiar el CUERPO de esa función, no los call-sites.

Verificado con el mismo PDF real (Administración General, 2do parcial):
extracción correcta (`organización` con acento, confirmado por codepoint —
lo que se veía roto era solo la consola, no el dato).

## Consecuencias

- `repo_mining._collect_context()` ahora incluye hasta 3 PDFs sueltos de un
  repo minado (whitepaper, docs/architecture.pdf), 4000 chars cada uno.
- Dependencia declarada en `pyproject.toml` bajo el extra `docs`
  (`pip install -e ".[docs]"`), no en el core — mismo patrón que `memory`
  (fastembed) y `checkers`.
- Si se retoma Docling completo mas adelante: NO reinstalar `docling` a
  secas (trae `docling-ibm-models`+torch+torchvision+accelerate aunque se
  pida el extra liviano) — usar `docling-slim` explícito con los extras
  puntuales, y aceptar el costo de RAM a sabiendas.
