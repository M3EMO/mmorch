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
- **Validate**: `r00t oraculo evaluar --tarea memora --repo .../memora-sdlc` marca `Ausente` los módulos que el SDLC no construyó (el SDLC construyó 2 de 9 módulos, según `docs/contexto/mapa.md`). memora-ecc queda con todos `Cumplido`.

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
  - Precios por defecto (G1f y decisión 44): `deepseek-flash` 0,14 y 0,28 USD, `deepseek-v4-pro` 1,74 y 3,48 USD, y `claude-haiku-5-5` 0,10 y 0,50 USD por millón de tokens. Cada valor cita su origen en un comentario: `mmorch/config.py` para DeepSeek, y la referencia de la API de Anthropic del 2026-10-06 para Haiku. Un `[precios]` de `root.toml` reemplaza el default.
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
