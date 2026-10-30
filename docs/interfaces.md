# Interfaces de Root

Graft, REA y CAB usan Root por tres vías: la CLI con `--json`, los archivos de `ROOT_HOME` y el servidor MCP.
Ningún hito importa módulos internos de Root.
Lo marcado **pendiente** no existe en el binario hoy. Cada marca nombra la tarea del plan que lo construye.
Fuente del plan: `.claude/plans/root.plan.md`, sección «Interfaces fijas».

## Vault

Root no lee ni escribe el vault de mmorch y no depende de `mmorch_vault_write`; el nombre `mmorch` solo etiqueta el origen de los datos importados (`r00t importar-mmorch`, `extra.origen`).

## Uso desde Graft, REA y CAB

Usar Root por la CLI con `--json`, por el MCP o leyendo los archivos de `ROOT_HOME`.
Ningún hito importa un módulo de `src/`.

- Pedir modelo: `r00t --json route --rol <rol> --tipo <tipo>`, o la herramienta `root_route`.
- Llamar a un modelo: `r00t --json llama --rol <rol> --tipo <tipo> --prompt-file <f> --corrida <id>`, o `root_llama`.
- Evaluar un repo: `r00t --json oraculo evaluar --tarea <t> --repo <dir>`, o `root_evaluar`. Mirar el código de salida.
- Registrar un paso: `r00t observa evento --corrida <id> --tipo <humano|plan-aprobado|gate>`, o `root_registrar`.
- Leer el registro: `runs.jsonl` y `eventos.jsonl` en `ROOT_HOME`. Cada línea trae `"v": 1`.

### Uso previsto por hito

| Hito | Uso previsto | Estado hoy |
|---|---|---|
| 2, Graft | Pide el modelo de cada etapa con `r00t route`. Usa las sondas del Oracle como gates. Registra cada etapa con `r00t observa evento`. | `route`, `observa evento` y las sondas están construidos. Los gates de Graft son del hito 2. |
| 3, REA | Registra cada paso con `r00t observa evento`. Verifica su evidencia con el chequeo `archivo:línea` del juez. | `observa evento` está construido. La verificación de evidencia (`oracle/evidencia.rs`) existe como función, sin subcomando propio. |
| 4, CAB | Define una tarea con `tarea.toml` y mide cada agente con `r00t corre`. | `tarea.toml` está construido. `r00t corre` está construido con el contenedor del builder (Tarea 26, capa 1). La imagen y la corrida real son de la laptop. |
| ECC (decisión a) | Recibe sus modelos por rol desde `r00t route`. Sus hooks que filtran datos se apagan por defecto en `r00t corre`. | `route` está construido. `corre` está construido. La imagen del builder y la corrida real son de la laptop. |

## Códigos de salida

Con `--json`, cada subcomando construido imprime un solo objeto JSON en stdout.
`r00t esquema` e `importar-mmorch` imprimen JSON siempre.

| Código | Significado | Dónde sale hoy |
|---|---|---|
| `0` | ok o verde | `route`, `llama`, `observa`, `track`, `aprende`, `goal`, `importar-mmorch`, `ficha tarea`, `ficha revisar` sin hallazgos y con el juez en `ok` (con `--juez-proveedor` y `--juez-modelo`). `oraculo evaluar` e `oraculo informe` con decisión `Listo` (ver abajo). |
| `1` | rojo | `oraculo evaluar` e `oraculo informe` con decisión `NoListo` o `Bloqueado`. `ficha revisar` con hallazgos o ambigüedades, aunque el juez haya fallado. |
| `2` | error de uso | Argumentos inválidos (clap). `root.toml` con un valor fuera de rango. Un spec sin `## Ficha` o con una ficha inválida. Un tipo desconocido en `esquema`. |
| `3` | error de infraestructura, o sin veredicto | Un tope que impide la elección o la llamada (`route`, `llama`). Un error de proveedor, de Observe o de Track. `oraculo evaluar` sin veredicto (descarga o suite sin veredicto). `ficha revisar` sin hallazgos y sin veredicto del juez: sin `--juez-*` el juez queda `sin_veredicto: juez no corrido` y el código es 3, nunca 0. Los subcomandos **pendientes** salen con `3` porque aún no tienen lógica. |

`oraculo evaluar` y `oraculo informe` salen según la decisión del informe (Tarea 13b): `Listo` da `0`, `NoListo` y `Bloqueado` dan `1`.
Sin informe (una descarga o una suite sin veredicto), salen con `3`.
El estado del veredicto sale de la decisión, no de los conteos de tests: un test que falla fuera de un requisito de peso 3 no basta para `NoListo`.
Ningún conteo de tests decide el estado por sí solo. Esto sigue la decisión 16 del plan.

## CLI

Una fila por subcomando. Los argumentos entre corchetes son opcionales.

| Comando | Qué hace | Salida con `--json` | Estado |
|---|---|---|---|
| `r00t route --rol <rol> --tipo <tipo> [--checkeable] [--tokens N]` | Elige la vía y el modelo para un rol y un tipo. | `Eleccion` | construido |
| `r00t llama --rol <rol> --tipo <tipo> --prompt-file <f> --corrida <id> [--max-tokens N]` | Llama al modelo elegido. Registra el evento. Rechaza la llamada si no hay precio o tope. | `Respuesta` | construido |
| `r00t oraculo evaluar --tarea <t> --repo <dir>` | Corre las suites de la tarea sobre el repo y da el veredicto, con su informe. | `Veredicto` | construido (Tarea 13b). |
| `r00t oraculo pendientes --tarea <t> --repo <dir> [--importar <junit>... \| --colab]` | Arma el paquete de tests pendientes por entorno para Colab. Con `--importar`, guarda los junit de vuelta, uno por repetición (`pendientes-N.xml`). Con `--colab`, corre el paquete en Colab y lo importa. Solo una corrida por tarea a la vez: la segunda sale con `3` y nombra a la primera (pid y hora). | `{dir, tests}`, o `{importados, ignorados}` | construido (Tarea 7f). `--colab`: Tarea 7j |
| `r00t oraculo pendientes --liberar` | Libera el runtime de Colab anotado en `$ROOT_HOME/colab/activo.json`, si hay uno, y sale. | sin salida JSON | construido (Tarea 7j) |
| `r00t oraculo pendientes --vigilar` | Red de seguridad: lee `$ROOT_HOME/colab/activo.json`. Libera el runtime si pasó su tope o si su junit cumple lo pedido. Un runtime vigente no se toca. Sin `--tarea` ni `--repo`. | `{vigilancia, causa}`, con `vigilancia` `sin_runtime`, `vigente` o `liberado` | construido (Tarea 7l) |
| `r00t oraculo calibrar --tarea <t> --dimension <d> [--n 20] [--semilla 42] [--repo <dir>] [--tope-usd <n>]` | Calibra una dimensión con defectos inyectados: mitad con defecto, mitad limpia. Mide la sensibilidad, la especificidad y la exactitud balanceada, por dimensión y por ítem, contra una línea base. Agrega una fila a `$ROOT_HOME/calibracion.json` e imprime la fila. Sin `--repo`, usa `[calibracion] repo` de `tarea.toml`; sin ninguno, sale con `2`. Con `--tope-usd <n>`, antes de la línea base y de cada caso compara el gasto de API desde el inicio, más el peor caso del caso, con el tope. Si no entra, registra un `Gate` `calibracion-tope`, guarda la fila de los casos hechos como `incompleta` y sale con `3`. El tope debe ser finito y mayor que 0; si no, sale con `2`. | `Fila` | construido (Tarea 12 y decisión 48). |
| `r00t oraculo validar --casos <dir> [--json]` | Mide si el juicio de Root coincide con los casos de `<dir>` (orden conocido, defectos reales y el formulario a ciegas). Escribe `<dir>/validez.json`. Una métrica sin datos queda sin medir, nunca 0 ni 1. Sale con `0` aunque una métrica no cumpla. | `Validez` | construido (Tarea 12b). Formatos: sección ## Validez. |
| `r00t oraculo validar --preparar-ciego --fuentes <manifiesto.toml> --casos <dir> [--semilla N]` | Arma un paquete anónimo por fuente en `<dir>/ciego/`, con su formulario, y la clave `ciego/clave.toml`. | ids de los paquetes | construido (Tarea 12b). |
| `r00t oraculo informe --tarea <t> --repo <dir> [--md]` | Arma el informe del PM sobre el veredicto: decisión, regla que la disparó, requisitos, bloqueantes, riesgos, próximos pasos y confianza. Sin `--md`, imprime JSON; con `--md`, markdown legible. | `Informe` | construido (Tarea 13b). |
| `r00t oraculo validar --casos <dir>` | Mide si el juicio de Root coincide con resultados conocidos. | `Validez` | pendiente (Tarea 12b). No está en `--help`. |
| `r00t observa claude --archivo <salida-de-claude-p> --corrida <id> [--via cupo\|nube]` | Registra la salida JSON de `claude -p`. Con `--via nube`, registra una sesión en la nube. | `{registrados, via}` | construido |
| `r00t observa evento --corrida <id> --tipo <humano\|plan-aprobado\|gate> [--detalle txt]` | Registra un evento manual de la corrida. | evento registrado | construido |
| `r00t observa resumen --corrida <id>` | Resume una corrida. | `Resumen` | construido |
| `r00t observa uso` | Muestra el gasto y el margen de cada tope. | margen por tope | construido |
| `r00t observa costo --usd <n> --motivo <texto> [--fecha <iso>]` | Registra un costo de API que Root no vio (decisión 46). `--usd` debe ser finito y mayor que 0. `--fecha` es `AAAA-MM-DD` (12:00 locales) o RFC 3339; sin él, vale la hora actual. | evento `Llamada` registrado | construido |
| `r00t track agregar --fila <archivo.json>` | Agrega una fila a `runs.jsonl`. | `{agregada}` | construido |
| `r00t track listar [--tarea t]` | Lista las filas, opcionalmente de una tarea. | lista de `Fila` | construido |
| `r00t track comparar --a <c1> --b <c2> --tarea <t>` | Compara dos corridas de una tarea. | comparación | construido |
| `r00t aprende [--replay]` | Recalcula `politica.json`. Con `--replay`, mide la política sin escribirla. | `Politica`, o `Replay` | construido |
| `r00t corre --tarea <t> --config <c>` | Corre una tarea con una configuración (con `--json` global, imprime la fila completa): rutea todos los roles, los lanza por el contenedor del builder, registra sus sesiones y lecturas, evalúa y agrega una fila a `runs.jsonl`. Sin GOAL autorizado no arranca. | `Fila` | construido (Tareas 16 y 26, capa 1). Lanza cada rol en el contenedor `root-builder:1`, sin `ROOT_HOME` y con la credencial de Claude en solo lectura. Sin credencial, sale con `3` y no lanza nada. |
| `r00t goal verificar` | Verifica que `GOAL.md` coincida con `GOAL.hash`. | `{ok, hash}` | construido |
| `r00t goal autorizar` | Autoriza un cambio al GOAL. Exige una TTY y una confirmación tecleada. | sin salida JSON | construido |
| `r00t importar-mmorch --desde <metrics.jsonl>` | Copia en solo lectura el log de métricas de mmorch. | resumen de la importación | construido |
| `r00t esquema <tipo>` | Imprime el JSON Schema de un tipo. Ver `## Esquemas`. | JSON Schema | construido |
| `r00t ficha revisar --spec <SPEC.md> [--juez-proveedor p --juez-modelo m [--juez-max-tokens n]]` | Revisa la ficha y las ambigüedades. Con los dos flags de juez, también corre el juez. Sin ellos, el juez queda `sin_veredicto: juez no corrido`: sin hallazgos, sale 3. `--juez-max-tokens` fija el tope de salida del juez: 32 000 por defecto, al menos 1, y exige `--juez-modelo`. | `Revision` | construido |
| `r00t ficha tarea --spec <SPEC.md> --nombre <t>` | Arma el borrador de `tarea.toml` desde la ficha. | `{toml}` | construido |
| `r00t mcp` | Inicia el servidor MCP por stdio. | JSON-RPC 2.0 | construido |

**Colab (`oraculo pendientes --colab`).** Root pide un runtime T4 de Colab, corre el paquete, importa el junit y libera el runtime. Antes de pedirlo, Root escribe su id en `$ROOT_HOME/colab/activo.json`. Lo borra al liberar el runtime. Si el proceso muere con `SIGKILL`, el archivo queda. La próxima corrida de `--colab` libera ese runtime primero. `--liberar` hace solo esa limpieza. `SIGINT` y `SIGTERM` también liberan el runtime y registran un `Gate`. El login de Colab lo hace la persona antes de correr el comando. Root no lee el token de la CLI.

**Corte temprano.** `correr.sh` corre en segundo plano en el runtime y Root lo sondea cada 60 segundos. Root libera el runtime en cuanto se cumple lo que pedía la corrida: los junit por repetición traen cada test pendiente, con tantos resultados como `repeticiones` de `tarea.toml`, sumados entre todos. Un test que pasa en una repetición y falla en otra queda `inestable`, con su tasa. Si `correr.sh` termina antes, Root libera el runtime con lo que haya. El `Gate` de liberación lleva la clave `causa`: `requisito cumplido`, `correr.sh terminó`, `tope de duración`, `señal SIGINT`/`señal SIGTERM`, `gpu-no-t4` (la GPU no es una T4), `runtime-no-asignado` (`colab new` no dio el runtime) o `error-en-corrida` (un paso falló a mitad de la corrida). `--vigilar` es la red de seguridad: libera el runtime anotado si pasó su tope o si su junit cumple lo pedido, aunque la corrida ya no esté viva.

**Tope de duración.** Cada corrida de `--colab` tiene un tope, `[colab].minutos_max` en `root.toml`, con 120 minutos (2 horas) por defecto, con corte temprano. Al llegar al tope, Root libera el runtime, registra un `Gate` con la causa `tope de duración` y sale con código `3`. El `motivo` del `Gate` lleva el tope real, por ejemplo `tope de 120 minutos`. Los tests sin resultado quedan `Pendiente`.

## MCP

El servidor habla JSON-RPC 2.0 por stdio, con un mensaje por línea.
Responde a `initialize`, `ping`, `tools/list` y `tools/call`.
Cada herramienta llama a la misma función que su subcomando. Devuelve el mismo JSON.
Un error de herramienta sale con `isError: true`. Un argumento inválido sale con el código JSON-RPC `-32602`.

Las cuatro herramientas:

- `root_route`: elige el modelo y la vía. Equivale a `r00t route`.
- `root_llama`: llama a un modelo por API. Equivale a `r00t llama`. Sin margen, rechaza la llamada antes de enviarla.
- `root_evaluar`: evalúa un repo con una tarea. Equivale a `r00t oraculo evaluar`.
- `root_registrar`: registra un evento manual. Equivale a `r00t observa evento`.

### root_route

```json
{
  "type": "object",
  "properties": {
    "rol": {"type": "string", "enum": ["planifica", "organiza", "construye", "verifica", "juzga", "delega"], "description": "Rol del pedido."},
    "tipo": {"type": "string", "description": "Tipo de tarea, por ejemplo diseno o bulk."},
    "checkeable": {"type": "boolean", "description": "Si la salida se verifica con un oráculo."},
    "tokens": {"type": "integer", "minimum": 0, "description": "Tokens estimados del pedido."}
  },
  "required": ["rol", "tipo"]
}
```

