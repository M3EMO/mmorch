# Mapa wayfinder — Builder de punta a punta con oráculo agente
Label: wayfinder:map · Creado: 2026-10-13 · Tickets en `issues/` · Tracker: local-markdown (`docs/agents/issue-tracker.md`)

## Destination

Fijado por el ticket 01 (2026-10-15). El builder es una herramienta GENERAL: arma entregables (productos, servicios, flujos, funcionalidades) y cada uno queda medido como banco de mmorch. El mapa termina cuando el builder arma de punta a punta la calculadora de sueldo neto de F2 con cinco condiciones:

1. La capa computable queda verde: casos de neto, agente que maneja el preview, consola sin errores, Lighthouse y metadatos.
2. El usuario interviene solo en dos paradas (walkthrough y preview final), con menos de 1 h en total.
3. El gasto de APIs queda debajo de US$5.
4. El run-log registra coder sorteado, fricción, USD, cupo y horas del usuario.
5. El juez en sombra deja su opinión con rúbricas, sin bloquear.

Detalle completo: `brainstorms/2026-10-14-builder-e2e-destino.md`.

## Estado del destino (2026-10-16)

Corrida final tras los seis oks del walkthrough: `~/Proyectos/F2-seo-calculadoras/f2-sueldo-neto` (repo local).

| Condición | Estado |
|---|---|
| 1. Capa computable verde | Cumplida: 3 módulos built, integración verde (casos ocultos, metamórficas, consola, red, axe, Lighthouse, metadatos). |
| 2. Dos paradas y menos de 1 h del usuario | Parada 1 hecha (walkthrough). Parada 2 pendiente (preview). Las horas no se miden: `human_interventions` sigue en 0. |
| 3. APIs < US$5 | Cumplida: ~US$0,17 de DeepSeek; planner Sonnet ~US$0,16 equiv. de cupo; Haiku US$0,002. |
| 4. Run-log completo | Parcial: coder sorteado, rechazos y cupo sí; USD por run no (`orchestration-gkw`); horas del usuario no. |
| 5. Juez en sombra con rúbricas | Texto sí: prefiere la referencia prototipo-04 en los dos órdenes (desglose sin etiquetas en el final). Visual no: GLM sin saldo. |

Hallazgos de la corrida final:

- Bug del SDLC arreglado: `_LANG_NAMES` no conocía `.html`, así que el coder recibía `index.html` como "programador TypeScript" y escribía TypeScript adentro. Primer intento fallido guardado en `~/Proyectos/F2-seo-calculadoras/.fallidas/`.
- El juez detectó un defecto real que la capa computable no exigía: el desglose sin etiquetas. Ya es computable: `test_R5_desglose_con_etiquetas` en el contrato del módulo 3 pasa en prototipo-04 y falla en el sitio final, igual que la opinión del juez.

## Notes

- Origen: revisión de `rehan-remade/universal-modder` (2026-10-13). Su ciclo es recon → ruta → laboratorio → rebanada vertical → assets → verificación en el juego real → nota de campo. Veredicto de la revisión: no sirve para mmorch como código; sirve como patrón.
- Pedido del usuario: un builder de punta a punta, no solo con tests computables, capaz de armar desde cero de forma modular.
- Invariante que manda (GOAL.md, CLAUDE.md global): lo checkeable va a un oráculo, nunca a un LLM. El "oráculo agente" se parte en dos roles:
  - El agente maneja el producto y junta evidencia: capturas, logs, respuestas HTTP y salidas de la CLI.
  - Aserciones deterministas sobre la evidencia dictan el veredicto computable.
  - Un verificador separado, con razonamiento prendido, juzga solo el resto no computable.
  - Ese verificador empieza en sombra, como el juez visual de `gate_sprites` (`mmorch/sdlc.py:462`), hasta que una medición lo habilite.
- Debilidad de universal-modder que no se copia: el agente que escribe el mod también lee la captura y dicta el veredicto.
- Lo que ya existe: la skill `/project` con el SDLC de 6 etapas (`mmorch/sdlc.py`), modo grill para la aceptación, gates deterministas y fix ladder.
- Lo que falta: descomposición desde cero en módulos, oráculo de integración entre módulos, agente que maneja el producto y memoria entre corridas.
- Insumo en bd: `orchestration-4rf` mide notas de campo de corridas previas en el SDLC. Primaria: rechazos de gates y escalones del fix ladder; secundaria: éxito por ítem.
- Dato previo (`orchestration-34k`): en corridas reales el planner ya acierta 0.875 por ítem; el cuello de botella es el coder de la etapa 4.

## Decisions so far

<!-- una línea por ticket resuelto: gist + link -->

