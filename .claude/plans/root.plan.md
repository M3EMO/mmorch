# Plan: Root

**Source PRD**: `.claude/prds/sistema-nuevo.prd.md`
**Selected Milestone**: 1 — Root: reemplaza el banco y el ruteo de mmorch
**Complexity**: Large
**Status**: APROBADO por el usuario el 2026-10-27. Lo construye Sonnet con builders Haiku en otra corrida.

## Summary

Root es un binario de Rust llamado `root`. Root evalúa el valor de un proyecto, rutea trabajo entre cupo y APIs, observa cada corrida y aprende de lo que registra.
El Oracle trabaja como un project manager, no como un pseudo-compilador. Responde tres preguntas: qué prometió la spec, qué cumple el producto y si está listo para entregarse.
Juzga nueve dimensiones: completitud, funciona, entrega, pruebas, seguridad, eficiencia, estructura, arquitectura y diseño. Entrega un informe con una decisión (listo, no listo, bloqueado), los requisitos faltantes, los riesgos y los próximos pasos.
Root también mide su propio evaluador: una dimensión entra al puntaje global solo si su calibración la respalda.

## Decisiones del usuario (2026-10-27)

| # | Decisión |
|---|---|
| a | El producto es Root + Graft + ECC + REA entero (opción C). ECC, Graft y REA se reconstruyen en hitos posteriores. |
| b1 | Las mediciones de "Measured" valen como evidencia. El método de medición queda en duda: la calibración (Tarea 12) lo revisa. |
| b2 | Se conserva el mecanismo de hash del GOAL (`GOAL.md` + `GOAL.hash`). |
| b3 | `logs/metrics.jsonl` se importa una vez, en solo lectura. Track arranca con un log nuevo. |
| b4 | El vault queda fuera de Root. El hito 1 lo resuelve "porque empieza de 0": Root no lee ni escribe el vault de mmorch y no hereda nada de él. Ver Tarea 18. |
| b6 | Opus redacta `GOAL.borrador.md`. El usuario lo lee y lo copia a mano a `GOAL.md`. Ningún agente escribe `GOAL.md` ni `GOAL.hash`: lo impiden `~/.claude/never-edit.txt` y el hook never-edit de `.claude/settings.json` del worktree (probado con `--setting-sources project`). |
| b5 | El código arranca de cero. No se porta código de mmorch. |
| 3 | Root es Route + Oracle + Observe + Track, y además aprende. |
| 4 | Route rutea a DeepSeek y Anthropic, y balancea el uso de APIs contra el cupo. |
| 5 | Cuenta como intervención todo mensaje o ajuste humano posterior a la aprobación del plan. |
| 6 | Los oráculos ocultos viven en `~/.local/share/root/oraculos/`. |
| 7 | Haiku no se mide antes. Haiku es el builder. |
| 8 | Root es un evaluador de alto nivel ("project manager"), no un pseudo-compilador. |
| 9 | El cupo del hito no es una restricción. |
| 10 | Lenguaje Rust (confirmado). El paquete se llama `root`. |
| 10b | La configuración de Root es solo TOML (`root.toml`, `tarea.toml`). El Oracle evalúa TOML, YAML y Dockerfile en los proyectos (Tarea 9b). Root nunca lee YAML como configuración. |
| — | Roles: Opus escribe este plan. Sonnet organiza a los builders. Haiku construye. |
| 11 | "Las mediciones deben ser competentes, no me sirve un pseudo-compilador. Necesito un PM." Revisión de las Tareas 6 a 19: ver "Revisión PM" al final. |
| 12 | P1: se acepta la propuesta de bloqueo, más rúbricas basadas en estándares de la industria para completitud, diseño y el resto de las dimensiones. P2: el usuario califica a ciegas. P3: el usuario aprueba los requisitos; la spec sale de una sesión `/grill-me`. P4: recall ≥ 0,8. |
| 13 | P5: kappa ponderado ≥ 0,6 por dimensión y la misma decisión en 5 de los 6 paquetes. |
| 14 | El binario se llama `r00t`. El crate, `root.toml`, `ROOT_HOME` y las herramientas MCP `root_*` conservan el nombre `root`. |
| 15 | P6, topes duros: API 100 USD por mes; sesiones de Claude en la nube 250 USD por mes; cupo 35 USD equivalentes por 5 horas (corrección del usuario sobre los 100 iniciales). Un tope nunca se supera sin un gate humano. Ver Tarea 14. |
| 16 | "De que son los tests? ya dije no quiero un compilador de codigo, quiero un evaluador con criterio, un project manager con datos para decidir." Los tests ocultos son una fuente de evidencia de completitud, no el veredicto. El hito ya no exige reproducir conteos del banco: exige que la decisión del PM sea estable y que cada diferencia tenga una causa con evidencia. Ver "Revisión PM 2". |
| 17 | P10: "Si puede. Es lo mas correcto para tener librerias funcionales ya." Root baja dependencias con red en una fase previa a la evaluación (opción a). La respuesta del usuario es la autorización permanente que pide la zona roja del GOAL para la red. Ver Tarea 6b. |
| 18 | P11: "La idea es armar una ficha tecnica, arquitectonica, de diseño y de producto en los spec. La idea es hacerlo mas especifico, no ambigüedades." Cada spec lleva una ficha estructurada con cuatro partes. Root revisa la ficha antes de evaluar, y una ambigüedad se corrige en la ficha en vez de excluir un test. Ver Tarea 9f. |
| 19 | P13: "que diga pendiente o probar en collab". Un test que falla por el entorno queda `Pendiente` en el informe: no se cobra al producto ni se excluye en silencio. La otra salida es correrlo en Colab con GPU, con la aprobación del usuario (P15). Ver Tarea 7f. |
| 20 | P14: "en el spec. Obligatoria". La ficha vive dentro de `SPEC.md`, en una sección `## Ficha` con un bloque TOML. Sin ficha, Root no evalúa. Ver Tarea 9f. |
| — | P12: el usuario respondió "P12 si". La pregunta ofrecía dos opciones, así que la respuesta no elige una. Rige la lectura conservadora (solo crates.io y PyPI) hasta la aclaración P12b. |
| 21 | P12b: "b". La descarga acepta además dependencias por git y otros registros. Es la autorización del usuario para esa red de la zona roja. Ver Tarea 6c. |
| 22 | P15: "b". Un agente corre en Colab los tests pendientes por entorno, solo con una GPU T4. Ver Tarea 7h. |
| 23 | P16: "a". La imagen `oraculo-rust` instala `clippy`, `gitleaks`, `cargo-audit`, `hadolint` y `cargo-mutants`, con versiones fijas y hash verificado. Ver Tarea 7g. |
| 24 | P17: "c+b". La parte c) se aplica: Root baja la imagen base con red y construye con `--network none` (Tarea 9d2). La parte b) espera la aclaración P17b. |
| 25 | P18: "c pero que tambien evolucione a esquemas o arquitecturas de agentes". La parte c) se aplica: Aprende propone y el usuario aplica en `root.toml` (Tarea 15c). La segunda parte espera la aclaración P18b. |
| 26 | P19 y P20: "si". La SPEC de memora exige orden FIFO entre clientes bloqueados en `BLPOP`. "En el espacio local" de brasa significa los ejes locales del nodo. Las dos respuestas van a las fichas (G1). |
| 27 | P17b: "a". Si la construcción sin red queda `Pendiente`, Root la repite con red en forma automática. Es la autorización del usuario para correr con red los `RUN` del builder. Ver Tarea 9d3. |
| 28 | P18b: "a y b". Hito 1: cada `Fila` guarda la forma de la corrida, y Aprende propone cambios de forma que el usuario aplica a mano. Hito 4 (CAB): generar y medir arquitecturas de agentes nuevas. Ver Tarea 15d. |
| 29 | P21: "b". Root lee la credencial de un registro desde una variable de entorno nombrada en `root.toml`. La pasa solo al contenedor de descarga y nunca la escribe en un log. Es la autorización del usuario para esas claves. Ver Tarea 6d. |
| 30 | P34: "C pero con un juez (sonnet) para delimitar necesidades." La descarga respeta el `.cargo/config.toml` del repo, y un juez Sonnet evalúa qué entradas necesita el proyecto. El juez solo informa (decisión 35). Ver Tarea 6d. |
| 31 | P35: "b con c de respaldo". Colab instala Rust con `rustup-init` verificado por su `.sha256`. Si esa instalación falla, Root compila brasa en la máquina del usuario y lleva el binario. Ver Tarea 7k. |
| 32 | P36: "a". El "Outcome" del hito 1 en el PRD vuelve a su texto original. |
| 33 | P37: "P37 B pero sin limite de tiempo", precisada como "hagamos que el limite sean 2 hs pero que el planner y un hook/script corten una corrida si ya se recibio lo que requeria". La cuenta de Colab es paga y el usuario acepta el gasto de la T4. Cada corrida tiene un tope de 2 horas y se corta antes si ya entregó lo que pedía. Ver Tarea 7l. |
| 34 | "Prioricemos el uso de las sesiones de nube porque vence el 5 de noviembre, concentremos el trabajo alli." Las builds de Sonnet corren en sesiones de Claude Code en la nube, sin podman, sin `ROOT_HOME` y sin Colab, hasta el 2026-11-05. Ver "Nube o laptop". |
| 35 | P38: "a". El juez de la config de cargo solo informa: sus hallazgos van al informe, y `cargo` usa todo el config. El juez nunca abre ni cierra la red. |
| 36 | "Acepto P33b b, P39 c con P44 c, y P46 a." P39 c: memora y brasa calibran al evaluador (Tareas 12 y 12b), y la Tarea 16 mide con `r00t corre` una tarea de Graft con oráculo oculto nuevo (Tarea 16b). P44 c: memora sigue hasta G3, y después el usuario decide si sigue (gate G4). P33b y P46 son de la sesión de fichas y viven en sus borradores. |
| 37 | P48: "P48 a pero emita sus tareas en toml". La tarea de Graft de la Tarea 16 es un lector de planes: lee un `plan.md` y emite en TOML sus tareas, dependencias y comandos `Validate`. Ver Tarea 16b. |
| 38 | P49: "P49 b: la Tarea 16 corre Graft y memora." La corrida real de la Tarea 16 mide `graft-lector-planes` y memora, con una corrida completa más de Haiku sobre memora. |
| 39 | P50: "P50 c". La lectura del oráculo se cubre con dos capas: el builder corre aislado de `ROOT_HOME`, y Root además busca lecturas en el transcript. Ver Tarea 26. |
| 40 | P51: "P51 c". En Colab, `correr.sh` corre sin red dentro de `unshare -rn`. Si Colab no lo permite, la corrida se hace igual y el informe lo avisa. Ver Tarea 23. |
| 41 | G1d: "haiku con razonamiento". G1e: "usar un modelo disponible de deepseek." G1f: "d". DeepSeek usa modelos vigentes y envía `thinking` explícito. Haiku con razonamiento es el respaldo de Verifica y Juzga. Los precios de DeepSeek salen de `mmorch/config.py`: `deepseek-flash` 0,14 y 0,28 USD, `deepseek-v4-pro` 1,74 y 3,48 USD por millón de tokens. Ver Tarea 21b. |
| 42 | G1g: "G1g a, pero no pueden estar el tope de tokens ya alto de por si?" Sonnet sube el tope de salida de los dos jueces, y después Opus corre otra vez el juez de la ficha. Respuesta a la pregunta del usuario: sí, el tope por defecto debe ser alto. El tope de salida es un techo, no un gasto: Root paga solo los tokens que el modelo usa. El tope de API de Route sigue acotando el costo, porque su chequeo previo cuenta el peor caso. El default pasa a 32 000 tokens. Ver Tarea 10c. |
| 43 | P53: "P53 a". El builder corre en un contenedor de podman sin `ROOT_HOME`, con la credencial de Claude montada en solo lectura. Ver Tarea 26. |
| 44 | P52: "P52 c pero yo no lo confirmo." Un agente busca el precio vigente de Haiku y Root lo usa sin confirmación del usuario. El orquestador lo buscó en la referencia de la API de Anthropic (skill `claude-api`, tabla cacheada el 2026-10-06). `claude-haiku-5-5` cuesta 0,10 USD por millón de tokens de entrada y 0,50 de salida, con hasta 100 000 tokens de entrada. Con más de 100 000 tokens de entrada, cuesta 0,50 y 2,50 USD. Coincide con `mmorch/config.py`. Ver Tarea 21b. |
| 45 | G1h: "G1h: a ahora para correr el juez, y b como corrección en el código." Ahora: Opus sube `[proveedores].timeout_s` a 900 segundos en `root.toml` y corre otra vez el juez de la ficha. En el código: los dos jueces tienen un timeout propio, más largo que el de las demás llamadas. Ver Tareas 21, 21b y 21c. |
| 46 | P54: "P54 c". Opus registra 0,50 USD como peor caso de los 3 intentos cortados del juez de memora, con un evento de costo manual. Ver Tarea 21. |
| 47 | P55: "P55 a". Un agente busca el precio de Sonnet y Root lo usa sin confirmación del usuario, igual que con Haiku (decisión 44). El orquestador lo buscó en la referencia de la API de Anthropic (skill `claude-api`, tabla cacheada el 2026-10-06). `claude-sonnet-5-5` cuesta 2 USD por millón de tokens de entrada y 10 USD por millón de salida. La tabla no muestra tramos por tamaño de entrada. Ver Tarea 21b. |
| 48 | P56: "P56 c". Una dimensión sin veredicto que tiene algún ítem `bloquea = true`, como Seguridad o Completitud, da `SinDecision`. Una dimensión sin veredicto sin ítems que bloquean deja `Listo` con aviso en `confianza`. P57: "P57 a". La calibración real corre por separado en memora y en brasa, con `n = 20`, y cada calibración vale solo para su tarea. Ver Tareas 12, 13b y 28. |
| 49 | P58: "P58 a". Primero la evidencia: `decidir` aplica `Bloqueado` y `NoListo` antes que `SinDecision`. `SinDecision` solo reemplaza a un `Listo` cuando una dimensión con un ítem que bloquea queda sin veredicto. Esta decisión acota la decisión 48: su regla ya no va por encima de las demás. Ver Tarea 32. |
| 50 | P-req1: "P-req1 a". Root cruza los requisitos también con el junit del contrato, además del oculto. Ver Tarea 34. |
| 51 | P-req2: "P-req2 a, apruebo la descarga". Se agregan las 5 muestras glTF de Khronos a `oraculos/brasa/oculto/muestras/`. Es la autorización del usuario para esa descarga con red. La hace el orquestador. |
| 52 | P-req3: "P-req3 a". La ficha de brasa pide un `LICENSE` en la raíz para R14, en la prosa y en la aceptación. |
| 53 | P-req4: "P-req4 a". La sesión de fichas suma 20 tests ocultos a memora (R2: los 10 flags y el flag que pisa al archivo; R4: `CLIENT` y `HELLO`) y 1 a brasa (R13: `brasa info`). |
| 54 | P-test1: "P-test1 b", P-test2: "P-test2 a", P-test3: "P-test3 a". La ficha de brasa fija los nombres de wgpu de `brasa info`. Los sufijos de `--maxmemory` y la diferencia de `--appendfsync` quedan sin fijar. |
| 55 | P59: "B". Un test inestable con una cota inferior de Wilson al 90 % menor que 0,5 cuenta como falla. Uno con una cota de al menos 0,5 pone tope 2 a su requisito. La nube 9 lo construye en la Tarea 40. |
| 56 | P-mod1: "P-mod1 a". El informe filtra la lista `modulos` de un requisito sin cumplir contra los módulos del repo evaluado. Para los que existen escribe "revisar". Si no queda ninguno, escribe "falta" con la lista entera. Ver Tarea 42. |
| 57 | Contrato del juez: "Sí, al planner", sobre la propuesta del orquestador. El usuario había preguntado: "El juez tiene un contrato de toml? quizas necesita una rubrica y contrato para ser mas robusto". El juez responde todos los ítems pedidos, y Root reintenta una vez los que falten o no pasen el gate. Entra antes de la calibración real. Ver Tarea 44. |
| 58 | P60: "P60 c". El usuario había preguntado: "Que le falta a root? terminemos". El cierre del hito 1 es corto, con validez automática: la Tarea 12b mide el orden conocido y los defectos reales, sin el juicio humano de G3. G3, G4, Colab (Tarea 7h) y Graft (Tareas 16b y la corrida de Graft de la 16, con G5) pasan al hito 1b. Acota las decisiones 12, 13, 36 y 38. |
| 59 | P-val1: "P-val1 a". El recall de la Tarea 12b usa los 8 defectos reales de memora y brasa. La nota de cierre anota que faltan los 7 de F1, cuyos repos se borraron. |
| 60 | P-val2: "P-val2 a". No hay séptimo par: la validez queda con 6 pares de orden conocido. |
| 61 | G4: el usuario escribió "brasa y memora estan solo de medicion y seran eliminados". memora y brasa no siguen como banco después del hito 1. G4 queda respondido y sale del hito 1b. |
| 62 | Tarea 16: "a) Correr en memora". La corrida real de `r00t corre` usa memora para probar el runner de punta a punta, y después se borra. |
| 63 | Calibración: el usuario preguntó "No se puede hacer mas rapida? eficiente? o en paralelo? El diseño de por si digo", y eligió "a) Cortar y mejorar". Ver Tareas 48 y 49. |
| 64 | Objetivo: "terminar root. Calibrarlo a la perfeccion, no terminar con la primer version completa, seguir hasta terminar". Y: "Acelerar los procesos para que no sea todo secuencial, un recurso que hay que aprovechar es el tiempo." El hito 1 cierra con la calibración completa, y el trabajo independiente corre en paralelo. |
| 65 | P62: "a) Rechazar claves". Si Root pasa una credencial, rechaza en el config de cargo del repo las claves que ejecutan comandos o cambian el TLS. Sin credenciales, el config se respeta entero. Acota la decisión 30. Construida en la Tarea 51 (C2). |
| 66 | Calibración: el orquestador propuso reusar el `target` de la base en cada caso. El usuario respondió "Sí, esperar". La calibración real espera la Tarea 59. |

## Patterns to Mirror

No existe código Rust en el repo. Este plan fija los patrones de Rust; Haiku los copia de la Tarea 1 en adelante.
Las formas de datos sí tienen un antecedente. Root copia la forma, no el código.

| Category | Source | Pattern |
|---|---|---|
| Evento de costo | `mmorch/metrics.py:28-41` (rama `mmorch/auto`) | Una línea JSON por llamada: `ts, iso, phase, model, family, in_tokens, out_tokens, cost_usd, latency_s, extra`. `Evento` es un superconjunto. |
| Resultado del oráculo | `.scratch/ecc-vs-sdlc/oraculo/evaluar.py:17-50` | Junit XML por suite; conteo `pass/fail/skip` por archivo; la cola de la salida se guarda para diagnosticar. |
| Hash del GOAL | `mmorch/goal.py:45-54` | `sha256(GOAL.md)[:16]` en `GOAL.hash`. Solo un humano lo reescribe. |
| Etiqueta por construcción | `mmorch/gates_bench.py` (README "SDLC gate checks") | La etiqueta de un caso sale de cómo se construyó, nunca de la regla evaluada ni de un LLM. |
| Reintentos de API | `mmorch/providers.py:77-91` | 3 intentos, base 0,5 s, breaker de 3 fallas en 60 s. |
| Corrida de ECC | `~/Proyectos/bench-ecc-sdlc/correr-ecc-rol.sh` | `claude -p --setting-sources project --strict-mcp-config --plugin-dir`, `ECC_DISABLED_HOOKS`, `--output-format json`. |
| Errores | — (nuevo) | `thiserror` en la biblioteca; `anyhow` solo en `main.rs`. Ante un error de infraestructura, el Oracle no emite veredicto (falla cerrado). Nunca devuelve verde. |
| Tests | — (nuevo) | Unitarios en `#[cfg(test)]` dentro del módulo. Integración en `tests/<modulo>.rs`. Fixtures en `tests/fixtures/`. |

## Interfaces fijas (las usan Graft, REA y CAB)

Graft, REA y CAB usan Root por tres vías: la CLI con `--json`, los archivos de `ROOT_HOME` y el servidor MCP. Ningún hito posterior importa módulos internos de Root.

### Archivos

`ROOT_HOME` vale `~/.local/share/root/` por defecto.

```
$ROOT_HOME/
  root.toml               # modelos, precios, balance, umbrales (lo escribe el humano)
  eventos.jsonl           # Observe: una línea por evento, "v": 1
  runs.jsonl              # Track: una línea por corrida, "v": 1
  politica.json           # Aprende: política de Route recalculada
  calibracion.json        # Oracle: exactitud medida por dimensión
  oraculos/<tarea>/
    tarea.toml            # definición de la tarea
    contrato/             # tests visibles para el builder (se copian al bootstrap)
    oculto/               # tests que el builder nunca ve
    bootstrap/            # repo inicial
  importado/mmorch-metrics.jsonl   # copia de solo lectura (decisión b3)
<repo>/GOAL.md, <repo>/GOAL.hash   # contrato anti-drift (decisión b2)
```

Toda línea JSONL lleva `"v": 1`. Un cambio de formato sube `v`, y los lectores aceptan las versiones anteriores.

### `tarea.toml`

memora y brasa están escritos en Rust. Sus oráculos son tests de caja negra en pytest. Por eso `lenguaje` (el del proyecto) y la imagen (la del oráculo) son campos distintos.

```toml
nombre = "memora"
lenguaje = "rust"                        # elige el perfil de sondas (Tarea 9)
imagen = "root-oraculo-rust:1"           # Rust + Python/pytest para los oráculos de caja negra
containerfile = "bootstrap/Dockerfile"   # opcional: si existe, el Oracle construye la imagen desde el Dockerfile del proyecto
spec = "bootstrap/docs/SPEC.md"
[contrato]
cmd = "pytest -q -p no:cacheprovider --junitxml=/salida/contrato.xml /repo/tests"
[oculto]
cmd = "pytest -q -p no:cacheprovider --junitxml=/salida/oculto.xml /oraculo/oculto"

[[requisito]]                            # Tarea 9c: lo que el producto promete, sección por sección de la spec
id = "R1"
titulo = "Protocolo RESP2"
seccion = "## Protocolo"
peso = 3                                 # 1..=3, importancia para el usuario del producto
pruebas = ["test_m1_protocolo"]          # archivos o ids de tests ocultos: el caso feliz
bordes = ["test_m1_protocolo::test_trama_partida", "test_m1_protocolo::test_error_resp"]  # bordes y errores
escenarios = ["e2e-set-get"]             # escenarios de [[escenario]] que lo prueban
criterios = ["completitud.alcance"]      # ítems de rúbrica que este requisito alimenta

[[escenario]]                            # Tarea 9d: un flujo real de usuario, en un contenedor limpio
id = "e2e-set-get"
cmd = "/oraculo/escenarios/set_get.sh"   # sale 0 si el flujo funciona

[entrega]                                # Tarea 9d: comprobaciones de "listo para producción"
instalar = "cargo build --release"
correr = "./target/release/memora --config memora.conf.example"
archivos = ["README.md", "Dockerfile", "memora.conf.example", ".github/workflows/ci.yml"]

[pruebas]                                # Tarea 9e: calidad de los tests del propio proyecto
mutantes_max = 60
minutos_max = 20

[eficiencia]
cmd = "python /oraculo/bench.py /repo"   # imprime JSON {"ops_s": f64}
piso = 5000.0
[arquitectura]
capas = [["protocolo"], ["comandos"], ["almacen"]]   # una capa importa solo capas posteriores
```

### Cómo corre el oráculo en un contenedor (respuesta a la pregunta 6)

- El builder trabaja en su propio contenedor o directorio. Esa sesión nunca monta `oraculos/`.
- Al terminar la construcción, el Oracle copia el repo a un directorio temporal. Así un artefacto del builder no altera el original.
- El Oracle levanta un contenedor nuevo con `podman run --rm --network none`. El repo copiado se monta en `/repo:ro,z`. `oraculos/<tarea>` se monta en `/oraculo:ro,z`. `/salida` es el único directorio con escritura.
- El tag `z` es obligatorio en Fedora: SELinux bloquea el montaje sin ese tag.
- Los tests ocultos corren desde `/oraculo` y apuntan a `/repo`: `PYTHONPATH=/repo`, o el equivalente del lenguaje.
- **Fase de descarga** (decisión 17, Tarea 6b): antes de evaluar, un contenedor aparte baja las dependencias con red. Ese contenedor no monta `/oraculo` y no compila nada. Solo corre `cargo fetch --locked` (o `pip download` contra PyPI) y escribe en `$ROOT_HOME/cache/<lenguaje>/`. Después, la evaluación monta esa caché en solo lectura y corre con `--network none` y `CARGO_NET_OFFLINE=true`, como antes.
- El builder corre como el mismo usuario. Por eso puede leer `ROOT_HOME` aunque no lo monte. Hay dos defensas:
  - El runner niega `Read` y `Bash` sobre `oraculos/` en los permisos de la sesión.
  - Observe busca la ruta `oraculos/` en el transcript. Si la encuentra, la corrida queda inválida.

### Tipos (Rust, `root::tipos`, con serde)

```rust
pub enum Rol { Planifica, Organiza, Construye, Verifica, Juzga, Delega }
pub enum Proveedor { Anthropic, Deepseek }
pub enum Via { Cupo, Api, Oraculo }          // Oraculo = no llamar a ningún LLM
pub enum Dimension { Funciona, Seguridad, Eficiencia, Estructura, Arquitectura, Diseno }
pub enum Estado { Verde, Rojo, SinVeredicto }
pub enum Fuente { Sonda(String), Juez { modelo: String } }

pub struct Pedido { pub rol: Rol, pub tipo: String, pub checkeable: bool, pub tokens_estimados: u32 }
pub struct Eleccion { pub via: Via, pub proveedor: Option<Proveedor>, pub modelo: Option<String>,
                      pub razonamiento: bool, pub motivo: String }

pub struct Conteo { pub pasa: u32, pub total: u32 }
pub struct Hallazgo { pub dimension: Dimension, pub severidad: u8 /*1..=3*/, pub archivo: String,
                      pub linea: u32, pub cita: String, pub detalle: String, pub fuente: Fuente }
pub struct Puntaje { pub dimension: Dimension, pub valor: f32 /*0..=1*/, pub calibrada: bool,
                     pub exactitud_balanceada: Option<f32> }
pub struct Veredicto { pub v: u32, pub tarea: String, pub repo: String, pub commit: String,
                       pub contrato: Conteo, pub oculto: Conteo, pub puntajes: Vec<Puntaje>,
                       pub hallazgos: Vec<Hallazgo>, pub global: Option<f32>, pub estado: Estado,
                       pub costo_usd: f64 }

pub enum TipoEvento { Llamada, Sesion, Humano, PlanAprobado, Gate, OraculoLeido }
pub struct Evento { pub v: u32, pub ts: f64, pub iso: String, pub corrida: String, pub fase: String,
                    pub tipo: TipoEvento, pub via: Option<Via>, pub modelo: Option<String>,
                    pub familia: Option<String>, pub in_tokens: u64, pub out_tokens: u64,
                    pub cost_usd: f64, pub latency_s: f64, pub extra: serde_json::Map<String, Value> }
pub struct Resumen { pub usd_api: f64, pub usd_cupo: f64, pub minutos: f64, pub turnos: u32,
                     pub intervenciones: u32, pub invalida: bool }
pub struct Fila { pub v: u32, pub id: String, pub ts: f64, pub tarea: String, pub config: String,
                  pub commit_root: String, pub veredicto: Veredicto, pub resumen: Resumen }
```

#### Extensión PM (Tarea 5b, solo agrega; no cambia lo que construyeron las Tareas 1 a 5)

```rust
// Dimension suma tres variantes al final: Completitud, Entrega, Pruebas.
pub enum Cumplimiento { Cumplido, Parcial, Ausente, SinVeredicto, Pendiente } // Pendiente: Tarea 7f, al final
pub struct EstadoRequisito { pub id: String, pub titulo: String, pub peso: u8,
                             pub cumplimiento: Cumplimiento, pub evidencia: Vec<String> } // ids de tests, escenarios o hallazgos
pub enum Decision { Listo, NoListo, Bloqueado }
pub struct Accion { pub prioridad: u8, pub texto: String, pub refs: Vec<String> }        // refs = ids de requisitos o hallazgos
pub struct Informe { pub decision: Decision, pub motivo: String,
                     pub requisitos: Vec<EstadoRequisito>,
                     pub bloqueantes: Vec<String>,      // ids de hallazgos
                     pub riesgos: Vec<Accion>,          // máximo 5, ordenados por impacto
                     pub proximos_pasos: Vec<Accion>,   // máximo 5
                     pub resumen: String }              // prosa; cada oración cita un id
// Hallazgo suma `id: String` (estable: hash de dimensión+archivo+cita) e `impacto: Vec<String>` (requisitos que afecta).
// Veredicto suma `informe: Option<Informe>` con #[serde(default)].
```

#### Ficha del spec (Tarea 9f; decisiones 18 y 20)

La ficha vive dentro de `SPEC.md`, en una sección `## Ficha` con un solo bloque ` ```toml `. La prosa del spec explica, y la ficha fija los datos que Root mide. Cada dato de la ficha tiene un id que la prosa puede citar.
La ficha es obligatoria: un spec sin `## Ficha`, o con un bloque que no parsea, hace que `r00t oraculo evaluar` salga con código 2 y sin veredicto.
El builder ve el spec y su ficha, porque la ficha es la promesa del producto. Por eso la ficha nunca nombra tests ocultos: la relación requisito→test vive en `tarea.toml`, en `ROOT_HOME`.

```toml
[producto]
usuarios = ["operador que corre memora como caché"]
problema = "..."
fuera_de_alcance = ["clúster", "TLS"]
[[producto.requisito]]
id = "R1"
titulo = "Protocolo RESP2"
peso = 3
aceptacion = "un cliente redis-py 5.x completa SET/GET de 10 000 claves sin error"   # medible: número, comando o salida exacta
# sin `pruebas`: los ids de tests ocultos van solo en tarea.toml

[tecnica]
lenguaje = "rust"
version_minima = "1.85"
dependencias_permitidas = ["tokio", "bytes"]
plataformas = ["linux-x86_64"]
[[tecnica.limite]]
id = "T1"
metrica = "ops_s"
minimo = 5000.0
unidad = "operaciones por segundo"       # se copia a [eficiencia] de tarea.toml

[arquitectura]
capas = [["protocolo"], ["comandos"], ["almacen"]]   # se copia a [arquitectura] de tarea.toml
persistencia = "AOF en disco, fsync cada segundo"
concurrencia = "un hilo por conexión, almacén con lock por shard"
[[arquitectura.interfaz]]
id = "A1"
modulo = "almacen"
publica = ["get", "set", "del", "expire"]

[diseno]
errores = "todo error de red responde -ERR <mensaje> y no corta otras conexiones"
logs = "a stderr, un evento por línea, nivel configurable"
configuracion = "archivo memora.conf; cada directiva documentada en memora.conf.example"
[[diseno.regla]]
id = "D1"
texto = "ningún unwrap() en código que procesa entrada de la red"
item = "diseno.robustez"                 # ítem de rúbrica que esta regla ancla
```

`r00t ficha tarea` arma un borrador de `tarea.toml` desde la ficha. El borrador deja `pruebas`, `bordes` y `escenarios` vacíos: Opus los completa en `ROOT_HOME`. El usuario aprueba los requisitos al aprobar la ficha, en un solo gate (decisión 12).

#### Rúbricas (Tarea 8b; decisión 12)

Toda dimensión se puntúa con una rúbrica. Cada ítem de la rúbrica tiene niveles anclados y cita el estándar del que sale. Root no inventa criterios.

| Dimensión | Estándar de referencia |
|---|---|
| Completitud, Funciona | ISO/IEC 25010:2023, idoneidad funcional (completitud, corrección, pertinencia). Trazabilidad requisito→prueba según ISO/IEC/IEEE 29148. |
| Seguridad | OWASP ASVS 5.0, nivel 1 (los capítulos que aplican al producto) y CWE Top 25 para los hallazgos de código. |
| Entrega | CIS Docker Benchmark, sección 4 (imágenes), y los chequeos locales de OpenSSF Scorecard (dependencias fijadas, SAST, workflows peligrosos). |
| Pruebas | ISO/IEC 25010, testeabilidad, medida con el puntaje de mutación. |
| Eficiencia | ISO/IEC 25010, eficiencia de desempeño (tiempo, recursos, capacidad) contra el `piso` de la tarea. |
| Estructura, Arquitectura, Diseño | ISO/IEC 25010, mantenibilidad (modularidad, reusabilidad, analizabilidad, modificabilidad, testeabilidad). En Rust, además, el checklist de Rust API Guidelines. |

Los textos de ISO tienen copyright. Las rúbricas citan solo el nombre y la cláusula de cada característica, y Root escribe sus propias anclas. ASVS y CWE tienen licencia abierta y se citan con su id.

```toml
# rubricas/diseno.toml — se compila dentro del binario (include_str!); su hash va en cada Veredicto
id = "diseno"
version = 1
[[item]]
id = "diseno.modularidad"
fuente = "ISO/IEC 25010:2023 §mantenibilidad/modularidad"
mide = "juez"                  # "sonda" si es checkeable; "juez" si no; "mixto" si la sonda fija un tope
peso = 3
bloquea = false                # true: nivel 0 en este ítem da Decision::Bloqueado (Tarea 13b)
anclas = [                     # nivel 0..=3; cada nivel exige evidencia citada
  "0: un módulo concentra varios requisitos sin límites; un cambio toca todo",
  "1: hay módulos, pero con dependencias cruzadas o ciclos",
  "2: cada requisito vive en un módulo con una interfaz clara; quedan acoples puntuales",
  "3: módulos con una responsabilidad, sin ciclos, con interfaces chicas y probadas",
]
```

```rust
// Extensión de la Tarea 5b para rúbricas (solo agrega):
pub struct NivelItem { pub item: String, pub nivel: u8 /*0..=3*/, pub evidencia: Vec<String>, pub fuente: Fuente }
// Puntaje suma `items: Vec<NivelItem>` con #[serde(default)]. Veredicto suma `rubricas: Vec<String>` ("diseno@<hash>").
```

El valor de una dimensión es la media ponderada de sus ítems: `Σ peso·nivel / (3·Σ peso)`.
Un ítem `sonda` lo fija una sonda. Un ítem `juez` lo fija el juez, y su nivel se descarta si su evidencia no pasa el gate. Un ítem `mixto` lo fija el juez, pero la sonda le pone un tope: con ciclos de módulos, `diseno.modularidad` no supera 1.
Dos veredictos con hashes de rúbrica distintos no se comparan: `r00t track comparar` lo rechaza.

La `Decision` sale de reglas deterministas, nunca de un LLM. Las reglas viven en `[decision]` de `root.toml` (Tarea 13b).
El LLM solo escribe `resumen`, `riesgos` y `proximos_pasos`. Cada oración debe citar un id existente: el gate de evidencia descarta las que no citan.

`global` es la media ponderada de las dimensiones con `calibrada = true`. Si ninguna dimensión está calibrada, `global` es `None`.
`Estado` se deriva de la `Decision` del informe (Tarea 13b): `Listo` da `Verde`; `NoListo` y `Bloqueado` dan `Rojo`; sin informe, `SinVeredicto`. Ningún conteo de tests decide el estado por sí solo (decisión 16).

### Funciones públicas

```rust
// route
pub fn elegir(p: &Pedido, pol: &Politica, uso: &Uso, b: &Balance) -> Result<Eleccion, SinPresupuesto>; // pura, sin I/O
// observe
pub fn uso(ahora: f64) -> Result<Uso, ErrorObserve>;   // Uso { cupo_bloque_usd, cupo_bloque_inicio, api_mes_usd, nube_mes_usd }
// oracle
pub fn evaluar(repo: &Path, tarea: &Tarea, o: &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo>;
pub fn calibrar(tarea: &Tarea, dim: Dimension, n: usize, semilla: u64) -> Result<Calibracion, ErrorOraculo>;
pub fn informar(v: &Veredicto, tarea: &Tarea, reglas: &ReglasDecision) -> Result<Informe, ErrorOraculo>;
pub fn validar(casos: &Path) -> Result<Validez, ErrorOraculo>;  // ¿el juicio de Root coincide con resultados conocidos?
// observe
pub fn registrar(ev: &Evento) -> io::Result<()>;                              // append con lock
pub fn leer_sesion_claude(json: &Path, corrida: &str) -> Result<Vec<Evento>, ErrorObserve>;
pub fn resumir(corrida: &str) -> Result<Resumen, ErrorObserve>;
// track
pub fn agregar(f: &Fila) -> io::Result<()>;
pub fn filas(filtro: &Filtro) -> Result<Vec<Fila>, ErrorTrack>;
pub fn comparar(a: &str, b: &str, tarea: &str) -> Result<Comparacion, ErrorTrack>;
pub fn estadisticas(rol: Rol, tipo: &str) -> Result<Vec<StatModelo>, ErrorTrack>; // n, éxitos, cota Wilson, USD medio
// aprende
pub fn recalcular(filas: &[Fila], eventos: &[Evento]) -> Politica;            // pura
// proveedores
pub fn llamar(e: &Eleccion, prompt: &Prompt, corrida: &str) -> Result<Respuesta, ErrorProveedor>; // registra el Evento
// goal
pub fn verificar_goal(repo: &Path) -> Result<(), ErrorGoal>;
```

### CLI

Con `--json`, todo subcomando imprime un solo objeto JSON en stdout.
Los códigos de salida son cuatro:

- `0` significa ok o verde.
- `1` significa rojo.
- `2` significa un error de uso.
- `3` significa un error de infraestructura, sin veredicto.

```
r00t route --rol <rol> --tipo <tipo> [--checkeable] [--tokens N]   -> Eleccion
r00t llama --rol <rol> --tipo <tipo> --prompt-file <f> --corrida <id> -> Respuesta
r00t oraculo evaluar --tarea <t> --repo <dir>                        -> Veredicto
r00t oraculo calibrar --tarea <t> --dimension <d> [--n 20] [--semilla 42] -> Calibracion
r00t oraculo informe --tarea <t> --repo <dir> [--md]                 -> Informe (JSON, o markdown legible)
r00t oraculo validar --casos <dir>                                   -> Validez
r00t observa claude --archivo <salida-de-claude-p> --corrida <id>   # --json queda como flag global
r00t observa evento --corrida <id> --tipo <humano|plan-aprobado|gate> [--detalle txt]
r00t observa resumen --corrida <id>                                  -> Resumen
r00t observa uso                                                     -> Uso y margen de cada tope
r00t observa claude --archivo <f> --corrida <id> --via nube          # registra una sesión de Claude en la nube
r00t track agregar --fila <archivo.json> | listar [--tarea t] | comparar --a c1 --b c2 --tarea t
r00t aprende [--replay]                                              -> Politica | métrica de replay
r00t corre --tarea <t> --config <c>                                  -> Fila
r00t goal verificar | autorizar        # autorizar exige una TTY y una confirmación tecleada
r00t importar-mmorch --desde <metrics.jsonl>
r00t esquema <tipo>                    # imprime el JSON Schema del tipo
r00t mcp                               # servidor MCP por stdio
r00t ficha revisar --spec <SPEC.md>                                  -> Revision (ambigüedades con cita)
r00t ficha tarea --spec <SPEC.md> --nombre <t>                       -> borrador de tarea.toml
r00t oraculo pendientes --tarea <t> --repo <dir>                     -> paquete para correr los tests pendientes en Colab (Tarea 7f)
```

### Herramientas MCP (reemplazan el ruteo del MCP `mmorch`)

`root_route(rol, tipo, checkeable)`, `root_llama(rol, tipo, prompt)`, `root_evaluar(tarea, repo)` y `root_registrar(corrida, tipo, detalle)`. Cada herramienta devuelve el mismo JSON que su subcomando.

### Uso previsto por hito

- **Graft (hito 2)** pide el modelo de cada etapa con `r00t route`. Usa las sondas del Oracle como gates y registra cada etapa con `r00t observa evento`.
- **REA (hito 3)** registra cada paso con `r00t observa evento`. El Oracle verifica su evidencia con el mismo chequeo de `archivo:línea` que aplica a los hallazgos del juez.
- **CAB (hito 4)** define una tarea con `tarea.toml` y mide cada agente generado con `r00t corre`.
- **ECC (decisión a)** recibe sus modelos por rol desde `r00t route`. Sus hooks que filtran datos se apagan por defecto en `r00t corre`.

## Files to Change

| File | Action | Why |
|---|---|---|
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | CREATE | Crate `root`: biblioteca y binario |
| `src/main.rs`, `src/lib.rs`, `src/cli.rs` | CREATE | Entrada y CLI (`clap`) |
| `src/config.rs`, `src/tipos.rs`, `src/errores.rs` | CREATE | `ROOT_HOME`, `root.toml` y tipos compartidos |
| `src/goal.rs` | CREATE | Hash del GOAL |
| `src/observe/{mod,claude,jsonl}.rs` | CREATE | Eventos, lector de `claude -p` y lock |
| `src/track/{mod,stats}.rs` | CREATE | Filas, comparación y cota de Wilson |
| `src/oracle/{mod,tarea,contenedor,junit,sondas,juez,evidencia,calibra}.rs` | CREATE | Evaluador de alto nivel |
| `src/oracle/configs.rs` | CREATE | Sondas de TOML, YAML y Dockerfile |
| `src/oracle/perfiles/{mod,rust,python}.rs` | CREATE | Sondas por lenguaje del proyecto evaluado |
| `src/oracle/{requisitos,entrega,mutacion,mapa,informe,validez}.rs` | CREATE | Las partes de "project manager" |
| `src/oracle/descarga.rs` | CREATE (Tarea 6b) | Fase de descarga de dependencias con red |
| `src/ficha.rs` | CREATE (Tarea 9f) | Formato y revisión de la ficha del spec |
| `$ROOT_HOME/oraculos/{memora,brasa}/bootstrap/docs/SPEC.md` | UPDATE (Opus redacta `## Ficha`, el usuario aprueba) | Fichas de las dos tareas del banco, dentro del spec (decisión 20) |
| `src/oracle/pendientes.rs` | CREATE (Tarea 7f) | Paquete de tests pendientes por entorno para Colab |
| `src/tipos.rs` | UPDATE (Tarea 5b) | Extensión PM y de rúbricas, solo agrega |
| `rubricas/{completitud,funciona,seguridad,entrega,pruebas,eficiencia,estructura,arquitectura,diseno}.toml` | CREATE (Opus redacta, el usuario aprueba) | Rúbricas con anclas y la cita de su estándar |
| `src/oracle/rubrica.rs` | CREATE | Carga, hash y puntaje de las rúbricas |
| `imagenes/oraculo-rust/Containerfile` | CREATE | Rust, clippy, cargo-audit, cargo-mutants, gitleaks, hadolint, Python y pytest |
| `src/proveedores/{mod,deepseek,anthropic}.rs` | CREATE | Llamadas a las APIs |
| `src/route/{mod,politica}.rs`, `src/aprende.rs` | CREATE | Ruteo y aprendizaje |
| `src/corre.rs`, `src/mcp.rs`, `src/importar.rs` | CREATE | Runner, servidor MCP e importación |
| `imagenes/oraculo-python/Containerfile` | CREATE (después de la de Rust) | Para proyectos Python: pytest, bandit, radon, mutmut, gitleaks, hadolint |
| `tests/*.rs`, `tests/fixtures/` | CREATE | Integración |
| `GOAL.borrador.md` | CREATE (Opus, ya escrito) | Borrador del GOAL para que el usuario lo lea |
| `GOAL.md`, `GOAL.hash` | — (solo el usuario) | El usuario copia el borrador a mano y corre `r00t goal autorizar` |
| `docs/interfaces.md` | CREATE | Copia de la sección "Interfaces fijas" para los hitos 2 a 4 |
| `docs/evidencia.md` | CREATE | Índice: "Measured" vale como evidencia heredada y su método está en revisión |
| `.githooks/pre-commit` | CREATE | `cargo fmt --check`, `clippy -D warnings`, `cargo test` |
| `.claude/prds/sistema-nuevo.prd.md` | UPDATE | Decisiones (a), (b) y hito 1 en `in-progress` |

## Tasks

Cada tarea es chica: un builder Haiku la termina en una sesión. Cada tarea escribe primero sus tests y nombra un comando que la valida. Las tareas 1 a 5 son deterministas y desbloquean el resto.

### Tarea 1: Esqueleto del crate
- **Action**: `cargo new --lib root` con un binario. Dependencias: `clap` (derive), `serde`, `serde_json`, `toml`, `yaml-rust2`, `thiserror`, `anyhow`, `sha2`, `fs4` (lock), `ureq`, `quick-xml`, `schemars`, `time`. Escribir `.githooks/pre-commit`.
- **Nota**: `serde_yaml` está archivado. Antes de fijar `yaml-rust2`, el builder verifica que tenga una versión publicada en los últimos 12 meses.
- **Mirror**: la tabla de errores y tests de "Patterns to Mirror".
- **Validate**: `cargo build && cargo clippy -- -D warnings && ./target/debug/r00t --help`

### Tarea 2: Config y tipos
- **Action**: `config.rs` resuelve `ROOT_HOME` y parsea `root.toml`. `tipos.rs` contiene todos los tipos de "Interfaces fijas". `r00t esquema <tipo>` imprime el JSON Schema.
- **Mirror**: los nombres de campo de `MetricEvent`.
- **Validate**: `cargo test tipos` (ida y vuelta JSON de cada tipo; `v` presente) y `r00t esquema veredicto | jq .`

### Tarea 3: GOAL con hash
- **Action**: `goal.rs` implementa `verificar` y `autorizar`. `autorizar` rechaza la ejecución sin TTY y pide teclear el hash. El builder no escribe `GOAL.md` ni `GOAL.hash`: el hook los bloquea. Los tests usan un GOAL de fixture en un directorio temporal.
- **Mirror**: `mmorch/goal.py:45-54`.
- **Validate**: `cargo test goal` (un GOAL alterado falla `verificar`; `autorizar` sin TTY sale con código 2; sin `GOAL.md`, `verificar` sale con código 3)
- **Gate humano** (antes de la Tarea 16):
  1. El usuario lee `GOAL.borrador.md` y lo corrige si hace falta.
  2. El usuario lo copia a mano a `GOAL.md`.
  3. El usuario corre `r00t goal autorizar` en su terminal.
- **Por qué `autorizar` exige TTY**: el hook bloquea `Write` y `Edit`, pero no bloquea un binario corrido por `Bash`. La sesión `claude -p` del builder no tiene TTY, así que no puede autorizar el hash.
- **Ya hecho**: las cuatro líneas de `~/.claude/never-edit.txt` y el hook de `.claude/settings.json` del worktree.

### Tarea 4: Observe
- **Action**: `registrar` agrega una línea con lock y rota el archivo a los 50 MB. `leer_sesion_claude` convierte la salida JSON de `claude -p` en eventos (`total_cost_usd` va a `usd_cupo`, `num_turns`, `modelUsage`, `duration_ms`). `resumir` cuenta como intervención cada evento `Humano` posterior a `PlanAprobado`. Un evento `OraculoLeido` marca la corrida como inválida.
- **Mirror**: el formato de `logs/metrics.jsonl`. Fixture: una línea real de `~/Proyectos/bench-ecc-sdlc/logs/sistema-ecc.jsonl`.
- **Validate**: `cargo test observe` (16 escritores concurrentes no producen líneas rotas; 2 mensajes humanos después de aprobar dan 2 intervenciones; 1 mensaje antes de aprobar da 0)

### Tarea 5: Track e importación
- **Action**: `agregar`, `filas`, `comparar` y `estadisticas` con la cota inferior de Wilson al 90%. `r00t importar-mmorch` copia `metrics.jsonl` a `importado/` y lo convierte a `Evento` con `extra.origen = "mmorch"`. Nunca escribe sobre el original.
- **Validate**: `cargo test track` (Wilson con 0/0, 5/5 y 50/100 contra valores tabulados); importar el `metrics.jsonl` real deja 575 eventos y el md5 del original sin cambios.

### Tarea 5b: Extensión PM de los tipos
- **Action**: agregar a `src/tipos.rs` los tipos de "Extensión PM". Agregar también `Fila.modelos: BTreeMap<Rol, String>` (el modelo de cada rol en la corrida) con `#[serde(default)]`. Así `estadisticas(rol, tipo)` agrupa por `modelos[rol]`. Solo agrega: las variantes nuevas de `Dimension` van al final, y los campos nuevos llevan `#[serde(default)]`. Corre después de que Sonnet cierre las Tareas 1 a 5.
- **Validate**: `cargo test tipos` (un `Veredicto` v1 sin `informe` se sigue leyendo; ida y vuelta JSON de `Informe`) y `r00t esquema informe | jq .`. Incluye `NivelItem` y los campos `items` y `rubricas`.

### Tarea 5c: Ajustes a las Tareas 1 a 5 (desempates de Opus)
- **Action**: cinco cambios chicos sobre el código ya construido. El detalle de cada uno está en "Desempates técnicos":
  - El binario pasa a llamarse `r00t` con `[[bin]] name = "r00t"`. El crate sigue siendo `root`.
  - `resumir` y `filas` leen los archivos rotados `.N` y después el archivo vigente, con un lector común en `observe/jsonl.rs`.
  - `leer_sesion_claude` emite un `Evento` por cada modelo de `modelUsage`, con el `costUSD` y los tokens de ese modelo. Emite además un `Evento` `Sesion` con turnos y duración, y con `cost_usd = 0`, para no contar el costo dos veces.
  - Se activa la feature `float_roundtrip` de `serde_json`.
  - `observa claude --salida` pasa a `--archivo`.
- **Validate**: `cargo test` pasa (los 53 tests actuales y estos nuevos):
  - Un evento escrito antes de rotar cuenta en `resumir`.
  - Un `modelUsage` con Sonnet y Haiku da 2 eventos, y la suma de sus costos es igual a `total_cost_usd`.
  - Un `f64` sale igual tras una ida y vuelta por JSON.
  - `./target/debug/r00t --help` funciona.

### Tarea 6: Contenedor y junit
- **Action**: `oracle/contenedor.rs` arma el `podman run` de la sección "Cómo corre el oráculo". Aplica un timeout. Un error de infraestructura devuelve `ErrorOraculo::Infra` y nunca un veredicto. `oracle/junit.rs` cuenta pass, fail y skip por archivo.
- **Mirror**: `evaluar.py:17-50`.
- **Validate**: `cargo test --test contenedor` (un comando que escribe fuera de `/salida` falla; con `--network none`, `curl` falla; un junit con 3 tests y 1 falla da 2/3)

### Tarea 6b: Fase de descarga de dependencias (decisión 17)
- **Action**: `oracle/descarga.rs` corre antes de cada evaluación, en un contenedor aparte con red:
  - Monta el repo copiado en `/repo:ro,z` y `$ROOT_HOME/cache/<lenguaje>/` en escritura. No monta `/oraculo`.
  - En Rust corre `cargo fetch --locked`. En Python corre `pip download -r requirements.txt --index-url https://pypi.org/simple`. No corre builds ni scripts de compilación.
  - Antes de bajar nada, lee `Cargo.lock`. Una fuente que no es crates.io, como `git+...` o un registro privado, se rechaza: el resultado es `SinVeredicto` más un hallazgo de entrega. Hasta la aclaración P12b, rigen solo crates.io y PyPI.
  - En Rust baja también la base de avisos de RustSec a `$ROOT_HOME/cache/rustsec/`. La evaluación corre `cargo audit --no-fetch --db /cache/rustsec` sin red (Tarea 9). La base no se precarga en la imagen.
  - Sin `Cargo.lock`, corre `cargo generate-lockfile` y deja un hallazgo de entrega: el build no es reproducible.
  - Registra un evento `Gate` con la lista de paquetes bajados y sus fuentes (invariante de observabilidad).
  - Un registro que no responde da `SinVeredicto` con causa "infraestructura". Un paquete que no existe o fue retirado da un hallazgo de entrega: el producto no instala.
  
  La evaluación monta la caché en solo lectura, con `--network none` y `CARGO_NET_OFFLINE=true`.
- **Límite**: el filtro de fuentes depende del comando, no de la red. El contenedor de descarga ve toda la red, pero solo corre `cargo fetch` y `pip download`. Un filtro por dominio necesita un proxy y queda fuera del hito 1.
- **Huecos del filtro** (reporte de Sonnet después de la 6b; esperan a P12b):
  - (a) Un `Cargo.toml` con `git =` y sin `Cargo.lock` hace que `cargo generate-lockfile` clone esa fuente antes del filtro.
  - (b) Un `requirements.txt` con una URL o con `git+` pasa el filtro de PyPI.
  
  P12b quedó en b) (decisión 21). La Tarea 6c rediseña el filtro y cierra los dos huecos de otra forma: git y URL pasan a ser fuentes aceptadas, pero Root las lista y las registra antes de cualquier comando con red.
- **Validate**: `cargo test --test descarga`:
  - Un repo con una dependencia de crates.io fuera de la imagen compila en la evaluación sin red.
  - Un `Cargo.lock` con `git+` da `SinVeredicto` y no baja nada.
  - El contenedor de descarga no ve `/oraculo`.
  - La segunda evaluación del mismo repo no vuelve a bajar nada: usa la caché.
- **Orden**: va después de las Tareas 7 y 8. La Tarea 7 sigue con la imagen precargada de la 7a.

### Tarea 6c: Filtro de fuentes con git y otros registros (decisión 21)
- **Qué cambia**: la descarga acepta crates.io, PyPI, repositorios git y otros registros públicos. Lo que no cambia es la regla central de la 6b: la descarga nunca corre código del proyecto ni de sus dependencias, y la evaluación sigue sin red.
- **Action**: `oracle/descarga.rs` pasa a tres pasos:
  1. **Inventario, sin red.** Root lee `Cargo.toml` (con los miembros del workspace), `Cargo.lock` y `requirements.txt`, y arma la lista de fuentes. Cada fuente lleva su tipo (`registro`, `git` o `url`), su dirección y su versión o commit. Root registra esa lista en un evento `Gate` antes de usar la red. Así se cierran los dos huecos de la 6b: nada sale a la red sin quedar en la lista.
  2. **Reglas del inventario.** Root rechaza cuatro casos antes de usar la red, con `SinVeredicto` y un hallazgo de entrega:
     - un protocolo que no es `https`, como `ssh://`, `git://`, `http://` o `file://`;
     - una dependencia `path =` que apunta fuera del repo;
     - una dirección con credenciales adentro, como `https://usuario:clave@...`;
     - un registro que pide credenciales. Pasar credenciales al contenedor es zona roja y espera a P21.
     Una dependencia git sin commit fijo no se rechaza: Root baja la punta de la rama, registra el commit que obtuvo y deja un hallazgo de entrega, porque el build no es reproducible.
  3. **Descarga, sin ejecutar nada.**
     - Rust: `cargo fetch` baja crates y repositorios git. `cargo fetch` no corre `build.rs`. Sin `Cargo.lock`, `cargo generate-lockfile` corre después del inventario.
     - Python desde un índice: `pip download --only-binary=:all:`. Un paquete sin wheel no pasa por `pip`, porque `pip` correría su `setup.py` para leer los metadatos. Root baja ese sdist con `curl` desde la URL que da el índice y verifica su hash. Las dependencias de un sdist no se resuelven en la descarga: si falta alguna, la instalación sin red falla y deja un hallazgo de entrega ("requirements incompleto").
     - Python desde git o URL: Root clona con `git clone` al commit pedido, o baja el archivo con `curl`. No corre `pip` sobre esa fuente con red. La evaluación lo instala sin red, con `pip install --no-index --find-links /cache/python`.
  Después de bajar, Root registra otro evento `Gate` con el hash de cada fuente obtenida. Cuando el lock trae hash (`checksum` de Cargo, `--hash` de pip), Root lo verifica.
- **npm y otros ecosistemas**: el hito 1 tiene perfiles solo para Rust y Python. Una fuente npm se acepta en el inventario, pero no se baja hasta que exista un perfil de ese lenguaje.
- **Límite**: igual que en la 6b, el filtro depende del comando y no de la red. Un filtro por dominio necesita un proxy y queda fuera del hito 1.
- **Validate**: `cargo test --test descarga`:
  - Un `Cargo.toml` con `git = "https://..."` y sin `Cargo.lock` registra el `Gate` del inventario antes de cualquier comando con red.
  - Una dependencia git por `https` con commit fijo se baja, y la evaluación compila sin red.
  - Una dependencia `ssh://` da `SinVeredicto` y no baja nada.
  - Una dirección con credenciales da `SinVeredicto` y la credencial no aparece en `eventos.jsonl`.
  - Una dependencia `path = "../afuera"` da `SinVeredicto`.
  - Un `requirements.txt` con `git+https://...@<commit>` se clona sin correr `pip` con red, y la evaluación lo instala sin red.
  - Un sdist de fixture cuyo `setup.py` escribe un archivo al ejecutarse se baja, y ese archivo no aparece después de la descarga.
  - Una dependencia git sin commit fijo da un hallazgo de entrega con el commit obtenido.

### Tarea 7: Tareas memora y brasa
- **Action**: crear `oraculos/memora` y `oraculos/brasa` desde `~/Proyectos/bench-ecc-sdlc/bootstrap/` y `.scratch/ecc-vs-sdlc/oraculo/`. Construir `imagenes/oraculo-rust`: los dos proyectos son Rust y sus oráculos son pytest de caja negra (`ORACULO_REPO`, ver `oraculo/memora/conftest.py`). brasa necesita un adaptador de CPU (lavapipe) en la imagen: ver `~/Proyectos/bench-ecc-sdlc/probe-gpu/`. Los oráculos se copian a `ROOT_HOME`, nunca al repo de Root.
- **Requisitos**: Opus redacta `[[requisito]]` y `[[escenario]]` de cada tarea desde la spec. Cada sección de la spec da al menos un requisito. Los tests ocultos `test_m<n>_*.py` ya siguen los módulos de la spec, así que cada requisito apunta a sus archivos. El usuario aprueba los requisitos al aprobar la ficha (decisión 20, Tarea 9f). La relación requisito→test es material oculto: vive en `tarea.toml`, en `ROOT_HOME`, no en el repo.
- **Qué mide esta tarea** (decisión 16): que el instrumento del PM sea confiable. No mide que el Oracle repita los conteos del banco. Esos conteos vienen de una sola corrida en el host, así que también son una muestra.
- **Repeticiones**: `tarea.toml` suma `repeticiones = 3`. El Oracle corre la suite oculta esa cantidad de veces y guarda la tasa de éxito de cada test. Un test con resultados mezclados queda `inestable`. `Veredicto` suma, solo agregando, `repeticiones: u32` e `inestables: Vec<String>`.
- **Causa de cada diferencia**: cada test que difiere del banco se clasifica con evidencia en una de cuatro causas:
  - `producto`: falla en todas las repeticiones. Es un hallazgo con impacto en su requisito.
  - `inestable`: resultados mezclados. Es un hallazgo de "comportamiento no determinista" en su requisito. El origen queda "producto", "test" o "sin determinar", según la evidencia.
  - `entorno`: el mismo commit pasa en el host y falla en el contenedor, por ejemplo por lavapipe contra GPU. Va a `[[entorno]]` de `tarea.toml` con su motivo y no se cobra al producto. El informe lo muestra como `Pendiente` (decisión 19, Tarea 7f).
  - `oraculo`: el test depende de una frase ambigua de la spec, como el orden de `spin` (ticket 08). Va a `[[oraculo_dudoso]]` con su motivo y tampoco se cobra al producto.
  Las causas y su evidencia quedan en `oraculos/<tarea>/diferencias.md`.
- **Validate** (criterio del PM, no del compilador):
  - memora-ecc y brasa-ecc reciben el mismo nivel en cada requisito en 3 evaluaciones seguidas.
  - Cada diferencia contra los números del banco tiene una causa y una evidencia en `diferencias.md`. Ninguna queda "sin diagnosticar".
  - memora-ecc queda arriba de memora-sdlc en completitud.
  - Ya no se exige un conteo exacto como 61/61 o 48/48.

### Tarea 8: Proveedores
- **Action**: DeepSeek (API compatible con OpenAI) y Anthropic Messages. Reintentos y breaker como en `providers.py`. El costo sale de los precios de `root.toml`. Cada llamada registra un `Evento`. Las claves salen de variables de entorno y nunca entran a un log.
- **Validate**: `cargo test proveedores` con un servidor HTTP falso (429 reintenta; 3 fallas abren el breaker; la clave no aparece en `eventos.jsonl`)

### Tarea 8c: Ajustes a la Tarea 8 (desempates 4 y 5 de "Desempates técnicos 2")
- **Action**: dos cambios en `proveedores`:
  - Solo una falla transitoria cuenta para el breaker: 5xx, 429 con los reintentos agotados, timeout y error de conexión. Un 400, 401, 403, 404 o 422 no se reintenta y no cuenta. Un 401 o 403 da `ErrorProveedor::Auth`; un 400, 404 o 422 da `ErrorProveedor::Pedido`. Los dos salen con código 3.
  - Si la llamada tuvo éxito y `registrar` falla, `llamar` devuelve `ErrorProveedor::EventoNoRegistrado { respuesta, evento, causa }`. `r00t llama` imprime la respuesta en stdout, la línea JSON del `Evento` en stderr, y sale con código 3.
- **Validate**: `cargo test proveedores`:
  - Cinco 401 seguidos no abren el breaker, y cada uno hace un solo pedido al servidor falso.
  - Tres 503 seguidos abren el breaker.
  - Con `eventos.jsonl` sin permiso de escritura, una llamada exitosa da `EventoNoRegistrado` con la respuesta completa adentro.

### Tarea 7f: Repeticiones, pendientes por entorno y directorio temporal (desempates 3 y 6; decisión 19)
- **Action**: cuatro cambios sobre la Tarea 7:
  - **Repeticiones**: `repeticiones` pasa de 3 a 5 y vale para el contrato y para el oculto. Un test con resultados mezclados se vuelve a correr solo, hasta 10 corridas en total. Root guarda su tasa con la cota de Wilson al 90%. `tarea.toml` suma `repeticiones_inestable = 10`.
  - **Pendiente**: un test de `[[entorno]]` no entra al nivel de su requisito y figura como `Pendiente` con su motivo. Un requisito cuya evidencia está toda en `[[entorno]]` queda `Cumplimiento::Pendiente`. `Pendiente` no baja la `Completitud` ni dispara una regla de `Decision`. El informe lo muestra junto a la decisión, en `confianza`.
  - **Colab**: `r00t oraculo pendientes` arma un directorio con el repo, los tests pendientes y un script `correr.sh` que escribe junit. Root no lanza Colab: el usuario lo corre, o aprueba que un agente lo corra (P15). `r00t oraculo pendientes --importar <junit>` registra el resultado como evidencia de origen `colab`, con un evento `Gate`.
  - **Temporal**: `evaluar` crea sus directorios temporales en `$ROOT_HOME/tmp/` con `tempfile::Builder::tempdir_in`, nunca en `TMPDIR` ni en `/tmp`. Al arrancar, borra los directorios de `$ROOT_HOME/tmp/` con más de 24 horas. Un disco sin espacio da `ErrorOraculo::Infra`.
- **Validate**: `cargo test oracle`:
  - Un test falso con tasa 0,5 queda `inestable` y llega a 10 corridas.
  - Un test estable corre 5 veces y no más.
  - El contrato también corre 5 veces.
  - Un requisito con su único test en `[[entorno]]` da `Pendiente`, y la `Completitud` es la misma que sin ese requisito.
  - Con `TMPDIR=/nada`, `evaluar` funciona y no escribe fuera de `$ROOT_HOME/tmp/`.

### Tarea 7g: Herramientas en la imagen `oraculo-rust` (decisión 23)
- **Action**: `imagenes/oraculo-rust/Containerfile` instala cinco herramientas. Construir la imagen usa la red una vez; evaluar sigue sin red.
  - `clippy` con `rustup component add`, en la misma versión de Rust de la imagen.
  - `cargo-audit` y `cargo-mutants` con `cargo install --locked --version <fija>`.
  - `gitleaks` y `hadolint` como binarios de sus releases, con versión fija y `sha256sum -c` contra el hash publicado.
  El Containerfile anota la versión y la fuente de cada herramienta. El tag de la imagen sube a `root-oraculo-rust:2`, y las tareas memora y brasa lo usan.
- **Límite**: la instrucción anterior "no precargues más dependencias" sigue vigente para las dependencias de los proyectos. Estas cinco herramientas son la excepción que autorizó el usuario. La base de RustSec sigue bajando en la fase de descarga.
- **Validate**:
  - `podman run --rm --network none root-oraculo-rust:2 sh -c 'cargo clippy --version && cargo audit --version && cargo mutants --version && gitleaks version && hadolint --version'` sale con 0.
  - `cargo test --test sondas` corre las sondas con herramientas sobre los fixtures en la imagen real, y ninguna da `SinVeredicto` por herramienta ausente.

### Tarea 7h: Tests pendientes en Colab con T4 (decisión 22)
- **Action**: `r00t oraculo pendientes --colab` corre el paquete de la Tarea 7f en Colab con la CLI `colab` (`~/.local/bin/colab`).
  - Antes de empezar, Sonnet lee `colab --help` y confirma que la CLI puede pedir una T4. Si no puede, la tarea se detiene y vuelve a Opus.
  - Pide un runtime con GPU T4. Antes de correr los tests, ejecuta `nvidia-smi` en el runtime. Si la GPU no es una T4, libera el runtime y sale con código 3, sin correr nada. Root nunca pide una A100 ni una H100.
  - Sube el paquete, corre `correr.sh`, baja el junit y libera el runtime. Después corre el mismo camino que `--importar`.
  - Registra un evento `Gate` al pedir el runtime, con el tipo de GPU, y otro al liberarlo.
  - Sin sesión de la CLI, sale con código 3 y pide al usuario que corra el login. El login es OAuth y lo hace el usuario. El chequeo es el primer paso real, con stdin cerrado y un timeout de 60 segundos (Tarea 7i).
  - La forma de instalar Rust en el runtime espera a P35. Hasta la respuesta, no hay corrida real.
  - El paquete lleva los tests ocultos a Colab. El builder no accede a Colab, así que el invariante del oráculo oculto se mantiene. El paquete nunca lleva claves de API.
- **Validate**:
  - `cargo test oracle` con una CLI `colab` falsa: una GPU que no es T4 da código 3 y no corre los tests; una T4 importa el junit y deja dos eventos `Gate`.
  - Una corrida real sobre `test_rendimiento[gpu-10000-res1]` de brasa, después de G1, cambia ese test de `Pendiente` a un resultado con origen `colab`.

### Tarea 8b: Rúbricas
- **Action**: Opus redacta las nueve rúbricas de "Rúbricas", con 3 a 6 ítems cada una. Cada ítem cita su estándar. El usuario las aprueba una vez antes de que el Oracle las use, igual que los requisitos. `oracle/rubrica.rs` las compila dentro del binario, calcula su hash y aplica la fórmula del puntaje y los topes de los ítems `mixto`.
- **Validate**: `cargo test rubrica` cubre cuatro casos:
  - Un ítem sin `fuente` no carga.
  - Un nivel fuera de 0..=3 no carga.
  - Cambiar un ancla cambia el hash.
  - `comparar` rechaza dos rúbricas con hashes distintos.
- **Gate humano**: el usuario aprueba `rubricas/*.toml` antes de la Tarea 12.

### Tarea 9: Sondas deterministas por perfil de lenguaje
- **Action**: `oracle/perfiles/` elige las sondas por `lenguaje`. El perfil `rust` va primero porque memora y brasa son Rust. El perfil `python` va después.
  - **seguridad**: gitleaks, `cargo audit --no-fetch` con la base de RustSec de la Tarea 6b (dependencias con CVE; sin la base, `SinVeredicto`), conteo de `unsafe` y de `unwrap()`/`expect()` en código que no es de test. En Python: bandit.
  - **estructura**: tamaño de archivos y funciones, duplicación y `cargo clippy` (solo las categorías `correctness`, `suspicious` y `perf`; el estilo no cuenta). En Python: radon.
  - **arquitectura**: grafo de módulos desde `mod`/`use` (o `import`), ciclos y reglas de `capas`.
  - **eficiencia**: el comando `[eficiencia]` contra `piso`.
  Cada sonda devuelve `Puntaje` y `Hallazgo`s con `id`, `archivo:línea` e `impacto` (requisitos afectados, por la ruta del archivo).
- **Regla anti pseudo-compilador**: un hallazgo sin impacto en un requisito y con severidad 1 no baja el puntaje: solo se lista. El puntaje pondera por severidad y por el `peso` del requisito afectado, nunca por conteo bruto de avisos.
- **Validate**: `cargo test --test sondas` sobre fixtures Rust y Python con un defecto conocido por sonda; un fixture con 200 avisos de estilo y 0 defectos reales obtiene el puntaje máximo.

### Tarea 9c: Completitud por requisito
- **Action**: `oracle/requisitos.rs` cruza cada `[[requisito]]` con el junit oculto y los escenarios. Cada requisito recibe un nivel de la rúbrica `completitud`, y el nivel sale solo de pruebas, sin LLM:
  - **0 Ausente**: ningún test verde.
  - **1 Parcial**: algunos tests de `pruebas` verdes.
  - **2 Caso feliz**: todos los tests de `pruebas` verdes, pero falta algún borde o escenario.
  - **3 Cumplido**: todo `pruebas`, `bordes` y `escenarios` en verde, en todas las repeticiones.
  
  Un requisito con un test `inestable` no supera el nivel 2, y su hallazgo entra al informe. Un test de causa `entorno` se excluye del cálculo y figura como `Pendiente`, con su motivo (Tarea 7f). Un test de causa `oraculo` se trata igual solo hasta que exista la ficha aclarada (Tarea 9f).
  
  El puntaje de `Completitud` es `Σ peso·nivel / (3·Σ peso)`. Los requisitos salen de la spec, y la spec sale de una sesión `/grill-me` con el usuario (decisión 12). En memora y brasa, la spec ya existe en el banco.
- **Validate con fixtures** (no espera a G1): un requisito con su único test en `[[entorno]]` da `Pendiente`, y la `Completitud` es la misma que sin ese requisito. Este caso viene de la Tarea 7f, que no tenía `[[requisito]]` para probarlo.
- **Validate** (criterio corregido el 2026-10-30, ficha aprobada en G1): `r00t oraculo evaluar --tarea memora --repo .../memora-sdlc` marca `Ausente` los módulos que el SDLC no construyó (el SDLC construyó 2 de 9 módulos, según `docs/contexto/mapa.md`). memora-ecc queda con todos `Cumplido`, salvo R7, que queda `Parcial` por el defecto FIFO de `BLPOP`: la ficha aprobada exige FIFO (decisión 26).

### Tarea 9d: Entrega y escenarios de usuario
- **Action**: `oracle/entrega.rs` clona el repo en un contenedor limpio y corre `[entrega].instalar` y `[entrega].correr`. Verifica `[entrega].archivos`. Construye el `Dockerfile` del proyecto y lo arranca, y verifica que corre sin root. La construcción sigue la Tarea 9d2 (decisión 24). Después corre cada `[[escenario]]` contra el producto levantado. Un escenario es un flujo real de usuario, como conectar un cliente, guardar un valor, reiniciar y leerlo. Es lo que un PM prueba a mano antes de aprobar.
- **Validate**: `cargo test --test entrega` (un repo sin README da un hallazgo; un Dockerfile que corre como root da un hallazgo; un escenario que sale con código 1 deja su requisito en `Parcial`)

### Tarea 9d2: Construir el Dockerfile sin red (decisión 24, parte c)
- **Action**: `oracle/entrega.rs` construye el `Dockerfile` del proyecto en dos pasos:
  1. Con red, en la fase de descarga, Root lee cada `FROM` y corre solo `podman pull` de esa imagen. No corre ningún `RUN`. Registra la imagen y su digest en el evento `Gate` de la descarga.
  2. Sin red, `podman build --network none --pull=never` construye la imagen con el repo copiado como contexto. Después la arranca con `--network none`, verifica que corre sin root y corre los `[[escenario]]` que no necesitan red.
  - Un `RUN` que falla por falta de red deja la construcción `Pendiente`, con el motivo y la línea del Dockerfile. No se cobra al producto.
  - Un `RUN` que falla por otra causa da un hallazgo de entrega: la imagen no construye.
  - Para separar las dos causas, Root busca en la salida los errores de resolución de nombres y de conexión. Sin esa evidencia, la causa es "producto".
- **Pendiente de P17b**: el plan no construye con red. La parte b) de la respuesta espera la aclaración.
- **Validate**: `cargo test --test entrega`:
  - Un Dockerfile `FROM` con `COPY` y sin `RUN` de red construye y arranca sin root.
  - Un Dockerfile con `RUN apt-get install` queda `Pendiente` con su línea.
  - Un Dockerfile con `RUN false` da un hallazgo de entrega.
  - El paso con red no ejecuta ningún `RUN`: un `RUN touch /marca` no deja la marca en la imagen base bajada.

### Tarea 9e: Calidad de las pruebas del proyecto
- **Action**: `oracle/mutacion.rs` corre `cargo-mutants` (o `mutmut` en Python) sobre los tests del propio proyecto. Respeta `[pruebas].mutantes_max` y `minutos_max`. El puntaje de `Pruebas` es la proporción de mutantes muertos. Un mutante vivo da un hallazgo: "este cambio de comportamiento no lo detecta ningún test".
- **Mirror**: el veredicto ECC-F1 usó mutaciones para separar tests reales de tests decorativos (`docs/contexto/veredicto-ecc-f1.md`).
- **Validate**: `cargo test --test mutacion` (un fixture con tests sin asserts obtiene un puntaje menor que el mismo fixture con asserts)

### Tarea 9b: Sondas de TOML, YAML y Dockerfile
- **Action**: `oracle/configs.rs` busca los archivos `*.toml`, `*.yaml`, `*.yml`, `Dockerfile` y `Containerfile` del repo evaluado. `yaml-rust2` se usa solo en este archivo: Root nunca lee su propia configuración en YAML.
  - **TOML y YAML**: un error de parseo da un `Hallazgo` de estructura con `archivo:línea`. Una clave con forma de secreto (`password`, `token`, `api_key` con valor literal) da un `Hallazgo` de seguridad.
  - **Dockerfile**: hadolint corre en el contenedor de evaluación. Root agrega tres reglas propias de seguridad: imagen base sin versión fija (`:latest` o sin tag), ausencia de `USER` (corre como root) y un secreto en `ENV` o `ARG`.
  - Un repo sin estos archivos no recibe penalización.
- **Mirror**: el formato `Puntaje` + `Hallazgo` de la Tarea 9.
- **Validate**: `cargo test --test configs` (un YAML con tabulación inválida da un hallazgo con la línea exacta; un Dockerfile `FROM python:latest` sin `USER` da 2 hallazgos; un Dockerfile correcto da 0)
- **Calibración**: la Tarea 12 suma los tres defectos de Dockerfile y un secreto en YAML a su conjunto de mutaciones.

### Tarea 9f: Ficha del spec y revisión de ambigüedades (decisiones 18 y 20)
- **Action**: `ficha.rs` extrae el bloque TOML de la sección `## Ficha` de `SPEC.md`, y `r00t ficha revisar` la revisa como un PM revisa un spec antes de construir. La ficha es obligatoria: `evaluar` sin ficha válida sale con código 2. Hay dos capas.
  - **Reglas deterministas**:
    - Están las cuatro partes.
    - Cada requisito tiene `aceptacion`, y la aceptación incluye un número con unidad, un comando, una salida exacta o un id de test.
    - No aparece `TBD`, `etc.`, `...`, ni palabras vagas sin número: "rápido", "adecuado", "robusto", "escalable", "fácil", "suficiente".
    - Cada capa de `[arquitectura]` existe como módulo en el mapa del repo una vez construido.
    - Cada `[[diseno.regla]]` apunta a un ítem de rúbrica existente.
    - Cada `[[tecnica.limite]]` tiene unidad.
  - **Juez**: busca frases con dos lecturas posibles, como el orden de `spin` de brasa. Cada ambigüedad cita la frase exacta y las dos lecturas. El gate de evidencia descarta las que no citan.
  
  `r00t ficha tarea` arma el borrador de `tarea.toml`: `[[requisito]]` desde `[[producto.requisito]]`, `[eficiencia]` desde `[[tecnica.limite]]` y `[arquitectura]` desde `[arquitectura].capas`.
- **Efecto en la evaluación**: un requisito con una ambigüedad abierta queda "no evaluable" y figura en `Informe.confianza` con la cita. La salida no es excluir el test: se aclara la ficha y se vuelve a evaluar. La causa `oraculo` de la Tarea 7 deja de existir una vez que la ficha se aclara.
- **memora y brasa**: Opus redacta sus fichas desde las specs del banco. Cada ambigüedad que encuentre la revisión pasa al usuario como pregunta, y su respuesta va a la ficha. Si un test contradice la ficha aclarada, se corrige el test con la aprobación del usuario.
  Las dos preguntas de spec que Sonnet ya hizo (el orden FIFO de `BLPOP` en memora y "espacio local" en el `spin` de brasa) entran por este camino: el usuario las responde y la respuesta va a la ficha.
- **Gate humano**: el usuario aprueba las fichas de memora y brasa. Ese gate reemplaza la aprobación separada de requisitos (P3). Sin fichas aprobadas, la ficha obligatoria impide evaluar memora y brasa.
- **Calibración**: la Tarea 12 suma ambigüedades inyectadas en una ficha limpia: una aceptación sin número, una palabra vaga, una frase con dos lecturas. Así mide la sensibilidad y la especificidad de `r00t ficha revisar`.
- **Validate**: `cargo test ficha`:
  - Una ficha completa da 0 hallazgos.
  - "responde rápido" sin número da 1 hallazgo con su línea.
  - Un requisito sin `aceptacion` da 1 hallazgo.
  - Un `SPEC.md` sin `## Ficha` hace que `r00t oraculo evaluar` salga con código 2.
  - Una ficha con un campo `pruebas` da 1 hallazgo: la ficha no nombra tests ocultos.
  - `r00t ficha tarea` sobre una ficha de fixture da un `tarea.toml` que `r00t oraculo evaluar` acepta. La misma prueba sobre memora espera su ficha aprobada.
- **Transición**: la obligación entra con esta tarea. Las tareas de antes (7f, 6b) siguen evaluando sin ficha hasta que se construya la 9f.

### Tarea 10: Juez de diseño y arquitectura
- **Action**: el juez evalúa solo lo que ninguna sonda cubre: si el diseño y la arquitectura sirven a la spec. El juez no revisa estilo. Recibe la spec, los requisitos, el mapa del repo de `oracle/mapa.rs` (módulos, API pública, grafo de dependencias, tamaños) y los hallazgos de las sondas. Después abre los archivos que el mapa señala. El juez puntúa cada ítem `juez` o `mixto` de las rúbricas de diseño, arquitectura y estructura, con un nivel de 0 a 3. Cada nivel cita la evidencia que exige su ancla. Las anclas responden lo que un PM técnico pregunta:
  - ¿Qué requisito queda difícil de extender o de mantener con esta estructura?
  - ¿Qué decisión de diseño es un riesgo para producción, como un estado global, un bloqueo o un error que se traga?
  - ¿Qué falta para operar el producto: logs, configuración, manejo de errores?
  Cada hallazgo exige `archivo`, `línea`, `cita` e `impacto`. Un hallazgo sin requisito afectado se descarta. Usa `r00t route --rol juzga`, con razonamiento prendido. Refuta por default. El modelo del juez lo decide la calibración y no este plan: la Tarea 12 compara `deepseek-reasoner` contra Sonnet u Opus por cupo, y Route elige con lo medido.
- **Mirror**: los invariantes "checkeable va a un oráculo" y "el verificador razona y refuta".
- **Validate**: `cargo test juez` con un proveedor falso (un JSON roto da `SinVeredicto`, nunca verde)

### Tarea 11: Gate de evidencia
- **Action**: `oracle/evidencia.rs` descarta todo hallazgo cuya `cita` no aparezca en `archivo` cerca de `línea` (±3 líneas, sin contar espacios). Este gate también lo usa REA en el hito 3.
- **Validate**: `cargo test evidencia` (una cita inventada se descarta; una cita con otra indentación se acepta)

### Tarea 12: Calibración (mide al evaluador)
- **Action**: `r00t oraculo calibrar` parte de un repo conocido como bueno, por ejemplo memora-ecc. Inyecta en Rust defectos por construcción que un PM consideraría graves:
  - un secreto en el código;
  - un `unwrap()` en la ruta que parsea la entrada de la red;
  - un bucle O(n²) en el camino caliente;
  - un ciclo entre capas;
  - un archivo dios;
  - un módulo de la spec borrado (completitud);
  - un error que se traga con `let _ =`;
  - un test cuyo assert se borró (pruebas);
  - un Dockerfile que corre como root (entrega). Cada caso alterado tiene etiqueta "defecto". Cada copia sin alterar tiene etiqueta "limpio". El resultado es la sensibilidad, la especificidad y la exactitud balanceada por dimensión, en `calibracion.json`. Con exactitud balanceada mayor o igual a 0,9, la dimensión queda `calibrada`.
- **Mirror**: `gates_bench`, donde la etiqueta sale de la construcción.
- **Validate**: `r00t oraculo calibrar --tarea memora --dimension seguridad --n 20 --semilla 42` escribe una fila. Con la misma semilla, el resultado se repite igual.

- **Por ítem**: cada defecto inyectado apunta a un ítem de rúbrica, por ejemplo `seguridad.secretos` o `diseno.modularidad`. La calibración informa la exactitud balanceada por ítem además de por dimensión.
- **Límite**: la calibración prueba que Root **detecta** defectos conocidos. No prueba que Root **juzga bien** un proyecto real. Para eso está la Tarea 12b.

### Tarea 12b: Validez (¿Root juzga como un PM competente?)
- **Action**: `r00t oraculo validar` corre Root sobre casos cuyo resultado se conoce por una fuente externa a Root. Ni la regla evaluada ni un LLM ponen la etiqueta. Hay tres fuentes:
  - **Orden conocido**: memora-ecc contra memora-sdlc, brasa-ecc contra brasa-sdlc, y commits viejos contra commits nuevos del mismo repo. Root debe ordenar cada par como lo ordena el resultado medido.
  - **Defectos reales conocidos**: los 4 que la revisión de Opus encontró en F1 y no encontró ECC (`docs/contexto/veredicto-ecc-f1.md`), los 3 bugs reales del code-review de ECC en F1, y los defectos registrados en los logs del banco. Root debe encontrar cada uno con su `archivo:línea`. Esto mide el recall sobre problemas reales, no inyectados.
  - **Juicio humano** (decisión 12): `r00t oraculo validar --preparar-ciego` arma 6 paquetes: memora y brasa construidos por ECC con Sonnet, ECC con Haiku y el SDLC. Cada paquete es una copia anónima, sin `AGENTS.md`, `sdlc.toml`, logs, rastros de ECC ni autor de git, con un id al azar. También arma un formulario TOML con las mismas rúbricas. El usuario pone un nivel por ítem y una decisión por paquete, sin ver el veredicto de Root. `r00t oraculo validar` lee los formularios.
  El resultado va a `validez.json` con tres métricas:
  - la exactitud de orden;
  - el recall de defectos reales (umbral 0,8, decisión 12);
  - el acuerdo con el usuario: kappa de Cohen ponderado por ítem, acuerdo en la decisión y Spearman del orden de los 6 paquetes.
  El acuerdo por ítem también mide la rúbrica: un ítem con kappa bajo indica un ancla ambigua.
- **Validate**: `r00t oraculo validar --casos $ROOT_HOME/validez/` escribe las tres métricas. Si falta una fuente, la métrica queda como "sin medir", nunca como 0 ni como 1.

### Tarea 13: Oracle completo
- **Action**: `evaluar` arma el `Veredicto` con contrato, oculto, completitud, entrega, pruebas, sondas, juez y gate de evidencia. Calcula `global` solo con dimensiones calibradas y marca las demás como "sin calibrar".
- **Validate**: `r00t oraculo evaluar --tarea memora --repo .../memora-ecc --json | jq '.puntajes'` lista nueve dimensiones; memora-sdlc obtiene un `global` menor que memora-ecc.

### Tarea 13b: Informe del PM
- **Action**: `oracle/informe.rs` arma el `Informe` desde el `Veredicto`:
  - La `Decision` sale de `[decision]` de `root.toml` por reglas deterministas (decisión 12):
    - `Bloqueado` si un requisito de peso 3 queda en nivel 0, si hay un hallazgo de seguridad de severidad 3, o si un ítem de rúbrica marcado `bloquea = true` queda en nivel 0. Un ejemplo de ese ítem es `seguridad.secretos`.
    - `NoListo` si `Completitud` < 0,9 o si una dimensión calibrada queda bajo su `minimo` de `root.toml`.
    - `Listo` en otro caso.
    El informe nombra la regla que disparó la decisión.
  - `requisitos` lista cada requisito con su estado y su evidencia.
  - `riesgos` y `proximos_pasos` son 5 ítems como máximo, ordenados por `peso` del requisito y por severidad.
  - Un LLM escribe `resumen` y redacta los textos de `riesgos` y `proximos_pasos`. Cada oración cita un id existente; el gate de evidencia descarta las demás.
  - `r00t oraculo informe --md` imprime el informe legible: decisión, por qué, qué falta, qué arreglar primero.
  - **Datos para decidir** (decisión 16): `Informe` suma, solo agregando, `confianza: Confianza`. `Confianza` lista las repeticiones, los tests inestables con su tasa, los requisitos y tests `Pendiente` por entorno, las ambigüedades abiertas de la ficha, las dimensiones sin calibrar y las partes sin veredicto. El informe muestra la confianza junto a la decisión. Una decisión `Listo` con 3 dimensiones sin calibrar dice exactamente eso.
- **Validate**: el informe de memora-sdlc da `NoListo` o `Bloqueado` y nombra los módulos ausentes en `proximos_pasos`. El informe de memora-ecc no inventa faltantes: cada ítem cita un id real. Un test con un LLM falso que cita un id inexistente pierde esa oración.

### Tarea 14: Route
- **Action**: `elegir` es una función pura. Primero aplica tres invariantes:
  - Si `checkeable` es verdadero y la tarea tiene un oráculo, devuelve `Via::Oraculo`.
  - Los roles `Verifica` y `Juzga` siempre llevan `razonamiento = true`.
  - Un rol `Planifica`, `Organiza` o `Construye` respeta la asignación de `root.toml` (Opus, Sonnet, Haiku).
  
  Después aplica el balance: entre los modelos con cota de Wilson mayor o igual al umbral, elige el más barato. Si no hay datos, usa el default de `root.toml`.
- **Topes (decisión 15)**: `[balance]` de `root.toml` reemplaza el campo `tope_cupo_usd` de la Tarea 2 por tres topes:
  ```toml
  [balance]
  prefiere_api_sobre = 0.9          # fracción del tope de cupo del bloque
  [balance.cupo]
  tope_usd = 35.0                   # USD equivalentes de lista, por bloque de 5 horas
  bloque_horas = 5
  [balance.api]
  tope_usd = 100.0                  # DeepSeek + Anthropic por API, por mes calendario
  [balance.nube]
  tope_usd = 250.0                  # sesiones de Claude en la nube, por mes calendario
  ```
  - **Bloque de cupo**: imita la ventana de 5 horas del plan de Claude. Un bloque empieza con el primer evento `Via::Cupo` después de que venció el bloque anterior, y dura `bloque_horas`. `observe::uso` suma el `cost_usd` de los eventos `Cupo` del bloque vigente.
  - **Mes calendario**: suma los eventos `Via::Api` desde el día 1 a las 00:00, hora local. Es el mismo período con que facturan los proveedores.
  - **Reglas de `elegir`**, en orden:
    1. `Via::Oraculo` no gasta y nunca se frena.
    2. Si el cupo del bloque llega a `prefiere_api_sobre × tope`, Route prefiere la API, siempre que la API tenga margen.
    3. Si el cupo del bloque llega al 100% del tope, el cupo queda prohibido hasta que termine el bloque.
    4. Si la API del mes llega al 100% del tope, la API queda prohibida hasta el mes siguiente.
    5. Si las dos vías están prohibidas, `elegir` devuelve `SinPresupuesto` con el próximo momento de reapertura. `r00t route` y `r00t llama` salen con código 3 y registran un evento `Gate`. Root nunca sube un tope solo: subirlo es zona roja (dinero) y lo hace el humano en `root.toml`.
  - **Antes de cada llamada**: `r00t llama` estima el peor costo con `max_tokens` y los precios de `root.toml`. Si ese costo pasa el resto del tope, rechaza la llamada antes de enviarla. Así ninguna llamada supera el tope a mitad de camino. El módulo `proveedores` de la Tarea 8 no cambia: el chequeo vive en `r00t llama`.
  - **Límite**: el cupo se mide en USD equivalentes de lista (`costUSD` de `claude -p`), no en el porcentaje real del plan. Con ese porcentaje, el tope de 35 USD equivalentes puede no coincidir con el corte real de Anthropic.
  - **Nube (decisión 15)**: el tope de 250 USD cubre las sesiones de Claude en la nube. La Tarea 14 suma la variante `Via::Nube` al final del enum; el cambio solo agrega. Una sesión en la nube se registra con `r00t observa claude --via nube`, y `observe::uso` la suma al mes calendario.
    - En el hito 1, Root no lanza sesiones en la nube, así que Route nunca elige `Via::Nube`. El tope de nube no prohíbe ninguna vía: solo se informa en `r00t observa uso`, y al llegar al 100% se registra un evento `Gate`.
    - La sesión en la nube sí cambia la elección de Route por otro camino: suma al bloque de cupo (P9), y el cupo del bloque activa las reglas 2 y 3.
    - Root solo cuenta las sesiones en la nube que alguien registra. Una sesión que nadie registra no suma, y `r00t observa uso` lo advierte en su salida.
    - Una sesión en la nube suma también al bloque de cupo (P9, decisión del usuario).
  Hay dos fuentes de datos para las estadísticas:
  - Los roles `Planifica`, `Organiza` y `Construye` usan las `Fila`s: el éxito es `Decision::Listo`, agrupado por `Fila.modelos[rol]` y `tarea`.
  - Los roles `Delega`, `Verifica` y `Juzga` usan los eventos `Llamada` con `extra.exito`. El chequeo posterior a la llamada escribe ese campo.
- **Validate**: `cargo test route` con una tabla de casos:
  - Checkeable da `Oraculo`, aunque los dos topes estén agotados.
  - Un juez sin razonamiento es imposible.
  - Cupo del bloque en 32 USD (más del 90% de 35) da API.
  - Cupo en 35 USD y API en 40 USD dan API.
  - Cupo en 35 USD y API en 100 USD dan `SinPresupuesto`, con la hora de fin del bloque.
  - Una sesión en la nube de 10 USD suma 10 al mes de nube y 10 al bloque de cupo (P9).
  - Nube en 250 USD con el cupo del bloque en 0 registra un evento `Gate` y no cambia la elección de Route.
  - Una sesión en la nube de 32 USD en el bloque vigente lleva el cupo sobre el 90% y da API (P9).
  - Un evento de cupo de hace 6 horas no cuenta para el bloque vigente.
  - Un gasto de API del mes anterior no cuenta.
  - Una llamada estimada en 3 USD con 2 USD de margen se rechaza sin enviarse.

### Tarea 15: Aprende
- **Action**: `recalcular` lee filas y eventos y escribe `politica.json`. `r00t aprende --replay` aprende con el primer 70% de las filas y mide el 30% restante. Compara la política aprendida contra la de `root.toml` en éxito y en USD.
- **Validate**: `cargo test aprende` (con filas sintéticas donde Haiku acierta igual que Sonnet a menor costo, la política elige Haiku)

### Tarea 15c: Aprende propone cambios en los roles fijos (decisión 25, parte c)
- **Action**: `recalcular` suma, solo agregando, `propuestas: Vec<Propuesta>` a `politica.json`.
  - Una `Propuesta` nombra el rol, el modelo actual de `root.toml`, el modelo propuesto, la evidencia y la diferencia esperada. La evidencia es `n`, éxitos, cota de Wilson y USD medio de cada modelo, con los ids de las filas.
  - Solo hay propuesta para Planifica, Organiza y Construye. Hace falta una cota de Wilson del modelo propuesto ≥ `umbral_wilson` y al menos 3 filas por modelo. El propuesto debe acertar igual o mejor, o acertar igual y costar menos.
  - Route no lee `propuestas`: esos roles siguen respetando `root.toml`. El usuario aplica una propuesta a mano en `root.toml`.
  - `r00t aprende` imprime las propuestas, y con `--json` las incluye en el objeto. Cada propuesta impresa registra un evento `Gate`.
- **Pendiente de P18b**: la evolución hacia "esquemas o arquitecturas de agentes" no entra en esta tarea hasta la aclaración.
- **Validate**: `cargo test aprende`:
  - Con filas sintéticas donde Haiku organiza igual que Sonnet a menor costo, hay una propuesta para Organiza, y `elegir` sigue devolviendo Sonnet para Organiza.
  - Con 2 filas por modelo no hay propuesta.
  - Un `politica.json` viejo, sin `propuestas`, se sigue leyendo.

### Tarea 16: Runner
- **Action**: `r00t corre` corre primero `r00t goal verificar` y se detiene si el GOAL no está autorizado. Después copia el bootstrap a un directorio nuevo y corre `claude -p` por rol con `route`. Usa `ECC_DISABLED_HOOKS` y los permisos que niegan `oraculos/`. Registra `PlanAprobado` y `Humano`. Al final evalúa y agrega la `Fila`.
- **Mirror**: `correr-ecc-rol.sh`.
- **Validate** (cambiado por la decisión 36, P39 c):
  - Mecánica: `r00t corre` sobre una tarea de fixture chica, con un proveedor falso, deja una fila en `runs.jsonl` con `Informe`, resumen y `forma`. Este caso no gasta cupo ni API.
  - Corrida real (decisión 38): `r00t corre --tarea graft-lector-planes --config ecc-haiku` y `r00t corre --tarea memora --config ecc-haiku`. Cada una deja una fila con un veredicto de nueve dimensiones, un `Informe` y un resumen.
  - La corrida de Graft espera a G2 y a G5. La de memora espera a G1 y a G2.
  - Si G4 saca a memora del banco antes de esta corrida, el planner pregunta al usuario antes de quitar la corrida de memora.

### Tarea 16b: Tarea de Graft con oráculo oculto (decisión 36)
- **Qué tarea** (decisión 37): `graft-lector-planes`, una CLI en Rust que lee un `plan.md` y emite en TOML sus tareas, dependencias y comandos `Validate`. El hito 2 (Graft) la va a necesitar.
  - Los tests ocultos parsean el TOML emitido y comparan su estructura con la esperada. No comparan texto: dos TOML con distinto formato y los mismos datos son iguales.
  - La ficha fija el esquema de salida: nombres de tablas y claves, tipos y orden de las tareas.
  - Los planes de prueba incluyen el `plan.md` de Root y planes chicos escritos para cada borde, como una tarea sin `Validate` o una dependencia circular.
- **Quién escribe el oráculo**: Opus, en una sesión aparte, sin builders. Escribe tres cosas:
  - la `## Ficha` de su `SPEC.md`, que el builder sí ve;
  - los tests ocultos de caja negra y `tarea.toml` con `[[requisito]]`, `pruebas` y `bordes`;
  - una implementación de referencia mínima.
  El usuario aprueba la ficha y los requisitos (G5), igual que en G1. Sonnet construye solo lo que falte en Root para correr la tarea, nunca el oráculo.
- **Cómo se prueba el oráculo** (la etiqueta sale de la construcción, como en `gates_bench`):
  - Cada test oculto pasa sobre la implementación de referencia. Así ningún test pide algo imposible.
  - Cada test oculto falla sobre el bootstrap vacío. Así ningún test da un verde regalado.
  - `r00t ficha revisar` da 0 ambigüedades abiertas sobre la ficha aprobada.
- **Cómo el builder queda sin verlo**:
  - El oráculo, la referencia y `tarea.toml` viven solo en `$ROOT_HOME/oraculos/graft-lector-planes/`. Nunca entran al repo de Root ni a git. El repo se sube a GitHub y lo leen las sesiones en la nube.
  - El bootstrap del builder lleva solo `SPEC.md` con su ficha y los tests de `contrato/`.
  - `r00t corre` niega `Read` y `Bash` sobre `oraculos/` en los permisos de la sesión del builder.
  - Observe busca la ruta `oraculos/` y nombres de tests ocultos en el transcript. Un hallazgo invalida la corrida.
  - La sesión que escribe el oráculo no construye la tarea, y la sesión que construye no lee nada de esa sesión.
- **Validate**:
  - Los tres chequeos de "Cómo se prueba el oráculo" pasan.
  - `git ls-files | grep -i graft-` no lista archivos del oráculo.
  - Una corrida de prueba que lee un archivo de `oraculos/` queda inválida.
- **Dónde corre**: escribir el oráculo es trabajo de Opus. Probar el oráculo y correr la Tarea 16 necesita podman y `ROOT_HOME`, así que va en la laptop.

### Tarea 17: Servidor MCP
- **Action**: JSON-RPC por stdio (`initialize`, `tools/list`, `tools/call`) con las cuatro herramientas. Cada herramienta llama a la misma función que su subcomando.
- **Validate**: `cargo test --test mcp` (un `tools/call root_route` devuelve el mismo JSON que `r00t route --json`)

### Tarea 18: Vault fuera de Root
- **Action**: Root empieza de cero, así que no tiene código del vault ni depende de `mmorch_vault_write`. El vault global y tu `CLAUDE.md` quedan como están. Esta tarea no escribe código: solo agrega una línea a `docs/interfaces.md` y un test.
- **Validate**: `! grep -rniE 'vault' src/` y `! grep -rlE 'mmorch' src/ | grep -vE '^src/(importar|cli)\.rs$'`. El nombre `mmorch` solo aparece donde etiqueta el origen de los datos importados: el subcomando `importar-mmorch` y `extra.origen = "mmorch"` (decisión b3). Ningún otro módulo depende de mmorch.

### Tarea 19: Documentos y PRD
- **Action**: escribir `docs/interfaces.md` y `docs/evidencia.md`, y anotar las decisiones en el PRD.
- **Validate**: `r00t esquema` de cada tipo coincide con `docs/interfaces.md` (test de snapshot).

## Cómo se construye (roles)

- Opus escribió este plan y resuelve los desempates.
- Sonnet corre como sesión principal: `--model sonnet` con `CLAUDE_CODE_SUBAGENT_MODEL=haiku`. Sonnet reparte una tarea por builder y corre el `Validate` de cada una.
- Haiku construye cada tarea con TDD dentro de ECC.
- Una tarea que falla su `Validate` tres veces vuelve a Sonnet con el log. Sonnet decide si la divide.
- La Tarea 3 (GOAL) tiene gate humano.

## Validation

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
r00t goal verificar
r00t oraculo evaluar --tarea memora --repo ~/Proyectos/bench-ecc-sdlc/memora-ecc --json   # niveles por requisito estables en 3 corridas
r00t oraculo evaluar --tarea brasa  --repo ~/Proyectos/bench-ecc-sdlc/brasa-ecc  --json   # cada diferencia con causa en diferencias.md
for d in completitud entrega pruebas seguridad estructura arquitectura eficiencia diseno; do r00t oraculo calibrar --tarea memora --dimension $d --n 20 --semilla 42; done
r00t oraculo informe --tarea memora --repo ~/Proyectos/bench-ecc-sdlc/memora-sdlc --md   # NoListo o Bloqueado, con los módulos ausentes
r00t oraculo validar --casos $ROOT_HOME/validez/
r00t corre --tarea memora --config ecc-haiku && r00t track listar --tarea memora
r00t aprende --replay
```

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Haiku entra en ciclos con el borrow checker de Rust | Media | Tareas de menos de ~150 líneas; `cargo check` en cada paso; escalada a Sonnet después de 3 fallas |
| La biblioteca de YAML queda sin mantenimiento (`serde_yaml` ya está archivado) | Media | Verificar la fecha de la última versión en la Tarea 1; el parseo vive solo en `configs.rs`, así que cambiar de biblioteca toca un archivo |
| hadolint no corre sin la imagen de evaluación | Baja | La sonda falla cerrada: sin hadolint, la dimensión da `SinVeredicto`, nunca un puntaje limpio |
| El juez de diseño es subjetivo y la calibración solo cubre defectos inyectados | Alta | Una dimensión sin calibrar no entra a `global`. La Tarea 12b mide el juicio sobre casos reales. Sin juicio humano (P2), el acuerdo con el usuario queda "sin medir" y el informe lo dice. |
| Root degenera en un agregador de linters (pseudo-compilador) | Media | La Tarea 9 pondera por impacto en requisitos, no por conteo; el test de "200 avisos de estilo, 0 defectos" lo fija |
| Opus escribe los requisitos y también diseña el juez: el sesgo de la lista pasa al juicio | Media | El usuario aprueba la lista (P3); la Tarea 12b mide contra resultados externos |
| `cargo-mutants` tarda mucho en brasa (render con GPU) | Alta | `mutantes_max` y `minutos_max` en `tarea.toml`; al llegar al tope, `Pruebas` informa la muestra que alcanzó |
| brasa necesita un adaptador de CPU en el contenedor | Alta | lavapipe en `oraculo-rust`; la Tarea 7 reproduce 53/53 antes de seguir |
| El builder lee los oráculos ocultos (mismo usuario) | Media | Permisos que niegan `oraculos/`; detección en el transcript invalida la corrida |
| El cupo no se observa directo: `claude -p` da USD equivalentes, no el % del plan | Confirmada | El balance usa USD equivalentes y un tope configurable |
| ECC entero (decisión a) trae hooks que escriben secretos en `~/.claude/` | Confirmada | `r00t corre` apaga esos hooks por defecto; la reconstrucción de ECC los elimina |
| Haiku no está medido (decisión 7) | Media | Cada fila de `runs.jsonl` mide a Haiku de hecho; `r00t track comparar` lo hace visible |
| Podman sin el tag `z` falla por SELinux | Alta en Fedora | El tag `z` es fijo en `contenedor.rs` y tiene un test |
| El contenedor corre sin red: un repo con dependencias que no están en la imagen no compila | Resuelto por la decisión 17 | Fase de descarga de la Tarea 6b. Hasta que exista, rige `SinVeredicto` con la causa "dependencias no disponibles sin red". |
| Un paquete malicioso entra por la fase de descarga | Baja | La descarga no compila. El build corre después sin red, con el repo en solo lectura y sin acceso a `ROOT_HOME`, salvo la caché. Desde la decisión 21, git y otros registros se aceptan: la Tarea 6c los registra antes de usar la red, rechaza protocolos sin `https` y credenciales, y nunca corre `build.rs` ni `setup.py` con red. `cargo audit` marca los paquetes con CVE (Tarea 9). |
| Una ficha mal escrita traslada su ambigüedad al juicio de Root | Media | `r00t ficha revisar` corre antes de evaluar. Un requisito ambiguo queda "no evaluable" y nunca se juzga contra una de sus dos lecturas. |
| Un test inestable se lee como un defecto del producto, o al revés | Alta (ya pasó con `test_blpop_fifo_entre_clientes`) | Repeticiones y clasificación por causa con evidencia (Tarea 7). Sin evidencia, el origen queda "sin determinar" y el informe lo dice. |
| Un agente corre `r00t goal autorizar` por Bash | Baja | `autorizar` exige TTY; la sesión `claude -p` no tiene TTY |
| `GOAL.md` falta cuando arranca `r00t corre` | Media | `r00t corre` corre `r00t goal verificar` primero y sale con código 3 sin GOAL autorizado |

## Acceptance
- [ ] Las tareas 1 a 19 y sus variantes 5b, 5c, 6b, 7f, 8b, 8c, 9b, 9c, 9d, 9e, 9f, 12b y 13b están completas.
- [ ] Cada test `Pendiente` por entorno figura en el informe con su motivo, o tiene un resultado importado de Colab.
- [ ] La validación pasa.
- [ ] La decisión y los niveles por requisito de memora-ecc y brasa-ecc no cambian en 3 evaluaciones seguidas.
- [ ] Cada diferencia contra los números del banco tiene una causa y una evidencia (Tarea 7).
- [ ] Cada informe muestra su `confianza`.
- [ ] Cada dimensión en `global` tiene exactitud balanceada ≥ 0,9 en su calibración.
- [ ] Root ordena bien los pares de orden conocido (ECC contra SDLC en memora y brasa).
- [ ] Root encuentra los defectos reales conocidos de la Tarea 12b, con `archivo:línea`. Recall ≥ 0,8 (decisión 12).
- [ ] Las nueve rúbricas están aprobadas por el usuario, y cada `Veredicto` lleva sus hashes.
- [ ] `validez.json` informa el acuerdo con la calificación a ciegas del usuario. Kappa ponderado ≥ 0,6 por dimensión y la misma decisión en 5 de los 6 paquetes (decisión 13).
- [ ] El informe de memora-sdlc da `NoListo` o `Bloqueado` y nombra los módulos ausentes.
- [ ] `r00t corre` arma memora desde el bootstrap con Haiku y deja una fila completa.
- [ ] `docs/interfaces.md` coincide con los esquemas que imprime `r00t esquema`.
- [ ] Los patrones de este plan se copian, no se reinventan.

## Revisión PM (2026-10-27)

Pedido del usuario: "Las mediciones deben ser competentes, no me sirve un pseudo-compilador. Necesito un PM".
Revisión de las Tareas 6 a 19. Las Tareas 1 a 5 no cambian: Sonnet ya las construye.

**Qué fallaba en la versión aprobada**
- Las sondas eran de Python (bandit, radon), pero memora y brasa son Rust. Sobre estos proyectos, esas sondas no medían nada.
- El plan medía "pasa los tests" y sumaba avisos de linters. Eso es un pseudo-compilador.
- Ninguna tarea respondía lo que pregunta un PM: qué prometió la spec, qué falta, si está listo y qué hacer primero.
- La calibración probaba detección de defectos inyectados, no la calidad del juicio sobre proyectos reales.

**Qué se agregó**
- Tarea 5b: tipos `Informe`, `Decision` y `EstadoRequisito`, solo agregados.
- Tarea 9 por perfil de lenguaje (Rust primero), ponderada por impacto y no por conteo.
- Tarea 9c: completitud por requisito de la spec.
- Tarea 9d: entrega e instalación limpia, más escenarios reales de usuario.
- Tarea 9e: calidad de los tests del proyecto, por mutación.
- Tarea 10: el juez revisa el diseño contra la spec, con un mapa del repo, y no revisa estilo.
- Tarea 12b: validez contra resultados externos (orden conocido, defectos reales, juicio humano).
- Tarea 13b: informe del PM con decisión determinista y prosa citada.

**Preguntas respondidas (decisión 12)**
- **P1.** Se acepta la propuesta, más rúbricas basadas en estándares de la industria: Tarea 8b y regla de decisión de la Tarea 13b.
- **P2.** Sí: el usuario califica a ciegas los 6 paquetes de la Tarea 12b.
- **P3.** Sí: el usuario aprueba los requisitos. La spec sale de una sesión `/grill-me`.
- **P4.** Recall ≥ 0,8.

- **P5.** Se acepta: kappa ponderado ≥ 0,6 por dimensión y la misma decisión en 5 de los 6 paquetes.

## Desempates técnicos (Opus, 2026-10-27)

Respuestas a las preguntas de Sonnet al cerrar las Tareas 1 a 5. Los cambios de código van en la Tarea 5c, salvo el campo de `Fila`, que va en la Tarea 5b.

1. **Tarea 18 contra la Tarea 5.** Se aceptan las dos excepciones. `importar-mmorch` y `extra.origen = "mmorch"` etiquetan de dónde vienen los datos, y la decisión b3 los pide. El test de la Tarea 18 busca que Root no dependa de mmorch, no que nunca lo nombre. Su comando quedó acotado a `src/importar.rs` y `src/cli.rs`.
2. **`Fila` sin rol.** Sí: `Fila.modelos: BTreeMap<Rol, String>` entra en la Tarea 5b. Una `Fila` mide una corrida entera con varios roles, así que el mapa guarda qué modelo cumplió cada rol. Los roles por llamada (`Delega`, `Verifica`, `Juzga`) usan eventos, no filas (Tarea 14).
3. **Defaults de `root.toml`.**
   - `umbral_verde = 0.8`: se confirma.
   - `umbral_wilson = 0.5`: corregido a 0,6 en "Desempates técnicos 5". La cuenta de este punto usaba z = 1,645, y Track usa la cota unilateral al 90% (z = 1,2816).
   - `prefiere_api_sobre = 0.9`: se confirma, como fracción del tope.
   - `tope_cupo_usd = 200`: lo reemplazan los tres topes de la decisión 15 (Tarea 14).
4. **Rotación.** Sí: `resumir` y `filas` leen los archivos `.N` en orden y después el vigente. Una corrida que cruza una rotación debe contar entera (Tarea 5c).
5. **Detalles.**
   - `float_roundtrip`: sí. Las comparaciones exactas de costos y los snapshots de esquema no deben depender de 1 ULP.
   - Primera clave alfabética: no. Con Sonnet como organizador y Haiku como builder, esa clave elige a Haiku y le asigna el costo de Sonnet. Se emite un evento por modelo, con su `costUSD` (Tarea 5c).
   - El flag: `--json` sigue siendo el flag global de toda la CLI, porque es el contrato de salida. El flag de entrada pasa a `--archivo`, porque `--salida` se confunde con la salida del comando.

**Nombre del binario (decisión 14).** Todo comando de este plan se escribe `r00t <subcomando>`. El crate `root`, `root.toml`, `ROOT_HOME` y las herramientas MCP `root_*` no cambian.
`GOAL.md` nombra `r00t goal verificar` y `r00t goal autorizar`. El usuario lo editó a mano y lo re-autorizó (hash 311a6371c0cd414a). Ver P7.

**Preguntas abiertas (sin responder)**
- **P6.** Respondida (decisión 15): API 100 USD por mes, nube 250 USD por mes y cupo 35 USD equivalentes por 5 horas.
- **P8.** Respondida (decisión 15): "la nube" son las sesiones de Claude en la nube.
- **P9.** Respondida por el usuario (textual): "P9: sí, una sesión en la nube cuenta en el cupo de 5 horas y en los 250 USD por mes."
- **P7.** Cerrada: el usuario cambió `GOAL.md` a `r00t goal` y lo re-autorizó (textual: "ya puse autorizar."). `r00t goal verificar` da 311a6371c0cd414a.

## Revisión PM 2 (2026-10-27, Tarea 7)

Pedido del usuario: "De que son los tests? ya dije no quiero un compilador de codigo, quiero un evaluador con criterio, un project manager con datos para decidir".

**Qué medía todavía como un compilador**
- La Tarea 7 exigía repetir un conteo exacto (61/61, 48/48, 53/53, 15/16). Ese conteo viene de una sola corrida en el host. Perseguir el número no dice nada nuevo sobre el producto.
- `Estado::Verde` se decidía por conteos de tests y no por la decisión del PM.
- Un test inestable no tenía lugar: quedaba como "falla" o como "pasa" según la corrida.
- El informe no decía cuánto confiar en sus propios datos.

**Qué cambió**
- Tarea 7: criterio de estabilidad de la decisión, repeticiones y una causa con evidencia por diferencia.
- Tarea 9c: un requisito con un test inestable no llega al nivel 3; los tests de causa `entorno` u `oraculo` se excluyen con motivo.
- Tarea 13b: `Informe.confianza`.
- `Estado` se deriva de la `Decision`.
- Riesgo nuevo: contenedor sin red (P10).

**Preguntas abiertas (sin responder)**
- **P10.** Respondida (decisión 17): sí, con una fase de descarga previa. Es la opción a.
- **P11.** Respondida en parte (decisión 18): las ambigüedades se corrigen en una ficha del spec, no con exclusiones. Queda abierta la parte del entorno (P13).

**Preguntas respondidas**
- **P12.** El usuario respondió "P12 si". La respuesta no elige entre las dos opciones: queda la aclaración P12b.
- **P13.** Respondida (decisión 19): el informe dice `Pendiente`, o el test se prueba en Colab.
- **P14.** Respondida (decisión 20): la ficha va en el spec y es obligatoria.

## Desempates técnicos 2 (Opus, 2026-10-27, después de la Tarea 8)

Respuestas a las preguntas de Sonnet al cerrar las Tareas 7 y 8. Los cambios de código van en las Tareas 7f y 8c.

3. **Repeticiones.** Suben a 5 para el contrato y para el oculto. Con un test que falla 3 de cada 10 veces, 3 corridas lo dejan pasar como estable un 34% de las veces; 5 corridas, un 17%. Un test con resultados mezclados se vuelve a correr solo hasta 10 corridas, para medir su tasa. Correr la suite entera 10 veces cuesta mucho y no cambia la decisión del PM. El contrato también se repite: un test visible inestable es evidencia del producto, igual que uno oculto.
4. **Breaker.** No: un 400 o un 401 no abren el breaker. El breaker protege de un servicio caído, y un 4xx dice que el pedido o la clave están mal. Reintentar un 401 no lo arregla, y abrir el breaker escondería la causa real. Un 4xx distinto de 429 no se reintenta y sale como error de infraestructura con su causa.
5. **Evento no escribible.** Se mantiene el error, pero el error lleva la respuesta. Devolver la respuesta con un aviso deja un gasto fuera de `eventos.jsonl`: rompe el invariante de observabilidad, y Route calcula los topes con un uso menor que el real. Perder la respuesta empuja a reintentar y a pagar dos veces. Con `EventoNoRegistrado { respuesta, evento, causa }`, nada se pierde y un humano puede asentar el evento a mano desde stderr.
6. **`TMPDIR`.** `evaluar` no depende de `TMPDIR`. Sus temporales van a `$ROOT_HOME/tmp/`, que está en el disco de `/home`. El tmpfs de `/tmp` tiene 7,7 GB y una compilación de brasa ya lo llenó.
7. **Base de RustSec.** `cargo audit` necesita su base de avisos, y la evaluación corre sin red. La fase de descarga (Tarea 6b) la baja a la caché. No se precarga en la imagen, de acuerdo con la instrucción anterior de no precargar más dependencias.

## Orden de construcción (Opus, 2026-10-27)

Sonnet construye en este orden. Ninguna de estas tareas espera a un humano:

1. Tarea 8c: breaker y evento no escribible.
2. Tarea 7f: repeticiones, `Pendiente`, paquete para Colab y temporales en `ROOT_HOME`.
3. Tarea 6b: fase de descarga, con la base de RustSec.
4. Tarea 11: gate de evidencia. La usan 9f, 10 y 13b.
5. Tarea 8b, solo el código: `oracle/rubrica.rs` con rúbricas de fixture en `tests/fixtures/`. Las nueve rúbricas reales las redacta Opus.
6. Tarea 9f: ficha y revisión, con fichas de fixture.
7. Tareas 9 y 9b: sondas del perfil `rust` y de configs.
8. Tareas 9c, 9d y 9e, con fixtures. Su `Validate` sobre memora y brasa espera al gate G1.
9. Tarea 10: juez, con proveedor falso y rúbricas de fixture.
10. Tareas 14, 15, 17 y 18.

Gates humanos que frenan el resto:

- **G1, fichas.** Opus redacta `## Ficha` de memora y brasa, con las preguntas de ambigüedad. El usuario las responde y aprueba las fichas. Frena: el `Validate` real de 7, 9c, 13 y 13b.
- **G2, rúbricas.** Opus redacta las nueve rúbricas. El usuario las aprueba. Frena: 12, 13 y 16.
- **G3, calificación a ciegas.** El usuario califica los 6 paquetes de la Tarea 12b. Frena: la métrica de acuerdo de 12b.
- **P15, Colab.** Respondida (decisión 22): solo T4. Tarea 7h.
- **P16, herramientas en la imagen.** Respondida (decisión 23). Tarea 7g.
- **P17, construir el Dockerfile.** La parte c) está aplicada (Tarea 9d2). La parte b) espera a P17b.
- **P18, aprender en los roles fijos.** La parte c) está aplicada (Tarea 15c). La segunda parte espera a P18b.
- **P21, credenciales de registros.** Respondida (decisión 29). Tarea 6d.
- **P38, papel del juez de la config de cargo.** Respondida (decisión 35): el juez solo informa.
- **P39, memora como banco.** Respondida (decisión 36): opción c.
- **G4, memora después de G3.** Al cerrar G3, el usuario decide si memora sigue como banco (P44 c). Hasta entonces, memora sigue.
- **G5, oráculo de Graft.** El usuario aprueba la ficha y los requisitos de la tarea de Graft. Frena la corrida real de la Tarea 16.
- **P48.** Respondida (decisión 37).
- **P49.** Respondida (decisión 38): Graft y memora.

**Respuestas para las fichas (G1, decisión 26).** Opus las escribe en `## Ficha` al redactar G1:
- **memora**: el requisito de `BLPOP` exige orden FIFO entre clientes bloqueados. Su aceptación cita `test_blpop_fifo_entre_clientes`. Ese test es correcto, y su falla de 9 en 30 corridas es un defecto del producto con origen "producto".
- **brasa**: "en el espacio local" significa los ejes locales del nodo. `test_spin_despues_de_rotacion` es correcto. brasa-ecc falla porque `scene.rs:271` aplica el spin en los ejes del padre: es un defecto del producto. La causa `oraculo` de ese test desaparece de `diferencias.md`.
- Con esas dos respuestas, ningún test de memora ni de brasa queda en `[[oraculo_dudoso]]`.

**Orden nuevo (después de 15b, 17, 18 y 19):** 6c, 7g, 9d2, 15c y 7h. La 6c va primero porque el filtro vigente no refleja la decisión 21. La 7g va antes de la 9d2 porque `hadolint` entra con la imagen. La 7h va última: necesita el login de Colab del usuario, y su corrida real espera a G1.

**Orden siguiente (2026-10-28, después de la 7h):** 7i y después 6c-B. La 7i va primero porque cierra un posible borrado fuera de `ROOT_HOME` y los fail-open de las sondas. Después de la 6c-B, todo lo que queda espera a G1 o a G2: 12, 12b, 13, 13b, 16, el `Validate` real de 7 y 9c, y la corrida real de la 7h.

**Orden siguiente (2026-10-28, después de la 7i):** 7j y después 6c-B, con lo que faltó de la 7i. La 7j va primero porque cierra el riesgo de un runtime de Colab encendido después de un Ctrl-C. Ese riesgo no está activo todavía: la corrida real espera a G1, P35 y P37.

Las Tareas 12, 12b, 13, 13b, 16 y 19 van después de G1 y G2. Opus redacta G1 y G2 en otra sesión, mientras Sonnet construye la lista de arriba.

**Avance (2026-10-27):** Sonnet cerró 8c, 7f, 6b, 11, 8b (código) y 9f, con 341 tests verdes (commit 4f2be05). Una evaluación real de memora-ecc, antes de la ficha obligatoria, dio contrato 61/61 y oculto 47/48, con el FIFO de `BLPOP` inestable. Desde la 9f, memora y brasa salen con código 2 hasta tener `## Ficha` (G1).

## Desempates técnicos 3 (Opus, 2026-10-27, después de la Tarea 9f)

Respuestas a las preguntas de Sonnet al cerrar las Tareas 7f, 6b y 9f. Los cambios de código van en la Tarea 9f2.

3. **Salida de `ficha revisar`.** Con hallazgos deterministas, sale 1 aunque el juez quede `sin_veredicto`. Un hallazgo determinista ya prueba que la ficha no está limpia, así que el rojo es seguro y no depende del juez. El JSON sigue marcando al juez `sin_veredicto`. El código 3 queda para un caso: el juez sin veredicto y ningún hallazgo determinista. Root no puede decir que esa ficha está limpia. El código 0 exige las dos capas limpias.
4. **`ficha tarea --json`.** Sí: con `--json`, imprime `{"toml": "..."}`. El contrato de la CLI dice que todo subcomando con `--json` imprime un solo objeto JSON. Sin `--json`, sigue imprimiendo el TOML.
5. **`colab.json` malformado.** `evaluar` no falla. Los tests de ese archivo quedan `Pendiente`, que es el estado conservador: no da un verde falso. El veredicto y `Informe.confianza` dicen "colab.json ilegible" con la causa. Un aviso solo en stderr no alcanza: el dato tiene que llegar al informe. Un archivo malformado no tumba una evaluación entera por una evidencia opcional.
6. **Error original del proveedor.** Se conservan los dos. `ErrorProveedor::ErrorNoRegistrado { original, causa_registro }` lleva el error del proveedor y la causa del registro. Su mensaje muestra primero el original, porque el original explica qué pasó con la llamada. `r00t llama` escribe la línea JSON del `Evento` en stderr y sale con código 3, igual que `EventoNoRegistrado` (desempate 5 de "Desempates técnicos 2").

### Tarea 9f2: Ajustes a las Tareas 7f, 8c y 9f
- **Action**: los desempates 3 a 6 de arriba, más un arreglo: el mensaje "sin ficha" sale una sola vez en stderr, no tres.
- **Validate**: `cargo test`:
  - Una ficha con 1 hallazgo determinista y un juez falso que devuelve JSON roto sale con 1.
  - Una ficha sin hallazgos deterministas y con el mismo juez sale con 3.
  - `r00t ficha tarea --json` imprime un objeto con la clave `toml`, y ese TOML parsea.
  - Un `colab.json` malformado deja sus tests `Pendiente`, y `confianza` nombra el archivo.
  - Un 503 con `eventos.jsonl` sin permiso de escritura da `ErrorNoRegistrado`, y el mensaje contiene el 503.
  - `r00t oraculo evaluar` sin ficha escribe "sin ficha" una sola vez en stderr.

**Avance (2026-10-27):** Sonnet cerró 9f2, 9, 9b, 9c, 9d, 9e y 10, con 583 tests verdes (commit c9f398c). Hay tres límites conocidos:
- La imagen `oraculo-rust` no trae `clippy`, `gitleaks`, `cargo-audit`, `hadolint` ni `cargo-mutants`. Sus sondas están probadas con salidas de fixture, y con la imagen real dan `SinVeredicto`. Ninguna corrió de verdad (P16).
- La 9d verifica el Dockerfile solo en forma estática (P17).
- memora y brasa siguen sin evaluarse hasta G1.

## Desempates técnicos 4 (Opus, 2026-10-27, después de la Tarea 10)

Respuestas a las preguntas 3 a 6 de Sonnet y a sus notas. Los cambios de código van en la Tarea 10b. Las preguntas 1 y 2 tocan la red: van al usuario como P16 y P17.

3. **Nivel 0 sin evidencia.** Queda en `faltantes`, no vale 0. Toda ancla exige evidencia citada, también la del nivel 0. Un 0 sin evidencia cobra al producto un defecto que nadie probó: es un rojo falso. En `faltantes`, el ítem queda sin veredicto y `Informe.confianza` lo lista. Un 0 por ausencia ("no hay logs") cita el lugar donde debería estar, por ejemplo el `main.rs` o el módulo del mapa. Las anclas de nivel 0 de las rúbricas reales (G2) dirán qué evidencia prueba una ausencia.
4. **Doble penalización.** Un defecto da un solo hallazgo. Un Dockerfile sin `USER` es un hallazgo de Entrega, emitido por la 9d. La tabla de rúbricas asigna el CIS Docker Benchmark a Entrega, y la Tarea 12 ya lo inyecta como defecto de entrega. La 9b deja de emitir la regla de `USER`, y conserva la imagen base sin versión y el secreto en `ENV` o `ARG`. La regla general: dos sondas que miran el mismo `archivo:línea` con la misma regla emiten un hallazgo, y la sonda dueña es la que nombra la tabla de rúbricas.
5. **Falso positivo de secretos.** Sí: se afina antes de la Tarea 12. La clave se parte en segmentos por `_`, `-`, `.` y por mayúsculas internas. Hay secreto solo si un segmento es igual a `password`, `passwd`, `secret`, `token`, `apikey` o `key`, o si dos segmentos seguidos forman `api_key`. El valor debe ser literal: `${VAR}`, `<...>` y un valor vacío no cuentan. El caso viene de una revisión, no de los datos de la calibración. Por eso afinarlo no contamina la medición de la Tarea 12.
6. **Elección de juez inválida.** Sí: va en una variante propia, `ErrorOraculo::Eleccion(String)`, con código 2. Una elección sin razonamiento, sin API o sin modelo es un error de configuración de `root.toml` o de Route, no una falla de infraestructura. Sigue sin dar veredicto. `ErrorOraculo::Infra` queda para podman, timeouts y E/S.
7. **Constante compartida.** Sí. `perfiles::rust` exporta `pub const REGLA_CICLO: &str = "ciclo de módulos"`. La sonda arma el detalle con esa constante, y el juez la usa para el tope. Un test de integración corre la sonda sobre un fixture con un ciclo y comprueba que el juez topa `diseno.modularidad` en 1. Si alguien cambia el texto, ese test falla.
8. **Juez con extractos.** Se acepta para el hito 1. El juez recibe extractos con número de línea elegidos por el mapa, en lugar de abrir archivos con herramientas. `proveedores::llamar` no tiene bucle de herramientas, y agregarlo excede el hito. El límite: el juez no ve lo que el mapa no señala. La Tarea 12b mide ese costo con los defectos reales conocidos.

### Tarea 10b: Ajustes a las Tareas 9b, 9d y 10
- **Action**: los desempates 4 a 7 de arriba.
- **Validate**: `cargo test`:
  - Un Dockerfile sin `USER` da exactamente 1 hallazgo en la evaluación entera, de dimensión Entrega.
  - `tokenizer = "bpe"` da 0 hallazgos. `auth_token = "abc123"` da 1. `api_key = "${API_KEY}"` da 0.
  - Una elección de juez sin razonamiento da `ErrorOraculo::Eleccion` y sale con código 2.
  - La sonda de arquitectura sobre un fixture con un ciclo hace que el juez tope `diseno.modularidad` en 1.

**Avance (2026-10-27):** Sonnet cerró 10b, 14 y 15, con 694 tests verdes (commit 77cc4e7). Hay tres límites conocidos:
- Nada escribe todavía `extra.rol`, `extra.tipo` ni `extra.exito`. `recalcular` los lee, pero solo con datos sintéticos. Los escriben los llamadores de Verifica y Juzga en las Tareas 13 y 16.
- No hay filas reales para Planifica, Organiza y Construye. Las produce la Tarea 16.
- Los tests de CLI de la 15 y el test de integración de 9b con 9d no se vieron en rojo antes de implementar. La Tarea 15b lo corrige.

## Desempates técnicos 5 (Opus, 2026-10-27, después de la Tarea 15)

Respuestas a las preguntas 1, 3, 4 y 5 de Sonnet y a sus notas. La pregunta 2 es una decisión de producto: va al usuario como P18. Los cambios de código van en la Tarea 15b.

1. **Nube y Route.** Valen las dos cosas, y el texto de la Tarea 14 ya está corregido. El tope de nube no prohíbe ninguna vía: solo registra un `Gate`. La sesión en la nube suma al bloque de cupo (P9, decisión del usuario), y por eso sí puede cambiar la elección de Route.
3. **Tope exigido.** Se acepta. Si existe `[balance.cupo]`, `[balance.api]` o `[balance.nube]`, la tabla exige `tope_usd`, y sin ese campo la carga falla con código 2. Si la tabla no existe, rigen los topes de la decisión 15. Esos valores los fijó el usuario, así que no son un default peligroso.
4. **Permisos de `politica.json`.** Se queda con 0600. Graft, REA y el servidor MCP corren como el mismo usuario y la pueden leer. Multi-usuario está fuera de alcance en el GOAL, y 0644 la expone a otras cuentas sin necesidad.
5. **`key` como segmento.** Se corrige el desempate 5 de "Desempates técnicos 4". `key` solo cuenta si el segmento anterior es `api`, `secret`, `private`, `access`, `auth`, `signing` o `encryption`. Así `primary_key`, `foreign_key` y `sort_key` no cuentan. `password`, `passwd`, `secret`, `token` y `apikey` siguen contando solos. El caso viene de una revisión, no de los datos de la calibración.
6. **Modelo sin precio.** Se acepta que `llama` falle cerrado: un tope no se esquiva por omisión. La Tarea 19 agrega `docs/root.toml.example` con la forma de `[precios]` y los valores vacíos. El humano escribe los precios desde la página de cada proveedor. Ningún agente inventa un precio.
7. **Cupo sin `api_equivalente`.** El cupo lleno con API libre no debe dar `SinPresupuesto` por un dato faltante. La carga de `root.toml` exige `api_equivalente` en cada alias de cupo. Sin ese campo, la carga falla con código 2 y nombra el alias. Así el error aparece al cargar, no cuando el cupo se llena.
8. **`llama` con log ilegible.** Se acepta que falle cerrado: sin `eventos.jsonl`, Root no conoce el uso y no puede respetar los topes. El test con el `.lock` como directorio alcanza.
9. **Cota de Wilson.** Track usa la cota unilateral al 90% (z = 1,2816) desde la Tarea 5, y sus tests usan valores tabulados con esa z. Con esa cota, 2 de 2 dan 0,549, 3 de 3 dan 0,646 y 10 de 10 dan 0,859. Con `umbral_wilson = 0.5`, un modelo entraría con 2 corridas buenas, no con 3. El default sube a 0,6: 2 de 2 no alcanzan, 3 de 3 sí, y 3 de 4 dan 0,43 y salen.

### Tarea 15b: Ajustes a las Tareas 10b, 14 y 15
- **Action**: los desempates 3, 5, 7 y 9 de arriba. Además, Sonnet demuestra el rojo de los tests escritos después de implementar: revierte a mano la línea que cada test cubre, ve el test en rojo, restaura la línea y anota el resultado en el mensaje del commit.
- **Validate**: `cargo test`:
  - `[balance.api]` sin `tope_usd` falla con código 2.
  - Un alias de cupo sin `api_equivalente` falla al cargar con código 2 y nombra el alias.
  - `primary_key = "x"` da 0 hallazgos. `secret_key = "abc123"` da 1. `token = "abc123"` da 1.
  - Con `umbral_wilson` por defecto, 2 de 2 no entran a la política y 3 de 3 sí.
  - El test de la nube de la Tarea 14 con 32 USD en el bloque da API.

**Avance (2026-10-28):** Sonnet cerró 15b, 17, 18, 19, 6c, 7g, 9d2, 15c y 7h, con 851 tests verdes (commit 099dd2f). Hay cuatro límites conocidos:
- La 6c cubre solo Rust. La descarga de Python queda en la Tarea 6c-B.
- La 9d2 corre los `[[escenario]]` en el contenedor de evaluación, no contra la imagen construida (Tarea 7i).
- La 7h no se probó contra Colab real: espera a G1 y a P35.
- Otra sesión de Opus (cfc01dff) redactó G1 y G2 sin commitear: las fichas en `$ROOT_HOME/oraculos/{memora,brasa}/bootstrap/docs/SPEC.md` y las rúbricas en `docs/borradores/rubricas/`. Sus preguntas al usuario son P22 a P33.

## Desempates técnicos 6 (Opus, 2026-10-28, después de la Tarea 7h)

Respuestas a las preguntas técnicas de Sonnet de las dos últimas vueltas. Las preguntas sobre red, credenciales, código remoto sin verificar y el texto del PRD van al usuario como P34 a P36. Los cambios de código van en la Tarea 7i.

**Preguntas de esta vuelta**
1. **Imagen base privada o inexistente.** Da `SinVeredicto` en la construcción, con la causa "imagen base inaccesible: no existe o es privada". Sin evidencia que separe los dos casos, cobrar al producto sería un rojo falso. No es P21: Root no pasa credenciales, solo clasifica un error. `Informe.confianza` lista el caso.
2. **`FROM ${VAR}`.** Root resuelve primero los `ARG` declarados antes del `FROM` con su valor por defecto. Si la variable queda sin valor, la construcción da `SinVeredicto` con la causa "imagen base sin resolver" y la línea. El `Gate` sigue listando el `FROM` tal como está escrito.
3. **Evaluar con Dockerfile sin conexión.** Un pull que falla no corta la evaluación. Root no baja una imagen base que ya existe en el almacén local de podman con el mismo digest. Si el pull falla, solo la construcción del Dockerfile queda `SinVeredicto` con causa "infraestructura", y el resto de la evaluación sigue sin red. El invariante se cumple: esa parte no da verde.
4. **Proxy en `.cargo/config.toml`.** Toca red y credenciales: va al usuario como P34. Hasta la respuesta, rige la opción a) de P34, que es la más cerrada.
5. **Login de Colab.** Root no lee `~/.config/colab-cli/token.json`: ese archivo es interno de la CLI. El usuario hace el login a mano antes de `--colab`, como dice la Tarea 7h. El primer paso real (`colab new`) sirve de chequeo: corre con stdin cerrado y un timeout de 60 segundos. Si no responde, Root sale con código 3 y pide el login. Nunca espera 60 minutos.
6. **Índices de Python.** Sí. El inventario lista los índices que `pip` usa de verdad. `--index-url` reemplaza a PyPI, y cada `--extra-index-url` se suma. Un índice que no es `https` o que trae credenciales se rechaza, igual que en la 6c. Un `--extra-index-url` deja además un hallazgo de entrega de severidad 1: mezclar índices permite la confusión de dependencias.
7. **rustup en Colab.** Es código remoto sin verificar: va al usuario como P35.

**Preguntas de la vuelta anterior**
1. **Repeticiones en `diferencias.md`.** Sí, el diagnóstico se repite con 5 repeticiones. Se repite una sola vez, después de G1, en el `Validate` real de la Tarea 7. Las fichas cambian dos causas (FIFO y spin, decisión 26), así que repetirlo antes sería trabajo doble.
2. **PRD.** Va al usuario como P36.
3. **FIFO de `BLPOP` y "espacio local".** Respondidas (decisión 26): las dos respuestas son "sí" y van a las fichas.
4. **Umbral con `f32`.** Sí: todo umbral, cota y monto en USD pasa a `f64`. Un `f32` no representa 0,6 exacto, y la comparación con una cota en `f64` falla en el borde.
5. **P12b, P15, P16, P17 y P18.** Respondidas (decisiones 21 a 25). Quedan abiertas P17b, P18b, P21 y P34 a P36.

**Notas de Sonnet**
- **Fail-open de clippy.** El arreglo vale para todas las sondas con herramientas. Un reporte vacío, truncado o sin su marca de fin da `SinVeredicto`, nunca un valor limpio. La Tarea 7i revisa gitleaks, `cargo audit`, hadolint y `cargo-mutants` con la misma regla.
- **`~/.cache/root-dbg/` desapareció.** Borrar datos fuera del sandbox es zona roja. La causa no está probada, así que la Tarea 7i revisa todo borrado de Root. `limpiar_temporales` no debe seguir enlaces simbólicos ni salir de `$ROOT_HOME/tmp/`.
- **Escenarios de la 9d2.** Si la imagen construye, los `[[escenario]]` corren contra la imagen construida, como pide la 9d2. Si no construye, corren en el contenedor de evaluación. El informe dice dónde corrió cada uno.
- **Tests en rojo.** Los tests unitarios de la 7h se aceptan. Desde la 7i, cada test nuevo se ve en rojo antes de su implementación, y el mensaje del commit lo dice.

### Tarea 7i: Ajustes a las Tareas 6c, 7g, 7h, 9d2 y 15b
- **Action**: los desempates 1, 2, 3, 5 y 6 de esta vuelta, el desempate 4 de la vuelta anterior (`f64`) y las tres notas: fail-open, borrados y escenarios.
- **Validate**: `cargo test`:
  - Una imagen base que podman no puede bajar deja la construcción `SinVeredicto`, y el resto de la evaluación da su veredicto.
  - Con la imagen base ya presente en el almacén local y sin red, la construcción no intenta un pull.
  - `ARG BASE=debian:12` seguido de `FROM ${BASE}` se resuelve. `FROM ${SIN_VALOR}` da `SinVeredicto` con su línea.
  - Una CLI `colab` falsa que no responde hace salir a Root con código 3 en 60 segundos o menos.
  - Un `requirements.txt` con `--extra-index-url https://...` lista los dos índices y deja un hallazgo de severidad 1.
  - Un reporte vacío de gitleaks, `cargo audit`, hadolint o `cargo-mutants` da `SinVeredicto`.
  - Un enlace simbólico dentro de `$ROOT_HOME/tmp/` que apunta afuera se borra como enlace, y su destino queda intacto.
  - Una cota de Wilson exacta de 0,6 entra con `umbral_wilson = 0.6`.
  - Un escenario corre contra la imagen construida cuando la imagen construye, y el informe nombra el lugar.
- **Además**: Sonnet busca todo `remove_dir_all` y `remove_file` de `src/` (con `grep -w` y `codegraph callers`) y anota en el commit que ninguno sale de `ROOT_HOME`.

### Tarea 6c-B: Descarga de Python (decisión 21)
- **Action**: la mitad Python de la Tarea 6c: wheels con `pip download --only-binary=:all:`, sdist con `curl` desde la URL del índice con verificación de hash, y fuentes git o URL con `git clone` o `curl`. La evaluación instala sin red con `pip install --no-index --find-links /cache/python`. Las reglas y el `Validate` de Python son los de la 6c.
- **Orden**: después de la 7i. memora y brasa son Rust, así que la 6c-B no frena a G1 ni a G2.
- **Lo que faltó de la 7i** (entra en la 6c-B):
  - P34 a): la descarga corre `cargo` desde un directorio donde no descubre el `.cargo/config.toml` del repo. Hoy lo sigue leyendo.
  - El evento `Gate` de la fase `inventario` escribe también los hallazgos de Python, como el de `--extra-index-url`.
- **Validate extra**: un repo con `.cargo/config.toml` que define `[http] proxy = "http://127.0.0.1:9"` baja sus dependencias sin usar ese proxy.

**Avance (2026-10-28):** Sonnet cerró la 7i (commits d11c39a y c51a57c), con 942 tests verdes. La 6c-B no entró en la vuelta. 21 tests de la 7i ya pasaban antes de su implementación y quedaron como guardas. Se aceptan: cubren comportamiento que ya existía, y el commit los lista.

## Desempates técnicos 7 (Opus, 2026-10-28, después de la Tarea 7i)

Respuestas a las preguntas de Sonnet al cerrar la 7i. Los cambios de código van en la Tarea 7j. La pregunta sobre el costo de la T4 va al usuario como P37.

1. **Ctrl-C y Colab.** Sí. La Tarea 7h ya dice que Root libera siempre el runtime, y una señal no es una excepción.
   - Root maneja SIGINT y SIGTERM: libera el runtime, registra un evento `Gate` y después sale.
   - SIGKILL no se puede manejar. Por eso Root escribe el id del runtime en `$ROOT_HOME/colab/activo.json` al pedirlo, y lo borra al liberarlo. La próxima corrida de `--colab` libera primero un runtime que quedó anotado. `r00t oraculo pendientes --liberar` hace solo esa limpieza.
   - Toda corrida tiene un tope de duración, `[colab].minutos_max` en `root.toml`, con 30 minutos por defecto. Al llegar al tope, Root libera el runtime y sale con código 3. La decisión 33 cambia el default a 120 minutos y suma el corte temprano (Tarea 7l).
2. **Credencial residual.** Se acepta, con una condición: Root lee las direcciones con un parser de URL según RFC 3986, no con una expresión regular. En `https://tok/en@host`, la parte de usuario termina en la primera `/`. El host real es `tok`, y `en@host` es una ruta. Ninguna credencial viaja como autenticación, y el inventario registra el host real. Todo lo que el parser lee como usuario o clave se rechaza (6c). Un token escrito en una ruta es un secreto en el repo: lo detecta gitleaks (Tarea 9), no el filtro de fuentes.
3. **Cero mutantes.** Sigue en `SinVeredicto`, pero con dos causas distintas. Si `cargo-mutants` no dejó reporte, la causa es "no dejó reporte". Si dejó un reporte con 0 mutantes, la causa es "sin mutantes para medir". Ninguno de los dos casos es un puntaje: sin mutantes, Root no mide la calidad de los tests.
4. **Varios Dockerfiles.** Root elige uno, por este orden:
   1. el campo `containerfile` de `tarea.toml`, si existe;
   2. `Dockerfile` o `Containerfile` en la raíz del repo;
   3. el único Dockerfile del repo, si hay uno solo.
   Sin elección posible, los escenarios corren en el contenedor de evaluación, y el informe dice "varios Dockerfile, ninguno elegido". Todos los Dockerfiles se siguen revisando en forma estática.
5. **Imagen sin `sh`.** Se prueba con `FROM scratch` y un `COPY` de un archivo. Esa imagen no tiene `sh` y se construye sin red, porque `scratch` no se baja.
6. **Causa "pide credenciales".** Se queda, solo cuando el registro responde un pedido explícito de autenticación, como un 401 con `authentication required`. El texto ambiguo de Docker Hub sigue con la causa del desempate 1 de "Desempates técnicos 6". En los dos casos, la construcción da `SinVeredicto`.
- **Test de Colab con plazo real.** Un test que espera 60 segundos de verdad es inestable en una máquina cargada. El plazo se inyecta: en los tests vale 1 segundo, y en producción, 60.

### Tarea 7j: Ajustes a las Tareas 7h, 7i, 9d2 y 9e
- **Action**: los desempates 1 a 6 de arriba y el plazo inyectable de Colab.
- **Validate**: `cargo test`:
  - Con una CLI `colab` falsa, un SIGINT durante la corrida libera el runtime y deja un `Gate`.
  - Un `activo.json` de una corrida anterior hace que `--colab` libere ese runtime antes de pedir otro.
  - Una corrida que pasa `minutos_max` libera el runtime y sale con código 3.
  - `https://usuario:clave@host/x` se rechaza. `https://tok/en@host` registra el host `tok`.
  - Un reporte de `cargo-mutants` con 0 mutantes da `SinVeredicto` con la causa "sin mutantes para medir".
  - Con `Dockerfile` en la raíz y `docker/otro.Dockerfile`, los escenarios usan el de la raíz. Con dos Dockerfiles fuera de la raíz y sin `containerfile`, el informe dice "ninguno elegido".
  - Una imagen `FROM scratch` hace que los escenarios corran en el contenedor de evaluación, con aviso.
  - Un 401 explícito del registro da la causa "pide credenciales".
  - El test del plazo de Colab termina en menos de 5 segundos.

**Avance (2026-10-28):** Sonnet cerró la 7j (583a66a) y la 6c-B (573b2c4), con 1022 tests verdes y 1 ignorado. La 6c-B hizo que la descarga ignore el `.cargo/config.toml` del repo (P34 a). La decisión 30 cambia esa regla: la Tarea 6d la reemplaza.

## Desempates técnicos 8 (Opus, 2026-10-28, después de la 6c-B)

Respuestas a las siete preguntas de Sonnet de la build-12 y diseño de las decisiones 27 a 31. Los cambios de código van en las Tareas 6e, 6d, 9d3, 15d y 7k.

**Preguntas de Sonnet**
1. **Herramientas de build de Python.** Se bajan en la descarga, no en la imagen. Son dependencias del proyecto, y la regla vigente prohíbe precargar dependencias en la imagen. Para cada sdist o fuente git, Root lee `[build-system].requires` de su `pyproject.toml` sin ejecutar nada, y baja esos wheels. Sin `pyproject.toml`, baja `setuptools` y `wheel`, que es el respaldo de PEP 517. La evaluación instala sin red desde `/cache/python`. `pip` ya está en la imagen.
2. **Nombres de Dockerfile.** Sí: una sola función reconoce `Dockerfile`, `Containerfile`, `*.Dockerfile` y `*.Containerfile`. La usan la elección de Dockerfile y las sondas de `configs.rs`.
3. **`unauthorized` sin más.** No va a infraestructura. Infraestructura es red caída, timeout o error de podman. Un `unauthorized` sin "authentication required" da la causa ambigua del desempate 1 de "Desempates técnicos 6": "imagen base inaccesible". Las dos causas dan `SinVeredicto`, así que la diferencia solo afecta el texto del informe. Con una credencial configurada (decisión 29), un rechazo da la causa "credencial rechazada".
4. **Salida de `colab stop`.** Hasta verificarla en la corrida real, una salida que Root no reconoce cuenta como "liberación no confirmada". Root conserva `activo.json` y reintenta la liberación en la próxima corrida. Ante la duda, Root supone que el runtime sigue encendido, porque un runtime encendido gasta.
5. **Tope de Colab en un test.** No se agrega una variable de entorno. `[colab].minutos_max` pasa a `f64`, y el test usa `0.01` (0,6 segundos). Una variable que acorta topes en producción es un riesgo sin necesidad.
6. **Cero mutantes.** Confirmado por el desempate 3 de "Desempates técnicos 7". La dimensión Pruebas queda `SinVeredicto` con la causa "sin mutantes para medir", y `Informe.confianza` lo lista.
7. **Credencial sin separador en la ruta.** Confirmado por el desempate 2 de "Desempates técnicos 7": lo cubre gitleaks.

**Diseño del juez de configuración de cargo (decisión 30)**
- **Qué decide la configuración.** La descarga respeta el `.cargo/config.toml` del repo, como pidió el usuario con la opción c).
- **Qué hace el oráculo, sin LLM.** Todo lo checkeable va a reglas deterministas, por el invariante del GOAL:
  - Root lee cada `.cargo/config.toml` que `cargo` descubriría desde el repo.
  - Clasifica cada entrada: `[registries]`, `[source]`, `[http]`, `[net]`, `[env]`, `[build]`, `[target]` y `[alias]`.
  - Suma al inventario cada entrada que toca la red: registros, fuentes, proxy y `git-fetch-with-cli`. El `Gate` del inventario las lista antes de usar la red.
  - Un proxy o un registro con credenciales escritas en el repo es un secreto: da un hallazgo de seguridad de severidad 3. Root no lo bloquea, porque rige la opción c).
- **Qué decide el juez.** Para cada entrada, el juez dice si el proyecto la necesita: `necesaria`, `innecesaria` o `riesgosa`. Cada respuesta cita la entrada y el motivo, por ejemplo un crate de `Cargo.toml` que vive en ese registro. Esto no es checkeable: pide entender para qué existe cada entrada.
- **Qué ve el juez.** Las entradas del config, `Cargo.toml`, el inventario y la ficha del spec. No ve el código del proyecto ni los tests ocultos.
- **Cómo falla.** El juez corre con `r00t route --rol juzga`, con razonamiento prendido, y su modelo es Sonnet (decisión 30). El gate de evidencia descarta una respuesta sin cita. Un JSON roto o un error del proveedor da `SinVeredicto` en ese juicio, y la descarga sigue según la opción c).
- **Qué produce.** Cada `innecesaria` o `riesgosa` da un hallazgo de entrega con su cita. Esos hallazgos entran al informe.
- **Qué queda fuera del juez.** El juez no abre ni cierra la red, y no decide qué entradas usa `cargo`. Un LLM que decide sobre proxies o credenciales actúa en la zona roja sin gate humano, y eso choca con el GOAL. El usuario lo confirmó con P38 a) (decisión 35).

**Credenciales de registros (decisión 29)**
- `root.toml` suma `[[registro]]` con `nombre`, `url` y `credencial_env`. Solo el humano escribe esa lista.
- Root pasa la credencial solo si el host del registro coincide exacto con una `url` de la lista. Un repo no puede pedir la credencial para otro host: así no la saca hacia un servidor ajeno.
- La credencial entra al contenedor de descarga como variable de entorno: `CARGO_REGISTRIES_<NOMBRE>_TOKEN` para cargo, y un archivo `netrc` temporal para pip. Root nunca la pasa al contenedor de evaluación ni a la construcción con red de la 9d3.
- Las imágenes base privadas usan el mismo mecanismo con `podman pull --authfile`, desde un archivo temporal que Root borra al terminar.
- La credencial no aparece en `eventos.jsonl`, en un `Gate`, en stderr ni en un error.

**Construcción con red como respaldo (decisión 27)**
- Root intenta primero sin red (Tarea 9d2). Solo si la construcción queda `Pendiente` por falta de red, la repite con red.
- La repetición registra un `Gate` antes de empezar, con las líneas `RUN` que necesitan red.
- El contexto de la construcción es la copia del repo. No monta `ROOT_HOME` ni `/oraculo`, y no recibe credenciales.
- Usa la red de usuario de podman sin acceso al host (`allow_host_loopback=false`), con el mismo timeout de la evaluación.
- Si construye, la construcción queda verde y deja un hallazgo de entrega de severidad 1: "la imagen necesita red para construirse". Si falla, el hallazgo es "la imagen no construye", sin cambios.

**Hito 1 y hito 4 en P18b (decisión 28)**
- **Hito 1 (b, datos):** `Fila` suma, solo agregando, `forma: Forma` con `#[serde(default)]`. `Forma` lista los roles de la corrida en orden, si hubo verificador y la configuración. La Tarea 16 la llena. Hasta entonces, solo hay filas sintéticas.
- **Hito 1 (a, propuestas):** Aprende agrupa las filas por `forma` y propone un cambio de forma con la misma regla de la Tarea 15c: cota de Wilson ≥ `umbral_wilson` y al menos 3 filas por forma. Route no lee esas propuestas. El usuario las aplica a mano.
- **Hito 4 (CAB):** generar formas y arquitecturas de agentes nuevas, construirlas y medirlas con `r00t corre`. CAB lee las `Fila` con su `forma` y las propuestas de Aprende. Nada de eso entra al hito 1.

### Tarea 6e: Ajustes a las Tareas 7j y 6c-B
- **Action**: los desempates 1 a 5 de arriba.
- **Validate**: `cargo test`:
  - Un sdist cuyo `pyproject.toml` pide `setuptools>=68` baja ese wheel, y la evaluación lo instala sin red.
  - `docker/app.Dockerfile` cuenta para la elección de Dockerfile y para las sondas de `configs.rs`.
  - Un `unauthorized` sin "authentication required" da la causa "imagen base inaccesible", no infraestructura.
  - Una salida desconocida de `colab stop` conserva `activo.json`.
  - El tope de Colab con `minutos_max = 0.01` libera el runtime en menos de 5 segundos.

### Tarea 6d: Credenciales de registros y config de cargo con juez (decisiones 29 y 30)
- **Action**: el diseño de "Credenciales de registros" y del "juez de configuración de cargo". La descarga vuelve a respetar el `.cargo/config.toml` del repo.
- **Validate**: `cargo test --test descarga`:
  - Un registro de `root.toml` con su credencial en una variable de entorno se baja, y el valor no aparece en `eventos.jsonl`, en stderr ni en los `Gate`.
  - Un repo que apunta un registro a otro host no recibe la credencial.
  - Un `.cargo/config.toml` con `[http] proxy` suma el proxy al inventario, y su `Gate` sale antes de la red.
  - Un proxy con usuario y clave en el config da un hallazgo de seguridad de severidad 3.
  - Con un juez falso que dice `innecesaria`, la descarga igual usa la entrada y el informe tiene el hallazgo de entrega.
  - Con un juez falso que devuelve JSON roto, ese juicio queda `SinVeredicto` y la descarga sigue.

### Tarea 9d3: Construcción con red como respaldo (decisión 27)
- **Action**: el diseño de "Construcción con red como respaldo".
- **Validate**: `cargo test --test entrega`:
  - Un Dockerfile con `RUN apt-get install` queda `Pendiente` sin red, se repite con red y deja el hallazgo de severidad 1.
  - Un Dockerfile que construye sin red no se repite con red.
  - La construcción con red no recibe credenciales ni monta `ROOT_HOME`.
  - El `Gate` previo lista las líneas `RUN` que necesitaron red.

### Tarea 15d: Forma de la corrida y propuestas de forma (decisión 28, hito 1)
- **Action**: el diseño de "Hito 1 y hito 4 en P18b", solo la parte del hito 1.
- **Validate**: `cargo test aprende`:
  - Una `Fila` vieja sin `forma` se sigue leyendo.
  - Con filas sintéticas donde una forma sin verificador acierta igual y cuesta menos, hay una propuesta de forma y Route no cambia.
  - Con 2 filas por forma no hay propuesta.

### Tarea 7k: Rust en Colab (decisión 31)
- **Action**: la preparación del runtime baja `rustup-init` para x86_64 desde `static.rust-lang.org`, verifica su `.sha256` y recién después lo corre. Si la descarga, el hash o la instalación fallan, Root compila brasa con la imagen de la tarea (hoy `root-oraculo-rust:2`) en la máquina del usuario y sube el binario. Si el binario no corre en Colab, el test queda `Pendiente` con la causa. Root registra un `Gate` con el camino que usó.
- **Validate**: `cargo test oracle` con una CLI `colab` falsa:
  - Un `.sha256` que no coincide no corre `rustup-init` y pasa al binario compilado.
  - Un hash correcto corre `rustup-init` y no compila en la máquina local.
  - Ningún camino corre `curl ... | sh`.
- **Corrida real**: espera a G1. P37 ya está respondida (decisión 33).

### Tarea 7l: Tope de 2 horas y corte temprano en Colab (decisión 33)
- **Tope**: `[colab].minutos_max` vale 120 por defecto. Al llegar al tope, Root libera el runtime, registra un `Gate` con la causa "tope de 2 horas" y sale con código 3. Los tests sin resultado quedan `Pendiente`.
- **Qué pedía cada corrida.** Hoy hay un solo tipo de corrida en Colab: `r00t oraculo pendientes --colab`. Lo que pide es checkeable:
  - el junit de `correr.sh` existe en el runtime y parsea;
  - cada id de test del paquete aparece en ese junit;
  - cada id tiene tantos resultados como `repeticiones` de `tarea.toml`, y cada resultado es pasa, falla, error u omitido.
  La preparación de Rust (Tarea 7k) no es una corrida aparte: es el primer paso de la misma corrida.
  Un tipo de corrida nuevo no se lanza hasta que el plan escriba su criterio checkeable, igual que este.
- **Cuándo corta Root.** Root libera el runtime con lo que pase primero:
  1. se cumple lo que pedía la corrida;
  2. `correr.sh` termina, con el junit completo o no;
  3. se llega al tope de 2 horas.
  En el caso 1, Root baja el junit, lo importa y libera el runtime aunque `correr.sh` siga corriendo. Registra un `Gate` con la causa "requisito cumplido".
- **Quién corta.** El usuario nombró al planner y a un hook o script. Cada uno tiene un papel:
  - **El script (corte automático).** Mientras corre la corrida, Root revisa el junit en el runtime cada 60 segundos y aplica la regla de arriba. Si la CLI `colab` no puede leer un archivo del runtime durante la corrida, el corte 1 se vuelve igual al corte 2. Sonnet lo verifica con `colab --help` y lo anota en el commit.
  - **El vigía (red de seguridad).** `r00t oraculo pendientes --vigilar` lee `$ROOT_HOME/colab/activo.json`. Si el runtime anotado cumplió lo que pedía, o pasó las 2 horas, lo libera. Corre desde un timer de systemd de usuario cada 5 minutos en la laptop. Cubre el caso en que Root murió con SIGKILL y nadie volvió a correrlo.
  - **El planner o el orquestador (corte manual).** Cualquier sesión puede cortar con `r00t oraculo pendientes --liberar`. El planner define en el plan el criterio de cada tipo de corrida. Ningún agente sube el tope: eso es zona roja (dinero) y lo hace el usuario en `root.toml`.
- **Validate**: `cargo test oracle` con una CLI `colab` falsa:
  - Un junit completo mientras `correr.sh` sigue corriendo libera el runtime y deja el `Gate` "requisito cumplido".
  - Un junit al que le falta una repetición no corta antes de tiempo.
  - Con `minutos_max = 0.01`, el tope libera el runtime y deja el `Gate` "tope de 2 horas".
  - `--vigilar` con un `activo.json` vencido libera el runtime. Con uno vigente y sin junit completo, no hace nada.
  - Sin `minutos_max` en `root.toml`, el tope vale 120.
- **En la laptop**: instalar el timer de systemd del vigía y probarlo una vez con la CLI falsa.

## Nube o laptop (decisión 34, hasta el 2026-11-05)

Las sesiones en la nube no tienen podman, `ROOT_HOME` real ni Colab. Cada tarea pendiente se parte en una parte de nube y una parte de laptop. La parte de laptop junta todo lo que necesita podman, la red real de un registro o Colab.

| Tarea | Nube (sin podman, sin `ROOT_HOME`, sin Colab) | Laptop |
|---|---|---|
| 20 | Toda la tarea | Correr la suite completa una vez y comparar |
| 15d | Toda la tarea: es Rust puro con filas sintéticas | Nada |
| 6e | Desempates 2 a 5; lectura de `[build-system].requires` sin ejecutar nada | El test que instala sin red el wheel de build en la evaluación (podman); quitar el `#[ignore]` de `tests/descarga.rs` |
| 6d | Lista `[[registro]]`, coincidencia exacta de host, argumentos del contenedor, clasificación del config, juez con proveedor falso, ausencia de la credencial en eventos y errores | Descarga real con un registro de prueba y una credencial, y proxy real del config (podman y red) |
| 9d3 | Regla de repetir con red, `Gate` previo, argumentos de `podman build` (sin `ROOT_HOME`, sin credenciales, `allow_host_loopback=false`) | Construcción real con red y sin red de un Dockerfile con `apt-get` |
| 7k | Elección de camino con CLI falsa: hash bueno, hash malo, sin `curl … \| sh` | Compilar brasa en la imagen como respaldo (podman) |
| 7l | Toda la lógica con CLI falsa | Timer de systemd del vigía |

La corrida real de Colab espera a G1 y se hace en la laptop.

### Tarea 20: Suite apta para la nube (decisión 34)
- **Problema**: `tests/contenedor.rs`, `descarga.rs`, `entrega.rs`, `mutacion.rs`, `oraculo.rs`, `configs.rs` y `sondas.rs` llaman a podman y no se saltean sin él. El hook pre-commit corre `cargo test` entero, así que en la nube ningún commit pasa.
- **Action**:
  - Un test que necesita podman, la red real o archivos de la laptop llama a una guarda común. La guarda saltea el test solo si `ROOT_SIN_PODMAN=1`, y lo dice en stderr con el nombre del test.
  - Sin esa variable, el test corre siempre y falla si falta podman. Así un salteo nunca pasa en silencio en la laptop.
  - La sesión en la nube exporta `ROOT_SIN_PODMAN=1` y corre `git config core.hooksPath .githooks`.
  - El primer paso en la nube es correr `cargo test` y anotar en el commit cada test que falló por el entorno.
- **Validate**:
  - En la nube, `ROOT_SIN_PODMAN=1 cargo test` pasa, y stderr lista los tests salteados.
  - En la laptop, `cargo test` sin la variable corre la misma cantidad de tests que antes de la tarea.
- **Regla de merge**: el trabajo hecho en la nube entra a la laptop solo después de que `cargo test` pasa completo en la laptop, sin `ROOT_SIN_PODMAN`.

**Orden siguiente (2026-10-28, en la nube hasta el 2026-11-05):** 20, 15d y después las partes de nube de 6e, 6d, 9d3, 7k y 7l. Las partes de laptop se juntan en una sola vuelta en la laptop, en el mismo orden.

**Orden anterior (2026-10-28, después de la 6c-B):** 6e, 6d, 9d3, 15d y 7k.
- La 6e va primero: es chica y cierra la instalación sin red de Python.
- La 6d va antes de la 9d3: la 9d3 debe probar que no recibe credenciales.
- Después de la 7k, lo que queda espera a G1, a G2 o a P37.

**Preguntas respondidas (2026-10-28)**
- P12b: b (decisión 21). P15: b (decisión 22). P16: a (decisión 23). P17: c+b (decisión 24, en parte). P18: c, más una segunda parte (decisión 25, en parte). P19 y P20: sí (decisión 26).

**Preguntas abiertas (sin responder)**
- **P17b.** Respondiste "c+b" para construir el Dockerfile. La parte c) ya está en el plan. ¿Cómo entra la parte b), que construye con red y corre el código de los `RUN` con red?
  - a) Como respaldo automático: si la construcción sin red queda `Pendiente`, Root la repite con red.
  - b) Como respaldo con tu aprobación: Root la repite con red solo después de que vos la apruebes, caso por caso.
  - c) Siempre las dos: Root construye sin red y con red, y compara los resultados.
  
  Hasta tu respuesta, Root no construye con red.
- **P18b.** Respondiste "c pero que tambien evolucione a esquemas o arquitecturas de agentes". La parte c) ya está en el plan. ¿Qué querés que evolucione?
  - a) En el hito 1, Aprende propone también cambios de forma: qué roles hay, en qué orden y si hay un verificador. Vos los aplicás a mano, igual que en c).
  - b) Eso va al hito 4 (CAB), que genera y mide arquitecturas de agentes. El hito 1 solo guarda en cada `Fila` la forma de la corrida para que CAB la use.
  - c) Otra cosa. En ese caso, describila en una oración.
  
  Hasta tu respuesta, Aprende propone solo modelos por rol.
- **P21.** La decisión 21 acepta "otros registros", incluso uno privado. Un registro privado pide credenciales, y pasar credenciales al contenedor de descarga es zona roja (claves). ¿Qué elegís?
  - a) Solo registros públicos. Un registro que pide credenciales da `SinVeredicto`.
  - b) Root lee la credencial de una variable de entorno nombrada en `root.toml`, la pasa solo al contenedor de descarga y nunca la escribe en un log.
  
  Hasta tu respuesta, rige a).
- **P34. Configuración de cargo del repo.** Un repo puede traer `.cargo/config.toml` con `[http]` o `[net]`: un proxy, quizás con credenciales. `cargo` lo usaría en el contenedor de descarga, y el inventario no lo ve. ¿Qué elegís?
  - a) La descarga ignora la configuración de cargo del repo: corre `cargo` desde otro directorio. Costo: un repo que define registros o fuentes ahí no baja sus dependencias y da `SinVeredicto`.
  - b) Root lee esa configuración y suma al inventario sus `[registries]` y `[source]`. Rechaza `[http]`, `[net]` y toda credencial. Costo: más reglas que mantener.
  - c) La descarga respeta la configuración entera. Riesgo: el repo decide por dónde sale la red, y eso escapa al inventario.
  
  Hasta tu respuesta, rige a). Mi recomendación es b): mantiene el inventario completo sin cortar registros legítimos.
- **P35. Rust en Colab.** La preparación del runtime de Colab corre `curl https://sh.rustup.rs | sh` sin verificar ningún hash. Ese código corre en el runtime de tu cuenta de Google, junto a los tests ocultos. ¿Qué elegís?
  - a) Aceptarlo como está. Riesgo: si el sitio de rustup se compromete, ese código corre en tu sesión de Colab.
  - b) Bajar `rustup-init` para x86_64 desde `static.rust-lang.org` y verificar su `.sha256` antes de correrlo. Costo: unas líneas más. Límite: el hash viene del mismo origen, así que protege de una descarga corrupta, no de un origen comprometido.
  - c) Compilar brasa en tu máquina, dentro de `root-oraculo-rust:2`, y llevar a Colab solo el binario. Costo: el binario puede no correr si la glibc de Colab es más vieja que la de la imagen. Colab ya no necesita Rust.
  
  Hasta tu respuesta, la corrida real de la 7h no se hace. Mi recomendación es c), con b) como respaldo si la glibc no coincide.
- **P36. "Outcome" del hito 1 en el PRD.** Sonnet cambió el "Outcome" del hito 1 en `.claude/prds/sistema-nuevo.prd.md` (Tarea 19, commit 732e32d). Antes decía "Route, Oracle, Observe y Track corren desde el bootstrap del banco". Ahora dice "Route, Oracle (`evaluar`), Observe y Track corren; calibración, informe y `corre` siguen pendientes (Tareas 12, 13b y 16)". ¿Qué elegís?
  - a) Revertirlo. El "Outcome" es la meta del hito, y la columna "Status" ya dice `in-progress`.
  - b) Dejarlo así.
  - c) Volver a la meta original y anotar el avance en otra línea.
  
  Mi recomendación es a): el texto nuevo mezcla la meta con el avance.
- **P37. Costo de la T4 en Colab.** Respondiste P15 con "solo T4" porque el plan decía que la T4 es gratis. Eso vale para una cuenta gratuita de Colab. En una cuenta paga (Pro o pago por uso), la T4 también gasta compute units. ¿Cuál es tu caso?
  - a) Cuenta gratuita: la T4 no gasta. Rige el tope de 30 minutos por corrida.
  - b) Cuenta paga, y aceptás el gasto de la T4 con el tope de 30 minutos por corrida.
  - c) Cuenta paga, y cada corrida en Colab pide tu aprobación antes de empezar.
  
  Hasta tu respuesta, no hay corrida real en Colab. Esa corrida ya espera a G1 y a P35.

**Respuestas del 2026-10-28:** P17b "a" (decisión 27), P18b "a y b" (decisión 28), P21 "b" (decisión 29), P34 "C pero con un juez (sonnet)…" (decisión 30), P35 "b con c de respaldo" (decisión 31) y P36 "a" (decisión 32). P37: "P37 B pero sin limite de tiempo", precisada después como un tope de 2 horas con corte temprano (decisión 33).

- **P38. Papel del juez de la config de cargo.** Pediste respetar el `.cargo/config.toml` del repo con un juez Sonnet que delimite necesidades. Hoy el juez solo informa: sus hallazgos van al informe, y `cargo` usa todo el config. ¿Qué querés?
  - a) El juez solo informa. Esta opción rige hoy y respeta el GOAL.
  - b) El juez decide qué entradas usa `cargo`, sin un humano. Choca con el GOAL: un LLM decidiría sobre proxies y credenciales, que son zona roja, sin gate humano. Para elegir b), tenés que cambiar el GOAL a mano.
  - c) El juez propone qué entradas sacar, y vos aprobás antes de que la descarga deje de usarlas. Costo: un gate humano por proyecto con entradas dudosas.
  
  Mi recomendación es a) para el hito 1, y c) si un caso real lo pide.
- **P39. memora como banco.** Escribiste: "No se si es rentable usar memora, la idea es armar root, graft, y acoplar ECC y REA". memora y brasa no son productos: son las únicas tareas con tests ocultos y pares ECC contra SDLC ya medidos. Con ellas, Root se mide a sí mismo, como pide el GOAL. Hoy dependen de ellas las Tareas 7, 9c, 12, 12b, 13, 13b y 16. ¿Qué elegís?
  - a) Conservar memora y brasa como banco del hito 1. Costo: aprobar sus fichas (G1), calificar 6 paquetes a ciegas (G3) y una corrida completa con Haiku en la Tarea 16.
  - b) Conservar solo brasa. Costo: la mitad de G1 y de G3. Límite: un solo par conocido para la validez, y brasa tiene el test de GPU pendiente.
  - c) Conservar memora y brasa solo para calibrar (Tareas 12 y 12b), y cambiar la Tarea 16 para que `r00t corre` mida una tarea de Graft. Costo: esa tarea de Graft todavía no tiene oráculo oculto. Hay que escribirlo, o la corrida queda sin veredicto de completitud.
  - d) Sacar memora y brasa y medir Root sobre Root y Graft. Costo: no hay tests ocultos ni pares con resultado conocido, así que las Tareas 12b y 13 pierden su referencia externa. El GOAL pide que el evaluador se mida, y eso queda sin datos hasta crear un banco nuevo.
  
  Mi recomendación es a) o c): el GOAL exige medir al evaluador, y hoy solo memora y brasa dan esa medida.

**Preguntas ya respondidas (texto original)**
- **P12b.** Respondiste "P12 si" a una pregunta con dos opciones. ¿Cuál elegís?
  - a) La descarga acepta solo crates.io y PyPI. Hoy rige esta.
  - b) La descarga acepta además dependencias por git y otros registros, como npm o uno privado. Esta opción amplía la red de la zona roja.
  
  Tu respuesta también decide los dos huecos del filtro de la Tarea 6b. Con a), se cierran. Hasta tu respuesta, siguen abiertos.
- **P15.** ¿Aprobás correr en Colab con GPU los tests pendientes por entorno? Hoy el único es `test_rendimiento[gpu-10000-res1]` de brasa. Una GPU T4 es gratis; una A100 o H100 consume compute units. Hay tres respuestas posibles:
  - a) No: los tests quedan `Pendiente` y vos los corrés a mano cuando quieras.
  - b) Sí, solo con T4, y un agente lo corre.
  - c) Sí, con A100 o H100, y un agente lo corre. Cada corrida pide tu aprobación.
- **P16. Herramientas en la imagen.** `oraculo-rust` no trae `clippy`, `gitleaks`, `cargo-audit`, `hadolint` ni `cargo-mutants`. Sin ellas, cuatro dimensiones salen `SinVeredicto`: seguridad, estructura, pruebas y la parte de hadolint de entrega. Instalarlas necesita red al construir la imagen, no al evaluar. ¿Qué elegís?
  - a) Instalarlas en la imagen, con versiones fijas y hash verificado. La red se usa una vez, al construir la imagen. Costo: la imagen crece unos cientos de MB, y compilar `cargo-audit` y `cargo-mutants` tarda varios minutos. Riesgo: confiar en esas cinco herramientas. Las versiones fijas y los hashes acotan ese riesgo.
  - b) No instalarlas. Costo: las cuatro dimensiones quedan `SinVeredicto`, la Tarea 12 no puede calibrarlas y no entran a `global`. El criterio "nueve dimensiones" de la Tarea 13 no se cumple.
  - c) Correrlas en el host, fuera del contenedor. Riesgo: `cargo-mutants` compila y corre el código del builder en tu máquina, con red y sin aislamiento. No lo recomiendo.
  
  Mi recomendación es a): es la única opción que mide las nueve dimensiones sin correr código ajeno fuera del contenedor.
- **P17. Construir el Dockerfile del proyecto.** La 9d hoy solo lo revisa en forma estática. ¿Qué elegís?
  - a) Seguir solo con la revisión estática. Costo: nada nuevo. Límite: Root no sabe si la imagen construye ni si arranca, y la Tarea 9d queda incompleta.
  - b) Construirlo con red. Riesgo: cada `RUN` del Dockerfile es código del builder, y correría con red. Puede sacar datos o bajar cualquier cosa. Es más que la fase de descarga, que no corre scripts.
  - c) Bajar con red solo la imagen base (`FROM`), sin correr nada, y construir con `--network none`. Un `RUN` que necesita red falla, y la construcción queda `Pendiente` con su motivo: no se cobra al producto. Costo: la red baja imágenes de un registro, igual que en la fase de descarga. Límite: un Dockerfile que instala paquetes con `apt-get` queda `Pendiente`.
  
  Mi recomendación es c): prueba la construcción sin correr código ajeno con red.
- **P18. Lo que aprende Root y los roles fijos.** Hoy Route respeta `root.toml` para Planifica, Organiza y Construye (Opus, Sonnet, Haiku, tu decisión). Por eso la política que calcula Aprende no cambia esos tres roles. Sí cambia Delega, Verifica y Juzga. El GOAL dice que lo que Root registra cambia la próxima elección, y también que nada se auto-aplica sin validación. ¿Qué elegís?
  - a) Los tres roles quedan fijos en `root.toml`. Aprende solo informa qué modelo rendiría mejor. Costo: Root no aprende en esos roles.
  - b) Route usa la política aprendida en esos roles cuando hay datos suficientes (cota de Wilson ≥ 0,6 y al menos 3 filas por modelo). Riesgo: un cambio de builder entra sin que lo mires.
  - c) Aprende propone el cambio en `r00t aprende`, con su evidencia. Vos lo aplicás a mano en `root.toml`. Costo: un gate humano por cada cambio.
  
  Hoy rige a). Mi recomendación es c): Root aprende y tu decisión sobre los roles no cambia sin tu aprobación.

**Respuestas del 2026-10-28 (decisión 36):** P39 c y P44 c. P39 queda cerrada.

- **P48. ¿Qué tarea de Graft mide la Tarea 16?** Graft todavía no tiene PRD de hito ni plan. La tarea debe ser chica, checkeable y útil para el hito 2.

  - a) Lector de planes: lee un `plan.md` como el de Root y emite en JSON sus tareas, dependencias y comandos `Validate`. Se escribe en Rust y se usa como CLI.
  - b) Gate de etapa: recibe una tarea y un commit, corre su `Validate` y registra el resultado con `r00t observa evento`. Se escribe en Rust y se usa como CLI.
  - c) Esperar al plan del hito 2 y elegir desde ahí. Hasta entonces, la Tarea 16 se valida solo con la tarea de fixture.

  Mi recomendación es a): no depende de otras piezas de Graft y su salida se compara exacta.

- **P49. ¿La Tarea 16 corre también memora?** La opción c de P39 decía "cambiar la Tarea 16 para que mida una tarea de Graft". El orquestador lo resumió como "además". Las dos lecturas cuestan distinto.

  - a) Solo Graft. memora queda para calibrar. Esta opción rige hoy, porque sigue el texto de c.
  - b) Graft y memora. Costo: una corrida completa más con Haiku sobre memora.

**Respuestas del 2026-10-28 (decisión 37):** P48 "P48 a pero emita sus tareas en toml". P49: el usuario respondió "P49 c", pero P49 tiene solo a y b. El orquestador le pidió que aclare. Hasta la aclaración, rige a).

**Respuesta del 2026-10-28 (decisión 38):** P49 "P49 b: la Tarea 16 corre Graft y memora." P49 queda cerrada.

## Revisión de nube (2026-10-28): verificación de Opus

Una sesión de nube revisó el crate en el commit 573b2c4 con `/code-review high` y cuatro revisores. El informe vive en la rama `claude/amazing-ptolemy-q9kkem`, en `docs/revision/2026-10-28-revision-nube.md`. Encontró 4 hallazgos críticos, 11 altos, 16 medios y 19 bajos.
Opus verificó cada crítico y cada alto en el código actual. El código de `src/` no cambió entre 573b2c4 y 61d279d. Un hallazgo entra solo si el defecto está en el código.

| Id | Veredicto | Motivo |
|---|---|---|
| C1 | Confirmado | `requisitos_pip` deja una ruta local como especificador, y `es_directa` no la reconoce. Opus corrió el mismo `pip download --only-binary=:all:` sin red con un backend PEP 517 propio: pip lo ejecutó y creó el archivo marca. En la descarga eso pasa con red y con `/repo` montado. |
| C2 | Confirmado | `juez_de` (en `lib.rs`) arma la `Eleccion` a mano, y `ficha/juez.rs` llama a `proveedores::llamar` sin `chequear_tope`. Un modelo sin precio queda registrado con 0 USD. `oracle/juez.rs` tiene la misma llamada directa. |
| C3 | Confirmado | `Precio` usa `#[serde(default)]`, no rechaza claves desconocidas y `validar` no revisa precios. Con NaN, `estimado > margen` es falso y el chequeo pasa. |
| C4 | Confirmado | `llamar_con_chequeo` lee el gasto, compara y llama sin lock ni reserva. El evento se escribe recién al terminar la llamada. |
| A1 | Confirmado | `pendientes` copia los tests ocultos y `conftest.py` a `$ROOT_HOME/pendientes/`, fuera de `oraculos/`. `limpiar_temporales` solo borra `evaluar-*`. |
| A2 | Confirmado | Las dos suites montan `tarea.dir` entero, con `oculto/`, en `/oraculo`. Sin junit, el error `Infra` lleva hasta 4000 caracteres del stderr del contenedor, donde corre el código del builder. |
| A3 | Confirmado | Ningún código emite `TipoEvento::OraculoLeido`. Solo lo leen `resumir_eventos` y los tests. |
| A4 | Confirmado | La clave del junit es el último tramo de `classname` más el nombre. Dos clases con el mismo nombre en archivos distintos chocan, y el mapa guarda solo una. |
| A5 | Confirmado | Si existe el junit, el código de salida de pytest no se mira. `/salida` es escribible por el código bajo prueba. |
| A6 | Confirmado | Un timeout es error de red y se reintenta hasta 3 veces con un solo chequeo previo. El evento de falla queda con 0 USD. |
| A7 | Confirmado | La estimación usa un token cada tres caracteres. No es el peor caso. |
| A8 | Confirmado en parte | `curl https://sh.rustup.rs \| sh` existe en `GUION_PREPARAR`: ya lo corrige la Tarea 7k. El paquete de Colab lleva el código del builder y los tests ocultos a un runtime con internet: eso es una decisión del usuario (P51). |
| A9 | Confirmado (latente) | `archivos_rs` recorre solo `src/` e ignora errores de lectura. `resultado` siempre da un valor, así que sin archivos el puntaje es perfecto. |
| A10 | Confirmado (latente) | Un test omitido en todas las corridas no entra a `estables`, y el universo de los requisitos sale de `estables`. Un requisito con un test omitido puede quedar Cumplido. |
| A11 | Dudoso | `entrega.rs` y `configs.rs` no detectan `ADD` con una URL. Que buildah baje esa URL con `--network none` sale de su documentación, y nadie lo probó. Se verifica en la laptop. |
| B10 | Confirmado y elevado a alto | `anthropic.rs` nunca envía `Eleccion.razonamiento`. Un juez de Anthropic, como el Sonnet de la decisión 30, corre sin razonamiento. Eso rompe el invariante "el verificador razona". |

Ningún crítico ni alto quedó refutado. Los medios y bajos no se verificaron uno por uno: se agrupan en las tareas de abajo según el área.

### Tarea 21: Topes y llamadas a la API (C2, C3, C4, A6, A7, B10; gravedad crítica)
- **Action**:
  - Toda llamada a un proveedor pasa por una sola función con chequeo de tope, también el juez de la ficha y el juez del Oracle. `proveedores::llamar` deja de ser pública fuera de esa función.
  - `Precio` exige `usd_entrada` y `usd_salida`, rechaza claves desconocidas y exige valores finitos y no negativos. `validar` revisa cada precio. Un modelo sin precio se rechaza antes de enviar, en todos los caminos.
  - Reserva: con un lock sobre `eventos.jsonl`, Root lee el gasto, compara y escribe un evento de reserva con el costo estimado, en un solo tramo. `observe::uso` suma las reservas abiertas. El evento final de la llamada cierra su reserva con el costo real. Así dos procesos no gastan el mismo margen.
  - Cada reintento repite el chequeo de tope. Una falla después de enviar el pedido (timeout, 5xx o un 200 que no parsea) registra el costo estimado de peor caso, con la marca `costo_estimado`.
  - Intento cortado por timeout (decisión 45):
    - Root separa dos fallas de red. Una falla antes de enviar el pedido, como una conexión rechazada, no cuesta nada y se puede reintentar. Una falla después de enviarlo, como un timeout de lectura, puede costar.
    - Un timeout de lectura no se reintenta. Repetir el pedido repite el mismo razonamiento largo y multiplica el costo, como pasó con el juez de memora: 3 intentos en 10 minutos.
    - El intento cortado registra un evento `Llamada` con `extra.error = "timeout"`, `extra.costo_estimado = true` y el costo de peor caso. El peor caso es la estimación de entrada de esta tarea más `max_tokens` de salida, al precio del tramo que corresponde (decisión 44).
    - Ese costo entra al gasto del mes y cierra la reserva de la llamada. Así el tope de API ve el gasto posible.
  - Costo manual (decisión 46): `r00t observa costo --usd <n> --motivo <texto> [--fecha <iso>]` registra un evento `Llamada` con `via = api`, `extra.manual = true` y el motivo.
    - El monto debe ser finito y positivo: el comando solo suma gasto, nunca lo resta.
    - `--fecha` ubica el gasto en el mes en que ocurrió. Sin `--fecha`, vale la hora actual.
    - Después de que la Tarea 21 entre a la rama de la laptop, Opus corre `r00t observa costo --usd 0.50 --motivo "3 intentos cortados por timeout del juez de la ficha de memora (decisión 46)" --fecha 2026-10-29`. Así el gasto cae en octubre, que es el mes de las llamadas.
  - La estimación de entrada usa los bytes UTF-8 del prompt: un token nunca ocupa menos de un byte.
  - El razonamiento en los proveedores (B10) pasa a la Tarea 21b.
  - Medios y bajos de topes: M2 (`elegir` usa `tokens_estimados` para el cupo), M3 (la sesión importada usa su propia hora), B8 (el lock cubre la lista de archivos rotados), B9 (sin presupuesto, código 3 y `Gate`), B12 (sin `costUSD`, usa `total_cost_usd`; un costo negativo es error) y M1 (`observa claude --via nube` registra el `Gate` al cruzar el tope).
- **Dónde**: todo en la nube. Los tests usan el servidor HTTP falso y procesos en paralelo.
- **Validate**: `cargo test`:
  - `ficha revisar` con juez y el tope de API lleno no envía nada.
  - `usd_salda = 1.1` en `root.toml` falla al cargar con código 2. `usd_entrada = nan` también.
  - Cuatro procesos en paralelo con 99 USD gastados de 100 no pasan el tope.
  - Un servidor que tarda más que el timeout deja un evento con costo estimado distinto de 0.
  - Ese servidor recibe un solo pedido: el timeout de lectura no se reintenta.
  - Un servidor que rechaza la conexión sí se reintenta, y sus intentos valen 0 USD.
  - `r00t observa costo --usd 0.50 --motivo x --fecha 2026-10-29` suma 0,50 USD al gasto de octubre de 2026. Con `--usd -1`, el comando sale con código 2.
  - Un prompt de 1000 caracteres de 3 bytes estima al menos 3000 tokens de entrada.

### Tarea 10c: Tope de salida de los jueces y causa "tope alcanzado" (decisión 42; local, ya)
- **Problema**: otra sesión corrió el juez de la ficha con `deepseek-v4-pro`. Las dos llamadas gastaron justo 4000 tokens de salida, devolvieron contenido vacío y salieron con `sin_veredicto` y código 3. El tope fijo de `ficha/juez.rs` (4000) se gastó en el razonamiento. El juez del Oracle tiene el mismo defecto con 6000 en `oracle/juez.rs`. `deepseek.rs` no lee la causa de fin, así que el error dice "JSON inválido" en lugar de "tope alcanzado".
- **Valor por defecto: 32 000 tokens.** Ese era el default de salida que DeepSeek documentaba para `deepseek-reasoner`, con un máximo de 64 000. Deja lugar al razonamiento y a la respuesta del juez. Sonnet confirma el valor vigente en la documentación de DeepSeek y lo anota en el commit. Si el máximo vigente es menor, usa ese máximo.
  - Peor caso por llamada con 32 000 tokens: 0,009 USD con `deepseek-flash` y 0,11 USD con `deepseek-v4-pro`. El chequeo previo de Route cuenta ese peor caso contra el tope de 100 USD por mes.
- **Action**:
  - Una constante compartida, `MAX_TOKENS_JUEZ = 32_000`, reemplaza los dos topes fijos. La usan `ficha/juez.rs` y `oracle/juez.rs`.
  - `r00t ficha revisar` suma `--juez-max-tokens <n>` para cambiar el tope en una corrida. El ajuste en `root.toml` llega con la Tarea 21b, que es dueña de `config.rs`.
  - `Respuesta` y `Crudo` suman `truncada: bool`. Solo `Respuesta` lleva `#[serde(default)]`, porque `Crudo` es interno y no se serializa. DeepSeek la marca con `finish_reason = "length"`. Anthropic la marca con `stop_reason = "max_tokens"`.
  - Una respuesta truncada sigue el camino de éxito: Root registra su costo real. No es un error del proveedor, así que no vale 0 USD.
  - Los dos jueces miran `truncada` antes de parsear. Si es verdadera, el juicio queda `SinVeredicto` con la causa "tope de salida alcanzado (N tokens)". Esa causa reemplaza a "JSON inválido".
  - `docs/interfaces.md` se regenera por el campo nuevo.
- **Archivos**: `src/proveedores/mod.rs` (solo los dos structs), `src/proveedores/deepseek.rs`, `src/proveedores/anthropic.rs` (solo la lectura de `stop_reason`), `src/ficha/juez.rs`, `src/oracle/juez.rs`, `src/cli.rs` (solo el flag) y `docs/interfaces.md`.
- **Cómo evita choques con la rama de la nube**:
  - Las Tareas 20, 15d y 6e de la nube tocan la guarda de tests, Aprende, los tipos de `Fila` y la descarga de Python. Ninguna toca estos archivos.
  - La 10c no toca `config.rs`, el chequeo de tope ni el cuerpo de los pedidos. Esas partes son de las Tareas 21 y 21b.
  - Las Tareas 21 y 21b arrancan desde una base que ya incluye la 10c. El orquestador hace push de la 10c, y la rama de la nube se rebasa antes de empezar la 21.
  - En la 21b, la constante `MAX_TOKENS_JUEZ` pasa a ser el default de un campo de `root.toml`. El pensamiento de Anthropic cuenta dentro de ese tope.
- **Dónde**: en la laptop, con el builder local. No necesita podman ni Colab.
- **Validate**: `cargo test proveedores` y `cargo test juez`:
  - Una respuesta falsa de DeepSeek con `finish_reason = "length"` y contenido vacío da `truncada = true` y registra su costo real.
  - Con esa respuesta, el juez de la ficha da `SinVeredicto` con la causa "tope de salida alcanzado". El mensaje no dice "JSON inválido".
  - Una respuesta con `finish_reason = "stop"` y JSON roto sigue dando "JSON inválido".
  - Una respuesta falsa de Anthropic con `stop_reason = "max_tokens"` da `truncada = true`.
  - Los dos jueces piden `max_tokens = 32000` por defecto, y `--juez-max-tokens 8000` cambia el pedido de la ficha.
- **Después**: Opus corre otra vez el juez de ambigüedades de la ficha (G1).

### Tarea 21b: Razonamiento explícito y modelos vigentes (B10 y decisión 41; gravedad alta)
- **Problema**: el invariante "el verificador razona" no se cumple en ningún proveedor.
  - `anthropic.rs` nunca envía el razonamiento (B10).
  - `deepseek.rs` no envía `thinking`. El default de Verifica y Juzga es `deepseek-reasoner`, un alias que `/models` ya no lista. mmorch lo resuelve hoy como `deepseek-flash` con `"thinking": {"type": "enabled"}`.
  - Root no trae precios por defecto, así que `r00t llama` rechaza todo modelo hasta que el humano escriba `[precios]`.
- **Action**:
  - DeepSeek envía siempre `thinking` en forma explícita: `{"type": "enabled"}` si `Eleccion.razonamiento` es verdadero, y `{"type": "disabled"}` si es falso.
  - Los defaults de `[ruteo]` pasan a modelos vigentes: Delega usa `deepseek-flash` sin razonamiento. Verifica y Juzga usan `deepseek-flash` con razonamiento. `deepseek-v4-pro` queda disponible en `[precios]` para quien lo configure.
  - Un `root.toml` viejo con `deepseek-chat` o `deepseek-reasoner` se sigue leyendo. Root lo traduce a `deepseek-flash` con el razonamiento que pide el rol, y avisa en stderr.
  - Anthropic recibe el razonamiento con `thinking: {"type": "adaptive"}` (decisión 44). Root nunca envía `budget_tokens`: en Haiku 5.5, ese formato devuelve un error 400. Root no envía `output_config`, así que rige el esfuerzo por defecto, `medium`.
    - El pensamiento cuenta dentro de `max_tokens`. El chequeo previo de la Tarea 21 ya cuenta `max_tokens` entero como peor caso, así que no hace falta otra estimación.
    - Con `razonamiento = false`, Root omite el parámetro. Un modelo que razona por defecto razona igual: eso cuesta más, pero no rompe ningún invariante.
    - Antes de escribir el código, Sonnet confirma con la skill `claude-api` que Sonnet 5.5 y Opus 5.5 aceptan `adaptive`. Si un modelo lo rechaza, el error 400 sale como `ErrorProveedor::Pedido` con código 3, y Sonnet lo anota en el commit.
  - Respaldo (G1d): `[ruteo.verifica]` y `[ruteo.juzga]` suman `respaldo`, con Haiku por defecto. Route usa el respaldo solo si el proveedor principal no está disponible: sin clave, o con el breaker abierto. El respaldo también lleva razonamiento. Si un modelo no admite razonamiento, Verifica y Juzga dan `ErrorOraculo::Eleccion`.
  - Precios por defecto (G1f y decisión 44): `deepseek-flash` 0,14 y 0,28 USD, `deepseek-v4-pro` 1,74 y 3,48 USD, `claude-haiku-5-5` 0,10 y 0,50 USD, y `claude-sonnet-5-5` 2 y 10 USD por millón de tokens (decisión 47). Cada valor cita su origen en un comentario: `mmorch/config.py` para DeepSeek, y la referencia de la API de Anthropic del 2026-10-06 para Haiku y Sonnet. Sonnet no lleva tramos: la tabla no los muestra. Un `[precios]` de `root.toml` reemplaza el default.
  - Precio por tramos: `Precio` suma, solo agregando, `umbral_entrada`, `usd_entrada_alto` y `usd_salida_alto`. Los tres son opcionales, pero van juntos: uno sin los otros falla al cargar con código 2. Haiku los lleva con 100 000, 0,50 y 2,50.
    - Costo real: si los tokens de entrada del pedido pasan `umbral_entrada`, el pedido entero usa el precio alto, entrada y salida. Si no, usa el precio normal.
    - Chequeo previo: la estimación de entrada de la Tarea 21 es una cota superior de los tokens reales. Si esa estimación no pasa el umbral, la entrada real tampoco, y el precio normal es seguro. Si la pasa, el chequeo usa el precio alto.
  - `docs/root.toml.example` muestra esos defaults en lugar de valores vacíos.
  - Timeout de los jueces (decisión 45): `[proveedores]` suma `timeout_juez_s`, con 1200 segundos por defecto. Lo usan las llamadas de los roles Verifica y Juzga, incluidos los dos jueces. Las demás llamadas siguen con `timeout_s`, que vale 120.
    - Razón del default: un juez puede escribir hasta 32 000 tokens (decisión 42). A unos 30 tokens por segundo, eso tarda cerca de 1070 segundos. 1200 deja un margen chico.
    - La velocidad de 30 tokens por segundo era una suposición. La corrida de la parte a de G1h la midió con `deepseek-v4-pro`, en un solo intento cada una:
      - memora: 97 segundos, 5915 tokens de entrada y 9619 de salida, 0,0438 USD. Son cerca de 99 tokens por segundo.
      - brasa: 154 segundos, 5906 tokens de entrada y 13 010 de salida, 0,0556 USD. Son cerca de 84 tokens por segundo.
    - Ninguna pasó de 600 segundos, así que el default sigue en 1200. A 84 tokens por segundo, los 32 000 tokens del tope tardan unos 380 segundos. 1200 deja más de tres veces ese margen para un proveedor cargado.
    - Un timeout más largo solo cuesta tiempo de espera ante una conexión muerta. La Tarea 21c corta ese caso con su plazo de inactividad de 120 segundos.
    - Después de la 21b, Opus devuelve `[proveedores].timeout_s` a 120 en `root.toml`: el juez ya tiene su propio timeout.
- **Dónde**: todo en la nube, con el servidor HTTP falso.
- **Validate**: `cargo test proveedores` y `cargo test config`:
  - Un pedido de Juzga a DeepSeek lleva `"thinking": {"type": "enabled"}` en el cuerpo.
  - Un pedido de Delega a DeepSeek lleva `"thinking": {"type": "disabled"}`.
  - Un pedido a Anthropic con `razonamiento = true` lleva `"thinking": {"type": "adaptive"}` y no lleva `budget_tokens`.
  - Un pedido a Anthropic con `razonamiento = false` no lleva `thinking`.
  - Un pedido a Haiku con 150 000 tokens de entrada cuesta con 0,50 y 2,50 USD. Uno con 1000 tokens cuesta con 0,10 y 0,50.
  - Un `[precios]` con `umbral_entrada` y sin `usd_salida_alto` falla al cargar con código 2.
  - Sin `root.toml`, `claude-sonnet-5-5` tiene precio de 2 y 10 USD, sin tramos, y el juez de config puede llamarlo.
  - Sin `root.toml`, Verifica y Juzga eligen `deepseek-flash` con razonamiento, y `r00t llama` tiene precio para ese modelo.
  - Un `root.toml` con `deepseek-reasoner` se lee como `deepseek-flash` con razonamiento.
  - Sin `DEEPSEEK_API_KEY`, Juzga elige el respaldo Haiku con razonamiento.
  - Una llamada de Juzga usa `timeout_juez_s`, y una de Delega usa `timeout_s`.
- **En la laptop**: un pedido real a `/models` de DeepSeek confirma que `deepseek-flash` y `deepseek-v4-pro` siguen listados. Ese GET no genera costo.

### Tarea 21c: Respuestas en streaming (decisión 45; gravedad media)
- **Problema**: una respuesta larga sin streaming deja la conexión callada hasta el final. Un timeout total no distingue un modelo que piensa de una conexión muerta. Las SDK de Anthropic además rechazan pedidos sin streaming que pueden pasar de 10 minutos.
- **Decisión técnica**: sí conviene el streaming, para los dos proveedores. Va en una tarea propia porque cambia el cuerpo de los pedidos y la lectura de las respuestas, que son de la 21b.
- **Action**:
  - DeepSeek pide `"stream": true` con `stream_options: {"include_usage": true}`. Anthropic pide `"stream": true`. Root lee los eventos SSE y arma el texto.
  - Dos plazos: un plazo de inactividad de 120 segundos sin bytes, y un plazo total igual al timeout de la llamada (`timeout_s` o `timeout_juez_s`). El de inactividad corta una conexión muerta. El total corta un modelo que nunca termina.
  - Los tokens y el costo salen del evento final de uso. Sin ese evento, la llamada cuenta como cortada y registra el peor caso de la Tarea 21.
  - La causa de fin del stream (`length` o `max_tokens`) sigue marcando `truncada` (Tarea 10c).
- **Dónde**: todo en la nube, con un servidor SSE falso.
- **Validate**: `cargo test proveedores`:
  - Un stream que manda un byte cada 60 segundos durante 15 minutos no se corta por inactividad. El test usa plazos inyectados y dura menos de 5 segundos.
  - Un stream que se calla 120 segundos se corta y registra el peor caso.
  - Un stream completo registra los tokens del evento final de uso.
  - Un stream que termina con `finish_reason = "length"` da `truncada = true`.

### Tarea 22: Descarga y contenedores sin fugas (C1 y medios o bajos de seguridad; gravedad crítica)
- **Action**:
  - C1: el contenedor de descarga de Python no monta `/repo`. Recibe solo una copia de los `requirements*.txt` en su carpeta de trabajo. El inventario rechaza toda línea que sea una ruta local: absoluta, `./`, `../`, `file:` o un directorio del repo.
  - M5: sin `Cargo.lock`, Root revisa las fuentes del lock generado con las mismas reglas antes de bajar nada más. El inventario sigue las dependencias `path` del repo, no solo los miembros del workspace.
  - M4: todo `podman run` y `podman build` lleva `--http-proxy=false`.
  - B5: todo `podman pull` lleva `--authfile`. Sin credencial de la lista de `root.toml` (decisión 29), ese archivo está vacío. Root nunca usa el `auth.json` del usuario.
  - B4: un error de parseo de un archivo del repo no cita la línea. Cita solo el archivo y el número de línea.
  - M16: Root lee la salida del contenedor sin seguir enlaces simbólicos. La carpeta de salida vive en `$ROOT_HOME/tmp/` con permisos 0700.
  - B6: `ROOT_HOME` debe ser una ruta absoluta, distinta de `/` y de `$HOME`. Si no lo es, Root sale con código 2.
  - B7: `pendientes` valida el nombre de la tarea con `nombre_seguro`.
  - B17: los temporales de Root se borran por `src/borrado.rs`.
- **Dónde**: las reglas y los argumentos se prueban en la nube. En la laptop se repite la prueba de C1 con podman: un backend PEP 517 que escribe una marca no la escribe.
- **Validate**: `cargo test`:
  - Una línea `/repo` o `./` en `requirements.txt` da `SinVeredicto` antes de usar la red.
  - Los argumentos de cada contenedor llevan `--http-proxy=false`, y cada `podman pull` lleva `--authfile`.
  - Un `.cargo/config.toml` roto con `token = "x"` no copia el token en el `Gate`.
  - Un enlace en la salida que apunta a un archivo del host no se lee.
  - `ROOT_HOME=/` sale con código 2.

### Tarea 23: El oráculo oculto no sale de `oraculos/` (A1, A2, A4, A5, A10, M15, B16, B18; gravedad alta)
- **Action**:
  - A1 y B18: el paquete de pendientes se arma en `$ROOT_HOME/oraculos/<tarea>/pendientes/`, dentro de la ruta que niegan los permisos. Root lo borra al terminar la corrida de Colab. Las carpetas `tmp/colab-*` se borran al terminar y en `limpiar_temporales`.
  - A2: la suite de contrato no monta `/oraculo`. El error `Infra` de la suite oculta no lleva el stderr del contenedor: el stderr va a un archivo de `$ROOT_HOME/oraculos/<tarea>/registros/`, y el mensaje nombra ese archivo. `root_evaluar` por MCP devuelve el veredicto sin ids de tests ocultos: solo conteos y niveles por requisito. La CLI para el humano los conserva.
  - A4: la clave del junit usa el `classname` entero más el nombre. Root la convierte al node id de pytest sin perder el archivo.
  - A5: Root corre primero `pytest --collect-only` y guarda la lista de tests. El junit vale solo si el código de salida es 0 o 1, si coincide con sus fallas y si trae exactamente los tests recolectados. Si no, la suite da `SinVeredicto`.
  - A10: un test omitido cuenta en el universo de su requisito como "no verde". Un requisito con un test omitido no pasa del nivel 2, y si el test está en `[[entorno]]`, queda `Pendiente`.
  - M15: el escenario monta solo la carpeta `escenarios/` de la tarea.
  - B16: `colab.json` guarda el commit del repo, y `--importar` ignora un resultado de otro commit.
  - A8 (decisión 40): el paquete de Colab lleva las dependencias ya bajadas por la fase de descarga. `correr.sh` compila y corre los tests dentro de `unshare -rn`, sin red. Si Colab no permite `unshare`, la corrida sigue con red, registra un `Gate` y `Informe.confianza` dice "corrida de Colab con red: el oráculo pudo filtrarse".
- **Dónde**: las reglas de junit, requisitos y Colab se prueban en la nube. Los montajes y los códigos de salida de pytest se prueban en la laptop, con podman.
- **Validate**: `cargo test`:
  - Después de `pendientes --colab` no queda ningún archivo de tests ocultos fuera de `oraculos/`.
  - Un producto que escribe `/oraculo/oculto/*` en stderr y mata a pytest da `SinVeredicto`, y el mensaje no trae ese texto.
  - `test_set.TestBasico::test_vacio` y `test_get.TestBasico::test_vacio` cuentan como dos tests.
  - Un junit con 3 tests cuando pytest recolectó 40 da `SinVeredicto`.
  - Un requisito con un test que pasa y otro omitido no queda Cumplido.

### Tarea 24: Sondas sin puntaje regalado (A9, A11, M13, M14, B13, B14; gravedad alta, latente)
- **Action**:
  - A9: las sondas de Rust recorren todos los `*.rs` del repo, salvo `target/`. Un error de lectura o cero archivos da `SinVeredicto`.
  - A11: un `ADD` o `COPY` con una URL cuenta como un paso que necesita red. Sigue la regla de la decisión 27, y su fuente entra al inventario.
  - M13: un reporte de `cargo-mutants` vale solo si la suma de sus resultados es igual a `total_mutants`. Si no, da `SinVeredicto` con la causa "reporte truncado". Root pasa `mutantes_max` a `cargo-mutants`.
  - M14: clippy con una construcción fallida da `SinVeredicto`, aunque haya advertencias.
  - B13: `combinar` sin partes da `None`.
  - B14: un test con `skipped` y `error` cuenta como falla.
- **Dónde**: todo en la nube, salvo la prueba real de A11 con buildah, que va en la laptop.
- **Validate**: `cargo test`:
  - Un workspace con `crates/a/src/lib.rs` que tiene `unsafe` da un hallazgo de seguridad.
  - Un repo sin archivos `.rs` da `SinVeredicto` en seguridad, estructura y arquitectura.
  - Un reporte de mutantes con 2 de 50 da `SinVeredicto`.

### Tarea 25: Limpieza de Python y de la ficha (M6 a M12, B1 a B3, B15; gravedad media)
- **Action**: los defectos de la descarga de Python (M6 a M12, B1 a B3) y la salida de `ficha revisar` sin juez (B15). Sin juez, `ficha revisar` sale con 3 y dice "juez no corrido", como pide el desempate 3 de "Desempates técnicos 3". `docs/interfaces.md` se alinea con esa regla.
- **Conflicto**: la Tarea 6e de la nube también toca la descarga de Python. La Tarea 25 empieza después de que la 6e entre a la rama.
- **Dónde**: la lógica en la nube. La instalación real sin red en la laptop.
- **Validate**: un test por defecto, con el escenario del informe.

### Tarea 26: Lectura del oráculo: aislamiento y detección (A3, decisión 39)
- **Capa 1, aislamiento** (decisión 43): el builder de `r00t corre` corre en un contenedor de podman.
  - La imagen trae `claude` y las herramientas del builder. El contenedor monta el directorio de la corrida y la credencial de Claude en solo lectura. No monta `ROOT_HOME` ni el resto de `$HOME`.
  - La credencial nunca entra a un log, a un evento ni a la imagen.
  - Riesgo a verificar en la laptop: `claude` puede necesitar escribir la credencial para renovar el login. Si el montaje de solo lectura rompe esa renovación, el planner pregunta al usuario antes de cambiar el montaje.
- **Capa 2, detección**: `r00t corre` lanza cada sesión con `--output-format stream-json` y guarda el transcript en `ROOT_HOME`, fuera del alcance del builder.
  - Root revisa cada llamada a herramienta: su entrada y su salida.
  - Busca la ruta real de `oraculos/` y de `ROOT_HOME`, también después de resolver rutas relativas y enlaces cuando el comando las nombra.
  - Busca los nombres de los tests ocultos y una huella de su contenido: líneas largas y únicas de cada archivo oculto.
  - Un hallazgo emite un evento `OraculoLeido` con la cita, y la corrida queda inválida.
- **Límite**: la detección es un registro, no una defensa. La defensa es el aislamiento.
- **Dónde**: el lector de `stream-json` y la búsqueda se construyen en la nube con transcripts de fixture. El aislamiento y la integración con `r00t corre` van en la laptop, con la Tarea 16.
- **Validate**:
  - Un transcript de fixture con `cat ../../.local/share/root/oraculos/x/oculto/t.py` emite `OraculoLeido`.
  - Un transcript que muestra una línea larga de un test oculto, sin nombrar la ruta, también lo emite.
  - Un transcript limpio no emite nada.
  - Con el aislamiento activo, el builder no puede abrir `ROOT_HOME`.

**Ya, en local (2026-10-29):** la Tarea 10c corre en el builder local, antes de todo lo demás. G1 la espera.

**Avance (2026-10-29):** el builder cerró la Tarea 10c en el commit 2bf0593, con 1048 tests en verde. Opus puede correr otra vez el juez de la ficha (G1).

**Desempates técnicos 9 (Opus, 2026-10-29, después de la Tarea 10c)**
1. **`extra.truncada` solo cuando es `true`.** Se queda así. Sigue el mismo patrón que `precio_faltante`: un campo ausente vale falso. Un `false` explícito en cada evento agrega ruido y no da información. Los lectores tratan la ausencia como falso, y `docs/interfaces.md` lo dice.
2. **`content` nulo con `finish_reason = "length"`.** Se acepta como texto vacío, porque `truncada` ya lleva la causa. Con `finish_reason = "stop"` y `content` nulo, el caso sigue siendo un error de respuesta. Si falta un test de ese segundo caso, la Tarea 21b lo agrega.
3. **`Crudo` sin `serde`.** Se acepta. `Crudo` es interno y nunca se serializa, así que no necesita `#[serde(default)]`. `Respuesta`, que sí sale en JSON, ya lo lleva. Corrijo el texto de la Tarea 10c, que pedía el atributo en los dos structs.

**Orden (2026-10-29, con la decisión 45):** 21, 21b, 21c, 22, 23, 24, 25 y la parte de nube de 26. La 21b va justo después de la 21: hoy ningún juez razona, y Verifica y Juzga apuntan a un alias viejo. La 21b necesita la validación de precios de la 21.

**Orden anterior (2026-10-28):** 21, 22, 23, 24 y 25. La 21 va primero porque hoy un juez puede gastar API sin tope. La 22 cierra la ejecución de código con red. Todas empiezan después de que entren las Tareas 20, 15d y 6e de la rama `claude/loving-ptolemy-9bj686`. Las tareas no dependen de números de línea: nombran funciones y archivos.

**Preguntas abiertas (sin responder)**

- **P50. Detección de lectura del oráculo (A3).** El GOAL dice: "una corrida que lo lee queda inválida". Hoy nada lo detecta. `claude -p` con `--output-format json` no trae las llamadas a herramientas, así que no hay transcript para revisar. Una búsqueda por texto se esquiva con un glob, una ruta relativa o un enlace. ¿Qué elegís?

  - a) Detección: `r00t corre` usa `--output-format stream-json`, busca en cada llamada a herramienta las rutas de `oraculos/` y los nombres de tests ocultos, y marca la corrida inválida. Costo: bajo. Límite: un builder que esquiva el texto no queda detectado.
  - b) Aislamiento: el builder corre en un contenedor o con otro usuario del sistema que no puede leer `ROOT_HOME`. Costo: alto, porque `claude` y su login tienen que correr dentro de ese entorno. Ventaja: leer el oráculo se vuelve imposible.
  - c) Las dos: aislamiento como defensa y detección como registro.

  Mi recomendación es c). Si el costo de b) no entra en el hito 1, a) como paso previo.

- **P51. Colab con internet y tests ocultos (A8).** En Colab, el paquete lleva el código del builder y los tests ocultos juntos, y el runtime tiene internet. Un `build.rs` del builder puede mandar los tests ocultos a cualquier host. En la laptop, `--network none` lo impide. ¿Qué elegís?

  - a) Aceptarlo como está. Riesgo: el oráculo de esa tarea puede filtrarse.
  - b) Aislar la corrida: el paquete lleva las dependencias ya bajadas, y `correr.sh` corre dentro de `unshare -rn`, sin red. Si Colab no permite `unshare`, la corrida no se hace y los tests quedan `Pendiente`.
  - c) Como b), pero si Colab no permite `unshare`, la corrida se hace igual y el informe lo avisa.

  Mi recomendación es b).

**Respuestas del 2026-10-29:** P50 "P50 c" (decisión 39), P51 "P51 c" (decisión 40), y G1d, G1e y G1f (decisión 41).

- **P52. Precio de Haiku.** Tomaste los precios de `mmorch/config.py` (G1f). Ese archivo también lista `claude-haiku-5-5` a 0,10 USD de entrada y 0,50 USD de salida por millón de tokens. No verifiqué ese valor contra la lista de precios de Anthropic. Un precio bajo hace que el chequeo previo deje pasar llamadas que cuestan más. ¿Qué elegís?

  - a) Usar 0,10 y 0,50 USD, de `mmorch/config.py`.
  - b) Vos escribís el precio de Haiku en `root.toml` desde la página de Anthropic. Sin ese precio, el respaldo Haiku no se usa.
  - c) Un agente busca el precio vigente en la documentación de Anthropic, y vos lo confirmás antes de que entre al default.

  Hasta tu respuesta, rige b). Mi recomendación es c).

- **P53. Cómo se aísla el builder (decisión 39).** El builder no debe poder leer `ROOT_HOME`. Las dos formas tocan zona roja. ¿Qué elegís?

  - a) Contenedor de podman: el builder y `claude` corren adentro, sin montar `ROOT_HOME`. La credencial de Claude entra montada en solo lectura. Costo: armar una imagen con `claude` y sus herramientas.
  - b) Otro usuario del sistema: vos creás una vez un usuario con `sudo useradd`, y el builder corre con ese usuario. Ese usuario no puede leer tu `$HOME`. Costo: un cambio en el sistema operativo y un login de Claude para ese usuario.

  Mi recomendación es a): no cambia el sistema y deja el builder en un entorno descartable.

**Respuestas del 2026-10-29:** G1g "G1g a, pero no pueden estar el tope de tokens ya alto de por si?" (decisión 42) y P53 "P53 a" (decisión 43). P52 sigue abierta.

**Respuesta del 2026-10-29:** P52 "P52 c pero yo no lo confirmo." (decisión 44). P52 queda cerrada.

**Respuesta del 2026-10-29:** G1h "G1h: a ahora para correr el juez, y b como corrección en el código." (decisión 45). La parte a corre en la otra sesión de Opus.

- **P54. Gasto no registrado del juez de memora.** Los 3 intentos cortados registraron 0 USD, pero DeepSeek pudo cobrarlos. Con `deepseek-v4-pro` y 32 000 tokens de salida, el peor caso es cerca de 0,11 USD por intento, más la entrada. En total son menos de 0,50 USD. El tope de API no ve ese gasto. ¿Qué elegís?

  - a) Ignorarlo: el monto es chico frente al tope de 100 USD.
  - b) Vos mirás el uso real en el panel de DeepSeek, y Opus lo registra con un evento de costo manual. La Tarea 21 suma ese evento: `r00t observa costo --usd <n> --motivo <texto>`, que solo suma gasto y nunca lo resta.
  - c) Registrar el peor caso estimado, 0,50 USD, con el mismo evento de b), sin mirar el panel.

  Mi recomendación es c): el tope ve el gasto posible y no te pide trabajo.

**Respuesta del 2026-10-29:** P54 "P54 c" (decisión 46). P54 queda cerrada. La corrida de la parte a de G1h terminó: memora en 97 segundos y brasa en 154, cada una en un intento.

## G1, G2 y la mezcla de la nube 1 (2026-10-29)

**Gates cumplidos**
- **G2** se cumplió con el commit 8c5d5f0: las nueve rúbricas aprobadas por el usuario viven en `rubricas/`.
- **G1** se cumplió el 2026-10-29. El usuario escribió: "Apruebo G1. Mezclá la build de la nube y que el builder resuelva la compilación." El juez de ambigüedades corrió con razonamiento sobre las fichas de memora y brasa. Opus corrigió 3 hallazgos, y un segundo juez dio 0 hallazgos.
- **Mezcla**: la build de la nube 1 (Tareas 20, 15d y las partes de nube de 6e, 6d, 9d3, 7k y 7l) entró en c5ff327, junto con la Tarea 10c. `cargo test` con podman da 1152 tests en verde. El informe es `docs/builds/2026-10-28-build-nube-1.md`.
- En la mezcla, el builder arregló dos puntos: `juez_config.rs` usa `RespuestaJuez` con la causa "tope de salida alcanzado", y en `entrega.rs` un fallo de configuración de red de podman deja `Pendiente` con aviso, no un rojo falso.

**Tareas desbloqueadas por G1 y G2**
- El `Validate` real de las Tareas 7 y 9c. El diagnóstico de `diferencias.md` se repite con 5 repeticiones, como pedía el desempate 1 de "Desempates técnicos 6".
- Las Tareas 12, 12b (código), 13 y 13b.
- La corrida de memora de la Tarea 16 (decisión 38). La corrida de Graft sigue esperando a G5.
- La corrida real de Colab de la Tarea 7h (decisiones 22, 33 y 40).
- G3 queda abierto: el usuario califica a ciegas los 6 paquetes cuando la Tarea 12b los arme.

## Desempates técnicos 10 (Opus, 2026-10-29, después de la mezcla de la nube 1)

**Preguntas del builder de la mezcla**
- **a) Red del build de respaldo.** Root cambia a `pasta`, que es el default de Podman 5. No hace falta instalar `slirp4netns`. El argumento pasa a `--network pasta:--no-map-gw`. Con `--no-map-gw`, el contenedor no alcanza la dirección del host, y eso cumple la regla de la decisión 27 (`allow_host_loopback=false`). Un test de laptop abre un puerto en `127.0.0.1` del host y prueba que el build no lo alcanza.
- **b) `tokens_estimados` del juez de config.** Se queda en 32 000. El chequeo de Route debe ver el peor caso. Si el margen del mes no alcanza para ese peor caso, rechazar al juez es lo correcto: el tope existe para eso. Con `deepseek-flash`, el peor caso es 0,009 USD.

**Preguntas del informe de la nube 1**
1. **Forma actual (15d).** Se confirma la regla del organizador, acotada a la tarea: la forma actual es la de la última fila válida de esa tarea, por `ts` y después por `id`. `root.toml` no define formas.
2. **Prueba de Route (15d).** No alcanza. Un test suma propuestas de forma y de modelo a `politica.json` y compara `elegir` con y sin ellas en los seis roles. Planifica, Organiza y Construye no deben cambiar.
3. **Marcador `not found` (7l).** Se saca hasta verificar la CLI real. Una salida desconocida conserva `activo.json`, como pide el desempate 4 de "Desempates técnicos 8".
4. **`tar` en el host (6e).** La lectura pasa adentro del contenedor de descarga. El sdist viene de la red, y Root no abre en el host un archivo ajeno. El contenedor lee `pyproject.toml` con `tarfile` de Python, sin extraer a disco, y devuelve `[build-system].requires`.
5. **Modelo del juez de config (6d).** Manda la decisión 30: el juez es Sonnet. `[jueces]` suma `config_modelo`, con `claude-sonnet-5-5` por defecto, y la llamada pasa por la función con tope de la Tarea 21. Sonnet no tiene precio por defecto: P55 lo decide. Sin precio, el juez da `SinVeredicto` y la descarga sigue (decisión 35).
6. **Host y puerto (6d).** Se confirma. La coincidencia exacta de host y puerto es más estricta que la del plan, y evita mandar la credencial a otro servicio del mismo host.
7. **Usuario de pip (6d).** `[[registro]]` suma `usuario_env`, opcional. Sin ese campo, el usuario es `__token__`, la convención de los índices compatibles con PyPI para tokens.
8. **Conexión del juez de config (6d).** La conecta la Tarea 13, que arma el `Veredicto` completo.
9. **Opción de red (9d3).** Resuelta en a): `pasta:--no-map-gw`.
10. **Gate previo (9d3).** Se acepta, con dos campos separados: `primera_run_sin_red`, con la línea que falló, y `runs_del_dockerfile`, con todas. El Gate no afirma que todas necesitan red.
11. **Binario en Colab (7k).** El `conftest.py` oculto de brasa corre `cargo build -q --bin brasa` y usa `target/debug/brasa`. Ese archivo dice "No modificar". En el camino `binario-local`, Root compila brasa en la imagen con el perfil `debug` y deja el binario en `target/debug/brasa` del paquete. `correr.sh` pone en el `PATH` un `cargo` mínimo: acepta solo `build -q --bin brasa`, sale con 0 si el binario existe, y falla con un mensaje claro ante cualquier otra orden.
12. **Causa del tope (7l).** La causa pasa a "tope de duración", y `motivo` lleva los minutos reales. "tope de 2 horas" solo vale con el default.
13. **Repeticiones en Colab (7l).** `correr.sh` repite la suite pendiente `repeticiones` veces y junta los junit. El criterio de corte de la Tarea 7l no cambia.
14. **Causas de los demás cortes (7l).** Sí. La lista cerrada suma `gpu-no-t4`, `runtime-no-asignado` y `error-en-corrida`.

### Tarea 27: Ajustes de la mezcla de la nube 1
- **Action**: los desempates a) y 1 a 14 de arriba.
- **Dónde**: en la nube, salvo el test de `pasta` contra el host, que va en la laptop.
- **Validate**: `cargo test`:
  - Los argumentos del build de respaldo llevan `--network pasta:--no-map-gw`.
  - Con propuestas en `politica.json`, `elegir` da lo mismo en Planifica, Organiza y Construye.
  - Una salida de `colab stop` con "not found" conserva `activo.json`.
  - La lectura de `pyproject.toml` de un sdist corre en el contenedor de descarga: en la nube se prueba el comando que arma Root.
  - Sin `usuario_env`, el `netrc` de pip usa `__token__`.
  - Con `minutos_max = 30`, el Gate del tope dice "tope de duración" y su motivo dice 30.
  - El `cargo` mínimo de `correr.sh` sale con 0 ante `build -q --bin brasa` con el binario presente, y falla ante `build --release`.

## Orden de trabajo (2026-10-29): la nube primero

El crédito de nube vence el 2026-11-05. La nube toma toda la lógica que se prueba sin podman, sin Colab, sin claves y sin la credencial de Claude.

**Nube, sesión 2.** Arranca desde la punta de `claude/root-e7813e` que trae `docs/builds/brief-nube-2.md` (un commit después de c5ff327). El brief completo es ese archivo. Orden:
1. 21: topes y llamadas a la API, con `r00t observa costo`.
2. 21b: razonamiento explícito, modelos vigentes, precios por tramos, `timeout_juez_s` y `[jueces].config_modelo`.
3. 21c: streaming.
4. 27: ajustes de la mezcla de la nube 1.
5. 22: descarga y contenedores sin fugas (parte de nube).
6. 23: el oráculo oculto no sale de `oraculos/` (parte de nube).
7. 24: sondas sin puntaje regalado (parte de nube).
8. 13: Oracle completo, con ejecutor y proveedor falsos. Conecta el juez de config.
9. 13b: informe del PM, con un LLM falso.
10. 12: calibración, con ejecutor falso.
11. 12b: validez, solo el código y el armado de los paquetes a ciegas.
12. 26: lector de `stream-json` y búsqueda de lecturas del oráculo (parte de nube).
13. 25: limpieza de Python y de la ficha.

**Laptop.** Cada parte necesita al menos uno de estos recursos: podman, Colab, claves de API o la credencial de Claude.
- Podman: las partes de laptop de 6e, 6d, 9d3, 7k, 22, 23, 24 y 27, el timer del vigía de la 7l, y la suite completa sin `ROOT_SIN_PODMAN` antes de cada mezcla.
- Podman y `ROOT_HOME` real: el `Validate` real de 7 y 9c, con el diagnóstico de `diferencias.md` en 5 repeticiones; la calibración real (12); el veredicto real (13) y el informe real (13b) de memora y brasa.
- Claves de API: el GET a `/models` de DeepSeek (21b), y el evento de 0,50 USD de la decisión 46 después de la 21.
- Colab: la corrida real de la 7h sobre `test_rendimiento[gpu-10000-res1]` de brasa, con T4, tope de 2 horas y `unshare -rn`.
- Credencial de Claude y podman: el contenedor del builder (Tarea 26, decisión 43) y la Tarea 16 con memora.
- Opus: el oráculo de `graft-lector-planes` (Tarea 16b) y G5.
- Usuario: G3, la calificación a ciegas de la 12b.

**Preguntas abiertas (sin responder)**

- **P55. Precio de Sonnet.** El juez de la config de cargo es Sonnet (decisión 30). Root no tiene un precio por defecto para `claude-sonnet-5-5`, así que `r00t llama` rechaza esa llamada y el juez da `SinVeredicto`. Para Haiku elegiste que un agente busque el precio y Root lo use sin tu confirmación (decisión 44). ¿Qué elegís para Sonnet?

  - a) La misma regla que Haiku: un agente busca el precio vigente en la referencia de la API de Anthropic y Root lo usa sin tu confirmación.
  - b) Vos escribís el precio de Sonnet en `root.toml`.
  - c) El juez de config usa Haiku con razonamiento en lugar de Sonnet. Este cambio contradice la decisión 30.

  Hasta tu respuesta, rige b). Mi recomendación es a).

**Respuesta del 2026-10-29:** P55 "P55 a" (decisión 47). P55 queda cerrada. El precio de Sonnet entra en la Tarea 21b, que es dueña de los precios por defecto.

### Mensaje para la nube 2

El orquestador manda este texto a la sesión de nube 2, que arrancó desde 574c785:

> El usuario respondió P55 con la opción a (decisión 47 del plan). Esto reemplaza la nota del brief que prohibía un precio para Sonnet. En la Tarea 21b, sumá `claude-sonnet-5-5` a los precios por defecto: 2 USD por millón de tokens de entrada y 10 USD por millón de salida. Sonnet no lleva tramos por tamaño de entrada. El comentario del precio cita su origen: la referencia de la API de Anthropic, tabla cacheada el 2026-10-06. Agregá un test: sin `root.toml`, `claude-sonnet-5-5` tiene ese precio y el juez de config puede llamarlo. Con eso, `[jueces].config_modelo` sigue en `claude-sonnet-5-5`, como pide la decisión 30. Anotá este cambio en el informe de la build.

## Mezcla de la nube 2 (2026-10-29)

**Mezcla.** La build de la nube 2 hizo las 13 tareas de su brief en la rama `claude/elegant-euler-eg8cm8` (69eea70). La laptop corrió la suite con podman y encontró 11 tests en rojo. El usuario eligió que el builder los arregle, y el arreglo es 3d145a7 en `laptop/nube2-podman`. El usuario ordenó: "Mezclá laptop/nube2-podman (nube 2 más el arreglo) en claude/root-e7813e." La mezcla es 1f98ad9.
- El arreglo de la laptop tocó tres puntos:
  - La carpeta `escenarios/` pasó a ser obligatoria solo si algún `cmd` usa `/oraculo`. Ese cambio arregló 10 tests de `tests/entrega.rs`.
  - `commit_de` quita las variables `GIT_*` heredadas del hook.
  - `descarga_dependencia_de_crates_io_compila_en_la_evaluacion_sin_red` quedó `#[ignore]` hasta resolver la pregunta 9.
- Con podman, la suite da 1550 verdes, 0 rojos y 2 ignorados. El informe de la build es `docs/builds/2026-10-29-build-nube-2.md`.

**Lo que la nube 2 dejó sin probar** (va a la laptop):
- 21b: el GET a `/models` de DeepSeek.
- 21c: un stream real. Falta confirmar que DeepSeek y Anthropic mandan bytes mientras razonan. Si no los mandan, el plazo de inactividad de 120 segundos corta al juez.
- 22: la prueba de C1 con un backend PEP 517 real.
- 23: el formato real de `pytest --collect-only -q` y su código 5, los montajes reales y `unshare -rn` en Colab.
- 24: la prueba real de A11 con buildah.
- 27: `pasta:--no-map-gw` contra un puerto del host, y la subida de `target/debug` y `bin` con la CLI de Colab.
- 12 y 12b: la calibración real y la validez real.
- 26: un transcript real de `claude -p`.

**Lo que la nube 2 dejó sin hacer** (va a la nube 3, salvo lo que diga otra cosa):
- 13: `rubrica::EMBEBIDAS` está vacío, así que el juez de diseño no corre en producción y `Veredicto.rubricas` queda vacío.
- 13b: `Confianza.ambiguedades` queda vacío, y el aviso "corrida de Colab con red" no llega a `Veredicto.avisos`.
- 25: M9.
- 27: `netrc_para` y `usuario_de_pip` existen, pero no están conectadas a la descarga.
- 24 (M13): falta confirmar en la laptop si `cargo mutants --help` ofrece un límite de mutantes.
- 26: la capa 1 y la integración con `r00t corre`.
- 16: el runner todavía no existe.

**Tareas desbloqueadas.** G1 y G2 están cumplidos y el código de la nube 2 entró. Quedan desbloqueadas:
- En la laptop: el `Validate` real de 7, 9c, 13 y 13b sobre memora y brasa.
- En la laptop: la calibración real (12), que espera también a P57.
- En la nube 3: el código de la Tarea 16 con dobles de prueba.
- En la laptop: la corrida real de la 16 con memora, que necesita la capa 1 de la 26.
- En la laptop: la corrida real de Colab de la 7h.

## Reglas de la laptop (2026-10-29)

Estas reglas valen para toda corrida de la suite o de Root en la laptop.

1. **Sin claves y con un `ROOT_HOME` de prueba.** Con `DEEPSEEK_API_KEY` o `ANTHROPIC_API_KEY` en el entorno, la suite con podman gasta API real. También escribe eventos en el `ROOT_HOME` real, porque `rutear` y `llamar` leen `root_home()`. La suite corre siempre sin esas dos claves y con `ROOT_HOME` apuntando a una carpeta de prueba vacía. Solo una corrida real de Root, pedida en el plan, usa las claves y el `ROOT_HOME` real.
2. **Compilaciones en /home.** El `/tmp` de la laptop es un tmpfs de 7,7 GB, y un `target` de cargo ya lo llenó. Toda compilación y todo temporal de la suite van en /home: `TMPDIR` y `CARGO_TARGET_DIR` apuntan a carpetas bajo `$HOME/.cache/root-tests/`.
3. **El hook las aplica.** La Tarea 29 hace que `.githooks/pre-commit` quite las dos claves y fije `ROOT_HOME`, `TMPDIR` y `CARGO_TARGET_DIR` antes de `cargo test`. Una persona que corre `cargo test` a mano usa el mismo script: `tools/test-seguro.sh`.

## Desempates técnicos 11 (Opus, 2026-10-29, después de la mezcla de la nube 2)

Las preguntas 8, 5 y 43 de la nube tocan lo que el evaluador promete o el gasto: van al usuario como P56 y P57. Las demás son técnicas. Cada desempate tiene su razón en una línea.

**Rúbricas, Oracle e informe**
1. **Rúbricas embebidas.** El builder agrega los `include_str!` de los nueve archivos de `rubricas/`. El contenido ya está aprobado (G2) y no cambia. Un test compara el hash de cada rúbrica embebida con el de su archivo.
2. **Pesos de `global`.** Se confirman pesos iguales entre las dimensiones calibradas con veredicto: el plan no dio pesos, y el peso igual es el que menos supone.
3. **Ítems del juez sin respuesta.** Se revierte: un ítem sin respuesta queda en `faltantes`, no vale 0. Así lo fija el desempate 3 de "Desempates técnicos 4": un 0 sin evidencia es un rojo falso.
4. **Estructura y Arquitectura.** Se confirma la media simple de sondas y juez, y una parte sin veredicto deja la dimensión sin veredicto: es la lectura más cerrada.
5. **Calibración por tarea.** Pasa al usuario como P57, junto con la pregunta 43, porque decide el gasto.
6. **Funciona.** Se confirma: pasan sobre medidos, sin `[[entorno]]`, con los escenarios promediados. Sigue la Tarea 9c.
7. **Entrega.** La construcción de la 9d2 entra al veredicto, como pide la Tarea 9d. Sin `entregar`, la dimensión mide solo la mitad de lo prometido.
8. **Listo con dimensiones sin veredicto.** Pasa al usuario como P56.
9. **Completitud sin veredicto.** `Decision` suma, solo agregando, la variante `SinDecision`. Completitud sin veredicto da `SinDecision`, y `SinDecision` da `Estado::SinVeredicto` con código 3. Razón: el GOAL pide "sin veredicto antes que un veredicto falso", y un NoListo sin datos de completitud es un rojo falso.
10. **Rol del redactor.** Se confirma Delega: redacta, no juzga, y el gate de evidencia filtra cada oración.
11. **`umbral_verde` y tests fuera de requisitos.** `umbral_verde` se quita del código, y un `root.toml` viejo con ese campo se sigue leyendo con un aviso. Un test oculto que falla y no pertenece a ningún requisito da NoListo con la regla `test_oculto_fuera_de_requisitos`. Razón: un test oculto es una promesa del producto aunque la tarea no lo mapee.

**Topes y proveedores**
12. **Reservas abiertas.** Siguen contando como gasto: el proceso pudo enviar el pedido antes de morir. `r00t observa uso` lista las reservas abiertas con más de dos veces `timeout_juez_s`. Solo un humano puede corregirlas, con `observa costo`, que solo suma.
13. **Log no escribible.** Se confirma: sin reserva escrita, no se envía nada. Es fallar cerrado, y el desempate 8 de "Desempates técnicos 5" ya lo pedía.
14. **Clasificación de fallas.** Se confirma la tabla del organizador: es conservadora y el tope ve cada peor caso.
15. **Cupo con `tokens_estimados`.** Se confirma: sin precio no hay estimación, como en `r00t llama`.
16. **Sesión importada.** Se confirma `mtime - duration_ms`: es la mejor hora disponible sin un campo de inicio.
17. **Costo sin `costUSD`.** Se confirma, con un cambio: `costo_desconocido` también deja un `Gate`, para que el tope no lo pase por alto.
18. **`observa costo`.** Se confirma: corrida `manual`, motivo obligatorio y las 12:00 locales, que caen en el día pedido con cualquier huso.
19. **Precio en `root.toml`.** Se confirma: un precio del usuario reemplaza al default entero, y el umbral es estricto. Heredar tramos mezclaría dos fuentes de precio.
20. **Juez de la ficha.** Sí: `--juez-modelo` pasa por la lista de modelos que razonan. Un modelo fuera de la lista sale con código 2. Razón: el invariante "el verificador razona" vale para todo juez.
21. **Cita del precio de DeepSeek.** Se confirma la cita "config.py del proyecto anterior (decisión 41)": cumple la regla de la Tarea 18.

**Descarga y contenedores**
22. **Nombre de carpeta como requisito.** Se quita esa regla. Desde la Tarea 22, el contenedor de descarga de Python no monta `/repo`, así que un nombre de carpeta no puede resolver al repo. La regla solo daba falsos positivos.
23. **Opciones de pip.** `--prefer-binary` y `--only-binary` se aceptan y se ignoran: Root ya fuerza `--only-binary=:all:`. `-c` con solo versiones se acepta. `--no-binary`, `--trusted-host`, `-c` con fuentes y `-e` que no es git se siguen rechazando, porque abren la construcción o la red.
24. **`generar_lock` de Rust.** Se confirma: `cargo generate-lockfile` no corre `build.rs`, y necesita los manifiestos de las dependencias `path`.
25. **Clones git.** Se quedan en el host, con un tope de 1 MB por archivo. Leer un `pyproject.toml` como texto con el parser de TOML no ejecuta nada. El `tar` sí fue al contenedor, porque descomprime un archivo ajeno.
26. **Python en la imagen.** Se confirma: la imagen trae Python 3.11, y sin `tomllib` la salida 3 es fallar cerrado.
27. **Clave de caché de Python.** Sí: la clave suma el digest de la imagen, leído con `podman image inspect`. Un tag reconstruido no debe usar wheels de otra imagen.
28. **Junit con ids repetidos.** `correr.sh` de Colab escribe un junit por repetición, y Root los importa como repeticiones. Un test que alterna queda inestable, igual que en la laptop.
29. **Validación de `ROOT_HOME`.** Se confirma en `ejecutar`: cubre todos los subcomandos con un solo punto.
30. **Gate del build con red.** Los campos pasan a llamarse `primer_paso_sin_red` y `pasos_del_dockerfile`: hoy traen `RUN`, `ADD` y `COPY` con URL.
31. **M9.** Root resuelve solo con wheels. Si un paquete necesita a otro que solo tiene sdist, la descarga da `Pendiente` con el mensaje "B solo tiene sdist y la descarga no construye sdists". Bajar sdists con pip correría `setup.py` con red, contra la decisión 21.
32. **Límite de cargo-mutants.** Se confirma el límite en el parser. La laptop lee `cargo mutants --help` y lo anota.
33. **Arquitectura en workspaces.** Da `SinVeredicto` si no encuentra módulos: un 1,0 sin análisis es el mismo defecto que A9.

**Oráculo oculto, Colab y transcripts**
34. **A5 solo en pytest.** Se confirma: los otros runners no tienen `--collect-only`. Para ellos, el junit vale solo con código de salida 0 o 1.
35. **Contrato montado.** Se acepta montar solo `contrato/` en `/oraculo/contrato`: el builder ya ve esos tests. La Tarea 23 se corrige en ese punto.
36. **Redacción MCP.** Se confirma la lista: todo dato que nombra un test oculto queda fuera de `root_evaluar`.
37. **Ids en `ROOT_HOME`.** Se acepta. Con la capa 1 de la Tarea 26, el builder no ve `ROOT_HOME` entero.
38. **Paquete de pendientes.** Un lock por tarea impide dos `pendientes` simultáneos. El segundo sale con código 3 y nombra al primero.
39. **Transcript y `ROOT_HOME`.** La búsqueda sigue cubriendo todo `ROOT_HOME`. A cambio, `r00t corre` pone la carpeta de la corrida fuera de `ROOT_HOME`, en `$HOME/.cache/root/corridas/`. Así no hay falsos positivos y la detección no se achica.

**Calibración y validez**
40. **Repo bueno.** Se confirma: `--repo` o `[calibracion] repo` de `tarea.toml`.
41. **Detección.** Se confirma: un caso sin veredicto cuenta en contra, porque el evaluador no lo detectó.
42. **Inyectores aproximados.** Se aceptan para el hito 1. `tarea.toml` suma `[calibracion] camino_caliente`, una lista opcional de funciones. Con esa lista, el bucle O(n²) va en una de ellas.
43. **Costo de calibrar.** Pasa al usuario como P57.
44. **Umbral de orden.** `cumple` exige exactitud de orden 1,0. El criterio de aceptación dice "Root ordena bien los pares de orden conocido": son pocos pares, y uno mal ordenado ya es un fallo.
45. **Rastros en los paquetes a ciegas.** El armado borra `authors`, `repository` y `homepage` de `Cargo.toml`. También lista, por paquete, cada aparición de "ecc", "sdlc", "claude", "haiku", "sonnet" y "opus". Opus revisa esa lista antes de entregar los paquetes al usuario (G3).

**Preguntas del builder de la laptop**
- **a) `escenarios/` montada siempre.** No: se monta solo si algún `cmd` usa `/oraculo`. Los guiones de escenario son material del oráculo, y el contenedor corre el producto del builder.
- **b) Firma de `entregar`.** Sí: `entregar_con` recibe un ejecutor, y `entregar` queda como envoltorio. Así se prueba sin podman, como la Tarea 13.
- **c) Test ignorado.** Vuelve sin `#[ignore]`, con la aserción `SinVeredicto`, por el desempate 9.

### Tarea 28: Desempates de la nube 2
- **Action**: los desempates 1, 3, 7, 9, 11, 12, 17, 20, 22, 23, 27, 28, 30, 31, 33, 38, 39, 42, 44 y 45, y los del builder a, b y c.
- **Dónde**: en la nube, con dobles de prueba. En la laptop van el digest de la imagen (27) y los montajes reales (a).
- **Validate**: `cargo test`:
  - El hash de cada rúbrica embebida coincide con el de su archivo de `rubricas/`, y `Veredicto.rubricas` trae las nueve.
  - Un ítem del juez sin respuesta queda en `faltantes`, y la dimensión no baja.
  - Completitud sin veredicto da `SinDecision`, `Estado::SinVeredicto` y código 3.
  - Un test oculto que falla fuera de los requisitos da NoListo con `test_oculto_fuera_de_requisitos`.
  - `ficha revisar --juez-modelo deepseek-chat` sale con código 2.
  - `-c` con solo versiones se acepta, y `--trusted-host` se rechaza con su línea.
  - Un workspace sin módulos bajo `src/` da `SinVeredicto` en Arquitectura.
  - Dos `pendientes` simultáneos de la misma tarea: el segundo sale con código 3.
  - Un paquete a ciegas no trae `authors`, y la lista de rastros nombra un README que dice "ECC".
  - `descarga_dependencia_de_crates_io_compila_en_la_evaluacion_sin_red` vuelve sin `#[ignore]` y espera `SinVeredicto`.

### Tarea 29: Suite segura en la laptop (Reglas de la laptop)
- **Action**:
  - `tools/test-seguro.sh` quita `DEEPSEEK_API_KEY` y `ANTHROPIC_API_KEY`, crea un `ROOT_HOME` de prueba vacío bajo `$HOME/.cache/root-tests/`, apunta ahí `TMPDIR` y `CARGO_TARGET_DIR`, y corre `cargo test`.
  - `.githooks/pre-commit` usa ese script.
  - `rutear` y `llamar` dentro de `evaluar` usan `OpcionesOraculo.home`, no `root_home()`. Así una evaluación con un `ROOT_HOME` de prueba nunca escribe en el real.
- **Dónde**: el script y el cambio de código en la nube. La prueba con las claves exportadas, en la laptop.
- **Validate**:
  - Con `DEEPSEEK_API_KEY=x` exportada, `tools/test-seguro.sh` corre la suite y ningún test ve la clave.
  - `evaluar` con `OpcionesOraculo.home` de prueba no escribe en `root_home()`.

### Tarea 30: Lo que la nube 2 dejó sin hacer
- **Action**:
  - 13b: `Confianza.ambiguedades` recibe las ambigüedades abiertas de la ficha, y el aviso "corrida de Colab con red" llega a `Veredicto.avisos`.
  - 27: la descarga de Python arma el `netrc` de pip con `netrc_para` y `usuario_de_pip`, solo para los registros de `root.toml` (decisión 29).
- **Dónde**: en la nube.
- **Validate**:
  - Una ficha con una ambigüedad abierta la muestra en `Confianza.ambiguedades`.
  - Un `colab.json` con la marca de red deja el aviso en `Veredicto.avisos`.
  - Un registro de `root.toml` con `usuario_env` arma un `netrc` con ese usuario. La credencial no aparece en ningún evento.

## Orden de trabajo (2026-10-29, después de la nube 2)

El crédito de nube vence el 2026-11-05. La nube va primero.

**Nube, sesión 3.** Arranca desde la punta de `claude/root-e7813e` que trae `docs/builds/brief-nube-3.md` (un commit después de 1f98ad9). Orden:
1. 29: suite segura. Va primero porque protege cada corrida posterior de la laptop.
2. 28: desempates de la nube 2.
3. 30: lo que la nube 2 dejó sin hacer.
4. 16: runner con dobles de prueba: un `claude` falso que emite `stream-json`, el ejecutor de contenedor falso y la carpeta de la corrida fuera de `ROOT_HOME`.
5. 26, capa 1, solo el `Containerfile` del builder y los argumentos del contenedor. La imagen se construye en la laptop.
6. 12: la parte técnica de P57 que no depende de la respuesta. Cada caso de calibración corre solo las partes de `evaluar` que mide su dimensión.

**Laptop.**
- Siempre: `tools/test-seguro.sh`, o el hook, sin claves y con compilaciones en /home.
- Podman: la suite completa antes de cada mezcla; las pruebas sin probar de 22, 23, 24 y 27; el digest de la imagen (28); `cargo mutants --help` (M13); la imagen del builder (26).
- Podman y `ROOT_HOME` real: el `Validate` real de 7 y 9c (con 5 repeticiones), y de 13 y 13b sobre memora y brasa.
- Claves de API: el GET a `/models` (21b); un stream real de cada proveedor (21c); el evento de 0,50 USD (decisión 46); la calibración real (12), después de P57.
- Colab: la corrida real de la 7h, con T4, `unshare -rn` y tope de 2 horas.
- Credencial de Claude y podman: la Tarea 16 con memora, dentro del contenedor de la capa 1.
- Opus: el oráculo de `graft-lector-planes` (16b) y G5; la revisión de los rastros de los paquetes a ciegas antes de G3.
- Usuario: P56, P57 y G3.

**Preguntas abiertas (sin responder)**

- **P56. ¿Listo con dimensiones sin veredicto?** Una dimensión queda sin veredicto cuando Root no pudo medirla: falta una herramienta, falla un contenedor o falta un dato. Hoy la regla del informe permite `Listo` en ese caso, y el informe lo avisa en `confianza`. El GOAL pide "sin veredicto antes que un veredicto falso". ¿Qué elegís?

  - a) `Listo` con aviso, como hoy. Riesgo: un "listo" sin medir seguridad parece un "listo" completo.
  - b) `NoListo` si alguna dimensión queda sin veredicto. Riesgo: cobra al producto una falla del instrumento.
  - c) `SinDecision` solo si queda sin veredicto una dimensión con algún ítem `bloquea = true`, como Seguridad o Completitud. Las demás dejan `Listo` con aviso.

  Mi recomendación es c): Root no dice "listo" sin medir lo que puede bloquear, y no cobra al producto una falla del instrumento.

- **P57. Alcance y costo de la calibración.** La calibración corre unas 21 evaluaciones por dimensión y por tarea, con `n = 20`. Cada evaluación llama a la API: el redactor, el juez de diseño y, a veces, el juez de config. Calcular el costo exacto necesita una corrida real. Mi estimación es de 15 a 20 USD para las ocho dimensiones en memora y brasa. Esa cifra sale del peor caso por llamada de `deepseek-flash` y no está medida. El tiempo de la laptop puede pasar de un día, porque cada evaluación corre contenedores. La Tarea 12 ya corta el costo: cada caso corre solo las partes que mide su dimensión. ¿Qué elegís?

  - a) Calibrar memora y brasa por separado, con `n = 20`. Una calibración vale solo para su tarea.
  - b) Calibrar solo memora, con `n = 20`, y usar esa calibración también en brasa. Costo: la mitad. Riesgo: la exactitud de memora puede no valer en brasa, que renderiza con GPU.
  - c) Calibrar memora y brasa con `n = 10`. Costo: la mitad. Riesgo: la cota de la exactitud queda más ancha.

  Mi recomendación es a): mide el evaluador en cada tarea, y el tope de API de 100 USD por mes acota el gasto.

## Decisión 48 (2026-10-29): P56 c y P57 a

**P56 c en las Tareas**
- **Tarea 13b (regla de decisión).** Antes de las reglas de `Bloqueado`, `NoListo` y `Listo`, el informe revisa las dimensiones sin veredicto. Si alguna tiene un ítem `bloquea = true` en su rúbrica, la decisión es `SinDecision`, con la regla `dimension_que_bloquea_sin_veredicto: <dimensión>`. Las demás dimensiones sin veredicto no cambian la decisión, y `Informe.confianza` las lista.
- **Tarea 28.** El desempate 9 de "Desempates técnicos 11" ya crea `SinDecision` para Completitud sin veredicto. La decisión 48 lo generaliza a toda dimensión con un ítem que bloquea. Va en el mismo commit del grupo "rúbricas e informe".
- **Validate extra de la Tarea 28**:
  - Seguridad sin veredicto, con su ítem `seguridad.secretos` que bloquea, da `SinDecision` y código 3.
  - Eficiencia sin veredicto, sin ítems que bloquean, deja `Listo` con Eficiencia en `confianza`.
  - El mensaje del informe nombra la regla y la dimensión.
- La nube 3 sí cambia: la Tarea 28 suma esta regla. Ver "Mensaje para la nube 3".

**P57 a en las Tareas**
- **Dónde corre.** La calibración real corre en la laptop, en la parte real de la Tarea 12. Necesita podman, `ROOT_HOME` real y las claves de API. Empieza después de que la nube 3 entre a la rama, porque esa build achica cada caso a las partes que mide su dimensión.
- **Alcance.** Ocho dimensiones en memora y ocho en brasa, con `n = 20` y semilla 42. `calibracion.json` guarda una fila por tarea y dimensión. Una calibración de memora no cuenta para brasa.
- **Tope de gasto.** `r00t oraculo calibrar` suma `--tope-usd <n>`. La calibración real corre con `--tope-usd 25`: la estimación de 15 a 20 USD, más un 25 % de margen.
  - Root suma el gasto de la calibración con las reservas y los eventos de su corrida (Tarea 21).
  - Antes de cada caso, Root compara el peor caso del caso con el resto del tope. Si no alcanza, la calibración se detiene, registra un `Gate` y deja las filas hechas marcadas `incompleta`.
  - El tope de API de 100 USD por mes sigue rigiendo por encima.
  - Subir el tope de 25 USD es zona roja (dinero): lo decide el usuario.
- **Medición.** La corrida real deja su gasto en `eventos.jsonl`. Opus compara ese gasto con la estimación de 15 a 20 USD y lo anota en el plan.
- **Tarea 12 en la nube 3.** La nube 3 suma `--tope-usd` y su prueba con un proveedor falso. Ver "Mensaje para la nube 3".

### Mensaje para la nube 3

El orquestador manda este texto a la sesión de nube 3, que arrancó desde 6ed8be4:

> El usuario respondió P56 con la opción c y P57 con la opción a (decisión 48 del plan). En la Tarea 28, generalizá `SinDecision`: toda dimensión sin veredicto que tenga un ítem `bloquea = true` en su rúbrica da `SinDecision`, con la regla `dimension_que_bloquea_sin_veredicto: <dimensión>`. Una dimensión sin veredicto sin ítems que bloquean deja `Listo`, y `Informe.confianza` la lista. Probalo con Seguridad sin veredicto, que da `SinDecision` y código 3, y con Eficiencia sin veredicto, que deja `Listo`. En la Tarea 12, sumá `--tope-usd <n>` a `r00t oraculo calibrar`. Antes de cada caso, si el peor caso no entra en el resto del tope, la calibración se detiene, registra un `Gate` y marca `incompleta` las filas hechas. Probalo con un proveedor falso. Anotá los dos cambios en el informe de la build.

## Mezcla de la nube 3 (2026-10-30)

**Mezcla.** La build de la nube 3 hizo sus 6 tareas en la rama `claude/festive-hopper-klft3e` (7a612b8). Aplicó la decisión 48 en f59275d y 7f5f196. La laptop corrió `tools/test-seguro.sh` con podman y encontró 13 tests en rojo. El builder los arregló en 3d186f8, en `laptop/nube3-podman`. El usuario escribió: "Mezclá laptop/nube3-podman en claude/root-e7813e. Después pasale al planner las 34 preguntas de la nube 3 y el hallazgo de Seguridad." La mezcla es 0a26553.
- El arreglo de la laptop:
  - 10 tests de `tests/oraculo.rs` usan tareas `shell` sin requisitos. Esos tests pasaron de esperar `Rojo` a esperar `SinVeredicto`, por la decisión 48, y 3 cambiaron de nombre.
  - Un test unitario dejó el home `/nada` y usa una carpeta temporal.
  - Dos tests de Colab guardan una revisión del juez antes de evaluar.
  - `tools/test-seguro.sh` quita las variables `GIT_*` heredadas.
- Con podman, la suite da 1771 verdes, 0 rojos y 1 ignorado. El informe es `docs/builds/2026-10-30-build-nube-3.md`.

**Lo que la nube 3 dejó sin probar** (va a la laptop):
- 29: la suite con las dos claves exportadas.
- 28: el digest real, los montajes reales de `escenarios/` y la construcción del Dockerfile dentro de `evaluar`.
- 30: `NETRC` con un índice privado real.
- 16: las corridas reales de memora y de `graft-lector-planes`.
- 26: la imagen `root-builder:1`, `--userns=keep-id`, `pasta:--no-map-gw` contra un puerto del host, la renovación del login con la credencial en solo lectura, y la prueba de que el builder no abre `ROOT_HOME`.
- 12: la calibración real con `--tope-usd 25`.

**Tareas desbloqueadas.**
- En la laptop: construir `root-builder:1` y probar la capa 1 de la 26. Después, la corrida real de memora de la Tarea 16.
- En la laptop: el `Validate` real de 7 y 9c sobre memora y brasa. Esas dos tareas miden requisitos y no dependen de la decisión del informe.
- En la laptop: el `Validate` real de 13 y 13b, después de la Tarea 31 y de P58.
- En la laptop: la calibración real (12), después de la Tarea 31. La Tarea 31 conecta las sondas de configs, y la calibración inyecta defectos de Dockerfile y un secreto en YAML (Tarea 9b).

## Hallazgo de Seguridad (2026-10-30)

**Qué pasa.** Solo existe el perfil `rust` (`perfiles::sondas_nativas` devuelve vacío para otro lenguaje). Una tarea que no es `rust` no tiene ninguna sonda de Seguridad. Seguridad queda sin veredicto, y su ítem `seguridad.secretos` tiene `bloquea = true`. La decisión 48 da entonces `SinDecision`. Hoy `decidir` aplica `SinDecision` antes que `Bloqueado` y `NoListo`: así lo escribió Opus en la decisión 48. Resultado:
- Una tarea que no es `rust` nunca da `Listo`, `NoListo` ni `Bloqueado`.
- Un test oculto que falla en esa tarea da `SinVeredicto`, aunque la falla sea una evidencia directa.
- Después del arreglo de la laptop, ningún test prueba que una falla dé `Rojo` fuera de `rust`.
- La sonda `sondas_configs` (Tarea 9b) no corre en `evaluar`, así que un secreto en un YAML no se detecta en producción (pregunta 1 de la nube 3).
- En `rust`, la imagen `root-oraculo-rust:2` sí trae `gitleaks` 8.30.1 y `cargo-audit` 0.22.2 (verificado por Opus el 2026-10-30). La pregunta 5 de la nube 3 aplica a tareas con otra imagen, como los fixtures `alpine`.

**¿Contradice lo que pide el usuario?** Sí, en parte. El usuario pidió "un evaluador con criterio, un project manager con datos para decidir". Un PM que ve un test oculto en rojo dice "no está listo", aunque no haya medido la seguridad. La decisión 48 buscaba otra cosa: no decir "listo" sin medir lo que puede bloquear. Su texto en el plan, escrito por Opus, puso `SinDecision` por encima de toda otra regla. Eso calla también la evidencia negativa, y el usuario no lo pidió: su pregunta P56 era sobre `Listo`.

**Cómo se resuelve.** Hay dos partes:
1. **Medir Seguridad en todo lenguaje** (técnico, Tarea 31). `gitleaks` no depende del lenguaje, y las sondas de configs (TOML, YAML y Dockerfile) tampoco. Las dos corren en `evaluar` para toda tarea. Así `seguridad.secretos`, el único ítem que bloquea, se mide siempre que la imagen trae `gitleaks`. Los ítems sin herramienta para ese lenguaje, como las dependencias con CVE en un proyecto `shell`, van a `faltantes` y a `Informe.confianza`.
2. **Orden de las reglas** (decisión del usuario, P58). La propuesta es aplicar primero `Bloqueado` y `NoListo`, que se apoyan en evidencia. `SinDecision` solo reemplaza a un `Listo`.

## Desempates técnicos 12 (Opus, 2026-10-30, después de la mezcla de la nube 3)

Cada desempate tiene su razón en una línea. El orden de las reglas del informe va al usuario como P58.

1. **Sondas de configs sin conectar.** La Tarea 31 las conecta en `evaluar` para todo lenguaje, antes de calibrar: la Tarea 9b las pedía en el veredicto, y la calibración las mide.
2. **`SinDecision` en validez.** Se confirma en 0: sigue el desempate 41, y un caso sin decisión no acierta el orden.
3. **`Veredicto.ocultos_fallidos`.** Se confirma, sin los inestables: un inestable ya tiene su propio campo.
4. **`umbral_verde` viejo.** Se confirma la carga con aviso para cualquier tipo: una clave obsoleta no debe romper un `root.toml`.
5. **Seguridad sin veredicto en la laptop.** Se resuelve con la Tarea 31 y con la imagen correcta: `root-oraculo-rust:2` ya trae las herramientas.
6. **Arquitectura en un crate de un archivo.** Si la tarea declara `[arquitectura].capas` y el repo no tiene esos módulos, eso es un hallazgo con el mapa como evidencia. Si la tarea no declara capas, Arquitectura va a `faltantes` con el motivo "sin módulos". Razón: una capa prometida que no existe es evidencia; un proyecto chico sin capas no lo es.
7. **Entrega construye en cada evaluación.** Se acepta el tiempo: la Tarea 9d pide construir, y la construcción ocurre una sola vez por evaluación.
8. **Reservas viejas sin filtro de mes.** Se confirma, y la lista dice el mes de cada reserva: corregirla es trabajo humano igual.
9. **`--only-binary=:none:`.** Se confirma el rechazo: reabre la construcción de sdists.
10. **M9 sin `Pendiente`.** Se confirma la salida "sin veredicto por entorno" con el mensaje del desempate 31: falla cerrado y explica la causa.
11. **Digest con imagen ausente.** Root confirma que la imagen está local antes de calcular la clave. Sin la imagen, la evaluación no puede correr igual, así que da error de infraestructura.
12. **Netrc con un host en dos puertos.** Root deja un aviso en el `Registro` de la descarga, no un hallazgo: es un problema del instrumento, no del producto.
13. **Valor de credencial raro.** Se confirma el salto, con un aviso que nombra el registro y nunca el valor: el salto protege, y el aviso explica la falla posterior de pip.
14. **Turno de pendientes.** Se confirma: el lock que queda en disco evita carreras y no guarda datos.
15. **Ausente en una repetición.** Se confirma inestable: es la misma regla que en la laptop.
16. **Revisión de la ficha por hash del spec.** Se confirma: un spec cambiado necesita otra revisión, y el aviso lo dice.
17. **`ColabEntrada.confianza`.** Se confirma el campo nuevo con `serde(default)`.
18. **Rastros a la vista.** `--preparar-ciego` escribe los conteos en `$ROOT_HOME/validez/rastros.json`, no en la salida estándar. Solo Opus lo lee, antes de entregar los paquetes (G3). Razón: el usuario no debe ver qué paquete tiene más rastros.
19. **Límite de palabra.** La búsqueda pasa a subcadena sin distinguir mayúsculas: "opus2" debe contar. Un falso positivo cuesta poco, porque Opus revisa la lista.
20. **`Cargo.toml` reescrito.** Se acepta, siempre que el armado reescriba el `Cargo.toml` de los seis paquetes igual: el formato no debe delatar un origen.
21. **Partes de Estructura, Arquitectura y Diseño.** Se acepta el ahorro menor: el juez necesita esas sondas.
22. **Partes sin la tarea.** `partes_de` recibe la tarea. Funciona y Completitud construyen la Entrega solo si la tarea tiene escenarios. Razón: sin escenarios, la construcción no aporta a esas dos dimensiones.
23. **Peor caso con reintentos.** Se confirman los 3 intentos: un 5xx se reintenta, y el tope debe ver ese peor caso.
24. **Cota de entrada del juez.** La cota pasa a ser un límite real: un prompt de juez de más de 256 KiB da `SinVeredicto` en ese juicio, sin enviarse. Así la estimación nunca queda corta.
25. **Gasto de la calibración.** Se confirma sumar todo evento de API desde el inicio: sobreestima y nunca subestima.
26. **Formato de la config de la corrida.** Se confirma `<ROOT_HOME>/configs/<config>.toml`, y `docs/interfaces.md` lo documenta.
27. **Roles por API en `r00t corre`.** Se confirma el error de uso en el hito 1: el runner lanza sesiones de `claude -p`, que van por cupo.
28. **Aprobación automática del plan.** Se acepta en las corridas de medición, con `extra.automatica = true` en el evento `PlanAprobado`. Así una aprobación humana y una automática no se confunden.
29. **Fallo del ejecutor.** Se confirma: sin builder no hay nada que medir.
30. **Transcript con líneas no JSON.** Se confirma el `Gate`: una línea que el lector salta debilita la detección.
31. **Imagen y plazo del builder.** Se confirman `root-builder:1` y 2 horas por rol en el hito 1.
32. **Red del builder.** Se confirma `pasta:--no-map-gw`: el builder necesita la API de Claude y no ve el host.
33. **Credencial anidada.** Root borra el `.credentials.json` vacío de la carpeta de la corrida al terminar, con `src/borrado.rs`.
34. **Node.js de Debian.** La laptop confirma la versión al construir `root-builder:1`. Si no alcanza para Claude Code, la imagen usa el binario oficial de Node.js con versión fija y hash verificado, como en la decisión 23.
- **Tests de `tests/test_seguro.rs` en `/tmp`.** Pasan a crear sus carpetas bajo `TMPDIR`: la regla 2 de la laptop vale también para los tests.

### Tarea 31: Seguridad en todo lenguaje y sondas de configs conectadas
- **Action**:
  - `evaluar` corre `gitleaks` y `sondas_configs` para toda tarea, con cualquier `lenguaje`. Sus hallazgos van a Seguridad, Estructura y Entrega, según la Tarea 9b.
  - Un ítem de rúbrica sin herramienta para el lenguaje de la tarea va a `faltantes` con el motivo "sin sonda para <lenguaje>". La dimensión conserva el veredicto de sus ítems medidos.
  - Los desempates 6, 11, 12, 13, 18, 19, 20, 22, 24, 28 y 33, y el arreglo de `tests/test_seguro.rs`.
- **Dónde**: en la nube, con ejecutor falso. La prueba con `gitleaks` real va en la laptop.
- **Validate**: `cargo test`:
  - Una tarea `shell` con un secreto en un YAML da un hallazgo de Seguridad con su línea.
  - Una tarea `shell` limpia, con `gitleaks` falso que no encuentra nada, da Seguridad con veredicto y la dependencia con CVE en `faltantes`.
  - Una tarea `shell` sin `gitleaks` en la imagen deja Seguridad sin veredicto.
  - Un prompt de juez de 300 KiB no se envía y deja ese juicio sin veredicto.
  - `--preparar-ciego` no imprime conteos de rastros, y `validez/rastros.json` cuenta "opus2".
  - Ningún test de `tests/test_seguro.rs` crea carpetas en `/tmp`.

### Tarea 32: Orden de las reglas del informe (decisión 49)
- **Action**: `decidir` aplica primero `Bloqueado`, después `NoListo`, y recién después revisa las dimensiones con ítems que bloquean. Si la decisión iba a ser `Listo` y alguna de esas dimensiones queda sin veredicto, la decisión pasa a `SinDecision`.
- **Validate**:
  - Una tarea `shell` sin `gitleaks`, con un test oculto que falla, da `NoListo` y código 1.
  - La misma tarea con todos los tests verdes da `SinDecision` y código 3.
  - Los tests de `tests/oraculo.rs` que el arreglo de la laptop pasó a `SinVeredicto` vuelven a su expectativa original cuando tienen evidencia de falla.
- **Dónde**: en la nube. La prueba con podman de `tests/oraculo.rs` va en la laptop.

## Orden de trabajo (2026-10-30, después de la nube 3)

El crédito de nube vence el 2026-11-05.

**Nube, sesión 4.** Arranca desde la punta de `claude/root-e7813e` que trae `docs/builds/brief-nube-4.md` (un commit después de 0a26553). Orden:
1. 31: Seguridad en todo lenguaje y sondas de configs conectadas.
2. 32: orden de las reglas del informe (decisión 49).

**Laptop.**
- Siempre: `tools/test-seguro.sh`, o el hook, sin claves y con compilaciones en /home.
- Podman: la suite completa antes de cada mezcla; el digest, los montajes de `escenarios/` y la construcción dentro de `evaluar` (28); `NETRC` contra un índice privado de prueba local (30); `gitleaks` real en una tarea `shell` (31).
- Podman y la credencial de Claude: construir `root-builder:1` y confirmar su Node.js (34); `--userns=keep-id`; `pasta:--no-map-gw` contra un puerto del host; la renovación del login con la credencial en solo lectura; la prueba de que el builder no abre `ROOT_HOME`. Después, la corrida real de memora (16).
- Podman y `ROOT_HOME` real: el `Validate` real de 7 y 9c, ya mismo; el de 13 y 13b, después de 31 y 32.
- Claves de API: la suite con las claves exportadas, para probar que `test-seguro.sh` las quita (29); el GET a `/models`; un stream real de cada proveedor; el evento de 0,50 USD (decisión 46); la calibración real con `--tope-usd 25`, después de la 31.
- Colab: la corrida real de la 7h.
- Opus: el oráculo de `graft-lector-planes` (16b) y G5; la lectura de `validez/rastros.json` antes de G3.
- Usuario: P58, G3 y G5.

**Preguntas abiertas (sin responder)**

- **P58. Orden de las reglas del informe (cambia la decisión 48).** La decisión 48 dice que una dimensión sin veredicto con un ítem que bloquea da `SinDecision`. Opus la escribió por encima de toda otra regla. Por eso, hoy un test oculto que falla no da `NoListo` si Seguridad no se midió. Tu pregunta P56 era sobre `Listo`, no sobre `NoListo`. ¿Qué elegís?

  - a) Primero la evidencia: `Bloqueado` y `NoListo` se aplican antes. `SinDecision` solo reemplaza a un `Listo`. Un producto con un test en rojo da `NoListo` aunque falte medir Seguridad.
  - b) Como hoy: sin medir lo que puede bloquear, Root no decide nada, ni siquiera `NoListo`.

  Mi recomendación es a): un PM con datos no espera a medir la seguridad para decir que algo falla. Root sigue sin decir "listo" sin medir lo que bloquea, que era el objetivo de P56 c.

**Respuesta del 2026-10-30:** P58 "P58 a" (decisión 49). P58 queda cerrada.
- La decisión 48 sigue valiendo para `Listo`: Root no dice "listo" sin medir lo que puede bloquear. Su texto "Antes de las reglas de `Bloqueado`, `NoListo` y `Listo`" queda reemplazado: la revisión de dimensiones sin veredicto corre después de `Bloqueado` y `NoListo`.
- El comentario de `decidir` ("`SinDecision` gana sobre todo") cambia con la Tarea 32.
- La Tarea 32 entra en la nube 4 sin condición. El brief de la nube 4 ya lo dice.

## Mezcla de la nube 4 (2026-10-30)

**Mezcla.** La build de la nube 4 hizo las Tareas 31 y 32 en la rama `claude/eloquent-heisenberg-5p7zff` (f2d6a39). En la laptop, `tools/test-seguro.sh` con podman sobre f2d6a39 dio 1813 verdes, 0 rojos y 1 ignorado, sin tests salteados. No hizo falta un arreglo de la laptop. El usuario escribió: "Mezclá la nube 4 en claude/root-e7813e. Después pasale al planner las 14 preguntas de la nube 4." La mezcla es 55e7cd2. El informe es `docs/builds/2026-10-30-build-nube-4.md`.

**Lo que la suite con podman ya cubrió.** La corrida de la laptop ejecutó los tests de podman que la nube 4 dejó sin correr: `gitleaks` y hadolint dentro de `evaluar`, `podman image exists` antes de la caché de Python, y los dos tests de `tests/oraculo.rs` que cambiaron de expectativa.

**Lo que sigue sin probar** (va a la laptop):
- El borrado del `.credentials.json` vacío con un montaje real (desempate 33). Va con la imagen del builder.
- `rastros.json` con el `ROOT_HOME` real (desempate 18). Va con los paquetes a ciegas reales.
- Tres huecos de tests que la nube 4 nombró: `medir` de Estructura en `shell` con un juez que mide, `medir` con capas declaradas y un repo sin esos módulos, y el error de borrado de la credencial vacía. Van a la Tarea 33.

**Tareas desbloqueadas.** Con las Tareas 31 y 32 dentro, quedan desbloqueados:
- El `Validate` real de 13 y 13b, después de la Tarea 33 y de que Opus complete las capas de los `tarea.toml`.
- La calibración real (12), con la misma condición.

## Hallazgo: las capas de las fichas no llegan a `tarea.toml` (2026-10-30)

- Opus revisó la pregunta 6 de la nube 4. Ni memora ni brasa son workspaces de crates. Las cuatro implementaciones (memora-ecc, memora-sdlc, brasa-ecc y brasa-sdlc) tienen sus módulos bajo `src/`, así que Arquitectura mide su grafo de módulos y no cae por ser workspace.
- Las fichas de memora y brasa declaran `[arquitectura].capas`, por la respuesta P23 b del usuario: "una capa importa solo capas posteriores. Rige también para los repos del banco."
- Los `tarea.toml` de `$ROOT_HOME/oraculos/{memora,brasa}/` no traen esas capas. Por eso la regla de capas nunca corre en la evaluación.
- Las capas de cada ficha juntan los nombres de módulo de las dos implementaciones. Por ejemplo, memora tiene la capa `["cmd", "cmd_keys", "cmd_strings"]`: memora-ecc tiene `cmd`, y memora-sdlc tiene los tres.
- La Tarea 31 da un hallazgo por cada módulo que falta (cita "falta <módulo>"). Con capas que juntan nombres alternativos, cada repo perdería módulos que nunca debió tener, y Arquitectura caería a 0 sin razón.
- **Resolución técnica:** una capa está presente si existe al menos uno de sus módulos. El hallazgo sale solo cuando falta la capa entera, y su cita nombra la capa con sus módulos (Tarea 33). Después de la Tarea 33, Opus copia las capas de cada ficha a su `tarea.toml` con `r00t ficha tarea`, y revisa el resultado antes de evaluar.

## Desempates técnicos 13 (Opus, 2026-10-30, después de la mezcla de la nube 4)

Ninguna de las 14 preguntas toca una decisión del usuario. Cada desempate tiene su razón en una línea.

1. **Juez sin sondas.** Se confirma: la dimensión conserva el veredicto de sus ítems medidos, como pide la Tarea 31.
2. **Estructura en `shell`.** Se confirma: sin ningún ítem medido, la dimensión queda sin veredicto, y no bloquea porque no tiene ítems que bloquean.
3. **Eficiencia fuera de `rust`.** La sonda corre en todo lenguaje cuando la tarea declara `[eficiencia]`: su comando no depende del lenguaje.
4. **Hadolint ausente.** Se confirma: Entrega no tiene ítems que bloquean, y `confianza` lo lista.
5. **Lista de ítems sin sonda.** Se confirma: el test contra las rúbricas embebidas detecta un desvío de la lista.
6. **Capas en un workspace.** Ni memora ni brasa son workspaces. La regla cambia por el hallazgo de arriba: una capa cuenta como presente si existe uno de sus módulos.
7. **Severidad de una capa faltante.** Se confirma la severidad 3. El detalle nombra la capa por sus módulos, no por un índice desde 0: una persona lee los nombres, no la posición.
8. **Partes de Funciona y Completitud.** Se confirma: sin escenarios, la Entrega no aporta a esas dos dimensiones.
9. **Límite del juez de la ficha.** Sí, el mismo límite de 256 KiB: ese juez también pasa por el chequeo de tope, y su estimación necesita una cota real.
10. **Imagen local y caché.** Se confirma: la evaluación necesita la imagen igual.
11. **Aviso de dos puertos.** Se confirma: sin clave no hay credencial que intentar.
12. **Listas de rastros.** Las listas `rastros-<id>.toml` se mueven a `$ROOT_HOME/validez/`: la carpeta de los paquetes es la que ve el usuario en G3.
13. **Credencial vacía y corte.** El borrado pasa a un guardia que corre en todo camino de salida, también ante un fallo del ejecutor. Es un cambio chico y deja la regla sin excepción.
14. **Completitud sin veredicto con evidencia.** Se confirma: es la consecuencia directa de la decisión 49.

### Tarea 33: Capas por grupo y ajustes de la nube 4
- **Action**:
  - Una capa de `[arquitectura].capas` está presente si existe al menos uno de sus módulos. El hallazgo de severidad 3 sale solo por una capa entera ausente, y su cita nombra la capa: "falta la capa [cmd, cmd_keys, cmd_strings]".
  - Los desempates 3, 7, 9, 12 y 13.
  - Los tres huecos de tests de la nube 4: `medir` de Estructura en `shell` con un juez que mide, `medir` con capas declaradas y un repo sin esos módulos, y el error de borrado de la credencial vacía.
- **Dónde**: todo en la nube.
- **Validate**: `cargo test`:
  - Con la capa `["cmd", "cmd_keys", "cmd_strings"]` y un repo con solo `cmd`, no hay hallazgo de capa.
  - Con la misma capa y un repo sin ninguno de los tres módulos, hay un hallazgo de severidad 3 que nombra los tres.
  - Una tarea `shell` con `[eficiencia]` corre la sonda de eficiencia.
  - Un prompt de 300 KiB para el juez de la ficha no se envía.
  - Después de `--preparar-ciego`, la carpeta de los paquetes no tiene archivos `rastros-*`.
  - Un fallo del ejecutor después de montar la credencial no deja el `.credentials.json` vacío.

## Orden de trabajo (2026-10-30, después de la nube 4)

**Nube, sesión 5.** Queda una tarea chica que la nube puede hacer antes del 2026-11-05: la Tarea 33. Arranca desde la punta de `claude/root-e7813e` que trae `docs/builds/brief-nube-5.md` (un commit después de 55e7cd2).

**Laptop, en pasos.** Cada paso corre con `tools/test-seguro.sh` para la suite, y con claves solo si el paso lo dice.

1. **Validate real de las Tareas 7 y 9c. Arranca ya.** Necesita podman y el `ROOT_HOME` real. Corre sin claves de API: los jueces y el redactor quedan sin veredicto, y no hay gasto. Mide los niveles por requisito de memora y brasa en 3 evaluaciones seguidas, con 5 repeticiones, y rehace `diferencias.md`.
2. **Imagen del builder (Tarea 26, capa 1).** Necesita podman y red para construir `root-builder:1`. Confirma la versión de Node.js, `--userns=keep-id` y que `pasta:--no-map-gw` no alcanza un puerto del host. También confirma que el builder no abre `ROOT_HOME`.
3. **Credencial de Claude en el contenedor.** Necesita la credencial y gasta un poco de cupo: una sesión corta de `claude -p` dentro del contenedor. Prueba la renovación del login con la credencial en solo lectura y el borrado del `.credentials.json` vacío. Si la renovación falla, Opus pregunta al usuario antes de cambiar el montaje (decisión 43).
4. **Pruebas con claves, sin gasto o con gasto mínimo.** Necesita las claves de API. Prueba que `tools/test-seguro.sh` quita las claves exportadas. Hace el GET a `/models` de DeepSeek y un stream real de cada proveedor, por unos centavos. Registra el evento de 0,50 USD de la decisión 46.
5. **`NETRC` con un índice privado de prueba (Tarea 30).** Necesita podman y una imagen de índice local, que se baja con red. No gasta API.
6. **Capas en los `tarea.toml`.** Espera la mezcla de la nube 5. Opus corre `r00t ficha tarea` para memora y brasa, revisa las capas y las copia a cada `tarea.toml`.
7. **Validate real de las Tareas 13 y 13b.** Espera el paso 6. Necesita podman, el `ROOT_HOME` real y las claves. El gasto es bajo: el redactor y el juez de diseño por evaluación, sobre cuatro repos.
8. **Calibración real (Tarea 12).** Espera el paso 6. Necesita podman, el `ROOT_HOME` real y las claves. Gasta hasta 25 USD, con `--tope-usd 25` (decisión 48).
9. **Corrida real de memora (Tarea 16).** Espera los pasos 2 y 3. Necesita podman, la credencial y cupo: una construcción completa con Haiku (decisión 38).
10. **Colab (Tarea 7h).** Necesita la CLI de Colab con login del usuario y una T4 paga, con tope de 2 horas (decisiones 22, 33 y 40).
11. **Graft (Tareas 16b y 16).** Opus escribe el oráculo de `graft-lector-planes`, y el usuario aprueba G5. Después corre la corrida real de Graft.
12. **Paquetes a ciegas (Tarea 12b) y G3.** La laptop arma los paquetes reales, y Opus lee `validez/rastros.json`. Después, el usuario califica a ciegas (G3) y decide si memora sigue (G4).

## Validate real de las Tareas 7 y 9c (2026-10-30)

**Cómo se corrió.** El orquestador usó `r00t` release de 53985ce, el `ROOT_HOME` real, podman y ninguna clave de API. Las 8 evaluaciones tardaron 2,7 horas y gastaron 0 USD. Las salidas están en `~/.cache/root-wt/validate7/`. Antes, la sesión de fichas completó los `[[requisito]]` de memora (16) y brasa (14), solo con ids de la suite oculta.

| Repo | Decisión | Completitud | Contrato | Oculto | Motivo |
|---|---|---|---|---|---|
| memora-ecc ×3 | listo | 0,957 | 61/61 | 47/48 | ninguna regla disparó |
| brasa-ecc ×3 | no_listo | 0,852 | 53/53 | 14/15 | completitud_bajo_minimo 0,85 < 0,90 |
| memora-sdlc | bloqueado | 0,226 | 21/61 | 20/48 | bloqueo_requisito: R1 y R13 en nivel 0 con peso 3 |
| brasa-sdlc | bloqueado | 0,593 | 34/53 | 10/15 | bloqueo_requisito: R12 en nivel 0 con peso 3 |

**Qué cumple.**
- La Tarea 7 pide estabilidad: las 3 evaluaciones de cada repo ECC dan los mismos niveles por requisito. Ningún test quedó inestable.
- El orden conocido se respeta: memora-ecc queda por encima de memora-sdlc, y brasa-ecc por encima de brasa-sdlc.
- La Tarea 9c marca `Ausente` los requisitos que el SDLC no construyó. Cumple con el criterio corregido: memora-ecc queda `Cumplido` salvo R7.

**Qué no cumple todavía, y por qué.**
- **memora-ecc, R7 `Parcial`.** `test_blpop_fifo_entre_clientes` falló en las 15 corridas. La ficha aprobada exige FIFO (decisión 26), así que es un defecto del producto.
- **brasa-ecc, R3 `Parcial`.** `test_spin_despues_de_rotacion` falla: es el defecto del spin en los ejes del padre (decisión 26).
- **brasa-ecc, R9 `Ausente`.** Los 5 `test_modelo_khronos[...]` se omiten porque faltan las muestras `.glb`. Es un hueco del oráculo, no del producto. La decisión 51 lo cierra. Hasta repetir la evaluación, la decisión `no_listo` de brasa-ecc incluye ese hueco.
- **Seis requisitos `SinVeredicto`.** memora R2, R4 y R16, y brasa R7, R13 y R14 no tienen tests ocultos. El contrato sí los prueba. La decisión 50 y la Tarea 34 los cubren.
- **Estructura, Arquitectura y Diseño sin veredicto.** El juez no corrió, porque la evaluación fue sin claves. Es lo esperado en este paso.
- **Eficiencia sin veredicto.** Las tareas no definen `[eficiencia]`.

**Hallazgo sin diagnóstico: FIFO 9/30 contra 0/15.**
- `diferencias.md` de memora registraba 9 pases de 30 para `test_blpop_fifo_entre_clientes`, en el host y en el contenedor.
- Este Validate dio 0 pases de 15 en el contenedor.
- La causa no está probada. Las candidatas son tres: la imagen cambió de `root-oraculo-rust:1` a `:2`, la carga de la máquina era otra, o el test cambió con la ficha.
- El paso de la laptop "Diagnóstico de FIFO" lo investiga. Hasta entonces, el hallazgo figura en `diferencias.md` con la causa "sin diagnosticar". El origen sigue siendo "producto", porque falla en las dos mediciones.

## Desempates técnicos 14 (Opus, 2026-10-30): el contrato como evidencia (decisión 50)

1. **Formato de id.** Un id con el prefijo `contrato:` apunta a la suite de contrato, por ejemplo `contrato:test_contrato::test_R6_archivo_de_config`. Un id sin prefijo sigue apuntando a la suite oculta. Así los `tarea.toml` de hoy no cambian.
2. **Nivel.** La regla de niveles de la Tarea 9c no cambia. Cada id de `pruebas` y `bordes` se busca en su suite. Un test inestable de cualquier suite pone tope 2 al requisito. Un test de `[[entorno]]` sigue fuera del cálculo.
3. **Peso de la evidencia del contrato.** Cuenta igual que la oculta: el usuario eligió que el contrato pruebe requisitos. Como el builder ve el contrato, `Informe.confianza` lista los requisitos con evidencia solo del contrato, con la marca "evidencia visible al builder".
4. **Tests fuera de requisitos.** La regla `test_oculto_fuera_de_requisitos` no cambia. Se suma `test_contrato_fuera_de_requisitos`: un test del contrato que falla de forma estable y no pertenece a ningún requisito da `NoListo`. Razón: un test del contrato también es una promesa de la spec.
5. **Redacción por MCP.** Los ids del contrato no se redactan: el builder ya los ve.
6. **Decisión 16.** Los tests siguen siendo evidencia y no el veredicto. Los niveles salen de la evidencia, y la decisión sale de las reglas del informe.

### Tarea 34: Requisitos cruzados con el contrato (decisión 50)
- **Action**: los desempates 1 a 5 de arriba. `medir` pasa a `requisitos` las clasificaciones del contrato y del oculto. `Veredicto` suma `contrato_fallidos`, con los ids del contrato que fallan de forma estable.
- **Dónde**: todo en la nube.
- **Validate**: `cargo test`:
  - Un requisito con `pruebas = ["contrato:t::a"]` y ese test verde llega a nivel 2 o 3, según sus bordes.
  - El mismo requisito figura en `Informe.confianza` con "evidencia visible al builder".
  - Un id sin prefijo sigue buscándose en la suite oculta.
  - Un test del contrato inestable pone tope 2 a su requisito.
  - Un test del contrato que falla fuera de los requisitos da `NoListo` con `test_contrato_fuera_de_requisitos`.
  - `contrato:` con un id que no existe deja el requisito `Ausente` en ese test.

## Orden de trabajo (2026-10-30, después del Validate de 7 y 9c)

**Nube, sesión 6.** Hace la Tarea 34 antes del 2026-11-05. Su brief es `docs/builds/brief-nube-6.md`. El commit de partida es la mezcla de la nube 5, que hace el orquestador.

**Laptop, pasos nuevos.** Se suman a los pasos de "Orden de trabajo (2026-10-30, después de la nube 4)". El paso 1 de esa lista ya está hecho.
- **Probar y mezclar la nube 5.** La laptop corre `tools/test-seguro.sh` con podman sobre `claude/zen-mccarthy-61lrlw` (a54f059). La mezcla espera la orden del usuario.
- **Muestras de Khronos (decisión 51).** El orquestador baja las 5 muestras a `oraculos/brasa/oculto/muestras/`. Guarda la fuente, la versión y la licencia de cada archivo en `oraculos/brasa/oculto/muestras/FUENTES.md`. Esa carpeta vive en `ROOT_HOME`, nunca en el repo.
- **Diagnóstico de FIFO.** Necesita podman, sin claves. Corre `test_blpop_fifo_entre_clientes` 30 veces en `root-oraculo-rust:2` y 30 veces en el host, y anota la causa en `diferencias.md`.
- **Capas en los `tarea.toml`.** Después de la mezcla de la nube 5, Opus copia las capas de cada ficha (paso 6 de la lista anterior).
- **Ids del contrato.** Después de la mezcla de la nube 6, la sesión de fichas suma los ids `contrato:` a memora R2, R4 y R16, y a brasa R7, R13 y R14.
- **Validate de 7 y 9c, otra vez.** Después de las capas, las muestras y los ids del contrato, el orquestador repite las 8 evaluaciones sin claves. Tarda unas 3 horas y gasta 0 USD. Una sola corrida cubre los tres cambios.
- Después siguen los pasos 2 a 5 y 7 a 12 de la lista anterior.

No hay preguntas nuevas para el usuario.

## Estado (2026-10-31): nube 5 mezclada, nube 6 en prueba

- **Commit del planner.** El hook rechazó el commit del plan dos veces. El orquestador hizo `cargo clean -p root` en `~/.cache/root-tests/target` y commiteó en 8ddc677. El hook dio 1816 verdes y 2 ignorados. El diagnóstico del planner era correcto: la suite de la nube 5 dejó un binario viejo de `importar` en la carpeta `target` compartida.
- **Imágenes temporales.** Con una carpeta `target` propia y sin otra suite en paralelo, `tests/entrega.rs` de la nube 5 dio 47 verdes en 3 de 3 corridas. Las fallas "queda una imagen temporal" venían de dos suites con podman a la vez.
- **Nube 5.** El usuario ordenó la mezcla, y entró en 6ee7c1d. El brief de la nube 6 recibió su commit de partida en d26c138. El hook dio 1831 verdes con podman.
- **Nube 6.** Hizo la Tarea 34 en `claude/wizardly-hypatia-bzwf59` (bea5975). Su informe es `docs/builds/2026-10-30-build-nube-6.md`, en esa rama. Sin podman dio 1856 verdes, 0 rojos y 2 ignorados. El orquestador la prueba con podman en la laptop. La mezcla espera la orden del usuario.
- **En paralelo.** La sesión de fichas copia las capas de cada ficha a su `tarea.toml` y suma los ids `contrato:` a los seis requisitos sin tests ocultos.

## Regla 4 de la laptop (2026-10-31): una carpeta de compilación por worktree, y una suite con podman a la vez

- **Qué pasó.** Dos fallas del hook vinieron del entorno, no del código:
  - `importar_metrics_reales_da_575_eventos_sin_tocar_el_original` corrió aunque en 53985ce lleva `#[ignore]`. La suite de la nube 5 lo había compilado sin `#[ignore]` en la misma carpeta `target`, y cargo reusó ese binario.
  - Tres tests de `tests/entrega.rs` fallaron con "queda una imagen temporal", porque otra suite creaba imágenes `root-entrega-*` al mismo tiempo.
- **Regla.**
  - `tools/test-seguro.sh` usa una carpeta de compilación por worktree: `$HOME/.cache/root-tests/<hash corto de la ruta del repo>/target`. El `ROOT_HOME` y el `TMPDIR` de prueba también van por worktree.
  - Una sola suite con podman corre a la vez en la laptop. El script toma un lock con `flock` sobre `$HOME/.cache/root-tests/podman.lock` y espera su turno. Con `ROOT_SIN_PODMAN=1`, el script no toma el lock.
  - Mientras la regla no esté en el script, el orquestador no corre dos suites con podman a la vez.
- **Dónde.** Es la Tarea 35, y va a la nube 7. El script y sus tests no necesitan podman: el lock se prueba con dos corridas del script sobre un comando falso. La laptop solo confirma el lock con dos suites reales.

### Tarea 35: Suite por worktree y un lock de podman (regla 4)
- **Action**: la regla de arriba en `tools/test-seguro.sh`.
- **Dónde**: en la nube. La confirmación con dos suites reales va en la laptop.
- **Validate**:
  - Dos rutas de repo distintas dan dos carpetas `target` distintas. La misma ruta da siempre la misma carpeta.
  - Dos corridas en paralelo del script, sin `ROOT_SIN_PODMAN`, con un comando falso que tarda 2 segundos, no se pisan: la segunda empieza cuando termina la primera.
  - Con `ROOT_SIN_PODMAN=1`, dos corridas en paralelo no esperan el lock.
  - Ningún test crea carpetas en `/tmp`.

## Desempates técnicos 15 (Opus, 2026-10-31): preguntas de la nube 6

Ninguna de las 7 preguntas toca una decisión del usuario. Cada desempate tiene su razón en una línea.

1. **`[[entorno]]` y el contrato.** Una exclusión acepta el prefijo `contrato:` en forma explícita. Un id sin prefijo excluye solo el test oculto. Razón: el mismo nombre en dos suites son dos tests distintos, y el formato de id del desempate 1 de "Desempates técnicos 14" ya lo fija.
2. **Evidencia por MCP.** `redactar_para_mcp` conserva las entradas `contrato:` de la evidencia de los requisitos, y quita solo las ocultas. Razón: el builder ya ve el contrato, y el desempate 5 de "Desempates técnicos 14" dice que esos ids no se redactan.
3. **Redacción por subcadena.** Se acepta en el hito 1: un texto de más que se quita deja la salida más cerrada, no más abierta.
4. **Hallazgos de inestables del contrato.** Se confirma: un inestable se liga a un riesgo solo con el id de su suite, por el formato de id.
5. **Requisito mezclado.** Un requisito entra en `evidencia_visible` si ninguno de sus ids ocultos aporta un resultado medido: pasa, falla o inestable. Razón: `confianza` debe decir de dónde salió la evidencia que de verdad se midió, no la que se declaró.
6. **Doble cuenta del contrato.** Se acepta: Funciona mide si los tests pasan, y Completitud mide qué requisitos quedan cubiertos. Los tests ocultos ya cuentan en las dos dimensiones de la misma forma.
7. **Efecto antes de los ids de las fichas.** Se confirma: ningún resultado de los cuatro repos cambia, y la sesión de fichas suma los ids en paralelo.

### Tarea 36: Desempates de la nube 6
- **Action**: los desempates 1, 2 y 5 de arriba.
- **Dónde**: en la nube.
- **Validate**: `cargo test`:
  - `[[entorno]]` con `contrato:t::a` excluye ese test del contrato, y `t::a` no lo excluye.
  - `root_evaluar` conserva la evidencia `contrato:` de un requisito y quita la oculta.
  - Un requisito con un id `contrato:` verde y un id oculto ausente entra en `evidencia_visible`.

## Orden de trabajo (2026-10-31)

**Nube, sesión 7.** Hace las Tareas 36 y 35, en ese orden, antes del 2026-11-05. Su brief es `docs/builds/brief-nube-7.md`. El commit de partida lo pone el orquestador: es la mezcla de la nube 6.

**Laptop, en pasos.**
1. **Probar y mezclar la nube 6.** El orquestador corre la suite con podman sobre bea5975. La mezcla espera la orden del usuario. Mientras tanto, nadie corre otra suite con podman.
2. **Capas e ids del contrato.** La sesión de fichas termina de copiar las capas y de sumar los ids `contrato:`. Corre en paralelo, sin suite.
3. **Muestras de Khronos y diagnóstico de FIFO.** Necesitan podman y red para la descarga (decisión 51). Corren después del paso 1, de a una suite por vez.
4. **Validate de 7 y 9c, otra vez.** Espera los pasos 1 a 3. Necesita podman y el `ROOT_HOME` real, sin claves. Tarda unas 3 horas y gasta 0 USD.
5. **Pasos siguientes.** Siguen los pasos 2 a 5 y 7 a 12 de "Orden de trabajo (2026-10-30, después de la nube 4)": la imagen del builder, la credencial, las pruebas con claves, `NETRC`, el Validate de 13 y 13b, la calibración, la corrida de memora, Colab, Graft y G3.
6. **Mezcla de la nube 7.** La laptop confirma el lock con dos suites reales.

No hay preguntas nuevas para el usuario.

## Estado (2026-10-31): nube 6 y nube 7 mezcladas

- **Nube 6.** El orquestador la mezcló en cde8ac5 y commiteó la vuelta del planner en 6519c09. El hook dio 1856 verdes con podman.
- **Nube 7.** Hizo las Tareas 36 y 35 en `claude/awesome-maxwell-sf7z43` (acb2548). Su informe es `docs/builds/2026-10-31-build-nube-7.md`. Con podman, la laptop dio 1875 verdes, 0 rojos y 2 ignorados. El usuario escribió: "Mezclá la nube 7 en claude/root-e7813e y pasale al planner sus 8 preguntas." La mezcla es acda9e6, con el árbol igual a acb2548.
- **Lock de podman confirmado.** Dos suites reales corrieron a la vez. La segunda avisó "test-seguro: espera el lock de podman" y corrió después de la primera. Las dos dieron 1875 verdes. La regla 4 de la laptop queda cumplida en el script.
- **Carpetas viejas.** `~/.cache/root-tests/target`, `target-nube5`, `target-nube6` y `tmp` quedan en disco, sin uso. Son cachés de compilación de la suite.

**Fichas y oráculos en el `ROOT_HOME` real** (sesión de fichas, decisiones 52 a 54):
- Los dos `tarea.toml` tienen las capas de sus fichas aprobadas.
- memora R2, R4 y R16, y brasa R7, R13 y R14 tienen ids `contrato:`.
- La ficha de brasa pide un `LICENSE` en la raíz para R14 (decisión 52).
- memora suma 20 tests ocultos y brasa suma 1 (decisión 53). Los repos ECC los pasan en 3 de 3 corridas. Los repos SDLC fallan por faltantes del producto.
- La ficha de brasa fija los nombres de wgpu de `brasa info` (decisión 54).
- memora tiene 68 tests ocultos y brasa 22. Todos pertenecen a un requisito.

**Diagnóstico de FIFO (cerrado).**
- `test_blpop_fifo_entre_clientes` aislado sobre memora-ecc pasa 6 de 30 en `root-oraculo-rust:2` y 4 de 30 en el host. La imagen no explica el cambio.
- Con las mediciones anteriores (9 de 30 en el banco, 0 de 15 en el Validate), la tasa de pase queda cerca de 1 de 6. El 0 de 15 es azar de muestra: con esa tasa, 15 fallas seguidas pasan cerca de 6 veces cada 100.
- Con 5 repeticiones, un test que pasa 1 de cada 6 veces falla las 5 corridas cerca de 4 veces cada 10. Por eso Root lo clasificó como falla estable. Es un límite estadístico conocido de 5 repeticiones, no un defecto de Root.
- El origen es "producto": la ficha exige FIFO, y el producto lo viola en la mayoría de las corridas. R7 de memora-ecc queda `Parcial` con las dos lecturas.
- El resultado está en `diferencias.md` de memora.

**Muestras de Khronos (decisión 51).** La herramienta de permisos rechazó la descarga del orquestador. El usuario tiene el comando y todavía no lo corrió. Las muestras siguen faltando.

## Desempates técnicos 16 (Opus, 2026-10-31): preguntas de la nube 7 y `ficha revisar`

Ninguna pregunta toca una decisión del usuario. Cada desempate tiene su razón en una línea.

1. **Colab y el contrato.** El paquete de Colab también corre las entradas `contrato:` de `[[entorno]]`: un test del contrato que necesita GPU tampoco tiene resultado en la laptop, y el contrato no es secreto.
2. **Colab sin prefijo.** `pendientes::localizar` busca un id sin prefijo solo en `oculto/`, por el formato de id. El único `[[entorno]]` real (brasa, `test_rendimiento[gpu-10000-res1]`) vive en `oculto/`, así que ninguna tarea depende de la búsqueda en `contrato/`.
3. **`[[oraculo_dudoso]]` y el contrato.** Acepta el prefijo `contrato:` igual que `[[entorno]]`, y la suite del contrato saca esos tests: las dos exclusiones siguen la misma regla de id.
4. **Pendientes del contrato por MCP.** `Veredicto.pendientes` conserva las entradas `contrato:` en `root_evaluar`, como la evidencia del desempate 2 de "Desempates técnicos 15".
5. **Evidencia por subcadena.** Se confirma, por el desempate 3 de "Desempates técnicos 15".
6. **Evidencia visible sin medida.** Un requisito entra en `evidencia_visible` solo si al menos un test `contrato:` aporta un resultado medido y ningún test oculto lo aporta. Un requisito sin ningún resultado medido no tiene evidencia que mostrar.
7. **Lock con `ROOT_TESTS_BASE`.** Se confirma: los tests usan su propia base y nunca tocan el lock real.
8. **`sha256sum` y `flock`.** Se confirman: la laptop corre Fedora Linux, y la nube también corre en Linux.
9. **`ficha revisar` sin juez.** El código 3 es lo esperado: lo fijó el desempate B15 de la Tarea 25, porque la salida 0 exige las dos capas limpias. Para revisar solo la capa determinista, `ficha revisar` suma `--solo-deterministas`. Con ese flag, la salida es 0 o 1 según las reglas deterministas, y el texto dice "juez: no pedido". Razón: una revisión sin juez es útil, pero tiene que pedirse en forma explícita.
10. **Test inestable de la Tarea 35.** El hook del planner falló una vez en `test_seguro_dos_rutas_dan_dos_carpetas` con `ETXTBSY` ("Text file busy"). Un test escribe un ejecutable falso y lo lanza mientras otro hilo de la suite todavía tiene ese archivo abierto para escritura. Es una carrera de Linux con tests en paralelo. Los tests de `tests/test_seguro.rs` escriben cada ejecutable en un archivo temporal, lo cierran y recién después lo renombran a su nombre final. Razón: un `rename` no deja abierto el archivo final para escritura en ningún proceso.

### Tarea 37: Desempates de la nube 7
- **Action**: los desempates 1, 2, 3, 4, 6, 9 y 10 de arriba.
- **Dónde**: en la nube. La prueba del paquete de Colab con un test `contrato:` real va en la laptop.
- **Validate**: `cargo test`:
  - Un `[[entorno]]` con `contrato:t::a` suma ese test al paquete de Colab.
  - Un id sin prefijo que solo existe en `contrato/` no entra al paquete, y deja un aviso.
  - `[[oraculo_dudoso]]` con `contrato:t::a` saca ese test de la suite del contrato.
  - `root_evaluar` conserva un pendiente `contrato:` y quita uno oculto.
  - Un requisito con su único test `contrato:` ausente no entra en `evidencia_visible`.
  - `ficha revisar --solo-deterministas` sobre una ficha limpia sale con 0 y dice "juez: no pedido".
  - `tests/test_seguro.rs` pasa 20 veces seguidas con `cargo test --test test_seguro`, sin `ETXTBSY`.

## Orden de trabajo (2026-10-31, después de la nube 7)

**Nube, sesión 8.** Hace la Tarea 37 antes del 2026-11-05. Su brief es `docs/builds/brief-nube-8.md`. El commit de partida lo pone el orquestador. Después de la nube 8 no queda trabajo de código que la nube pueda hacer sin podman, `ROOT_HOME`, claves o Colab.

**Validate de 7 y 9c: memora ya, brasa con las muestras.** memora no depende de las muestras de Khronos. Sus cambios (20 tests ocultos, ids `contrato:` y capas) se miden ya. brasa espera las muestras: sin ellas, R9 queda `Ausente` por un hueco del oráculo, y esa corrida habría que repetirla igual.

**Laptop, en pasos.**
1. **Validate de memora. Arranca ya.** Corre memora-ecc tres veces y memora-sdlc una vez. Necesita podman y el `ROOT_HOME` real, sin claves. Gasta 0 USD.
2. **Muestras de Khronos.** El usuario corre el comando de descarga que tiene el orquestador (decisión 51).
3. **Validate de brasa.** Espera el paso 2. Corre brasa-ecc tres veces y brasa-sdlc una vez, sin claves.
4. **Carpetas viejas.** El orquestador borra `~/.cache/root-tests/target`, `target-nube5`, `target-nube6` y `tmp` cuando no corre ninguna suite. Son cachés de la suite, y la Tarea 35 ya no las usa.
5. **Mezcla de la nube 8.** La laptop corre la suite con podman, y la mezcla espera la orden del usuario.
6. **Pasos siguientes.** Siguen los pasos 2 a 5 y 7 a 12 de "Orden de trabajo (2026-10-30, después de la nube 4)": la imagen del builder, la credencial, las pruebas con claves, `NETRC`, el Validate de 13 y 13b, la calibración, la corrida de memora, Colab, Graft y G3.

No hay preguntas nuevas para el usuario.

## Validate de memora (2026-10-31, sobre 18ec96a)

El usuario escribió: "Sí, pasale al planner los dos hallazgos del Validate de memora." El orquestador corrió `r00t` release de 18ec96a, con el `ROOT_HOME` real y sin claves. brasa no corrió, porque faltan las muestras de Khronos. Las salidas están en `~/.cache/root-wt/validate7/memora-*.json` y en `~/.cache/root-wt/validate7/intento2/`.

| Repo | Decisión | Completitud | Ocultos | Detalle |
|---|---|---|---|---|
| memora-ecc #1 | listo | 1,000 | 68 de 68 | R7 en nivel 3: FIFO pasó las 5 repeticiones |
| memora-ecc #2 | listo | 0,964 | 67 de 68 | R7 en nivel 1: FIFO inestable, pasa 7 de 10 |
| memora-ecc #3 | listo | 0,964 | 67 de 68 | R7 en nivel 1: FIFO inestable, pasa 6 de 10 |
| memora-sdlc #1 | bloqueado | 0,225 | 24 de 68 | R1 y R13 en nivel 0 |

- **Qué cumple.** La decisión de memora-ecc es la misma en las 3 corridas, como pide la decisión 16. El orden conocido se respeta: memora-ecc queda por encima de memora-sdlc. Los 20 tests ocultos nuevos y los ids `contrato:` entraron sin errores.
- **Qué no cumple.** El nivel de R7 cambia entre corridas (hallazgo 1). Las 4 evaluaciones traen un riesgo falso de Arquitectura (hallazgo 2).
- **Riesgos de Arquitectura reales.** Con las capas de la ficha (P23 b), memora-ecc tiene un ciclo `cmd -> persist -> cmd`, `persist` importa `cmd`, y `store` importa `persist`. Una capa importa solo capas posteriores, así que esos tres riesgos son defectos del producto.

## Desempates técnicos 17 (Opus, 2026-10-31): hallazgos del Validate de memora

**Hallazgo 1: R7 cambia de nivel entre corridas.**
- **Causa.** Dentro de la evaluación, `test_blpop_fifo_entre_clientes` pasa cerca del 65 % de las veces. Con 5 repeticiones, ese test pasa las 5 en cerca de 12 corridas de cada 100, y Root no lo ve inestable. Con 10 repeticiones, la chance baja a 1 de cada 100, pero cada evaluación duplica su tiempo.
- **Cuál criterio falla.** La decisión 16 del usuario pide que la decisión del PM sea estable, y lo es. El criterio "mismos niveles por requisito en 3 evaluaciones" lo agregó Opus en la Tarea 7. Un instrumento con muestras chicas no puede cumplirlo frente a un test con 65 % de pase.
- **Desempate.** Root acumula evidencia por versión del repo. Guarda el resultado de cada test por corrida en `$ROOT_HOME/evidencia/<tarea>/<hash del contenido del repo>/historial.jsonl`. La clasificación de estable o inestable usa todas las corridas acumuladas de esa misma versión. Razón: un PM con datos no olvida una falla que ya vio, y más evidencia del mismo producto da un juicio más firme sin repetir más la suite.
  - El hash sale del contenido de la copia del repo, sin `target/` ni `.git/`. Así un cambio sin commit no mezcla evidencia de dos versiones.
  - Un repo con otro contenido arranca un historial vacío.
  - `Informe.confianza` dice cuántas corridas acumula cada test inestable.
- **Criterio de la Tarea 7, corregido.** La decisión es la misma en 3 evaluaciones seguidas. Desde la segunda evaluación de la misma versión, los niveles por requisito no cambian.
- **R7.** Con el historial, R7 de memora-ecc queda en nivel 1 de forma estable. Es el defecto FIFO que exige la ficha (decisión 26).

**Hallazgo 2: "falta la capa [main]" es un falso positivo.**
- **Causa.** `perfiles::modulo_de` excluye `src/main.rs` y `src/lib.rs`, y `mapa.rs` los llama `(raiz)`. La capa `["main"]` de memora nunca aparece, aunque el repo tenga `src/main.rs`.
- **brasa.** Tiene el mismo problema: su primera capa es `["main", "lib"]`, y los dos archivos existen en brasa-ecc y en brasa-sdlc.
- **Desempate.** Para la regla de capas, `src/main.rs` cuenta como el módulo `main` y `src/lib.rs` como el módulo `lib`, con sus importaciones. Razón: las fichas aprobadas nombran esas capas, y la raíz del crate es la que importa a las demás.
- **Lo que no cambia.** Para el criterio "sin módulos" del desempate 6 de "Desempates técnicos 12", la raíz del crate no cuenta. Un crate de un archivo sigue sin módulos, y no recibe un puntaje de Arquitectura sin análisis.
- **Fichas y `tarea.toml`.** No cambian: las capas de la ficha son correctas, y el defecto está en el código de Root.

### Tarea 38: Raíz del crate en las capas (hallazgo 2)
- **Action**: el desempate del hallazgo 2. El test del perfil que fija "un repo con solo `src/main.rs` da módulos: ninguno" sigue valiendo para el criterio "sin módulos", y se suma un test para la regla de capas.
- **Dónde**: en la nube.
- **Validate**: `cargo test`:
  - Con `capas = [["main"], ["server"]]` y un repo con `src/main.rs` y `src/server.rs`, no hay hallazgo de capa faltante.
  - Con `capas = [["main", "lib"], ["app"]]` y un repo con `src/lib.rs` y `src/app.rs`, no hay hallazgo de capa faltante.
  - Si `src/server.rs` importa algo de `main`, hay un hallazgo de orden de capas.
  - Un repo con solo `src/main.rs` sigue dando Arquitectura en `faltantes` con el motivo "sin módulos".

### Tarea 39: Historial de evidencia por versión del repo (hallazgo 1)
- **Action**: el desempate del hallazgo 1.
- **Dónde**: en la nube. La prueba con memora-ecc real va en la laptop.
- **Validate**: `cargo test`:
  - Un test que pasa en las 5 corridas de una evaluación, y falla en una corrida de la evaluación anterior de la misma versión, queda inestable.
  - Un repo con un archivo cambiado arranca un historial vacío.
  - `target/` y `.git/` no cambian el hash de la versión.
  - `Informe.confianza` muestra las corridas acumuladas de cada test inestable.
  - El historial vive en `$ROOT_HOME/evidencia/`, fuera de `oraculos/`, y no guarda el contenido de los tests ocultos, solo sus ids y resultados.

**Orden de la nube 8 (actualizado).** 37, 38 y 39, en ese orden. El brief de la nube 8 ya lo dice. La laptop repite después el Validate de memora, y el de brasa cuando estén las muestras.

**Pedido para la sesión de fichas.** No hay: ningún `tarea.toml` ni ficha cambia por estos hallazgos.

No hay preguntas nuevas para el usuario.

## Mezcla de la nube 8 y Validate de 7 y 9c, intento 4 (2026-10-31)

**Mezcla.** El usuario escribió: "Mezclá la nube 8 en claude/root-e7813e." La mezcla es 92d8b18, con las Tareas 37, 38 y 39. Con podman, la suite dio 1919 verdes, 0 rojos y 2 ignorados. El informe es `docs/builds/2026-10-31-build-nube-8.md`.

**Cambio de fichas.** La sesión de fichas arregló `test_modelo_khronos` del oculto de brasa, con la aprobación del usuario: "P2 a: pasale el arreglo del test de R9 a la sesión de fichas." El test saca la escala y el centro de cada modelo de su caja en mundo, que incluye las matrices de los nodos. También falla si el modelo toca el borde del cuadro.

**Validate, intento 4.** El orquestador usó `r00t` release de 92d8b18, el `ROOT_HOME` real y ninguna clave. Tardó 213 minutos. Las salidas están en `~/.cache/root-wt/validate7/intento4/`.

| Repo | Decisión | Completitud | Ocultos | Detalle |
|---|---|---|---|---|
| memora-ecc #1 | listo | 0,964 | 67 de 68 | R7 en nivel 1: FIFO pasa 5 de 10 |
| memora-ecc #2 | listo | 0,928 | 66 de 68 | R7 en 1 (9 de 20); R4 en 1: `test_client_id` pasa 13 de 15 |
| memora-ecc #3 | listo | 0,928 | 66 de 68 | R7 en 1 (13 de 30); R4 en 1 (18 de 20) |
| memora-sdlc #1 | bloqueado | 0,225 | 24 de 68 | R1 y R13 en nivel 0 |
| brasa-ecc #1, #2 y #3 | listo | 0,938 | 20 de 21 | R9 sube a nivel 3; R3 sigue en nivel 1 |
| brasa-sdlc #1 | bloqueado | 0,562 | 10 de 21 | R12 en nivel 0; R8, R9, R10, R13 y R14 en 0 |

**Qué comprobó el orquestador.**
- Tarea 38: "falta la capa [main]" y "falta la capa [main, lib]" desaparecen en las 8 evaluaciones. Las aristas `crate::X` hacia la raíz no suman riesgos nuevos.
- Tarea 38: quedan riesgos de capas que parecen reales: el ciclo `gltf_load -> scene -> gltf_load` de brasa-ecc, las capas `[shadow, pbr]` y `[camera, light, transform]` que brasa-ecc no tiene, y `[gltf_load]` y `[limits]` que brasa-sdlc no tiene.
- Tarea 39: R7 de memora-ecc queda en nivel 1 en las 3 evaluaciones. El historial guarda solo ids y resultados: 45 líneas, 15 por evaluación.
- R9: brasa-ecc sube de no_listo a listo, y su completitud pasa de 0,896 a 0,938.
- La salida no trae claves.

**Estado de los criterios de las Tareas 7 y 9c.**
- **Tarea 7: cumple.** La decisión es la misma en las 3 evaluaciones de cada repo ECC. Desde la segunda evaluación de cada versión, los niveles por requisito no cambian. Cada diferencia con el banco tiene una causa en `diferencias.md`.
- **Tarea 9c: cumple.** memora-sdlc y brasa-sdlc marcan `Ausente` los requisitos que el SDLC no construyó. memora-ecc queda `Cumplido` salvo R7 (FIFO) y R4 (`test_client_id` inestable). brasa-ecc queda `Cumplido` salvo R3 (spin).
- El orden conocido se respeta en memora y en brasa: ECC queda por encima de SDLC.
- Los niveles de los requisitos con un test inestable cambian con el desempate 1 de abajo, después de P59.

## Desempates técnicos 18 (Opus, 2026-10-31): nube 8, hallazgo 3 y redondeo

**Hallazgo 3: un test inestable de 90 % baja su requisito igual que uno de 43 %.**
- **Desvío del código.** La Tarea 9c dice: "un requisito con un test `inestable` no supera el nivel 2". El código hace otra cosa: `verde()` cuenta solo `Pasa`, así que un inestable cuenta como "no verde". Por eso R7 y R4 de memora-ecc quedan en nivel 1, no en 2.
- **Desempate técnico.** El código vuelve a la regla de la Tarea 9c: el nivel se calcula con el test inestable como verde, y después queda con tope 2. Con esa regla, R7 (43 %) y R4 (90 %) quedan los dos en nivel 2.
- **Parte de producto.** Si un test que pasa 43 % de las veces debe valer lo mismo que uno de 90 % cambia lo que el evaluador promete. Va al usuario como P59.

**Defecto del redondeo.**
- En el intento 3, brasa-ecc dio el motivo "completitud 0,90 < 0,90", con un valor real de 0,896. El texto redondeaba a dos decimales.
- **Desempate.** `numero` muestra tres decimales en las reglas que comparan contra un umbral. Si los dos números siguen iguales con tres decimales, muestra cuatro. Razón: un motivo que compara dos números iguales no explica nada.

**Preguntas de la nube 8.** Cada desempate tiene su razón en una línea.
1. **Colab sin tests.** El aviso de la omisión llega a `Veredicto.avisos`: un aviso que se pierde no informa a nadie.
2. **Conftest de Colab.** El paquete separa las suites: `tests/oculto/` y `tests/contrato/`, cada una con su conftest, y ninguno en `tests/`. Razón: en la laptop, cada suite corre con su propio conftest, y Colab debe dar el mismo resultado.
3. **Mismo archivo en dos suites.** Se confirma el error de tarea: el junit no distingue la carpeta, y un error claro es mejor que un resultado mezclado.
4. **Pendientes del contrato por MCP.** Se confirma, por el desempate 3 de "Desempates técnicos 15".
5. **Revisión guardada.** Se confirma: `--solo-deterministas` no borra una revisión del juez, porque esa revisión sigue valiendo para el mismo spec.
6. **ETXTBSY.** Se confirma el arreglo actual. Si la falla vuelve, los tests lanzan cada ejecutable falso con `sh <ruta>` en lugar de ejecutarlo directo: `sh` no ejecuta el archivo, solo lo lee, y no da `ETXTBSY`.
7. **Aristas hacia la raíz.** Se confirma: con la regla de capas del usuario (P23 b), usar un tipo de la raíz desde una capa posterior es importar una capa anterior.
8. **Elección de la raíz.** Se confirma: `lib` si `src/lib.rs` declara el módulo, `main` si no. Es la regla de Rust para la raíz de cada árbol de módulos.
9. **Nombre `main` repetido.** No hace falta cubrirlo en el hito 1: Rust no compila un crate con `src/main/` y `src/main.rs` como dos módulos `main`.
10. **Errores del historial.** Se confirma: un error de lectura o escritura es infraestructura, como en los demás escritores del Oracle.
11. **Colab y el historial.** Se confirma que queda fuera: Colab corre en otro entorno, y mezclar sus corridas con las de la laptop mezclaría dos entornos.
12. **Tamaño del historial.** La calibración no escribe historial: cada caso es una versión distinta y se evalúa una vez. `evaluar` borra, por `src/borrado.rs`, los historiales de versiones sin uso en 30 días.
13. **Número repetido.** El texto deja solo "pasa X de N": N ya es la cantidad de corridas acumuladas.

### Tarea 40: Desempates de la nube 8, tope 2 de inestables y redondeo
- **Action**: el desempate técnico del hallazgo 3, el del redondeo, y los desempates 1, 2, 12 y 13 de las preguntas de la nube 8. Si el usuario responde P59 antes de la build, se suma su regla.
- **Dónde**: en la nube. La prueba con memora-ecc real va en la laptop.
- **Validate**: `cargo test`:
  - Un requisito con dos pruebas, una verde y una inestable, queda en nivel 2.
  - El mismo requisito sin el test inestable, y con sus bordes y escenarios verdes, llega a nivel 3.
  - Un motivo con completitud 0,896 y mínimo 0,90 dice "0,896 < 0,900".
  - Si todas las entradas de `[[entorno]]` se omiten, `Veredicto.avisos` trae el aviso.
  - El paquete de Colab tiene `tests/oculto/conftest.py` y `tests/contrato/conftest.py`, y no tiene `tests/conftest.py`.
  - Una calibración no escribe historial.
  - Un historial de una versión sin uso en 31 días se borra al evaluar otra versión.
  - El texto de un inestable dice "pasa X de N" una sola vez.

## Orden de trabajo (2026-10-31, después de la nube 8)

**Nube, sesión 9.** Hace la Tarea 40 antes del 2026-11-05. Su brief es `docs/builds/brief-nube-9.md`. El commit de partida lo pone el orquestador.

**Laptop: qué sigue.** Las Tareas 7 y 9c cumplen sus criterios. El paso siguiente es probar los proveedores reales, porque el Validate de 13 y 13b y la calibración gastan API y dependen de ellos.
1. **Pruebas con claves. Arranca ya.** Necesita las claves de API y gasta centavos. Prueba que `tools/test-seguro.sh` quita las claves exportadas. Hace el GET a `/models` de DeepSeek y un stream real de cada proveedor, con razonamiento. Registra el evento de 0,50 USD de la decisión 46.
2. **Validate de 13 y 13b.** Espera el paso 1. Necesita podman, el `ROOT_HOME` real y las claves. Gasto bajo: el redactor y los jueces sobre los cuatro repos.
3. **Calibración real.** Espera el paso 2. Gasta hasta 25 USD, con `--tope-usd 25` (decisión 48).
4. **Imagen del builder y credencial de Claude.** Corre en paralelo con los pasos 1 a 3, porque no gasta API. Necesita podman, red y la credencial. Gasta un poco de cupo.
5. **`NETRC` con un índice privado de prueba.** Necesita podman y red. No gasta API.
6. **Corrida real de memora (Tarea 16).** Espera el paso 4. Necesita la credencial y cupo.
7. **Colab (7h), Graft con G5 y paquetes a ciegas con G3.** Siguen después, como en la lista anterior.

**Pedido para la sesión de fichas.** No hay: ningún `tarea.toml` ni ficha cambia por esta vuelta.

**Preguntas abiertas (sin responder)**

- **P59. ¿Un test inestable vale lo mismo con 43 % que con 90 % de pase?** Hoy los dos dejan su requisito en el mismo nivel. La Tarea 40 corrige un desvío del código: los dos pasan del nivel 1 al nivel 2, como dice la Tarea 9c. Queda la pregunta de producto. ¿Qué elegís?

  - a) La regla de la Tarea 9c: todo test inestable pone tope 2, sin importar su tasa de pase. FIFO (43 %) y `test_client_id` (90 %) quedan en nivel 2.
  - b) Por tasa: un test inestable cuya cota inferior de Wilson al 90 % es al menos 0,5 pone tope 2. Uno con una cota menor cuenta como falla. Con los datos de hoy, `test_client_id` (18 de 20, cota 0,78) queda en nivel 2, y FIFO (13 de 30, cota 0,32) cuenta como falla.
  - c) Como hoy: todo test inestable cuenta como falla. Los dos requisitos quedan en nivel 1.

  Mi recomendación es b): un PM distingue una función que falla de vez en cuando de una que falla casi siempre. La cota de Wilson ya es la medida que Root usa en Track y en Route.

## Vuelta del 2026-11-01: P59 b, pruebas con claves, Tarea 7 y Validate de 13 (memora)

**P59 b (decisión 55).** El usuario respondió "B". El orquestador arrancó la nube 9 desde dcd43fb con esa regla, por orden del usuario: "Arrancá la nube 9 con P59 b." La nube 9 todavía no entregó su rama.

**Pruebas con claves (paso 1).** El usuario las ordenó: "a".
- Tarea 29: la suite con podman corrió con las dos claves exportadas, falsas. Dio 1919 verdes, 0 rojos y 2 ignorados. Ningún test dio un 401, y el `eventos.jsonl` real no recibió líneas de la suite.
- 21b: `GET /models` de DeepSeek responde 200 y lista `deepseek-flash` y `deepseek-v4-pro`.
- 21c, DeepSeek: el stream manda `reasoning_content` durante todo el razonamiento. El hueco más largo entre bytes fue 1,6 s.
- 21c, Anthropic: `claude-haiku-5-5` con `thinking: {"type": "adaptive"}` manda el razonamiento en un solo `thinking_delta`, al final del bloque. El hueco fue 3,2 s con 661 tokens y 5,0 s con 996. Solo llegó un `ping`, al principio.
- `r00t llama` real: `verifica` con `deepseek-v4-pro`, `delega` con `deepseek-flash` y el respaldo Haiku sin la clave de DeepSeek funcionan. `verifica` llegó al tope de 3000 tokens en 26,5 s, y Root la marcó truncada.
- Decisión 46: el evento de 0,50 USD quedó registrado con `--fecha 2026-10-29`. El orquestador registró además 0,0065 USD de streams corridos fuera de Root.

**Criterio de la Tarea 7 y diagnóstico de brasa.** El usuario escribió: "Mantené el criterio exacto. Arreglá el test del oráculo para que espere a los 3 clientes bloqueados en vez de dormir 0,1 s, y que Sonnet diagnostique cada diferencia de brasa."
- FIFO: la sesión principal arregló `test_blpop_fifo_entre_clientes` en el oráculo del banco. Ahora espera `blocked_clients:N` de `INFO clients` en vez de dormir 0,1 s. Con esa espera, memora-ecc falla en 17 de 20 corridas.
- FIFO: la causa está en el producto. `block_loop` de `src/server.rs` despierta a todos con un `Notify` y no guarda una cola FIFO. Antes, el sueño de 0,1 s mezclaba una carrera del test con el defecto.
- FIFO: el orquestador aplica el mismo cambio en la copia de Root (`oraculos/memora/oculto/test_oculto_memora.py`) después del Validate de 13 y 13b.
- brasa: un subagente Sonnet diagnosticó brasa-ecc solo con lecturas. El contrato es idéntico byte a byte, y el banco y Root dan 53/53. `test_spin_despues_de_rotacion` falla en los dos por el producto: `scene.rs:271` aplica `Rs·Re`, y la ficha pide `T·Re·Rs(t)·S`.
- brasa: el 15/16 del banco y el 20/21 de Root miden suites distintas. Root suma los 5 tests de Khronos y `test_info_lista_adaptador_con_backend_y_tipo`, y deja el test de GPU pendiente por entorno. En los tests comunes, Root da lo mismo que el banco.

**Validate de 13 y 13b, memora.** El orquestador corrió `r00t --json oraculo evaluar` con las claves y el `ROOT_HOME` real, con `r00t` release de dcd43fb. memora gastó 0,16 USD. Las salidas están en `~/.cache/root-wt/validate13/`.

| Repo | Decisión | Completitud | Funciona | Seguridad | Estructura | Arquitectura | Diseño | Pruebas | Global |
|---|---|---|---|---|---|---|---|---|---|
| memora-ecc | listo | 0,928 | 0,984 | 0,926 | sin veredicto | 0,667 | 0,083 | 0,433 | vacío |
| memora-sdlc | bloqueado (R1 y R13) | 0,225 | 0,349 | 1,000 | sin veredicto | 0,644 | 0,750 | 0,000 | vacío |

- Cumple: los dos repos listan las 9 dimensiones; memora-sdlc da bloqueado y nombra su regla; los pasos de memora-ecc citan ids reales (R4 y R7); la salida no trae claves.
- No cumple: las causas A a D de abajo.

**brasa del Validate de 13.** Se cortó dos veces por reinicios, a las 22:16 y a las 23:57. El orquestador la relanza después de esta vuelta. El reinicio también borró el registro de costos del orquestador, que vivía en `/tmp`. Por la regla 2 de la laptop, ese registro también va en /home.

## Desempates técnicos 19 (Opus, 2026-11-01)

**Hallazgo 4: inactividad de Anthropic con razonamiento.**
- Con `adaptive`, Anthropic manda el razonamiento al final del bloque. A unos 200 tokens por segundo, el silencio pasa los 120 s de inactividad cerca de los 24 000 tokens, y los jueces piden hasta 32 000.
- **Desempate.** El plazo de inactividad depende del proveedor. DeepSeek sigue con 120 s: el stream medido nunca calló más de 1,6 s. Anthropic con razonamiento usa como inactividad el mismo plazo total de la llamada (`timeout_juez_s`), sin un corte propio. Razón: el silencio de Anthropic es parte de su protocolo, no una conexión muerta.
- La laptop confirma después, con una llamada de Haiku de más de 24 000 tokens de razonamiento, si Anthropic manda `ping` en esa espera. El costo es de centavos.

**Causa A: `global` vacío antes de calibrar.**
- `global` usa solo dimensiones calibradas (Tarea 13), y la calibración real es el paso siguiente. El criterio "memora-sdlc obtiene un `global` menor que memora-ecc" no se puede medir antes.
- **Desempate.** El orden de los pasos no cambia: la calibración necesita la Tarea 13 funcionando. El criterio de la Tarea 13 se parte en dos. Las nueve dimensiones, la decisión y las citas se validan ahora. La comparación de `global` se valida después de la calibración real.

**Causa B: Estructura sin veredicto por un ítem del juez.**
- `juntar_partes` deja sin veredicto la dimensión entera si la parte del juez no tiene veredicto, y descarta los 3 ítems medidos por las sondas.
- **Desempate.** El valor de una dimensión sale de sus ítems medidos, de sonda o de juez. Un ítem sin nivel válido va a `faltantes`. La dimensión queda sin veredicto solo si no midió ningún ítem, o si no cumple la cobertura de la causa C. Esto reemplaza el desempate 4 de "Desempates técnicos 11" y unifica la regla con el desempate 1 de "Desempates técnicos 13". Razón: tirar tres mediciones por una que falta pierde datos sin ganar nada.

**Causa B': faltantes perdidos.**
- `juntar_partes` y `sin_veredicto_puntaje` arman el `Puntaje` con `faltantes` vacío.
- **Desempate.** Los dos llevan los ítems que faltaron, con su motivo: "el juez no dio nivel" o "la evidencia no pasó el gate". `Informe.confianza` los lista. Razón: el informe debe decir qué no midió.

**Causa C: Diseño al revés.**
- memora-ecc recibió 4 ítems del juez y memora-sdlc 2. Los dos promedios usan ítems distintos, así que no se comparan. El juez además puntúa el código que existe, no lo que falta.
- **Desempate.** Una dimensión con ítems del juez necesita cobertura: los ítems medidos deben sumar al menos dos tercios del peso de su rúbrica. Con menos, queda sin veredicto y con sus `faltantes`. Razón: un promedio sobre una parte chica y distinta de los ítems no compara dos repos.
- Que el juez acierte lo miden la calibración (Tarea 12) y la validez (Tarea 12b). Hasta entonces, Diseño no entra a `global`.

**Causa D: los pasos no nombran módulos.**
- `faltante` nombra módulos solo si el requisito declara `modulos`, y ningún requisito de memora ni de brasa lo declara.
- **Desempate.** No cambia el código: el mapa entre requisitos y módulos es conocimiento de la tarea. La sesión de fichas suma `modulos` a cada requisito (pedido de abajo).

**Historial y oráculo cambiado.**
- La Tarea 39 indexa el historial solo por el contenido del repo. Con el test FIFO cambiado en el oráculo, las corridas viejas y las nuevas del mismo repo se mezclarían.
- **Desempate.** La clave del historial suma el hash de las carpetas `oculto/` y `contrato/` de la tarea. Un oráculo cambiado arranca un historial vacío.

**Valores de referencia de la Tarea 7.** El criterio no cambia, como pidió el usuario. Los números del banco tampoco cambian: son la medición histórica. La causa de cada diferencia queda en `diferencias.md`:
- memora-ecc: con el test FIFO arreglado, R7 falla por el producto (sin cola FIFO en `block_loop`).
- brasa-ecc: 20/21 en Root contra 15/16 en el banco, por suites distintas. En los tests comunes, los dos dan lo mismo.

### Tarea 41: Mediciones parciales, inactividad de Anthropic y clave del historial
- **Action**: los desempates del hallazgo 4, B, B', C y la clave del historial.
- **Dónde**: en la nube 9, por mensaje del orquestador. La prueba de Haiku con más de 24 000 tokens va en la laptop.
- **Validate**: `cargo test`:
  - Estructura con 3 ítems de sonda medidos y el ítem del juez sin nivel da un valor, y lista ese ítem en `faltantes`.
  - Diseño con ítems medidos que suman menos de dos tercios del peso queda sin veredicto, con sus `faltantes`.
  - `sin_veredicto_puntaje` conserva sus `faltantes`, y `Informe.confianza` los muestra.
  - Un stream falso de Anthropic con razonamiento que calla 200 segundos no se corta antes del plazo total.
  - Un stream falso de DeepSeek que calla 121 segundos se corta.
  - Cambiar un archivo de `oculto/` arranca un historial vacío para el mismo repo.

### Mensaje para la nube 9

El orquestador manda este texto a la sesión de nube 9, que arrancó desde dcd43fb:

> El planner suma la Tarea 41 a esta build, después de la Tarea 40. Está en `.claude/plans/root.plan.md`, en "Desempates técnicos 19" y "Tarea 41", en el commit que te pasa el orquestador. Primero: el valor de una dimensión sale de sus ítems medidos, y un ítem del juez sin nivel va a `faltantes`, sin borrar las sondas. Segundo: `juntar_partes` y `sin_veredicto_puntaje` conservan sus `faltantes`, y `Informe.confianza` los muestra. Tercero: una dimensión con ítems del juez necesita que los ítems medidos sumen dos tercios del peso de su rúbrica; si no, queda sin veredicto. Cuarto: Anthropic con razonamiento no tiene corte de inactividad propio, solo el plazo total; DeepSeek sigue con 120 s. Quinto: la clave del historial suma el hash de `oculto/` y `contrato/` de la tarea. Anotá la Tarea 41 en el informe de la build.

### Pedido para la sesión de fichas

> Sumá el campo `modulos` a cada `[[requisito]]` de `oraculos/memora/tarea.toml` y `oraculos/brasa/tarea.toml`. Cada lista nombra los módulos que implementan ese requisito en las dos implementaciones, con los nombres de las capas de la ficha. Por ejemplo, el requisito de persistencia de memora nombra `persist` y `db`. Un requisito que no vive en un módulo propio lleva la lista vacía. Root usa ese campo para nombrar los módulos que faltan en los próximos pasos del informe (causa D del Validate de 13).

## Orden de trabajo (2026-11-01)

1. **brasa del Validate de 13 y 13b.** El orquestador la relanza con las claves. Gasto bajo. Va sola, sin otra suite con podman.
2. **Arreglo de FIFO en la copia de Root.** Después del paso 1. Cambia el oráculo de memora, y con la Tarea 41 el historial arranca de cero.
3. **Mezcla de la nube 9.** Con las Tareas 40 y 41. Espera la orden del usuario.
4. **Ids de módulos.** La sesión de fichas suma `modulos` (pedido de arriba). Corre en paralelo.
5. **Validate de 7 y 13 sobre memora, otra vez.** Después de los pasos 2 a 4. Mide R7 con el test arreglado, la regla P59 b y las mediciones parciales.
6. **Calibración real (Tarea 12).** Con `--tope-usd 25` (decisión 48). Después, la comparación de `global` de la Tarea 13.
7. **Prueba de Haiku con más de 24 000 tokens de razonamiento.** Gasto de centavos. Confirma el hallazgo 4.
8. Siguen la imagen del builder, la credencial, `NETRC`, la corrida de memora, Colab, Graft con G5 y los paquetes a ciegas con G3.

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-02: brasa del Validate de 13, FIFO en Root y nube 9

**Validate de 13 y 13b, brasa.** El orquestador corrió `r00t --json oraculo evaluar` con las claves y el `ROOT_HOME` real, con `r00t` release de dcd43fb, antes de la Tarea 41. brasa gastó 0,27 USD, con el informe md de memora-sdlc incluido. Las salidas están en `~/.cache/root-wt/validate13/`.

| Repo | Decisión | Contrato | Oculto | Funciona | Completitud | Estructura | Arquitectura | Diseño | Pruebas | Global |
|---|---|---|---|---|---|---|---|---|---|---|
| brasa-ecc | listo | 53/53 | 20/21 | 0,986 | 0,938 | sin veredicto | sin veredicto | 0,333 | 0,233 | vacío |
| brasa-sdlc | bloqueado (R12, peso 3) | 34/53 | 10/21 | 0,595 | 0,563 | 0,426 | 0,417 | 0,000 | 0,000 | vacío |

- Cumple: brasa-ecc da 20/21 en el oculto, igual que el diagnóstico de Sonnet. Su único paso es R3, por el fallo de spin del producto.
- Cumple: brasa-sdlc da bloqueado y nombra su regla. Sus pasos citan R12, R3, R8, R9 y R10.
- Cumple: el test de GPU queda pendiente por entorno, fuera del denominador. La salida no trae claves.
- No cumple: las mismas causas A a D de memora. Estructura y Arquitectura de brasa-ecc quedan sin veredicto con "ningún ítem tiene nivel": es la causa B, que la Tarea 41 corrige.

**FIFO en la copia de Root.** El orquestador aplicó el hunk del banco a `oraculos/memora/oculto/test_oculto_memora.py`, con respaldo `.bak2`. El hunk suma `_esperar_bloqueados` y reemplaza `time.sleep(0.1)` del bucle. Nadie lo corrió todavía en Root.

**Nube 9.** Entregó las Tareas 40 y 41 en `claude/sleepy-darwin-paheql`. Su informe es `docs/builds/2026-11-01-build-nube-9.md`, en el commit 9934827. La laptop corrió `tools/test-seguro.sh --no-fail-fast` con podman en un worktree descartable: 1974 verdes, 0 fallas y 2 ignorados. El usuario escribió: "Si mezcla." La mezcla es f23829e, sin conflictos.

**En paralelo.** El usuario escribió: "Si ambos". La sesión de fichas suma `modulos` a los requisitos de memora y brasa.

## Desempates técnicos 20 (Opus, 2026-11-02): preguntas 11 a 16 de la nube 9

Cada desempate tiene su razón en una línea.

11. **Peso de las sondas sin ítems.** Se confirma: la parte de sondas pesa 1 en el valor y no cuenta en la cobertura. La cobertura mide qué parte de la rúbrica midió el juez, y las sondas sin ítems no son parte de la rúbrica.
12. **Fórmula ponderada.** Se confirma `(Ws·vs + Wj·vj) / (Ws + Wj)`: es el promedio de los ítems medidos que pide la causa B de "Desempates técnicos 19". Reemplaza la media simple del desempate 4 de "Desempates técnicos 11".
13. **Orden de los motivos.** Se confirma: la cobertura va primero, porque explica por qué la dimensión no tiene valor.
14. **Estructura en `shell` sin veredicto por cobertura.** Se confirma. La regla de cobertura de la causa C de "Desempates técnicos 19" reemplaza el desempate 1 de "Desempates técnicos 13" para las dimensiones con ítems del juez. Un juez que mide 2 de 7 ítems no da un valor comparable.
15. **`oculto` o `contrato` como archivo.** Pasa de infraestructura a error de tarea, con código 2. Una carpeta mal definida en la tarea es un error de definición, no una falla del entorno. Va en la Tarea 42.
16. **Plazos del test de DeepSeek.** Se confirman 300 ms de silencio contra 120 ms de inactividad: el plazo es inyectable, y el test prueba la misma regla sin esperar dos minutos.

**Hueco de Arquitectura en Rust sin módulos.** Con la Tarea 41, Arquitectura en un repo Rust sin módulos puede tomar valor del juez. Ningún test lo cubre con las rúbricas embebidas. No hace falta antes del Validate: memora y brasa tienen módulos, así que este caso no los toca. El test va en la Tarea 42.

### Tarea 42: Error de tarea por carpeta inválida, test de Arquitectura sin módulos y módulos de los pasos
- **Action**: el desempate 15 y el test del hueco de Arquitectura. Con las rúbricas embebidas, un repo Rust sin módulos y un juez que mide sus ítems de Arquitectura con cobertura suficiente da un valor. Con cobertura insuficiente, queda sin veredicto, con sus `faltantes`.
- **Action, P-mod1 a (decisión 56)**: `faltante` (`src/oracle/informe.rs`) filtra la lista `modulos` del requisito contra los módulos del repo evaluado.
  - La fuente es el mismo conjunto `presentes` que usa la regla de capas de la Tarea 38 (`capas_sin_modulo` en `src/oracle/perfiles/rust.rs`): los módulos del mapa más `main` y `lib` si existe su archivo. Ese conjunto sale de una sola función compartida, así el informe y la regla de capas nunca difieren.
  - Si quedan módulos que existen, el paso dice "revisar" con esos módulos.
  - Si no queda ninguno, el paso dice "falta" con la lista entera del requisito.
  - Vale igual para un requisito ausente y para uno parcial. Sin lista `modulos`, `faltante` sigue como hoy y nombra los tests que no pasan.
  - Hoy `faltante` escribe "falta" si el requisito está ausente y "revisar" si está parcial, sin mirar el repo. Ese texto cambia.
- **Action, ETXTBSY otra vez**: el 2026-11-02, el hook del planner falló en `test_seguro_misma_ruta_da_la_misma_carpeta` con `ETXTBSY`. Se aplica el desempate 6 de "Desempates técnicos 18": los tests de `tests/test_seguro.rs` lanzan cada ejecutable falso con `sh <ruta>`, nunca directo.
- **Dónde**: en la laptop, con el builder local (Sonnet organiza, Haiku construye). Es chica y no necesita podman ni claves.
- **Validate**: `cargo test`:
  - Una tarea con `oculto` como archivo sale con código 2 y un error de tarea.
  - Los dos casos de Arquitectura en Rust sin módulos dan lo que dice la Action.
  - Un requisito con `modulos = ["cmd", "cmd_keys"]` sobre un repo que tiene solo `cmd` dice "revisar" con `cmd`, y no nombra `cmd_keys`.
  - Un requisito con `modulos = ["persist", "store"]` sobre un repo sin ninguno de los dos dice "falta" con `persist` y `store`.
  - Un requisito con `modulos = ["main"]` sobre un repo con `src/main.rs` dice "revisar" con `main`.
  - Un requisito sin `modulos` sigue nombrando los tests que no pasan.
  - La regla de capas y `faltante` usan la misma función para los módulos presentes.
  - `tests/test_seguro.rs` pasa 50 veces seguidas con `cargo test --test test_seguro`, sin `ETXTBSY`.

## Orden de trabajo (2026-11-02)

1. **Ids de módulos.** La sesión de fichas suma `modulos` a memora y brasa. Ya corre.
2. **Prueba de Haiku con más de 24 000 tokens de razonamiento.** Gasta centavos y no usa podman. Confirma el hallazgo 4 antes de usar el respaldo Haiku en la calibración.
3. **Validate de 7 y 13, otra vez.** Espera el paso 1. Corre con `r00t` release de f23829e o posterior, con las claves y el `ROOT_HOME` real. El gasto estimado es de 1 USD o menos.
   - memora-ecc corre 3 veces y memora-sdlc una. memora repite el Validate de 7, porque su test FIFO cambió y su historial arranca de cero.
   - brasa-ecc y brasa-sdlc corren una vez cada uno. brasa no repite el Validate de 7: su oráculo no cambió, y no tiene tests inestables. Sí repite el de 13, para confirmar las causas B, B', C y D.
4. **Tarea 42.** En paralelo con el paso 3, con el builder local. No usa podman.
5. **Calibración real (Tarea 12).** Espera el paso 3. Con `--tope-usd 25` (decisión 48). Después, la comparación de `global` de la Tarea 13.
6. Siguen la imagen del builder, la credencial, `NETRC`, la corrida de memora, Colab, Graft con G5 y los paquetes a ciegas con G3.

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-02: `modulos` en las fichas y P-mod1 a

**Resultado de la sesión de fichas.**
- La sesión sumó `modulos` a los 30 requisitos: 16 de memora y 14 de brasa. Guardó `tarea.toml.bak6` en las dos tareas.
- El orquestador comprobó que los dos `tarea.toml` parsean, y que el diff solo suma líneas `modulos`.
- Cada nombre sale de `[arquitectura] capas`. Cada lista junta los módulos de ECC y de SDLC.
- Los repos del banco están en `~/Proyectos/bench-ecc-sdlc/`.
- Faltan estos módulos: memora-ecc no tiene `cli`, `cmd_keys` ni `cmd_strings`; memora-sdlc no tiene `config`, `persist` ni `store`; brasa-ecc no tiene `camera`, `cli`, `light`, `pbr`, `renderer`, `shadow` ni `transform`; brasa-sdlc no tiene `app`, `gltf_load` ni `render`.
- La sesión corrigió el pedido del planner: `faltante` nunca mira qué módulos existen en el repo. Por eso nombrar la lista entera mezclaría módulos de las dos implementaciones.

**P-mod1 (decisión 56).** El usuario respondió "P-mod1 a". El cambio de código entra en la Tarea 42.

**El Validate espera a la Tarea 42.** La causa D del Validate de 13 se mide con el texto de los pasos, y ese texto cambia con la Tarea 42. Correr el Validate antes obligaría a repetirlo. La Tarea 42 es chica y la hace el builder local. Mientras tanto corre la prueba de Haiku, que no depende de la 42.

## Orden de trabajo (2026-11-02, después de `modulos`)

1. **Tarea 42 con el builder local.** Brief: `docs/builds/brief-local-42.md`. No usa podman.
2. **Prueba de Haiku con más de 24 000 tokens de razonamiento.** En paralelo con el paso 1. Gasta centavos.
3. **Validate de 7 y 13, otra vez.** Espera el paso 1. Igual que en "Orden de trabajo (2026-11-02)": memora corre 3 veces más una, y brasa una vez por repo. Gasto estimado: 1 USD o menos.
4. **Calibración real (Tarea 12).** Con `--tope-usd 25`. Después, la comparación de `global` de la Tarea 13.
5. Siguen la imagen del builder, la credencial, `NETRC`, la corrida de memora, Colab, Graft con G5 y los paquetes a ciegas con G3.

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-02: prueba de Haiku, Tarea 42 y su mezcla

**Prueba de Haiku con razonamiento largo.** El usuario la ordenó: "Sí, las dos". El orquestador corrió 3 streams directos de `claude-haiku-5-5` con `thinking: {"type": "adaptive"}`, fuera de Root. Gastó 0,033 USD, registrados con `r00t observa costo`. El script es `~/.cache/root-wt/orquestador/haiku-largo.py`.

| Corrida | Esfuerzo | Tokens de razonamiento | Duración | Hueco máximo | Pings |
|---|---|---|---|---|---|
| 1 | por defecto (medium) | 412 | 3,1 s | 2,0 s | 1 |
| 2 | por defecto (medium) | 405 | 5,2 s | 1,9 s | 1 |
| 3 | `output_config: {"effort": "max"}` | 64 000 (tope) | 259 s | 30,1 s | 9 |

- En la corrida 3, Anthropic mandó un `ping` cada 30 s. El razonamiento llegó en un solo `thinking_delta` al final, a unos 247 tokens por segundo.
- Root corta la inactividad por bytes, y un `ping` trae bytes. El corte de 120 s nunca salta con Anthropic.
- El hallazgo 4 de "Desempates técnicos 19" suponía silencio total. La prueba lo refuta: las pruebas cortas de 21c vieron un solo `ping` porque duraron menos de 30 s.
- Con el esfuerzo por defecto, Haiku razonó unos 400 tokens. Root no manda `output_config`, así que sus llamadas usan `medium`.

**Tarea 42.** El usuario la ordenó: "Sí, las dos". Sonnet organizó y Haiku construyó, por 1,18 USD. La rama es `laptop/tarea-42`, y el informe es `docs/builds/2026-11-02-build-local-42.md`.

| Parte | Commit |
|---|---|
| 1. `oculto` o `contrato` como archivo da error de tarea con código 2 | 176e887 |
| 2. Tests de Arquitectura en Rust sin módulos | 672ef1a |
| 3. Módulos de los pasos (decisión 56) | 90c6ab6 |
| 4. ETXTBSY: los falsos se lanzan con `sh` | 6042ad8 |
| Informe | 4667511 |

- La suite con podman dio 1990 verdes, 0 fallas y 2 ignorados.
- La parte 2 no cambió código: sus dos tests quedaron como guardas.
- ETXTBSY no se reprodujo en 50 corridas seguidas, ni antes ni después del cambio.
- Ningún test recorre `evaluar_con` completo con `modulos`.

**Mezcla.** El usuario escribió: "Sí, mezclá". La mezcla es 64479c4, sin conflictos, con el árbol igual a 4667511.

## Desempates técnicos 21 (Opus, 2026-11-02)

**Inactividad de Anthropic: se revierte la excepción de la Tarea 41.**
- Anthropic manda un `ping` cada 30 s durante el razonamiento, y Root corta por bytes. Con 120 s, Anthropic nunca se corta por error.
- La excepción de la Tarea 41 (b790507) dejaba a Anthropic sin detección de una conexión muerta durante todo `timeout_juez_s`.
- **Desempate.** Anthropic vuelve al mismo plazo de inactividad de 120 s que DeepSeek. Razón: la medición refutó la causa de la excepción, y la excepción tenía un costo real.
- Va en la Tarea 43, con el builder local.

**Preguntas del informe de la Tarea 42.**
1. **Código 2 solo para la carpeta inválida.** Se confirma la variante `ErrorOraculo::TareaDefinicion` con código 2. Cambiar todos los `Tarea` a 2 toca 37 usos y un contrato fijado por tests. Queda como deuda: un error de definición de tarea debería salir siempre con 2.
2. **Repo que no es Rust.** Se corrige. Sin un perfil de lenguaje, Root no conoce los módulos del repo. Decir "falta" con la lista entera afirma algo que Root no midió. Sin perfil, `faltante` vuelve al texto de antes y nombra los tests que no pasan.
3. **Requisito ausente con un módulo presente.** Se confirma: es lo que pide la decisión 56. El módulo existe, así que el paso apunta a revisarlo.
4. **Módulo con otro nombre.** Se confirma el nombre exacto: los nombres salen de las capas de la ficha aprobada.
5. **`ROOT_TEST_SEGURO_CARGO`.** Se confirma: solo la usan los tests.
6. **`sh` contra `bash`.** Se corrige. `tools/test-seguro.sh` usa `BASH_SOURCE`, así que es un script de bash. Los tests lo lanzan con `bash <ruta>`. Los ejecutables falsos, que son POSIX, siguen con `sh`. Razón: en Fedora `sh` es bash, pero en otras imágenes de Linux, como muchas de nube, `sh` es `dash`, y la suite fallaría.

### Tarea 43: Inactividad de Anthropic, módulos sin perfil y `bash` en los tests
- **Action**: los desempates de inactividad, 2 y 6 de "Desempates técnicos 21".
- **Dónde**: en la laptop, con el builder local. Brief: `docs/builds/brief-local-43.md`.
- **Validate**: `cargo test`:
  - Un stream falso de Anthropic con razonamiento que calla 121 segundos se corta, con el plazo inyectado.
  - Un stream falso de Anthropic que manda un `ping` cada 30 segundos durante 200 segundos no se corta.
  - Un requisito con `modulos` sobre un repo `shell` nombra los tests que no pasan, no los módulos.
  - `tests/test_seguro.rs` lanza `tools/test-seguro.sh` con `bash`, y los falsos con `sh`.

## Orden de trabajo (2026-11-02, después de la Tarea 42)

1. **Validate de 7 y 13. Arranca ya.** Con `r00t` release de 64479c4, las claves y el `ROOT_HOME` real. memora corre 3 veces más una, y brasa una vez por repo. Gasto estimado: 1 USD o menos. La Tarea 43 no cambia sus resultados: el Validate usa DeepSeek, y memora y brasa son Rust.
2. **Tarea 43 con el builder local.** En paralelo con el paso 1. Corre la suite con `ROOT_SIN_PODMAN=1`, así no choca con el Validate.
3. **Calibración real (Tarea 12).** Espera el paso 1. Con `--tope-usd 25` (decisión 48). Después, la comparación de `global` de la Tarea 13.
4. Siguen la imagen del builder, la credencial, `NETRC`, la corrida de memora, Colab, Graft con G5 y los paquetes a ciegas con G3.

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-02: Tarea 43, Validate de memora y contrato del juez

**Tarea 43.** El usuario escribió: "43 primero". El builder local la construyó en `laptop/tarea-43`, de 983bb08 a d084fbc, por 0,77 USD. El informe es `docs/builds/2026-11-02-build-local-43.md`, en esa rama.
- La suite dio 1991 verdes, 0 fallas y 2 ignorados, sin podman y con podman.
- La parte 1 borró `inactividad_de` y sus 4 tests. La parte 2 sumó `tiene_perfil` en `src/oracle/perfiles/mod.rs`. La parte 3 lanza `test-seguro.sh` con `bash`, y su test corrió 10 veces sin fallar.
- La rama no está mezclada. La mezcla espera la orden del usuario.

**Validate de 7 y 13, memora.** El orquestador usó `r00t` release de 3ea49f1, las claves y el `ROOT_HOME` real, con el oráculo FIFO arreglado y los `modulos` nuevos. memora gastó 0,35 USD. Las salidas están en `~/.cache/root-wt/validate14/`.

| Corrida | Decisión | Contrato | Oculto | Estructura | Arquitectura | Diseño | Inestable | Duración |
|---|---|---|---|---|---|---|---|---|
| ecc 1 | listo | 61/61 | 67/68 | 0,333 | sin veredicto (cobertura 5 de 10) | sin veredicto (7 de 13) | ninguno | 70 min |
| ecc 2 | listo | 61/61 | 66/68 | 0,333 | 0,381 | sin veredicto (6 de 13) | `test_client_id` | 25 min |
| ecc 3 | listo | 61/61 | 66/68 | 0,524 | 0,467 | 0,200 | `test_client_id` | 25 min |
| sdlc 1 | bloqueado (R1 y R13) | 21/61 | 24/68 | 0,000 | sin veredicto (3 de 10) | sin veredicto (8 de 13) | ninguno | 14 min |

- FIFO: R7 queda en nivel 1 en las 3 corridas de memora-ecc, y `test_blpop_fifo_entre_clientes` falla por el producto.
- Causa B: Estructura tiene valor en los 4 repos. Causa B': `Informe.confianza` lista cada ítem faltante con su motivo. Causa C: una dimensión sin cobertura queda sin veredicto y nombra "cobertura M de T". Causa D: los pasos nombran módulos, por ejemplo "revisar los módulos cmd, server y store (R7)".
- Funciona da 0,992, 0,984 y 0,984. Completitud da 0,964, 0,946 y 0,946. `global` sigue vacío, porque nada está calibrado.
- La corrida ecc 1 duró 70 minutos. La causa no está medida.

**Variación del juez.** Con el mismo commit, Arquitectura dio sin veredicto, 0,381 y 0,467, y Diseño dio sin veredicto o 0,200. El orquestador contó 10 ítems del juez en total: 6 de Diseño, 3 de Arquitectura y 1 de Estructura.

| Corrida | Ítems sin nivel, de 10 | Motivo |
|---|---|---|
| memora-ecc 1 | 6 | el juez no dio nivel |
| memora-ecc 2 | 5 | el juez no dio nivel |
| memora-ecc 3 | 1 | el juez no dio nivel |
| memora-sdlc 1 | 6 | 3 sin nivel y 3 "la evidencia no pasó el gate" |

- La causa está en `PROMPT_SISTEMA` de `src/oracle/juez.rs`: "Sin evidencia para un ítem, no lo incluyas en items." Omitir un ítem no le cuesta nada al juez, y cada corrida omite ítems distintos.
- El usuario preguntó: "El juez tiene un contrato de toml? quizas necesita una rubrica y contrato para ser mas robusto". El orquestador propuso ítems obligatorios y un reintento. El usuario eligió: "Sí, al planner" (decisión 57).

## Desempates técnicos 22 (Opus, 2026-11-02)

**Preguntas de la Tarea 43.**
1. **Dónde vive `tiene_perfil`.** Se confirma `src/oracle/perfiles/mod.rs`: el conocimiento de qué lenguaje tiene perfil vive con los perfiles.
2. **Informes fechados que citan `inactividad_de`.** Se confirma no corregirlos: un informe fechado registra lo que pasó en su fecha.

**Decisión 56 en R13 de memora-sdlc.**
- R13 lista `persist`, `store` y `db`. memora-sdlc tiene solo `db`, así que el paso dice "revisar el módulo db".
- **Desempate: no cambia.** La lista junta los módulos de las dos implementaciones. `persist` y `store` son nombres de memora-ecc, y memora-sdlc guarda su persistencia en `db`. Decir "faltan persist y store" en memora-sdlc nombraría módulos de otra implementación, que es lo que la decisión 56 quiso evitar.
- La capa faltante sí aparece por otro camino: el hallazgo de Arquitectura `c4835ab8c31a177e` la nombra, y entra en los riesgos del informe.

**Contrato del juez (decisión 57).**
- **Ítems obligatorios.** Root pasa al juez la lista exacta de ids que debe responder, en orden fijo. El juez responde cada id, una sola vez. Cada respuesta trae un nivel de 0 a 3 con su evidencia, o `"nivel": null` con `"motivo": "sin_evidencia"`.
- **Qué hace Root con "sin evidencia".** El ítem va a `faltantes` con el motivo "el juez no encontró evidencia". No vale 0. Razón: las anclas de nivel 0 piden una cita, y un 0 sin cita es un rojo falso (desempate 3 de "Desempates técnicos 4"). "Refuta por default" sigue valiendo: el nivel parte en 0 y solo sube con evidencia, pero un 0 también necesita la suya.
- **Respuesta que rompe el contrato.** Un id pedido que falta, repetido o sin nivel ni `sin_evidencia` rompe el contrato. Un id que no se pidió se ignora y deja un aviso.
- **Reintento.** Root hace un solo reintento, solo con los ids que faltaron y los que no pasaron el gate de evidencia. El prompt del reintento nombra, para cada id del gate, la cita que no se encontró y su `archivo:línea`.
  - Si el reintento tampoco responde un id, ese id va a `faltantes` con el motivo "el juez no respondió tras un reintento".
  - Razón de uno solo: el primer reintento ataca la causa medida (ítems omitidos). Más reintentos suben el costo sin una medición que los justifique. La calibración dirá si hace falta más.
- **Tope de costo.** El reintento pasa por el chequeo previo de tope de la Tarea 21, como toda llamada. Su `max_tokens` es el mismo del juez. El peor caso de una evaluación sube de una llamada por juicio a dos. Con `deepseek-v4-pro`, el peor caso del reintento es cerca de 0,11 USD. El tope de 25 USD de la calibración sigue rigiendo.
- **Tres muestras con mediana.** Quedan afuera. La calibración mide si la variación sigue después de este contrato.

### Tarea 44: Contrato del juez con ítems obligatorios y un reintento (decisión 57)
- **Action**: el diseño de "Contrato del juez" de arriba, en `src/oracle/juez.rs`. Cambia `PROMPT_SISTEMA`, el parser de la respuesta y el armado de `faltantes`.
- **Dónde**: en la laptop, con el builder local. Brief: `docs/builds/brief-local-44.md`. No usa podman ni claves.
- **Validate**: `cargo test`:
  - El prompt nombra la lista exacta de ids, en orden.
  - Una respuesta con un id en `"nivel": null` y `"motivo": "sin_evidencia"` deja ese id en `faltantes` con "el juez no encontró evidencia", y no hace reintento por él.
  - Una respuesta que omite un id hace un reintento solo con ese id. Si el reintento lo responde, el id tiene nivel.
  - Una evidencia que no pasa el gate hace un reintento con la cita y el `archivo:línea` que fallaron.
  - Si el reintento tampoco responde, el id queda en `faltantes` con "el juez no respondió tras un reintento".
  - Nunca hay más de un reintento por juicio.
  - Un id que no se pidió se ignora y deja un aviso.
  - El reintento pasa por el chequeo de tope: con el tope de API lleno, no se envía.

## Estado de los criterios (2026-11-02, memora)

- **Tarea 7, memora: cumple.** La decisión es la misma en las 3 corridas. Las corridas 2 y 3 dan los mismos niveles. Cada diferencia tiene su causa en `diferencias.md`.
- **Tarea 9c, memora: cumple.**
- **Tarea 13, memora: cumple en parte.** Las nueve dimensiones salen, y la decisión y las citas son correctas. Falta comparar `global`, que espera la calibración. Arquitectura y Diseño varían entre corridas por el juez: la Tarea 44 ataca esa causa.
- **Tarea 13b, memora: cumple.** La decisión nombra su regla, los pasos citan ids reales y nombran módulos.
- **Falta medir:** la parte de brasa; la variación del juez después de la Tarea 44, con dos corridas de memora-ecc; y la comparación de `global` después de la calibración.

## Orden de trabajo (2026-11-02, después de la Tarea 43)

1. **Parte de brasa del Validate de 7 y 13.** Ya corre.
2. **Tarea 44 con el builder local.** En paralelo con el paso 1, con `ROOT_SIN_PODMAN=1`. Brief: `docs/builds/brief-local-44.md`.
3. **Mezcla de las Tareas 43 y 44.** Espera la orden del usuario.
4. **Dos corridas de memora-ecc con el juez nuevo.** Gasto cerca de 0,2 USD. Miden si la variación de Arquitectura y Diseño baja.
5. **Calibración real (Tarea 12).** Con `--tope-usd 25`. Después, la comparación de `global` de la Tarea 13.
6. Siguen la imagen del builder, la credencial, `NETRC`, la corrida de memora, Colab, Graft con G5 y los paquetes a ciegas con G3.

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-02: brasa del Validate, Tarea 44 y su mezcla

**Validate de 7 y 13, brasa.** El orquestador la corrió sola, con `r00t` release de 3ea49f1, las claves y el `ROOT_HOME` real. El Validate entero duró de 15:03 a 18:27 y gastó 0,53 USD: memora 0,35 y brasa 0,18. Las salidas están en `~/.cache/root-wt/validate14/`.

| Repo | Decisión | Contrato | Oculto | Funciona | Completitud | Estructura | Arquitectura | Diseño | Ítems del juez sin nivel |
|---|---|---|---|---|---|---|---|---|---|
| brasa-ecc | listo | 53/53 | 20/21 | 0,986 | 0,938 | 0,190 | 0,567 | 0,333 | 0 |
| brasa-sdlc | bloqueado (R12) | 34/53 | 10/21 | 0,595 | 0,562 | 0,603 | 0,500 | 0,795 | 0 |

- Cumple: brasa-ecc da 20/21, y su único paso es "R3 ... revisar el módulo scene". Los pasos de brasa-sdlc nombran módulos, por ejemplo "R9 ... falta el módulo gltf_load".
- Cumple: el juez dio nivel a los 10 ítems en los dos repos, y ninguna dimensión quedó sin veredicto por cobertura.
- No cumple: el juez sale al revés en Estructura (0,190 contra 0,603) y en Diseño (0,333 contra 0,795).

**Tarea 44.** El usuario escribió: "Sí, mezclá". El builder local la construyó en `laptop/tarea-44`, de f782efc a cb8cd10, por 1,54 USD. El informe es `docs/builds/2026-11-02-build-local-44.md`.

| Parte | Commit |
|---|---|
| 1. Ids obligatorios en el prompt del juez | f782efc |
| 2. Lectura de la respuesta | 4befa9e |
| 3. Un reintento por juicio | 9289de5 |
| Informe | cb8cd10 |

- La suite dio 2011 verdes, 0 fallas y 2 ignorados, sin podman y con podman.
- No se escribió el test de integración del tope lleno con `JuezProveedor`. Un test del trait con un modelo falso cubre el caso.
- Si el reintento falla, Root deja solo "el juez no respondió tras un reintento", sin la causa.
- El builder ajustó 9 tests viejos: una respuesta que omite ítems ahora pide un segundo envío.

**Mezcla y medición.** El usuario escribió: "Sí, las dos cosas". La mezcla es 25084f1, sin conflictos, con el árbol igual a cb8cd10. Dos evaluaciones de memora-ecc corren con `r00t` release de 25084f1, en `~/.cache/root-wt/validate15/`.

## Desempates técnicos 23 (Opus, 2026-11-02): preguntas de la Tarea 44

1. **Gate que falla dos veces.** El motivo conserva la causa: "la evidencia no pasó el gate tras un reintento". Lo mismo vale para las otras causas: "el juez no dio un nivel válido tras un reintento" y "el juez no respondió tras un reintento". Razón: `confianza` debe decir por qué falta cada ítem, y la calibración necesita separar las causas.
2. **Nivel inválido.** Rompe el contrato y va al reintento. Un nivel mal tipado o fuera de 0 a 3 no es un nivel ni un `sin_evidencia`, y el contrato pide uno de los dos.
3. **Id repetido.** Se confirma: la repetición rompe el contrato, como dice la Tarea 44.
4. **Nivel y `sin_evidencia` juntos.** Se confirma que gana el nivel: su evidencia pasa por el gate igual, así que un nivel sin evidencia válida no sobrevive.
5. **Hallazgos del reintento.** Se confirma que se ignoran: el reintento pide solo ítems.
6. **Costo.** Se confirma. Para que la calibración pueda contar los reintentos, el evento de la llamada de reintento lleva `extra.reintento_juez = true`.
7. **Sección REINTENTO antes de EXTRACTOS.** Se confirma: queda junto a la lista de ids que el juez debe responder.
8. **Test del tope con clave falsa.** Se acepta y se escribe. El test usa un `ROOT_HOME` de prueba, una clave falsa y un servidor HTTP falso en `127.0.0.1`. Nunca llega a una API real, y cubre el camino de `JuezProveedor` con el tope lleno.

### Tarea 45: Motivos del reintento, nivel inválido y marca de reintento
- **Action**: los desempates 1, 2, 6 y 8 de arriba.
- **Dónde**: en la laptop, con el builder local. Brief: `docs/builds/brief-local-45.md`. Va antes de la calibración, porque cambia qué ítems del juez se reintentan.
- **Validate**: `cargo test`:
  - Una evidencia que falla el gate dos veces deja "la evidencia no pasó el gate tras un reintento".
  - Un nivel "alto" o 5 dispara el reintento. Si vuelve inválido, deja "el juez no dio un nivel válido tras un reintento".
  - La llamada de reintento registra `extra.reintento_juez = true` en su evento.
  - `JuezProveedor` con el tope de API lleno no envía el reintento. El test usa una clave falsa y un servidor falso en `127.0.0.1`.

## Criterio de la Tarea 44 y la inversión del juez

**Criterio de la Tarea 44.** Antes del contrato, memora-ecc dejó 6, 5 y 1 de 10 ítems del juez sin nivel. La Tarea 44 cumple si, en cada una de las dos evaluaciones de memora-ecc con 25084f1:
- quedan como máximo 2 de 10 ítems sin nivel, por cualquier motivo, `sin_evidencia` incluido; y
- Arquitectura y Diseño tienen veredicto en las dos evaluaciones.

Razón: con 2 ítems de 10 sin nivel, las dos dimensiones conservan la cobertura de dos tercios, y su valor deja de saltar entre "sin veredicto" y un número. Si no cumple, el planner revisa los motivos de cada faltante antes de la calibración.

**Inversión de Estructura y Diseño en brasa.**
- brasa-sdlc tiene menos código y recibe más puntaje del juez que brasa-ecc. El juez puntúa la calidad del código que ve, no lo que falta. Lo que falta lo mide Completitud.
- **Desempate: no se cambia antes de la calibración.** La calibración mide si el juez detecta defectos inyectados. La Tarea 12b mide si Root ordena bien los pares conocidos. Si el juez falla esas mediciones, sus dimensiones no entran a `global`, que es la defensa del GOAL para este caso.
- Las salidas de brasa del Validate 14 quedan como caso de referencia para la Tarea 12b.

## Orden de trabajo (2026-11-02, después de la Tarea 44)

1. **Dos evaluaciones de memora-ecc con 25084f1.** Ya corren. Dan el criterio de la Tarea 44.
2. **Tarea 45 con el builder local.** En paralelo con el paso 1, con `ROOT_SIN_PODMAN=1`, sin commits hasta que terminen las evaluaciones.
3. **Mezcla de la Tarea 45.** Espera la orden del usuario.
4. **Calibración real (Tarea 12).** Con `--tope-usd 25`, sobre la mezcla de la 45. Después, la comparación de `global` de la Tarea 13.
5. Siguen la imagen del builder, la credencial, `NETRC`, la corrida de memora, Colab, Graft con G5 y los paquetes a ciegas con G3.

No hay preguntas nuevas para el usuario.

## Decisión 58 (2026-11-02): alcance del cierre del hito 1

El usuario preguntó: "Que le falta a root? terminemos". El orquestador listó 10 pasos y propuso tres alcances. El usuario respondió: "P60 c". El orquestador había avisado que la opción c cambia las decisiones 12, 36 y 38.

**Primera evaluación del juez nuevo.** memora-ecc #1 con `r00t` release de 25084f1 tardó 24 minutos y gastó 0,107 USD. Dio `listo`, contrato 61/61 y oculto 66/68. Quedaron 0 de 10 ítems del juez sin nivel. Arquitectura dio 0,567, Diseño 0,179 y Estructura 0,429. Solo Eficiencia quedó sin veredicto: "la tarea no define [eficiencia]". La segunda evaluación sigue corriendo.

**Decisiones que la 58 acota.** Las decisiones originales quedan en la tabla. Su parte humana pasa al hito 1b:
- **Decisión 12 (P2).** La calificación a ciegas del usuario (G3) pasa al hito 1b. P1, P3 y P4 siguen en el hito 1: rúbricas, requisitos aprobados y recall de 0,8 o más.
- **Decisión 13 (P5).** El kappa ponderado de 0,6 o más por dimensión y la misma decisión en 5 de 6 paquetes pasan al hito 1b, porque los mide G3.
- **Decisión 36 (P39 c y P44 c).** memora y brasa siguen calibrando el evaluador en el hito 1. G4 (si memora sigue después de G3) pasa al hito 1b. La corrida de `graft-lector-planes` de la Tarea 16 pasa al hito 1b.
- **Decisión 38 (P49 b).** En el hito 1, la Tarea 16 corre solo memora. La corrida de Graft pasa al hito 1b.

**Tarea 12b en el hito 1 (validez automática).**
- Mide dos fuentes externas a Root: el orden conocido y los defectos reales conocidos.
- `--preparar-ciego` sigue existiendo, pero no se usa en el hito 1.
- En `validez.json` queda **sin medir** la métrica de acuerdo con el usuario: el kappa ponderado por ítem, el acuerdo en la decisión y el Spearman del orden de los 6 paquetes. La Tarea 12b ya pide "sin medir" para una fuente que falta, nunca 0 ni 1.
- La exactitud de orden y el recall de defectos reales se miden en el hito 1.

**Criterios de aceptación que pasan al hito 1b.**
- "`validez.json` informa el acuerdo con la calificación a ciegas del usuario. Kappa ponderado ≥ 0,6 por dimensión y la misma decisión en 5 de los 6 paquetes (decisión 13)."
- La corrida real de Colab de la Tarea 7h. En el hito 1, el test de GPU de brasa sigue `Pendiente` con su motivo, y el criterio "cada test `Pendiente` por entorno figura en el informe con su motivo" ya se cumple.
- La corrida de `graft-lector-planes` con G5.

## Hito 1b (después del cierre del hito 1)

- **G3.** La laptop arma los 6 paquetes a ciegas con `--preparar-ciego`. Opus lee `validez/rastros.json` antes de entregarlos. El usuario califica. `validez.json` suma la métrica de acuerdo.
- **G4.** Después de G3, el usuario decide si memora sigue como banco (P44 c).
- **Colab (Tarea 7h).** La corrida real sobre `test_rendimiento[gpu-10000-res1]`, con T4, `unshare -rn` y tope de 2 horas.
- **Graft (Tareas 16b y 16).** Opus escribe el oráculo de `graft-lector-planes`. El usuario aprueba G5. Después corre `r00t corre --tarea graft-lector-planes`.

## Orden de trabajo final del hito 1 (2026-11-02)

1. **Segunda evaluación de memora-ecc con 25084f1.** Ya corre. Con la primera, decide el criterio de la Tarea 44 de "Criterio de la Tarea 44 y la inversión del juez". La corre el orquestador. No es una tarea nueva.
2. **Tarea 45.** La construye el builder local, con el brief `docs/builds/brief-local-45.md`. Después la mezcla, con la orden del usuario.
3. **Calibración real (Tarea 12).** La corre el orquestador en la laptop, con `--tope-usd 25`, sobre memora y brasa, con `n = 20` (decisiones 48 y 57). Después compara `global` de memora-ecc contra memora-sdlc, y de brasa-ecc contra brasa-sdlc: es la segunda parte del criterio de la Tarea 13. No necesita brief. Si la calibración deja preguntas, vuelven al planner.
4. **Imagen del builder, credencial y `NETRC`.** El orquestador construye `root-builder:1`, prueba la credencial de Claude en solo lectura y corre `NETRC` con un índice privado de prueba (Tareas 26 y 30). No es una tarea nueva. Si aparece un defecto, el planner escribe una tarea chica para el builder local, con su brief.
5. **Corrida real de memora (Tarea 16).** Espera el paso 4. El orquestador corre `r00t corre --tarea memora --config ecc-haiku`. Gasta cupo: una construcción completa con Haiku (decisión 38, acotada por la 58). La fila de `runs.jsonl` debe traer el veredicto de nueve dimensiones, el `Informe` y el resumen.
6. **Casos de validez (Tarea 46, nueva).** Opus arma `$ROOT_HOME/validez/` con dos fuentes:
   - orden conocido: memora-ecc sobre memora-sdlc, brasa-ecc sobre brasa-sdlc, y commits viejos contra commits nuevos del mismo repo;
   - defectos reales: los 4 de la revisión de Opus y los 3 del code-review de ECC en F1 (`docs/contexto/veredicto-ecc-f1.md`), más los defectos registrados en los logs del banco, cada uno con su `archivo:línea`.
   La hace Opus en una sesión propia, porque elegir los casos es juicio. No necesita brief de builder: los casos viven en `ROOT_HOME`, fuera del repo.
7. **Validez automática.** El orquestador corre `r00t oraculo validar --casos $ROOT_HOME/validez/`. Gasta API: una evaluación por caso. `validez.json` debe dar la exactitud de orden y el recall, con el acuerdo con el usuario "sin medir". El criterio es la exactitud de orden de 1,0 (desempate 44 de "Desempates técnicos 11") y un recall de 0,8 o más (decisión 12).
8. **Cierre (Tarea 47, nueva).**
   - El orquestador corre `/ecc:code-review` sobre la rama `claude/root-e7813e`, contra el commit base del hito.
   - Opus verifica cada hallazgo crítico y alto en el código, como hizo con la revisión de nube. Los confirmados van a una tarea del builder local, con su brief.
   - El orquestador corre los comandos de la sección "Validation" del plan. `r00t aprende --replay` tiene una sola fila real, así que su métrica queda "sin datos suficientes".
   - Opus escribe la nota final en `docs/cierre-hito-1.md`: cada criterio de "Acceptance" con su estado, su evidencia y su commit, y la lista del hito 1b.
   - El usuario aprueba el cierre.

**Tareas nuevas.**
- **Tarea 46: casos de validez.** La corre Opus. No necesita brief: es trabajo de juicio sobre datos del banco, y su resultado vive en `ROOT_HOME`.
- **Tarea 47: cierre.** La corren el orquestador (code review y comandos) y Opus (verificación y nota final). Los arreglos que salgan del code review van al builder local, con un brief por tarea.

**PRD.** "Delivery Milestones" no necesita un cambio para cerrar el hito 1. El "Outcome" del hito 1 sigue siendo verdad: "Route, Oracle, Observe y Track corren desde el bootstrap del banco". Sumar una fila para el hito 1b cambiaría un documento que el usuario revisa a mano, así que va como P61.

**Preguntas abiertas (sin responder)**

- **P61. ¿El hito 1b entra al PRD?** El hito 1b junta G3, G4, Colab y Graft temprano. Hoy vive solo en el plan.

  - a) Sumar una fila "1b" en "Delivery Milestones" del PRD, con su outcome: "El juicio de Root coincide con el del usuario, y Graft corre su primera tarea medida".
  - b) No tocar el PRD. El hito 1b vive en el plan del hito 1 y en la nota de cierre.

  Mi recomendación es b): el hito 1b completa el hito 1 y no es un resultado de negocio nuevo. El PRD queda como lo dejaste en P36.

## Vuelta del 2026-11-02 (noche): P61, Tarea 45, juez nuevo y calibración

**P61.** El usuario respondió: "P61 b". El PRD no cambia. El hito 1b vive en el plan y en la nota de cierre.

**Tarea 45.** El builder local la construyó en `laptop/tarea-45`, de fb18062 a e51d488, por 1,30 USD. El informe es `docs/builds/2026-11-02-build-local-45.md`. La suite dio 2020 verdes, 0 fallas y 2 ignorados, sin podman y con podman. El usuario escribió: "Sí, las dos". La mezcla es 4070f99.

**Criterio de la Tarea 44: cumple.**

| Evaluación | Oculto | Ítems del juez sin nivel | Estructura | Arquitectura | Diseño |
|---|---|---|---|---|---|
| memora-ecc 1 | 66/68 | 0 de 10 | 0,429 | 0,567 | 0,179 |
| memora-ecc 2 | 66/68 | 0 de 10 | 0,429 | 0,467 | 0,385 |

- Las dos evaluaciones dejan 0 ítems sin nivel, y Arquitectura y Diseño tienen veredicto. Gastaron 0,22 USD.
- Los niveles del juez todavía varían entre corridas. La calibración mide si esa variación le quita exactitud a la dimensión.

**Calibración real, en curso.**
- Arrancó a las 22:55 con `r00t` release de 4070f99: 8 dimensiones por tarea, `n = 20`, semilla 42 y repo bueno `*-ecc`.
- `[calibracion] repo` no existe en ningún `tarea.toml`, así que el orquestador pasa `--repo`.
- El usuario preguntó: "No podemos acelerar estos procesos?". Eligió las dos opciones del orquestador: "También brasa ya / Y lo seguro". memora y brasa calibran en paralelo.
- El tope de 25 USD vale para las dos. Cada comando recibe la mitad del saldo, medido en `eventos.jsonl`.
- La Tarea 46 corre en paralelo en la sesión de fichas, sin podman ni API.

## Desempates técnicos 24 (Opus, 2026-11-02)

**Preguntas de la Tarea 45.**
1. **Nivel inválido y después omisión.** Se confirma: el motivo describe el último intento, porque ese intento decide que el ítem quedó sin nivel.
2. **Nivel float.** Un `2.0` vale como 2. Un `2.5` sigue siendo inválido. El contrato pide un nivel entero de 0 a 3, y `2.0` es ese valor con otro tipo de JSON. Rechazarlo cuesta un reintento sin ganar nada.
3. **Precedencia de motivos.** Se confirma el orden gate, nivel inválido, sin respuesta. Hoy un ítem tiene un solo motivo.
4. **Tope que frena el reintento.** Se suma el motivo "reintento no enviado por el tope de API". Razón: separa un límite del presupuesto de una falla del modelo, y la calibración debe poder distinguirlos.
5. **Regla del tope.** Se confirma el peor caso de la Tarea 21: el tope debe frenar antes de gastar, y el costo real recién se sabe después.
6. **Evento `Gate` del tope.** Se confirma leer la causa en `fase = "tope"`.

Los desempates 2 y 4 cambian código. Esperan al cierre: van en el lote de arreglos de la Tarea 47, junto con los hallazgos del code review. Ninguno cambia la medición en curso: el 2 solo ahorra reintentos, y el 4 solo cambia un texto.

**Calibración con carga de CPU.**
- memora y brasa calibran en paralelo. La carga puede hacer fallar un test de tiempo, cortar a `cargo-mutants` por su plazo o dar un error de infraestructura. Esas fallas no vienen del defecto inyectado.
- **Qué dimensiones toca.** Seguridad, Estructura, Arquitectura, Diseño y Entrega salen de sondas estáticas y del juez: la carga no cambia su resultado. Funciona, Completitud y Pruebas corren tests o mutantes: la carga sí puede cambiarlos.
- **Desempate.** Al terminar, el orquestador lista los casos de esas tres dimensiones que dieron `SinVeredicto`, un error de infraestructura, un timeout de test o un reporte de mutantes truncado. Cada uno se repite una vez, solo, sin la otra calibración en paralelo. La fila final usa el resultado de la repetición.
- Si la repetición vuelve a dar sin veredicto, el caso cuenta en contra, como dice el desempate 41 de "Desempates técnicos 11".
- `calibracion.json` marca cada caso repetido con `repetido_por_entorno = true`, y el informe de la calibración dice cuántos fueron.
- Razón: la calibración mide al evaluador, no a la laptop. Repetir solo los casos que fallaron por entorno separa las dos causas sin repetir toda la corrida.
- Si la repetición necesita código, es una tarea chica para el builder local. Si alcanza con correr `r00t oraculo calibrar` con menos casos, la hace el orquestador.

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-03: validez, calibración cortada y su rediseño

**Respuestas del usuario (decisiones 59 a 63).** P-val1 a, P-val2 a, G4 respondido, Tarea 16 sobre memora y calibración rediseñada. G4 sale del hito 1b: memora y brasa se borran después del hito 1.

**Tarea 46.** La sesión de fichas armó `$ROOT_HOME/validez/` por 9,69 USD: 6 pares de orden, 8 defectos reales y 9 clones. Los dos TOML parsean. Los 7 defectos de F1 quedaron afuera, porque sus repos se borraron. Con 8 defectos, el recall de 0,8 exige encontrar al menos 7.

**Calibración real, cortada.** Corrió de las 22:55 a las 13:54, en paralelo, con 4070f99. Gastó 4,19 USD del tope de 25. Los JSON están en `~/.cache/root-wt/calibra1/`, y las filas en `$ROOT_HOME/calibracion.json`.

| Dimensión | memora | brasa |
|---|---|---|
| Completitud | 0,85 | 1,0 |
| Entrega | 1,0 | sin valor (n = 10) |
| Pruebas | 0,5 (sensibilidad 0, especificidad 1) | 0,5 (sensibilidad 0, especificidad 1) |
| Seguridad | 0,8 | 0,8 |
| Estructura | 0,5 (sensibilidad 0, especificidad 1, `items` vacío) | cortada |
| Eficiencia | error: "la línea base no tiene veredicto de eficiencia" | sin correr |
| Arquitectura y Diseño | sin correr | sin correr |

- El JSON no guarda los casos, así que no se ve por qué un defecto no se detectó.
- Pruebas tardó más de 6 horas por tarea, y Estructura de memora 6,6 horas.
- El orquestador encontró cuatro causas de lentitud: cada caso corre todas las sondas, también la mutación; la mutación corre hasta 20 minutos y Root mide solo 60 mutantes; los casos limpios repiten todas sus sondas; y los casos corren en serie aunque el juez espera a la red.

## Diagnóstico de la calibración (Opus, 2026-11-03, sobre el código de 4070f99)

**Estructura, sensibilidad 0: el defecto rompe la compilación.**
- `dios` (`src/oracle/calibra/inyectar.rs`) copia el contenido de otros módulos dentro de `pub mod concat_<x> { ... }`, en otro archivo. No borra los originales.
- Dentro del módulo nuevo, las rutas `super::` y `crate::` del contenido copiado cambian de sentido. El repo deja de compilar.
- clippy da `SinVeredicto` ante una construcción fallida (desempate M14). Sin la parte de clippy, Estructura queda sin veredicto.
- `detecta_dimension` cuenta un defecto sin veredicto como falso negativo. Por eso todos los defectos dieron falso negativo. Los casos limpios sí compilan, y por eso la especificidad es 1.
- Es la causa más probable. Sin los casos guardados no se puede confirmar caso por caso. La Tarea 48 guarda los casos.

**Pruebas, sensibilidad 0: el defecto casi nunca toca la muestra.**
- `assert_borrado` borra un assert al azar de `tests/` o de un módulo de tests.
- `cargo-mutants` mide solo los primeros 60 mutantes (`mutantes_max`). Un assert al azar casi nunca es el único que mata un mutante de esa muestra.
- Entonces la proporción de mutantes muertos no cambia, y el defecto no se detecta.

**Eficiencia de memora: no es un error, es un "no aplica".**
- Los `tarea.toml` de memora y brasa no definen `[eficiencia]`. La línea base no puede medir Eficiencia, y ningún defecto tiene contra qué compararse.
- memora y brasa se borran después del hito 1 (decisión 61). No se suma un `[eficiencia]` para ellas.
- La calibración debe escribir una fila "no aplica: la tarea no define [eficiencia]", no un error. Eficiencia queda fuera de `global`.

**brasa, Entrega sin valor con n = 10.** Lo más probable es que ningún defecto de Entrega aplique a brasa. Con los casos guardados, la Tarea 48 lo confirma.

## Desempates técnicos 25 (Opus, 2026-11-03): mejoras de la calibración

La regla para elegir: una mejora entra solo si el caso sigue midiendo lo mismo que una evaluación real.

1. **Sondas ajenas desde la base: entra en parte.** Un caso de Estructura, Arquitectura, Diseño, Seguridad, Completitud o Entrega toma de la base solo la mutación. Las demás sondas corren otra vez. Razón: la mutación es el costo mayor, y el defecto de esas dimensiones no cambia la muestra de mutantes. Las sondas estáticas son baratas, y el juez las lee: correrlas otra vez mantiene su entrada igual a la de una evaluación real.
2. **Casos limpios livianos: entra.** Un caso limpio es una copia idéntica de la base. Toma de la base las salidas de las sondas deterministas: estáticas, clippy, gitleaks, audit, hadolint y mutación. Corre otra vez lo no determinista: el juez y los tests. Razón: una sonda determinista con la misma entrada da la misma salida, y correrla otra vez no mide nada. La especificidad mide los falsos positivos de lo que sí varía.
3. **Juez en paralelo: entra, con límite.** Las llamadas al juez de varios casos corren de a 4 a la vez. Los contenedores siguen en serie. Razón: el juez espera a la red y no carga la CPU. La reserva con lock de la Tarea 21 ya protege el tope ante llamadas paralelas.
4. **Mutación acotada: entra.** `cargo-mutants` corre solo la muestra que Root mide, en la evaluación real y en la calibración. Razón: hoy corre hasta 20 minutos y Root descarta lo que pasa de 60 mutantes. La medición no cambia.
5. **`AssertBorrado` dirigido: entra.** El defecto elige un mutante de la muestra que la base mata, y borra los asserts del test que lo cubre. Razón: así el defecto toca lo que Pruebas mide, como un PM borraría un test que importa.
6. **Calibración por versión del oráculo: no entra.** El usuario eligió que una calibración vale solo para su tarea (decisión 48, P57 a). Las filas viejas quedan en `calibracion.json` como historia y no se borran.

**Arreglos que pide el diagnóstico.**
7. **`ArchivoDios` compilable.** El defecto agranda un módulo sin romper la compilación: le suma funciones nuevas y largas, válidas y sin uso, hasta pasar el límite de tamaño de archivo y de función. Razón: el defecto debe medir Estructura, no la compilación.
8. **Inyección inválida.** Si un defecto de una dimensión que no es Completitud ni Funciona rompe la compilación, el caso es una "inyección inválida". Root lo reemplaza por el siguiente candidato del mismo defecto, y la fila cuenta los reemplazos. Razón: un defecto que no compila mide otra cosa.
9. **Casos guardados.** Cada calibración escribe sus casos en `$ROOT_HOME/calibracion/<tarea>-<dimensión>-<ts>.casos.jsonl`: defecto, archivo y línea, detectado, motivo si quedó sin veredicto, y si se repitió por entorno. Razón: sin los casos no se puede diagnosticar una sensibilidad baja.
10. **Eficiencia sin `[eficiencia]`.** La calibración escribe una fila "no aplica", sin error.

**Qué se vuelve a calibrar.** Todas las dimensiones, con el diseño nuevo y una sola versión del código. Razones:
- Pruebas y Estructura cambian de inyector.
- Completitud de memora (0,85) y Seguridad (0,8) quedaron bajo 0,9. Sin los casos guardados no se sabe por qué, y la nueva corrida los guarda.
- Una sola versión del código para todas las filas hace comparables las dimensiones de `global`.
- El gasto estimado es de 6 a 8 USD, dentro de los 20,81 USD que quedan del tope de 25. La estimación sale del peor caso del juez y no está medida.

### Tarea 48: Calibración que mide bien (diagnóstico)
- **Action**: los desempates 5, 7, 8, 9 y 10.
- **Dónde**: en la laptop, con el builder local. Brief: `docs/builds/brief-local-48.md`.
- **Validate**: `cargo test`:
  - Un test por defecto sobre un fixture chico: cada uno de los 10 defectos se inyecta y su dimensión lo detecta. Los defectos estáticos usan las sondas reales. Los que necesitan una herramienta usan su salida real con podman, o un falso que refleja el defecto si la herramienta no está en la imagen.
  - `ArchivoDios` sobre un fixture que compila deja un repo que compila, y da un hallazgo de archivo grande.
  - `AssertBorrado` dirigido sobre un fixture con mutantes deja vivo un mutante que la base mataba.
  - Un defecto que rompe la compilación en Estructura se reemplaza, y la fila cuenta el reemplazo.
  - La calibración escribe su archivo `.casos.jsonl` con un renglón por caso.
  - Eficiencia sobre una tarea sin `[eficiencia]` escribe una fila "no aplica" y sale con 0.

### Tarea 49: Calibración más rápida
- **Action**: los desempates 1, 2, 3 y 4.
- **Dónde**: en la laptop, con el builder local, después de la Tarea 48. El mismo brief la cubre, como parte 2.
- **Validate**: `cargo test`:
  - Un caso de Diseño toma la mutación de la base y corre otra vez las sondas estáticas y el juez.
  - Un caso limpio toma de la base las sondas deterministas y corre otra vez el juez y los tests.
  - Con 8 casos de Diseño, nunca hay más de 4 llamadas al juez a la vez, y el tope no se pasa con 4 llamadas en paralelo.
  - `cargo-mutants` recibe solo la muestra de `mutantes_max`, en la evaluación y en la calibración.
  - Una evaluación real da el mismo veredicto de Pruebas antes y después del cambio, sobre un fixture con podman.

## Orden final del hito 1 (2026-11-03)

1. **Tareas 48 y 49 con el builder local.** Brief: `docs/builds/brief-local-48.md`. La 48 va primero. Podman está libre, porque la calibración se cortó.
2. **Mezcla de las Tareas 48 y 49.** Espera la orden del usuario.
3. **Calibración real, otra vez.** Las 8 dimensiones de memora y brasa, con `n = 20`, semilla 42, `--repo` y `--tope-usd` igual al saldo del tope de 25 USD. Corre una tarea por vez, para que la carga no ensucie la medición.
4. **Comparación de `global`.** Segunda parte del criterio de la Tarea 13.
5. **Imagen del builder, credencial y `NETRC`** (Tareas 26 y 30).
6. **Corrida real de memora** (Tarea 16, decisión 62).
7. **Validez automática** (Tarea 12b) sobre `$ROOT_HOME/validez/`: exactitud de orden de 1,0 y recall de 0,8 o más, con 8 defectos (decisión 59) y 6 pares (decisión 60).
8. **Cierre** (Tarea 47): `/ecc:code-review`, verificación de Opus, arreglos con el builder local (incluye los desempates 2 y 4 de "Desempates técnicos 24"), comandos de "Validation", nota final en `docs/cierre-hito-1.md` y aprobación del usuario.
9. **Después del cierre:** memora y brasa se borran (decisión 61).

**Hito 1b (actualizado).** G3, Colab (Tarea 7h) y Graft (Tareas 16b y 16 con G5). G4 sale del hito 1b, porque la decisión 61 lo respondió.

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-03 (tarde): objetivo del usuario, imagen del builder y huecos de la Tarea 16

**Objetivo (decisión 64).** El usuario escribió: "terminar root. Calibrarlo a la perfeccion, no terminar con la primer version completa, seguir hasta terminar". Después sumó: "Acelerar los procesos para que no sea todo secuencial, un recurso que hay que aprovechar es el tiempo." El orquestador corre ahora en paralelo cuatro trabajos: el builder de las Tareas 48 y 49, el `/ecc:code-review` del hito, la prueba de `NETRC` y la imagen del builder.

**Imagen del builder (Tarea 26, capa 1): hecha.**
- `scripts/construir-imagen-builder.sh` construyó `root-builder:1` con `CLAUDE_CODE_VERSION=2.1.317`. La imagen trae Node.js 20.19.2 y Claude Code 2.1.317.
- Con `--userns=keep-id`, el usuario es `builder`, con uid 1000.
- El builder no ve `ROOT_HOME`: `ls <ROOT_HOME>/oraculos` da "No such file or directory".
- Con `pasta:--no-map-gw`, el builder no alcanza un puerto de 127.0.0.1 del host ni `host.containers.internal`. Sí sale a internet.

**Credencial en solo lectura: hecha en parte.**
- `claude -p` con Haiku corrió dentro de la imagen, con los montajes de `args_podman_builder`. Respondió "listo" por 0,0021 USD de cupo.
- El montaje deja un `.credentials.json` vacío en el estado, con uid 524288.
- La renovación del login no se probó: el token vencía en 283 minutos.
- **Hallazgo:** el archivo montado trae también `mcpOAuth`, con los tokens de MCP del usuario.

**Defecto que bloquea la Tarea 16.**
- `args_claude` (`src/corre/args.rs`) no pasa `--permission-mode`, y el bootstrap de memora no trae `.claude/settings.json`.
- El orquestador lo probó con la imagen real y los mismos argumentos: Haiku no pudo crear `hola.txt`. Hubo 2 denegaciones, de `Bash` y de `Write`.
- Una corrida real no construiría nada. El test mecánico de la Tarea 16 no lo ve, porque usa un proveedor falso.

**Otros huecos de la Tarea 16.**
- `$ROOT_HOME/configs/ecc-haiku.toml` no existe, así que `r00t corre --config ecc-haiku` falla por uso.
- La imagen no trae el plugin de ECC, y `args_claude` no pasa `--plugin-dir`. El nombre `ecc-haiku` promete ECC.
- El banco construyó memora-ecc con `~/Proyectos/bench-ecc-sdlc/correr-ecc-rol.sh`. Ese script usa `.ecc-plugin` para Opus y Sonnet, y `.ecc-plugin-haiku` para la sesión que organiza. También fija `CLAUDE_CODE_SUBAGENT_MODEL=haiku`, `ECC_AGENT_DATA_HOME`, `ECC_DISABLED_HOOKS`, `--permission-mode auto` y un plazo de 5400 segundos.

## Desempates técnicos 26 (Opus, 2026-11-03): el builder dentro del contenedor

**Modo de permisos: `auto`, con las reglas de negación de siempre.**
- `args_claude` pasa `--permission-mode` con el valor de la config, `auto` por defecto.
- Razón: es el modo con que el banco construyó memora-ecc, así que la corrida mide en las mismas condiciones. Un modo que salta todo control sería una diferencia más contra el banco.
- `--disallowedTools` sigue con sus reglas sobre `oraculos/`: las reglas de negación valen en todo modo de permisos. La primera defensa sigue siendo el contenedor, que no monta `ROOT_HOME` (decisión 43).

**ECC entra como en el banco: plugin montado por rol.**
- La config de la corrida suma, por rol, el plugin de ECC y el modelo de los subagentes. El runner monta cada plugin en solo lectura en `/opt/plugin` y pasa `--plugin-dir /opt/plugin`.
- El runner fija `ECC_AGENT_DATA_HOME` en la carpeta de estado de la corrida, no en la del banco, y `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0`.
- El nombre `ecc-haiku` queda: la config cumple lo que su nombre promete.
- Razón: la decisión 38 pide medir memora con ECC y Haiku, como el banco. Cambiar de nombre y correr sin ECC mediría otra cosa.

**`mcpOAuth` no entra al contenedor.**
- El runner no monta el archivo de credenciales del usuario. Arma, para cada corrida, una copia que trae solo `claudeAiOauth`, con permisos 0600, en la carpeta de estado. La monta en solo lectura y la borra al terminar, por `src/borrado.rs`.
- Razón: el builder corre código con red, y los tokens de MCP del usuario no le hacen falta. Las claves son zona roja.
- **Vencimiento.** Antes de lanzar, el runner lee el vencimiento de `claudeAiOauth`. Si el token vence antes del plazo total de la corrida, sale con código 3 y pide renovar el login en el host. Razón: la copia es de solo lectura, así que el contenedor nunca renueva ni rota el token del usuario.

### Tarea 50: Builder que construye dentro del contenedor
- **Action**: los tres desempates de arriba, en `src/corre/`. No toca `src/oracle/`, así que corre en paralelo con las Tareas 48 y 49.
- **Dónde**: en la laptop, con el builder local, en otro worktree. Brief: `docs/builds/brief-local-50.md`. La config: `docs/builds/config-ecc-haiku.toml`, que el orquestador copia a `$ROOT_HOME/configs/ecc-haiku.toml`.
- **Validate**: `cargo test`:
  - `args_claude` pasa `--permission-mode auto` por defecto, y el valor de la config si la config lo trae.
  - Con plugin en la config, los argumentos montan el plugin en solo lectura y pasan `--plugin-dir /opt/plugin`.
  - El entorno del builder trae `CLAUDE_CODE_SUBAGENT_MODEL`, `ECC_DISABLED_HOOKS` y `ECC_AGENT_DATA_HOME` dentro de la carpeta de estado.
  - La credencial montada no trae `mcpOAuth`. La copia tiene permisos 0600 y se borra al terminar.
  - Un token que vence antes del plazo de la corrida da código 3 sin lanzar nada.
  - `docs/builds/config-ecc-haiku.toml` carga con `ConfigCorrida::desde_texto`.
  - **Con podman y la imagen real, sin `claude`:** un comando de shell dentro del contenedor, con los argumentos de la corrida, escribe un archivo en el repo de la corrida y no puede listar `<ROOT_HOME>/oraculos`.
  - **Con podman, la imagen real y `claude`:** un test con `#[ignore]` lanza `claude -p` con Haiku y le pide crear `hola.txt` y leer un archivo de `oraculos/`. `hola.txt` existe y la lectura falla. Solo corre con `ROOT_TEST_BUILDER_REAL=1`, porque gasta cupo. Lo corre el orquestador.

## Orden de trabajo (2026-11-03, en paralelo)

En paralelo ahora, en worktrees distintos:
- **A. Tareas 48 y 49.** El builder local, en `laptop/tarea-48`.
- **B. Tarea 50.** El builder local, en otro worktree, con `docs/builds/brief-local-50.md`.
- **C. `/ecc:code-review` del hito.** El orquestador. Sus hallazgos esperan la verificación de Opus.
- **D. `NETRC` con un índice privado de prueba.** El orquestador.

En serie, después:
1. Mezcla de A y B, con la orden del usuario. Cada mezcla corre la suite con podman, una por vez.
2. Calibración real otra vez, una tarea por vez (necesita A).
3. Corrida real de memora (Tarea 16, decisión 62). Necesita B, la config copiada a `ROOT_HOME` y el test real de B en verde.
4. Comparación de `global`, validez automática y cierre (Tarea 47), como en "Orden final del hito 1 (2026-11-03)".

No hay preguntas nuevas para el usuario.

## Code review del hito (Tarea 47, adelantado, 2026-11-03)

El orquestador adelantó el `/ecc:code-review` para aprovechar el tiempo (decisión 64). Corrió con Opus en `~/.cache/root-wt/review47`: la base 1862c1e con el código de fedcf0b. Costó 7,18 USD. Ocho revisores Sonnet leyeron por grupos, y nadie corrió cargo, tests, podman ni APIs. El informe es `~/.cache/root-wt/orquestador/review-47-informe.md`. Conteo: 2 CRITICAL, 5 HIGH, 22 MEDIUM y 14 LOW. Ese worktree no tenía `tests/`, `GOAL.md` ni `.gitignore`: esos faltantes son del worktree de revisión, no del repo.

**Verificación de Opus (sobre 7c1f48b, por función).**

| Id | Veredicto | Dónde y por qué |
|---|---|---|
| C1 | Real | `commit_final` (`src/corre/corrida.rs`) corre `git status --porcelain` y `git add -A` sobre el repo que el builder escribió en `/trabajo` con `.git/` incluido (`src/corre/contenedor.rs`, montaje `rw,Z`). Solo el `commit` lleva `core.hooksPath=/dev/null`. Un `core.fsmonitor` o un filtro en `.git/config` corre en el host con el usuario real. |
| C2 | Real en el código de Root | `clasificar_config` (`src/oracle/descarga/inventario.rs`) registra `credential-provider` solo como hallazgo. `cargo vendor` (`COMANDO_VENDOR`, `src/oracle/descarga.rs`) corre con red y con `secretos_de(credenciales)` en el entorno. Que cargo ejecute ese proveedor sale de su documentación, sin prueba. |
| H1 | Real | En la suite oculta, el producto corre en el mismo contenedor que `/oraculo` (solo lectura) y `/salida` (escritura). Un producto puede leer los tests ocultos mientras corre. Un producto que corre dentro del proceso de pytest puede además reescribir el junit después de pytest. |
| H2 | Real | `commit_de` (`src/oracle/mod.rs`) da `git rev-parse --short HEAD`, o vacío sin git. `pendientes.rs` usa ese valor como identidad de versión, así que un cambio sin commit no cambia la identidad. |
| H3 | Real en el código de Root | `requisito_de_build_aceptable` (`src/oracle/descarga/python.rs`) rechaza solo `://`, `@` y un `-` inicial. Una línea `foo==1.0 --no-binary :all:` pasa. |
| H4 | Real en el código de Root | `entrega.rs` mapea el `STEP k/n` de buildah con `instrucciones_df.get(paso.saturating_sub(1))`, sobre la lista global del Dockerfile. En un Dockerfile multietapa, el índice apunta a otra etapa. |
| H5 | Real en el código de Root | Después de bajar un sdist, `pendientes.remove(pos)` lo saca del índice de pip, y nadie baja sus dependencias de runtime. El install sin red falla, y el producto queda en rojo por un límite de Root. |

**Medios y bajos que entran al hito 1.** El criterio: entra lo que cambia lo que mide el evaluador, lo que deja ver un gasto o una clave, y lo que es barato y está en una zona que ya se toca.
- **Cambian la medición:** M2 (un fallo de podman o un token vencido da una fila roja), M3 (`SinVeredicto` cuenta como fracaso en Wilson), M4 y M5 (calibración), M6 (suite no-pytest con junit vacío), M8 (el gate acepta una cita como `}`), M18 (una tarea copiada pisa a otra), M19 y M20 (rojos falsos en Entrega), y L7 (el prompt del juez prohíbe juzgar nombres, y `estructura.legibilidad` los mide).
- **Gasto, topes y claves:** M9 (una línea truncada deja ciego al tope), M10, M11 y L5 (topes mal escritos se ignoran), L4 (`observa claude` duplica el costo), L6, L10 (un token cortado sale sin redactar) y L1 (un `plan-aprobado` sin marca).
- **Robustez barata en zonas que se tocan:** M1, M7, M15, M21, M22, L2, L12, L13 y L14.
- **Pasan al hito 1b, con Colab y los paquetes a ciegas:** M12, M13, M14, M16, M17, L9 y L11. Tocan solo Colab o `--preparar-ciego`, que corren en el hito 1b.
- **Sin cambio:** L3 (el aviso de symlinks omitidos no cambia la medida) y L8 (la redacción ya pasa por el gate de evidencia). Las dos notas sin severidad quedan anotadas: el hash de 64 bits del GOAL sigue la decisión b2, y `evaluar` no verifica el GOAL porque solo `corre` actúa.

**Desempates técnicos 27 (Opus, 2026-11-03).**
- **C1.** Root no corre git del host sobre un `.git` del builder. Al terminar cada rol, Root copia el árbol de trabajo sin `.git` ni enlaces a un repo nuevo del host, y hace el commit ahí. Todo git sobre un repo que vino del builder corre además con `-c core.fsmonitor=false -c core.hooksPath=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_CONFIG_GLOBAL=/dev/null` y sin filtros. Razón: el sandbox termina donde el host lee algo que el builder escribió.
- **C2.** El arreglo cambia la decisión 30, que pide respetar el config del repo entero. Va al usuario como P62. Hasta la respuesta, ninguna tarea de hito 1 usa un registro privado, así que el riesgo no está activo.
- **H1.** Root corre la suite oculta con un plugin propio de pytest. Después de la colección, el plugin borra los `*.py` de la copia de los tests ocultos, antes de correr el primer test. Los datos que los tests leen, como `oculto/muestras/`, quedan. El junit se escribe con un nombre aleatorio por corrida. Razón: un producto que corre como subproceso, como memora, brasa y Graft, ya no puede leer los tests. **Límite que queda:** un producto que corre dentro del proceso de pytest puede todavía reescribir el junit. Root lo documenta en `docs/interfaces.md`, y ninguna tarea del hito 1 tiene un producto así.
- **H2.** La identidad de versión de Colab es `historial::clave_de_version`, que hashea el contenido. Razón: es la misma identidad que ya usa el historial.
- **H3.** Cada especificador de pip pasa por una gramática PEP 508 estricta. Un espacio seguido de `-`, un `\n` o un `\r` lo rechazan. Vale para `requirements.txt` y para `[build-system].requires` de un sdist.
- **H4.** Root ubica la instrucción que falló por el texto del paso que imprime buildah, no por su número. Si no la encuentra, el caso queda sin veredicto, no rojo.
- **H5.** Root lee `Requires-Dist` del `PKG-INFO` de cada sdist y `[project].dependencies` de cada clon, sin ejecutar nada, y baja esos wheels. Si los metadatos son dinámicos, la descarga da sin veredicto explícito: "dependencias de runtime no declaradas en forma estática".
- **M4 y M5.** Van en una tarea aparte, después de la mezcla de la 48. El builder de la 48 ya corre y no recibe mensajes. M5 exige al menos 5 defectos aplicables y 5 limpios para que una dimensión quede calibrada.
- **L7.** El prompt del juez dice que no juzga estilo, salvo lo que pide el ancla de un ítem. Las rúbricas están aprobadas (G2) y no cambian.

### Tareas 51 a 55: arreglos del code review, por zona de archivos

| Tarea | Hallazgos | Archivos | Arranca | Brief |
|---|---|---|---|---|
| 51 | C2 (según P62), H3, H5, M15, M22, L10, L13, L14 | `src/oracle/descarga.rs`, `src/oracle/descarga/` | Ya, en paralelo. La parte C2 espera P62 | `docs/builds/brief-local-51.md` |
| 52 | H4, M1 (oráculo), M7, M19, M20, M21, L2 | `src/oracle/entrega.rs`, `src/oracle/contenedor.rs` | Ya, en paralelo | `docs/builds/brief-local-52.md` |
| 53 | M3, M9, M10, M11, L1, L4, L5, L6, L12 (historial) | `src/observe/`, `src/track/`, `src/aprende.rs`, `src/config.rs`, `src/mcp.rs`, `src/lib.rs`, `src/oracle/historial.rs` | Ya, en paralelo | `docs/builds/brief-local-53.md` |
| 54 | H1, H2, M4, M5, M6, M8, M18, L7 | `src/oracle/mod.rs`, `src/oracle/calibra*`, `src/oracle/evidencia.rs`, `src/oracle/juez.rs`, `src/oracle/tarea.rs`, `src/oracle/pendientes.rs` | Después de la mezcla de las Tareas 48 y 49 | `docs/builds/brief-local-54.md` |
| 55 | C1, M2, M1 (runner) | `src/corre/`, y `comando_git` y `commit_de` de `src/oracle/mod.rs` | Después de la mezcla de la 50 y de la 54 | `docs/builds/brief-local-55.md` |

Cada tarea corre en su propio worktree. Ninguna toca los archivos de otra que corra al mismo tiempo.

## Orden final del hito 1 (2026-11-03, después del code review)

**En paralelo ahora:** las Tareas 48 y 49, la 50, la 51 (sin la parte C2), la 52 y la 53.

**En serie después:**
1. Mezcla de 48 y 49, de 50, de 51, de 52 y de 53, una por vez, cada una con su suite con podman.
2. Tarea 54. Después, su mezcla.
3. Tarea 55. Después, su mezcla.
4. **Calibración real otra vez.** Espera a la 54, porque M4, M5, M8 y L7 cambian lo que mide la calibración.
5. **Corrida real de memora (Tarea 16).** Espera a la 50 y a la 55, porque C1 deja correr código en el host.
6. Comparación de `global`, validez automática (Tarea 12b) y cierre (Tarea 47): verificación de los hallazgos que queden, comandos de "Validation", nota final y aprobación del usuario.
7. Borrado de memora y brasa (decisión 61).

**Hito 1b (actualizado).** Suma M12, M13, M14, M16, M17, L9 y L11, que tocan solo Colab o los paquetes a ciegas.

**Preguntas abiertas (sin responder)**

- **P62. Credenciales y config de cargo del repo (C2).** Pediste respetar el `.cargo/config.toml` del repo entero (decisión 30) y pasar las credenciales de los registros de `root.toml` (decisión 29). Juntas, permiten que un repo declare un `credential-provider` que cargo ejecuta con red y con tu token. ¿Qué elegís?

  - a) Si Root pasa una credencial, rechaza en el config del repo las claves que ejecutan comandos o cambian el TLS: `credential-provider`, `credential-alias`, `registry.global-credential-providers`, `http.cainfo`, `http.proxy` y `net.git-fetch-with-cli`. Sin credenciales, el config se respeta entero, como hoy.
  - b) Si el config del repo trae alguna de esas claves, Root no pasa la credencial. El config se respeta entero, y la descarga de ese registro puede fallar.
  - c) Como hoy. Riesgo: un repo puede sacar tu token.

  Mi recomendación es a): cambia la decisión 30 solo cuando hay una clave tuya en juego. Hoy ninguna tarea usa un registro privado, así que no frena nada.

## Vuelta del 2026-11-03 (noche): Tareas 48 a 53 terminadas

**Estado de las ramas.**
- La Tarea 50 está mezclada en 517580e. El usuario escribió: "Sí, mezclar la 50".
- Las Tareas 52 (`laptop/tarea-52`) y 53 (`laptop/tarea-53`) están terminadas. Juntas sobre 517580e dan 2109 verdes, 0 fallas y 3 ignorados con podman. Su mezcla espera la orden del usuario.
- Las Tareas 48 y 49 (`laptop/tarea-48`) están terminadas. Su informe es `docs/builds/2026-11-03-build-local-48.md`, en esa rama.
- La 48 choca con la 52 en `src/oracle/contenedor.rs`. El orquestador lo resolvió: el turno global queda en `correr_podman`, y `correr_comando` no lo toma, porque `terminar` lo llama dentro de `ejecutar` y el `Mutex` no es reentrante.
- La suite con podman de 53, 52 y 48 juntas sobre 517580e corre ahora.

**Hallazgos del orquestador** (`~/.cache/root-wt/orquestador/hallazgos-para-planner.md`):
1. `proveedores_stream_con_un_trozo_cada_60_s_no_corta_por_inactividad` falló en un hook con carga 22: manda un trozo cada 60 ms con 120 ms de inactividad.
2. y 3. Con `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0`, los subagentes en segundo plano mueren cuando termina el turno de `claude -p`. Les pasó a los builders 48 y 53. La Tarea 50 fija ese mismo valor en `env_del_builder`.
4. Un builder cortado por el tope de tiempo no deja el JSON con el costo.
5. Las Tareas 48, 50 y 52 llegaron al tope de 90 minutos del script del orquestador. El orquestador lo sube a 4 horas.
6. Con H4 de la 52, un paso fallido de `podman build` da `Pendiente`, y Entrega queda en 1,0 si no hay otros hallazgos.
7. El choque de `contenedor.rs`, resuelto como dice arriba.
8. El builder 48 costó 13,81 USD en su reanudación: Sonnet 6,93 y Haiku 6,88. Las Tareas 48 y 49 pasan de 19,9 USD en total.
9. El turno de contenedor es un `Mutex` del proceso, así que no excluye a `src/corre/ejecutor.rs` ni a otro proceso.

## Desempates técnicos 28 (Opus, 2026-11-03): informes de las Tareas 48, 50, 52 y 53

Cada respuesta dice si la decisión del organizador queda, cambia o pasa a una tarea.

**Informe de las Tareas 48 y 49.**
1. **9 o 10 defectos.** Queda: son 9. El plan contó mal en la Tarea 48.
2. **`repetido_por_entorno`.** Queda en `false`. La calibración real ahora corre una tarea por vez, así que la carga de otra calibración ya no existe.
3. **Alfabeto de secretos.** Queda el alfabeto `A-Z2-7`. Seguridad se recalibra, como todas las dimensiones (desempates técnicos 25).
4. **Límites de Estructura.** Queda `LimitesEstructura::default()`: ninguna tarea fija límites propios.
5. **`AssertBorrado` sin mutante muerto.** Queda como no aplicable: volver al azar repetiría el error.
6. **Tests que cubren un mutante.** Queda.
7. **Inyección inválida.** Queda.
8. **Tope en cada reemplazo.** Cambia, en la Tarea 54: el tope se revisa antes de cada reemplazo. Un reemplazo puede gastar otra llamada del juez, y el tope debe frenar antes de gastar.
9. **Eficiencia sin `[eficiencia]`.** Queda: si la tarea perdió `[eficiencia]`, la fila real anterior ya no describe la tarea.
10. **Línea de `DockerfileRoot`.** Queda la línea del `FROM`, que es la que cita la sonda.
11. **Línea de `ArchivoDios`.** Queda.
12. **Muestra de mutación.** Queda: la evaluación y la calibración usan la misma muestra.
13. **Reuso del listado.** Queda.
14. **Defectos de Diseño y sondas deterministas.** Queda la regla del desempate 1 de "Desempates técnicos 25": el juez lee esas sondas.
15. **Reservas contadas dos veces.** Cambia, en la Tarea 54: el coordinador cuenta cada reserva una sola vez. Con el juez de config en Sonnet, contar dos veces frenaría la calibración mucho antes del tope.
16. **Turno y `src/corre/ejecutor.rs`.** Queda sin código. Regla de la laptop: una calibración real y un `r00t corre` real no corren a la vez. El turno es un `Mutex` del proceso y no puede excluir a otro proceso.
17. **Limpieza de 24 horas.** Cambia, en la Tarea 54: cada evaluación toma un lock sobre su carpeta temporal, y `limpiar_temporales` salta las carpetas con lock. Una calibración ya duró 15 horas.
18. **Memo de una sola tarea.** Cambia, en la Tarea 54: `MemoSondas` filtra por tarea. Es una línea y evita un error silencioso.
19. **Plazo de 300 s del test del turno.** Queda.
20. **Archivo de casos.** Cambia, en la Tarea 54: el error del tope nombra el archivo de casos. Sin política de retención: los archivos son chicos.

**Informe de la Tarea 50.**
1. **Plugin con `ro,z`.** Queda: la carpeta del banco se borra con memora y brasa (decisión 61).
2. **Vigencia de 4 horas.** Queda: es el peor caso de dos roles.
3. **`verificar_antes` en el trait.** Queda.
4. **Borrado de la credencial copiada.** Queda.
5. **Plugin de un rol que no está en `roles`.** Queda.

**Informe de la Tarea 52.**
1. **H4 deja Entrega en 1,0 (y hallazgo 6).** Cambia, en la Tarea 56. Una construcción `Pendiente`, o una imagen base que no se descargó, deja su ítem de Entrega en `faltantes` con su motivo. Entrega toma su valor de los ítems medidos. Si no midió ninguno, queda sin veredicto. Razón: un 1,0 sobre algo que no se midió es un verde falso.
2. **Acotar `correr_podman` de la descarga.** Pasa a la Tarea 51, que es dueña de `src/oracle/descarga*`.
3. **Python sin caché.** Queda. Ninguna tarea del hito 1 es Python: memora, brasa y Graft son Rust.
4. **Dos `RUN` iguales en etapas distintas.** Queda: sin veredicto es lo seguro.
5. **Un commit para M7, M1 y L2.** Queda.

**Informe de la Tarea 53.**
1. **`TopeMes` estricto y los demás no.** Cambia, en la Tarea 56: `Ruteo`, `ModeloApi`, `Modelos` y `ConfigProveedores` también llevan `deny_unknown_fields`. Un typo ahí cambia el modelo de un rol sin aviso.
2. **Test cambiado.** Queda.
3. **`registrar_sesion` del builder.** Pasa a la Tarea 55, que es dueña de `src/corre/`.
4. **Log corrupto en `observa claude`.** Queda: falla cerrado.
5. **Más validación de `[balance]`.** Cambia, en la Tarea 56: `tope_usd` finito y no negativo, `prefiere_api_sobre` entre 0 y 1, y `bloque_horas` mayor que 0.

**Hallazgos del orquestador.**
1. **Test de tiempo frágil.** Tarea 56: el test usa un reloj inyectado, sin plazos reales.
2. y 3. **Subagentes cortados.** Tarea 55: `env_del_builder` fija `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS` igual al plazo del rol, no 0. Razón: el runner debe medir lo que el organizador construye, no cortar su trabajo. La config `docs/builds/config-ecc-haiku.toml` además pide subagentes en primer plano.
4. **Costo de un builder cortado.** Tarea 55: si el runner corta un rol por tiempo, suma el `usage` de los mensajes del transcript `stream-json` y registra ese costo con `extra.costo_estimado = true`.
5. **Builders lentos.** Sin código de Root: es una regla del orquestador, que ya subió su tope a 4 horas.
6. Ver la pregunta 1 de la Tarea 52.
7. **Choque de `contenedor.rs`.** Queda la resolución del orquestador.
8. **Costo de los builders.** Queda anotado como dato: una sola reanudación no alcanza para cambiar la decisión del usuario sobre los roles.
9. Ver la pregunta 16 de la Tarea 48.

### Tarea 56: Entrega sin verde falso, config estricta y test de stream con reloj
- **Action**: pregunta 1 de la Tarea 52, preguntas 1 y 5 de la Tarea 53, y hallazgo 1.
- **Archivos**: `src/oracle/entrega.rs`, `src/config.rs`, `tests/proveedores.rs` y, si hace falta inyectar el reloj, el stream de `src/proveedores/`.
- **Arranca**: después de la mezcla de las Tareas 52 y 53. Corre en paralelo con la 51 y la 54.
- **Brief**: `docs/builds/brief-local-56.md`.

## Orden final del hito 1 (2026-11-03, noche)

1. **Mezclas de 53, 52 y 48.** Una por vez, cuando termine su suite conjunta y con la orden del usuario.
2. **En paralelo, después de esas mezclas:** la Tarea 51 (descarga, sin la parte C2), la 54 (núcleo del Oracle y calibración) y la 56 (Entrega, config y stream). No comparten archivos.
3. **Tarea 55** (runner y git seguro). Arranca después de la mezcla de la 54, porque toca `comando_git` y `commit_de` de `src/oracle/mod.rs`. Puede correr en paralelo con la 51 y la 56 si todavía corren.
4. **Calibración real otra vez.** Espera a la 54 y a la 56, porque las dos cambian lo que mide: la 54 cambia la calibración y el juez, y la 56 cambia Entrega.
5. **Corrida real de memora (Tarea 16).** Espera a la 55. No corre a la vez que la calibración real (pregunta 16 de la Tarea 48).
6. Comparación de `global`, validez automática y cierre (Tarea 47).
7. Borrado de memora y brasa (decisión 61).

P62 sigue abierta, y la parte C2 de la Tarea 51 la espera. No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-04: mezclas hasta 8b417db y Tarea 54 terminada

**Estado.** `claude/root-e7813e` está en 8b417db, con las Tareas 48, 49, 50, 51 (con C2), 52, 53 y 56. El usuario ordenó cada mezcla: "Sí, avanzar" para la 53, la 52, la 48 y la 51, y "Sí, mezclar ambas" para la 56 y la C2. El código de 8b417db es igual al árbol probado con podman: 2258 verdes, 0 fallas y 3 ignorados. La Tarea 54 (`laptop/tarea-54`) terminó 11 de 13 puntos, y su prueba con podman corre ahora. Costos de la tanda: 51 = 1,85 USD, C2 = 0,68 USD, 56 = 1,43 USD y 54 = 6,09 USD.

**Hallazgos del orquestador.**
1. La 54 no hizo los puntos 10 (reservas contadas una vez) ni 12 (memo por tarea): exigían archivos fuera de su lista.
2. La 54 tocó una línea de `src/lib.rs` para H2.
3. La 51 dejó L14 a medias: falta cambiar el llamador de `src/lib.rs` a `cache_de_con_error(...)?`.
4. La 51 deja como acierto una caché de Python anterior a H5.
5. La C2 debilitó tres guardas de secretos: buscan `clave@` en vez del valor.
6. La C2 no tiene un test de punta a punta con podman.
7. La 56 no hizo `faltantes` por ítem en Entrega.
8. `calibra_un_caso_con_error_corta_y_se_propaga_el_primero` falló una vez bajo carga.
9. El builder 54 vio 2 fallas en un worktree auxiliar, que el orquestador no reproduce.
10. H1 de la 54 corre la suite oculta desde `/salida/.oculto`. Los `tarea.toml` reales de memora y brasa llaman a pytest con la ruta literal `/oraculo/oculto` y con `--junitxml=/salida/oculto.xml`.

## Desempates técnicos 29 (Opus, 2026-11-04)

**Informe 51.**
1. P62: respondida (decisión 65).
2. **Clon con `setup.py` legado.** Queda sin cortar, y suma un aviso en `confianza`: "clon sin metadatos estáticos: dependencias de runtime no verificadas". Razón: cortar rompía un caso real, y el aviso dice lo que Root no midió. Tarea 57.
3. **`[project]` sin `dependencies`.** Queda en cero dependencias.
4. **Clave de caché de Python.** Cambia: la clave suma una versión de formato, así las cachés anteriores a H5 dejan de ser acierto. Tarea 57.
5. **`src/lib.rs` y L14.** Cambia: el llamador pasa a `cache_de_con_error(...)?`. Tarea 57.
- **Dependencia de runtime que solo tiene sdist.** Cambia: da sin veredicto, no `no-instala`. Es un límite de Root, como fija el desempate 31 de "Desempates técnicos 11". Tarea 57.
- **`PKG-INFO` anterior a Metadata 2.1.** Cambia: suma el mismo aviso de dependencias no verificadas. Tarea 57.

**Informe 51-C2.**
1. **`credential-provider` en cualquier sección.** Queda.
2. **`[credential-alias]` como tabla.** Queda.
3. **`http.proxy` con usuario y clave.** Queda.
4. **Rechazo sin registro.** Cambia: un rechazo del inventario registra un evento `Gate` con la clave rechazada y su línea, sin valores. Vale también para el corte por `inv.rechazos` que ya existía. Razón: el invariante de observabilidad. Tarea 57.

**Informe 56.**
1. **Un requisito cuenta como medido en Entrega.** Cambia: Entrega toma su valor solo de sus propios ítems. Los requisitos son de Completitud. Los dos tests de `src/oracle/mod.rs` que fijan el caso se ajustan. Tarea 58.
2. **`faltantes` por ítem en Entrega.** Cambia: `ResultadoEntrega` suma `faltantes`, y el ítem de construcción pendiente aparece en `Puntaje.faltantes`. Tarea 58.
3. **Reloj simulado.** Queda: inyectar un reloj real exige cambiar el transporte de `ureq`.
4. **Build pendiente sin hallazgos.** Queda sin veredicto.

**Informe 54.**
1. **Reservas contadas una vez.** Cambia: cada reserva lleva el id del caso, y el coordinador no suma el peor caso de un caso que ya tiene reserva. Toca `src/proveedores/` y `src/observe/` solo para la etiqueta. Tarea 58.
2. **Memo por tarea.** Cambia: se autoriza `src/oracle/memo.rs`. Tarea 58.
3. **La línea de `src/lib.rs` para H2.** Queda.
4. **`colab.json` viejos descartados.** Queda: Colab es del hito 1b.
5. **M18 en `cargar_en`.** Queda.
6. **M6 y `ModuloBorrado`.** Queda: las suites ocultas de memora y brasa son pytest, así que M6 no las toca.
7. **Regla exacta de M6.** Queda.
8. **Filas viejas sin muestra.** Queda: la calibración se repite entera.
9. **Un umbral para la ficha y el juez.** Queda: una frase de spec con menos de 8 caracteres alfanuméricos no es una cita útil.
10. **Otros prompts.** Queda sin cambio: el juez de la ficha y el de la config no puntúan ítems de rúbrica.
11. a 13. Quedan.
14. **Lock de Colab.** Pasa al hito 1b, con Colab.
15. **Ruta de la suite oculta.** Se verifica con una evaluación de humo antes de calibrar (ver abajo). Si la ruta literal rompe memora o brasa, el arreglo va a la Tarea 58.
16. **`PYTHONPATH` previo.** Queda.
17. **Base en otro worktree.** La mezcla de la 54 corre la suite con podman dos veces seguidas. Una falla que no se repite se registra como inestable, con su nombre.

**Hallazgos del orquestador.**
- 1, 2 y 7: ver arriba.
- 3 y 4: Tarea 57.
- 5. **Guardas de secretos.** Cambia: cada guarda busca el valor del secreto de prueba, no el nombre del campo. Tarea 57.
- 6. **C2 de punta a punta.** Tarea 57 suma un test con podman: un repo con `credential-provider` y una credencial de prueba no ejecuta el proveedor, y deja el `Gate` del rechazo.
- 8. **Test frágil.** Tarea 58: el coordinador propaga el error del caso con el índice más bajo, no el primero en el tiempo.
- 9. Ver la respuesta 17 del informe 54.
- 10. Ver la respuesta 15 del informe 54.

## ¿Cuándo arranca la calibración real?

Arranca después de la mezcla de la 54, con dos condiciones:
1. **Evaluación de humo.** El orquestador corre una evaluación de memora-ecc y una de brasa-ecc, sin claves y con 0 USD. El oculto debe dar lo mismo que el Validate 14: 66/68 y 20/21. Si cambia, la calibración espera el arreglo de la Tarea 58.
2. **Margen de tope.** Hasta que entre la Tarea 58, una reserva puede no verse por hasta 3 casos en vuelo. La calibración corre con `--tope-usd` igual al saldo menos 1 USD. Con `deepseek-v4-pro`, 3 casos en vuelo cuestan como mucho 0,33 USD, así que el tope del usuario no se pasa (decisión 15).

No hace falta esperar a la 55, la 57 ni la 58: ninguna cambia lo que mide la calibración de memora y brasa.

**Calibración y builders a la vez.** Mientras corre la calibración, los builders exportan `ROOT_SIN_PODMAN=1`. Así sus tests y sus hooks no cargan la CPU con podman. Sus suites con podman corren al mezclar, cuando la calibración termina o entre memora y brasa.

### Tarea 57: Descarga y guardas de secretos
- **Action**: respuestas 2, 4 y 5 y los dos puntos sueltos del informe 51; respuesta 4 del informe C2; hallazgos 5 y 6; y el tope de salida en `contenedor::ejecutar`.
- **Archivos**: `src/oracle/descarga.rs`, `src/oracle/descarga/`, `src/oracle/contenedor.rs` (solo `ejecutar`), y una línea de `src/lib.rs` (el llamador de `cache_de`).
- **Arranca**: ya, desde 8b417db. Corre en paralelo con la 55 y la 58.
- **Brief**: `docs/builds/brief-local-57.md`.

### Tarea 58: Núcleo del Oracle después de la 54
- **Action**: respuestas 1 y 2 del informe 56; respuestas 1 y 2 del informe 54; hallazgo 8; el tope de salida de `entrega.rs`; y el arreglo de la ruta de la suite oculta, si la evaluación de humo falla.
- **Archivos**: `src/oracle/mod.rs` (salvo `comando_git` y `commit_de`), `src/oracle/calibra.rs`, `src/oracle/calibra/`, `src/oracle/memo.rs`, `src/oracle/entrega.rs`, y en `src/proveedores/` y `src/observe/` solo la etiqueta del caso en la reserva.
- **Arranca**: después de la mezcla de la 54. Corre en paralelo con la 55 y la 57.
- **Brief**: `docs/builds/brief-local-58.md`.

## Orden final del hito 1 (2026-11-04)

1. **Mezcla de la 54.** Con la orden del usuario, y con la suite con podman dos veces.
2. **Evaluación de humo** de memora-ecc y brasa-ecc, sin claves.
3. **En paralelo después:**
   - la calibración real, memora primero y brasa después, con el margen de tope;
   - las Tareas 55, 57 y 58, con `ROOT_SIN_PODMAN=1` mientras corre la calibración.
4. **Mezclas de 55, 57 y 58.** Una por vez, cada una con su suite con podman, sin la calibración corriendo.
5. **Comparación de `global`.**
6. **Corrida real de memora (Tarea 16).** Espera a la 55. No corre a la vez que la calibración.
7. **Validez automática (Tarea 12b)** y **cierre (Tarea 47)**.
8. **Borrado de memora y brasa** (decisión 61).

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-04 (madrugada): Tarea 54 mezclada, humo y compilación

**Estado.** `claude/root-e7813e` está en 367b0b4, con la Tarea 54. El usuario escribió: "Sí, mezclar la 54". La máquina se reinició cerca de las 00:04, y el orquestador retomó la humo y los builders de las Tareas 55 y 57. La Tarea 58 no arrancó.

**Evaluación de humo** (release de 367b0b4, sin claves, 0 USD).
- memora-ecc: contrato 61/61 y oculto 65/68, en 653 s. Las 3 fallas son inestables medidos: `test_blpop_fifo_entre_clientes` 4/40, `test_bgsave_no_bloquea` 32/40 y `test_client_id` 42/50. El Validate 14 dio 66/68 con los mismos inestables.
- brasa-ecc: contrato 53/53 y oculto 20/21, en 2423 s. Falla `test_spin_despues_de_rotacion`, como en el Validate 14.
- La ruta nueva de la suite oculta (`/salida/.oculto`) no rompe memora ni brasa. El punto 7 de la Tarea 58 no hace falta.

**Medición de la compilación** (memora-ecc en `root-oraculo-rust:2`, con carga 30).

| Comando | En frío | Con el `target` de la base, tras tocar `src/config.rs` |
|---|---|---|
| `cargo test --no-run` | 80 s | 4,7 s |
| `cargo build --release` | 67 s | 28,7 s |

- Hoy cada caso compila desde cero en su propio `/salida/target`: la suite oculta, la de contrato y clippy fijan `CARGO_TARGET_DIR=/salida/target`.
- El disco es btrfs, así que `cp --reflink=auto` copia un `target` casi sin costo.
- La cuenta gruesa del orquestador: unos 2 minutos por caso, unos 320 casos, unas 10 horas de CPU.

**Otros datos.**
- Colab Pro+ no sirve para el oráculo: la red de podman falla, `--network none` no levanta la interfaz y los cgroups están en solo lectura.
- El orquestador mantiene podman: arrancar un contenedor cuesta unos 0,3 s, y un caso compila durante minutos.
- El orquestador planea 3 procesos de `r00t` en paralelo. La laptop tiene 8 núcleos y 15 GB, y cada contenedor usa hasta 2 GB.

## Desempates técnicos 30 (Opus, 2026-11-04): reuso del `target` y reparto de la calibración

**Reuso del `target` (decisión 66).**
- Cada caso de calibración arranca con una copia del `target` de la base de su misma calibración, hecha con `cp --reflink=auto`. La copia va al mismo `/salida/target` del contenedor, así que las rutas que cargo guarda no cambian.
- **Por qué mide lo mismo.** Cargo decide qué recompilar por sus huellas. El caso cambia archivos del repo, y cargo recompila ese crate. Las dependencias de `/cache/vendor` no cambian, y su compilación se reusa. El resultado de los tests y de clippy es el mismo que con un `target` vacío: solo cambia el tiempo.
- **Cuándo no se reusa.** Si el caso cambia `Cargo.toml` o `Cargo.lock`, si cambia el digest de la imagen, o si la base no dejó `target`, el caso compila en frío y lo registra en su renglón de `.casos.jsonl`.
- **La evaluación real no reusa `target` de otra evaluación.** Dentro de una evaluación, las repeticiones ya comparten su `/salida/target`. Reusar entre evaluaciones de repos distintos dejaría que el `build.rs` de un producto plante artefactos para otro. En la calibración, la base y sus casos salen del mismo producto, así que esa vía no existe.
- **Prueba.** Sobre un fixture, un caso con el `target` reusado y el mismo caso en frío dan el mismo veredicto.

**Tests inestables en la calibración.**
- La humo de memora-ecc tiene 3 inestables con tasas de 10 %, 80 % y 84 %. En un caso limpio, esos tests pueden cambiar de clase y bajar un nivel sin ningún defecto. Eso es un falso positivo que no dice nada del evaluador.
- **Desempate.** En Completitud y Funciona, un caso detecta un defecto solo por tests que la base clasificó como estables. Los inestables de la base no cuentan para detectar, ni en un caso con defecto ni en uno limpio. Razón: la evaluación real ya informa los inestables aparte, y la calibración mide si el evaluador ve el defecto inyectado, no el ruido conocido.
- Va en la Tarea 59, porque toca la misma regla de detección.

**Reparto en 3 procesos.**
- 3 procesos entran en 15 GB: cada proceso corre un contenedor por vez, así que son unos 6 GB en contenedores, más el host.
- Los procesos se reparten por dimensión, para que cada uno tenga una carga parecida. Primero memora, y después brasa:
  - proceso A: Completitud y Funciona, que corren la suite oculta y son los más lentos;
  - proceso B: Pruebas, Seguridad y Estructura;
  - proceso C: Arquitectura, Diseño y Eficiencia (que da "no aplica").
- Entrega se calibra después de la mezcla de la Tarea 58, porque la 58 cambia cómo mide Entrega (respuestas 1 y 2 del informe 56). Esto corrige lo que dijo el plan en la vuelta anterior: la 58 sí cambia una dimensión de la calibración.
- **Tiempo de brasa.** Una evaluación de brasa tardó 2423 s. Un caso de Completitud o Funciona de brasa tarda casi lo mismo, porque el reuso del `target` solo ahorra la compilación. Con 21 casos por dimensión, el proceso A de brasa puede tardar unas 28 horas. Es una estimación, y la calibración real la mide.

**Tarea 58 y Tarea 59.** Corren una después de la otra: las dos tocan `src/oracle/mod.rs` y `src/oracle/calibra*`. La 59 va primero, porque la calibración la espera. La 58 arranca después de la mezcla de la 59, y corre mientras calibran los procesos A, B y C, con `ROOT_SIN_PODMAN=1`.

### Tarea 59: Reuso del `target` de la base y detección sin inestables
- **Action**: el reuso del `target` y la regla de los tests inestables, de "Desempates técnicos 30".
- **Archivos**: `src/oracle/calibra.rs`, `src/oracle/calibra/`, y en `src/oracle/mod.rs` solo lo necesario para recibir un `target` inicial en la evaluación de un caso.
- **Arranca**: ya, desde 367b0b4. Corre en paralelo con la 55 y la 57, que no tocan esos archivos.
- **Brief**: `docs/builds/brief-local-59.md`.

## Orden final del hito 1 (2026-11-04, madrugada)

1. **Tarea 59** con el builder local. En paralelo siguen la 55 y la 57.
2. **Mezcla de la 59**, con la orden del usuario.
3. **Calibración real de memora**, en 3 procesos (A, B y C), con `--tope-usd` igual al saldo menos 1 USD. Después, la de brasa.
4. **En paralelo con la calibración:** la Tarea 58, con `ROOT_SIN_PODMAN=1`. Las mezclas de la 55, la 57 y la 58 esperan una pausa de la calibración, entre memora y brasa o al final.
5. **Calibración de Entrega** de memora y brasa, después de la mezcla de la 58.
6. **Comparación de `global`.**
7. **Corrida real de memora (Tarea 16).** Espera a la 55, y no corre a la vez que la calibración.
8. **Validez automática** y **cierre** (Tarea 47).
9. **Borrado de memora y brasa** (decisión 61).

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-04 (tarde): 55, 57 y 59 mezcladas, corrida real de memora en curso

**Estado.** `claude/root-e7813e` está en 10ad8d3, con las Tareas 55, 57 y 59. El usuario ordenó cada mezcla: "Sí, mezclar la 55", "Sí, mezclar la 57" y "Sí, mezclar la 59". Las pruebas con podman dieron 2318, 2342 y 2379 verdes, sin fallas. La 57 chocó con la 54 en `tests/oraculo.rs`, y el orquestador conservó los dos bloques. La corrida real de memora (`r00t corre --tarea memora --config ecc-haiku`, release de 50df9c4) arrancó a las 13:01 y va por su segundo rol. La Tarea 58 corre desde 10ad8d3 con `ROOT_SIN_PODMAN=1`. La calibración espera a que termine la corrida real.

**Hallazgos del orquestador 10 a 13.**
- **10.** El usuario respondió "Sí, al hito 1b" a la propuesta del oráculo de industria. Ver el hito 1b abajo.
- **11.** Dos tests de `tests/colab.rs` fallaron con la máquina cargada: `colab_tope_minutos_001_deja_gate_tope_de_duracion` y `colab_sigterm_durante_la_corrida_libera_y_sale_3`, este con "PID numérico: ParseIntError { kind: Empty }".
- **12.** Los subagentes de la 59 con `isolation: "worktree"` cayeron en `~/.claude/orchestration/.claude/worktrees/agent-*`, del repo de mmorch. No trabajaron, y `mmorch/auto` no cambió.
- **13.** La 59 reusa el `target` solo en las suites. Clippy y la mutación siguen compilando en frío.

**Regla del orquestador (hallazgo 12).** Ningún pedido a un builder usa `isolation: "worktree"`. Cada builder trabaja en el worktree que le arma el orquestador.

## Desempates técnicos 31 (Opus, 2026-11-04)

**Informe 55.**
1. **Precio pesimista sin precio.** Queda: un tope debe sobreestimar.
2. **Alias de modelos.** Cambia: en producción, `eleccion.modelo` de un rol por cupo es un alias como `haiku`, y su precio no existe con esa clave. El costo de un rol cortado traduce el alias con `ruteo.api_equivalente` antes de buscar el precio. Opus no tiene precio por defecto, así que sigue con el pesimista. Tarea 61.
3. **Código distinto de 0 con `result` exitoso.** Queda: el `result` es la evidencia del rol.
4. **`FASE_ROL` unificada.** Queda.
5. **Mensaje de un fallo de escritura.** Cambia: un fallo de escritura del log dice "sesión no registrada", no "sesión no leída". Tarea 61.
6. **Códigos 125 a 137 en el oráculo.** Cambia: `src/oracle/contenedor.rs` trata 125, 126, 127 y 137 como infraestructura, igual que el runner. Tarea 61.
7. **Historial del builder.** Queda descartado: leer el `.git` del builder reabre C1.
8. **Session id del fixture.** Queda.

**Informe 57.**
1. **Aviso en `Confianza.avisos`.** Cambia: el aviso de dependencias no verificadas llega a `Confianza.avisos`, como pide el desempate 2 del informe 51 en "Desempates técnicos 29". Toca `src/oracle/mod.rs`, así que va en la Tarea 61, después de la 58.
2. y 3. Quedan.
4. **Error de compilación de una dependencia.** Queda como infraestructura: es un límite del entorno, no del producto.
5. y 6. Quedan.

**Informe 59.**
1. **Estable que pasa a inestable.** Queda: el defecto puede crear una carrera.
2. **Menos tests en el caso.** Queda.
3. **Filas viejas.** Queda: la calibración se repite entera.
4. Ver el hallazgo 13.
5. a 10. Quedan. Sobre el 8: el disco de la laptop es btrfs, y otro sistema de archivos no está en el hito 1.
11. **`Guardar` parcial.** Cambia: la copia va a una carpeta temporal y se renombra al final. Un `target` parcial nunca se reusa. Tarea 61.
12. **Escenarios en la regla de estables.** Queda: un escenario es un flujo de punta a punta, y la Tarea 58 revisa Entrega.

**Hallazgo 11: tests de tiempo de Colab.** Los dos tests usan plazos inyectados, sin esperas reales. El de SIGTERM espera a que el falso escriba su PID antes de leerlo. Tarea 60, ya, en paralelo con la 58: toca solo `tests/colab.rs` y, si hace falta, `src/oracle/colab.rs`.

**Hallazgo 13: clippy y mutación en frío.** Pruebas y Estructura se calibran como están. Razón: la Tarea 49 ya acotó la mutación a su muestra, y reusar el `target` en clippy y en `cargo-mutants` ahorra cerca de una hora de una calibración de más de 30. Una tarea nueva tocaría el núcleo de `src/oracle/` mientras corre la 58, y la calibración esperaría más de lo que ahorra.

### Tarea 60: Tests de tiempo de Colab
- **Action**: el hallazgo 11.
- **Archivos**: `tests/colab.rs` y `src/oracle/colab.rs`.
- **Arranca**: ya, en paralelo con la 58 y con la calibración, con `ROOT_SIN_PODMAN=1`.
- **Brief**: `docs/builds/brief-local-60.md`.

### Tarea 61: Ajustes de los informes 55, 57 y 59
- **Action**: respuestas 2, 5 y 6 del informe 55, respuesta 1 del informe 57 y respuesta 11 del informe 59.
- **Archivos**: `src/corre/`, `src/oracle/contenedor.rs`, `src/oracle/mod.rs` (solo el aviso en `Confianza.avisos`) y `src/oracle/calibra.rs` (solo `Guardar`).
- **Arranca**: después de la mezcla de la 58, que toca `mod.rs` y `calibra.rs`. Ninguno de sus cambios altera lo que mide la calibración.
- **Brief**: `docs/builds/brief-local-61.md`.

## Calibración: qué arranca y cuándo

Apenas termina la corrida real de memora, arrancan todas las dimensiones salvo Entrega, en 3 procesos. memora primero y brasa después:
- **Proceso A:** Completitud y Funciona.
- **Proceso B:** Seguridad, Pruebas y Estructura.
- **Proceso C:** Arquitectura, Diseño y Eficiencia.

Entrega se calibra después de la mezcla de la 58. Si la calibración de memora ya terminó, Entrega de memora corre sola, y la de brasa se suma al proceso que termine primero.

## Hito 1b (actualizado)

Suma el **oráculo de industria** (hallazgo 10). El usuario respondió: "Sí, al hito 1b". Una capa compara el producto con productos reales en cuatro ejes: compatibilidad, rendimiento, robustez y durabilidad. Cada eje tiene un comando y no usa juez. Las tres recomendaciones del orquestador quedan como diseño de partida:
1. Un archivo `industria.toml` por tarea, aparte de `tarea.toml`.
2. La población de productos la elige el planner, con productos de alcance parecido.
3. Corre en podman, con la CPU fija para que el rendimiento se compare.

La propuesta completa está en `/home/mpera/.config/Claude/side-session-notes/local_c0426133-6733-48e8-851f-10d539c72f11/2026-11-04-propuesta-root-oraculo-industria.md`.

## Orden final del hito 1 (2026-11-04, tarde)

1. **Ahora:** la corrida real de memora, la Tarea 58 y la Tarea 60.
2. **Al terminar la corrida real:** la calibración de memora en 3 procesos, sin Entrega. Después, la de brasa.
3. **Mezclas de la 58 y la 60**, en una pausa de la calibración, con la orden del usuario.
4. **Tarea 61** después de la mezcla de la 58, con `ROOT_SIN_PODMAN=1` si la calibración sigue.
5. **Calibración de Entrega** de memora y brasa, después de la mezcla de la 58.
6. **Comparación de `global`.**
7. **Validez automática** y **cierre** (Tarea 47).
8. **Borrado de memora y brasa** (decisión 61).

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-04 (noche): 58 y 60 mezcladas, corrida real cortada

**Estado.**
- `claude/root-e7813e` está en 29b0203, con las Tareas 58 y 60. El usuario escribió: "Sí, mezclar ambas". La prueba con podman dio 2398 verdes y 0 fallas.
- La Tarea 61 terminó en `laptop/tarea-61` (6ffe985), por 1,09 USD. Sin podman dio 2407 verdes, y su prueba con podman corre ahora.
- La app de escritorio se reinició cerca de las 14:50 y cortó la corrida real de memora a la 1 h 50 min. La corrida no tiene modo de retomar.
- El orquestador relanzó todo con `systemd-run --user`, para que otro reinicio de la app no lo corte.
- La corrida real no arranca: Root la frena porque el login de Claude vence a las 19:14, antes del plazo total de la corrida.

## Desempates técnicos 32 (Opus, 2026-11-04)

**Informe 58.**
1. **Memo por tarea y `mutacion.rs`.** Queda latente: cada memo se crea para una sola tarea, y el arreglo exige exponer otro comando sin ganar una medición.
2. **Caso con varias llamadas de juez.** Queda: el margen de 1 USD de la calibración cubre esa subcuenta, y cada llamada pasa igual por el chequeo de tope de la Tarea 21.
3. a 5. Quedan, por el mismo margen.
6. **`entrega.contenedor` como ítem de construcción.** Queda: su ancla 3 pide construir sin red.
7. y 8. Quedan.
9. **Nombres de tests.** Cambia: los dos tests se renombran, porque un nombre que dice lo contrario engaña al próximo lector. Tarea 62.
10. **`COLA` en dos archivos.** Queda: unificarla no cambia ninguna medición.
- **Tope antes de clasificar en `entrega.rs`.** Queda como límite anotado: si un paso imprime más de 4000 bytes, la cabecera `STEP` puede cortarse. El resultado es sin veredicto, no un rojo falso.

**Informe 60.**
1. **Plazo inyectado.** Queda sin variable nueva: un test de 6 s con cota de 30 s es estable, y los tests unitarios de `colab.rs` cubren el tope fino.
2. Queda.

**Informe 61.**
1. y 2. Quedan.
3. **127 como infraestructura.** Queda: un 127 dice que no se encontró el comando, y eso no mide al producto.
- **Texto del aviso repetido.** Cambia: `mod.rs` y `descarga/python.rs` comparten una sola constante. Tarea 62.
- **`src/oracle/__pycache__/`.** Cambia: `.gitignore` suma `__pycache__/`. Tarea 62.

**Orden de la calibración y la corrida real.**
- **La calibración arranca primero, después de la mezcla de la 61.** La regla que sigue es otra: las dos no corren a la vez, porque la corrida real carga la CPU y ensucia los tests de tiempo. El orden entre ellas nunca fue una dependencia: la calibración usa claves de API y no el login, y no lee nada de la corrida real.
- **Qué se pierde con la calibración primero.** La corrida real se atrasa hasta una pausa, entre memora y brasa. Si esa corrida encuentra un defecto del runner, se conoce más tarde. Ese defecto no cambia la calibración, porque el runner no entra en `evaluar`.
- **Qué se perdería con la corrida real primero.** La calibración se atrasaría al menos 4 horas más la Tarea 62, sin ganar nada para su medición.
- **Entrega.** La 58 ya está mezclada, así que Entrega entra ahora al proceso C.

**Chequeo de vigencia del login.** Sí, es demasiado estricto, y además deja un riesgo.
- **Por qué es estricto.** Exige que el token cubra 2 horas por cada rol de la config, desde el arranque. Son 4 horas de peor caso con dos roles. Pero el archivo del host se renueva solo cuando Claude Code corre en el host, y el planificador suele tardar mucho menos que su tope.
- **El riesgo.** La copia del builder trae todo `claudeAiOauth`, con el token de refresco. Si el token de acceso vence durante un rol, `claude` dentro del contenedor puede refrescarlo. Un refresco puede rotar el token e invalidar el login del host. El desempate de "Desempates técnicos 26" pedía que el contenedor nunca renueve ni rote el token, y hoy solo lo evita la vigencia.
- **Desempate.**
  - La copia no lleva `refreshToken`. El contenedor no puede rotar el login.
  - La vigencia se revisa antes de cada rol, contra el plazo de ese rol (2 horas). Antes de cada rol, Root vuelve a leer la credencial del host y arma otra copia.
  - Si la vigencia no alcanza antes de un rol que no es el primero, Root registra un `Gate` y espera hasta 30 minutos a que el host renueve el archivo. Después sale con código 3. Razón: la corrida no se puede retomar, y una espera corta evita perder lo ya hecho.
  - Antes del primer rol, sigue saliendo con código 3 de inmediato.
- Una prueba real confirma que `claude` acepta una credencial sin `refreshToken`. Si no la acepta, el builder lo anota y conserva el token, con la vigencia por rol. Entonces el planner pregunta al usuario.

### Tarea 62: Vigencia por rol, credencial sin refresco y limpieza
- **Action**: el desempate de vigencia, y los nombres de tests, la constante del aviso y `.gitignore`.
- **Archivos**: `src/corre/`, la constante del aviso en `src/oracle/descarga/python.rs` y `src/oracle/mod.rs`, los dos tests a renombrar y `.gitignore`.
- **Arranca**: después de la mezcla de la 61, con `ROOT_SIN_PODMAN=1`, porque la calibración corre al mismo tiempo. Ninguno de sus cambios altera lo que mide la calibración.
- **Brief**: `docs/builds/brief-local-62.md`.

## Orden final del hito 1 (2026-11-04, noche)

1. **Mezcla de la 61.** Espera su prueba con podman y la orden del usuario.
2. **Calibración real de memora**, en 3 procesos con `systemd-run --user`, sobre esa mezcla y con `--tope-usd` igual al saldo menos 1 USD. Proceso A: Completitud y Funciona. Proceso B: Seguridad, Pruebas y Estructura. Proceso C: Arquitectura, Diseño, Eficiencia y Entrega.
3. **Tarea 62** en paralelo con el paso 2, sin podman.
4. **Pausa entre memora y brasa:** mezcla de la 62, comparación de `global` de memora y corrida real de memora (Tarea 16), con el test `ROOT_TEST_BUILDER_REAL=1` antes.
5. **Calibración real de brasa**, en los mismos 3 procesos. Después, la comparación de `global` de brasa.
6. **Validez automática** (Tarea 12b) y **cierre** (Tarea 47).
7. **Borrado de memora y brasa** (decisión 61).

**Límite conocido.** `systemd-run --user` protege de un reinicio de la app, no de un reinicio de la máquina. Una calibración cortada pierde solo la dimensión en curso: las filas terminadas quedan en `calibracion.json`.

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-05: calibración de memora y parte de brasa

**Estado.** `claude/root-e7813e` está en 0d00eb0, con la Tarea 61. La Tarea 62 está en `laptop/tarea-62` (758091b): su test real con `claude` pasa, y la suite sin podman da 2413 verdes. La calibración corrió con el binario 26f2170 y gastó 11,3877 USD de los 25 del usuario, con 1 USD de margen. Quedan 12,6123 USD.

| Tarea | Dimensión | Sensibilidad / especificidad | Segundos |
|---|---|---|---|
| memora | seguridad | 1.0 / 1.0 | 32 |
| memora | entrega | 1.0 / 1.0 | 88 |
| memora | pruebas | sin dato / 1.0, 10 defectos no aplicables | 167 |
| memora | completitud | 1.0 / 0.3 | 935 |
| memora | estructura | 1.0 / 0.6 | 1951 |
| memora | arquitectura | 1.0 / 0.0 | 2538 |
| memora | diseno | 0.8 / 0.0 | 3213 |
| memora | eficiencia | no aplica | 0 |
| memora | funciona | exit 2: "no tiene defectos inyectables" | 0 |
| brasa | completitud | 1.0 / 1.0 | 1101 |
| brasa | seguridad | 1.0 / 1.0 | 16 |
| brasa | pruebas | 1.0 / 1.0 | 28644 |
| brasa | funciona | exit 2 | 0 |

Falta brasa: estructura, arquitectura, diseno, eficiencia y entrega.

## Diagnóstico de la calibración (Opus, 2026-11-05)

**La pregunta "¿el repo base tiene esos defectos?" no explica estas falsas alarmas.** La detección compara cada caso contra la base, no contra un ideal. Un defecto real de la base aparece igual en la base y en el caso, así que no dispara. Lo que dispara es lo que cambia entre dos corridas del mismo repo. Los riesgos de capas de memora-ecc siguen siendo defectos reales del producto, y no son la causa.

**Hechos medidos.**
- `detecta_dimension` (`src/oracle/calibra.rs`) cuenta como detección cualquiera de tres cosas: un hallazgo con id nuevo, un `valor` menor o un ítem con nivel menor. Un hallazgo del juez tiene como id el hash de dimensión, archivo y cita.
- Con el mismo commit, el juez dio Arquitectura 0,567 y 0,467, y Diseño 0,179 y 0,385 (medición del juez nuevo).
- En la ventana de la calibración de memora, `deepseek-v4-pro` hizo 66 llamadas, con 3 reintentos, por 6,4374 USD. Son 63 evaluaciones, a 0,102 USD cada una.
- Las 27 llamadas de Haiku con `sin_clave` no son de la calibración. La última ocurrió antes de su inicio, y costaron 0 USD.
- Falsas alarmas medidas en 10 limpios: Arquitectura 10, Diseño 10, Completitud 7 y Estructura 4.
- En Diseño, 4 de esas 10 tienen `sin_veredicto` por cobertura. Cuentan como detectadas, y las otras 6 salen de la regla.
- `calibracion.json` no guarda veredictos, y `.casos.jsonl` no guarda qué disparó cada detección. Por eso no se puede leer qué dijo el juez en cada caso limpio.
- memora-ecc tiene tests inestables con tasas de 84 % y 80 %: `test_client_id` y `test_bgsave_no_bloquea`.

**Causas, con su certeza.**
- **Arquitectura y Diseño (especificidad 0): causa probable, por el código y la varianza medida.** Una sola muestra del juez en la base se compara con otra muestra en cada caso. Cada corrida cita otras líneas, así que casi siempre aparece un id nuevo. El instrumento mide la varianza del juez, no el defecto.
- **Estructura (0,6): la misma causa, con menos peso.** Solo 1 de sus 4 ítems es del juez.
- **Completitud de memora (0,3): no es el juez, porque Completitud no lo usa.** Esto corrige la lectura del informe. La causa probable es el desempate 1 del informe 59, que aprobé: un test estable en la base que pasa a inestable cuenta como detección. Con 5 repeticiones, `test_client_id` (84 %) sale estable en la base cerca de 4 veces de cada 10. Después, en un caso limpio, se mezcla y dispara. El 7 de 10 medido es coherente con esa cuenta. brasa no tiene esos tests, y da 1,0.
- **Pruebas de memora (10 no aplicables): sin causa probada.** La hipótesis: los mutantes muertos de la muestra no tienen un `#[test]` con asserts de una línea que llame a su función. El caso no aplicable no deja renglón en `.casos.jsonl`, así que hoy no se puede ver.
- **Funciona (exit 2): no es un defecto.** Ningún `Defecto` apunta a Funciona, y la sección "Validation" del plan tampoco la calibra. Funciona queda fuera de `global` si no se calibra, y el objetivo del usuario pide calibrar: la Tarea 63 suma un defecto.
- **Brasa pruebas (7,9 horas): hecho.** Son 10 casos con defecto, y cada uno corre `cargo-mutants` entero sobre la muestra. Los limpios reusan la mutación de la base.
- **`target_reusado` solo en Completitud: hecho conocido.** Es la decisión del hallazgo 13: clippy y la mutación compilan en frío.

## Desempates técnicos 33 (Opus, 2026-11-05)

**Qué se arregla en Root.** La regla de detección y la ficha de los casos. El evaluador real no cambia: estos cambios tocan solo cómo la calibración mide.

1. **Dimensiones con juez (Estructura, Arquitectura y Diseño).** Un caso detecta solo si pasa una de estas tres cosas:
   - aparece un hallazgo nuevo de una sonda, o baja el nivel de un ítem de sonda;
   - el juez cita el archivo del defecto a 5 líneas o menos de su lugar;
   - el ítem que el defecto apunta baja 2 niveles o más frente a la base.
   Un hallazgo del juez con id nuevo no cuenta por sí solo, y `valor` tampoco. Razón: un cambio de un nivel y una cita nueva son ruido medido del juez.
2. **Regla simétrica en los limpios.** Cada caso limpio usa el defecto y el lugar del caso con defecto que lo precede, y aplica las mismas tres condiciones. Razón: si no, la especificidad sale buena por construcción.
3. **Tests con ruido (Completitud y Funciona).** Después de correr todos los casos, Root arma el conjunto de tests que variaron entre la base y los casos limpios. Esos tests no cuentan para detectar. Esto cambia el desempate 1 del informe 59. Razón: la tasa real de un test no se conoce con 5 repeticiones de la base, y los casos limpios ya pagaron esa medición.
4. **Limpio sin veredicto.** Deja de contar como detección. La fila suma `limpios_sin_veredicto`. Una dimensión queda calibrada solo si esos casos son como mucho el 20 % de los limpios y si su exactitud llega a 0,9. Esto ajusta el desempate 41 de "Desempates técnicos 11". Razón: una falla del instrumento se informa aparte y no se mezcla con una falsa alarma, pero sigue impidiendo calibrar.
5. **Motivo en cada caso.** `.casos.jsonl` guarda qué disparó cada detección, los niveles del juez y una línea por caso no aplicable, con su motivo.
6. **Limpios compartidos del juez.** Un caso limpio mide la misma copia de la base para las tres dimensiones con juez. Su resultado se reusa entre las tres calibraciones de la tarea. Razón: una llamada al juez puntúa los 10 ítems de las tres, y esto evita 20 llamadas por tarea.
7. **Defecto para Funciona.** `ComportamientoRoto` cambia una comparación de código de producción (`==` por `!=`), compila y hace fallar tests. Apunta a Funciona. Razón: sin defecto, Funciona nunca entra a `global`.
8. **Muestras del juez.** No se suman. El desempate de "Desempates técnicos 22" dijo que la calibración decidiría. Primero se mide la regla nueva: si la especificidad queda bajo 0,9, el planner propone 3 muestras por caso.

**Propiedades reales del repo base (no se arreglan).** Los riesgos de capas de memora-ecc, sus tests inestables y sus defectos de FIFO y de spin son del producto. La varianza del juez en la evaluación real sigue existiendo, y `global` la excluye mientras la dimensión no quede calibrada.

**Tiempo de brasa pruebas.** No se repite: su regla de detección no cambia, y dio 1,0 / 1,0. Esa fila queda. Para memora, que sí se repite, un caso con `AssertBorrado` corre `cargo-mutants` solo sobre el archivo del mutante elegido. Razón: borrar un assert no puede matar otros mutantes, así que la proporción solo puede bajar por ahí. La detección da lo mismo con mucho menos tiempo.

**Costo de volver a medir.** La medición de memora dio 0,102 USD por evaluación, con reintentos incluidos. El plan usa 0,11 USD como margen. El costo de brasa con el juez no está medido.
- Por tarea: 30 casos con defecto, 10 limpios compartidos y 1 base, o sea 41 evaluaciones, cerca de 4,5 USD.
- Dos tareas: cerca de 9 USD. Quedan unos 3,6 USD de margen sobre 12,6123 USD.
- Completitud, Funciona, Pruebas, Seguridad y Entrega no llaman al juez. Su gasto es casi 0.
- Sin los limpios compartidos, cada tarea cuesta cerca de 6,44 USD, y dos tareas pasan los 12,6123 USD. Por eso el desempate 6 es obligatorio.
- Si memora pasa de 5,5 USD, el orquestador frena antes de brasa y avisa al planner.

**Qué se vuelve a calibrar.** Todo salvo brasa pruebas y Eficiencia. Las dimensiones sin juez no gastan API. Pruebas de memora corre primero, porque tarda 3 minutos y ahora deja el motivo de sus no aplicables.

**Riesgos de la medición nueva.**
- La sensibilidad 1,0 de Arquitectura y Estructura tampoco vale, porque el ruido también disparaba en los casos con defecto. La sensibilidad puede bajar con la regla nueva.
- Diseño tuvo 4 de 10 limpios sin veredicto. Si eso se repite, Diseño no queda calibrada aunque la regla funcione.
- Si una dimensión no llega a 0,9, `global` la excluye y el planner trae el dato. Solo entonces se discute cambiar la promesa del evaluador.

**Informe 62.**
1. **`espera_vigencia` en el trait.** Queda.
2. **Intervalo cero.** Queda: falla cerrado.
3. **Aviso mientras espera.** Queda: el `Gate` ya lo registra.
- **Corrección de `refreshTokenExpiresAt`.** Queda.

### Tarea 63: Regla de detección de la calibración
- **Action**: los desempates 1 a 7, y el `cargo-mutants` acotado de `AssertBorrado`.
- **Archivos**: `src/oracle/calibra.rs`, `src/oracle/calibra/`, `src/oracle/mutacion.rs`.
- **Arranca**: ya, desde la punta de `claude/root-e7813e`, con `ROOT_SIN_PODMAN=1`.
- **Brief**: `docs/builds/brief-local-63.md`.

## Orden final del hito 1 (2026-11-05)

1. **Mezcla de la 62.** Espera su prueba con podman y la orden del usuario.
2. **Corrida real de memora (Tarea 16), ahora, antes de arreglar el evaluador.** Razón: la calibración está parada, así que hay una ventana sin carga. La Tarea 63 cambia solo cómo mide la calibración, no `evaluar`, así que la fila de la corrida sigue valiendo.
3. **Tarea 63**, en paralelo con el paso 2, sin podman. Después, su mezcla con la orden del usuario.
4. **Calibración otra vez**: primero Pruebas de memora, después el resto de memora y al final brasa. El tope es `--tope-usd 11,6`: el saldo menos 1 USD de margen, si el saldo sigue en 12,6123 USD.
5. **Comparación de `global`.**
6. **Validez automática** (Tarea 12b) y **cierre** (Tarea 47).
7. **Borrado de memora y brasa** (decisión 61).

No hay preguntas nuevas para el usuario.

## Vuelta del 2026-11-05 (tarde): Tarea 63 construida y revisada

**Estado.**
- La Tarea 62 pasó con podman en 758091b: 2413 verdes y 0 fallas. Su mezcla espera la orden del usuario.
- La Tarea 63 terminó por 3,35 USD en `laptop/tarea-63` (96f2207), sobre 8cc1a62. Sin podman da 2465 verdes.
- El punto 5 quedó en `laptop/tarea-63-p5` (e575fd0). Sin podman da 2469 verdes.
- La calibración no corre. Quedan 12,6123 USD de los 25 del usuario.

## Revisión del código de la Tarea 63 (Opus, sobre e575fd0)

**Cumple lo pedido.**
- Desempate 1: `src/oracle/calibra/regla.rs:46-49`.
- Desempate 2: `src/oracle/calibra.rs:1497-1524`.
- Desempate 3: `src/oracle/calibra/estables.rs:53-68` y `src/oracle/calibra.rs:1537-1544`.
- Desempate 4: `src/oracle/calibra.rs:838-841` y `src/oracle/calibra.rs:150-153`.
- Desempate 5: `src/oracle/calibra.rs:1348-1350` y `src/oracle/calibra.rs:1356-1373`.
- Desempate 7: `src/oracle/calibra/inyectar.rs:855-876`.

**Falla 1, los limpios compartidos no sirven con el evaluador real (bloquea medir).**
- `Partes::mide` solo acepta la dimensión calibrada (`src/oracle/mod.rs:268-271`).
- Las otras dimensiones salen con `sin_veredicto` "no medida en esta calibración" (`src/oracle/mod.rs:1029`).
- `Compartidos::leer` solo pide que el puntaje exista (`src/oracle/calibra/compartidos.rs:48-50`). Ese puntaje existe, sin veredicto.
- Efecto: Diseño y Estructura reusan 10 limpios sin veredicto, sin evaluar. Su especificidad sale 1,0 falsa.
- La compuerta del 20 % las deja sin calibrar. Los 20 casos con defecto de esas dos dimensiones gastarían cerca de 2 USD en filas inútiles.
- El test no lo ve, porque su evaluador falso devuelve puntajes de las dos dimensiones (`src/oracle/calibra.rs:2760-2768`).

**Falla 2, `AssertBorrado` acotado compara poblaciones distintas (bloquea medir Pruebas).**
- Este error nació en mi desempate, no en el builder. Dije que acotar daba lo mismo, y me equivoqué.
- La base mide los primeros `mutantes_max` del repo. El caso mide solo un archivo (`src/oracle/mutacion.rs`, `acotado_de_archivo`).
- `motivo_de_deteccion` compara `valor` y ids nuevos (`src/oracle/calibra.rs:1607-1621`).
- Un archivo con menos mutantes muertos que el repo da `valor_menor` sin defecto real.
- Un mutante vivo que la muestra de la base no incluía da un id nuevo. Ambos inflan la sensibilidad.
- Lo correcto: detecta solo un mutante que la base mató y que el caso deja vivo.

**Diferencias menores.**
- La clave no cubre modelo, prompt ni `tarea.toml` (`src/oracle/calibra/compartidos.rs:40`).
- `detecta_item` sigue la regla vieja (`src/oracle/calibra.rs:1650-1655`). Ninguna decisión lee esa métrica, pero `calibracion.json` la muestra.
- `regla.rs:105` amplía la cita al archivo entero con línea 0. Hoy solo `ModuloBorrado` usa línea 0, y es de Completitud, sin juez. No afecta.

**Corrección a mi costo anterior.** Cada dimensión evalúa su propia base (`src/oracle/calibra.rs:651`). Son 3 bases por tarea, no 1. Una tarea cuesta 43 evaluaciones, cerca de 4,7 USD a 0,11 USD.

## Desempates técnicos 34 (Opus, 2026-11-05)

**Preguntas del informe 63.**
1. **Mezclar `laptop/tarea-63-p5`:** sí. La línea de `src/oracle/mod.rs` es un literal de test que el esquema nuevo exige. Autorizo esa excepción.
2. **Modelo en la clave:** sí, más el hash del prompt y el de `tarea.toml`. Un cache de otro juez mediría a otro evaluador.
3. **Reuso en una segunda calibración:** solo con la misma semilla. La semilla entra en la clave.
4. **Limpio sin defecto anterior aplicable:** queda como lo decidió el organizador. La fila ya cuenta `no_aplicable`, y el mínimo de 5 defectos protege la medición.
5. **Definición de ruidoso:** queda. Más de un estado entre la base y los limpios.
6. **Doc de `motivo`:** queda.

**Límites de la sección 3.**
- **Clave sin modelo ni prompt:** se corrige en la Tarea 64.
- **Hilo local de `con_archivo_acotado`:** queda. Revisé con grep, y ni `evaluar` ni las sondas abren hilos. La Tarea 64 suma una prueba por `evaluar` real, que se pone roja si eso cambia.
- **`--file` sin `cargo-mutants` real:** no pide un test nuevo. Pruebas de memora tarda minutos y no gasta API, así que es la prueba real. Si `--file` falla, el caso sale sin veredicto y se ve.
- **`detecta_item`:** se corrige en la Tarea 64, para que la fila no muestre un número viejo.
- **`ComportamientoRoto` sin repo real:** no pide un test nuevo. Calibrar Funciona no gasta API. memora-ecc tiene 69 líneas con ` == ` en `src/`, sin filtrar.

**Varianza con limpios cacheados.**
- Dentro de una corrida no se pierde nada: los 10 limpios son 10 muestras independientes del juez.
- Cada dimensión evalúa además su propia base, que es otra muestra.
- Las tres dimensiones comparten esas 10 muestras. Sus errores quedan correlacionados, pero cada una mide sus propios ítems. La fila de cada una sigue valiendo.
- Sí se pierde la repetición entre corridas: la misma semilla devuelve los mismos limpios, y un acuerdo entre corridas no probaría nada.
- Importa para la validez solo si alguien cita esa estabilidad. La Tarea 12b deja "acuerdo" como "sin medir", así que hoy no cambia ningún criterio.
- Para medirla basta otra semilla, que no reusa nada. Cuesta cerca de 1 USD por tarea en limpios. No entra en esta vuelta.
- Cada renglón suma `limpio_reusado`, para que el dato quede visible.

**Orden entre dimensiones.** Arquitectura corre primero en cada tarea, y Estructura y Diseño después. Razón: dos procesos a la vez no comparten limpios, y el costo sube cerca de 2 USD.

**Tope.** `--tope-usd` cuenta el gasto desde el inicio de cada invocación (`src/oracle/calibra.rs:1088` y `src/oracle/calibra.rs:722`). Por eso cada invocación usa el saldo de ese momento menos 1 USD, no un número fijo.

**Costo otra vez.** Son dos tareas, a cerca de 4,7 USD cada una: cerca de 9,4 USD sobre 12,6123 USD. Quedan cerca de 3,2 USD. La regla de frenar si memora pasa de 5,5 USD sigue.

### Tarea 64: Limpios compartidos reales y Pruebas comparable
- **Action**: las correcciones de la revisión de la Tarea 63.
- **Archivos**: `src/oracle/calibra.rs`, `src/oracle/calibra/`, `src/oracle/mod.rs` (solo `Partes::mide` y sus tests), `src/oracle/juez.rs` (solo la huella del prompt).
- **Arranca**: ya, desde `laptop/tarea-63-p5` (e575fd0), con `ROOT_SIN_PODMAN=1`. No espera la mezcla de la 62.
- **Razón para no esperar**: los cambios de la 62 en `src/oracle/mod.rs` caen en las líneas 44 y 1677, y en tests. La 64 toca la línea 268.
- **Brief**: `docs/builds/brief-local-64.md`.

## Orden final del hito 1 (2026-11-05, tarde)

1. **Tarea 64**, ahora, en paralelo con los pasos 2 y 3.
2. **Prueba con podman de 63, p5 y 62**: sigue, porque detecta choques entre ellas. No habilita mezclar la 63.
3. **Mezcla de la 62**, con la orden del usuario. Después, la **corrida real de memora (Tarea 16)**.
4. **Prueba con podman de 63, p5 y 64 sobre la 62.** Después, la mezcla, con la orden del usuario.
5. **Calibración**, cuando la corrida real termine. Nunca al mismo tiempo. El orden es:
   - memora: Pruebas, Funciona, Completitud, Seguridad y Entrega, sin API;
   - memora: Arquitectura, después Estructura y Diseño;
   - brasa: Funciona, Completitud, Seguridad y Entrega, sin API;
   - brasa: Arquitectura, después Estructura y Diseño, si memora no pasó de 5,5 USD.
6. **Comparación de `global`.**
7. **Validez automática** (Tarea 12b) y **cierre** (Tarea 47).
8. **Borrado de memora y brasa** (decisión 61).

## Pregunta para el usuario (2026-11-05, tarde)

**P63: ¿Mezclar la Tarea 62 ahora?** La Tarea 62 pasó con podman y destraba la corrida real de memora.
- **a)** Mezclarla ahora. La corrida real arranca mientras se construye la 64. Recomendada.
- **b)** Esperar a la prueba conjunta de 63, p5 y 62. La corrida real se demora lo que tarde esa prueba.

## Vuelta del 2026-11-05 (noche): Tarea 64 construida y revisada

**Estado.**
- La Tarea 64 terminó por 1,83 USD en `laptop/tarea-64` (8c4d382), con push. Sin podman da 2495 verdes, 0 fallas y 3 ignorados.
- El orquestador corre con podman 64, 63, p5 y 62 juntas, en `~/.cache/root-wt/prueba64` (34dd370).
- P63 sigue sin respuesta. Quedan 12,6123 USD de los 25 del usuario.

## Revisión del código de la Tarea 64 (Opus, sobre 8c4d382)

**Las dos fallas quedaron resueltas, por lectura.**
- **Falla 1.** `Partes::mide` acepta el trío con `juez_diseno` (`src/oracle/mod.rs:270-279`). `Compartidos::leer` rechaza `NO_MEDIDA` (`src/oracle/calibra/compartidos.rs:81-86`). La prueba usa `evaluar_con` real, y el builder la vio en rojo revirtiendo `mide`.
- **Falla 2.** Un caso `AssertBorrado` usa solo `motivo_de_mutante_vivo` (`src/oracle/calibra.rs:1569-1570` y `src/oracle/calibra.rs:1651`). El hallazgo real coincide con los muertos: `parsear_outcomes` pasa `archivo` y `nombre` a `crear_hallazgo`, que no los cambia (`src/oracle/mutacion.rs:210-219`, `src/oracle/perfiles/mod.rs:82-84`).

**Busqué falsos que difieran del evaluador real. Ninguno bloquea medir.**
- **Route:** no tiene azar, y `root.toml` fija `juzga` en `deepseek-v4-pro`. La clave no cambia entre dimensiones.
- **Repo bueno:** `.pytest_cache/` entra en la versión del repo, porque `SALTADAS` solo saltea `target` y `.git`. El oráculo corre con `no:cacheprovider` sobre una copia, así que el hash no cambia entre invocaciones.
- **`Veredicto`:** no tiene campos con `serde(skip)`, así que el cache no pierde datos.
- **`calibracion.json`:** `Fila` no usa `deny_unknown_fields`. Un binario sin la 63 lee las filas nuevas.

**Límites que quedan, sin corrección antes de medir.**
- **Falla pasajera congelada.** `leer` reusa cualquier `sin_veredicto` que no sea `NO_MEDIDA`. El test llama "por cobertura" a un "juez caído" (`src/oracle/calibra/compartidos.rs`, `leer_reusa_un_limpio_con_sin_veredicto_por_cobertura`). Un corte de red en un limpio pesa en las tres dimensiones. En 125 llamadas hubo 1 `red_cortada` y 3 errores de `respuesta`. La compuerta tolera 2 de 10. Si pasa, otra semilla lo resuelve.
- **Especificidad de Pruebas.** Los limpios reusan la mutación de la base, así que la especificidad da 1,0 por construcción. También vale para la fila de brasa. La fila mide sensibilidad y no mide mutantes inestables.
- **Mutante muerto por un test inestable.** En memora, un test con 84 % de pase puede "matar" un mutante al azar. Eso puede inflar la sensibilidad de Pruebas.
- **Nombre con línea.** `nombre` trae `archivo:línea:columna`. Si un assert borrado está antes del mutante en el mismo archivo, el nombre cambia y el caso no detecta. Es un falso negativo honesto, y la corrida real lo muestra.
- **Commit 4147e82 sin compilar sus tests.** Rompe un bisect, y no cambia el resultado. Queda.
- **Copia de la constante en `BAJA_ITEM_CON_JUEZ`.** Queda. Está comentada.

## Desempates técnicos 35 (Opus, 2026-11-05)

**Preguntas del informe 64.**
1. **`mutante_vivo` reemplaza las reglas viejas:** queda.
2. **`estables::permite` en la regla nueva:** queda. En Pruebas siempre da `true` (`src/oracle/calibra/estables.rs:17`), así que no decide nada.
3. **Coincidencia por archivo y nombre:** queda.
4. **`sin_juez` sin `root.toml`:** queda. El `ROOT_HOME` real tiene `root.toml`.
5. **Clave sin proveedor ni razonamiento:** queda para esta medición, porque `[ruteo.juzga]` no cambia. Si alguien cambia esa ruta, la calibración usa otra semilla.
6. **`target_reusado` de un limpio reusado:** queda en `false`, y `limpio_reusado` da el dato.
7. **Entrega con juez:** no se apaga.
   - El juez lee los hallazgos de Entrega (`src/oracle/mod.rs`, comentario de `partes_de`). Apagarla calibra otro instrumento, y la regla de la Tarea 48 lo prohíbe.
   - **Limpios de memora:** cerca de 4 s por evaluación. Entrega midió 88 s en 21 evaluaciones, con construcción en caché.
   - **Casos con defecto de memora:** cambian `src/`, así que `COPY src` invalida la capa y `cargo build --release` corre otra vez. Eso no está medido.
   - **Cota de esos casos:** Arquitectura midió 2538 s. Quitando cerca de 640 s de juez, quedan 190 s por defecto, con clippy incluido. Entrega suma como mucho 25 minutos por dimensión, hipótesis sin medir.
   - **brasa:** no tiene Dockerfile, así que no construye imagen.
   - **Reusar la Entrega de la base:** tampoco. Ahorra como mucho 25 minutos y pide otra tarea con otra prueba con podman.
8. **El trío sale junto:** queda, y sirve para la compuerta de cobertura de abajo.

**Tarea 65: no hace falta.** Nada de lo revisado invalida la medición.

**Pruebas y Funciona de memora con el binario de 34dd370: sí, con una condición.**
- Corren cuando termine la prueba con podman. El hallazgo 11 mostró tests que fallan con la máquina cargada, y la prueba en curso no debe ensuciarse.
- No gastan API. Corren sin claves, y así no pueden gastar.
- Sus filas valen después de mezclar: 34dd370 es el árbol que se mezcla.

**El orden con la corrida real cambia.** La calibración va primero, si la prueba con podman da verde. Razón: una no depende de la otra, P63 sigue abierta, y `global` de la corrida real usa las filas calibradas. Esto reemplaza el orden anterior, que era mío.

**Compuerta de costo, reemplaza la regla de 5,5 USD.** brasa no tiene costo medido, así que las dos Arquitecturas corren juntas y lo miden. Después, gasto real más 20 casos por tarea al costo medido debe entrar en el saldo menos 1 USD. Si no entra, el orquestador frena y consulta al planner.

**Compuerta de cobertura, nueva.** Diseño tuvo 4 limpios de 10 sin veredicto, y el límite es 2. Los limpios de Arquitectura ya traen el puntaje de Diseño y el de Estructura, sin costo extra. Si una dimensión tiene más de 2 de 10 sin veredicto, no corre en esa tarea y vuelve al planner. Eso ahorra cerca de 1,1 USD por dimensión.

**Línea base sin veredicto.** Si la base de Diseño queda sin veredicto, `calibrar` sale con error. Se repite el mismo comando hasta 2 veces. Cuesta cerca de 0,10 USD cada vez, y sus limpios siguen en el cache.

## Comandos de la calibración (2026-11-05)

Variables comunes:

```bash
BIN=<binario r00t de 34dd370>
MEM=~/Proyectos/bench-ecc-sdlc/memora-ecc
BRA=~/Proyectos/bench-ecc-sdlc/brasa-ecc
SIN="env -u DEEPSEEK_API_KEY -u ANTHROPIC_API_KEY"
CAL="oraculo calibrar --n 20 --semilla 42"
# TOPE = saldo en el momento de lanzar el grupo, menos 1 USD.
```

La máquina tiene 8 núcleos y 15 GB. Los corchetes marcan procesos que corren a la vez. Dentro de cada proceso, 4 casos corren a la vez y los contenedores van de a uno.

**Grupo 0, al terminar la prueba con podman. Valida `--file` y `ComportamientoRoto`. Sin API.**

```bash
$SIN $BIN $CAL --tarea memora --dimension pruebas  --repo $MEM &
$SIN $BIN $CAL --tarea memora --dimension funciona --repo $MEM &
wait
```

- **Se sigue si:** Pruebas tiene casos con `mutante_vivo:` y ningún `sin_veredicto` por `--file`, y Funciona tiene casos con motivo. Si no, vuelve al planner.

**Grupo 1, sin juez de diseño. Solo sigue con el Grupo 0 correcto.**

```bash
$SIN $BIN $CAL --tarea memora --dimension completitud --repo $MEM &
$SIN $BIN $CAL --tarea brasa  --dimension completitud --repo $BRA &
$SIN $BIN $CAL --tarea brasa  --dimension funciona    --repo $BRA &
( $SIN $BIN $CAL --tarea memora --dimension seguridad  --repo $MEM
  $SIN $BIN $CAL --tarea brasa  --dimension seguridad  --repo $BRA
  $BIN $CAL --tarea memora --dimension entrega   --repo $MEM --tope-usd $TOPE
  $BIN $CAL --tarea brasa  --dimension entrega   --repo $BRA --tope-usd $TOPE
  $BIN $CAL --tarea brasa  --dimension eficiencia --repo $BRA --tope-usd $TOPE ) &
wait
```

- Entrega lleva claves, porque su juez de configuración puede pedirlas. No gastó nada en memora.
- brasa no tiene Dockerfile, así que Entrega de brasa puede salir con defectos no aplicables. Ese resultado se informa.
- brasa pruebas no se repite, y memora eficiencia ya tiene su fila.

**Grupo 2a, con juez. Solo con la prueba con podman en verde.**

```bash
$BIN $CAL --tarea memora --dimension arquitectura --repo $MEM --tope-usd $TOPE &
$BIN $CAL --tarea brasa  --dimension arquitectura --repo $BRA --tope-usd $TOPE &
wait
```

Son 42 evaluaciones, cerca de 4,3 USD. Después se aplican las dos compuertas.

**Compuerta de cobertura, sin dinero.**

```bash
python3 - <<'PY'
import glob, json, os
for t in ("memora", "brasa"):
    fs = glob.glob(os.path.expanduser(f"~/.local/share/root/calibracion/limpios/{t}-*.json"))
    for d in ("estructura", "diseno"):
        sv = 0
        for f in fs:
            p = [x for x in json.load(open(f))["puntajes"] if x["dimension"].lower() == d]
            sv += bool(p and p[0].get("sin_veredicto"))
        print(t, d, "limpios", len(fs), "sin_veredicto", sv)
PY
```

**Grupo 2b, las dimensiones que pasaron las compuertas.**

```bash
$BIN $CAL --tarea memora --dimension estructura --repo $MEM --tope-usd $TOPE &
$BIN $CAL --tarea memora --dimension diseno     --repo $MEM --tope-usd $TOPE &
$BIN $CAL --tarea brasa  --dimension estructura --repo $BRA --tope-usd $TOPE &
$BIN $CAL --tarea brasa  --dimension diseno     --repo $BRA --tope-usd $TOPE &
wait
```

- `$TOPE` se calcula una vez y vale para los cuatro: cada proceso ve el gasto de los otros desde su inicio.
- Son 40 casos y cerca de 4,1 USD, si pasan todas las dimensiones.
- Si aparece un timeout de infraestructura por la carga, los procesos bajan a 2 por vez.

**Qué no puede ir en paralelo.** Arquitectura debe terminar antes de Estructura y Diseño de su tarea. La calibración no corre a la vez que una prueba con podman ni que la corrida real.

## Orden final del hito 1 (2026-11-05, noche)

1. **Prueba con podman de 34dd370**, en curso.
2. **Grupo 0.** Si da correcto, **Grupo 1**.
3. **Grupos 2a y 2b**, con el verde del paso 1.
4. **Mezcla de la 62 y de 63, p5 y 64**, con la orden del usuario (P63 y P64). Puede ocurrir en cualquier momento después del verde, porque es el mismo árbol.
5. **Corrida real de memora (Tarea 16)**, sobre la mezcla, sin calibración en curso.
6. **Comparación de `global`.**
7. **Validez automática** (Tarea 12b) y **cierre** (Tarea 47).
8. **Borrado de memora y brasa** (decisión 61).

## Preguntas para el usuario (2026-11-05, noche)

**P63 sigue abierta.** Con el orden nuevo, la respuesta destraba la mezcla, no la calibración.

**P64: ¿Mezclar 63, p5 y 64 cuando la prueba con podman dé verde?**
- **a)** Sí, sin volver a preguntar, junto con la 62. Recomendada: es el árbol que se prueba.
- **b)** No. El orquestador trae el resultado y espera otra orden.

## Vuelta del 2026-11-05 (mañana): Grupo 0 de la calibración

**Resultado, con el binario 34dd370, sin claves y con 0 USD.**
- `memora funciona` dio 0,9 / 1,0 en 5094 s. Los 9 detectados tienen motivo `valor_menor`. El falso negativo es el caso 16, `src/cmd/server.rs:222`. `ComportamientoRoto` quedó probado con el repo real.
- `memora pruebas` sigue sin sensibilidad, en 110 s. Los 10 casos con defecto dicen "sin candidato para inyectar". La especificidad es 1,0 con 10 limpios.
- La compuerta del Grupo 0 no pasó: no hay ningún caso con `mutante_vivo:`, y `--file` sigue sin probarse con la herramienta real.

## Diagnóstico de `assert_borrado` en memora (Opus, 2026-11-05)

**Hechos medidos, sin correr contenedores.**
- memora-ecc tiene 34 tests con 113 asserts de una línea, y ninguno de varias líneas. La forma de los asserts no es el problema: mi hipótesis anterior era falsa.
- El listado completo de memora trae 2263 mutantes en 16 archivos (`sondas-sniVzY`, 2026-10-30, mismo commit del repo).
- La muestra toma los primeros 60 (`src/oracle/mutacion.rs:438`). La corrida del 2026-11-03 (`sondas-Jq04pk`) midió 58 mutantes: 48 de `src/cmd/mod.rs` y 10 de `src/main.rs`. Son 2 archivos de 16.
- Esa corrida mató 23 mutantes. Porté `asserts_de_tests_que_llaman` a Python y lo corrí sobre los 23: ninguno tiene candidato.
- Sus funciones son `run`, `main`, `db`, `table`, `lookup`, `arity_ok`, `run_command` y `replay`. Solo las ejercita `tests/server.rs`, que habla por socket con el binario. Ningún test las llama por nombre.
- Los archivos siguientes sí tienen candidatos. En la corrida entera del 2026-10-30, tienen candidato 42 muertos de `src/util.rs`, 38 de `src/db.rs`, 24 de `src/persist.rs`, 14 de `src/config.rs` y 7 de `src/resp.rs`.
- Simulé 60 mutantes repartidos por todo el listado: tocan los 16 archivos. Solo 20 tienen resultado conocido, porque aquella corrida no llegó a los demás. De ellos, 9 son muertos, y 4 tienen candidato (`src/config.rs`, `src/db.rs`, `src/persist.rs` y `src/resp.rs`).

**Hipótesis, sin medir.**
- La corrida de hoy dio los mismos muertos que la del 2026-11-03. Hoy no se puede ver: la memoria de la base no queda en disco.
- Los 110 s de hoy contra 20 minutos del 2026-11-03 no prueban una falla. Aquella corrida tuvo una compilación de 385 s, y la mediana por mutante fue de 2 s de build y 2,5 s de test. La Tarea 65 deja el motivo visible.

**Causa.** El inyector no falla. Falla la muestra: mide el 2,6 % de los mutantes, en los 2 primeros archivos del orden de `cargo-mutants`. Esto es un defecto del evaluador real, no solo de la calibración. Hoy Pruebas de memora no dice nada de 14 archivos. brasa dio 1,0 porque sus primeros mutantes sí tienen tests que los llaman.

**El falso negativo de Funciona no pide cambios.** El `==` de `src/cmd/server.rs:222` solo evita repetir una clave en la lista de WATCH. Invertirlo casi no cambia la conducta visible, hipótesis por lectura. 0,9 / 1,0 da exactitud 0,95, y la dimensión llega al umbral.

## Desempates técnicos 36 (Opus, 2026-11-05)

1. **Pruebas no queda fuera de `global`, y el inyector no cambia.** Se arregla la muestra: los `mutantes_max` se reparten en forma pareja por el listado. Razón: dejarla fuera esconde un instrumento sesgado, y arreglar solo el inyector mide los mismos 2 archivos. El número de mutantes y el costo no cambian.
2. **Borrar asserts de `tests/server.rs`: descartado.** Daría 10 casos con la misma inyección, y la muestra seguiría sesgada.
3. **`--file` sale.** Un archivo con más mutantes que el máximo queda cortado, y el mutante elegido puede no entrar: `src/cmd/mod.rs` tiene 157. Un caso acotado corre el mismo listado y la misma muestra, filtrados por archivo con `-F`. `-F` ya corrió con la herramienta real el 2026-11-03.
4. **Esto corrige mi desempate del acotado una segunda vez.** La comparación ahora es exacta: el caso mide un subconjunto de la muestra de la base.
5. **Cómo se prueba con la herramienta real.** Un test con `cargo-mutants` real, en la suite con podman, y después Pruebas de memora, sin API. brasa no sirve: tarda horas por corrida, y su fila es del binario viejo.
6. **`--file` no bloquea nada,** porque ya no se usa.

**Qué pasa con cada grupo.**
- **Grupo 1 sigue ya con 34dd370.** Completitud, Funciona, Seguridad, Entrega y Eficiencia no corren la mutación (`partes_de`). La Tarea 65 no cambia sus filas. La fila de `memora funciona` también vale.
- **Grupos 2a y 2b esperan la Tarea 65.** El trío corre la mutación, y el juez lee sus hallazgos. Con la muestra vieja se calibraría otra entrada del juez, y el saldo no alcanza para repetir: 12,6123 USD contra cerca de 8,4 USD por pasada.
- **`brasa pruebas` se repite** con la muestra nueva, sin API. Esto revierte "brasa pruebas no se repite". Como cada caso mide pocos mutantes, debería tardar mucho menos que las 7,9 horas, hipótesis sin medir.

### Tarea 65: Muestra repartida de la mutación
- **Action**: la muestra pareja, el acotado por `-F`, los motivos separados y el test con la herramienta real.
- **Archivos**: `src/oracle/mutacion.rs`, `src/oracle/calibra.rs`, `src/oracle/calibra/`.
- **Arranca**: ya, desde `laptop/tarea-64` (8c4d382), con `ROOT_SIN_PODMAN=1`, en paralelo con el Grupo 1.
- **Brief**: `docs/builds/brief-local-65.md`.

## Orden final del hito 1 (2026-11-05, mañana)

1. **Grupo 1** con 34dd370, y **Tarea 65** en paralelo.
2. **Prueba con podman de 65, 64, 63, p5 y 62**, cuando el Grupo 1 termine.
3. **Pruebas de memora** con el binario nuevo. Se sigue si hay casos con `mutante_vivo:`.
4. **Grupo 2a**, con `brasa pruebas` en paralelo. Después, las dos compuertas, y el **Grupo 2b**. Los comandos son los de "Comandos de la calibración", con el binario nuevo.
5. **Mezcla**, con la orden del usuario (P63 y P64).
6. **Corrida real de memora (Tarea 16)**.
7. **Comparación de `global`.**
8. **Validez automática** (Tarea 12b) y **cierre** (Tarea 47).
9. **Borrado de memora y brasa** (decisión 61).

**P64 cambia de alcance.** Ahora cubre 63, p5, 64 y 65.

## Vuelta del 2026-11-05 (tarde 2): Grupo 1 y Tarea 65

**Grupo 1, binario 34dd370, 0 USD.**

| Tarea | Dimensión | Sens. / espec. | Segundos |
|---|---|---|---|
| memora | seguridad | 1.0 / 1.0 | 54 |
| memora | entrega | 1.0 / 1.0 | 179 |
| memora | completitud | 1.0 / 0.3 | 1465 |
| brasa | seguridad | 1.0 / 1.0 | 66 |
| brasa | completitud | 1.0 / 1.0 | 1939 |
| brasa | funciona | 0.3 / 1.0 | 4406 |
| brasa | eficiencia | no aplica | 0 |
| brasa | entrega | exit 3: "la línea base no tiene veredicto de entrega" | 1 |

**Tarea 65.** Terminó por 1,11 USD en `laptop/tarea-65` (9699f70), con 2505 verdes sin podman. El orquestador la prueba con podman en d4b9679, junto con 64, 63, p5 y 62.

## Diagnóstico del Grupo 1 (Opus, 2026-11-05)

### Memora completitud 0,3

**Hechos, por código y datos.**
- Un test inestable crea un hallazgo de Completitud, con su id como `cita` (`src/oracle/requisitos.rs:112-146`). En `src/oracle/` no hay otra fuente de hallazgos de Completitud fuera de tests.
- `motivo_de_deteccion` mira ese hallazgo primero (`src/oracle/calibra.rs`, `hallazgo_nuevo`).
- `estables::permite` tiene tres salidas. La tercera es `hallazgo_nuevo` (`src/oracle/calibra/estables.rs:22` y `:112-118`), y no consulta `ruidosos`.
- Entonces, un test nuevo inestable en un limpio pasa el filtro por esa puerta. El punto 4 de la 63 tapó solo la puerta de los tests.
- El limpio 7 tiene los mismos niveles que la base, y aun así detecta. El limpio 19 tiene los mismos niveles, y no detecta. La diferencia no está en los niveles.
- Los limpios detectados cambiaron entre corridas: `[3,5,7,11,13,15,19]` antes y `[1,3,5,7,9,11,13]` ahora. Ambas dan 7 de 10, y el orden no se repite.

**Corrección mía.** El test de la 63 usó un veredicto falso sin ese hallazgo, y mi revisión no lo vio. Yo busqué fuentes de hallazgos, y mi búsqueda no miró `requisitos.rs`.

**Hipótesis, sin medir.** El test es `test_client_id` (84 % de pase, R4). R4 ya vale 1 en la base, así que su inestabilidad no cambia el nivel. Con 5 repeticiones, sale inestable cerca de 6 veces de cada 10.

**Cómo se confirma sin API.** La Tarea 66 guarda `hallazgo_nuevo:<cita>`, y Completitud de memora corre otra vez en cerca de 25 minutos.

### Brasa funciona 0,3

**Hechos, por lectura de los inyectados.**

| Línea | Casos | Qué hace el `==` |
|---|---|---|
| `src/app.rs:395` y `:396` | 4 | Eventos de ratón de la ventana. |
| `src/render.rs:538` | 1 | Decide si reusa los targets. Invertirlo rehace los targets, y la imagen no cambia. |
| `src/limits.rs:62` | 1 | Decodifica `%` en una URI. |
| `src/gltf_load.rs:225` | 1 | Usa las normales del archivo si el tamaño coincide. |

- Los 3 detectados están en validaciones y en el bucle de mipmaps: `src/render.rs:701`, `src/scene.rs:538` y `src/render.rs:1097`.

**Hipótesis, sin medir.** La suite corre sin ventana, así que no ejercita `src/app.rs:395-396`. Eso explicaría 4 de los 7 fallos. `src/render.rs:538` parece equivalente. `src/limits.rs:62` y `src/gltf_load.rs:225` quedan sin explicar.

**Decisión: 0,3 mezcla dos cosas, y se separan.**
- Un defecto en código que ningún test ejecuta no prueba que Funciona sea ciega en lo que promete. Funciona mide lo que la suite especifica.
- Un defecto en código ejecutado que pasa los tests sí mide algo real, y debe contar en contra.
- El inyector debe elegir código ejecutado por la suite. No hace falta instalar una herramienta de cobertura.
- `src/render.rs:538` seguirá contando como fallo. Es honesto: esos mutantes equivalentes existen.

### Brasa entrega

**Hechos.**
- `tarea.toml` de brasa no tiene `[entrega]`, y el repo no tiene Dockerfile.
- `elegir_dockerfile` da `SinDockerfile`, y la construcción termina sin medir ningún ítem (`src/oracle/entrega.rs:359-365`).
- Entonces la base queda sin veredicto, y `calibrar` sale con 3 (`src/oracle/calibra.rs:656-661`).

**Corrección mía.** Dije que brasa no construye imagen, y era cierto. No vi que eso deja la base sin veredicto.

**Decisión.** Es el mismo caso que Eficiencia sin `[eficiencia]`: la tarea no aplica la dimensión. La calibración escribe una fila "no aplica" y sale con 0. La evaluación real no cambia. Los ítems de Entrega no bloquean, así que no deciden `Listo` (decisión 48).

## Desempates técnicos 37 (Opus, 2026-11-05)

**Informe 65.**
1. **Prefijo `<archivo>:`:** queda. La guarda con la herramienta real lo comprueba.
2. **Muestra sin nombres del archivo:** sigue sin veredicto, y el caso con defecto cuenta como falso negativo. Solo pasa si el borrado corre el listado, y debe verse. Si Pruebas de memora muestra ese motivo, vuelve al planner.
3. **Texto de los motivos:** queda.

**Límite nuevo, se informa.** El conjunto `ruidosos` sale de los limpios, así que en Completitud y Funciona la especificidad casi no puede bajar por tests. Una fila con 1,0 no prueba estabilidad. La Tarea 66 guarda la lista de `ruidosos` en la fila, para que se vea cuántos tests quedaron fuera.

**Grupos 2a y 2b: pueden seguir.**
- Siguen con el binario de d4b9679, si la prueba con podman da verde y Pruebas de memora pasa su compuerta.
- Las partes del trío no incluyen `oculto` ni `contrato` (`src/oracle/mod.rs`, `partes_de`). El juez no lee las filas de la calibración.
- La Tarea 66 no toca `regla.rs`, y `estables::permite` da `true` en el trío (`src/oracle/calibra/estables.rs:17`). Sus filas siguen valiendo después de mezclar la 66.

**Filas que la Tarea 66 invalida, todas sin API.**
- `memora completitud`, por la regla. Cerca de 25 minutos.
- `memora funciona` y `brasa funciona`, por la selección de líneas. Cerca de 85 y 75 minutos, más las sondas nuevas.
- `brasa entrega`, que hoy no tiene fila.
- `brasa completitud` queda: su especificidad ya es 1,0 y no tiene inestables nuevos.

### Tarea 66: Hallazgos de tests con ruido, líneas ejercitadas y Entrega no aplicable
- **Action**: los cuatro puntos del brief.
- **Archivos**: `src/oracle/calibra.rs`, `src/oracle/calibra/`.
- **Arranca**: ya, desde `laptop/tarea-65` (9699f70), con `ROOT_SIN_PODMAN=1`.
- **Brief**: `docs/builds/brief-local-66.md`.

## Orden final del hito 1 (2026-11-05, tarde 2)

1. **Tarea 66**, ahora, en paralelo con todo lo que sigue.
2. **Prueba con podman de d4b9679.** Después, **Pruebas de memora**. Se sigue con `mutante_vivo:`.
3. **Grupo 2a** con `brasa pruebas` en paralelo. Después, las dos compuertas y el **Grupo 2b**.
4. **Prueba con podman de 66 sobre todo lo anterior**, cuando no corra la calibración.
5. **Filas sin API con el binario de la 66:** `memora completitud`, `memora funciona`, `brasa funciona` y `brasa entrega`.
6. **Mezcla**, con la orden del usuario (P63 y P64).
7. **Corrida real de memora (Tarea 16)**.
8. **Comparación de `global`.**
9. **Validez automática** (Tarea 12b) y **cierre** (Tarea 47).
10. **Borrado de memora y brasa** (decisión 61).

**P64 cambia de alcance otra vez.** Ahora cubre 63, p5, 64, 65 y 66.