### root_llama

```json
{
  "type": "object",
  "properties": {
    "rol": {"type": "string", "enum": ["planifica", "organiza", "construye", "verifica", "juzga", "delega"], "description": "Rol del pedido."},
    "tipo": {"type": "string", "description": "Tipo de tarea, por ejemplo diseno o bulk."},
    "prompt": {"type": "string", "description": "Texto que se envía al modelo."},
    "corrida": {"type": "string", "description": "Corrida de los eventos. Por defecto mcp."},
    "max_tokens": {"type": "integer", "minimum": 0, "description": "Tope de tokens de salida. Por defecto 4096."}
  },
  "required": ["rol", "tipo", "prompt"]
}
```

### root_evaluar

```json
{
  "type": "object",
  "properties": {
    "tarea": {"type": "string", "description": "Nombre de la tarea en ROOT_HOME/oraculos."},
    "repo": {"type": "string", "description": "Ruta del repositorio a evaluar."}
  },
  "required": ["tarea", "repo"]
}
```

### root_registrar

```json
{
  "type": "object",
  "properties": {
    "corrida": {"type": "string", "description": "Corrida a la que pertenece el evento."},
    "tipo": {"type": "string", "enum": ["humano", "plan-aprobado", "gate"], "description": "Tipo del evento."},
    "detalle": {"type": "string", "description": "Texto libre que acompaña al evento."}
  },
  "required": ["corrida", "tipo"]
}
```

## Archivos de ROOT_HOME

`ROOT_HOME` vale `~/.local/share/root/` por defecto. Si la variable `ROOT_HOME` existe y no está vacía, Root usa esa ruta.

```
$ROOT_HOME/
  root.toml               # modelos, precios, balance, umbrales (lo escribe el humano)
  eventos.jsonl           # Observe: una línea por evento, "v": 1
  eventos.jsonl.lock      # cerrojo de Observe
  calibracion.lock        # cerrojo de Oracle al agregar una calibración
  runs.jsonl              # Track: una línea por corrida, "v": 1
  politica.json           # Aprende: política de Route recalculada
  calibracion.json        # Oracle: exactitud medida por dimensión (Tarea 12)
  oraculos/<tarea>/
    tarea.toml            # definición de la tarea
    contrato/             # tests visibles para el builder (convención de carpeta)
    oculto/               # tests que el builder nunca ve (convención de carpeta)
    bootstrap/            # repo inicial (convención de carpeta)
    diferencias.md        # registro de diferencias contra el banco (lo escribe una persona)
  cache/                  # dependencias bajadas en la fase de descarga (Tarea 6b)
    rust/<hash>/
    python/<hash>/
    rustsec/
  tmp/                    # temporales del Oracle (evaluar-*, colab-*); se limpian por antigüedad
  evidencia/<tarea>/colab.json   # resultados importados de Colab, con el commit del repo (Tarea 7f, B16)
  colab/activo.json       # runtime de Colab pedido y no liberado; lo borra Root al liberarlo (Tarea 7j)
  oraculos/<tarea>/       # tests ocultos: el builder no puede leer esta carpeta (Tarea 23)
    pendientes/           # paquete de tests pendientes por entorno; lo borra Root al terminar Colab
    registros/            # stderr de las corridas que no dejaron junit (A2)
  importado/
    mmorch-metrics.jsonl  # copia de solo lectura del log de mmorch (decisión b3)
    mmorch-eventos.jsonl  # eventos derivados de la importación
<repo>/GOAL.md, <repo>/GOAL.hash   # contrato anti-drift (decisión b2)
```

Toda línea JSONL lleva `"v": 1`. Un cambio de formato sube `v`, y los lectores aceptan las versiones anteriores.
`oraculos/<tarea>/diferencias.md` no lo crea ningún subcomando. Lo escribe la persona que compara con el banco.
Las subcarpetas `contrato/`, `oculto/` y `bootstrap/` son convención. Cada suite del contenedor monta solo su carpeta (`contrato/` o `oculto/`), y los escenarios montan solo `escenarios/`. El Oracle lee solo `tarea.toml` y los comandos que ese archivo declara.

## tarea.toml

Ruta: `$ROOT_HOME/oraculos/<tarea>/tarea.toml`. Lo lee `Tarea::cargar_de` (`src/oracle/tarea.rs`).

`lenguaje` (el del proyecto) y `imagen` (la del oráculo) son campos distintos. Por eso `memora` y `brasa`, escritos en Rust, pueden correr sus oráculos de caja negra en pytest.

| Clave | Tipo | Obligatoria | Default | Regla |
|---|---|---|---|---|
| `nombre` | texto | sí | | Nombre de la tarea. |
| `lenguaje` | texto | sí | | Elige el perfil de sondas (Tarea 9). |
| `imagen` | texto | sí | | Imagen del contenedor del oráculo. |
| `containerfile` | texto | no | | Raíz. Si existe, el Oracle construye la imagen desde el Dockerfile del proyecto. Va antes de cualquier tabla. |
| `spec` | texto | sí | | Ruta del SPEC.md, relativa a la carpeta de la tarea. |
| `repeticiones` | entero | no | 1 | Veces que corre cada suite completa. Debe ser 1 o más. |
| `repeticiones_inestable` | entero | no | 10 | Corridas totales de un test inestable. No puede ser menor que `repeticiones`. |
| `[contrato]` `cmd` | texto | sí | | Suite visible. Corre con `sh -c` dentro del contenedor. |
| `[contrato]` `reintento` | texto | no | | Comando de las corridas extra de un test inestable. `{tests}` se reemplaza por los node ids. |
| `[oculto]` `cmd` | texto | sí | | Suite oculta. |
| `[oculto]` `reintento` | texto | no | | Igual que en `[contrato]`. |
| `[[entorno]]` `test`, `motivo` | texto | no | | Test que depende del entorno. El motivo no puede estar vacío. Un test aparece una sola vez. |
| `[[oraculo_dudoso]]` `test`, `motivo` | texto | no | | Test que depende de una frase ambigua de la spec. Mismas reglas que `[[entorno]]`. |
| `[[requisito]]` `id`, `titulo`, `seccion`, `peso`, `pruebas`, `bordes`, `escenarios`, `criterios`, `modulos` | varios | no | `seccion` = ""; `peso` = 1 | `peso` debe estar entre 1 y 3. `id` no puede repetirse. `modulos` liga el requisito a los módulos del código. |
| `[[escenario]]` `id`, `cmd` | texto | no | | Flujo real de usuario. `cmd` sale con 0 si el flujo funciona. `id` solo usa `A-Za-z0-9._-` y no puede repetirse. |
| `[entrega]` `instalar`, `correr` | texto | no | | Instalación y arranque del producto en un contenedor limpio. Ambos son obligatorios si la tabla existe. |
| `[entrega]` `archivos` | lista de texto | no | [] | Archivos que el repo debe traer. |
| `[entrega]` `espera_segundos` | entero | no | 5 | Segundos antes de revisar que `correr` sigue vivo. |
| `[pruebas]` `mutantes_max`, `minutos_max` | entero | no | 60, 20 | Límites de la sonda de mutación. Ambos deben ser 1 o más. |
| `[eficiencia]` `cmd`, `piso` | texto, número | no | | `cmd` imprime `{"ops_s": f64}`. Debe llegar a `piso`. |
| `[arquitectura]` `capas` | lista de listas | no | | Una capa importa solo capas posteriores. |
| `[calibracion]` `repo` | texto | no | | Repo bueno de `r00t oraculo calibrar` cuando no se pasa `--repo`. Una ruta relativa se resuelve desde la carpeta de la tarea. |

Las claves que Root no conoce se ignoran al parsear. Por eso una tarea con secciones de etapas futuras carga igual.

### Ejemplo de tarea.toml

```toml
nombre = "memora"
lenguaje = "rust"
imagen = "root-oraculo-rust:1"
containerfile = "bootstrap/Dockerfile"
spec = "bootstrap/docs/SPEC.md"
repeticiones = 3
repeticiones_inestable = 10

[contrato]
cmd = "pytest -q -p no:cacheprovider --junitxml=/salida/contrato.xml /repo/tests"

[oculto]
cmd = "pytest -q -p no:cacheprovider --junitxml=/salida/oculto.xml /oraculo/oculto"

[[entorno]]
test = "test_oculto_brasa::test_rendimiento[gpu-10000-res1]"
motivo = "lavapipe contra GPU: el host tiene adaptador y el contenedor no"

[[oraculo_dudoso]]
test = "test_oculto_x::test_orden"
motivo = "la spec no fija el orden de servicio"

[[requisito]]
id = "R1"
titulo = "Protocolo RESP2"
seccion = "## Protocolo"
peso = 3
pruebas = ["test_m1_protocolo"]
bordes = ["test_m1_protocolo::test_trama_partida", "test_m1_protocolo::test_error_resp"]
escenarios = ["e2e-set-get"]
criterios = ["completitud.alcance"]
modulos = ["protocolo"]

[[escenario]]
id = "e2e-set-get"
cmd = "/oraculo/escenarios/set_get.sh"

[entrega]
instalar = "cargo build --release"
correr = "./target/release/memora --config memora.conf.example"
archivos = ["README.md", "Dockerfile", "memora.conf.example", ".github/workflows/ci.yml"]

[pruebas]
mutantes_max = 60
minutos_max = 20

[eficiencia]
cmd = "python /oraculo/bench.py /repo"
piso = 5000.0

[arquitectura]
capas = [["protocolo"], ["comandos"], ["almacen"]]
```

## Ficha

La ficha vive en `SPEC.md`, dentro de una sección `## Ficha` con un solo bloque ` ```toml `.
La prosa del spec explica. La ficha fija los datos que Root mide. Cada dato tiene un id que la prosa puede citar.

- La ficha es obligatoria. Un spec sin `## Ficha`, o con un bloque que no parsea, sale con código `2` y sin veredicto.
- El builder ve el spec y su ficha. La ficha nunca nombra tests ocultos. La relación requisito→test vive en `tarea.toml`.
- `r00t ficha tarea` arma un borrador de `tarea.toml`. El borrador deja `pruebas`, `bordes` y `escenarios` vacíos. La persona que aprueba la ficha completa esos campos en `ROOT_HOME`.
- `r00t ficha revisar` aplica reglas deterministas. Con `--juez-proveedor` y `--juez-modelo`, también corre el juez de ambigüedades.
- Si el juez llega al tope de salida, su causa es `sin_veredicto: tope de salida alcanzado (N tokens)`. N es el tope pedido. Esa causa no dice «JSON inválido».

Tablas de la ficha:

| Tabla | Campos |
|---|---|
| `[producto]` | `usuarios`, `problema`, `fuera_de_alcance` |
| `[[producto.requisito]]` | `id`, `titulo`, `peso`, `aceptacion` (medible: número, comando o salida exacta) |
| `[tecnica]` | `lenguaje`, `version_minima`, `dependencias_permitidas`, `plataformas` |
| `[[tecnica.limite]]` | `id`, `metrica`, `minimo`, `maximo`, `unidad`. Se copia a `[eficiencia]` de `tarea.toml`. |
| `[arquitectura]` | `capas`, `persistencia`, `concurrencia`. Se copia a `[arquitectura]` de `tarea.toml`. |
| `[[arquitectura.interfaz]]` | `id`, `modulo`, `publica` |
| `[diseno]` | `errores`, `logs`, `configuracion` |
| `[[diseno.regla]]` | `id`, `texto`, `item` (ítem de rúbrica que la regla ancla) |

### Ejemplo de ficha

```toml
[producto]
usuarios = ["operador que corre memora como caché"]
problema = "..."
fuera_de_alcance = ["clúster", "TLS"]

[[producto.requisito]]
id = "R1"
titulo = "Protocolo RESP2"
peso = 3
aceptacion = "un cliente redis-py 5.x completa SET/GET de 10 000 claves sin error"

[tecnica]
lenguaje = "rust"
version_minima = "1.85"
dependencias_permitidas = ["tokio", "bytes"]
plataformas = ["linux-x86_64"]

[[tecnica.limite]]
id = "T1"
metrica = "ops_s"
minimo = 5000.0
unidad = "operaciones por segundo"

[arquitectura]
capas = [["protocolo"], ["comandos"], ["almacen"]]
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
item = "diseno.robustez"
```

## Rúbricas

Cada dimensión se puntúa con una rúbrica. Cada ítem tiene cuatro anclas, una por nivel de 0 a 3. Cada ancla exige evidencia citada. Root no inventa criterios: cada ítem cita su estándar.

Campos de la rúbrica (`src/oracle/rubrica.rs`):

- `id`, `version`: identifican la rúbrica. Su hash va en cada `Veredicto`, como `"<id>@<hash>"`.
- `[[item]]` `id`, `fuente`, `mide`, `peso`, `bloquea`, `anclas`.
- `fuente`: obligatoria. Sin fuente, el ítem no carga.
- `mide`: `"sonda"` (una sonda fija el nivel), `"juez"` (el juez fija el nivel) o `"mixto"` (el juez fija el nivel y una sonda pone un tope).
- `peso`: de 1 a 3.
- `bloquea`: si es `true`, nivel 0 en el ítem bloquea la decisión (Tarea 13b, **pendiente**).
- `anclas`: cuatro textos, uno por nivel.

Valor de una dimensión: `Σ peso·nivel / (3·Σ peso)`, sobre los ítems con nivel. Un ítem sin nivel no entra al cálculo.
Dos veredictos con hashes de rúbrica distintos no se comparan.

Estado de las rúbricas reales: **pendiente**. Ninguna rúbrica viene embebida hoy. `EMBEBIDAS` en `src/oracle/rubrica.rs` está vacío. Las rúbricas de la Tarea 8b esperan la aprobación del usuario.

Estándares de referencia del plan:

| Dimensión | Estándar |
|---|---|
| Completitud, Funciona | ISO/IEC 25010:2023, idoneidad funcional. Trazabilidad según ISO/IEC/IEEE 29148. |
| Seguridad | OWASP ASVS 5.0, nivel 1, y CWE Top 25. |
| Entrega | CIS Docker Benchmark, sección 4, y chequeos locales de OpenSSF Scorecard. |
| Pruebas | ISO/IEC 25010, testeabilidad, con el puntaje de mutación. |
| Eficiencia | ISO/IEC 25010, eficiencia de desempeño, contra el `piso` de la tarea. |
| Estructura, Arquitectura, Diseño | ISO/IEC 25010, mantenibilidad. En Rust, el checklist de Rust API Guidelines. |

Las rúbricas citan solo el nombre y la cláusula de cada característica de ISO. Root escribe sus propias anclas.

### Ejemplo de rúbrica

Ejemplo de formato. No es una rúbrica aprobada.

```toml
id = "diseno"
version = 1

[[item]]
id = "diseno.modularidad"
fuente = "ISO/IEC 25010:2023 §mantenibilidad/modularidad"
mide = "juez"
peso = 3
bloquea = false
anclas = [
  "0: un módulo concentra varios requisitos sin límites; un cambio toca todo",
  "1: hay módulos, pero con dependencias cruzadas o ciclos",
  "2: cada requisito vive en un módulo con una interfaz clara; quedan acoples puntuales",
  "3: módulos con una responsabilidad, sin ciclos, con interfaces chicas y probadas",
]
```

## Tipos y funciones