- [Juez en sombra con rúbricas](issues/09-juez-en-sombra.md) — `mmorch/juez_sombra.py`: de a pares, orden invertido, sin puntajes, nunca bloquea; texto de F2 consistente (gana prototipo-04, coincide con el orquestador); visual caído por saldo de GLM.
- [Haiku 5.5 como coder](issues/10-haiku-coder.md) — coder `claude:<modelo>` por la CLI aislada y sorteo balanceado con semilla en el builder; Haiku construyó m1 por US$0,0015 equiv.; DeepSeek falló m3 en esa corrida.

- [Walkthrough como Artifact](issues/08-walkthrough-artifact.md) — página privada con seis secciones, comentario y ok por sección; los oks viven en la colección `oks` y Claude los lee antes de lanzar el builder; el de F2 está publicado y espera los oks del usuario.

- [Lighthouse y axe-core en el driver](issues/11-lighthouse-y-axe.md) — corren sin red en `builder-driver`; variantes siguen en 1,000; el sitio real de F2 queda rojo por un input sin etiqueta (axe critical) que Lighthouse aprobaba con 0,91.

- [Bootstrap y encadenado automáticos](issues/06-bootstrap-y-encadenado.md) — `python -m mmorch.builder <plan> <carpeta>`: bootstrap, módulos en cadena por el SDLC e integración en contenedor; F2 armado desde una carpeta vacía con un solo comando en ~8 min, integración verde.

- [Errores del entorno contra errores del código](issues/07-errores-del-entorno.md) — builtin o paquete declarado pero ausente = entorno; paquete inventado o import relativo = código; en contenedor Claude no instala y nombra qué falta en la imagen. Caso real: de 3 vueltas + 2 escaladas a 0 vueltas + 1 llamada con diagnóstico claro.

- [Descomposición en módulos](issues/04-descomposicion-modular.md) — prototipo real: bootstrap con contratos antes del código, 3 módulos en cadena por el SDLC (built, 0 escaladas, ~9 min), integración verde con el driver contra 70 casos ocultos; US$0,14 de DeepSeek. Hallazgos: títulos `test_R<n>_` en tests no Python; el SDLC no separa errores del entorno; encadenado a mano.

- [Driver de producto mínimo](issues/05-driver-de-producto.md) — Playwright en el contenedor sin red maneja el sitio real; 10 variantes, exactitud balanceada 1,000, ~6 s cada una; los casos ocultos cargan el veredicto y las metamórficas no atrapan tope, tramos ni ejemplos hardcodeados; imagen `builder-driver` de 2,4 GB.

- [Oráculo agente por tipo de entregable](issues/03-oraculo-agente.md) — la verdad vive fuera del worktree y el coder no la ve; CLI ~90 %, API ~85 %, web ~70 %, flujo ~60 % computable; el loopback funciona sin red; para F2 bloquean casos ocultos, relaciones metamórficas, consola, Lighthouse, axe y metadatos; la convención anualizada o acumulativa se decide en el walkthrough.

- [Destino y alcance](issues/01-destino-y-alcance.md) — herramienta general medida como banco; primer entregable F2 sueldo neto; termina en el preview verificado; capa computable bloquea y el juicio con rúbricas va en sombra; walkthrough como Artifact y dos paradas humanas; APIs US$5; Claude en descomposición, planner y agente del preview; coder Haiku o DeepSeek por sorteo.

- [Inventario: qué del SDLC sirve por módulo y qué falta](issues/02-inventario-sdlc.md) — el SDLC sirve para el módulo 2 en adelante; bloquean el bootstrap del repo vacío, la descomposición (`division.md` sin consumidor) y el encadenado entre módulos (cada corrida parte del HEAD).

## Not yet specified

- Tickets 06 a 11 (creados 2026-10-15) cubren lo que falta para el destino: bootstrap y encadenado, errores del entorno, walkthrough, juez en sombra, Haiku coder, Lighthouse y axe.
- Promovidos a bd porque son del SDLC en general: `usd_run` en null y hooks del usuario en la revisión 5b (orchestration-gkw y orchestration-xdi).
- VLM del juez visual: GLM sin saldo (código 1113); recargar o elegir otro modelo con visión.
- Kappa del juez: el usuario etiqueta pares de entregables (mínimo 20) para decidir si sale de sombra.
- Prueba fuerte del driver: defectos de corridas reales del coder, no plantados por el autor del driver. Primer caso real: el input sin etiqueta de F2 (ticket 11).
- Integración roja sin vuelta atrás: hoy el builder corta; falta devolver el defecto al módulo que lo causó (por ejemplo, m3 con el hallazgo de axe como nueva aceptación).
- `human_interventions` del SDLC nunca se incrementa: contarlo para medir las horas del usuario.
- Tope de US$5 por entregable que sume todos sus módulos.

## Out of scope

- Modificar juegos: eso es universal-modder, clonado aparte en el escritorio.
- Agente de publicación (hosting, dominio, AdSense): esfuerzo aparte, con gate humano por zona roja.
- Segundo entregable de otro tipo: destino del mapa siguiente.
