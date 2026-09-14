# Spec: Robustez del modulo de plugins

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.

## Contrato

Paths modificados: `mmorch/plugins.py` y `mmorch/plugin_worker.py`. Ningun otro archivo se toca (tests incluidos).

### `mmorch/plugins.py`

```python
def load_manifest(plugin_dir: Path, *, allow: set[str]) -> dict
```
Devuelve `{"name": str, "version": str, "entry": str, "dir": str, "capabilities": list, "contributes": list}`, con `"dir" = str(Path(plugin_dir).resolve())` y `"entry"` igual al del `plugin.json`. `allow` no cambia el comportamiento en esta feature.

```python
def invoke(manifest: dict, method: str, args: dict, *, host_services: dict, timeout: float) -> dict
```
Retorna siempre `{"ok": True, "value": <json>}` o `{"ok": False, "error": str}`. Nunca propaga excepciones del worker ni del plugin (ni `JSONDecodeError`, ni `BrokenPipeError`, ni `TimeoutError`).

Lanza el worker con:
`subprocess.Popen([sys.executable, "-m", "mmorch.plugin_worker"], stdin=PIPE, stdout=PIPE, stderr=DEVNULL, text=True, encoding="utf-8", cwd=manifest["dir"], env={**os.environ, "PYTHONPATH": REPO_ROOT + os.pathsep + os.environ.get("PYTHONPATH", "")})`, con `REPO_ROOT = str(Path(__file__).resolve().parents[1])`.

Protocolo NDJSON, un mensaje por linea UTF-8:
- host -> worker: `{"type": "invoke", "method": str, "args": <json>, "host_services": <json>, "dir": str, "entry": str}`
- worker -> host: `{"type": "result", "value": <json>}` | `{"type": "error", "error": str}`

`invoke` arma `deadline = time.monotonic() + timeout`, levanta `threading.Thread(target=_pump, args=(proc, q), daemon=True)`, envia el request con `_send` y consume con `_next_message`. En `finally`: cierra `proc.stdin`, mata el proceso si sigue vivo y joinea el thread.

```python
def _decode_line(line: str) -> dict | None
```
Hace `json.loads(line)` y devuelve el objeto solo si es `dict` con `isinstance(obj.get("type"), str)`; en cualquier otro caso (linea vacia, texto no-JSON, array, numero, objeto sin `type` str) devuelve `None`. No levanta excepciones.

```python
def _send(proc: subprocess.Popen[str], msg: dict) -> bool
```
Escribe `json.dumps(msg, ensure_ascii=False) + "\n"` en `proc.stdin` y hace `flush()`. Devuelve `True`; ante `BrokenPipeError` (o `OSError`/`ValueError` de stdin ya cerrado) devuelve `False` sin propagar.

```python
def _pump(proc: subprocess.Popen[str], q: queue.Queue[str | None]) -> None
```
Target del thread lector: por cada linea cruda de `proc.stdout` hace `q.put(line)`; al EOF hace `q.put(None)`. Es el unico lector de `proc.stdout`.

```python
class _Eof: ...
EOF: _Eof
def _next_message(q: queue.Queue[str | None], deadline: float) -> dict | _Eof | None
```
`q.get(timeout=max(0.0, deadline - time.monotonic()))`. Devuelve:
- `None` si vence `queue.Empty` (deadline agotado),
- `EOF` si lee el centinela `None` (el worker cerro stdout),
- el dict parseado por `_decode_line`; si la linea no parsea, la descarta y sigue el loop.

```python
def _map_error_payload(payload: dict) -> dict
```
Traduce `{"type": "error", "error": <str>}` a `{"ok": False, "error": <str>}` sin prefijos extra.

### `mmorch/plugin_worker.py`

```python
_STDOUT: TextIO                     # sys.stdout capturado al importar el modulo
def _send(msg: dict) -> bool        # escribe NDJSON en _STDOUT y hace flush; False ante BrokenPipeError
def _load_entry(dir: str, entry: str) -> types.ModuleType
def main() -> int
```
`_load_entry` hace `sys.path.insert(0, dir)`, `importlib.util.spec_from_file_location("<plugin>_main", os.path.join(dir, entry))`, `module_from_spec` y `exec_module`; propaga lo que lance el import (`SyntaxError`, `ImportError`, `FileNotFoundError`, ...).

