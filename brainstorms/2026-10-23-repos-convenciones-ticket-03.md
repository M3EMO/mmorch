# Repos-convenciones, ticket 03 (mecanismo de actualización): Brainstorm / Discovery Notes
Date: 2026-10-23 · Goal: decidir con qué mecanismo un repo adoptado converge a las convenciones vigentes (`.scratch/repos-convenciones/issues/03-mecanismo-de-actualizacion.md`, bd `orchestration-4u6`).

Contexto heredado del ticket 01 (resuelto): tabla única de costumbres con un chequeo determinista por fila; oráculo con reporte JSON (desvíos y avisos separados), exit code de tres estados y salida byte a byte estable.

## Summary / key decisions

- Test de aceptación aprobado con 15 puntos (P8). Ticket 03 resuelto.
- Arreglos seguros: lista cerrada (P7). Todo lo demás lo reporta `doctor` y lo decide el ticket 04.
- La tabla, `doctor` y `--upgrade` viven en `sdlc/` (por ejemplo `sdlc/convenciones.py`), junto a `init()` (P6).
- Costumbre retirada = desvío de `doctor`; `--upgrade` nunca borra archivos del usuario. `_cfg()` deja de leer `docs/sdlc/sdlc.toml` (P5).
- `--upgrade` nunca escribe claves con default en código; `doctor` marca solo la ausencia de claves sin default (`accept_cmd`, `files`) (P4).
- El pipeline corre `doctor` al inicio, guarda el reporte en `run-log.json` y sigue; ningún desvío de `doctor` frena una corrida (P3).
- `contract_version` se retira (decisión del ticket 11 de `sdlc-6-gates` sin implementar). `doctor` marca como aviso cada clave desconocida de `sdlc.toml` (P2).
- Mecanismo: opción D con convergencia. `python -m sdlc doctor <repo>` es el oráculo y nunca escribe; `python -m sdlc init --upgrade <repo>` aplica solo lo que `doctor` marca como seguro. El estado deseado es la tabla única del ticket 01; no hay lista de migraciones (P1).

## Q&A log

### P1 — Mecanismo
- Asked: ¿opción A (convergencia sola), B (migraciones versionadas), C (diagnóstico solo) o D (diagnóstico más convergencia)?
- Captured: el usuario pidió primero ver las opciones (Claude no las había listado). Después confirmó "D con convergencia".
- Decisión: `doctor` = oráculo del ticket 01, sin escrituras. `init --upgrade` aplica solo arreglos seguros. El `docs/sdlc/sdlc.toml` legacy entra como una fila más de la tabla.

### P2 — `contract_version`
- Asked: ¿A (agregar `contract_version`), B (retirarla; clave desconocida = aviso) o C (retirarla; clave desconocida = desvío)?
- Captured: el usuario confirma B.
- Decisión: sin `contract_version`. `doctor` marca como aviso cada clave de `sdlc.toml` fuera de la tabla. Motivo: una clave desconocida nombra el problema exacto, y el pipeline viejo la ignora sin romperse.

### P3 — Pipeline ante un repo desactualizado
- Asked: ¿A (rechazar con cualquier desvío), B (correr `doctor`, guardar el reporte en `run-log.json` y seguir) o C (ignorar `doctor`)?
- Captured: el usuario confirma B.
- Decisión: `doctor` al inicio de cada corrida, reporte en `run-log.json`, sin bloqueo. Los desvíos que rompen una corrida (por ejemplo, sin `accept_cmd`) ya los frena el pipeline hoy.

### P4 — Claves con default en código
- Asked: ¿A (escribir cada default), B (nunca escribirlos; `doctor` no marca su ausencia) o C (escribirlos comentados)?
- Captured: el usuario confirma B.
- Decisión: un default escrito queda congelado y no recibe mejoras del código; por eso `--upgrade` no lo escribe. El comentario de cabecera de `init()` sigue como referencia de claves opcionales.

### P5 — Costumbres retiradas
- Asked: ¿A (fusionar `docs/sdlc/sdlc.toml` en la raíz y borrarlo), B (desvío de `doctor`, sin tocarlo) o C (quitar la lectura en `_cfg()` más B)?
- Captured: el usuario confirma C.
- Decisión: se quita la lectura legacy de `_cfg()` (hoy pisa la raíz sin aviso; ningún repo sondeado tiene el archivo). Regla general: una costumbre retirada es un desvío de `doctor`, y `--upgrade` nunca borra archivos del usuario.

### P6 — Dónde vive el mecanismo
- Asked: ¿A (todo en `sdlc/`) o B (tabla y `doctor` en `mmorch/`, `--upgrade` en `sdlc/`)?
- Captured: el usuario confirma A.
- Decisión: las costumbres son el contrato del builder con cada repo; su dueño es `sdlc/`. `doctor` no mide con casos etiquetados, así que no entra en `orchestration-bpn`. Evita otro import entre paquetes.

### P7 — Qué arreglo es seguro
- Asked: ¿confirmas la lista cerrada de arreglos seguros? El usuario preguntó si faltaba algo.
- Captured: Claude corrigió su lista y agregó arreglos, una frontera y resguardos. El usuario confirmó la lista corregida.
- Decisión, arreglos seguros:
  - Crear un archivo que falta (por ejemplo una plantilla de `docs/sdlc/`).
  - Agregar una clave sin default que falta en `sdlc.toml`, insertada antes del primer encabezado de tabla (al final caería dentro de `[sprites]` u otra tabla). Sin valor adivinable (`accept_cmd`), la clave va comentada con ayuda, como en `init()`.
  - Agregar un patrón que falta al final de `.gitignore`.
  - Agregar un bloque con marca propia al final de un archivo, si la marca no existe.
  - Correr el comando de `orchestration-ior` cuando falta `.beads/`, recién con su test verde.
  - Actualizar una plantilla intacta y vieja, solo si el ticket 04 aprueba cómo detectar que nadie la editó.
- Frontera: `--upgrade` nunca crea `sdlc.toml`; un repo sin `sdlc.toml` no está adoptado (caso Adepor).
- Resguardos: escritura atómica; `tomllib` relee `sdlc.toml` y una lectura fallida deshace la escritura; `doctor` corre al final y el desvío arreglado desaparece sin desvíos nuevos.
- Modificar, mover o borrar contenido existente queda fuera de `--upgrade`.

### P8 — Test de aceptación
- Asked: ¿apruebas el test de 12 puntos? El usuario preguntó si faltaba algo.
- Captured: Claude corrigió el punto 2 (quedan desvíos de `.beads/` y de claves sin valor adivinable) y el punto 5 (un `.gitignore` sin salto final cambia la última línea). Agregó los puntos 13 a 15. El usuario aprobó.
- Decisión: test aprobado (ver el `## Answer` del ticket 03).

## Open flags (pending input)
- Actualizar una plantilla intacta y vieja depende del ticket 04 -> grilling del ticket 04.
- `.beads/` depende del comando de `orchestration-ior` -> sesión md-estructura.
