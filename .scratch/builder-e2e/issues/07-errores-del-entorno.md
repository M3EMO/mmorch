# Errores del entorno contra errores del código
Type: task
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Cómo distingue la etapa 4 un error del entorno de un error del código, para no gastar vueltas del coder ni escaladas a Claude?

- Caso real del ticket 04: `TS2307 Cannot find module 'node:test'` (faltaban tipos en la imagen); el coder hizo 3 vueltas y Claude 2 escaladas en vano.
- `_toolchain_missing` (`sdlc/pipeline.py`) reconoce comandos ausentes, pero no módulos ni tipos ausentes del entorno.
- Medir con los casos guardados en `logs/sdlc/diag-cases/`: cuántas escaladas eran errores del entorno.

## Answer

Resuelto (2026-10-15). Cambio en `sdlc/pipeline.py` con tests en `tests/test_sdlc_entorno.py`; repetición real en `research/07-entorno/replay.py`.

Regla nueva (`_falta_del_entorno`, usada por `_toolchain_missing`):

- Del entorno: un builtin (`node:test`, tipos de Node) o un paquete DECLARADO en un manifiesto (package.json, pyproject, requirements, Cargo.toml, pom.xml) que no está instalado. Maven y cargo sin red cuentan siempre como entorno.
- Del código: un import relativo roto o un paquete que nadie declaró, porque el coder lo inventó.
- En modo contenedor, el prompt de entorno avisa que no hay red: Claude no instala; arregla la configuración o nombra qué agregar a la imagen.

Medición sobre el caso real (m1 de F2 con el bootstrap viejo):

| | Vueltas del coder | Llamadas a Claude en la etapa 4 | Resultado |
|---|---|---|---|
| Antes | 3 | 2 (escalera de compilación) | falla con el log crudo |
| Primer intento del arreglo | 0 | 1 (camino de entorno) | Claude intentó `npm install` sin red |
| Con el aviso de contenedor | 0 | 1 (camino de entorno) | Claude no cambia nada y nombra qué falta en la imagen |

Límite: n = 1 caso real. `logs/sdlc/diag-cases` de la laptop tiene un solo caso; los del escritorio no se revisaron.
