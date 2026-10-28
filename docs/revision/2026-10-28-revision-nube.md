# Revisión del crate `root` (hito 1), 2026-10-28

## Alcance y método

- La revisión cubre el crate `root` (binario `r00t`) en el commit `573b2c4` de la rama `claude/root-e7813e`.
- Esta sesión no cambió código. Solo agrega este documento.
- El plugin ECC no está disponible en esta sesión. Por eso corrió `/code-review high`.
- `/code-review high` revisó solo el último commit (Tarea 6c-B, descarga de Python).
- Cuatro revisores cubrieron el crate entero, uno por área: fail-open, fugas, borrados con red, y topes de Route.
- Yo verifiqué en el código los hallazgos críticos y altos. La columna "Verificado" dice cómo.
- La severidad sigue esta escala: crítica, alta, media, baja.
- "Latente" significa que el defecto existe, pero ningún camino de producción llama a esa función hoy.

## Resultado de `cargo test`

- Los tests unitarios pasan: 692 de 692.
- Las suites de integración sin podman pasan: aprende, cli, colab, configs, esquema, ficha, goal, importar, interfaces, llama_cli, mcp, observe, proveedores, route, route_cli, track, uso_cli y vault.
- Fallan 97 tests en seis suites: contenedor, descarga, entrega, mutacion, oraculo y sondas.
- Las 97 fallas vienen del entorno. Este contenedor no tiene `podman` ni las imágenes `root-oraculo-rust` y `alpine:3.20`.
- Los mensajes de pánico son "no se pudo lanzar podman", "falta la imagen … en podman" y "Once instance has previously been poisoned".
- Ninguna falla es una falla real del código.

## Resumen

| Severidad | Cantidad |
|---|---|
| Crítica | 4 |
| Alta | 11 |
| Media | 16 |
| Baja | 19 |

Los cuatro hallazgos críticos son los primeros a corregir:

1. La descarga de Python ejecuta `setup.py` del proyecto con red.
2. `r00t ficha revisar` con juez llama a la API sin pasar por Route ni por el tope.
3. Un precio incompleto en `root.toml` vale 0 USD, y un precio NaN rompe `eventos.jsonl`.
4. Dos llamadas concurrentes pasan el mismo margen y superan el tope de API.

## Hallazgos críticos

### C1. La descarga de Python ejecuta código del proyecto con red

- **Dónde:** `src/oracle/descarga/inventario.rs:1846`, `src/oracle/descarga/python.rs:390-396`, `src/oracle/descarga/python.rs:89`.
- **Defecto:** `requisitos_pip` pone una ruta local (`/repo`, `/repo/sub`) en `especificadores`. El inventario no la lista ni la rechaza.
- **Defecto:** `pip download --only-binary=:all:` prepara una ruta local como árbol fuente. Eso ejecuta `setup.py` o el backend PEP 517.
- **Escenario:** el `requirements.txt` del proyecto tiene la línea `/repo`. El contenedor de descarga corre con `red: true` y monta `/repo`.
- **Escenario:** pip ejecuta el `setup.py` del builder con internet. El código puede leer el repo y mandarlo afuera, o bajar lo que quiera.
- **Contrato roto:** el plan dice que la fase de descarga nunca compila ni corre código del proyecto. El encabezado de `python.rs` dice "Nunca corre setup.py".
- **Verificado:** un revisor corrió el mismo comando de pip con un `setup.py` que crea un archivo marca. El archivo apareció. Yo confirmé que `es_directa` (`inventario.rs:1037`) no reconoce rutas locales.

### C2. `r00t ficha revisar` con juez llama a la API sin Route y sin tope

- **Dónde:** `src/lib.rs:260-272`, `src/ficha/juez.rs:45`, `src/proveedores/mod.rs:318-321`.
- **Defecto:** `juez_de` arma una `Eleccion` a mano. `JuezProveedor` llama a `proveedores::llamar` sin `chequear_tope`.
- **Defecto:** un modelo sin precio queda registrado con `cost_usd = 0` y la marca `precio_faltante`.
- **Escenario:** el gasto de API del mes es 150 USD sobre un tope de 100 USD. `r00t ficha revisar --juez-proveedor anthropic --juez-modelo X` igual envía el pedido.
- **Escenario:** si X no tiene precio, el evento registra 0 USD. El gasto real no entra a la cuenta del mes.
- **Latente también:** `src/oracle/juez.rs:88` tiene la misma llamada directa. Hoy solo la usan los tests.
- **Verificado:** el revisor lo reprodujo con un servidor HTTP falso. Yo confirmé en el código que no hay chequeo de tope en ese camino.

