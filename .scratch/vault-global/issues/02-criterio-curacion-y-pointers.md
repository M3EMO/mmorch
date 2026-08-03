# Criterio de curación vivo-vs-histórico + formato del pointer

Type: grilling
Status: resolved
Blocked by: 01

## Question

Con el inventario en mano: ¿qué regla decide si un doc migra al vault ("vivo")
o queda ("histórico")? ¿Y qué queda en el repo de origen — un stub markdown con
link al vault, una línea en el CLAUDE.md del proyecto, o nada? El criterio debe
ser aplicable por un agente sin juicio caso-a-caso.

## Answer

Grilling 2026-08-02, tres decisiones:

1. **Criterio = por TIPO de contenido** (aplicable por agente sin juicio caso a caso):
   - VIVO (migra a vault/research o carpeta temática): veredictos sobre libs/repos/técnicas,
     benchmarks/mediciones, design-rationale de cosas shipped, brainstorms-ancestro citados
     por docs operativos (AGENTS/GOAL/README).
   - HISTÓRICO: planes ya ejecutados, prompts/artefactos de generación, audits/roadmaps
     auto-generados, logs de progreso.
   - Para la migración inicial: usar la clasificación del inventario (ticket 01), que ya
     aplicó esta regla sobre los ~38 candidatos.

2. **Pointer = stub en el mismo path**: el archivo migrado se reemplaza por 3 líneas
   (movido al vault + path destino + fecha). Grep y links viejos siguen encontrando algo;
   una sola copia viva. Aplica YA a los 3 duplicados byte-idénticos (docs/intuition-layer,
   docs/fable-workflow, docs/paperclip-grafts → stub apuntando a vault/research/).

3. **Históricos: migran a vault/archive/** (todo en un lugar), también con stub en origen.
   El archive no entra en MOCs ni recall por default — es archivo, no capa curada.
