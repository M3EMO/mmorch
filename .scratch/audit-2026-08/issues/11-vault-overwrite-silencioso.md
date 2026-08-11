# vault.write_note sobreescribe notas en silencio ante colisión de slug (fuente de verdad)

Type: task
Status: resolved
Severity: IMPORTANTE
Effort: M
Eje: robustez
Evidence: mmorch/vault.py:16-18,31-33,81-114 · mcp_server.py:366

`_slug` normaliza y trunca a 60 chars (colisiones deterministas); `write_note` escribe
sin chequear existencia; `write_validated` delega sin protección y `log_op` registra
"write" indistinguible de overwrite. Dos títulos que colisionan, o re-usar un título,
pierde el contenido anterior sin backup ni warning. Git mitiga solo si se commitea
seguido; el write no commitea.

**Fix:** si el path existe con contenido distinto → sufijo `-2`/`-3` o error explícito +
`log_op("overwrite", ...)`.

## Comments
`write_note` ahora compara contenido antes de escribir: mismo path + mismo contenido =
re-write idempotente (reusa el path); mismo path + contenido DISTINTO = busca el próximo
sufijo libre `-2`, `-3`, ... y loguea `log_op("overwrite_avoided", ...)`. Nunca pisa una
nota existente con contenido distinto en silencio. Tests:
`tests/test_vault_write.py::test_colision_de_slug_no_pisa_la_nota_vieja` y
`::test_mismo_titulo_mismo_contenido_reusa_el_path`.