### C3. Un precio incompleto vale 0 USD y un precio NaN rompe el log

- **Dónde:** `src/config.rs:71-76`, `src/config.rs:252-291`, `src/route/comando.rs:189`.
- **Defecto:** `Precio` usa `#[serde(default)]` y acepta campos desconocidos. `validar` nunca revisa los precios.
- **Escenario:** el usuario escribe `usd_salda = 1.1` en vez de `usd_salida`. El precio de salida queda en 0.
- **Escenario:** `r00t llama --max-tokens 4000000000` pasa el chequeo previo. El revisor registró 5 millones de tokens de salida por 2,7e-6 USD.
- **Escenario:** `usd_entrada = nan` pasa el chequeo, porque `NaN > margen` es falso. El evento queda con `"cost_usd": null`.
- **Efecto del NaN:** después, `eventos.jsonl` no se puede leer. Toda llamada a `llama`, `route` y `observa uso` sale con código 3.
- **Defecto:** un precio negativo también pasa y baja el gasto medido del mes.
- **Contrato roto:** el desempate 6 dice "un tope no se esquiva por omisión".
- **Verificado:** el revisor lo reprodujo con el binario. Yo confirmé el `#[serde(default)]` y la falta de validación.

### C4. Sin reserva entre el chequeo y la llamada, los pedidos concurrentes superan el tope

- **Dónde:** `src/route/comando.rs:97-127`.
- **Defecto:** el gasto se lee en la línea 99 y se compara en la línea 113. La llamada sale en la línea 127. Ningún lock cubre ese tramo.
- **Defecto:** el evento se escribe recién cuando la llamada termina. Una llamada puede durar 3 × 120 s.
- **Escenario:** el gasto del mes es 99 USD. Cuatro procesos `r00t llama` estiman 0,88 USD cada uno. Los cuatro pasan el chequeo.
- **Resultado:** el mes cierra en 102,52 USD sobre un tope de 100 USD.
- **Alcance:** Graft, REA, la CLI y cada cliente MCP corren en procesos distintos. El caso es normal, no raro.
- **Verificado:** el revisor lo reprodujo con cuatro procesos en paralelo. Yo confirmé el orden de lectura, chequeo y llamada.

## Hallazgos altos

### A1. Copias de los tests ocultos quedan fuera de `oraculos/` y nunca se borran

- **Dónde:** `src/oracle/pendientes.rs:89-99`, `src/oracle/colab.rs:383`, `src/oracle/colab.rs:682`, `src/oracle/mod.rs:257`.
- **Defecto:** `pendientes` copia los `.py` ocultos y `oculto/conftest.py` a `$ROOT_HOME/pendientes/<tarea>-<ts>/tests/`.
- **Defecto:** Colab deja `pendientes.xml` en `$ROOT_HOME/tmp/colab-<sesion>/`. Ese XML trae ids y texto de fallas que citan el código oculto.
- **Defecto:** `limpiar_temporales` solo borra `evaluar-*`. Ningún código borra `pendientes/` ni `tmp/colab-*`.
- **Escenario:** el builder corre como el mismo usuario. `cat ~/.local/share/root/pendientes/memora-*/tests/*.py` muestra los tests ocultos.
- **Escenario:** las dos defensas del plan buscan la ruta `oraculos/`. Esta ruta no la contiene, así que ninguna defensa actúa.
- **Verificado:** yo leí la copia en `pendientes.rs:89-99`.

### A2. El builder puede leer el oráculo oculto desde el error de infraestructura

- **Dónde:** `src/oracle/mod.rs:449`, `src/oracle/mod.rs:461-466`, `src/oracle/contenedor.rs:119-134`, `src/mcp.rs:217`.
- **Defecto:** las suites de contrato y oculta montan todo `tarea.dir` en `/oraculo`, con `oculto/` incluido. El código del builder corre en ese contenedor.
- **Defecto:** si no hay junit, el error `Infra` lleva hasta 4000 caracteres del stderr del contenedor. Ese texto llega a la CLI y a `root_evaluar` por MCP.
- **Escenario:** durante los tests, el producto escribe `/oraculo/oculto/*` en stderr y mata a pytest. No queda junit.
- **Escenario:** el builder corre `r00t oraculo evaluar` o `root_evaluar`. Recibe hasta 4000 caracteres de tests ocultos por llamada.
- **Agravante:** el JSON del veredicto nombra tests ocultos en `inestables`, `tasas`, `pendientes` y las citas de requisitos. `runs.jsonl` guarda lo mismo.
- **Verificado:** yo leí el montaje y el mensaje de error en `mod.rs`.

### A3. La detección de lectura del oráculo no existe