Los tipos se documentan con su JSON Schema en `## Esquemas`. La fuente es `src/tipos.rs`, o el módulo que se indica.

| Tipo (`r00t esquema`) | Módulo | Estado |
|---|---|---|
| `veredicto` | `tipos` | construido. Su `informe` sale vacío hasta la Tarea 13b. |
| `evento` | `tipos` | construido |
| `fila` | `tipos` | construido |
| `pedido` | `tipos` | construido |
| `eleccion` | `tipos` | construido |
| `resumen` | `tipos` | construido |
| `hallazgo` | `tipos` | construido |
| `puntaje` | `tipos` | construido |
| `conteo` | `tipos` | construido |
| `config` | `config` | construido |
| `informe` | `tipos` | el tipo existe. El cálculo del informe es **pendiente** (Tarea 13b). |
| `estadorequisito` | `tipos` | construido |
| `accion` | `tipos` | construido |
| `nivelitem` | `tipos` | construido |
| `pendiente` | `tipos` | construido (Tarea 7f) |
| `revision` | `ficha` | construido |
| `uso` | `observe::uso` | construido |
| `sinpresupuesto` | `tipos` | construido |
| `politica` | `route` | construido |
| `usomargenes` | `observe::uso` | construido |
| `replay` | `aprende` | construido |
| `validez` | `oracle::validez` | construido (Tarea 12b) |

Enums: `Rol`, `Proveedor` (`anthropic`, `deepseek`), `Via` (`cupo`, `api`, `oraculo`, `nube`), `Dimension`, `Estado` (`verde`, `rojo`, `sin_veredicto`), `Cumplimiento`, `Decision`, `Fuente`, `TipoEvento`. Sus valores aparecen en los esquemas.

Funciones que usa el código:

| Función | Estado | Nota |
|---|---|---|
| `route::comando::rutear(rol, tipo, checkeable, tokens)` | construido | Fachada de `route` que usan la CLI y el MCP. |
| `route::comando::llamar_con_chequeo(...)` | construido | Chequea el costo antes de llamar. |
| `observe::uso::uso(ahora)` | construido | Devuelve `Uso`. |
| `observe::registrar(ev)` | construido | Agrega un evento con cerrojo. |
| `observe::claude::leer_sesion_claude(json, corrida)` | construido | Lee la salida de `claude -p`. |
| `observe::resumir(corrida)` | construido | Devuelve `Resumen`. |
| `oracle::evaluar(repo, tarea, opciones)` | construido | `oracle::evaluar_por_nombre(nombre, repo)` lee la tarea desde `ROOT_HOME`. |
| `oracle::calibra::calibrar_tarea(home, nombre, dimension, n, semilla, repo, evaluador)` | construido (Tarea 12) | Calibra y agrega la fila a `calibracion.json`. `calibrar_con` recibe la tarea y el repo ya resueltos. |
| `oracle::calibra::calibrar_tarea_con(home, nombre, dimension, n, semilla, repo, presupuesto, evaluador)` | construido (decisión 48) | Como `calibrar_tarea`, con el tope opcional `Presupuesto`: gasto desde el inicio, peor caso de un caso y tope en USD. Si el tope detiene la calibración, escribe las filas hechas como `incompleta` y sale con `3`. |
| `oracle::informe::informar(veredicto, tarea, reglas, redactor)` | construido (Tarea 13b) | El redactor es opcional. `informar_con` recibe además las rúbricas. |
| `oracle::validez::validar(casos)` | construido (Tarea 12b) | `validar_con` recibe el evaluador. `preparar_ciego(casos, fuentes, semilla)` arma los paquetes a ciegas. |
| `track::agregar(fila)` | construido | |
| `track::filas(filtro)` | construido | |
| `track::comparar(a, b, tarea)` | construido | Rechaza hashes de rúbrica distintos. |
| `track::estadisticas(rol, tipo)` | construido | Sin subcomando propio. |
| `aprende::recalcular(filas, eventos)` | construido | Función pura. |
| `proveedores::llamar(eleccion, prompt, corrida)` | construido | Registra el evento. |
| `goal::verificar_goal(repo)` | construido | |

## Validez

`r00t oraculo validar --casos <dir>` mide si el juicio de Root coincide con resultados que una fuente externa conoce. Escribe `<dir>/validez.json`, tipo `validez`, y resume cada métrica en texto. Con `--json` imprime el mismo objeto.

**Carpeta de casos.** Todo archivo es opcional. Sin archivo, su métrica queda sin medir: `valor` vale `null` y `motivo` dice por qué. Nunca vale 0 ni 1.

- `orden.toml`: `[[par]]` con `tarea`, `mejor`, `peor` y `fuente` (de dónde sale el orden). `mejor` y `peor` son rutas de repo relativas a `<dir>`.
- `defectos.toml`: `[[defecto]]` con `tarea`, `repo`, `archivo`, `linea`, `fuente` (por ejemplo "revisión de Opus F1") y `detalle`. `repo` es relativo a `<dir>`.
- `ciego/`: los paquetes a ciegas que arma `--preparar-ciego`. `ciego/clave.toml` es la clave que lee Root: `[[paquete]]` con `id`, `tarea`, `origen` y `repo`. El paquete y su formulario no nombran el origen.

**Métricas.**

- *Exactitud de orden*: fracción de pares donde Root pone `mejor` por encima de `peor`. Root compara primero la decisión (listo, no listo, bloqueado). A igual decisión compara el `global` si los dos lo tienen; si no, la completitud. Un empate o un par sin veredicto cuenta como fallo. Umbral 1,0: cumple solo si todos los pares quedan bien ordenados (desempate 44).
- *Recall de defectos*: fracción de defectos con un hallazgo del veredicto en el mismo `archivo` y a ±3 líneas de `linea`. Umbral 0,8 (decisión 12).
- *Acuerdo con el usuario*: `acuerdo_decision` cuenta los paquetes cuya decisión de Root coincide con la del formulario; cumple con 5 de 6 (decisión 13). `kappa_dimension` es el kappa de Cohen con pesos cuadráticos sobre los niveles 0 a 3, sumando los ítems de cada dimensión; cumple con 0,6 o más. `kappa_item` lo mide por ítem, sin umbral: un kappa bajo marca un ancla ambigua. `spearman` correlaciona el `orden` del formulario (1 es el mejor) con el orden de Root.

Un par, defecto o paquete que no tiene dato del usuario o de Root queda fuera de su métrica, y `avisos` lo cuenta. Un error del evaluador aborta la medición y no escribe `validez.json`.

**Paquetes a ciegas.** `r00t oraculo validar --preparar-ciego --fuentes <manifiesto.toml> --casos <dir> [--semilla N]` lee el manifiesto. Su `[[fuente]]` tiene `tarea`, `origen` y `repo` (ruta absoluta o relativa al manifiesto). Por cada fuente arma `<dir>/ciego/<id>/` con:

- `repo/`: copia del repo sin los nombres `AGENTS.md`, `CLAUDE.md`, `sdlc.toml`, `.claude`, `.git`, `logs` y `target` en cualquier nivel; sin archivos `*.log` ni nombres que empiecen con `.ecc`; sin enlaces simbólicos. Estos nombres son los rastros de ECC, de SDLC, de git, de logs y de build. En cada `Cargo.toml` copiado, el armado borra `authors`, `repository` y `homepage` de `[package]` y de `[workspace.package]`. Un `Cargo.toml` que no parsea queda igual y se anota en la lista.
- `rastros-<id>.toml`, junto a la carpeta del paquete y fuera de ella: `aparicion` con `archivo`, `linea` y `palabra` de cada `ecc`, `sdlc`, `claude`, `haiku`, `sonnet` u `opus` en los archivos de texto del `repo/`, sin distinguir mayúsculas y como palabra completa (`recclamo` no cuenta). Los binarios no se revisan. `avisos` lista lo que el armado no pudo revisar. El resumen de la CLI dice cuántas apariciones hay por paquete y dónde está la lista. Opus la revisa antes de entregar los paquetes (desempate 45).
- `formulario.toml`: `decision` (`listo`, `no_listo` o `bloqueado`), `orden` (posición entre los del lote; 1 es el mejor) y un `[[item]]` por ítem de rúbrica, con `nivel` de 0 a 3. Las anclas van como comentario. Sin rúbricas embebidas, el formulario lleva solo la decisión y el orden, y el comando avisa en stderr.
- El `id` tiene 8 caracteres hexadecimales y sale de un PRNG con la semilla (`--semilla`, o el reloj). Ningún `id` contiene el origen ni la tarea. Si `<dir>/ciego/` ya tiene contenido, el comando sale con `2`.

Las rúbricas del formulario salen de `rubrica::cargar_embebidas()`, las mismas que usa Root.

## Esquemas

Salida exacta de `r00t esquema <tipo>`, en orden alfabético. El test `tests/interfaces.rs` la compara con el binario.

### accion

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Accion",
  "description": "`refs` lleva ids de requisitos o hallazgos.",
  "type": "object",
  "properties": {
    "prioridad": {
      "type": "integer",
      "format": "uint8",
      "maximum": 255,
      "minimum": 0
    },
    "refs": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "texto": {
      "type": "string"
    }
  },
  "required": [
    "prioridad",
    "texto",
    "refs"
  ]
}
```

### calibracion

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Calibraciones",
  "description": "Contenido de `calibracion.json`. Sin archivo, no hay filas: nada está calibrado.",
  "type": "object",
  "properties": {
    "filas": {
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/Fila"
      }
    },
    "v": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    }
  },
  "required": [
    "v"
  ],
  "$defs": {
    "Dimension": {
      "type": "string",
      "enum": [
        "funciona",
        "seguridad",
        "eficiencia",
        "estructura",
        "arquitectura",
        "diseno",
        "completitud",
        "entrega",
        "pruebas"
      ]
    },
    "Fila": {
      "description": "Una medición de una dimensión de una tarea, con su muestra.",
      "type": "object",
      "properties": {
        "defectos": {
          "description": "Defectos aplicados y medidos. La sensibilidad sale de ellos.",
          "type": "integer",
          "format": "uint32",
          "default": 0,
          "minimum": 0
        },
        "dimension": {
          "$ref": "#/$defs/Dimension"
        },
        "especificidad": {
          "type": [
            "number",
            "null"
          ],
          "format": "float",
          "default": null
        },
        "exactitud_balanceada": {
          "type": [
            "number",
            "null"
          ],
          "format": "float",
          "default": null
        },
        "incompleta": {
          "description": "`true` si el tope de gasto detuvo la calibración antes de medir todos sus casos (decisión 48). Una\nfila incompleta no calibra la dimensión, aunque su exactitud llegue a `UMBRAL`.",
          "type": "boolean",
          "default": false
        },
        "items": {
          "description": "Exactitud balanceada de cada ítem de rúbrica, por ejemplo `seguridad.secretos`.",
          "type": "object",
          "additionalProperties": {
            "type": "number",
            "format": "float"
          },
          "default": {}
        },
        "limpios": {
          "description": "Copias sin alterar medidas. La especificidad sale de ellas.",
          "type": "integer",
          "format": "uint32",
          "default": 0,
          "minimum": 0
        },
        "n": {
          "description": "Casos medidos: los defectos aplicados más los limpios. No cuenta los `no_aplicable`.",
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "no_aplicable": {
          "description": "Casos que no se pudieron aplicar al repo, por ejemplo sin Dockerfile. No entran a las métricas.",
          "type": "integer",
          "format": "uint32",
          "default": 0,
          "minimum": 0
        },
        "semilla": {
          "description": "Semilla de la muestra: con la misma semilla, la medición se repite igual.",
          "type": "integer",
          "format": "uint64",
          "minimum": 0
        },
        "sensibilidad": {
          "type": [
            "number",
            "null"
          ],
          "format": "float",
          "default": null
        },
        "tarea": {
          "type": "string"
        },
        "ts": {
          "description": "Epoch en segundos de la medición. Gana la fila más reciente.",
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "tarea",
        "dimension",
        "n",
        "semilla",
        "ts"
      ]
    }
  }
}
```

