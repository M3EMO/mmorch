---
title: "Vault y compresor: supersesión en el vault (grilling)"
created: 2026-11-26
tags: [brainstorm, orchestration, vault-y-compresor]
status: seed
---
# Supersesión en el vault: notas del grilling
Fecha: 2026-11-26 · Objetivo: resolver el ticket "Supersesión en el vault" (`.scratch/vault-y-compresor/issues/07-supersesion-en-el-vault.md`).

## Contexto leído antes de preguntar

- El destino pide un test determinista de pares que pase en el 100%. El recall y los MOC nunca devuelven una nota reemplazada.
- El research 04 recomienda un registro determinista: `valid_from`, `valid_to`, `superseded_by` y una clave por hecho.
- El research 05 mide 0 llamadas a `mmorch_recall` en un mes. Las sesiones leen el vault con Read, cat y grep.
- El vault tiene 184 notas con frontmatter fuera de `moc/`. 138 notas tienen `status: seed`.
- Ninguna nota tiene hoy `superseded_by`, `valid_to` ni `supersedes`.
- `write_research_note` puentea un gist a duckdb con el texto "[proyecto] título — nota del vault en <path>".
- `memory.recall` ya excluye filas con `tombstone` o `needs_review`. El fallback episódico (FIX B) devuelve el episodio crudo sin filtro.

## Resumen y decisiones

- La spec construye dos módulos apagables: `supersedes` explícito al escribir y atado de una nota a un módulo de código.
- Vencimiento, clave por hecho y chequeo LLM quedan como módulos futuros, sin código.
- `supersedes` mueve la nota vieja a `vault/archive/`, le pone `superseded_by` y la línea "Reemplazada por [[nueva]]" al tope.
- `mmorch_vault_write` aplica `supersedes`, y `regenerate_moc` completa el que falte de una edición a mano. Una nota inexistente da error.
- El atado a código no oculta la nota. Un commit posterior en el módulo la marca "a revisar" en recall y MOC, calculado con `git log`.
- El recall nunca devuelve una nota reemplazada ni archivada, tampoco por el gist en duckdb ni por el fallback episódico.
- El MOC lista una nota reemplazada solo en "Reemplazadas", con link a la nueva. Una seed archivada sale en "Archivadas", sin link.
- Las notas viejas no se migran. Una seed sin lectura por nombre en los transcripts de los últimos 60 días va a `archive/`.
- Esa regla cuenta solo seeds con `created` de más de 60 días, y no corre con menos de 60 días de transcripts (hoy, hasta el 2026-12-11).
- El archivo de seeds es un comando a mano: lista las candidatas y mueve solo con `--aplicar`.
- Quien escribe la spec elige el nombre del campo del atado a código. En una cadena A→B→C, cada nota linkea a su sucesora directa.

## Registro de preguntas

### P1 — Quién declara el reemplazo
- Pregunta: quién declara que una nota nueva reemplaza a una vieja. Opciones: declaración explícita, clave por hecho, clave más chequeo LLM, solo fecha de vencimiento.
- Respuesta textual: "1 + Por modulos, asi puede ser mas versatil".
- Captura: el usuario elige la declaración explícita al escribir (`supersedes`). Agrega "por módulos" para ganar versatilidad.
- Flag: "por módulos" pide aclaración (P1b) -> resuelto en P1b.

### P1b — Qué significa "por módulos"
- Pregunta: tres lecturas. Un punto de enchufe sin código extra, varios módulos apagables construidos ya, o notas atadas a un módulo de código del repo.
- Respuesta textual: "3 y 2".
- Captura: la spec construye ya varios mecanismos, cada uno como módulo apagable (lectura 2). Uno de ellos ata una nota a un módulo de código del repo; un cambio en ese módulo marca la nota para revisión (lectura 3).
- Captura: la declaración explícita `supersedes` de P1 sigue elegida.
- Flag: el conjunto exacto de módulos de hoy queda para P2 -> usuario.

### P2 — Qué módulos se construyen ahora
- Pregunta: además de `supersedes` y el atado a código, ¿suma vencimiento, clave por hecho o chequeo LLM?
- Respuesta: "Solo esos dos (Recommended)".
- Captura: la spec construye dos módulos apagables, `supersedes` explícito y atado a código.
- Captura: vencimiento, clave por hecho y chequeo LLM quedan como módulos futuros, sin código.

### P3 — Qué le pasa a una nota atada cuando su módulo cambia
- Pregunta: un commit nuevo toca el módulo de código atado a la nota. ¿La nota queda visible con marca, se oculta hasta revisar o solo aparece en un reporte?
- Respuesta: "Visible con marca (Recommended)".
- Captura: la nota sigue en el recall y los MOC con la marca "a revisar".
- Captura: un commit no prueba que la nota sea falsa, así que la marca no cuenta como reemplazo. Solo `supersedes` oculta una nota.
- Captura: un humano o una nota nueva con `supersedes` cierra la marca.