- **Dónde:** `src/tipos.rs:313`, `src/observe/mod.rs:89`, `src/observe/claude.rs`, `src/cli.rs:227-243`, `src/mcp.rs:238-246`.
- **Defecto:** ningún código emite `TipoEvento::OraculoLeido`. Solo lo leen `resumir_eventos` y los tests.
- **Defecto:** la CLI `observa evento` y `root_registrar` no permiten ese tipo.
- **Defecto:** Observe solo lee el JSON de resultado de `claude -p`. Ese JSON no trae llamadas a herramientas, así que no hay transcript para buscar `oraculos/`.
- **Escenario:** el builder lee `oculto/`. La corrida nunca queda `invalida`.
- **Contrato roto:** el GOAL dice "una corrida que lo lee queda inválida". El plan (línea 151) pide la búsqueda, pero ninguna tarea la construye.
- **Nota:** una búsqueda por texto se esquiva con globs (`orac*/`), `cd` y ruta relativa, un symlink o las copias de A1.
- **Verificado:** `grep OraculoLeido src` da solo esos tres lugares.

### A4. Dos tests con la misma clave en junit ocultan una falla

- **Dónde:** `src/oracle/junit.rs:109`, `src/oracle/junit.rs:146-148`, `src/oracle/mod.rs:333-338`.
- **Defecto:** la clave es el último tramo de `classname` más `::name`. Un `BTreeMap` guarda solo el último caso con esa clave.
- **Escenario:** `test_set.TestBasico::test_vacio` falla y `test_get.TestBasico::test_vacio` pasa. Las dos claves son `TestBasico::test_vacio`.
- **Resultado:** el conteo oculto da 1 de 1 en vez de 1 de 2. El estado pasa de Rojo (código 1) a SinVeredicto (código 3).
- **Efecto extra:** en tests con clase, la clave usa el nombre de la clase, no el archivo. Los requisitos, `[[entorno]]` y `node_id` apuntan al lugar equivocado.
- **Alcance:** camino de producción (`r00t oraculo evaluar`).
- **Verificado:** el revisor lo probó con un crate aparte. Yo leí la clave en `junit.rs:109`.

### A5. El producto puede falsificar el junit, y el código de salida se ignora

- **Dónde:** `src/oracle/mod.rs:459-468`, `src/oracle/contenedor.rs:88`.
- **Defecto:** si existe el XML de junit, el código de salida de pytest no se mira. Los códigos 2, 3 y 5 cuentan como corrida normal.
- **Defecto:** `/salida` es escribible por el código bajo prueba.
- **Escenario:** el producto escribe `/salida/oculto.xml` con todos los tests en "pasa" y después mata a pytest. Root acepta ese XML.
- **Escenario:** el producto manda SIGINT a su grupo. Pytest deja un junit parcial con 3 de 3 en vez de 3 de 40.
- **Escenario:** el código 5 (ningún test recolectado) da 0 de 0 sin aviso.
- **Alcance:** camino de producción. Hoy `estado_de` nunca da Verde, pero el conteo oculto y la completitud quedan falsos.
- **Verificado:** yo leí que `resultado.codigo` solo aparece en el mensaje de error.

### A6. Los reintentos no vuelven a chequear el tope, y las fallas valen 0 USD

- **Dónde:** `src/proveedores/mod.rs:403-412`, `src/proveedores/mod.rs:340-344`, `src/proveedores/mod.rs:453-456`.
- **Defecto:** un timeout cuenta como error transitorio. Root reintenta hasta 3 veces con un solo chequeo previo.
- **Defecto:** el evento de falla guarda `cost_usd = 0`. El proveedor puede cobrar los tres intentos.
- **Escenario:** el servidor tarda más que `timeout_s`. Root envía 3 pedidos y registra un evento con `"intentos": 3, "cost_usd": 0.0`.
- **Escenario:** una respuesta 200 que `leer` no puede parsear también queda en 0 USD, aunque se cobró.
- **Verificado:** el revisor lo reprodujo con un servidor lento.

### A7. La estimación de entrada no es el peor caso

- **Dónde:** `src/route/comando.rs:186`.
- **Defecto:** la estimación usa un token cada tres caracteres.
- **Escenario:** un prompt en chino, japonés, con emoji, o en base64 tiene un token o más por carácter. La estimación queda tres veces corta o más.
- **Resultado:** una sola llamada cruza el tope en pleno vuelo. El chequeo previo existe justo para impedir eso.
- **Verificado:** yo leí la fórmula en `comando.rs:186`.

### A8. Colab corre código del builder con internet, junto a los tests ocultos