### config

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Config",
  "type": "object",
  "properties": {
    "balance": {
      "$ref": "#/$defs/Balance",
      "default": {
        "api": {
          "tope_usd": 100.0
        },
        "cupo": {
          "bloque_horas": 5,
          "tope_usd": 35.0
        },
        "nube": {
          "tope_usd": 250.0
        },
        "prefiere_api_sobre": 0.9
      }
    },
    "colab": {
      "$ref": "#/$defs/ConfigColab",
      "default": {
        "minutos_max": 120.0
      }
    },
    "decision": {
      "description": "Sección `[decision]`: reglas deterministas del informe (Tarea 13b).",
      "$ref": "#/$defs/ReglasDecision",
      "default": {
        "completitud_minima": 0.9,
        "minimo": {}
      }
    },
    "jueces": {
      "description": "Sección `[jueces]`: modelo de los jueces que no pasan por `route`.",
      "$ref": "#/$defs/Jueces",
      "default": {
        "config_modelo": "claude-sonnet-5-5"
      }
    },
    "modelos": {
      "$ref": "#/$defs/Modelos",
      "default": {
        "construye": "haiku",
        "organiza": "sonnet",
        "planifica": "opus"
      }
    },
    "precios": {
      "description": "Precio por modelo, en USD por millón de tokens.",
      "type": "object",
      "additionalProperties": {
        "$ref": "#/$defs/Precio"
      },
      "default": {
        "claude-haiku-5-5": {
          "umbral_entrada": 100000,
          "usd_entrada": 0.1,
          "usd_entrada_alto": 0.5,
          "usd_salida": 0.5,
          "usd_salida_alto": 2.5
        },
        "claude-sonnet-5-5": {
          "umbral_entrada": null,
          "usd_entrada": 2.0,
          "usd_entrada_alto": null,
          "usd_salida": 10.0,
          "usd_salida_alto": null
        },
        "deepseek-flash": {
          "umbral_entrada": null,
          "usd_entrada": 0.14,
          "usd_entrada_alto": null,
          "usd_salida": 0.28,
          "usd_salida_alto": null
        },
        "deepseek-v4-pro": {
          "umbral_entrada": null,
          "usd_entrada": 1.74,
          "usd_entrada_alto": null,
          "usd_salida": 3.48,
          "usd_salida_alto": null
        }
      }
    },
    "proveedores": {
      "$ref": "#/$defs/ConfigProveedores",
      "default": {
        "anthropic_url": "https://api.anthropic.com",
        "deepseek_url": "https://api.deepseek.com",
        "espera_base_ms": 500,
        "timeout_juez_s": 1200,
        "timeout_s": 120
      }
    },
    "registro": {
      "description": "Registros privados de paquetes (`[[registro]]`). Solo los escribe el humano.",
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/RegistroConfig"
      }
    },
    "ruteo": {
      "$ref": "#/$defs/Ruteo",
      "default": {
        "api_equivalente": {
          "haiku": {
            "modelo": "claude-haiku-5-5",
            "proveedor": "anthropic"
          },
          "opus": {
            "modelo": "claude-opus-5-5",
            "proveedor": "anthropic"
          },
          "sonnet": {
            "modelo": "claude-sonnet-5-5",
            "proveedor": "anthropic"
          }
        },
        "delega": {
          "modelo": "deepseek-flash",
          "proveedor": "deepseek"
        },
        "juzga": {
          "modelo": "deepseek-flash",
          "proveedor": "deepseek",
          "respaldo": {
            "modelo": "claude-haiku-5-5",
            "proveedor": "anthropic"
          }
        },
        "verifica": {
          "modelo": "deepseek-flash",
          "proveedor": "deepseek",
          "respaldo": {
            "modelo": "claude-haiku-5-5",
            "proveedor": "anthropic"
          }
        }
      }
    },
    "umbral_wilson": {
      "description": "Cota inferior de Wilson (90%, unilateral) que un candidato debe alcanzar para entrar a la política.",
      "type": "number",
      "format": "double",
      "default": 0.6
    }
  },
  "$defs": {
    "Balance": {
      "description": "Sección `[balance]`: topes duros de gasto. Route los lee; nunca los sube.",
      "type": "object",
      "properties": {
        "api": {
          "$ref": "#/$defs/TopeMes",
          "default": {
            "tope_usd": 100.0
          }
        },
        "cupo": {
          "$ref": "#/$defs/TopeBloque",
          "default": {
            "bloque_horas": 5,
            "tope_usd": 35.0
          }
        },
        "nube": {
          "$ref": "#/$defs/TopeMes",
          "default": {
            "tope_usd": 250.0
          }
        },
        "prefiere_api_sobre": {
          "description": "Fracción del tope de cupo (en (0, 1]) a partir de la cual se prefiere la API.",
          "type": "number",
          "format": "double",
          "default": 0.9
        }
      }
    },
    "ConfigColab": {
      "description": "Sección `[colab]`: tope de duración de una corrida en Colab.",
      "type": "object",
      "properties": {
        "minutos_max": {
          "description": "Minutos máximos de una corrida en Colab. Al llegar al tope, Root libera el runtime.",
          "type": "number",
          "format": "double",
          "default": 120.0
        }
      }
    },
    "ConfigProveedores": {
      "description": "Sección `[proveedores]`: URLs de las APIs, espera base de reintentos y timeout por intento.",
      "type": "object",
      "properties": {
        "anthropic_url": {
          "type": "string",
          "default": "https://api.anthropic.com"
        },
        "deepseek_url": {
          "type": "string",
          "default": "https://api.deepseek.com"
        },
        "espera_base_ms": {
          "description": "Espera antes del primer reintento, en milisegundos. Cada reintento la duplica.",
          "type": "integer",
          "format": "uint64",
          "default": 500,
          "minimum": 0
        },
        "timeout_juez_s": {
          "description": "Tiempo máximo de cada intento de los roles Verifica y Juzga, en segundos. Un juez puede\nescribir hasta `MAX_TOKENS_JUEZ` tokens.",
          "type": "integer",
          "format": "uint64",
          "default": 1200,
          "minimum": 0
        },
        "timeout_s": {
          "description": "Tiempo máximo de cada intento HTTP, en segundos.",
          "type": "integer",
          "format": "uint64",
          "default": 120,
          "minimum": 0
        }
      }
    },
    "Jueces": {
      "description": "Sección `[jueces]`: el modelo del juez de la configuración de Cargo (decisión 47). Vía API.\nEl proveedor sale del nombre: `claude-*` es Anthropic y `deepseek-*` es DeepSeek.",
      "type": "object",
      "properties": {
        "config_modelo": {
          "type": "string",
          "default": "claude-sonnet-5-5"
        }
      }
    },
    "ModeloApi": {
      "type": "object",
      "properties": {
        "modelo": {
          "type": "string"
        },
        "proveedor": {
          "$ref": "#/$defs/Proveedor"
        },
        "respaldo": {
          "description": "Modelo que Route usa si el proveedor principal no está disponible. Solo en Verifica y Juzga.",
          "anyOf": [
            {
              "$ref": "#/$defs/ModeloApi"
            },
            {
              "type": "null"
            }
          ]
        }
      },
      "required": [
        "proveedor",
        "modelo"
      ]
    },
    "Modelos": {
      "type": "object",
      "properties": {
        "construye": {
          "type": "string",
          "default": "haiku"
        },
        "organiza": {
          "type": "string",
          "default": "sonnet"
        },
        "planifica": {
          "type": "string",
          "default": "opus"
        }
      }
    },
    "Precio": {
      "description": "Precio de un modelo, en USD por millón de tokens. Sus dos claves son obligatorias. El tramo alto\n(`umbral_entrada`, `usd_entrada_alto`, `usd_salida_alto`) es opcional, pero va completo o no va.",
      "type": "object",
      "properties": {
        "umbral_entrada": {
          "description": "Tokens de entrada por encima de los que el pedido entero cuesta al precio alto.",
          "type": [
            "integer",
            "null"
          ],
          "format": "uint64",
          "default": null,
          "minimum": 0
        },
        "usd_entrada": {
          "description": "USD por millón de tokens de entrada.",
          "type": "number",
          "format": "double"
        },
        "usd_entrada_alto": {
          "description": "USD por millón de tokens de entrada, en el tramo alto.",
          "type": [
            "number",
            "null"
          ],
          "format": "double",
          "default": null
        },
        "usd_salida": {
          "description": "USD por millón de tokens de salida.",
          "type": "number",
          "format": "double"
        },
        "usd_salida_alto": {
          "description": "USD por millón de tokens de salida, en el tramo alto.",
          "type": [
            "number",
            "null"
          ],
          "format": "double",
          "default": null
        }
      },
      "additionalProperties": false,
      "required": [
        "usd_entrada",
        "usd_salida"
      ]
    },
    "Proveedor": {
      "type": "string",
      "enum": [
        "anthropic",
        "deepseek"
      ]
    },
    "RegistroConfig": {
      "description": "Una entrada `[[registro]]`: un registro privado y la variable de entorno que tiene su credencial.\nRoot pasa la credencial solo a un repo cuyo registro tiene el mismo host que `url`.",
      "type": "object",
      "properties": {
        "credencial_env": {
          "description": "Variable de entorno de Root que tiene la credencial. Su valor nunca se escribe en un archivo.",
          "type": "string",
          "default": ""
        },
        "nombre": {
          "description": "Nombre del registro. Cargo lo usa en `CARGO_REGISTRIES_<NOMBRE>_TOKEN`.",
          "type": "string",
          "default": ""
        },
        "url": {
          "description": "Dirección del índice, con `https` y host.",
          "type": "string",
          "default": ""
        },
        "usuario_env": {
          "description": "Variable de entorno de Root con el usuario para pip. Sin ella, el usuario es `__token__`.",
          "type": [
            "string",
            "null"
          ],
          "default": null
        }
      }
    },
    "ReglasDecision": {
      "description": "Sección `[decision]`: las reglas que deciden `Listo`, `NoListo` o `Bloqueado` (Tarea 13b). Todo valor\nva de 0 a 1 y es finito; si no, la carga falla con código 2.",
      "type": "object",
      "properties": {
        "completitud_minima": {
          "description": "Completitud mínima para `Listo`.",
          "type": "number",
          "format": "double",
          "default": 0.9
        },
        "minimo": {
          "description": "Mínimo por dimensión, con la clave de la dimensión (`seguridad`, `diseno`...). Solo rige\nsobre dimensiones calibradas. Vacío por defecto.",
          "type": "object",
          "additionalProperties": {
            "type": "number",
            "format": "double"
          },
          "default": {}
        }
      }
    },
    "Ruteo": {
      "description": "Sección `[ruteo]`: modelo por rol cuando la vía es API, y equivalente de API de cada alias de cupo.",
      "type": "object",
      "properties": {
        "api_equivalente": {
          "description": "Clave: alias de cupo (\"opus\", \"sonnet\", \"haiku\"). Valor: su modelo en la API.",
          "type": "object",
          "additionalProperties": {
            "$ref": "#/$defs/ModeloApi"
          },
          "default": {
            "haiku": {
              "modelo": "claude-haiku-5-5",
              "proveedor": "anthropic"
            },
            "opus": {
              "modelo": "claude-opus-5-5",
              "proveedor": "anthropic"
            },
            "sonnet": {
              "modelo": "claude-sonnet-5-5",
              "proveedor": "anthropic"
            }
          }
        },
        "delega": {
          "$ref": "#/$defs/ModeloApi",
          "default": {
            "modelo": "deepseek-flash",
            "proveedor": "deepseek"
          }
        },
        "juzga": {
          "$ref": "#/$defs/ModeloApi",
          "default": {
            "modelo": "deepseek-flash",
            "proveedor": "deepseek",
            "respaldo": {
              "modelo": "claude-haiku-5-5",
              "proveedor": "anthropic"
            }
          }
        },
        "verifica": {
          "$ref": "#/$defs/ModeloApi",
          "default": {
            "modelo": "deepseek-flash",
            "proveedor": "deepseek",
            "respaldo": {
              "modelo": "claude-haiku-5-5",
              "proveedor": "anthropic"
            }
          }
        }
      }
    },
    "TopeBloque": {
      "description": "Tope de cupo por bloque de horas, imitando la ventana del plan.\nLa tabla tiene defaults: si falta una clave, vale el default de esta sección.",
      "type": "object",
      "properties": {
        "bloque_horas": {
          "type": "integer",
          "format": "uint32",
          "default": 5,
          "minimum": 0
        },
        "tope_usd": {
          "description": "USD equivalentes de lista por bloque.",
          "type": "number",
          "format": "double",
          "default": 35.0
        }
      }
    },
    "TopeMes": {
      "description": "Tope mensual. Sin defaults por campo: `api` y `nube` tienen topes distintos,\nasí que un `[balance.nube]` sin `tope_usd` debe fallar en vez de tomar el de la API.",
      "type": "object",
      "properties": {
        "tope_usd": {
          "description": "USD por mes calendario.",
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "tope_usd"
      ]
    }
  }
}
```

### conteo

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Conteo",
  "type": "object",
  "properties": {
    "pasa": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    },
    "total": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    }
  },
  "required": [
    "pasa",
    "total"
  ]
}
```

### eleccion

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Eleccion",
  "type": "object",
  "properties": {
    "modelo": {
      "type": [
        "string",
        "null"
      ]
    },
    "motivo": {
      "type": "string"
    },
    "proveedor": {
      "anyOf": [
        {
          "$ref": "#/$defs/Proveedor"
        },
        {
          "type": "null"
        }
      ]
    },
    "razonamiento": {
      "type": "boolean"
    },
    "rol": {
      "description": "Rol del pedido. `route` lo llena siempre; un juez armado a mano lo pone en `Juzga`.",
      "anyOf": [
        {
          "$ref": "#/$defs/Rol"
        },
        {
          "type": "null"
        }
      ],
      "default": null
    },
    "via": {
      "$ref": "#/$defs/Via"
    }
  },
  "required": [
    "via",
    "razonamiento",
    "motivo"
  ],
  "$defs": {
    "Proveedor": {
      "type": "string",
      "enum": [
        "anthropic",
        "deepseek"
      ]
    },
    "Rol": {
      "type": "string",
      "enum": [
        "planifica",
        "organiza",
        "construye",
        "verifica",
        "juzga",
        "delega"
      ]
    },
    "Via": {
      "description": "`Oraculo` significa no llamar a ningún modelo.",
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "cupo",
            "api",
            "oraculo"
          ]
        },
        {
          "description": "Sesión de Claude en la nube. Route nunca la elige; solo `observe::uso` la cuenta.",
          "type": "string",
          "const": "nube"
        }
      ]
    }
  }
}
```

### estadorequisito

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "EstadoRequisito",
  "type": "object",
  "properties": {
    "cumplimiento": {
      "$ref": "#/$defs/Cumplimiento"
    },
    "evidencia": {
      "description": "Ids de tests, escenarios o hallazgos.",
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "id": {
      "type": "string"
    },
    "nivel": {
      "description": "Nivel de completitud de 0 a 3. `None` si no hay veredicto o está pendiente (Tarea 9c).",
      "type": [
        "integer",
        "null"
      ],
      "format": "uint8",
      "default": null,
      "maximum": 255,
      "minimum": 0
    },
    "peso": {
      "type": "integer",
      "format": "uint8",
      "maximum": 255,
      "minimum": 0
    },
    "titulo": {
      "type": "string"
    }
  },
  "required": [
    "id",
    "titulo",
    "peso",
    "cumplimiento",
    "evidencia"
  ],
  "$defs": {
    "Cumplimiento": {
      "type": "string",
      "enum": [
        "cumplido",
        "parcial",
        "ausente",
        "sin_veredicto",
        "pendiente"
      ]
    }
  }
}
```