### P4 — Qué queda visible de una nota reemplazada
- Pregunta: archivo con aviso en su carpeta, mover a `archive/`, solo frontmatter, o MOC con sección "Reemplazadas".
- Respuesta textual: "2 y 4".
- Captura: el tool mueve la nota vieja a `vault/archive/`. Un grep sobre `research/` deja de verla.
- Captura: el MOC lista la nota vieja en una sección "Reemplazadas", con link a la nota nueva.
- Captura inferida: la opción 4 incluía la línea "Reemplazada por [[nueva]]" al tope del cuerpo. Confirmar en P5.
- Hecho del código: `regenerate_moc` excluye hoy la carpeta `archive/`. La sección "Reemplazadas" pide leer `archive/` también.
- Hecho: Obsidian resuelve `[[nombre]]` por nombre de archivo, sin carpeta. Un link viejo sigue vivo si el nombre no choca con otra nota.
- Conflicto: el destino dice "los MOC nunca devuelven una nota reemplazada". La sección "Reemplazadas" la lista. Resuelto en P5.

### P5 — Definición del test de pares
- Pregunta: cómo convive el test del destino con la sección "Reemplazadas", y si el archivo archivado lleva aviso.
- Respuesta: "Sí, así (Recommended)".
- Captura: una nota reemplazada nunca sale en el recall. El recall cubre el gist en duckdb y el episodio crudo del fallback.
- Captura: una nota reemplazada nunca sale en el MOC fuera de la sección "Reemplazadas".
- Captura: en "Reemplazadas", la nota siempre sale con link a su nota nueva.
- Captura: el archivo movido a `archive/` lleva la línea "Reemplazada por [[nueva]]" al tope del cuerpo.

### P6 — Qué pasa con las notas viejas
- Pregunta: sin migración, atar módulos viejos, pasada de supersesión o archivar seeds viejas.
- Respuesta textual: "1 + 4".
- Captura: ninguna pasada retroactiva de supersesión ni de atado a código. La supersesión corre hacia adelante.
- Captura: las seeds sin lecturas en N días van a `archive/`, sin nota nueva que las reemplace.
- Hecho: el conteo real es 138 notas con `status: seed`; el mapa dice 126.
- Hecho: el vault no guarda un log de lecturas. El research 05 contó lecturas escaneando transcripts.
- Flag: el valor de N y la fuente de lecturas quedan para P7 -> usuario.

### P7 — Regla de archivo de seeds
- Pregunta: ventana sin lecturas de 90, 60 o 180 días, o un log de lecturas nuevo.
- Respuesta: "60 días".
- Captura: una seed sin lectura por nombre de archivo en los transcripts durante 60 días va a `archive/`.
- Captura: la fuente de lecturas son los transcripts (Read, cat, sed, grep que nombran la nota). Un glob o regex no cuenta.
- Hecho medido hoy: el transcript más viejo es del 2026-10-12, unos 45 días de historia.
- Hecho medido hoy: las seeds por mes de `created` son 20 de agosto, 17 de septiembre, 42 de octubre y 59 de noviembre. Una seed no tiene `created`.
- Problema: una ventana contada desde `created` cae antes del 2026-10-12 para 37 seeds. Esas seeds no tienen historia de lecturas. Resolver en P7b.

### P7b — Ventana de 60 días frente a 45 días de historia
- Pregunta: ventana móvil con piso de cobertura, ventana móvil con la historia que haya, o ventana fija desde `created`.
- Respuesta: "Últimos 60 días (Recommended)".
- Captura: la ventana son los últimos 60 días antes de la corrida.
- Captura: solo cuentan seeds con `created` de más de 60 días.
- Captura: la regla se niega a correr si los transcripts cubren menos de 60 días. Hoy eso la frena hasta el 2026-12-11.

### P8 — Cómo se ve una seed archivada
- Pregunta: sección "Archivadas" en el MOC, fuera del MOC, o visible en el recall.
- Respuesta: "Sección \"Archivadas\" (Recommended)".
- Captura: el recall excluye una seed archivada igual que a una reemplazada.
- Captura: el MOC la lista en una sección "Archivadas", separada de "Reemplazadas" y sin link a sucesora.
- Captura: volverla atrás es moverla a su carpeta a mano.

### P9 — Cuándo corre el archivo de seeds
- Pregunta: comando a mano, paso nocturno o al escribir una nota.
- Respuesta: "Comando a mano (Recommended)".
- Captura: un comando lista las seeds candidatas y las mueve solo con `--aplicar`.
- Captura: la marca "a revisar" del atado a código sale de `git log` al regenerar el MOC, sin estado guardado.
- Hecho: `mmorch/nightly.py` nombra el Task Scheduler de Windows, que ya no existe. No hay timer de systemd ni cron que lo corra.

### P10 — Escrituras a mano
- Pregunta: solo el tool aplica `supersedes`, el tool y la regeneración del MOC, o solo el comando de P9.
- Respuesta: "Tool + MOC completan (Recommended)".
- Captura: `mmorch_vault_write` recibe `supersedes` y lo aplica al escribir.
- Captura: `regenerate_moc` lee `supersedes` del frontmatter de cualquier nota y completa lo que falte: mover, aviso, `superseded_by` y gist en duckdb.
- Captura: un `supersedes` que nombra una nota inexistente da error.
- Hecho: el research 05 midió 67 escrituras con el tool y 25 con Edit o Write en un mes.

### P11 — Cierre
- Pregunta: ¿queda algo sin tocar? El nombre del campo del atado y las cadenas quedan a quien escribe la spec.
- Respuesta: "Nada más, cerrá".

## Flags abiertos

- Ninguno. El mapa dice 126 seeds y el conteo de hoy da 138; el mapa se corrige al cerrar el ticket.
