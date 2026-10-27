# Plan: Root

**Source PRD**: `.claude/prds/sistema-nuevo.prd.md`
**Selected Milestone**: 1 — Root: reemplaza el banco y el ruteo de mmorch
**Complexity**: Large
**Status**: APROBADO por el usuario el 2026-10-27. Lo construye Sonnet con builders Haiku en otra corrida.

## Summary

Root es un binario de Rust llamado `root`. Root evalúa el valor de un proyecto, rutea trabajo entre cupo y APIs, observa cada corrida y aprende de lo que registra.
El Oracle no se limita a contar tests. Juzga seis dimensiones: funciona, seguridad, eficiencia, estructura, arquitectura y diseño.
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

```toml
nombre = "memora"
lenguaje = "python"
imagen = "root-oraculo-python:1"        # imagen del contenedor de evaluación
containerfile = "bootstrap/Dockerfile"   # opcional: si existe, el Oracle construye la imagen desde el Dockerfile del proyecto
spec = "bootstrap/docs/SPEC.md"
[contrato]
cmd = "pytest -q -p no:cacheprovider --junitxml=/salida/contrato.xml /repo/tests"
[oculto]
cmd = "pytest -q -p no:cacheprovider --junitxml=/salida/oculto.xml /oraculo/oculto"
[[sonda]]                                # una sonda determinista por dimensión
dimension = "seguridad"
cmd = "bandit -r /repo -f json -o /salida/seguridad.json"
parser = "bandit-json"
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

`global` es la media ponderada de las dimensiones con `calibrada = true`. Si ninguna dimensión está calibrada, `global` es `None`.
`Estado::Verde` exige tres condiciones: contrato completo, oculto sin regresiones y `global` mayor o igual a `umbral_verde` de `root.toml`.

### Funciones públicas

```rust
// route
pub fn elegir(p: &Pedido, pol: &Politica, uso: &Uso) -> Eleccion;            // pura, sin I/O
// oracle
pub fn evaluar(repo: &Path, tarea: &Tarea, o: &OpcionesOraculo) -> Result<Veredicto, ErrorOraculo>;
pub fn calibrar(tarea: &Tarea, dim: Dimension, n: usize, semilla: u64) -> Result<Calibracion, ErrorOraculo>;
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
root route --rol <rol> --tipo <tipo> [--checkeable] [--tokens N]   -> Eleccion
root llama --rol <rol> --tipo <tipo> --prompt-file <f> --corrida <id> -> Respuesta
root oraculo evaluar --tarea <t> --repo <dir>                        -> Veredicto
root oraculo calibrar --tarea <t> --dimension <d> [--n 20] [--semilla 42] -> Calibracion
root observa claude --json <salida-de-claude-p> --corrida <id>
root observa evento --corrida <id> --tipo <humano|plan-aprobado|gate> [--detalle txt]
root observa resumen --corrida <id>                                  -> Resumen
root track agregar --fila <archivo.json> | listar [--tarea t] | comparar --a c1 --b c2 --tarea t
root aprende [--replay]                                              -> Politica | métrica de replay
root corre --tarea <t> --config <c>                                  -> Fila
root goal verificar | autorizar        # autorizar exige una TTY y una confirmación tecleada
root importar-mmorch --desde <metrics.jsonl>
root esquema <tipo>                    # imprime el JSON Schema del tipo
root mcp                               # servidor MCP por stdio
```

### Herramientas MCP (reemplazan el ruteo del MCP `mmorch`)

`root_route(rol, tipo, checkeable)`, `root_llama(rol, tipo, prompt)`, `root_evaluar(tarea, repo)` y `root_registrar(corrida, tipo, detalle)`. Cada herramienta devuelve el mismo JSON que su subcomando.

### Uso previsto por hito

- **Graft (hito 2)** pide el modelo de cada etapa con `root route`. Usa las sondas del Oracle como gates y registra cada etapa con `root observa evento`.
- **REA (hito 3)** registra cada paso con `root observa evento`. El Oracle verifica su evidencia con el mismo chequeo de `archivo:línea` que aplica a los hallazgos del juez.
- **CAB (hito 4)** define una tarea con `tarea.toml` y mide cada agente generado con `root corre`.
- **ECC (decisión a)** recibe sus modelos por rol desde `root route`. Sus hooks que filtran datos se apagan por defecto en `root corre`.

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
| `src/proveedores/{mod,deepseek,anthropic}.rs` | CREATE | Llamadas a las APIs |
| `src/route/{mod,politica}.rs`, `src/aprende.rs` | CREATE | Ruteo y aprendizaje |
| `src/corre.rs`, `src/mcp.rs`, `src/importar.rs` | CREATE | Runner, servidor MCP e importación |
| `imagenes/oraculo-python/Containerfile` | CREATE | Imagen de evaluación (pytest, bandit, radon, gitleaks, hadolint) |
| `tests/*.rs`, `tests/fixtures/` | CREATE | Integración |
| `GOAL.borrador.md` | CREATE (Opus, ya escrito) | Borrador del GOAL para que el usuario lo lea |
| `GOAL.md`, `GOAL.hash` | — (solo el usuario) | El usuario copia el borrador a mano y corre `root goal autorizar` |
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
- **Validate**: `cargo build && cargo clippy -- -D warnings && ./target/debug/root --help`

### Tarea 2: Config y tipos
- **Action**: `config.rs` resuelve `ROOT_HOME` y parsea `root.toml`. `tipos.rs` contiene todos los tipos de "Interfaces fijas". `root esquema <tipo>` imprime el JSON Schema.
- **Mirror**: los nombres de campo de `MetricEvent`.
- **Validate**: `cargo test tipos` (ida y vuelta JSON de cada tipo; `v` presente) y `root esquema veredicto | jq .`

### Tarea 3: GOAL con hash
- **Action**: `goal.rs` implementa `verificar` y `autorizar`. `autorizar` rechaza la ejecución sin TTY y pide teclear el hash. El builder no escribe `GOAL.md` ni `GOAL.hash`: el hook los bloquea. Los tests usan un GOAL de fixture en un directorio temporal.
- **Mirror**: `mmorch/goal.py:45-54`.
- **Validate**: `cargo test goal` (un GOAL alterado falla `verificar`; `autorizar` sin TTY sale con código 2; sin `GOAL.md`, `verificar` sale con código 3)
- **Gate humano** (antes de la Tarea 16):
  1. El usuario lee `GOAL.borrador.md` y lo corrige si hace falta.
  2. El usuario lo copia a mano a `GOAL.md`.
  3. El usuario corre `root goal autorizar` en su terminal.
- **Por qué `autorizar` exige TTY**: el hook bloquea `Write` y `Edit`, pero no bloquea un binario corrido por `Bash`. La sesión `claude -p` del builder no tiene TTY, así que no puede autorizar el hash.
- **Ya hecho**: las cuatro líneas de `~/.claude/never-edit.txt` y el hook de `.claude/settings.json` del worktree.

### Tarea 4: Observe
- **Action**: `registrar` agrega una línea con lock y rota el archivo a los 50 MB. `leer_sesion_claude` convierte la salida JSON de `claude -p` en eventos (`total_cost_usd` va a `usd_cupo`, `num_turns`, `modelUsage`, `duration_ms`). `resumir` cuenta como intervención cada evento `Humano` posterior a `PlanAprobado`. Un evento `OraculoLeido` marca la corrida como inválida.
- **Mirror**: el formato de `logs/metrics.jsonl`. Fixture: una línea real de `~/Proyectos/bench-ecc-sdlc/logs/sistema-ecc.jsonl`.
- **Validate**: `cargo test observe` (16 escritores concurrentes no producen líneas rotas; 2 mensajes humanos después de aprobar dan 2 intervenciones; 1 mensaje antes de aprobar da 0)

### Tarea 5: Track e importación
- **Action**: `agregar`, `filas`, `comparar` y `estadisticas` con la cota inferior de Wilson al 90%. `root importar-mmorch` copia `metrics.jsonl` a `importado/` y lo convierte a `Evento` con `extra.origen = "mmorch"`. Nunca escribe sobre el original.
- **Validate**: `cargo test track` (Wilson con 0/0, 5/5 y 50/100 contra valores tabulados); importar el `metrics.jsonl` real deja 575 eventos y el md5 del original sin cambios.

### Tarea 6: Contenedor y junit
- **Action**: `oracle/contenedor.rs` arma el `podman run` de la sección "Cómo corre el oráculo". Aplica un timeout. Un error de infraestructura devuelve `ErrorOraculo::Infra` y nunca un veredicto. `oracle/junit.rs` cuenta pass, fail y skip por archivo.
- **Mirror**: `evaluar.py:17-50`.
- **Validate**: `cargo test --test contenedor` (un comando que escribe fuera de `/salida` falla; con `--network none`, `curl` falla; un junit con 3 tests y 1 falla da 2/3)

### Tarea 7: Tareas memora y brasa
- **Action**: crear `oraculos/memora` y `oraculos/brasa` desde `~/Proyectos/bench-ecc-sdlc/bootstrap/` y `.scratch/ecc-vs-sdlc/oraculo/`. Construir `imagenes/oraculo-python`. Los oráculos se copian a `ROOT_HOME`, nunca al repo de Root.
- **Validate**: `root oraculo evaluar --tarea memora --repo ~/Proyectos/bench-ecc-sdlc/memora-ecc` da contrato 61/61 y oculto 48/48. brasa-ecc da 53/53 y 15/16.

### Tarea 8: Proveedores
- **Action**: DeepSeek (API compatible con OpenAI) y Anthropic Messages. Reintentos y breaker como en `providers.py`. El costo sale de los precios de `root.toml`. Cada llamada registra un `Evento`. Las claves salen de variables de entorno y nunca entran a un log.
- **Validate**: `cargo test proveedores` con un servidor HTTP falso (429 reintenta; 3 fallas abren el breaker; la clave no aparece en `eventos.jsonl`)

### Tarea 9: Sondas deterministas
- **Action**: una sonda por dimensión checkeable:
  - **seguridad**: bandit y gitleaks.
  - **estructura**: duplicación, tamaño de archivos y funciones, complejidad con radon.
  - **arquitectura**: ciclos de import y reglas de `capas`.
  - **eficiencia**: el comando `[eficiencia]` contra `piso`.
  Cada sonda devuelve `Puntaje` y `Hallazgo`s con `archivo:línea`.
- **Validate**: `cargo test --test sondas` sobre fixtures con un defecto conocido por sonda.

### Tarea 9b: Sondas de TOML, YAML y Dockerfile
- **Action**: `oracle/configs.rs` busca los archivos `*.toml`, `*.yaml`, `*.yml`, `Dockerfile` y `Containerfile` del repo evaluado. `yaml-rust2` se usa solo en este archivo: Root nunca lee su propia configuración en YAML.
  - **TOML y YAML**: un error de parseo da un `Hallazgo` de estructura con `archivo:línea`. Una clave con forma de secreto (`password`, `token`, `api_key` con valor literal) da un `Hallazgo` de seguridad.
  - **Dockerfile**: hadolint corre en el contenedor de evaluación. Root agrega tres reglas propias de seguridad: imagen base sin versión fija (`:latest` o sin tag), ausencia de `USER` (corre como root) y un secreto en `ENV` o `ARG`.
  - Un repo sin estos archivos no recibe penalización.
- **Mirror**: el formato `Puntaje` + `Hallazgo` de la Tarea 9.
- **Validate**: `cargo test --test configs` (un YAML con tabulación inválida da un hallazgo con la línea exacta; un Dockerfile `FROM python:latest` sin `USER` da 2 hallazgos; un Dockerfile correcto da 0)
- **Calibración**: la Tarea 12 suma los tres defectos de Dockerfile y un secreto en YAML a su conjunto de mutaciones.

### Tarea 10: Juez de diseño y arquitectura
- **Action**: el juez evalúa solo lo que ninguna sonda cubre: diseño, legibilidad y coherencia de la arquitectura. Usa `root route --rol juzga`; por defecto, `deepseek-reasoner` con razonamiento prendido. Recibe una rúbrica fija. Refuta por default. Cada hallazgo exige `archivo`, `línea` y `cita` textual.
- **Mirror**: los invariantes "checkeable va a un oráculo" y "el verificador razona y refuta".
- **Validate**: `cargo test juez` con un proveedor falso (un JSON roto da `SinVeredicto`, nunca verde)

### Tarea 11: Gate de evidencia
- **Action**: `oracle/evidencia.rs` descarta todo hallazgo cuya `cita` no aparezca en `archivo` cerca de `línea` (±3 líneas, sin contar espacios). Este gate también lo usa REA en el hito 3.
- **Validate**: `cargo test evidencia` (una cita inventada se descarta; una cita con otra indentación se acepta)

### Tarea 12: Calibración (mide al evaluador)
- **Action**: `root oraculo calibrar` parte de un repo conocido como bueno, por ejemplo memora-ecc. Inyecta defectos por construcción: un secreto en el código, SQL concatenado, un bucle O(n²) en el camino caliente, un ciclo entre capas, un archivo dios. Cada caso alterado tiene etiqueta "defecto". Cada copia sin alterar tiene etiqueta "limpio". El resultado es la sensibilidad, la especificidad y la exactitud balanceada por dimensión, en `calibracion.json`. Con exactitud balanceada mayor o igual a 0,9, la dimensión queda `calibrada`.
- **Mirror**: `gates_bench`, donde la etiqueta sale de la construcción.
- **Validate**: `root oraculo calibrar --tarea memora --dimension seguridad --n 20 --semilla 42` escribe una fila. Con la misma semilla, el resultado se repite igual.

### Tarea 13: Oracle completo
- **Action**: `evaluar` arma el `Veredicto` con contrato, oculto, sondas, juez y gate de evidencia. Calcula `global` solo con dimensiones calibradas y marca las demás como "sin calibrar".
- **Validate**: `root oraculo evaluar --tarea memora --repo .../memora-ecc --json | jq '.puntajes'` lista seis dimensiones; memora-sdlc obtiene un `global` menor que memora-ecc.

### Tarea 14: Route
- **Action**: `elegir` es una función pura. Primero aplica tres invariantes:
  - Si `checkeable` es verdadero y la tarea tiene un oráculo, devuelve `Via::Oraculo`.
  - Los roles `Verifica` y `Juzga` siempre llevan `razonamiento = true`.
  - Un rol `Planifica`, `Organiza` o `Construye` respeta la asignación de `root.toml` (Opus, Sonnet, Haiku).
  
  Después aplica el balance: entre los modelos con cota de Wilson mayor o igual al umbral, elige el más barato. Si el uso de cupo supera el tope de `[balance]`, prefiere la API. Si no hay datos, usa el default de `root.toml`.
- **Validate**: `cargo test route` con una tabla de casos (checkeable da Oráculo; un juez sin razonamiento es imposible; cupo al 95% elige la API)

### Tarea 15: Aprende
- **Action**: `recalcular` lee filas y eventos y escribe `politica.json`. `root aprende --replay` aprende con el primer 70% de las filas y mide el 30% restante. Compara la política aprendida contra la de `root.toml` en éxito y en USD.
- **Validate**: `cargo test aprende` (con filas sintéticas donde Haiku acierta igual que Sonnet a menor costo, la política elige Haiku)

### Tarea 16: Runner
- **Action**: `root corre` corre primero `root goal verificar` y se detiene si el GOAL no está autorizado. Después copia el bootstrap a un directorio nuevo y corre `claude -p` por rol con `route`. Usa `ECC_DISABLED_HOOKS` y los permisos que niegan `oraculos/`. Registra `PlanAprobado` y `Humano`. Al final evalúa y agrega la `Fila`.
- **Mirror**: `correr-ecc-rol.sh`.
- **Validate**: `root corre --tarea memora --config ecc-haiku` deja una fila en `runs.jsonl` con un veredicto de seis dimensiones y un resumen.

### Tarea 17: Servidor MCP
- **Action**: JSON-RPC por stdio (`initialize`, `tools/list`, `tools/call`) con las cuatro herramientas. Cada herramienta llama a la misma función que su subcomando.
- **Validate**: `cargo test --test mcp` (un `tools/call root_route` devuelve el mismo JSON que `root route --json`)

### Tarea 18: Vault fuera de Root
- **Action**: Root empieza de cero, así que no tiene código del vault ni depende de `mmorch_vault_write`. El vault global y tu `CLAUDE.md` quedan como están. Esta tarea no escribe código: solo agrega una línea a `docs/interfaces.md` y un test.
- **Validate**: `! grep -rniE 'vault|mmorch' src/` (Root no nombra el vault ni a mmorch)

### Tarea 19: Documentos y PRD
- **Action**: escribir `docs/interfaces.md` y `docs/evidencia.md`, y anotar las decisiones en el PRD.
- **Validate**: `root esquema` de cada tipo coincide con `docs/interfaces.md` (test de snapshot).

## Cómo se construye (roles)

- Opus escribió este plan y resuelve los desempates.
- Sonnet corre como sesión principal: `--model sonnet` con `CLAUDE_CODE_SUBAGENT_MODEL=haiku`. Sonnet reparte una tarea por builder y corre el `Validate` de cada una.
- Haiku construye cada tarea con TDD dentro de ECC.
- Una tarea que falla su `Validate` tres veces vuelve a Sonnet con el log. Sonnet decide si la divide.
- La Tarea 3 (GOAL) tiene gate humano.

## Validation

```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
root goal verificar
root oraculo evaluar --tarea memora --repo ~/Proyectos/bench-ecc-sdlc/memora-ecc --json   # 61/61, 48/48
root oraculo evaluar --tarea brasa  --repo ~/Proyectos/bench-ecc-sdlc/brasa-ecc  --json   # 53/53, 15/16
for d in seguridad estructura arquitectura eficiencia diseno; do root oraculo calibrar --tarea memora --dimension $d --n 20 --semilla 42; done
root corre --tarea memora --config ecc-haiku && root track listar --tarea memora
root aprende --replay
```

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Haiku entra en ciclos con el borrow checker de Rust | Media | Tareas de menos de ~150 líneas; `cargo check` en cada paso; escalada a Sonnet después de 3 fallas |
| La biblioteca de YAML queda sin mantenimiento (`serde_yaml` ya está archivado) | Media | Verificar la fecha de la última versión en la Tarea 1; el parseo vive solo en `configs.rs`, así que cambiar de biblioteca toca un archivo |
| hadolint no corre sin la imagen de evaluación | Baja | La sonda falla cerrada: sin hadolint, la dimensión da `SinVeredicto`, nunca un puntaje limpio |
| El juez de diseño es subjetivo y la calibración solo cubre defectos inyectados | Alta | Una dimensión sin calibrar no entra a `global`. El "buen diseño" holístico queda medido solo en parte, y el veredicto lo dice. |
| El builder lee los oráculos ocultos (mismo usuario) | Media | Permisos que niegan `oraculos/`; detección en el transcript invalida la corrida |
| El cupo no se observa directo: `claude -p` da USD equivalentes, no el % del plan | Confirmada | El balance usa USD equivalentes y un tope configurable |
| ECC entero (decisión a) trae hooks que escriben secretos en `~/.claude/` | Confirmada | `root corre` apaga esos hooks por defecto; la reconstrucción de ECC los elimina |
| Haiku no está medido (decisión 7) | Media | Cada fila de `runs.jsonl` mide a Haiku de hecho; `root track comparar` lo hace visible |
| Podman sin el tag `z` falla por SELinux | Alta en Fedora | El tag `z` es fijo en `contenedor.rs` y tiene un test |
| Un agente corre `root goal autorizar` por Bash | Baja | `autorizar` exige TTY; la sesión `claude -p` no tiene TTY |
| `GOAL.md` falta cuando arranca `root corre` | Media | `root corre` corre `root goal verificar` primero y sale con código 3 sin GOAL autorizado |

## Acceptance
- [ ] Las tareas 1 a 19, incluida la 9b, están completas.
- [ ] La validación pasa.
- [ ] El Oracle reproduce 61/61 y 48/48 en memora-ecc, y 53/53 y 15/16 en brasa-ecc.
- [ ] Cada dimensión en `global` tiene exactitud balanceada ≥ 0,9 en su calibración.
- [ ] `root corre` arma memora desde el bootstrap con Haiku y deja una fila completa.
- [ ] `docs/interfaces.md` coincide con los esquemas que imprime `root esquema`.
- [ ] Los patrones de este plan se copian, no se reinventan.
