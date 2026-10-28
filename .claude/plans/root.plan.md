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
  
  Si P12b queda en a), los dos huecos se cierran: Root rechaza esas fuentes antes de cualquier comando con red. Si P12b queda en b), Opus rediseña el filtro. Hasta la respuesta, el código de la 6b no cambia y los dos huecos siguen abiertos.
- **Validate**: `cargo test --test descarga`:
  - Un repo con una dependencia de crates.io fuera de la imagen compila en la evaluación sin red.
  - Un `Cargo.lock` con `git+` da `SinVeredicto` y no baja nada.
  - El contenedor de descarga no ve `/oraculo`.
  - La segunda evaluación del mismo repo no vuelve a bajar nada: usa la caché.
- **Orden**: va después de las Tareas 7 y 8. La Tarea 7 sigue con la imagen precargada de la 7a.

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
- **Action**: `oracle/entrega.rs` clona el repo en un contenedor limpio y corre `[entrega].instalar` y `[entrega].correr`. Verifica `[entrega].archivos`. Construye el `Dockerfile` del proyecto y lo arranca, y verifica que corre sin root. Hoy solo verifica el Dockerfile en forma estática: construirlo necesita red, y eso espera a P17. Después corre cada `[[escenario]]` contra el producto levantado. Un escenario es un flujo real de usuario, como conectar un cliente, guardar un valor, reiniciar y leerlo. Es lo que un PM prueba a mano antes de aprobar.
- **Validate**: `cargo test --test entrega` (un repo sin README da un hallazgo; un Dockerfile que corre como root da un hallazgo; un escenario que sale con código 1 deja su requisito en `Parcial`)

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

### Tarea 16: Runner
- **Action**: `r00t corre` corre primero `r00t goal verificar` y se detiene si el GOAL no está autorizado. Después copia el bootstrap a un directorio nuevo y corre `claude -p` por rol con `route`. Usa `ECC_DISABLED_HOOKS` y los permisos que niegan `oraculos/`. Registra `PlanAprobado` y `Humano`. Al final evalúa y agrega la `Fila`.
- **Mirror**: `correr-ecc-rol.sh`.
- **Validate**: `r00t corre --tarea memora --config ecc-haiku` deja una fila en `runs.jsonl` con un veredicto de nueve dimensiones, un `Informe` y un resumen.

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
| Un paquete malicioso entra por la fase de descarga | Baja | La descarga no compila. El build corre después sin red, con el repo en solo lectura y sin acceso a `ROOT_HOME`, salvo la caché. Las fuentes `git+` se rechazan (P12). `cargo audit` marca los paquetes con CVE (Tarea 9). |
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
- **P15, Colab.** No frena nada. Sin aprobación, los tests de entorno quedan `Pendiente`.
- **P16, herramientas en la imagen.** Frena la calibración (Tarea 12) de seguridad, estructura, pruebas y entrega. No frena 14, 15, 17 ni 18.
- **P17, construir el Dockerfile.** Frena la parte dinámica de la 9d. No frena nada más.
- **P18, aprender en los roles fijos.** No frena 17, 18 ni 19. Si la respuesta es b) o c), agrega una tarea chica sobre Route o Aprende.

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

**Preguntas abiertas (sin responder)**
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

