# A/B/C — engine de /project vs SDLC de 6 etapas (automático) vs híbrido con Claude · 2026-09-10

Feature: portar el matcher del prototipo `demo.py` (QueTePario/ChatBot) a Java 17, paquete
`com.qtp.bot`, sin Spring. Aceptación: `BotAcceptanceTest.java`, 35 asserts, traducción 1:1
del `test_demo.py` del prototipo. Oráculo: el Python. Baseline compartido: commit `d6853c9`
del repo ChatBot (pom + test). Los tres brazos parten de ahí.

## Resultado

| brazo | quién decide en los gates | aceptación | llamadas | USD | min | intervenciones |
|---|---|---|---|---|---|---|
| A: engine `/project` | nadie | **rojo**, 5 intentos | 11 → 21 | 0,81 total | ~120 | 4 fixes de engine + 5 relanzamientos |
| B: 6 etapas, script | el script | **verde** (v2) | 31 | 1,13 (2,68 con v1) | 22 | 0 humanas, 2 de arnés |
| C: 6 etapas, híbrido | Claude en los gates | **verde** | 9 | 0,16 | 9 | 2 de Claude, 4 min |

B y C usaron la misma spec y el mismo plan (deepseek-reasoner) y el mismo coder
(deepseek-v4-pro). Lo único que cambió fue qué pasó cuando el test dio rojo.

## Qué falló y por qué

- **A, intentos 1-2** (deepseek-chat, 57 s): el planner creó una unidad "acceptance-test"
  con el comando de aceptación como `test_cmd`; nunca puede pasar sola → el engine escaló
  antes de correr la integración. El coder de `Bot` nunca vio `Msg.java` e inventó su API.
  El intento 2 fue byte a byte igual: worklist cacheado por task.
- **A, intento 3**: murió con la sesión de Claude Code (proceso hijo).
- **A, intento 4** (v4-pro): la unidad `Bot` devolvió 3 veces `out_tokens=0` — 16.384 de
  budget agotados en reasoning.
- **A, intento 5** (v4-pro, 32.768): `Bot.java` de 571 líneas aterrizó, pero el engine escaló
  igual (forma VERIFY con `max_depth=1`; `gen_model` no se propaga a la recursión: 12
  sub-unidades codeadas por deepseek-chat). El código estaba **a un método de verde**: el
  mismo `Catalog.load throws IOException` que C corrigió con 4 líneas.
- **B, v1**: el fix loop reescribía los 9 archivos por vuelta y en la vuelta 3 rompió la
  compilación. **v2**: el modelo nombra qué archivos tocar, gate de compile por vuelta con
  revert → verde en 2 vueltas (Reply, Catalog).
- **C**: revisión del plan (3 min, aprobado), build compila a la primera, test-compile falla
  por la excepción chequeada, 1 método corregido a mano, verde.

## Los 4 fixes del engine que salieron de esto (commits 58ce334, ea8b775)

1. `validate_worklist` rechaza una unidad que es el test de aceptación (4 tests).
2. El coder recibe el código real de las unidades de las que depende.
3. `/run/workflow` acepta `gen_model` y `max_fix`.
4. El coder corre con `max_tokens=32768` y timeout 400 s.

Pendientes (ticket 05 del mapa): `gen_model` en la recursión, `max_depth` de la forma
VERIFY, gate `test-compile`, realimentar el fallo de integración al coder.

## Lo que la medición NO prueba

Una feature, un repo, un día. Los tres brazos recibieron correcciones durante el
experimento (A: 4 de engine; B: 2 de arnés; C: ninguna). C tiene un costo que no está en
dólares: cupo de Claude, 4 minutos. La comparación justa pide 3 features más (ticket 06/07).

## Archivos

- `ab_results.json` — registro por intento y brazo, con costos del ledger `logs/metrics.jsonl`.
- `ab_sdlc_b.py` — driver de B y C (6 etapas, gates deterministas, fix loop v2, `--from-stage`, `--wt`, `--phase`, `--max-fix`).
- `spec-reasoner.md`, `plan-reasoner.md` — artefactos de las etapas 2 y 3, compartidos por B y C.
- `B-run-log.json` — etapas, vueltas y gates de B v2.
- `C-supervision.md` — las 2 intervenciones de Claude en C, con tiempo y motivo.
- `synth_seed47_findings.md` — checkers sintetizados por tipo, 6 modelos, incluido el co-fallo cross-familia.
- `wayfinder-prompt-sdlc6.md` — el prompt con el que se charteó `.scratch/sdlc-6-gates/`.
- Ramas en el repo ChatBot: `ab/b-sdlc` (f9798ad), `ab/c-hybrid` (dcdef5c), `mmorch/wt-9151a412` (A5).