### evento

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Evento",
  "type": "object",
  "properties": {
    "corrida": {
      "type": "string"
    },
    "cost_usd": {
      "type": "number",
      "format": "double"
    },
    "extra": {
      "type": "object",
      "additionalProperties": true
    },
    "familia": {
      "type": [
        "string",
        "null"
      ]
    },
    "fase": {
      "type": "string"
    },
    "in_tokens": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0
    },
    "iso": {
      "type": "string"
    },
    "latency_s": {
      "type": "number",
      "format": "double"
    },
    "modelo": {
      "type": [
        "string",
        "null"
      ]
    },
    "out_tokens": {
      "type": "integer",
      "format": "uint64",
      "minimum": 0
    },
    "tipo": {
      "$ref": "#/$defs/TipoEvento"
    },
    "ts": {
      "type": "number",
      "format": "double"
    },
    "v": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    },
    "via": {
      "anyOf": [
        {
          "$ref": "#/$defs/Via"
        },
        {
          "type": "null"
        }
      ]
    }
  },
  "required": [
    "v",
    "ts",
    "iso",
    "corrida",
    "fase",
    "tipo",
    "in_tokens",
    "out_tokens",
    "cost_usd",
    "latency_s",
    "extra"
  ],
  "$defs": {
    "TipoEvento": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "llamada",
            "sesion",
            "humano",
            "plan_aprobado",
            "gate",
            "oraculo_leido"
          ]
        },
        {
          "description": "Monto que una llamada a la API reserva antes de enviar el pedido. Lo cierra el `Llamada`.",
          "type": "string",
          "const": "reserva"
        }
      ]
    },
    "Via": {
      "description": "`Oraculo` significa no llamar a ningún modelo.",
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "cupo",
            "api",
            "oraculo"
          ]
        },
        {
          "description": "Sesión de Claude en la nube. Route nunca la elige; solo `observe::uso` la cuenta.",
          "type": "string",
          "const": "nube"
        }
      ]
    }
  }
}
```

### fila

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Fila",
  "type": "object",
  "properties": {
    "commit_root": {
      "type": "string"
    },
    "config": {
      "type": "string"
    },
    "forma": {
      "description": "Forma de la corrida. Las filas anteriores a la decisión 28 la leen vacía.",
      "$ref": "#/$defs/Forma",
      "default": {
        "config": "",
        "roles": [],
        "verificador": false
      }
    },
    "id": {
      "type": "string"
    },
    "modelos": {
      "description": "Modelo que cumplió cada rol en la corrida.",
      "type": "object",
      "properties": {
        "construye": {
          "type": "string"
        },
        "delega": {
          "type": "string"
        },
        "juzga": {
          "type": "string"
        },
        "organiza": {
          "type": "string"
        },
        "planifica": {
          "type": "string"
        },
        "verifica": {
          "type": "string"
        }
      },
      "additionalProperties": false,
      "default": {}
    },
    "resumen": {
      "$ref": "#/$defs/Resumen"
    },
    "tarea": {
      "type": "string"
    },
    "ts": {
      "type": "number",
      "format": "double"
    },
    "v": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    },
    "veredicto": {
      "$ref": "#/$defs/Veredicto"
    }
  },
  "required": [
    "v",
    "id",
    "ts",
    "tarea",
    "config",
    "commit_root",
    "veredicto",
    "resumen"
  ],
  "$defs": {
    "Accion": {
      "description": "`refs` lleva ids de requisitos o hallazgos.",
      "type": "object",
      "properties": {
        "prioridad": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        },
        "refs": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "texto": {
          "type": "string"
        }
      },
      "required": [
        "prioridad",
        "texto",
        "refs"
      ]
    },
    "Confianza": {
      "description": "Lo que el veredicto no mide del todo: cuánto pesa la decisión (decisión 16). Todos sus campos\nsalen del `Veredicto`.",
      "type": "object",
      "properties": {
        "ambiguedades": {
          "description": "Ambigüedades abiertas de la ficha, como `línea <n>: <frase>`. Salen de la revisión guardada del juez.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "avisos": {
          "description": "Avisos del veredicto, por ejemplo una corrida de Colab con red.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "faltantes": {
          "description": "Ítems sin sonda para el lenguaje, de todos los puntajes, en el orden de las dimensiones.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/ItemFaltante"
          }
        },
        "inestables": {
          "description": "Tests inestables con su tasa. Claves `contrato:<id>` u `oculto:<id>`, como `Veredicto.tasas`.",
          "type": "object",
          "additionalProperties": {
            "$ref": "#/$defs/TasaTest"
          },
          "default": {}
        },
        "repeticiones": {
          "description": "Repeticiones del oculto que corrieron.",
          "type": "integer",
          "format": "uint32",
          "default": 0,
          "minimum": 0
        },
        "requisitos_pendientes": {
          "description": "Requisitos en estado `Pendiente`: un test falla por el entorno.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "sin_calibrar": {
          "description": "Dimensiones con veredicto que no están calibradas.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/Dimension"
          }
        },
        "sin_veredicto": {
          "description": "Dimensiones sin veredicto, con su motivo.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/ParteSinVeredicto"
          }
        },
        "tests_pendientes": {
          "description": "Tests de `[[entorno]]` con su motivo.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/Pendiente"
          }
        }
      }
    },
    "Conteo": {
      "type": "object",
      "properties": {
        "pasa": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "total": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        }
      },
      "required": [
        "pasa",
        "total"
      ]
    },
    "Cumplimiento": {
      "type": "string",
      "enum": [
        "cumplido",
        "parcial",
        "ausente",
        "sin_veredicto",
        "pendiente"
      ]
    },
    "Decision": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "listo",
            "no_listo",
            "bloqueado"
          ]
        },
        {
          "description": "Una dimensión que bloquea no tiene veredicto: no hay \"listo\" ni \"no listo\" que decir (decisión 48).",
          "type": "string",
          "const": "sin_decision"
        }
      ]
    },
    "Dimension": {
      "type": "string",
      "enum": [
        "funciona",
        "seguridad",
        "eficiencia",
        "estructura",
        "arquitectura",
        "diseno",
        "completitud",
        "entrega",
        "pruebas"
      ]
    },
    "Estado": {
      "type": "string",
      "enum": [
        "verde",
        "rojo",
        "sin_veredicto"
      ]
    },
    "EstadoRequisito": {
      "type": "object",
      "properties": {
        "cumplimiento": {
          "$ref": "#/$defs/Cumplimiento"
        },
        "evidencia": {
          "description": "Ids de tests, escenarios o hallazgos.",
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "id": {
          "type": "string"
        },
        "nivel": {
          "description": "Nivel de completitud de 0 a 3. `None` si no hay veredicto o está pendiente (Tarea 9c).",
          "type": [
            "integer",
            "null"
          ],
          "format": "uint8",
          "default": null,
          "maximum": 255,
          "minimum": 0
        },
        "peso": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        },
        "titulo": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "titulo",
        "peso",
        "cumplimiento",
        "evidencia"
      ]
    },
    "Forma": {
      "description": "Forma de una corrida (decisión 28): roles en orden, si hubo verificador y la configuración.",
      "type": "object",
      "properties": {
        "config": {
          "type": "string"
        },
        "roles": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Rol"
          }
        },
        "verificador": {
          "type": "boolean"
        }
      },
      "required": [
        "roles",
        "verificador",
        "config"
      ]
    },
    "Fuente": {
      "description": "Quién produjo un hallazgo: una sonda determinista o un juez con modelo.",
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "Sonda": {
              "type": "string"
            }
          },
          "additionalProperties": false,
          "required": [
            "Sonda"
          ]
        },
        {
          "type": "object",
          "properties": {
            "Juez": {
              "type": "object",
              "properties": {
                "modelo": {
                  "type": "string"
                }
              },
              "required": [
                "modelo"
              ]
            }
          },
          "additionalProperties": false,
          "required": [
            "Juez"
          ]
        }
      ]
    },
    "Hallazgo": {
      "description": "`severidad` va de 1 a 3.",
      "type": "object",
      "properties": {
        "archivo": {
          "type": "string"
        },
        "cita": {
          "type": "string"
        },
        "detalle": {
          "type": "string"
        },
        "dimension": {
          "$ref": "#/$defs/Dimension"
        },
        "fuente": {
          "$ref": "#/$defs/Fuente"
        },
        "id": {
          "description": "Estable: hash de dimensión, archivo y cita.",
          "type": "string",
          "default": ""
        },
        "impacto": {
          "description": "Ids de los requisitos que afecta.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "linea": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "severidad": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        }
      },
      "required": [
        "dimension",
        "severidad",
        "archivo",
        "linea",
        "cita",
        "detalle",
        "fuente"
      ]
    },
    "Informe": {
      "description": "`riesgos` y `proximos_pasos` tienen como máximo 5 entradas.",
      "type": "object",
      "properties": {
        "bloqueantes": {
          "description": "Ids de hallazgos.",
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "confianza": {
          "description": "Datos para decidir. Un informe viejo lee `Confianza` vacía.",
          "$ref": "#/$defs/Confianza",
          "default": {
            "ambiguedades": [],
            "avisos": [],
            "faltantes": [],
            "inestables": {},
            "repeticiones": 0,
            "requisitos_pendientes": [],
            "sin_calibrar": [],
            "sin_veredicto": [],
            "tests_pendientes": []
          }
        },
        "decision": {
          "$ref": "#/$defs/Decision"
        },
        "motivo": {
          "type": "string"
        },
        "proximos_pasos": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Accion"
          }
        },
        "requisitos": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/EstadoRequisito"
          }
        },
        "resumen": {
          "description": "Prosa. Cada oración cita un id.",
          "type": "string"
        },
        "riesgos": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Accion"
          }
        }
      },
      "required": [
        "decision",
        "motivo",
        "requisitos",
        "bloqueantes",
        "riesgos",
        "proximos_pasos",
        "resumen"
      ]
    },
    "ItemFaltante": {
      "description": "Un ítem de rúbrica que ninguna sonda mide para el lenguaje de la tarea, con el motivo (Tarea 31).",
      "type": "object",
      "properties": {
        "item": {
          "type": "string"
        },
        "motivo": {
          "type": "string"
        }
      },
      "required": [
        "item",
        "motivo"
      ]
    },
    "NivelItem": {
      "description": "Nivel de un ítem de rúbrica. `nivel` va de 0 a 3.",
      "type": "object",
      "properties": {
        "evidencia": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "fuente": {
          "$ref": "#/$defs/Fuente"
        },
        "item": {
          "type": "string"
        },
        "nivel": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        }
      },
      "required": [
        "item",
        "nivel",
        "evidencia",
        "fuente"
      ]
    },
    "ParteSinVeredicto": {
      "description": "Una dimensión sin veredicto y el motivo.",
      "type": "object",
      "properties": {
        "dimension": {
          "$ref": "#/$defs/Dimension"
        },
        "motivo": {
          "type": "string"
        }
      },
      "required": [
        "dimension",
        "motivo"
      ]
    },
    "Pendiente": {
      "description": "Test que falla por el entorno y no se cobra al producto. Figura en el informe con su motivo.",
      "type": "object",
      "properties": {
        "colab": {
          "description": "Resultado importado de Colab: \"pasa\", \"falla\", \"omite\" o \"inestable\". Ausente si no se importó.",
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "motivo": {
          "type": "string"
        },
        "tasa": {
          "description": "Tasa de aprobación si Colab dejó el test inestable entre repeticiones. Ausente en un test estable.",
          "anyOf": [
            {
              "$ref": "#/$defs/TasaTest"
            },
            {
              "type": "null"
            }
          ],
          "default": null
        },
        "test": {
          "description": "Id del test con forma `archivo::nombre`.",
          "type": "string"
        }
      },
      "required": [
        "test",
        "motivo"
      ]
    },
    "Puntaje": {
      "description": "`valor` va de 0 a 1.",
      "type": "object",
      "properties": {
        "calibrada": {
          "type": "boolean"
        },
        "dimension": {
          "$ref": "#/$defs/Dimension"
        },
        "exactitud_balanceada": {
          "type": [
            "number",
            "null"
          ],
          "format": "float"
        },
        "faltantes": {
          "description": "Ítems de la dimensión que ninguna sonda mide para el lenguaje de la tarea.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/ItemFaltante"
          }
        },
        "items": {
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/NivelItem"
          }
        },
        "sin_veredicto": {
          "description": "Por qué la dimensión no tiene veredicto. Con `Some`, `valor` vale 0 y ningún lector lo usa.",
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "valor": {
          "type": "number",
          "format": "float"
        }
      },
      "required": [
        "dimension",
        "valor",
        "calibrada"
      ]
    },
    "Resumen": {
      "type": "object",
      "properties": {
        "intervenciones": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "invalida": {
          "type": "boolean"
        },
        "minutos": {
          "type": "number",
          "format": "double"
        },
        "turnos": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "usd_api": {
          "type": "number",
          "format": "double"
        },
        "usd_cupo": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "usd_api",
        "usd_cupo",
        "minutos",
        "turnos",
        "intervenciones",
        "invalida"
      ]
    },
    "Rol": {
      "type": "string",
      "enum": [
        "planifica",
        "organiza",
        "construye",
        "verifica",
        "juzga",
        "delega"
      ]
    },
    "TasaTest": {
      "description": "Tasa de aprobación de un test inestable, con la cota inferior de Wilson al 90%.",
      "type": "object",
      "properties": {
        "pasa": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "total": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "wilson_inf": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "pasa",
        "total",
        "wilson_inf"
      ]
    },
    "Veredicto": {
      "type": "object",
      "properties": {
        "ambiguedades": {
          "description": "Ambigüedades abiertas de la ficha, como `línea <n>: <frase>`. Vacío si no hay revisión guardada\npara el spec. Un veredicto viejo lee vacío.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "avisos": {
          "description": "Problemas que no tumban la evaluación, por ejemplo un `colab.json` ilegible.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "commit": {
          "type": "string"
        },
        "contrato": {
          "$ref": "#/$defs/Conteo"
        },
        "costo_usd": {
          "type": "number",
          "format": "double"
        },
        "estado": {
          "$ref": "#/$defs/Estado"
        },
        "global": {
          "type": [
            "number",
            "null"
          ],
          "format": "float"
        },
        "hallazgos": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Hallazgo"
          }
        },
        "inestables": {
          "description": "Ids ordenados de los tests inestables del oculto. Un veredicto viejo lee vacío.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "inestables_contrato": {
          "description": "Ids ordenados de los tests inestables del contrato. Un veredicto viejo lee vacío.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "informe": {
          "anyOf": [
            {
              "$ref": "#/$defs/Informe"
            },
            {
              "type": "null"
            }
          ],
          "default": null
        },
        "oculto": {
          "$ref": "#/$defs/Conteo"
        },
        "ocultos_fallidos": {
          "description": "Ids ordenados de los tests ocultos que fallan de forma estable. Un requisito que no los prueba da\n`NoListo` (`test_oculto_fuera_de_requisitos`). Un veredicto viejo lee vacío.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "pendientes": {
          "description": "Tests de `[[entorno]]`, con su motivo. No entran al conteo. Un veredicto viejo lee vacío.\nCompletitud y el nivel por requisito llegan con la Tarea 9c; por ahora solo se listan.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/Pendiente"
          }
        },
        "puntajes": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Puntaje"
          }
        },
        "repeticiones": {
          "description": "Repeticiones del oculto que corrieron. Un veredicto viejo lee 0.",
          "type": "integer",
          "format": "uint32",
          "default": 0,
          "minimum": 0
        },
        "repo": {
          "type": "string"
        },
        "requisitos": {
          "description": "Estado de cada requisito de la tarea. Un veredicto viejo lee vacío.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/EstadoRequisito"
          }
        },
        "rubricas": {
          "description": "Hashes de rúbrica, por ejemplo \"diseno@<hash>\".",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "sin_calibrar": {
          "description": "Dimensiones con veredicto que todavía no están calibradas. Un veredicto viejo lee vacío.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/Dimension"
          }
        },
        "tarea": {
          "type": "string"
        },
        "tasas": {
          "description": "Tasa de cada test inestable. Claves `contrato:<id>` u `oculto:<id>`. Un veredicto viejo lee vacío.",
          "type": "object",
          "additionalProperties": {
            "$ref": "#/$defs/TasaTest"
          },
          "default": {}
        },
        "v": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        }
      },
      "required": [
        "v",
        "tarea",
        "repo",
        "commit",
        "contrato",
        "oculto",
        "puntajes",
        "hallazgos",
        "estado",
        "costo_usd"
      ]
    }
  }
}
```

