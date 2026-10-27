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
pub enum Cumplimiento { Cumplido, Parcial, Ausente, SinVeredicto }
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
`Estado::Verde` exige tres condiciones: contrato completo, oculto sin regresiones y `global` mayor o igual a `umbral_verde` de `root.toml`.

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

### Tarea 7: Tareas memora y brasa
- **Action**: crear `oraculos/memora` y `oraculos/brasa` desde `~/Proyectos/bench-ecc-sdlc/bootstrap/` y `.scratch/ecc-vs-sdlc/oraculo/`. Construir `imagenes/oraculo-rust`: los dos proyectos son Rust y sus oráculos son pytest de caja negra (`ORACULO_REPO`, ver `oraculo/memora/conftest.py`). brasa necesita un adaptador de CPU (lavapipe) en la imagen: ver `~/Proyectos/bench-ecc-sdlc/probe-gpu/`. Los oráculos se copian a `ROOT_HOME`, nunca al repo de Root.
- **Requisitos**: Opus redacta `[[requisito]]` y `[[escenario]]` de cada tarea desde la spec. Cada sección de la spec da al menos un requisito. Los tests ocultos `test_m<n>_*.py` ya siguen los módulos de la spec, así que cada requisito apunta a sus archivos. El usuario aprueba la lista una vez (pregunta P3). La lista es material oculto: vive en `ROOT_HOME`, no en el repo.
- **Validate**: `r00t oraculo evaluar --tarea memora --repo ~/Proyectos/bench-ecc-sdlc/memora-ecc` da contrato 61/61 y oculto 48/48. brasa-ecc da 53/53 y 15/16.

### Tarea 8: Proveedores
- **Action**: DeepSeek (API compatible con OpenAI) y Anthropic Messages. Reintentos y breaker como en `providers.py`. El costo sale de los precios de `root.toml`. Cada llamada registra un `Evento`. Las claves salen de variables de entorno y nunca entran a un log.
- **Validate**: `cargo test proveedores` con un servidor HTTP falso (429 reintenta; 3 fallas abren el breaker; la clave no aparece en `eventos.jsonl`)

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
  - **seguridad**: gitleaks, `cargo audit` (dependencias con CVE), conteo de `unsafe` y de `unwrap()`/`expect()` en código que no es de test. En Python: bandit.
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
  - **3 Cumplido**: todo `pruebas`, `bordes` y `escenarios` en verde.
  
  El puntaje de `Completitud` es `Σ peso·nivel / (3·Σ peso)`. Los requisitos salen de la spec, y la spec sale de una sesión `/grill-me` con el usuario (decisión 12). En memora y brasa, la spec ya existe en el banco.
- **Validate**: `r00t oraculo evaluar --tarea memora --repo .../memora-sdlc` marca `Ausente` los módulos que el SDLC no construyó (el SDLC construyó 2 de 9 módulos, según `docs/contexto/mapa.md`). memora-ecc queda con todos `Cumplido`.

### Tarea 9d: Entrega y escenarios de usuario
- **Action**: `oracle/entrega.rs` clona el repo en un contenedor limpio y corre `[entrega].instalar` y `[entrega].correr`. Verifica `[entrega].archivos`. Construye el `Dockerfile` del proyecto y lo arranca, y verifica que corre sin root. Después corre cada `[[escenario]]` contra el producto levantado. Un escenario es un flujo real de usuario, como conectar un cliente, guardar un valor, reiniciar y leerlo. Es lo que un PM prueba a mano antes de aprobar.
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
    - En el hito 1, Root no lanza sesiones en la nube, así que Route nunca elige `Via::Nube`. El tope solo se informa en `r00t observa uso`, y al llegar al 100% se registra un evento `Gate`.
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
  - Nube en 250 USD registra un evento `Gate` y no cambia la elección de Route.
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
r00t oraculo evaluar --tarea memora --repo ~/Proyectos/bench-ecc-sdlc/memora-ecc --json   # 61/61, 48/48
r00t oraculo evaluar --tarea brasa  --repo ~/Proyectos/bench-ecc-sdlc/brasa-ecc  --json   # 53/53, 15/16
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
| Un agente corre `r00t goal autorizar` por Bash | Baja | `autorizar` exige TTY; la sesión `claude -p` no tiene TTY |
| `GOAL.md` falta cuando arranca `r00t corre` | Media | `r00t corre` corre `r00t goal verificar` primero y sale con código 3 sin GOAL autorizado |

## Acceptance
- [ ] Las tareas 1 a 19 y sus variantes 5b, 9b, 9c, 9d, 9e, 12b y 13b están completas.
- [ ] La validación pasa.
- [ ] El Oracle reproduce 61/61 y 48/48 en memora-ecc, y 53/53 y 15/16 en brasa-ecc.
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
   - `umbral_wilson = 0.5`: se confirma. Con la cota inferior de Wilson al 90%, 3 éxitos de 3 dan ~0,54. Un modelo entra después de 3 corridas buenas y sale ante una mala racha.
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