- **Dónde:** `src/oracle/pendientes.rs:89-99`, `src/oracle/colab.rs:130`, `src/lib.rs:118`.
- **Defecto:** el paquete de Colab trae el repo copiado y los tests ocultos juntos. `correr.sh` compila y corre el código del builder sin restricción de red.
- **Escenario:** un `build.rs` manda `../tests/*.py` a cualquier host. En local, `--network none` impide esto.
- **Defecto:** la preparación corre `curl https://sh.rustup.rs -sSf | sh` sin verificar un hash.
- **Contrato roto:** el plan dice que no hay corrida real hasta responder P35 y P37. Ningún código lo bloquea.
- **Verificado:** yo leí `GUION_PREPARAR` en `colab.rs:125-133`.

### A9. Las sondas de Rust dan puntaje perfecto cuando no leen nada (latente)

- **Dónde:** `src/oracle/perfiles/rust.rs:163-200`.
- **Defecto:** `archivos_rs` solo recorre `<repo>/src`. Ignora los errores de `read_dir` y de `read_to_string`.
- **Defecto:** `resultado()` siempre devuelve `Some(puntaje)`.
- **Escenario:** un workspace de Cargo tiene `crates/a/src/lib.rs` con `unsafe` y `unwrap()`. Seguridad, estructura y arquitectura dan 1,0 con 0 hallazgos.
- **Escenario:** un repo inexistente da el mismo 1,0.
- **Efecto extra:** `mapa::construir` devuelve un mapa vacío, y el juez trabaja sin código.
- **Verificado:** yo leí `recorrer` y `resultado` en `rust.rs`.

### A10. Un test omitido desaparece y el requisito queda Cumplido (latente)

- **Dónde:** `src/oracle/mod.rs:403-406`, `src/oracle/requisitos.rs:28-33`, `src/oracle/requisitos.rs:242-252`.
- **Defecto:** un test omitido en todas las corridas no entra a `estables`. `requisitos` solo copia `estables`.
- **Escenario:** `pruebas = ["test_login"]`. `t1` pasa y `t2` se omite por falta de red. El requisito queda Cumplido, nivel 3, completitud 1,0.
- **Escenario:** un test omitido que se nombra por id exacto queda "ausente". El requisito queda Ausente en vez de Pendiente.
- **Verificado:** yo leí el filtro `r != ResultadoTest::Omite` en `mod.rs:403`.

### A11. `podman build --network none` igual baja archivos con `ADD` (latente)

- **Dónde:** `src/oracle/entrega.rs:233-244`.
- **Defecto:** `--network none` cubre solo los pasos `RUN`. Buildah baja `ADD https://…` y `ADD <url git>` desde el host.
- **Defecto:** ni `entrega.rs` ni `configs.rs` detectan o rechazan esos `ADD`.
- **Escenario:** un Dockerfile con `ADD https://host/x /x` baja el archivo. La construcción cuenta como "sin red".
- **Certeza:** sale del comportamiento documentado de buildah. No se probó, porque falta podman.
- **Alcance:** hoy solo los tests llaman a `construir_dockerfile` y a `entregar`.

## Hallazgos medios

### M1. El tope de 250 USD de la nube nunca se aplica

- **Dónde:** `src/observe/uso.rs:139-165`, `src/lib.rs:483-501`.
- **Defecto:** el Gate de la nube se escribe solo cuando alguien corre `r00t observa uso`.
- **Escenario:** `observa claude --via nube` registra una sesión que supera 250 USD. No sale ningún Gate y nada frena el gasto.
- **Nota:** el plan acepta esto para el hito 1. El usuario quiere gastar el crédito de nube antes del 5 de noviembre, así que el riesgo es actual.

### M2. El tope del cupo se chequea solo cuando ya se alcanzó

- **Dónde:** `src/route/mod.rs:88-96`, `src/route/mod.rs:229`.
- **Defecto:** `elegir` nunca lee `Pedido.tokens_estimados`.
- **Escenario:** el bloque lleva 34,99 USD. Route igual devuelve `Cupo` para un pedido de cualquier tamaño.
- **Defecto:** el gasto del cupo entra al log solo con `observa claude`, después de la sesión. Las sesiones simultáneas no se ven.
- **Contrato roto:** la decisión 15 dice "un tope nunca se supera sin un gate humano".

### M3. Las sesiones importadas llevan la hora de importación

- **Dónde:** `src/observe/claude.rs:28`.
- **Defecto:** `eventos_desde` usa `ahora()`. No usa la hora de la sesión.
- **Escenario:** una sesión del 31 de octubre se importa el 1 de noviembre. El gasto cuenta en el mes de noviembre.
- **Escenario:** el bloque de 5 horas se ancla en la importación, no en el inicio de la sesión. `duration_ms` existe pero no se usa.

