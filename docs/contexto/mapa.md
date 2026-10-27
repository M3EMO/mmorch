# Mapa wayfinder — Sistema nuevo de banco y builder, construido con ECC
Label: wayfinder:map · Creado: 2026-10-27 · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Destination

Propuesta del agente, a confirmar en el ticket 01 (HITL).

Un sistema nuevo, en un repo nuevo y con un nombre nuevo, reemplaza a mmorch y al SDLC. Lo construye ECC con Haiku 5.5. El mapa termina cuando se cumplen estas cuatro condiciones:

1. El sistema nuevo arma memora y brasa desde el bootstrap del banco, con los números de ECC con Sonnet: contrato 61/61 y 53/53, oráculo oculto 48/48 y 15/16.
2. Cero intervenciones humanas durante la construcción.
3. El sistema mide cada corrida con un oráculo, como hoy mide mmorch: contrato, oculto, minutos, cupo y USD.
4. Lo que mmorch tiene medido sigue valiendo, o se vuelve a medir. Ningún invariante de `GOAL.md` se pierde sin una decisión del usuario.

## Pedido del usuario (2026-10-27)

- Hacer el esquema del SDLC con ECC.
- Quizás armar de cero la idea de mmorch con otro nombre, porque "ya no es eso", y así el SDLC queda más completo.
- Cambiar Sonnet por Haiku 5.5 dentro de ECC.

## Notes

- **Postura previa del agente:** recomendó combinar en vez de reescribir. Motivos: el mérito medido de ECC fue de Sonnet, y en F1 Sonnet sin ECC empató con ECC. El usuario eligió reconstruir. El mapa sigue esa decisión y mide cada paso.
- **Haiku 5.5 en ECC no está medido.** El banco usó Sonnet. El ticket 03 lo mide primero, porque todo el mapa depende de que Haiku alcance.
- **Cómo fija ECC el modelo:**
  - De los 71 agentes, 61 tienen `model: sonnet`, 4 tienen `model: opus` y 6 tienen `model: haiku` en su frontmatter.
  - `--model haiku` cambia solo la sesión principal.
  - `CLAUDE_CODE_SUBAGENT_MODEL=haiku` es la perilla que documenta ECC. No está claro si pisa el frontmatter de cada agente.
- **ECC solo aconseja su proceso:** en brasa salteó su propio `/orch-build-mvp`. Para que Haiku siga el proceso, los gates tienen que ser hooks que bloqueen (ticket 06).
- **Los hooks de ECC filtran datos:** dos de ellos escriben comandos completos en `~/.claude/`. Toda corrida usa `ECC_DISABLED_HOOKS="post:bash:command-log-audit,post:bash:command-log-cost"`.
- **El nombre mmorch tiene mucho alcance:** aparece en unos 620 archivos `.py` del repo, en el servidor MCP `mmorch`, en 5 archivos de configuración del usuario, y en skills, hooks, memoria y tags del vault.
- **Invariantes vigentes (`GOAL.md`):**
  - Lo checkeable va a un oráculo.
  - El verificador razona y refuta.
  - Nada se auto-aplica sin rollback y `fitness()` verde.
  - El costo está acotado.
  - Ninguna complejidad entra sin una métrica que la justifique.

## Punto de partida medido (banco del 2026-10-27)

| Repo | Contrato | Oráculo oculto | Intervenciones | Costo |
|---|---|---|---|---|
| memora con ECC y Sonnet | 61/61 | 48/48 | 0 | ~US$5,6 cupo |
| brasa con ECC y Sonnet | 53/53 | 15/16 | 0 | ~US$6,8 cupo |
| memora con el SDLC | 21/61 (2 de 9 módulos) | 20/48 | 4 | US$2,92 API |
| brasa con el SDLC | sin medir (3 de 10 módulos, sigue) | sin medir | 5 | US$3,37 API |

Fuentes:

- Banco: `~/Proyectos/bench-ecc-sdlc/` (specs, bootstrap, planes, logs, `logs/intervenciones.log`).
- Oráculos y resultados: `.scratch/ecc-vs-sdlc/oraculo/`.
- Prueba previa con control: `vault/research/ecc-affaan-mecc-veredicto-prueba-real-recreando-el-generador.md`.

## Fog

- Si Haiku 5.5 con ECC llega a los números de Sonnet, o si el sistema necesita Sonnet en algunos pasos.
- Qué parte de mmorch es medición que vale conservar y qué parte es andamiaje que se puede tirar.
- Si el sistema nuevo es un plugin de ECC, un paquete propio que usa ECC, o las dos cosas.
- Cuánto cupo cuesta construir el sistema entero con ECC.
- Cómo convive el sistema nuevo con mmorch mientras dura la migración.

## Decisions so far

<!-- una línea por ticket resuelto: gist + link -->
