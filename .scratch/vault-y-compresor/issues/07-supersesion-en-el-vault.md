# Supersesión en el vault
Type: grilling
Status: resolved
Blocked by: 04, 05
Map: ../map.md

## Question

Decidí con el usuario cómo una nota nueva invalida a una vieja en el vault. Las piezas posibles son un campo `superseded_by`, una fecha de vencimiento y un chequeo de contradicción al escribir. Decidí también qué ve el recall de una nota invalidada.

## Answer

Resuelto con el usuario el 2026-11-26 en un grilling. Captura completa: `vault/brainstorms/2026-11-26-supersesion-en-el-vault.md`.

- La spec construye dos módulos apagables: `supersedes` explícito al escribir y atado de una nota a un módulo de código.
- Vencimiento, clave por hecho y chequeo LLM quedan como módulos futuros, sin código.
- `supersedes` mueve la nota vieja a `vault/archive/`, le pone `superseded_by` y la línea "Reemplazada por [[nueva]]" al tope.
- `mmorch_vault_write` aplica `supersedes`, y `regenerate_moc` completa el que falte de una edición a mano. Una nota inexistente da error.
- El atado a código no oculta la nota. Un commit posterior en el módulo la marca "a revisar" en recall y MOC, calculado con `git log`.
- El recall nunca devuelve una nota reemplazada ni archivada. Eso cubre el gist en duckdb y el fallback episódico.
- El MOC lista una nota reemplazada solo en "Reemplazadas", con link a la nueva. Una seed archivada sale en "Archivadas", sin link.
- El test de pares chequea esas tres cosas: recall, MOC fuera de "Reemplazadas" y link a la nueva.
- Las notas viejas no se migran. Una seed sin lectura por nombre en los transcripts de los últimos 60 días va a `archive/`.
- Esa regla cuenta solo seeds con `created` de más de 60 días. No corre con menos de 60 días de transcripts (hoy, hasta el 2026-12-11).
- El archivo de seeds es un comando a mano: lista las candidatas y mueve solo con `--aplicar`.
