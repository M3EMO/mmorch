"""CLI de validacion del pipeline (tickets 06/07 y reparacion por modulos D2..D15).

Desde el ticket 05 (paso 1, 2026-09-15) el pipeline vive en `mmorch/sdlc.py`; este script solo conserva la tabla
FEATURES (tareas de dogfood sobre mmorch, con su test de aceptacion en accept/<id>/) y llama a mmorch.sdlc.

Uso:
  python driver_py.py --task D15 --wt <dir> [--phase sdlc-D15-r1] [--from-stage 5] [--self-check]
  python driver_py.py --task rate-limiter --wt <dir>      (task del bench: mmorch.sdlc.main)
"""
from __future__ import annotations

import pathlib
import sys
import types

sys.path.insert(0, r"C:\Users\map12\.claude\orchestration")
from mmorch import sdlc  # noqa: E402


def _arg(flag, default):
    return sys.argv[sys.argv.index(flag) + 1] if flag in sys.argv else default


TASK_NAME = _arg("--task", "rate-limiter")
HERE = pathlib.Path(__file__).resolve().parent

# Features de dogfood sobre un repo existente (ticket 06, D*). El test de aceptacion lo escribe Claude
# (accept/<id>/tests/...) y el pipeline no toca tests/. `suite` = gate de regresion total (ticket 13).
FEATURES = {
    "D3": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Agrega un detector de atasco a `build_unit` en mmorch/project_driver.py: si `build_fn` devuelve un "
              "codigo byte-identico al de la vuelta anterior y el gate lo rechaza, `build_unit` devuelve status "
              "'escalate' en ESA vuelta con un `detail` que contenga la palabra 'atascado', sin gastar las vueltas "
              "restantes de max_fix. Codigos distintos siguen agotando max_fix como hoy. Solo se modifica "
              "mmorch/project_driver.py; no se tocan tests ni otros archivos."),
        files=["mmorch/project_driver.py"],
        accept={"tests/test_sdlc_d3_atasco.py": HERE / "accept/D3/tests/test_sdlc_d3_atasco.py"},
        contract=["build_unit", "escalate", "atascado", "mmorch/project_driver.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    "D2": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Agrega un gate `test-compile` a `build_project` en mmorch/project_integrate.py, antes del `external_test` "
              "de integracion. Nuevos parametros keyword: `compile_cmd: str | None = None` y `run_compile=None`. "
              "`run_compile(compile_cmd)` devuelve un `CheckResult` de mmorch.checkers con checker='test_compile'; "
              "el default corre `compile_cmd` con subprocess (shell=True, cwd=repo, timeout) y passed = returncode 0. "
              "En `integrate_fn`: si `compile_cmd` esta declarado y el CheckResult no pasa, devolver "
              "(False, 'test-compile: ' + detail) SIN llamar a `integrate` (fail-closed). Si pasa, sigue como hoy. "
              "Sin `compile_cmd` nada cambia. Solo se modifica mmorch/project_integrate.py; no se tocan tests ni otros archivos."),
        files=["mmorch/project_integrate.py"],
        accept={"tests/test_sdlc_d2_test_compile.py": HERE / "accept/D2/tests/test_sdlc_d2_test_compile.py"},
        contract=["build_project", "compile_cmd", "run_compile", "CheckResult", "test_compile", "integration_failed",
                  "mmorch/project_integrate.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Reparacion de mmorch por modulos (2026-09-14): cada rojo previo de la suite es una aceptacion ya escrita.
    "D4": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("mmorch/synth_store.py ancla `STORE_PATH` a la raiz del repo con `pathlib.Path(__file__).resolve().parent.parent`. "
              "El gate `tests/test_paths.py::test_gate_sin_anclas_de_estado_fuera_de_paths` prohibe ese patron fuera de mmorch/paths.py. "
              "Reemplazalo por `from .paths import repo_root` y `STORE_PATH = repo_root() / \"synth_checkers.json\"` "
              "(el archivo sigue versionado en la raiz del repo; el comportamiento no cambia). "
              "Solo se modifica mmorch/synth_store.py; no se tocan tests ni otros archivos."),
        files=["mmorch/synth_store.py"],
        accept={"tests/test_paths.py": r"C:\Users\map12\.claude\orchestration\tests\test_paths.py"},
        contract=["STORE_PATH", "repo_root", "synth_checkers.json", "mmorch/synth_store.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    "D5": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("mmorch/mcp_server.py fija `mcp._mcp_server.version` con `importlib.metadata.version(\"mmorch\")`. En un git worktree "
              "no hay metadata del paquete (PackageNotFoundError) y el test `tests/test_w6_ronda3.py::test_mcp_server_reporta_version_de_mmorch` "
              "falla. Agrega un fallback determinista: si `importlib.metadata.version` falla, leer `version = \"...\"` de la seccion "
              "[project] del pyproject.toml en `mmorch.paths.repo_root()` con `tomllib`; si tampoco existe, devolver \"0.0.0\". "
              "Expone en mmorch/mcp_server.py una funcion `mmorch_version() -> str` con ese fallback y usala en la asignacion de "
              "`mcp._mcp_server.version`. Dentro de `mmorch_version` llama `importlib.metadata.version(\"mmorch\")` accediendo por el "
              "modulo (`import importlib.metadata`), NO con `from importlib.metadata import version`: los tests lo parchean. "
              "Solo se modifica mmorch/mcp_server.py; no se tocan tests ni otros archivos."),
        files=["mmorch/mcp_server.py"],
        accept={"tests/test_sdlc_d5_version.py": HERE / "accept/D5/tests/test_sdlc_d5_version.py"},
        contract=["mmorch_version", "PackageNotFoundError", "pyproject.toml", "tomllib", "repo_root", "mmorch/mcp_server.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Robustez por modulos (2026-09-14). Modulo 1: plugins (host + worker). Tests de modos de falla escritos por Claude.
    "D6": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Robustez del modulo de plugins: `invoke()` en mmorch/plugins.py y el worker mmorch/plugin_worker.py nunca hacen "
              "crashear al host por lo que haga el plugin. Tres requisitos: (R1) un plugin que imprime en stdout al IMPORTAR "
              "(antes del redirect del worker) no corrompe el protocolo NDJSON: el worker redirige sys.stdout a stderr ANTES de "
              "importar el entry, y el host ignora lineas que no parsean como JSON en vez de levantar JSONDecodeError. "
              "(R2) si el timer de timeout mato al worker, el host devuelve {ok: False, error: 'plugin timeout after <t>s'} "
              "(la palabra 'exited' NO aparece en ese caso). (R3) si el entry no importa (SyntaxError, ImportError, entry ausente), "
              "el worker envia {type: 'error', error: 'load failed: <causa>'} por el protocolo y el host lo devuelve como "
              "{ok: False, error: ...} sin decir 'timeout'. `_send` del host tolera BrokenPipeError devolviendo ok False. "
              "Solo se modifican mmorch/plugins.py y mmorch/plugin_worker.py; no se tocan tests ni otros archivos."),
        files=["mmorch/plugins.py", "mmorch/plugin_worker.py"],
        accept={"tests/test_sdlc_d6_plugins_robustez.py": HERE / "accept/D6/tests/test_sdlc_d6_plugins_robustez.py"},
        contract=["invoke", "timeout", "load failed", "JSONDecodeError", "BrokenPipeError", "mmorch/plugins.py", "mmorch/plugin_worker.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    "D7": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Cablear `try_automerge` (mmorch/automerge.py, hoy sin caller vivo: lo marca tests/test_no_museum.py) en "
              "mmorch/auto_apply.py. En `_finish_merge`, cuando `merge_sha` es None: importar a nivel de modulo "
              "`from .automerge import try_automerge` y llamar `try_automerge(str(runtime.path), state['branch'], "
              "base=state['base_sha'], source='auto_apply')`. Si el resultado trae `merged` True, usar su `merge_sha` y seguir "
              "como hoy (advance a 'merged' y 'observing'). Si `merged` es False y tanto `state.get('zone')` como el `zone` del "
              "resultado son 'yellow', conservar el merge directo actual con `_git(... 'merge' ...)` (carril amarillo acotado). "
              "En cualquier otro rechazo, `_halt(store, reason=f\"automerge {r.get('veredicto')}: {r.get('reason', '')}\", "
              "expected='candidate')`. Con `merge_sha` dado no se llama al semaforo. Solo se modifica mmorch/auto_apply.py; "
              "no se tocan tests ni otros archivos."),
        files=["mmorch/auto_apply.py"],
        accept={"tests/test_sdlc_d7_automerge_cableado.py": HERE / "accept/D7/tests/test_sdlc_d7_automerge_cableado.py",
                "tests/test_no_museum.py": r"C:\Users\map12\.claude\orchestration\tests\test_no_museum.py"},
        contract=["try_automerge", "_finish_merge", "merge_sha", "yellow", "_halt", "mmorch/auto_apply.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Modulo 2: project_driver. Los seams inyectados (plan_fn, commit_fn, integrate_fn) sin proteccion.
    "D8": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Robustez de `run_project_build` en mmorch/project_driver.py: los seams inyectados no pueden tirar abajo el build. "
              "(R1) si `plan_fn` levanta una excepcion, devolver {'status': 'escalate', 'reason': f'planner failed: {type(e).__name__}: {str(e)[:200]}', 'task': task[:80]}. "
              "(R2) si `commit_fn` levanta, la unidad sigue 'built', se agrega al dict de resultado de esa unidad la clave "
              "'commit_error' con f'{type(e).__name__}: {str(e)[:200]}' y el build continua con la siguiente unidad. "
              "(R3) si `integrate_fn` levanta, devolver {'status': 'integration_failed', 'depth': depth, 'external_test': external_test, "
              "'detail': f'integrate_fn {type(e).__name__}: {str(e)[:200]}', 'results': results}. "
              "La recursion (sub-builds) hereda el mismo comportamiento porque pasa por la misma funcion. "
              "Solo se modifica mmorch/project_driver.py; no se tocan tests ni otros archivos; el docstring del modulo se conserva."),
        files=["mmorch/project_driver.py"],
        accept={"tests/test_sdlc_d8_project_driver_robustez.py": HERE / "accept/D8/tests/test_sdlc_d8_project_driver_robustez.py"},
        contract=["run_project_build", "planner failed", "commit_error", "integrate_fn", "integration_failed", "mmorch/project_driver.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Modulo 3: superficie de red (server_fleet + server_pty). Entrada malformada nunca es 500.
    "D9": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Robustez de las rutas en mmorch/server_fleet.py y mmorch/server_pty.py: una entrada malformada devuelve un JSON "
              "{'error': ...} con 4xx, nunca un 500. (R1) en todos los handlers que hacen `await request.json()`, un body que no "
              "parsea (JSONDecodeError/ValueError) o que no es un objeto devuelve 400 {'error': 'body JSON invalido'}. "
              "(R2) en `pty_open` y `pty_resize`, `rows`/`cols` no enteros devuelven 400 {'error': 'rows/cols deben ser enteros'} "
              "(usar int() dentro de try/except ValueError/TypeError). (R3) en `fleet_run`, si `forward` devuelve un dict con "
              "`ok` False y el error dice que el host no esta registrado, responder 404 con ese error; cualquier otro `ok` False "
              "responde 502 con el error. (R4) en `pty_resize` y `pty_input`, la sesion inexistente se verifica ANTES de leer el body "
              "(404 aunque el body este roto). Conserva el orden actual: primero `_token_ok` -> 401. "
              "Solo se modifican mmorch/server_fleet.py y mmorch/server_pty.py; no se tocan tests ni otros archivos; los docstrings se conservan."),
        files=["mmorch/server_fleet.py", "mmorch/server_pty.py"],
        accept={"tests/test_sdlc_d9_server_bordes.py": HERE / "accept/D9/tests/test_sdlc_d9_server_bordes.py"},
        contract=["request.json", "400", "404", "502", "rows", "cols", "forward", "mmorch/server_fleet.py", "mmorch/server_pty.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Modulo 4: workflow_engine. Validacion al cargar, como promete su docstring.
    "D10": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Validacion al cargar en mmorch/workflow_engine.py: `start_workflow(steps, task)` valida los pasos y levanta "
              "ValueError con mensaje claro antes de devolver estado. (R1) `steps` vacio o no lista -> ValueError con la palabra "
              "'steps'. (R2) `gate` fuera de _GATES -> ValueError con 'gate' y el valor. (R3) `loop_back` presente pero negativo o "
              ">= indice del propio step -> ValueError con 'loop_back'. (R4) step sin `role` (str no vacio) -> ValueError con 'role'. "
              "(R5) `submit_workflow` en fase 'produce' con `block_id` None -> ValueError con 'block_id'. (R6) un workflow valido "
              "se comporta exactamente como hoy; el self-check del `__main__` sigue pasando. Implementar la validacion en una "
              "funcion `validate_steps(steps) -> None` llamada por `start_workflow`. Solo se modifica mmorch/workflow_engine.py; "
              "no se tocan tests ni otros archivos; el docstring del modulo se conserva."),
        files=["mmorch/workflow_engine.py"],
        accept={"tests/test_sdlc_d10_workflow_engine_validacion.py": HERE / "accept/D10/tests/test_sdlc_d10_workflow_engine_validacion.py"},
        contract=["validate_steps", "start_workflow", "ValueError", "loop_back", "block_id", "_GATES", "mmorch/workflow_engine.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Hermeticidad: project_loop disparaba `codegraph init/index` reales en repos temporales (594 ERROR de suite).
    "D11": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Hermeticidad de `_codegraph_context` en mmorch/project_loop.py. (R1) Si el repo NO tiene `.codegraph`, no se corre "
              "`init` ni `index` salvo opt-in explicito: la variable MMORCH_CODEGRAPH_AUTOINDEX pasa a default '0' (hoy '1'); "
              "sin opt-in la funcion devuelve '' sin tocar el repo. (R2) Aun con opt-in, si el repo esta bajo "
              "`tempfile.gettempdir()` (comparar paths resueltos) no se indexa: devuelve ''. (R3) Si `.codegraph` ya existe, "
              "`sync` y el resto siguen igual que hoy. Actualizar el docstring de la funcion para reflejar el default nuevo. "
              "Solo se modifica mmorch/project_loop.py; no se tocan tests ni otros archivos; el docstring del modulo se conserva."),
        files=["mmorch/project_loop.py"],
        accept={"tests/test_sdlc_d11_codegraph_hermetico.py": HERE / "accept/D11/tests/test_sdlc_d11_codegraph_hermetico.py"},
        contract=["_codegraph_context", "MMORCH_CODEGRAPH_AUTOINDEX", "gettempdir", "sync", "mmorch/project_loop.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Cableo aprobado: megasource al nightly (mensual, propone, no escribe prices.json).
    "D12": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Cablear `megasource` al nightly en mmorch/nightly.py. Agregar `run_price_check(now=None, propose=None, "
              "state_path=None) -> dict`: estado JSON en `mmorch.paths.home() / 'nightly_prices.json'` con `last_check_ts`; "
              "si la ultima corrida tiene menos de 30 dias devuelve {'ran': False, 'reason': 'reciente'} sin llamar a propose; "
              "si no, llama `propose()` (default: `from mmorch.megasource import propose_price_update`), guarda `last_check_ts = now` "
              "y devuelve {'ran': True, 'n_changed': r.get('n_changed', 0), 'diff': r.get('diff', {})}. Si propose levanta, "
              "devuelve {'ran': True, 'error': f'{type(e).__name__}: {str(e)[:200]}'} y NO escribe el estado. Nunca modifica prices.json. "
              "En `main()`, invocarla dentro de su propio try/except despues del bloque del digest, registrando el resultado con `_log` "
              "(rec {'step': 'price_check', **resultado}). Solo se modifica mmorch/nightly.py; no se tocan tests ni otros archivos; "
              "el docstring del modulo se conserva."),
        files=["mmorch/nightly.py"],
        accept={"tests/test_sdlc_d12_megasource_nightly.py": HERE / "accept/D12/tests/test_sdlc_d12_megasource_nightly.py"},
        contract=["run_price_check", "nightly_prices.json", "last_check_ts", "propose_price_update", "price_check", "mmorch/nightly.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Cableo aprobado: code_embedder a memory (notas de codigo) con pesos verificados por weights.
    "D13": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Cablear `code_embedder` a `memory` con verificacion de pesos. (R1) En mmorch/code_embedder.py, `_load()` resuelve "
              "el .npz y el vocab via `from .weights import resolve, verify, card`: primero `verify('code_embedder')`; si no pasa, "
              "cachea 'no disponible' y `available()` devuelve False y `embed_code()` None; si pasa, usa `resolve('code_embedder')` "
              "para el .npz y `card('code_embedder')['vocab_path']` (relativo a `weights.ROOT`) para el vocab. Mantener el cache de "
              "carga en una variable de modulo `_CACHE` (None = sin cargar). (R2) En mmorch/memory.py, `write_note(..., kind: str = 'text')`: "
              "con kind='code' el embedding sale de `code_embedder.embed_code(text)` y se guarda emb_model='code_embedder', dim=384; "
              "si no esta disponible, embedding NULL como hoy. (R3) `recall(query, scope, ..., kind: str = 'text')`: con kind='code' "
              "embebe la query con code_embedder y en el rerank fino compara SOLO notas con emb_model='code_embedder'; con kind='text' "
              "compara SOLO notas con emb_model distinto de 'code_embedder' (los espacios no se mezclan). (R4) Todo lo demas igual: "
              "kind default 'text' conserva el comportamiento actual byte a byte. Solo se modifican mmorch/code_embedder.py y "
              "mmorch/memory.py y tests/test_capas.py (sacar 'code_embedder' y 'weights' de _NO_ALCANZADOS: el ratchet se achica en el mismo commit); no se tocan otros tests ni archivos; los docstrings de modulo se conservan."),
        files=["mmorch/code_embedder.py", "mmorch/memory.py", "tests/test_capas.py"],  # cableo: el ratchet se achica en el mismo commit
        accept={"tests/test_sdlc_d13_code_embedder_recall.py": HERE / "accept/D13/tests/test_sdlc_d13_code_embedder_recall.py"},
        contract=["code_embedder", "verify", "resolve", "_CACHE", "kind", "emb_model", "write_note", "recall",
                  "mmorch/code_embedder.py", "mmorch/memory.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Cableo aprobado: schedule y effort a providers.call (automatico: off_peak en cada registro; knob effort).
    "D14": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Cablear `schedule` y `effort` en mmorch/providers.py. (R1) `call(...)` acepta keyword `effort: str | None = None`; "
              "si viene, el modelo efectivo es `effort.model_for_effort(effort)` (import `from .effort import model_for_effort`) "
              "y reemplaza a `model_key` desde el principio de la funcion (spec, cliente, costo y registro usan el modelo efectivo). "
              "(R2) Sin `effort`, nada cambia. (R3) Cada `log_event(...)` de `call` (exito, error de API, budget_cap, breaker_open) "
              "incluye en su dict `extra` la clave `off_peak` con el bool de `schedule.is_off_peak()` (import `from . import schedule` "
              "a nivel de modulo y llamar `schedule.is_off_peak()` en cada evento, para que los tests lo parcheen); si `extra` no "
              "existia en ese registro, crearlo como {'off_peak': ...}. Se modifican mmorch/providers.py y tests/test_capas.py (sacar 'effort' y 'schedule' de _NO_ALCANZADOS); no se tocan otros tests ni "
              "otros archivos; el docstring del modulo se conserva."),
        files=["mmorch/providers.py", "tests/test_capas.py"],  # D14: sin esto el coder esquivo el ratchet con importlib
        accept={"tests/test_sdlc_d14_schedule_effort_providers.py": HERE / "accept/D14/tests/test_sdlc_d14_schedule_effort_providers.py"},
        contract=["effort", "model_for_effort", "schedule.is_off_peak", "off_peak", "log_event", "mmorch/providers.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
    # Robustez hallada en vivo (2026-09-14): la API devolvio un objeto sin `choices` y call() murio con TypeError.
    "D15": dict(
        repo=r"C:\Users\map12\.claude\orchestration",
        task=("Robustez de `call` en mmorch/providers.py ante respuestas malformadas de la API. (R1) Si `resp.choices` es None "
              "o vacio, levantar `RuntimeError` con mensaje que contenga 'respuesta sin choices' y el modelo; nunca dejar escapar "
              "un TypeError. (R2) Antes de levantar, registrar el fallo con `log_event(...)` igual que los errores de API, con "
              "error='EmptyResponse', error_msg con el modelo y error_class='empty_response'. (R3) Si `resp.usage` es None con "
              "choices validos, in_tokens y out_tokens valen 0 y la llamada sigue normal. Conservar el resto byte a byte. "
              "Solo se modifica mmorch/providers.py; no se tocan tests ni otros archivos; el docstring del modulo se conserva."),
        files=["mmorch/providers.py"],
        accept={"tests/test_sdlc_d15_providers_respuesta_vacia.py": HERE / "accept/D15/tests/test_sdlc_d15_providers_respuesta_vacia.py"},
        contract=["choices", "respuesta sin choices", "EmptyResponse", "empty_response", "usage", "mmorch/providers.py"],
        suite=["tests", "-q", "-rfE", "-p", "no:cacheprovider", "--basetemp", r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc-wt"],
    ),
}


if __name__ == "__main__":
    for _s in (sys.stdout, sys.stderr):
        _s.reconfigure(encoding="utf-8", errors="replace")
    feat = FEATURES.get(TASK_NAME)
    if not feat:
        sys.exit(sdlc.main(sys.argv[1:]))
    feat = dict(feat, accept={rel: pathlib.Path(src).read_text(encoding="utf-8") for rel, src in feat["accept"].items()})
    task = types.SimpleNamespace(name=TASK_NAME, task=feat["task"], accept_files=feat["accept"])
    sdlc.configure(task, contract=feat["contract"], feat=feat, wt=_arg("--wt", None), phase=_arg("--phase", None),
                   max_fix=_arg("--max-fix", 3))
    if "--self-check" in sys.argv:
        sys.exit(sdlc.self_check())
    sdlc.run(float(_arg("--from-stage", "2")))
    (HERE / f"run-log-{sdlc.PHASE}.json").write_text(sdlc.LOG.read_text(encoding="utf-8"), encoding="utf-8")