### M4. Variables de proxy del host entran a todos los contenedores

- **Dónde:** `src/oracle/contenedor.rs:55-60`, `src/oracle/entrega.rs:689-702`, `src/oracle/entrega.rs:233-244`.
- **Defecto:** podman pasa `HTTP_PROXY`, `HTTPS_PROXY` y `NO_PROXY` por defecto. Root nunca pasa `--http-proxy=false`.
- **Escenario:** el host tiene `HTTPS_PROXY=http://usuario:clave@proxy:3128`. El código del builder imprime la variable en stderr y aborta.
- **Resultado:** la clave del proxy llega al error `Infra` de A2. Con `entrega`, también llega a la cita de un hallazgo en `runs.jsonl`.
- **Nota:** con `--network none` el contenedor no llega al proxy. El defecto filtra la credencial, pero no abre red.
- **Certeza:** sale del comportamiento documentado de podman. No se probó, porque falta podman.

### M5. Sin `Cargo.lock`, el lock generado no pasa por las reglas de fuentes

- **Dónde:** `src/oracle/descarga/inventario.rs:731-739`, `src/oracle/descarga/inventario.rs:586-589`, `src/oracle/descarga.rs:413-432`.
- **Defecto:** el inventario solo lee el manifiesto raíz y los miembros del workspace.
- **Defecto:** después de `generar_lock`, Root lee el lock nuevo pero no revisa sus fuentes.
- **Escenario:** el repo no tiene lock. `sub = { path = "sub" }` no es miembro, y `sub/Cargo.toml` pide `x = { git = "http://host/x" }`.
- **Resultado:** `cargo generate-lockfile` clona esa fuente `http` con red. La fuente no aparece en el Gate, y nada rechaza el protocolo.
- **Contrato roto:** la Tarea 6c dice "nada sale a la red sin quedar en la lista".

### M6. Python: `-e git+…`, `-r` y `-c` se descartan en la descarga

- **Dónde:** `src/oracle/descarga/inventario.rs:1839-1840`, `src/oracle/descarga/inventario.rs:966-971`.
- **Defecto:** `requisitos_pip` ignora toda línea que empieza con `-`. El inventario, en cambio, trata `-e git+…` como fuente directa.
- **Escenario:** el `requirements.txt` tiene `-r requirements/base.txt` o `-e git+https://…@<sha>#egg=y`. La descarga termina bien sin esos paquetes.
- **Resultado:** los tests fallan con `ModuleNotFoundError`. Root cobra la falla al producto en vez de dar SinVeredicto.

### M7. Python: una fuente git sin referencia deja un árbol vacío

- **Dónde:** `src/oracle/descarga/python.rs:99-113`.
- **Defecto:** `comando_clon` siempre clona con `--no-checkout`. Solo hace checkout cuando hay un commit.
- **Escenario:** el requirements tiene `git+https://github.com/x/y#egg=y`. El clon deja solo `.git`, y `rev-parse HEAD` igual funciona.
- **Resultado:** la instalación sin red falla con "does not appear to be a Python project". Cada suite da error de infraestructura.

### M8. Python: la clave de la caché ignora la imagen

- **Dónde:** `src/oracle/descarga/python.rs:317-321`.
- **Defecto:** la clave es solo el hash de `requirements.txt`. `pip download --only-binary=:all:` elige wheels según el intérprete de la imagen.
- **Escenario:** una tarea usa `python:3.8` y otra usa `python:3.12`, con el mismo requirements. La segunda usa los wheels cp38 de la caché.
- **Resultado:** la instalación sin red falla. Todas las suites de la segunda tarea dan error de infraestructura hasta borrar la caché a mano.

### M9. Python: un sdist que otro paquete necesita corta la evaluación

- **Dónde:** `src/oracle/descarga/python.rs:408-438`.
- **Defecto:** después de bajar un sdist, Root saca su línea de `pendientes`. Pip vuelve a fallar en el mismo nombre si otro paquete lo necesita.
- **Escenario:** `A==1.0` (wheel, depende de B) y `B==2.0` (solo sdist). La segunda pasada falla en B, y B ya no está en `pendientes`.
- **Resultado:** el corte "sin-version-fijada", aunque B tiene versión fija.

### M10. Python: las fuentes git no se instalan desde `/cache` de solo lectura

- **Dónde:** `src/oracle/descarga/python.rs:540`, `src/oracle/mod.rs:456`.
- **Defecto:** la línea offline apunta a `/cache/src/<dir>`. La evaluación monta `/cache` en solo lectura.
- **Escenario:** pip 21.3 o más nuevo construye el directorio en su lugar. setuptools escribe `*.egg-info` en el árbol fuente y falla con "Read-only file system".
- **Resultado:** la suite no corre y da error de infraestructura.

