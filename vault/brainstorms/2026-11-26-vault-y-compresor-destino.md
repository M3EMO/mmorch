---
title: "Vault y compresor: destino del mapa (grilling)"
created: 2026-11-26
tags: [brainstorm, orchestration, vault-y-compresor]
status: seed
---
# Vault y compresor: notas del grilling de destino
Fecha: 2026-11-26 · Objetivo: resolver el ticket "Destino y hogar del compresor" (`.scratch/vault-y-compresor/issues/01-destino-y-hogar.md`).

## Resumen y decisiones

- El mapa termina en una spec, y ECC la construye (no `/project`).
- La supersesión vive en el vault de este repo. El compresor vive en Root.
- El compresor cubre solo el arranque. El compresor de notas queda fuera de alcance.
- El compresor está listo con un arranque de 55k tokens o menos, sobre el piso de fidelidad (corregido; ver P4b).
- La supersesión está lista cuando el recall nunca devuelve una nota reemplazada, en el 100% de un test de pares.

## Registro de preguntas

### P1 — En qué termina el mapa
- Pregunta: ¿spec para `/project` o cambio construido dentro del mapa?
- Respuesta del usuario: "Spec para ECC".
- Captura: el mapa termina en una spec. La construcción corre fuera del mapa con el pipeline de ECC. Esto sigue la línea del mapa `.scratch/sdlc-con-ecc/` (ECC como builder).
- Pendiente: qué flujo de ECC recibe la spec (por ejemplo `ecc:plan-prd` o `ecc:feature-dev`) → se decide al escribir la spec.

### P2 — Dónde vive cada pieza
- Pregunta: ¿supersesión en el vault de este repo y compresor en Root, o todo a un repo nuevo?
- Respuesta del usuario: "Sí, así".
- Captura: la supersesión queda en `mmorch/vault.py` de este repo, como mantenimiento. El compresor va a Root, como pieza nueva. Mudar el vault queda fuera del mapa.

### P3 — ¿El compresor de notas sigue en el destino?
- Pregunta: el research midió ~21k tokens por mes en lecturas del vault (~0.009% del input). ¿Sacamos el compresor de notas?
- Respuesta del usuario: "Sí, sacarlo".
- Captura: el compresor de notas pasa a Out of scope, porque ahorra disco y no cupo. El compresor cubre solo el contexto de arranque. El ticket "Compresor de notas" se cierra como fuera de alcance.

### P4 — Qué número marca "listo" para el compresor
- Pregunta: ¿"listo" es un arranque de 52k tokens o menos (hoy 66.8k de mediana)?
- Respuesta del usuario: "Sí, 52k".
- Captura: el compresor está listo cuando el arranque medido baja a 52k tokens o menos, con el piso de fidelidad del ticket "Oráculo de fidelidad del compresor". La medición usa el `usage` real del primer mensaje en transcripts.

### P4b — Corrección de la meta (2026-11-26, después del ticket 09)
- Contexto: el ticket 09 estimó ~54.7k. La meta de 52k pedía recortar ~79% de los `.md`, y SkillReducer midió -39% a -48%.
- Respuesta del usuario: "Sí, 55k".
- Captura: la meta del compresor pasa a 55k tokens o menos.

### P5 — Qué marca "listo" para la supersesión
- Pregunta: ¿"listo" es que el recall nunca devuelva una nota reemplazada?
- Respuesta del usuario: "Si, asi."
- Captura: la supersesión está lista cuando un test determinista con pares de notas (vieja y nueva) pasa en el 100% de los pares. El recall y los MOC muestran solo la nota vigente. Cómo se detecta el reemplazo lo decide el ticket "Supersesión en el vault".

## Pendientes
