# Prototipo: MOC por proyecto en Obsidian

Type: prototype
Status: resolved

## Question

¿Cómo se ve y navega un MOC (map of content) por proyecto sobre el contenido
ACTUAL del vault? Prototipar con /prototype: un `vault/moc/<proyecto>.md` por
proyecto activo con wikilinks a sus notas (vía tag de frontmatter), para
reaccionar sobre algo concreto antes de fijar la convención en la spec.
Incluye decidir si el MOC se genera (script) o se mantiene a mano.

## Answer

Prototipo corrido sobre el vault REAL (asset: `vault/moc/_gen_moc_PROTOTYPE.py`,
outputs `vault/moc/mmorch.md` y `sin-proyecto.md` quedan como muestra). Decisiones:

1. **Formato validado tal cual**: secciones por carpeta, línea = `[[wikilink]] — status · conf · babel ✓`.
2. **Generado AL ESCRIBIR**: `mmorch_vault_write` regenera el MOC del proyecto en cada
   write (no nightly, no a mano). El script limpio se integra al flujo del tool.
3. **Infra excluida**: README/lexicon/templates fuera del MOC (y archive/ también, por el 02).

Hallazgo del prototipo (alimenta la spec): 6/10 notas actuales caen en "sin-proyecto" —
las ingresadas por babel.ingest no llevan tags. Refuerza la validación mínima-dura del 03
(tag de proyecto obligatorio) y agrega tarea de spec: backfillear tags de las notas existentes.