### M11. Python: no se baja el backend de construcción de un sdist ni sus dependencias

- **Dónde:** `src/oracle/descarga/python.rs:449`.
- **Defecto:** para sdists, fuentes git y URLs, nada baja `setuptools`, `wheel`, `hatchling` ni sus `install_requires`.
- **Escenario:** el requirements fija un paquete que solo tiene sdist. La instalación sin red usa aislamiento de construcción.
- **Resultado:** falla con "Could not find a version that satisfies the requirement setuptools". Da error de infraestructura o ImportError en los tests.

### M12. Python: el preludio no tiene camino de respaldo

- **Dónde:** `src/oracle/descarga.rs:62`.
- **Defecto:** `PRELUDIO_PYTHON` usa `import ensurepip` para decidir si `venv` funciona.
- **Escenario:** en Debian o Ubuntu sin `python3-venv`, `ensurepip` importa pero `python3 -m venv` falla. La suite se salta y da error de infraestructura.
- **Defecto:** la rama `--target` nunca agrega `/tmp/pydeps/bin` al `PATH`. Un comando `pytest …` da "command not found".

### M13. cargo-mutants cortado por timeout cuenta como completo (latente)

- **Dónde:** `src/oracle/mutacion.rs:103-138`, `src/oracle/mutacion.rs:186-197`.
- **Defecto:** Root toma `Baseline` como marca de fin. Baseline es el primer escenario que corre cargo-mutants.
- **Defecto:** Root ignora `total_mutants`, descarta el código de salida de `timeout`, y no pasa `mutantes_max` a cargo-mutants.
- **Escenario:** el reporte tiene Baseline, 2 mutantes atrapados y `total_mutants: 50`. Da `valor = 1,0`, `muestra = 2` y `truncado = false`.
- **Contrato roto:** el desempate técnico 6 dice que un reporte truncado da SinVeredicto.
- **Certeza:** supone que cargo-mutants reescribe `outcomes.json` durante la corrida.

### M14. clippy puntúa una construcción que falló por otra causa (latente)

- **Dónde:** `src/oracle/perfiles/herramientas.rs:531-535`.
- **Defecto:** con `success = false` y sin errores de rustc, cualquier hallazgo de clippy cuenta como veredicto. Eso incluye advertencias.
- **Escenario:** el `build.rs` del crate B falla por falta de pkg-config. El crate A tiene una advertencia de clippy.
- **Resultado:** `Some(0,926)` sin motivo. B nunca se analizó.

### M15. El escenario monta toda la tarea dentro de la imagen del builder (latente)

- **Dónde:** `src/oracle/entrega.rs:682-700`, `src/oracle/entrega.rs:758-761`.
- **Defecto:** el contenedor del escenario monta todo `tarea.dir` (`oculto/`, `tarea.toml`) en `/oraculo`. Arranca desde la imagen que construyó el builder.
- **Defecto:** el `sh`, el `sleep` y el `timeout` de esa imagen son del builder.
- **Escenario:** la última línea de salida pasa a la cita del hallazgo o al motivo de Pendiente. Eso filtra una línea oculta por escenario y por corrida.
- **Corrección sugerida:** montar solo los scripts de escenario.

### M16. `leer_salida` sigue symlinks del contenedor (latente)

- **Dónde:** `src/oracle/perfiles/herramientas.rs:379-383`, `src/oracle/perfiles/herramientas.rs:75-80`.
- **Defecto:** el contenedor puede crear `/salida/x.log -> /home/u/.ssh/id_rsa`. Root lee ese archivo del host y lo pone en hallazgos.
- **Defecto:** la carpeta de salida se crea en `/tmp` con modo 0777. La Tarea 7f pide `$ROOT_HOME/tmp`.
- **Escenario:** otro usuario local planta archivos `.rc` o `.log` en esa carpeta.
- **Alcance:** hoy solo los tests llaman a `EjecutorContenedor`.

## Hallazgos bajos

### B1. Un paquete Python inexistente da error de infraestructura

- **Dónde:** `src/oracle/descarga/python.rs:400`.
- **Defecto:** `sin_wheel` captura "No matching distribution found" antes que `diagnosticar`.
- **Escenario:** `reqeusts==2.31.0` (error de tipeo). El JSON de PyPI da 404, y `bajar_sdist_pypi` lo vuelve `ErrorOraculo::Infra`.
- **Resultado:** la tarea da error en vez de SinVeredicto con un hallazgo "no-instala" de severidad 3.

