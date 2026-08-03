# Contrato de escritura al vault desde otros proyectos

Type: grilling
Status: resolved

## Question

¿Cómo ESCRIBE una sesión de Claude (o mmorch) parada en otro proyecto una nota
al vault? Opciones sobre la mesa: (a) tool MCP nuevo `mmorch_vault_write` que
envuelve `vault.write_note` (valida frontmatter: tag de proyecto obligatorio),
(b) path directo + convención documentada en CLAUDE.md global, (c) solo vía
memoria de Claude y un job que consolida al vault. Decidir también: ¿el ingest
babel corre en el momento de escribir o lo barre el nightly?

## Answer

Contrato decidido (grilling 2026-08-02, 4 decisiones):

1. **Puerta única**: tool MCP nuevo `mmorch_vault_write` que envuelve
   `vault.write_note`. Path directo queda para humanos/Obsidian; la validación
   vive solo en el tool. (Fact: el MCP server hoy no expone el vault.)
2. **Validación mínima dura**: obligatorios `title` + tag de proyecto;
   `created` lo autocompleta el tool. Resto (status/confidence/verifier/sources)
   opcional con defaults del template research-note — barrera baja para escribir,
   curado después.
3. **Babel async al escribir**: el tool dispara `babel.ingest()` en background
   (thread best-effort / job del server engine EXISTENTE — no cola nueva) y el
   nightly barre como red de seguridad: notas sin `.babel.md` o con hash del
   original cambiado. El write en sí es instantáneo.
4. **Colisiones = update semantics**: mismo slug pisa y devuelve `updated=true`
   (comportamiento actual de write_note, estilo Obsidian); git history del repo
   mmorch respalda lo pisado.

Implementación NO es de este mapa salvo el wiring (ticket 07, ahora desbloqueado);
el detalle de dónde corre el thread async va a la spec final.

## Answer

Grilling 2026-08-02 (sesión anterior, asentado hoy), tres decisiones:

1. **Puerta única = MCP tool nuevo `mmorch_vault_write`**: envuelve `vault.write_note`,
   disponible en cualquier sesión con el MCP mmorch conectado. Path directo queda solo
   para humanos/Obsidian; la validación vive en el tool.

2. **Validación = mínimo duro**: obligatorio title + tag de proyecto; `created` lo
   autocompleta el tool. status/confidence/sources opcionales con defaults del template
   research-note.md — barrera baja para escribir, curado después.

3. **Babel = async al escribir**: el write encola un job de `babel.ingest()` en el server
   mmorch (infra de jobs ya existe: server_engine) y responde al toque. El nightly queda
   como red de seguridad: barre notas sin .babel.md o con hash de original cambiado
   (refresh) — esto cubre también la política de refresh que estaba en la niebla.