### hallazgo

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Hallazgo",
  "description": "`severidad` va de 1 a 3.",
  "type": "object",
  "properties": {
    "archivo": {
      "type": "string"
    },
    "cita": {
      "type": "string"
    },
    "detalle": {
      "type": "string"
    },
    "dimension": {
      "$ref": "#/$defs/Dimension"
    },
    "fuente": {
      "$ref": "#/$defs/Fuente"
    },
    "id": {
      "description": "Estable: hash de dimensión, archivo y cita.",
      "type": "string",
      "default": ""
    },
    "impacto": {
      "description": "Ids de los requisitos que afecta.",
      "type": "array",
      "default": [],
      "items": {
        "type": "string"
      }
    },
    "linea": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    },
    "severidad": {
      "type": "integer",
      "format": "uint8",
      "maximum": 255,
      "minimum": 0
    }
  },
  "required": [
    "dimension",
    "severidad",
    "archivo",
    "linea",
    "cita",
    "detalle",
    "fuente"
  ],
  "$defs": {
    "Dimension": {
      "type": "string",
      "enum": [
        "funciona",
        "seguridad",
        "eficiencia",
        "estructura",
        "arquitectura",
        "diseno",
        "completitud",
        "entrega",
        "pruebas"
      ]
    },
    "Fuente": {
      "description": "Quién produjo un hallazgo: una sonda determinista o un juez con modelo.",
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "Sonda": {
              "type": "string"
            }
          },
          "additionalProperties": false,
          "required": [
            "Sonda"
          ]
        },
        {
          "type": "object",
          "properties": {
            "Juez": {
              "type": "object",
              "properties": {
                "modelo": {
                  "type": "string"
                }
              },
              "required": [
                "modelo"
              ]
            }
          },
          "additionalProperties": false,
          "required": [
            "Juez"
          ]
        }
      ]
    }
  }
}
```

### informe

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Informe",
  "description": "`riesgos` y `proximos_pasos` tienen como máximo 5 entradas.",
  "type": "object",
  "properties": {
    "bloqueantes": {
      "description": "Ids de hallazgos.",
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "confianza": {
      "description": "Datos para decidir. Un informe viejo lee `Confianza` vacía.",
      "$ref": "#/$defs/Confianza",
      "default": {
        "ambiguedades": [],
        "avisos": [],
        "faltantes": [],
        "inestables": {},
        "repeticiones": 0,
        "requisitos_pendientes": [],
        "sin_calibrar": [],
        "sin_veredicto": [],
        "tests_pendientes": []
      }
    },
    "decision": {
      "$ref": "#/$defs/Decision"
    },
    "motivo": {
      "type": "string"
    },
    "proximos_pasos": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Accion"
      }
    },
    "requisitos": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/EstadoRequisito"
      }
    },
    "resumen": {
      "description": "Prosa. Cada oración cita un id.",
      "type": "string"
    },
    "riesgos": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Accion"
      }
    }
  },
  "required": [
    "decision",
    "motivo",
    "requisitos",
    "bloqueantes",
    "riesgos",
    "proximos_pasos",
    "resumen"
  ],
  "$defs": {
    "Accion": {
      "description": "`refs` lleva ids de requisitos o hallazgos.",
      "type": "object",
      "properties": {
        "prioridad": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        },
        "refs": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "texto": {
          "type": "string"
        }
      },
      "required": [
        "prioridad",
        "texto",
        "refs"
      ]
    },
    "Confianza": {
      "description": "Lo que el veredicto no mide del todo: cuánto pesa la decisión (decisión 16). Todos sus campos\nsalen del `Veredicto`.",
      "type": "object",
      "properties": {
        "ambiguedades": {
          "description": "Ambigüedades abiertas de la ficha, como `línea <n>: <frase>`. Salen de la revisión guardada del juez.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "avisos": {
          "description": "Avisos del veredicto, por ejemplo una corrida de Colab con red.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "faltantes": {
          "description": "Ítems sin sonda para el lenguaje, de todos los puntajes, en el orden de las dimensiones.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/ItemFaltante"
          }
        },
        "inestables": {
          "description": "Tests inestables con su tasa. Claves `contrato:<id>` u `oculto:<id>`, como `Veredicto.tasas`.",
          "type": "object",
          "additionalProperties": {
            "$ref": "#/$defs/TasaTest"
          },
          "default": {}
        },
        "repeticiones": {
          "description": "Repeticiones del oculto que corrieron.",
          "type": "integer",
          "format": "uint32",
          "default": 0,
          "minimum": 0
        },
        "requisitos_pendientes": {
          "description": "Requisitos en estado `Pendiente`: un test falla por el entorno.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "sin_calibrar": {
          "description": "Dimensiones con veredicto que no están calibradas.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/Dimension"
          }
        },
        "sin_veredicto": {
          "description": "Dimensiones sin veredicto, con su motivo.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/ParteSinVeredicto"
          }
        },
        "tests_pendientes": {
          "description": "Tests de `[[entorno]]` con su motivo.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/Pendiente"
          }
        }
      }
    },
    "Cumplimiento": {
      "type": "string",
      "enum": [
        "cumplido",
        "parcial",
        "ausente",
        "sin_veredicto",
        "pendiente"
      ]
    },
    "Decision": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "listo",
            "no_listo",
            "bloqueado"
          ]
        },
        {
          "description": "Una dimensión que bloquea no tiene veredicto: no hay \"listo\" ni \"no listo\" que decir (decisión 48).",
          "type": "string",
          "const": "sin_decision"
        }
      ]
    },
    "Dimension": {
      "type": "string",
      "enum": [
        "funciona",
        "seguridad",
        "eficiencia",
        "estructura",
        "arquitectura",
        "diseno",
        "completitud",
        "entrega",
        "pruebas"
      ]
    },
    "EstadoRequisito": {
      "type": "object",
      "properties": {
        "cumplimiento": {
          "$ref": "#/$defs/Cumplimiento"
        },
        "evidencia": {
          "description": "Ids de tests, escenarios o hallazgos.",
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "id": {
          "type": "string"
        },
        "nivel": {
          "description": "Nivel de completitud de 0 a 3. `None` si no hay veredicto o está pendiente (Tarea 9c).",
          "type": [
            "integer",
            "null"
          ],
          "format": "uint8",
          "default": null,
          "maximum": 255,
          "minimum": 0
        },
        "peso": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        },
        "titulo": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "titulo",
        "peso",
        "cumplimiento",
        "evidencia"
      ]
    },
    "ItemFaltante": {
      "description": "Un ítem de rúbrica que ninguna sonda mide para el lenguaje de la tarea, con el motivo (Tarea 31).",
      "type": "object",
      "properties": {
        "item": {
          "type": "string"
        },
        "motivo": {
          "type": "string"
        }
      },
      "required": [
        "item",
        "motivo"
      ]
    },
    "ParteSinVeredicto": {
      "description": "Una dimensión sin veredicto y el motivo.",
      "type": "object",
      "properties": {
        "dimension": {
          "$ref": "#/$defs/Dimension"
        },
        "motivo": {
          "type": "string"
        }
      },
      "required": [
        "dimension",
        "motivo"
      ]
    },
    "Pendiente": {
      "description": "Test que falla por el entorno y no se cobra al producto. Figura en el informe con su motivo.",
      "type": "object",
      "properties": {
        "colab": {
          "description": "Resultado importado de Colab: \"pasa\", \"falla\", \"omite\" o \"inestable\". Ausente si no se importó.",
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "motivo": {
          "type": "string"
        },
        "tasa": {
          "description": "Tasa de aprobación si Colab dejó el test inestable entre repeticiones. Ausente en un test estable.",
          "anyOf": [
            {
              "$ref": "#/$defs/TasaTest"
            },
            {
              "type": "null"
            }
          ],
          "default": null
        },
        "test": {
          "description": "Id del test con forma `archivo::nombre`.",
          "type": "string"
        }
      },
      "required": [
        "test",
        "motivo"
      ]
    },
    "TasaTest": {
      "description": "Tasa de aprobación de un test inestable, con la cota inferior de Wilson al 90%.",
      "type": "object",
      "properties": {
        "pasa": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "total": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "wilson_inf": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "pasa",
        "total",
        "wilson_inf"
      ]
    }
  }
}
```

### nivelitem

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "NivelItem",
  "description": "Nivel de un ítem de rúbrica. `nivel` va de 0 a 3.",
  "type": "object",
  "properties": {
    "evidencia": {
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "fuente": {
      "$ref": "#/$defs/Fuente"
    },
    "item": {
      "type": "string"
    },
    "nivel": {
      "type": "integer",
      "format": "uint8",
      "maximum": 255,
      "minimum": 0
    }
  },
  "required": [
    "item",
    "nivel",
    "evidencia",
    "fuente"
  ],
  "$defs": {
    "Fuente": {
      "description": "Quién produjo un hallazgo: una sonda determinista o un juez con modelo.",
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "Sonda": {
              "type": "string"
            }
          },
          "additionalProperties": false,
          "required": [
            "Sonda"
          ]
        },
        {
          "type": "object",
          "properties": {
            "Juez": {
              "type": "object",
              "properties": {
                "modelo": {
                  "type": "string"
                }
              },
              "required": [
                "modelo"
              ]
            }
          },
          "additionalProperties": false,
          "required": [
            "Juez"
          ]
        }
      ]
    }
  }
}
```

### pedido

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Pedido",
  "type": "object",
  "properties": {
    "checkeable": {
      "type": "boolean"
    },
    "rol": {
      "$ref": "#/$defs/Rol"
    },
    "tipo": {
      "type": "string"
    },
    "tokens_estimados": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    }
  },
  "required": [
    "rol",
    "tipo",
    "checkeable",
    "tokens_estimados"
  ],
  "$defs": {
    "Rol": {
      "type": "string",
      "enum": [
        "planifica",
        "organiza",
        "construye",
        "verifica",
        "juzga",
        "delega"
      ]
    }
  }
}
```

### pendiente

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Pendiente",
  "description": "Test que falla por el entorno y no se cobra al producto. Figura en el informe con su motivo.",
  "type": "object",
  "properties": {
    "colab": {
      "description": "Resultado importado de Colab: \"pasa\", \"falla\", \"omite\" o \"inestable\". Ausente si no se importó.",
      "type": [
        "string",
        "null"
      ],
      "default": null
    },
    "motivo": {
      "type": "string"
    },
    "tasa": {
      "description": "Tasa de aprobación si Colab dejó el test inestable entre repeticiones. Ausente en un test estable.",
      "anyOf": [
        {
          "$ref": "#/$defs/TasaTest"
        },
        {
          "type": "null"
        }
      ],
      "default": null
    },
    "test": {
      "description": "Id del test con forma `archivo::nombre`.",
      "type": "string"
    }
  },
  "required": [
    "test",
    "motivo"
  ],
  "$defs": {
    "TasaTest": {
      "description": "Tasa de aprobación de un test inestable, con la cota inferior de Wilson al 90%.",
      "type": "object",
      "properties": {
        "pasa": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "total": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "wilson_inf": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "pasa",
        "total",
        "wilson_inf"
      ]
    }
  }
}
```

### politica

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Politica",
  "description": "Candidatos por clave `\"<rol>:<tipo>\"`, con el rol en snake_case.",
  "type": "object",
  "properties": {
    "estadisticas": {
      "type": "object",
      "additionalProperties": {
        "type": "array",
        "items": {
          "$ref": "#/$defs/Candidato"
        }
      },
      "default": {}
    },
    "propuestas": {
      "description": "Cambios sugeridos en los roles fijos. Route no los lee.",
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/Propuesta"
      }
    },
    "propuestas_forma": {
      "description": "Cambios de forma sugeridos. Route no los lee.",
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/PropuestaForma"
      }
    },
    "v": {
      "type": "integer",
      "format": "uint32",
      "default": 1,
      "minimum": 0
    }
  },
  "$defs": {
    "Candidato": {
      "description": "Un modelo con sus estadísticas. `wilson_inf` es la cota inferior de Wilson de la tasa de éxito.",
      "type": "object",
      "properties": {
        "exitos": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "modelo": {
          "type": "string"
        },
        "n": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "proveedor": {
          "anyOf": [
            {
              "$ref": "#/$defs/Proveedor"
            },
            {
              "type": "null"
            }
          ]
        },
        "usd_medio": {
          "type": "number",
          "format": "double"
        },
        "via": {
          "description": "`Cupo` o `Api`.",
          "$ref": "#/$defs/Via"
        },
        "wilson_inf": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "modelo",
        "via",
        "n",
        "exitos",
        "wilson_inf",
        "usd_medio"
      ]
    },
    "Diferencia": {
      "description": "Cambio esperado al aplicar la propuesta. `usd` positivo significa que ahorra.",
      "type": "object",
      "properties": {
        "exito": {
          "description": "Tasa de éxito propuesta menos la actual.",
          "type": "number",
          "format": "double"
        },
        "usd": {
          "description": "USD medio actual menos el propuesto.",
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "exito",
        "usd"
      ]
    },
    "Evidencia": {
      "description": "Evidencia de una propuesta: el modelo actual de `root.toml` y el propuesto.",
      "type": "object",
      "properties": {
        "actual": {
          "$ref": "#/$defs/EvidenciaModelo"
        },
        "propuesto": {
          "$ref": "#/$defs/EvidenciaModelo"
        }
      },
      "required": [
        "actual",
        "propuesto"
      ]
    },
    "EvidenciaForma": {
      "description": "Evidencia de una forma: n, éxitos, cota de Wilson, USD medio e ids de las filas.",
      "type": "object",
      "properties": {
        "exitos": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "filas": {
          "description": "Ids de las filas válidas que cuentan para esta forma, ordenados.",
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "forma": {
          "$ref": "#/$defs/Forma"
        },
        "n": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "usd_medio": {
          "type": "number",
          "format": "double"
        },
        "wilson_inf": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "forma",
        "n",
        "exitos",
        "wilson_inf",
        "usd_medio",
        "filas"
      ]
    },
    "EvidenciaModelo": {
      "description": "Estadísticas de un modelo en una tarea, con los ids de las filas que lo sustentan.",
      "type": "object",
      "properties": {
        "exitos": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "filas": {
          "description": "Ids de las filas válidas que cuentan para este modelo, ordenados.",
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "modelo": {
          "type": "string"
        },
        "n": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "usd_medio": {
          "type": "number",
          "format": "double"
        },
        "wilson_inf": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "modelo",
        "n",
        "exitos",
        "wilson_inf",
        "usd_medio",
        "filas"
      ]
    },
    "Forma": {
      "description": "Forma de una corrida (decisión 28): roles en orden, si hubo verificador y la configuración.",
      "type": "object",
      "properties": {
        "config": {
          "type": "string"
        },
        "roles": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Rol"
          }
        },
        "verificador": {
          "type": "boolean"
        }
      },
      "required": [
        "roles",
        "verificador",
        "config"
      ]
    },
    "Propuesta": {
      "description": "Cambio que Aprende sugiere en un rol fijo. El usuario lo aplica a mano en `root.toml`.",
      "type": "object",
      "properties": {
        "diferencia_esperada": {
          "$ref": "#/$defs/Diferencia"
        },
        "evidencia": {
          "$ref": "#/$defs/Evidencia"
        },
        "modelo_actual": {
          "type": "string"
        },
        "modelo_propuesto": {
          "type": "string"
        },
        "rol": {
          "$ref": "#/$defs/Rol"
        },
        "tarea": {
          "type": "string"
        }
      },
      "required": [
        "rol",
        "tarea",
        "modelo_actual",
        "modelo_propuesto",
        "evidencia",
        "diferencia_esperada"
      ]
    },
    "PropuestaForma": {
      "description": "Cambio de forma sugerido para una tarea. Route no lo lee: el usuario lo aplica a mano.",
      "type": "object",
      "properties": {
        "actual": {
          "$ref": "#/$defs/EvidenciaForma"
        },
        "diferencia_esperada": {
          "$ref": "#/$defs/Diferencia"
        },
        "propuesta": {
          "$ref": "#/$defs/EvidenciaForma"
        },
        "tarea": {
          "type": "string"
        }
      },
      "required": [
        "tarea",
        "actual",
        "propuesta",
        "diferencia_esperada"
      ]
    },
    "Proveedor": {
      "type": "string",
      "enum": [
        "anthropic",
        "deepseek"
      ]
    },
    "Rol": {
      "type": "string",
      "enum": [
        "planifica",
        "organiza",
        "construye",
        "verifica",
        "juzga",
        "delega"
      ]
    },
    "Via": {
      "description": "`Oraculo` significa no llamar a ningún modelo.",
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "cupo",
            "api",
            "oraculo"
          ]
        },
        {
          "description": "Sesión de Claude en la nube. Route nunca la elige; solo `observe::uso` la cuenta.",
          "type": "string",
          "const": "nube"
        }
      ]
    }
  }
}
```

### propuesta

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Propuesta",
  "description": "Cambio que Aprende sugiere en un rol fijo. El usuario lo aplica a mano en `root.toml`.",
  "type": "object",
  "properties": {
    "diferencia_esperada": {
      "$ref": "#/$defs/Diferencia"
    },
    "evidencia": {
      "$ref": "#/$defs/Evidencia"
    },
    "modelo_actual": {
      "type": "string"
    },
    "modelo_propuesto": {
      "type": "string"
    },
    "rol": {
      "$ref": "#/$defs/Rol"
    },
    "tarea": {
      "type": "string"
    }
  },
  "required": [
    "rol",
    "tarea",
    "modelo_actual",
    "modelo_propuesto",
    "evidencia",
    "diferencia_esperada"
  ],
  "$defs": {
    "Diferencia": {
      "description": "Cambio esperado al aplicar la propuesta. `usd` positivo significa que ahorra.",
      "type": "object",
      "properties": {
        "exito": {
          "description": "Tasa de éxito propuesta menos la actual.",
          "type": "number",
          "format": "double"
        },
        "usd": {
          "description": "USD medio actual menos el propuesto.",
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "exito",
        "usd"
      ]
    },
    "Evidencia": {
      "description": "Evidencia de una propuesta: el modelo actual de `root.toml` y el propuesto.",
      "type": "object",
      "properties": {
        "actual": {
          "$ref": "#/$defs/EvidenciaModelo"
        },
        "propuesto": {
          "$ref": "#/$defs/EvidenciaModelo"
        }
      },
      "required": [
        "actual",
        "propuesto"
      ]
    },
    "EvidenciaModelo": {
      "description": "Estadísticas de un modelo en una tarea, con los ids de las filas que lo sustentan.",
      "type": "object",
      "properties": {
        "exitos": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "filas": {
          "description": "Ids de las filas válidas que cuentan para este modelo, ordenados.",
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "modelo": {
          "type": "string"
        },
        "n": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "usd_medio": {
          "type": "number",
          "format": "double"
        },
        "wilson_inf": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "modelo",
        "n",
        "exitos",
        "wilson_inf",
        "usd_medio",
        "filas"
      ]
    },
    "Rol": {
      "type": "string",
      "enum": [
        "planifica",
        "organiza",
        "construye",
        "verifica",
        "juzga",
        "delega"
      ]
    }
  }
}
```