### B2. Una referencia git con tag se rechaza, pero una sin referencia pasa

- **Dónde:** `src/oracle/descarga/python.rs:512`.
- **Defecto:** una referencia que no es hexadecimal de 7 a 40 caracteres da "referencia git no segura", severidad 3.
- **Escenario:** `@v1.2.0` corta la evaluación. Una URL sin referencia, que es menos reproducible, pasa con severidad 2.

### B3. Python: `#subdirectory=` se pierde y dos líneas del mismo repo chocan

- **Dónde:** `src/oracle/descarga/python.rs:510`, `src/oracle/descarga/python.rs:524`.
- **Defecto:** Root descarta el fragmento de la URL. El directorio del clon es el hash de la base.
- **Escenario:** `…@<sha>#subdirectory=pkg_a` instala la raíz del monorepo. Una segunda línea `#subdirectory=pkg_b` falla con "destination path already exists".

### B4. Un error de parseo de TOML copia una línea con token sin redactar

- **Dónde:** `src/oracle/descarga/inventario.rs:534`, `src/oracle/descarga/inventario.rs:710`.
- **Defecto:** el mensaje de error de `toml` cita la línea. `redactar` solo oculta la forma `://…@`.
- **Escenario:** `.cargo/config.toml` tiene `token = "cio_SECRET" basura`. El motivo del rechazo trae el token.
- **Resultado:** el token llega al Gate de `eventos.jsonl` y al `detalle` del hallazgo.
- **Verificado:** el revisor confirmó el formato de error de `toml` 1.1.8.

### B5. `podman pull` usa las credenciales de registro del host

- **Dónde:** `src/oracle/descarga.rs:937`.
- **Defecto:** Root no pasa `--authfile` ni `--creds`. Podman lee el `auth.json` del usuario.
- **Escenario:** el `FROM` del builder nombra una imagen privada del usuario. Root la baja con las credenciales del usuario.
- **Contrato roto:** el código dice "Root no pasa credenciales (P21)". El camino 401 nunca se activa.

### B6. `ROOT_HOME` no se valida

- **Dónde:** `src/config.rs:22-26`, `src/oracle/mod.rs:124-126`, `src/oracle/mod.rs:249-283`.
- **Defecto:** el guardia de `borrado` solo rechaza una raíz igual a `/`.
- **Escenario:** con `ROOT_HOME=/`, la limpieza borra todo `/tmp/evaluar-*` del usuario con más de 24 h, aunque sea de otro programa.
- **Escenario:** con `ROOT_HOME=$HOME`, Root borra en `~/tmp/evaluar-*` y en `~/cache/{rust,python}/<hash>`.

### B7. El nombre de la tarea no se valida en `pendientes`

- **Dónde:** `src/oracle/pendientes.rs:83-89`.
- **Defecto:** `tarea.nombre` entra a `home/pendientes/{nombre}-{seg}` sin `nombre_seguro`.
- **Escenario:** `nombre = "../../../x"` hace que `r00t oraculo pendientes` escriba una copia del repo fuera de `ROOT_HOME`.
- **Nota:** el revisor de fugas dice que `Tarea::cargar` rechaza separadores. Hay que confirmar si `crear` recibe una tarea cargada por otro camino.

### B8. Una rotación del log durante la lectura puede perder gasto

- **Dónde:** `src/observe/jsonl.rs:69-104`, `src/observe/uso.rs:107-115`.
- **Defecto:** Root lista los archivos primero y bloquea cada uno después.
- **Escenario:** `agregar` rota `eventos.jsonl` a `.N` en el medio. Ese archivo no se lee, y hasta 50 MB de eventos cuentan como 0 USD en ese chequeo.

### B9. `r00t llama` con el tope de API lleno da un error engañoso

- **Dónde:** `src/route/comando.rs:107-111`.
- **Defecto:** los roles delega, verifica y juzga caen al cupo. El usuario ve "este rol va por cupo" con código 2.
- **Contrato roto:** el plan espera un Gate de presupuesto con código 3.

### B10. El razonamiento es solo una etiqueta

- **Dónde:** `src/proveedores/anthropic.rs:10-28`.
- **Defecto:** `Eleccion.razonamiento` nunca llega al proveedor.
- **Escenario:** un modelo de Anthropic para verifica o juzga corre sin pensamiento extendido. El invariante 2 no se cumple.

### B11. Aprende no recibe datos de los eventos

- **Dónde:** `src/proveedores/mod.rs:488-497`.
- **Defecto:** los eventos de llamada no llevan `extra.rol`, `extra.tipo` ni `extra.exito`. Ningún código los escribe.
- **Resultado:** `aprende::acumular_evento` nunca produce candidatos para delega, verifica y juzga.

