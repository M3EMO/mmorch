# Contrato de escritura al vault desde otros proyectos

Type: grilling
Status: open

## Question

¿Cómo ESCRIBE una sesión de Claude (o mmorch) parada en otro proyecto una nota
al vault? Opciones sobre la mesa: (a) tool MCP nuevo `mmorch_vault_write` que
envuelve `vault.write_note` (valida frontmatter: tag de proyecto obligatorio),
(b) path directo + convención documentada en CLAUDE.md global, (c) solo vía
memoria de Claude y un job que consolida al vault. Decidir también: ¿el ingest
babel corre en el momento de escribir o lo barre el nightly?
