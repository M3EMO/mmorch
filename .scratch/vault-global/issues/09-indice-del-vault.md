# Índice del vault: ¿duckdb, índice propio, o bridge?

Type: grilling
Status: resolved

## Question

¿Cómo encuentra el recall semántico las notas del vault? Hoy son DOS stores:
memory.duckdb (notas semánticas + embeddings, recall ya rankea) y el vault
(archivos markdown, navegable por MOC/Obsidian pero invisible al recall).

## Answer

Grilling 2026-08-03: **Bridge a duckdb** — `write_validated` hace además
`remember(gist + path)` a memory.duckdb scope global. El recall existente (ranking +
MMR) encuentra el gist y la sesión lee la nota completa (o su babel) por path.
Un solo índice, cero infra nueva. La migración inicial también bridgea las notas
existentes del vault (backfill de gists).