`main()`, en este orden exacto:
1. `sys.stdout = sys.stderr` — redirect ANTES de tocar el entry. `_STDOUT` queda apuntando al pipe de protocolo.
2. Lee una linea de `sys.stdin`; si no parsea como el request de `invoke`, `_send({"type": "error", "error": "bad request"})` y `return 1`.
3. `module = _load_entry(dir, entry)` dentro de `try`; ante `BaseException as e`: `_send({"type": "error", "error": f"load failed: {type(e).__name__}: {e}"})` y `return 1`.
4. `fn = getattr(module, method, None)`; si no es callable: `_send({"type": "error", "error": f"call failed: method '{method}' not found"})` y `return 1`.
5. `value = fn(args, host_services)` dentro de `try`; ante `BaseException as e`: `_send({"type": "error", "error": f"call failed: {type(e).__name__}: {e}"})` y `return 1`.
6. Serializa `{"type": "result", "value": value}` dentro de `try`; ante `TypeError`/`ValueError`: `_send({"type": "error", "error": f"call failed: {type(e).__name__}: {e}"})` y `return 1`.
7. `_send` del result y `return 0`.

Cierre del modulo: `if __name__ == "__main__": raise SystemExit(main())`. Nada mas escribe en `_STDOUT`.

## Requisitos

- R1: el worker captura `_STDOUT = sys.stdout` al importar y ejecuta `sys.stdout = sys.stderr` ANTES de importar el entry, asi todo `print` del plugin (al importarse y al ejecutarse) va a stderr y nunca al pipe de protocolo; el host evalua cada linea del worker con `_decode_line` y descarta sin levantar `JSONDecodeError` toda linea que no sea objeto JSON con `type` str, siguiendo el loop hasta el primer mensaje valido. Orden de evaluacion: redirect -> import del entry -> ejecucion de `method` -> `_send` del resultado por `_STDOUT` -> decodificacion tolerante en el host.
- R2: `invoke` espera mensajes hasta `deadline = time.monotonic() + timeout` y, si el deadline vence con el worker aun vivo, lo mata con `proc.kill()` y devuelve `{"ok": False, "error": f"plugin timeout after {timeout:g}s"}`; en esa ruta y en cualquier otra ruta de error del host la palabra `exited` no aparece en `error`. Ademas `_send` convierte `BrokenPipeError` en `False` y `invoke` traduce ese `False` a `{"ok": False, "error": "plugin worker unavailable"}` en vez de propagar la excepcion.
- R3: si el entry no importa (SyntaxError, ImportError, entry ausente) el worker envia por `_STDOUT` `{"type": "error", "error": "load failed: <TipoDeExcepcion>: <mensaje>"}` y termina con codigo 1; el host mapea ese payload a `{"ok": False, "error": "load failed: <TipoDeExcepcion>: <mensaje>"}` sin agregar, anteponer ni contener la palabra `timeout`.

## Casos borde

- R1: lineas vacias (`"\n"`), texto plano, JSON array, JSON numero u objeto JSON sin `type` str -> `_decode_line` devuelve `None` y el host las ignora sin cortar la espera.
- R1: plugin que imprime en stdout durante el import y devuelve un `value` valido -> el resultado llega intacto y el protocolo queda con una sola linea parseable.
- R2: `timeout` 0 o negativo -> deadline ya vencido -> se mata al worker y se devuelve `{"ok": False, "error": "plugin timeout after 0s"}`.
- R2: worker que muere sin mandar mensaje (EOF sin payload) -> `{"ok": False, "error": "plugin worker died with code <rc>"}`, sin `exited`.
- R2: `_send` con el stdin del worker ya cerrado -> devuelve `False` -> `{"ok": False, "error": "plugin worker unavailable"}`.
- R3: entry ausente -> `FileNotFoundError` -> `{"ok": False, "error": "load failed: FileNotFoundError: ..."}`.
- R3: entry que importa pero no define `method` -> `{"ok": False, "error": "call failed: method 'go' not found"}`, sin `timeout`.
- R3: resultado no serializable a JSON -> `{"ok": False, "error": "call failed: TypeError: ..."}`, sin `timeout`.

## Criterios de exito

- R1: plugin cuyo import hace `print("hola desde el import")` y cuyo `go(args, host)` devuelve `{"ok": True, "n": args["n"] + 1}`; `invoke(man, "go", {"n": 1}, host_services={}, timeout=20)` -> `{"ok": True, "value": {"ok": True, "n": 2}}`, sin `JSONDecodeError`.
- R2: plugin cuyo `go` hace `time.sleep(30)`; `invoke(man, "go", {}, host_services={}, timeout=0.8)` -> `{"ok": False, "error": "plugin timeout after 0.8s"}` con `"timeout" in error.lower()` y `"exited" not in error.lower()`.
- R3: `main.py` con `def go(args, host)` sin `:`; `invoke(man, "go", {}, host_services={}, timeout=20)` -> `{"ok": False, "error": "load failed: SyntaxError: ..."}` con `"timeout" not in error.lower()`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_print_al_importar_no_corrompe_el_protocolo |
| R2 | test_R2_timeout_se_reporta_como_timeout |
| R3 | test_R3_entry_roto_devuelve_error_con_causa |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.