### puntaje

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Puntaje",
  "description": "`valor` va de 0 a 1.",
  "type": "object",
  "properties": {
    "calibrada": {
      "type": "boolean"
    },
    "dimension": {
      "$ref": "#/$defs/Dimension"
    },
    "exactitud_balanceada": {
      "type": [
        "number",
        "null"
      ],
      "format": "float"
    },
    "faltantes": {
      "description": "Ítems de la dimensión que ninguna sonda mide para el lenguaje de la tarea.",
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/ItemFaltante"
      }
    },
    "items": {
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/NivelItem"
      }
    },
    "sin_veredicto": {
      "description": "Por qué la dimensión no tiene veredicto. Con `Some`, `valor` vale 0 y ningún lector lo usa.",
      "type": [
        "string",
        "null"
      ],
      "default": null
    },
    "valor": {
      "type": "number",
      "format": "float"
    }
  },
  "required": [
    "dimension",
    "valor",
    "calibrada"
  ],
  "$defs": {
    "Dimension": {
      "type": "string",
      "enum": [
        "funciona",
        "seguridad",
        "eficiencia",
        "estructura",
        "arquitectura",
        "diseno",
        "completitud",
        "entrega",
        "pruebas"
      ]
    },
    "Fuente": {
      "description": "Quién produjo un hallazgo: una sonda determinista o un juez con modelo.",
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "Sonda": {
              "type": "string"
            }
          },
          "additionalProperties": false,
          "required": [
            "Sonda"
          ]
        },
        {
          "type": "object",
          "properties": {
            "Juez": {
              "type": "object",
              "properties": {
                "modelo": {
                  "type": "string"
                }
              },
              "required": [
                "modelo"
              ]
            }
          },
          "additionalProperties": false,
          "required": [
            "Juez"
          ]
        }
      ]
    },
    "ItemFaltante": {
      "description": "Un ítem de rúbrica que ninguna sonda mide para el lenguaje de la tarea, con el motivo (Tarea 31).",
      "type": "object",
      "properties": {
        "item": {
          "type": "string"
        },
        "motivo": {
          "type": "string"
        }
      },
      "required": [
        "item",
        "motivo"
      ]
    },
    "NivelItem": {
      "description": "Nivel de un ítem de rúbrica. `nivel` va de 0 a 3.",
      "type": "object",
      "properties": {
        "evidencia": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "fuente": {
          "$ref": "#/$defs/Fuente"
        },
        "item": {
          "type": "string"
        },
        "nivel": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        }
      },
      "required": [
        "item",
        "nivel",
        "evidencia",
        "fuente"
      ]
    }
  }
}
```

### replay

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Replay",
  "description": "Resultado de `aprende --replay`. Mejora positiva en `mejora_usd` significa que la política ahorra.",
  "type": "object",
  "properties": {
    "advertencia": {
      "type": [
        "string",
        "null"
      ]
    },
    "aprendida": {
      "$ref": "#/$defs/MetricaPolitica"
    },
    "entrenamiento": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    },
    "medicion": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    },
    "mejora_exito": {
      "type": [
        "number",
        "null"
      ],
      "format": "double"
    },
    "mejora_usd": {
      "type": [
        "number",
        "null"
      ],
      "format": "double"
    },
    "root_toml": {
      "$ref": "#/$defs/MetricaPolitica"
    }
  },
  "required": [
    "entrenamiento",
    "medicion",
    "aprendida",
    "root_toml"
  ],
  "$defs": {
    "MetricaPolitica": {
      "description": "Métrica de una política sobre las filas de medición que coinciden con su elección.\n`exito` y `usd_medio` son `None` si no hay filas cubiertas.",
      "type": "object",
      "properties": {
        "cubiertas": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "exito": {
          "type": [
            "number",
            "null"
          ],
          "format": "double"
        },
        "usd_medio": {
          "type": [
            "number",
            "null"
          ],
          "format": "double"
        }
      },
      "required": [
        "cubiertas"
      ]
    }
  }
}
```

### resumen

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Resumen",
  "type": "object",
  "properties": {
    "intervenciones": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    },
    "invalida": {
      "type": "boolean"
    },
    "minutos": {
      "type": "number",
      "format": "double"
    },
    "turnos": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    },
    "usd_api": {
      "type": "number",
      "format": "double"
    },
    "usd_cupo": {
      "type": "number",
      "format": "double"
    }
  },
  "required": [
    "usd_api",
    "usd_cupo",
    "minutos",
    "turnos",
    "intervenciones",
    "invalida"
  ]
}
```

### revision

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Revision",
  "description": "Resultado de revisar un spec. `ambiguedades` queda vacía hasta que corra el juez.\n`juez` es `\"ok\"` (el JSON parseó) o `\"sin_veredicto: <causa>\"`. Sin juez, la causa es `juez no corrido`.",
  "type": "object",
  "properties": {
    "ambiguedades": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Ambiguedad"
      }
    },
    "descartadas": {
      "description": "Frases que el juez citó y el gate de evidencia descartó.",
      "type": "array",
      "default": [],
      "items": {
        "type": "string"
      }
    },
    "hallazgos": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/HallazgoFicha"
      }
    },
    "juez": {
      "type": "string"
    }
  },
  "required": [
    "hallazgos",
    "ambiguedades",
    "juez"
  ],
  "$defs": {
    "Ambiguedad": {
      "description": "Una frase con dos lecturas posibles. La produce el juez y la acepta el gate de evidencia.",
      "type": "object",
      "properties": {
        "frase": {
          "type": "string"
        },
        "lectura_a": {
          "type": "string"
        },
        "lectura_b": {
          "type": "string"
        },
        "linea": {
          "type": "integer",
          "format": "uint",
          "minimum": 0
        }
      },
      "required": [
        "frase",
        "linea",
        "lectura_a",
        "lectura_b"
      ]
    },
    "HallazgoFicha": {
      "description": "Un hallazgo de las reglas deterministas. `linea` es la línea del SPEC.md.",
      "type": "object",
      "properties": {
        "cita": {
          "type": "string"
        },
        "detalle": {
          "type": "string"
        },
        "linea": {
          "type": "integer",
          "format": "uint",
          "minimum": 0
        },
        "regla": {
          "type": "string"
        }
      },
      "required": [
        "regla",
        "linea",
        "cita",
        "detalle"
      ]
    }
  }
}
```

### sinpresupuesto

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "SinPresupuesto",
  "description": "No hay vía con margen. `reabre` es el epoch en segundos del próximo momento con margen.",
  "type": "object",
  "properties": {
    "motivo": {
      "type": "string"
    },
    "reabre": {
      "type": "number",
      "format": "double"
    }
  },
  "required": [
    "reabre",
    "motivo"
  ]
}
```

### uso

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Uso",
  "description": "Gasto medido en USD equivalentes de lista. Lo arma `observe::uso`.",
  "type": "object",
  "properties": {
    "api_mes_usd": {
      "description": "Gasto de API del mes calendario.",
      "type": "number",
      "format": "double"
    },
    "cupo_bloque_inicio": {
      "description": "Epoch en segundos del primer evento del bloque vigente. `None` si no hay bloque.",
      "type": [
        "number",
        "null"
      ],
      "format": "double",
      "default": null
    },
    "cupo_bloque_usd": {
      "description": "Gasto de cupo del bloque vigente.",
      "type": "number",
      "format": "double"
    },
    "nube_mes_usd": {
      "description": "Gasto de sesiones en la nube del mes calendario.",
      "type": "number",
      "format": "double"
    }
  },
  "required": [
    "cupo_bloque_usd",
    "api_mes_usd",
    "nube_mes_usd"
  ]
}
```

### usomargenes

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "UsoMargenes",
  "description": "Salida de `r00t observa uso`: gasto, márgenes y el aviso de la nube.",
  "type": "object",
  "properties": {
    "aviso": {
      "type": "string"
    },
    "margenes": {
      "$ref": "#/$defs/Margenes"
    },
    "reservas_viejas": {
      "description": "Reservas de API abiertas con más de dos veces `timeout_juez_s`. Solo un humano las corrige.",
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/ReservaVieja"
      }
    },
    "uso": {
      "$ref": "#/$defs/Uso"
    }
  },
  "required": [
    "uso",
    "margenes",
    "aviso"
  ],
  "$defs": {
    "MargenTope": {
      "description": "Gasto y margen de un tope.",
      "type": "object",
      "properties": {
        "fraccion": {
          "description": "Usado sobre tope. Puede pasar de 1 si el gasto superó el tope.",
          "type": "number",
          "format": "double"
        },
        "lleno": {
          "description": "`usado_usd >= tope_usd`.",
          "type": "boolean"
        },
        "reabre": {
          "description": "Cupo: fin del bloque vigente. API y nube: inicio del mes siguiente. `None` si no está lleno.",
          "type": [
            "number",
            "null"
          ],
          "format": "double"
        },
        "restante_usd": {
          "description": "Lo que queda del tope, sin bajar de cero.",
          "type": "number",
          "format": "double"
        },
        "tope_usd": {
          "type": "number",
          "format": "double"
        },
        "usado_usd": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "usado_usd",
        "tope_usd",
        "fraccion",
        "restante_usd",
        "lleno"
      ]
    },
    "Margenes": {
      "description": "Márgenes de los tres topes del balance.",
      "type": "object",
      "properties": {
        "api": {
          "$ref": "#/$defs/MargenTope"
        },
        "cupo": {
          "$ref": "#/$defs/MargenTope"
        },
        "nube": {
          "$ref": "#/$defs/MargenTope"
        }
      },
      "required": [
        "cupo",
        "api",
        "nube"
      ]
    },
    "ReservaVieja": {
      "description": "Una reserva de API abierta y vieja. `r00t observa uso` la lista para que un humano la corrija.",
      "type": "object",
      "properties": {
        "antiguedad_s": {
          "description": "Segundos desde que se escribió la reserva, hasta el momento de la consulta.",
          "type": "number",
          "format": "double"
        },
        "corrida": {
          "type": "string"
        },
        "modelo": {
          "description": "Modelo de la llamada, si la reserva lo trae.",
          "type": [
            "string",
            "null"
          ]
        },
        "reserva": {
          "description": "Id de la reserva. `None` si la reserva no trae id.",
          "type": [
            "string",
            "null"
          ]
        },
        "usd": {
          "description": "Costo de peor caso que la reserva apartó.",
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "corrida",
        "antiguedad_s",
        "usd"
      ]
    },
    "Uso": {
      "description": "Gasto medido en USD equivalentes de lista. Lo arma `observe::uso`.",
      "type": "object",
      "properties": {
        "api_mes_usd": {
          "description": "Gasto de API del mes calendario.",
          "type": "number",
          "format": "double"
        },
        "cupo_bloque_inicio": {
          "description": "Epoch en segundos del primer evento del bloque vigente. `None` si no hay bloque.",
          "type": [
            "number",
            "null"
          ],
          "format": "double",
          "default": null
        },
        "cupo_bloque_usd": {
          "description": "Gasto de cupo del bloque vigente.",
          "type": "number",
          "format": "double"
        },
        "nube_mes_usd": {
          "description": "Gasto de sesiones en la nube del mes calendario.",
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "cupo_bloque_usd",
        "api_mes_usd",
        "nube_mes_usd"
      ]
    }
  }
}
```

