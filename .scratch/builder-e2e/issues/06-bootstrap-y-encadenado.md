# Bootstrap y encadenado automáticos
Type: task
Status: resolved
Blocked by: 
Map: ../map.md

## Question

¿Cómo arma el builder, sin manos, el commit inicial de un repo vacío y la cadena de módulos?

- Bootstrap: `src/tipos.ts` o su equivalente (contratos), `sdlc.toml`, configuración del lenguaje, `Dockerfile.sdlc` e imagen construida.
- Encadenado: cada módulo parte de la rama del módulo anterior, o de `main` tras un merge automático y verificado.
- Hoy lo hace a mano `research/04-modular/correr_modulos.py`; la aceptación es rearmar el prototipo de F2 desde una carpeta vacía con un solo comando.

## Answer

Resuelto (2026-10-15). Módulo nuevo `sdlc/builder.py`, con tests en `tests/test_builder.py`.

    python -m sdlc.builder <plan.json> <carpeta_vacia>

- El plan es la salida de la descomposición: archivos del bootstrap, módulos en orden (tarea, archivos, contrato) e integración (comandos en contenedor, con `{repo}` y `{base}`).
- `bootstrap()` nunca pisa una carpeta con contenido; hace el commit inicial y construye la imagen si hay `Dockerfile.sdlc` y `contenedor`.
- `construir()` pasa cada módulo por el SDLC y fusiona su rama en `main` antes del siguiente; corta en el primer módulo que no se construye.
- `integrar()` corre los comandos en contenedor; el código de salida decide. El driver del ticket 05 ahora sale con 1 si el veredicto es rojo.

Aceptación cumplida: un solo comando armó F2 desde una carpeta vacía (`~/Proyectos/F2-seo-calculadoras/prototipo-06`, plan en `research/06-bootstrap/f2.plan.json`).

| Paso | Resultado | Minutos |
|---|---|---|
| m1 parámetros | built (1 rechazo: trazabilidad-plan) | 1,8 |
| m2 cálculo | built (1 rechazo: G3-compile, resuelto por el coder) | 3,5 |
| m3 interfaz | built | 2,6 |
| `tsc -p tsconfig.build.json` | ok | — |
| driver contra 70 casos ocultos | ok, 6 checks verdes | — |

Reglas de la casa: `builder` entra a `_ENTRADAS` (CLI, como `sdlc`) y a las exclusiones del self-check (corre el SDLC real).