### B12. Un costo importado puede valer 0 o ser negativo

- **Dónde:** `src/observe/claude.rs:48`.
- **Defecto:** si una entrada de `modelUsage` no trae `costUSD`, vale 0. Root ignora `total_cost_usd` cuando existe `modelUsage`.
- **Defecto:** Root acepta costos negativos.

### B13. `combinar` sin partes devuelve 1,0

- **Dónde:** `src/oracle/perfiles/mod.rs:117`, `src/oracle/perfiles/mod.rs:124`.
- **Defecto:** `combinar(dim, vec![], …)` da `Some(1.0)`.
- **Alcance:** hoy solo los tests lo llaman.

### B14. Un test con skip y error cuenta como omitido

- **Dónde:** `src/oracle/junit.rs:139-145`.
- **Defecto:** `<skipped/>` más `<error/>` da `Omite`. Un error de teardown después de un skip se pierde.

### B15. `ficha revisar` sale con 0 cuando el juez no corrió

- **Dónde:** `src/ficha.rs:170-178`.
- **Defecto:** sin `--juez-*`, el juez queda "no corrido" y el código es 0.
- **Contrato:** el plan dice "el código 0 exige las dos capas limpias". `docs/interfaces.md` dice 0 para "sin hallazgos". Los dos documentos no coinciden.

### B16. Los resultados de Colab no se atan a un commit

- **Dónde:** `src/oracle/pendientes.rs:115-133`, `src/lib.rs:177-178`.
- **Defecto:** `colab.json` usa solo el id del test como clave. `--importar` ignora `--repo`.
- **Escenario:** un "pasa" de un commit viejo aparece en veredictos nuevos.

### B17. Algunos borrados no pasan por `src/borrado.rs`

- **Dónde:** `src/oracle/mod.rs:128`, `src/oracle/mod.rs:143`, `src/oracle/descarga.rs:474-477`, `src/oracle/entrega.rs:222`, `src/oracle/perfiles/herramientas.rs:75`, `src/aprende.rs:443`.
- **Defecto:** el `drop` de `TempDir` y `NamedTempFile` borra sin pasar por `borrado.rs`.
- **Nota:** solo tocan carpetas que Root creó, y `remove_dir_all` no sigue symlinks. El riesgo práctico es bajo, pero rompe la regla escrita.

### B18. `tmp/colab-*` nunca se limpia

- **Dónde:** `src/oracle/colab.rs:383`, `src/oracle/mod.rs:257`.
- **Defecto:** `limpiar_temporales` solo borra `evaluar-*`. Las carpetas `colab-<sesion>` crecen sin límite.
- **Nota:** este hallazgo se suma a A1, porque esas carpetas guardan texto de tests ocultos.

### B19. Carrera teórica dentro del guardia de borrado

- **Dónde:** `src/borrado.rs:62`, `src/borrado.rs:19-35`.
- **Defecto:** Root resuelve el padre y después borra por ruta. Un cambio del padre por un symlink en el medio haría seguir el link.
- **Nota:** ningún flujo actual tiene otro proceso activo en ese momento.

## Lo que se revisó y está bien

- `estado_de` nunca devuelve Verde, y `global` siempre es `None`. Ninguna dimensión sin calibrar entra al puntaje global.
- Un fallo de podman, un timeout, una señal o el código 125 dan `Infra`. Un junit ausente, vacío o roto da código 3.
- Una suite 0 de 0 da SinVeredicto. Un test que falta en una corrida queda inestable y nunca pasa.
- Las divisiones por cero de rúbrica, completitud, muestra de mutantes y eficiencia están protegidas.
- Un error del juez, un JSON roto o claves faltantes dan SinVeredicto.
- El guardia de `borrado.rs` compara por componente, normaliza `..` y rechaza `/` aunque llegue por symlink.
- Los contenedores de evaluación usan `--network none`. Los de descarga no montan `/oraculo`.
- La descarga de Rust corre desde `/tmp` con `--manifest-path`, así que no lee el `.cargo/config.toml` del repo.
- Las URLs con credenciales se rechazan en el inventario sin citar la credencial.
- Las claves de API no aparecen en eventos, errores, `Debug` ni argumentos de podman.
- El mes es calendario y en hora local. El bloque de 5 horas se ancla bien. `api_ok` usa `<`.
- Una línea ilegible en `eventos.jsonl` es un error (falla cerrado). Un `root.toml` sin topes usa los valores de la decisión 15.
- Ni `aprende`, ni `politica.json`, ni una variable de entorno pueden subir un tope.
- `root_llama` por MCP pasa por el mismo `llamar_con_chequeo` que la CLI.