### validez

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Validez",
  "description": "Resultado de `validar`. Cada métrica lleva su `valor`, su `cumple` y, si no se mide, su `motivo`.",
  "type": "object",
  "properties": {
    "acuerdo_decision": {
      "description": "Paquetes con la misma `Decision` que el usuario. `cumple`: 5 de 6 (decisión 13).",
      "$ref": "#/$defs/Metrica"
    },
    "avisos": {
      "description": "Casos que quedaron fuera o datos que faltan, con su motivo.",
      "type": "array",
      "items": {
        "type": "string"
      }
    },
    "exactitud_orden": {
      "description": "Fracción de pares de `orden.toml` que Root ordena como el resultado medido. Umbral 1,0.",
      "$ref": "#/$defs/Metrica"
    },
    "kappa_dimension": {
      "description": "Una entrada por dimensión, en el orden de `TODAS_LAS_DIMENSIONES`. `cumple`: kappa >= 0,6.",
      "type": "array",
      "items": {
        "$ref": "#/$defs/Kappa"
      }
    },
    "kappa_item": {
      "description": "Una entrada por ítem de los formularios. Sin umbral: un kappa bajo marca un ancla ambigua.",
      "type": "array",
      "items": {
        "$ref": "#/$defs/Kappa"
      }
    },
    "recall": {
      "description": "Fracción de defectos reales que Root encuentra con `archivo` y `linea` a ±3. Umbral 0,8.",
      "$ref": "#/$defs/Metrica"
    },
    "spearman": {
      "description": "Spearman entre el `orden` de los formularios y el orden de Root. Sin umbral.",
      "$ref": "#/$defs/Metrica"
    },
    "v": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    }
  },
  "required": [
    "v",
    "exactitud_orden",
    "recall",
    "acuerdo_decision",
    "kappa_dimension",
    "kappa_item",
    "spearman",
    "avisos"
  ],
  "$defs": {
    "Kappa": {
      "description": "Kappa de Cohen con pesos cuadráticos. `clave` es una dimensión (`seguridad`) o un ítem (`seguridad.secretos`).",
      "type": "object",
      "properties": {
        "casos": {
          "description": "Pares de niveles (usuario, Root) que entraron al kappa.",
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "clave": {
          "type": "string"
        },
        "cumple": {
          "type": [
            "boolean",
            "null"
          ],
          "default": null
        },
        "motivo": {
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "umbral": {
          "description": "Umbral de la dimensión (0,6). `None` en los ítems, que solo son diagnóstico de anclas.",
          "type": [
            "number",
            "null"
          ],
          "format": "float",
          "default": null
        },
        "valor": {
          "type": [
            "number",
            "null"
          ],
          "format": "float"
        }
      },
      "required": [
        "clave",
        "casos"
      ]
    },
    "Metrica": {
      "description": "Una medida de validez. Sin casos válidos, `valor` es `None` y `motivo` dice por qué.",
      "type": "object",
      "properties": {
        "aciertos": {
          "description": "Aciertos entre los casos. Solo en las fracciones; `None` en las demás.",
          "type": [
            "integer",
            "null"
          ],
          "format": "uint32",
          "default": null,
          "minimum": 0
        },
        "casos": {
          "description": "Casos válidos que entraron a la medida.",
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "cumple": {
          "description": "`true` si `valor` llega al umbral. `None` sin umbral o sin medir.",
          "type": [
            "boolean",
            "null"
          ],
          "default": null
        },
        "motivo": {
          "description": "Por qué la métrica no se mide. Solo con `valor` en `None`.",
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "umbral": {
          "description": "Umbral de la métrica. `None` si el plan no fija uno.",
          "type": [
            "number",
            "null"
          ],
          "format": "float",
          "default": null
        },
        "valor": {
          "type": [
            "number",
            "null"
          ],
          "format": "float"
        }
      },
      "required": [
        "casos"
      ]
    }
  }
}
```

### veredicto

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "Veredicto",
  "type": "object",
  "properties": {
    "ambiguedades": {
      "description": "Ambigüedades abiertas de la ficha, como `línea <n>: <frase>`. Vacío si no hay revisión guardada\npara el spec. Un veredicto viejo lee vacío.",
      "type": "array",
      "default": [],
      "items": {
        "type": "string"
      }
    },
    "avisos": {
      "description": "Problemas que no tumban la evaluación, por ejemplo un `colab.json` ilegible.",
      "type": "array",
      "default": [],
      "items": {
        "type": "string"
      }
    },
    "commit": {
      "type": "string"
    },
    "contrato": {
      "$ref": "#/$defs/Conteo"
    },
    "costo_usd": {
      "type": "number",
      "format": "double"
    },
    "estado": {
      "$ref": "#/$defs/Estado"
    },
    "global": {
      "type": [
        "number",
        "null"
      ],
      "format": "float"
    },
    "hallazgos": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Hallazgo"
      }
    },
    "inestables": {
      "description": "Ids ordenados de los tests inestables del oculto. Un veredicto viejo lee vacío.",
      "type": "array",
      "default": [],
      "items": {
        "type": "string"
      }
    },
    "inestables_contrato": {
      "description": "Ids ordenados de los tests inestables del contrato. Un veredicto viejo lee vacío.",
      "type": "array",
      "default": [],
      "items": {
        "type": "string"
      }
    },
    "informe": {
      "anyOf": [
        {
          "$ref": "#/$defs/Informe"
        },
        {
          "type": "null"
        }
      ],
      "default": null
    },
    "oculto": {
      "$ref": "#/$defs/Conteo"
    },
    "ocultos_fallidos": {
      "description": "Ids ordenados de los tests ocultos que fallan de forma estable. Un requisito que no los prueba da\n`NoListo` (`test_oculto_fuera_de_requisitos`). Un veredicto viejo lee vacío.",
      "type": "array",
      "default": [],
      "items": {
        "type": "string"
      }
    },
    "pendientes": {
      "description": "Tests de `[[entorno]]`, con su motivo. No entran al conteo. Un veredicto viejo lee vacío.\nCompletitud y el nivel por requisito llegan con la Tarea 9c; por ahora solo se listan.",
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/Pendiente"
      }
    },
    "puntajes": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/Puntaje"
      }
    },
    "repeticiones": {
      "description": "Repeticiones del oculto que corrieron. Un veredicto viejo lee 0.",
      "type": "integer",
      "format": "uint32",
      "default": 0,
      "minimum": 0
    },
    "repo": {
      "type": "string"
    },
    "requisitos": {
      "description": "Estado de cada requisito de la tarea. Un veredicto viejo lee vacío.",
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/EstadoRequisito"
      }
    },
    "rubricas": {
      "description": "Hashes de rúbrica, por ejemplo \"diseno@<hash>\".",
      "type": "array",
      "default": [],
      "items": {
        "type": "string"
      }
    },
    "sin_calibrar": {
      "description": "Dimensiones con veredicto que todavía no están calibradas. Un veredicto viejo lee vacío.",
      "type": "array",
      "default": [],
      "items": {
        "$ref": "#/$defs/Dimension"
      }
    },
    "tarea": {
      "type": "string"
    },
    "tasas": {
      "description": "Tasa de cada test inestable. Claves `contrato:<id>` u `oculto:<id>`. Un veredicto viejo lee vacío.",
      "type": "object",
      "additionalProperties": {
        "$ref": "#/$defs/TasaTest"
      },
      "default": {}
    },
    "v": {
      "type": "integer",
      "format": "uint32",
      "minimum": 0
    }
  },
  "required": [
    "v",
    "tarea",
    "repo",
    "commit",
    "contrato",
    "oculto",
    "puntajes",
    "hallazgos",
    "estado",
    "costo_usd"
  ],
  "$defs": {
    "Accion": {
      "description": "`refs` lleva ids de requisitos o hallazgos.",
      "type": "object",
      "properties": {
        "prioridad": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        },
        "refs": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "texto": {
          "type": "string"
        }
      },
      "required": [
        "prioridad",
        "texto",
        "refs"
      ]
    },
    "Confianza": {
      "description": "Lo que el veredicto no mide del todo: cuánto pesa la decisión (decisión 16). Todos sus campos\nsalen del `Veredicto`.",
      "type": "object",
      "properties": {
        "ambiguedades": {
          "description": "Ambigüedades abiertas de la ficha, como `línea <n>: <frase>`. Salen de la revisión guardada del juez.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "avisos": {
          "description": "Avisos del veredicto, por ejemplo una corrida de Colab con red.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "faltantes": {
          "description": "Ítems sin sonda para el lenguaje, de todos los puntajes, en el orden de las dimensiones.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/ItemFaltante"
          }
        },
        "inestables": {
          "description": "Tests inestables con su tasa. Claves `contrato:<id>` u `oculto:<id>`, como `Veredicto.tasas`.",
          "type": "object",
          "additionalProperties": {
            "$ref": "#/$defs/TasaTest"
          },
          "default": {}
        },
        "repeticiones": {
          "description": "Repeticiones del oculto que corrieron.",
          "type": "integer",
          "format": "uint32",
          "default": 0,
          "minimum": 0
        },
        "requisitos_pendientes": {
          "description": "Requisitos en estado `Pendiente`: un test falla por el entorno.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "sin_calibrar": {
          "description": "Dimensiones con veredicto que no están calibradas.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/Dimension"
          }
        },
        "sin_veredicto": {
          "description": "Dimensiones sin veredicto, con su motivo.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/ParteSinVeredicto"
          }
        },
        "tests_pendientes": {
          "description": "Tests de `[[entorno]]` con su motivo.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/Pendiente"
          }
        }
      }
    },
    "Conteo": {
      "type": "object",
      "properties": {
        "pasa": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "total": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        }
      },
      "required": [
        "pasa",
        "total"
      ]
    },
    "Cumplimiento": {
      "type": "string",
      "enum": [
        "cumplido",
        "parcial",
        "ausente",
        "sin_veredicto",
        "pendiente"
      ]
    },
    "Decision": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "listo",
            "no_listo",
            "bloqueado"
          ]
        },
        {
          "description": "Una dimensión que bloquea no tiene veredicto: no hay \"listo\" ni \"no listo\" que decir (decisión 48).",
          "type": "string",
          "const": "sin_decision"
        }
      ]
    },
    "Dimension": {
      "type": "string",
      "enum": [
        "funciona",
        "seguridad",
        "eficiencia",
        "estructura",
        "arquitectura",
        "diseno",
        "completitud",
        "entrega",
        "pruebas"
      ]
    },
    "Estado": {
      "type": "string",
      "enum": [
        "verde",
        "rojo",
        "sin_veredicto"
      ]
    },
    "EstadoRequisito": {
      "type": "object",
      "properties": {
        "cumplimiento": {
          "$ref": "#/$defs/Cumplimiento"
        },
        "evidencia": {
          "description": "Ids de tests, escenarios o hallazgos.",
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "id": {
          "type": "string"
        },
        "nivel": {
          "description": "Nivel de completitud de 0 a 3. `None` si no hay veredicto o está pendiente (Tarea 9c).",
          "type": [
            "integer",
            "null"
          ],
          "format": "uint8",
          "default": null,
          "maximum": 255,
          "minimum": 0
        },
        "peso": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        },
        "titulo": {
          "type": "string"
        }
      },
      "required": [
        "id",
        "titulo",
        "peso",
        "cumplimiento",
        "evidencia"
      ]
    },
    "Fuente": {
      "description": "Quién produjo un hallazgo: una sonda determinista o un juez con modelo.",
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "Sonda": {
              "type": "string"
            }
          },
          "additionalProperties": false,
          "required": [
            "Sonda"
          ]
        },
        {
          "type": "object",
          "properties": {
            "Juez": {
              "type": "object",
              "properties": {
                "modelo": {
                  "type": "string"
                }
              },
              "required": [
                "modelo"
              ]
            }
          },
          "additionalProperties": false,
          "required": [
            "Juez"
          ]
        }
      ]
    },
    "Hallazgo": {
      "description": "`severidad` va de 1 a 3.",
      "type": "object",
      "properties": {
        "archivo": {
          "type": "string"
        },
        "cita": {
          "type": "string"
        },
        "detalle": {
          "type": "string"
        },
        "dimension": {
          "$ref": "#/$defs/Dimension"
        },
        "fuente": {
          "$ref": "#/$defs/Fuente"
        },
        "id": {
          "description": "Estable: hash de dimensión, archivo y cita.",
          "type": "string",
          "default": ""
        },
        "impacto": {
          "description": "Ids de los requisitos que afecta.",
          "type": "array",
          "default": [],
          "items": {
            "type": "string"
          }
        },
        "linea": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "severidad": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        }
      },
      "required": [
        "dimension",
        "severidad",
        "archivo",
        "linea",
        "cita",
        "detalle",
        "fuente"
      ]
    },
    "Informe": {
      "description": "`riesgos` y `proximos_pasos` tienen como máximo 5 entradas.",
      "type": "object",
      "properties": {
        "bloqueantes": {
          "description": "Ids de hallazgos.",
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "confianza": {
          "description": "Datos para decidir. Un informe viejo lee `Confianza` vacía.",
          "$ref": "#/$defs/Confianza",
          "default": {
            "ambiguedades": [],
            "avisos": [],
            "faltantes": [],
            "inestables": {},
            "repeticiones": 0,
            "requisitos_pendientes": [],
            "sin_calibrar": [],
            "sin_veredicto": [],
            "tests_pendientes": []
          }
        },
        "decision": {
          "$ref": "#/$defs/Decision"
        },
        "motivo": {
          "type": "string"
        },
        "proximos_pasos": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Accion"
          }
        },
        "requisitos": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/EstadoRequisito"
          }
        },
        "resumen": {
          "description": "Prosa. Cada oración cita un id.",
          "type": "string"
        },
        "riesgos": {
          "type": "array",
          "items": {
            "$ref": "#/$defs/Accion"
          }
        }
      },
      "required": [
        "decision",
        "motivo",
        "requisitos",
        "bloqueantes",
        "riesgos",
        "proximos_pasos",
        "resumen"
      ]
    },
    "ItemFaltante": {
      "description": "Un ítem de rúbrica que ninguna sonda mide para el lenguaje de la tarea, con el motivo (Tarea 31).",
      "type": "object",
      "properties": {
        "item": {
          "type": "string"
        },
        "motivo": {
          "type": "string"
        }
      },
      "required": [
        "item",
        "motivo"
      ]
    },
    "NivelItem": {
      "description": "Nivel de un ítem de rúbrica. `nivel` va de 0 a 3.",
      "type": "object",
      "properties": {
        "evidencia": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "fuente": {
          "$ref": "#/$defs/Fuente"
        },
        "item": {
          "type": "string"
        },
        "nivel": {
          "type": "integer",
          "format": "uint8",
          "maximum": 255,
          "minimum": 0
        }
      },
      "required": [
        "item",
        "nivel",
        "evidencia",
        "fuente"
      ]
    },
    "ParteSinVeredicto": {
      "description": "Una dimensión sin veredicto y el motivo.",
      "type": "object",
      "properties": {
        "dimension": {
          "$ref": "#/$defs/Dimension"
        },
        "motivo": {
          "type": "string"
        }
      },
      "required": [
        "dimension",
        "motivo"
      ]
    },
    "Pendiente": {
      "description": "Test que falla por el entorno y no se cobra al producto. Figura en el informe con su motivo.",
      "type": "object",
      "properties": {
        "colab": {
          "description": "Resultado importado de Colab: \"pasa\", \"falla\", \"omite\" o \"inestable\". Ausente si no se importó.",
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "motivo": {
          "type": "string"
        },
        "tasa": {
          "description": "Tasa de aprobación si Colab dejó el test inestable entre repeticiones. Ausente en un test estable.",
          "anyOf": [
            {
              "$ref": "#/$defs/TasaTest"
            },
            {
              "type": "null"
            }
          ],
          "default": null
        },
        "test": {
          "description": "Id del test con forma `archivo::nombre`.",
          "type": "string"
        }
      },
      "required": [
        "test",
        "motivo"
      ]
    },
    "Puntaje": {
      "description": "`valor` va de 0 a 1.",
      "type": "object",
      "properties": {
        "calibrada": {
          "type": "boolean"
        },
        "dimension": {
          "$ref": "#/$defs/Dimension"
        },
        "exactitud_balanceada": {
          "type": [
            "number",
            "null"
          ],
          "format": "float"
        },
        "faltantes": {
          "description": "Ítems de la dimensión que ninguna sonda mide para el lenguaje de la tarea.",
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/ItemFaltante"
          }
        },
        "items": {
          "type": "array",
          "default": [],
          "items": {
            "$ref": "#/$defs/NivelItem"
          }
        },
        "sin_veredicto": {
          "description": "Por qué la dimensión no tiene veredicto. Con `Some`, `valor` vale 0 y ningún lector lo usa.",
          "type": [
            "string",
            "null"
          ],
          "default": null
        },
        "valor": {
          "type": "number",
          "format": "float"
        }
      },
      "required": [
        "dimension",
        "valor",
        "calibrada"
      ]
    },
    "TasaTest": {
      "description": "Tasa de aprobación de un test inestable, con la cota inferior de Wilson al 90%.",
      "type": "object",
      "properties": {
        "pasa": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "total": {
          "type": "integer",
          "format": "uint32",
          "minimum": 0
        },
        "wilson_inf": {
          "type": "number",
          "format": "double"
        }
      },
      "required": [
        "pasa",
        "total",
        "wilson_inf"
      ]
    }
  }
}
```
