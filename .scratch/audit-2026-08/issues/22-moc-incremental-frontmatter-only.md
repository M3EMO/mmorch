# regenerate_moc relee todos los .md del vault en cada write

Type: task
Status: resolved
Severity: NICE-TO-HAVE
Effort: M
Eje: eficiencia
Evidence: mmorch/vault.py:99,117-150

Cada `write_validated` escanea todas las carpetas y hace read_text + parse de frontmatter
de todos los .md. O(vault) por write en una memoria diseñada para crecer.

**Fix:** leer solo el bloque frontmatter (hasta el 2do `---`) y/o actualizar el MOC
incrementalmente.

## Comments
Implementé la opción "frontmatter-only" (no el MOC incremental completo — mínimo diff):
`vault._read_frontmatter_only` lee línea por línea hasta el 2do `---` y corta, sin
`read_text()` del body completo. `regenerate_moc` la usa en vez de `_split_frontmatter`
sobre el archivo entero. Sigue siendo O(archivos del vault) por write, pero cada archivo
ahora cuesta O(tamaño del frontmatter) en vez de O(tamaño de la nota). Cubierto por la
suite existente de `tests/test_vault_write.py` (MOC sigue verde).
