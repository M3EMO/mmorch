# SDD fase 2, ticket 09 (propiedades sin duplicar gate_mutacion): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir qué aporta un test de propiedades que `gate_mutacion` no mida ya, y cómo se mide ese aporte sin un LLM. Ticket: `.scratch/sdd-fase-2/issues/09-propiedades-y-mutacion.md`.

Insumos ya decididos: las propiedades nacen en la etapa 1, entran a la aceptación y pueden pasar en HEAD (ticket 08); una marca excesiva se atrapa acá, porque una propiedad vacua no mata mutantes (ticket 07). Hecho del código: `gate_mutacion` muta solo las líneas cambiadas, corre la aceptación entera por mutante y, sin `mutation_min`, prueba una muestra de 5 y observa.

## Summary / key decisions

- Aporte por test sobre las corridas que ya existen (opción B): el run-log guarda qué test mató cada mutante; una propiedad que no mata ninguno queda marcada como vacua. Sin corridas extra (Q1).
- La marca de vacua bloquea solo con `mutation_min`; sin él, queda anotada (Q2).
- Dentro de los mutantes, Hypothesis corre un perfil con 20 ejemplos; la aceptación normal, el defecto (Q3).
- Dentro de los mutantes, pytest corre sin `-x`; test aprobado con once casos (Q4). Ticket 09 resuelto.

## Q&A log

### Q1 — Cómo medir el aporte de las propiedades
- Asked: ¿solo el puntaje global (A), leer qué test falló en cada corrida existente y marcar la propiedad vacua (B) o correr cada propiedad sola contra los mutantes de su requisito (C)?
- Captured: el usuario elige **B**. pytest informa qué test falló en cada corrida; el número de corridas no crece. C multiplicaba corridas (`orchestration-cbr`: 10 a 30 min por la etapa). A no detectaba la propiedad vacua.
- Flags: si la marca de vacua bloquea u observa (Q2).

### Q2 — La marca de vacua bloquea u observa
- Asked: ¿solo anota (A), bloquea con `mutation_min` y anota sin él (B) o bloquea siempre (C)?
- Captured: el usuario elige **B**. Misma regla que `gate_mutacion`: sin número medido, observa. Con `mutation_min` corren todos los mutantes y una propiedad sin muertes sí es vacua. C bloquearía por el azar de una muestra de 5.
- Flags: cuántos ejemplos corre Hypothesis dentro de cada mutante (Q3).

### Relaciones metamórficas de `builder-e2e` (resuelto por hechos, sin pregunta)
- Viven en el driver de producto (Playwright, `.scratch/builder-e2e/research/05-driver/`), fuera de la aceptación del SDLC. No cuentan como propiedades de este mapa.

### Q3 — Ejemplos de Hypothesis dentro de cada mutante
- Asked: ¿defecto también en mutantes (A), número menor solo dentro de los mutantes con un perfil y defecto en la aceptación normal (B) o una clave de `sdlc.toml` igual para todo (C)?
- Captured: el usuario elige **B**. Valor inicial 20 dentro de los mutantes; la aceptación normal conserva el defecto. El número exacto se mide en la corrida del ticket 11. El mecanismo del perfil pertenece al ticket 10.
- Flags: test de aceptación (Q4).

### Q4 — Test de aceptación
- Asked: ¿aprobás el test con nueve casos? Después: ¿falta algo?
- Hallazgo de Claude en `sdlc/pipeline.py`: `_accept_mata()` corre pytest con `-x`; si el test de ejemplo falla primero, la propiedad no corre contra ese mutante y quedaría marcada vacua sin serlo. Arreglo: correr sin `-x` dentro de los mutantes.
- Captured: el usuario aprueba correr sin `-x` y once casos: los nueve más un mutante que mata primero el de ejemplo igual registra el resultado de la propiedad, y con `accept_cmd` de otro lenguaje el run-log avisa que no hay atribución por test y el gate queda como hoy (alcance de lenguajes: ticket 10).
- Flags: ninguno. Ticket 09 resuelto.

## Open flags (pending input)
