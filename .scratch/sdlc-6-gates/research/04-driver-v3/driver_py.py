"""PROTOTIPO throwaway — driver v3 para tasks Python del bench (ticket 06/07). No es el engine vivo.

Mismo pipeline que driver_v3.py (Java) sobre pytest: spec -> plan -> build -> test -> review Claude -> pr.
Gates: G1 contrato en spec, plan-allowlist, baseline (tests_accept intacto), py_compile, collect-only,
regresion por unidad, topes por avance (stall / novedad / USD), revision de Claude con test que falla.
Escalera: fix loop 3 -> reasoner x2 -> Claude (edit) -> humano.

Uso:
  python driver_py.py --task rate-limiter --wt <dir> [--phase sdlc-rate-limiter-r1]
  python driver_py.py --self-check
"""
from __future__ import annotations

import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
import time

sys.path.insert(0, r"C:\Users\map12\.claude\orchestration")
from mmorch.providers import call  # noqa: E402
from mmorch import bench  # noqa: E402


def _arg(flag, default):
    return sys.argv[sys.argv.index(flag) + 1] if flag in sys.argv else default


TASK_NAME = _arg("--task", "rate-limiter")
# Worktrees FUERA de Desktop/Claude: codegraph indexa ese workspace (207 MB, 19 worktrees = 1.5 GB) y su MCP dejo de conectar.
WT = pathlib.Path(_arg("--wt", rf"C:\Users\map12\AppData\Local\Temp\sdlc-runs\{TASK_NAME}-r1"))
PHASE = _arg("--phase", f"sdlc-{TASK_NAME}-r1")
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
              "mmorch/memory.py; no se tocan tests ni otros archivos; los docstrings de modulo se conservan."),
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
              "existia en ese registro, crearlo como {'off_peak': ...}. Solo se modifica mmorch/providers.py; no se tocan tests ni "
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
FEAT = FEATURES.get(TASK_NAME)
TESTS_PREFIX = "tests/" if FEAT else "tests_accept/"
MAX_FIX = int(_arg("--max-fix", "3"))
REASONER_TRIES = 2
# 2026-09-14 15:18: el tier flash de DeepSeek (chat/reasoner) dejo de responder y v4-pro siguio en 1.4 s.
# Override por env para no cambiar el protocolo (ticket 07) en el codigo: SDLC_WRITER=deepseek-v4-pro.
WRITER = os.environ.get("SDLC_WRITER", "deepseek-reasoner")
CODER = os.environ.get("SDLC_CODER", "deepseek-v4-pro")
PY = sys.executable
LOG = WT / "docs" / "sdlc" / "run-log.json"
METRICS = pathlib.Path(r"C:\Users\map12\.claude\orchestration\logs\metrics.jsonl")
REVIEW_REL = f"{TESTS_PREFIX}test_review_sdlc.py"
PY_RE = r"`?((?:\w+/)*\w+\.py)`?"
TPL = pathlib.Path(__file__).resolve().parent / "templates"  # spec-kit recortado (tickets 02 y 11)

# Tokens que la spec tiene que nombrar verbatim (G1). Uno por task; se agrega al llegar cada feature.
CONTRACTS = {
    "rate-limiter": ["TokenBucket", "MultiLimiter", "allow", "capacity", "refill_per_s", "now", "key",
                     "limiter/core.py", "limiter/multi.py", "limiter/__init__.py"],
    "etl-pipeline": ["parse_lines", "normalize", "to_summary", "run", "maxsplit", "count", "total", "by_name",
                     "etl/extract.py", "etl/transform.py", "etl/load.py", "etl/__init__.py"],
    "lru-ttl-cache": ["LRUCache", "maxsize", "ttl_s", "get", "put", "now", "None", "cache/core.py", "cache/__init__.py"],
}


def _cfg():
    """Topes del ticket 07. docs/sdlc/sdlc.toml pisa los defaults."""
    d = {"usd_max": 3.0, "stall_rounds": 2, "diff_novelty_min": 0.10, "cmd_timeout_s": 600, "suite_timeout_s": 1800}
    p = WT / "docs" / "sdlc" / "sdlc.toml"
    if p.exists():
        import tomllib
        d.update(tomllib.loads(p.read_text(encoding="utf-8")))
    return d


CFG = _cfg()

state: dict = {
    "proto": "driver_py", "task": TASK_NAME,
    "stages": [], "gates": [], "calls": 0, "human_interventions": 0,
    "claude_calls": 0, "gate_rejects": [], "escalated_to_claude": False,
}
SNAP: dict[str, str] = {}
SEEN: set[str] = set()


def llm(model, system, user, timeout=400):
    state["calls"] += 1
    try:
        r = call(model, [{"role": "system", "content": system}, {"role": "user", "content": user}],
                 pattern=PHASE, node=model, phase=PHASE, temperature=0.0, timeout=timeout, max_tokens=32768)
    except RuntimeError as e:  # D13: el reasoner agoto 32k tokens razonando -> un intento con el coder
        if "respuesta vacia" not in str(e) or model == CODER:
            raise
        rec_gate("presupuesto-razonamiento", False, f"{model}: {str(e)[:120]}; reintento con {CODER}")
        return llm(CODER, system, user, timeout)
    usd = ledger_usd() or 0
    if usd > CFG["usd_max"]:
        rec_gate("usd-tope", False, f"US${usd} > {CFG['usd_max']}")
        write_supervision(f"ESCALATE_HUMAN: tope USD {usd} superado")
        _flush()
        sys.exit(f"tope USD {usd}: escala a humano")
    return r.text


def rec_gate(gid, ok, detail):
    state["gates"].append({"id": gid, "ok": ok, "detail": detail, "calls": state["calls"]})
    if not ok:
        state["gate_rejects"].append({"gate": gid, "detail": detail})
    return ok, detail


def stage(name):
    def deco(fn):
        def run():
            t0 = time.time(); c0 = state["calls"]
            ok, note = fn()
            state["stages"].append({"stage": name, "ok": ok, "minutes": round((time.time() - t0) / 60, 2),
                                    "calls": state["calls"] - c0, "note": note})
            _flush()
            print(f"[{name}] ok={ok} {note} ({state['calls'] - c0} llamadas, {(time.time() - t0) / 60:.1f} min)", flush=True)
            if not ok:
                sys.exit(f"etapa {name} fallo: {note}")
        return run
    return deco


def _flush():
    LOG.parent.mkdir(parents=True, exist_ok=True)
    LOG.write_text(json.dumps(state, indent=1, ensure_ascii=False), encoding="utf-8")


def strip_fence(t):
    # D2 r1: project_integrate.py contiene "```" dentro de un string; el recorte perezoso cortaba el archivo.
    # Se toma la valla EXTERIOR (primera apertura, ultimo cierre); sin valla, el texto entero.
    m = re.match(r"\s*```(?:\w+)?\s*\n(.*)\n\s*```\s*$", t, re.S)
    if m:
        return m.group(1).strip()
    m = re.search(r"```(?:\w+)?\s*(.*?)```", t, re.S)
    return (m.group(1) if m else t).strip()


def one_file(code: str, rel: str) -> str:
    """Gate un-archivo (r1 2026-09-11): el coder imito las cabeceras "# path" del prompt y pego 3 archivos en uno."""
    parts = re.split(r"(?m)^# ((?:\w+/)*\w+\.py)\s*$", code)
    if len(parts) < 3:
        return code
    secs = {parts[i]: parts[i + 1].strip() for i in range(1, len(parts), 2)}
    rec_gate("un-archivo", rel in secs, f"{rel}: recorte de {list(secs)}")
    return secs.get(rel, code)


def sh(cmd: list[str], timeout: float | None = None, keep: int = 6000):
    try:
        p = subprocess.run(cmd, cwd=WT, capture_output=True, text=True, encoding="utf-8", errors="replace",
                           timeout=timeout or CFG["cmd_timeout_s"])
    except subprocess.TimeoutExpired:
        return False, f"TIMEOUT {timeout or CFG['cmd_timeout_s']}s: {' '.join(cmd)[:200]}"
    return p.returncode == 0, (p.stdout + p.stderr)[-keep:]


def _suite_args(tag: str) -> list[str]:
    """D8/D10: 594 ERROR de setup = pytest no puede borrar el --basetemp de la corrida ANTERIOR porque un proceso
    hijo de algun test (codegraph.db abierto) sigue vivo y lo bloquea en Windows. Basetemp unico por corrida."""
    stamp = f"{PHASE}-{tag}-{int(time.time())}"
    return [f"{a}-{stamp}" if a.startswith(r"C:\Users\map12\AppData\Local\Temp\pyt-sdlc") else a for a in FEAT["suite"]]


def gate_suite_total() -> tuple[bool, str]:
    """Regresion total (ticket 13): la suite entera del repo no gana fallos nuevos respecto del baseline.
    mmorch tarda ~20 min y trae 4 rojos previos (medido 2026-09-11): se compara por NOMBRE, no por verde."""
    # D3 r1b: el baseline medido en el arbol principal no vale (tests sin commitear, metadata del paquete).
    # Se mide EN el worktree, sobre el commit base, una vez por sha; se cachea en suite-baseline-<sha>.txt.
    sha = subprocess.run(["git", "rev-parse", "--short", state.get("base_sha", "HEAD~1")], cwd=WT, capture_output=True, text=True).stdout.strip()
    base_file = HERE / f"suite-baseline-{sha}.txt"
    if not base_file.exists():
        keep = {f: (WT / f).read_text(encoding="utf-8") for f in state["plan_files"] if (WT / f).exists()}
        subprocess.run(["git", "checkout", "HEAD", "--", *keep], cwd=WT, check=True)  # sin stash: la pila es compartida
        try:
            _, blog = sh([PY, "-m", "pytest", *_suite_args("base")], timeout=CFG["suite_timeout_s"], keep=400000)
        finally:
            for f, c in keep.items():
                _write(f, c)
        _write("docs/sdlc/suite-baseline.log", blog)
        if "TIMEOUT" in blog:  # D15: un baseline vacio por timeout se cacheaba y volvia "nuevos" a los rojos previos
            return rec_gate("suite-total", False, f"baseline: {blog[:200]}")
        base_file.write_text("\n".join(re.findall(r"(?m)^(?:FAILED|ERROR) \S+", blog)) + "\n", encoding="utf-8")
    # D8: la suite del worktree traia 594 ERROR (setup) que el gate no miraba porque solo leia FAILED.
    base = set(re.findall(r"(?m)^(?:FAILED|ERROR) (\S+)", base_file.read_text(encoding="utf-8")))
    ok, log = sh([PY, "-m", "pytest", *_suite_args("final")], timeout=CFG["suite_timeout_s"], keep=400000)
    _write("docs/sdlc/suite.log", log)
    now = set(re.findall(r"(?m)^(?:FAILED|ERROR) (\S+)", log))
    new = sorted(now - base)
    if new and "TIMEOUT" not in log:
        # D2 r1: 3 tests de providers fallaron en la suite y pasaron solos -> flaky bajo carga.
        # Un fallo nuevo cuenta como regresion solo si se repite AISLADO (determinista, sin juicio).
        _, rlog = sh([PY, "-m", "pytest", *new[:50], "-q", "-rfE", "-p", "no:cacheprovider"], keep=400000)
        flaky = sorted(set(new) - set(re.findall(r"(?m)^(?:FAILED|ERROR) (\S+)", rlog)))
        new = sorted(set(new) - set(flaky))
        state["suite_flaky"] = flaky
    state["suite_total"] = test_counts(log) | {"new_failures": new, "baseline_failures": len(base)} if test_counts(log) else {"log": log[-300:]}
    if "TIMEOUT" in log:
        return rec_gate("suite-total", False, log[:200])
    return rec_gate("suite-total", not new, "sin fallos nuevos" if not new else f"fallos nuevos: {new}")


def _accept_paths():
    paths = list(TASK.accept_files)
    if (WT / REVIEW_REL).exists():
        paths.append(REVIEW_REL)
    return paths


def accept():
    return sh([PY, "-m", "pytest", *_accept_paths(), "-q", "-p", "no:cacheprovider"])


def test_counts(log):
    # D3: con -rf el log repite "N failed" dentro de los asserts; se cuenta SOLO la linea de resumen final
    # D4/D5: los "N failed" dentro de los asserts de -rf inflaban el conteo (594): solo la ULTIMA linea de pytest
    lines = [l for l in log.strip().splitlines() if l.strip()]
    tail = [l for l in lines[-3:] if re.search(r"\d+ (?:passed|failed|errors?)\b", l)]
    if tail:
        log = tail[-1]
    f = sum(int(x) for x in re.findall(r"(\d+) (?:failed|error)", log))
    p = sum(int(x) for x in re.findall(r"(\d+) passed", log))
    if not (f or p):
        return None if "no tests ran" not in log else {"run": 0, "failed": 10 ** 6}
    return {"run": f + p, "failed": f}


def _novelty(old: dict, new: dict) -> float:
    """Fraccion de lineas agregadas en esta vuelta que nunca aparecieron en vueltas previas (ticket 07)."""
    before = {l.strip() for c in old.values() for l in c.splitlines()}
    added = {l.strip() for c in new.values() for l in c.splitlines()} - before
    SEEN.update(before)
    nov = len(added - SEEN) / max(1, len(added))
    SEEN.update(added)
    return round(nov, 2)


def sha_file(rel):
    return hashlib.sha256((WT / rel).read_bytes()).hexdigest()


def snapshot_baseline():
    for rel in TASK.accept_files:
        SNAP[rel] = sha_file(rel)


def _need_files() -> list[str]:
    """Archivos que el plan DEBE incluir. Feature de repo: lista explicita (D4: la tarea nombra archivos que NO se
    tocan, el regex los tomaba como obligatorios). Bench: los paths que nombra el enunciado."""
    if FEAT:
        return list(FEAT["files"])
    return list(dict.fromkeys(re.findall(r"[\w/]+\.py", TASK.task)))


def gate_plan_allowlist(plan_md: str, files: list[str]) -> tuple[bool, str]:
    m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", plan_md, re.S | re.I)
    block = m.group(1) if m else plan_md
    if re.search(r"(?im)^\s*[-*]+\s*`?" + re.escape(TESTS_PREFIX), block):
        return rec_gate("plan-allowlist", False, f"plan lista {TESTS_PREFIX} como archivo a escribir")
    need = [f for f in _need_files() if f not in files]
    if need:
        return rec_gate("plan-allowlist", False, f"plan omite {need}")
    return rec_gate("plan-allowlist", True, f"{files}")


def _test_names() -> set[str]:
    return {n for rel in TASK.accept_files for n in re.findall(r"(?m)^def (test_\w+)", (WT / rel).read_text(encoding="utf-8"))}


def gate_traza_spec(spec_md: str, tests: set[str]) -> tuple[bool, str]:
    """Trazabilidad lado spec (ticket 02): IDs R<n> y tabla que cita tests existentes por nombre."""
    ids = set(re.findall(r"\bR\d+\b", spec_md))
    if not ids:
        return rec_gate("trazabilidad-spec", False, "sin IDs R<n>")
    m = re.search(r"## Trazabilidad\b(.*?)(?:\n## |\Z)", spec_md, re.S)
    rows = re.findall(r"(?m)^\|\s*(R\d+)\s*\|([^|]*)\|", m.group(1) if m else "")
    cov = {rid: set(re.findall(r"test_\w+", cell)) for rid, cell in rows}
    sin_test = sorted(i for i in ids if not cov.get(i))
    inexist = sorted(t for ts in cov.values() for t in ts if t not in tests)
    if sin_test or inexist:
        return rec_gate("trazabilidad-spec", False, f"sin test: {sin_test}; tests inexistentes: {inexist}")
    return rec_gate("trazabilidad-spec", True, f"{len(ids)} IDs cubiertos")


def gate_traza_plan(spec_md: str, plan_block: str, files: list[str]) -> tuple[bool, str]:
    """Trazabilidad lado plan: cada R<n> de la spec aparece en el plan; cada archivo cita >= 1 R<n>."""
    ids = set(re.findall(r"\bR\d+\b", spec_md))
    huerfanos = sorted(i for i in ids if not re.search(rf"\b{i}\b", plan_block))
    items = re.findall(r"(?m)^\s*[-*]\s*(.*)$", plan_block)
    sin_id = [f for f in files if not any(f in it and re.search(r"\bR\d+\b", it) for it in items)]
    if huerfanos or sin_id:
        return rec_gate("trazabilidad-plan", False, f"IDs sin unidad: {huerfanos}; archivos sin ID: {sin_id}")
    return rec_gate("trazabilidad-plan", True, "ok")


def gate_clarificaciones(spec_md: str) -> tuple[bool, str]:
    """Salida de la revision de spec (ticket 11, de clarify.md): seccion con 0..5 preguntas respondidas."""
    m = re.search(r"## Clarificaciones\b(.*?)(?:\n## |\Z)", spec_md, re.S)
    if not m:
        return rec_gate("spec-review", False, "falta ## Clarificaciones")
    if "sin preguntas" in m.group(1):
        return rec_gate("spec-review", True, "0 preguntas")
    qs = re.findall(r"(?m)^- P\d+: .+\| R: \S.*\| afecta: R\d+", m.group(1))
    if not 1 <= len(qs) <= 5:
        return rec_gate("spec-review", False, f"{len(qs)} preguntas validas (esperado 1..5)")
    return rec_gate("spec-review", True, f"{len(qs)} preguntas respondidas")


def _tree() -> dict[str, str]:
    """path -> sha de todo archivo del worktree (tracked + nuevos), para medir que toco una pasada de Claude."""
    out = subprocess.run(["git", "ls-files", "-co", "--exclude-standard"], cwd=WT, capture_output=True, text=True).stdout
    return {p: sha_file(p) for p in out.splitlines() if p and (WT / p).is_file()}


def gate_alcance(gid: str, before: dict[str, str], allowed: tuple[str, ...]) -> tuple[bool, str]:
    """Una pasada de Claude solo puede tocar `allowed` (r4b: la revision de spec implemento core.py y multi.py).
    Lo demas vuelve al estado previo: tracked/staged con checkout, nuevo con clean. Determinista, USD 0."""
    after = _tree()
    changed = [p for p in set(before) | set(after) if before.get(p) != after.get(p)]
    fuera = sorted(p for p in changed if not p.startswith(allowed) and not p.startswith("docs/sdlc/supervision"))
    if fuera:
        subprocess.run(["git", "checkout", "--", *[p for p in fuera if p in before]], cwd=WT, capture_output=True)
        subprocess.run(["git", "clean", "-fq", "--", *[p for p in fuera if p not in before]], cwd=WT, capture_output=True)
    return rec_gate(gid, not fuera, "ok" if not fuera else f"toco fuera de alcance, revertido: {fuera}")


def gate_baseline() -> tuple[bool, str]:
    bad = [rel for rel, h in SNAP.items() if not (WT / rel).exists() or sha_file(rel) != h]
    return rec_gate("baseline-intacto", not bad, "ok" if not bad else f"toco {bad}")


def gate_compile(files) -> tuple[bool, str]:
    ok, log = sh([PY, "-m", "py_compile", *files])
    return rec_gate("G3-compile", ok, "compila" if ok else log[-800:])


def gate_test_compile() -> tuple[bool, str]:
    """Equivalente a mvn test-compile: los tests importan y se recolectan."""
    ok, log = sh([PY, "-m", "pytest", *_accept_paths(), "--collect-only", "-q", "-p", "no:cacheprovider"])
    return rec_gate("test-compile", ok, "ok" if ok else log[-800:])


def ledger_usd():
    if not METRICS.exists():
        return None
    t0 = state.get("t0_epoch") or 0
    usd = 0.0
    state["usd_by_family"] = {}
    for line in METRICS.read_text(encoding="utf-8").splitlines()[-8000:]:
        try:
            r = json.loads(line)
        except json.JSONDecodeError:
            continue
        if r.get("phase") == PHASE and float(r.get("ts") or 0) >= t0:
            c = float(r.get("cost_usd") or 0)
            usd += c
            fam = r.get("family") or "?"
            state["usd_by_family"][fam] = round(state["usd_by_family"].get(fam, 0) + c, 4)
    return round(usd, 4)


def _joined(written):
    return "\n\n".join(f"# {k}\n{v}" for k, v in written.items())


def _all_code():
    return {f: (WT / f).read_text(encoding="utf-8") for f in state["plan_files"] if (WT / f).exists()}


def _write(rel, code):
    p = WT / rel
    p.parent.mkdir(parents=True, exist_ok=True)
    if rel.endswith(".py") and code and not code.endswith("\n"):
        code += "\n"  # D4: strip_fence borraba la linea final y el diff mostraba "No newline at end of file"
    p.write_text(code, encoding="utf-8")


def _docstrings(files) -> dict:
    import ast
    out = {}
    for f in files:
        try:
            out[f] = ast.get_docstring(ast.parse((WT / f).read_text(encoding="utf-8"))) if (WT / f).exists() else None
        except SyntaxError:
            out[f] = None
    return out


def gate_clones(written: dict) -> tuple[bool, str]:
    """D6: el coder escribio el contenido de plugins.py dentro de plugin_worker.py. Dos archivos del plan
    con > 80% de lineas identicas es un clon, no una feature. Determinista, USD 0."""
    files = list(written)
    for i, a in enumerate(files):
        la = {l.strip() for l in written[a].splitlines() if l.strip()}
        for b in files[i + 1:]:
            lb = {l.strip() for l in written[b].splitlines() if l.strip()}
            if la and lb and len(la & lb) / min(len(la), len(lb)) > 0.8:
                return rec_gate("sin-clones", False, f"{a} y {b} comparten > 80% de lineas")
    return rec_gate("sin-clones", True, "ok")


_orig_files: dict[str, str] = {}


def gate_cambio_minimo(before: dict, written: dict) -> tuple[bool, str]:
    """D11: el coder reescribio project_loop.py (-216 lineas) para agregar una guarda. Sobre un archivo que ya existia,
    borrar mas del 30% de sus lineas (minimo 40) sin que la tarea diga 'reescrib' es un cambio no pedido. USD 0."""
    if "reescrib" in TASK.task.lower():
        return rec_gate("cambio-minimo", True, "la tarea pide reescribir")
    malos = []
    for f, orig in _orig_files.items():
        if f not in written or not orig.strip():
            continue
        old_lines = [l for l in orig.splitlines() if l.strip()]
        new_set = {l.strip() for l in written[f].splitlines()}
        borradas = sum(1 for l in old_lines if l.strip() not in new_set)
        if borradas > max(40, int(0.3 * len(old_lines))):
            malos.append(f"{f}: {borradas} de {len(old_lines)} lineas originales desaparecieron")
    return rec_gate("cambio-minimo", not malos, "ok" if not malos else "; ".join(malos))


def gate_docstring(before: dict) -> tuple[bool, str]:
    """D4: el coder borro el docstring del modulo (22 lineas) y ningun test lo ve. Determinista, USD 0:
    un docstring de modulo que existia antes tiene que seguir existiendo, salvo que la tarea hable de docstrings."""
    if "docstring" in TASK.task.lower():
        return rec_gate("docstring-intacto", True, "la tarea lo cubre")
    after = _docstrings(before)
    perdidos = [f for f, d in before.items() if d and not after.get(f)]
    return rec_gate("docstring-intacto", not perdidos, "ok" if not perdidos else f"docstring de modulo borrado en {perdidos}")


def _tests_text():
    t = "\n\n".join(f"# {k}\n{(WT / k).read_text(encoding='utf-8')}" for k in TASK.accept_files)
    if (WT / REVIEW_REL).exists():
        t += "\n\n# test_review_sdlc.py (revision de Claude, no se toca)\n" + (WT / REVIEW_REL).read_text(encoding="utf-8")
    return t


def write_supervision(note: str):
    p = WT / "docs" / "sdlc" / "supervision.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    prev = p.read_text(encoding="utf-8") if p.exists() else "# supervision.md — driver_py\n\n"
    p.write_text(prev + f"\n## candidato {time.strftime('%H:%M:%S')}\n{note}\n", encoding="utf-8")


def _revert(backup):
    for f, old in backup.items():
        _write(f, old)


def reasoner_rounds(log) -> tuple[bool, str]:
    """Nivel 2: dos intentos con deepseek-reasoner. Una instruccion por archivo."""
    for i in range(REASONER_TRIES):
        written = _all_code()
        pick = llm(WRITER, 'Sos un diagnosticador. Respondes SOLO JSON: {"files": [paths], "instructions": {path: "una instruccion"}}. Una instruccion por archivo.',
                   f"Los tests de aceptacion fallan. Diagnostica y indica QUE cambiar.\n\nSALIDA:\n{log}\n\n"
                   f"TAREA:\n{TASK.task}\n\nTESTS:\n{_tests_text()}\n\nARCHIVOS:\n{_joined(written)}")
        write_supervision(f"reasoner {i+1}: {pick[:2000]}")
        try:
            data = json.loads(strip_fence(pick))
            files = [f for f in data.get("files", []) if f in written]
            instr = data.get("instructions") or {}
        except Exception:
            files, instr = list(written)[:1], {}
        backup = {f: written[f] for f in files}
        for f in files:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"Aplica ESTA instruccion a {f}. No toques los tests.\n"
                      f"INSTRUCCION: {instr.get(f, 'corregi el fallo del test')}\n\n"
                      f"SALIDA:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
            code = one_file(strip_fence(out), f)
            if code:
                _write(f, code)
        if not gate_baseline()[0] or not gate_compile(files)[0] or not gate_test_compile()[0]:
            _revert(backup)
            continue
        ok, log = accept()
        state["reasoner_rounds"] = state.get("reasoner_rounds", []) + [{"i": i + 1, "files": files, "ok": ok}]
        if ok:
            state["suite_total"] = test_counts(log)
            return True, f"reasoner {i+1} verde"
    return False, f"reasoner x{REASONER_TRIES} rojo"


def claude_fix(log) -> tuple[bool, str]:
    """Nivel 3: Claude en el worktree (modo edit). Deja correccion; el test de aceptacion decide."""
    from mmorch.claude_exec import run_claude
    os.environ.pop("CLAUDECODE", None)  # ponytail: el CLI no anida dentro de una sesion Claude Code
    state["escalated_to_claude"] = True
    state["claude_calls"] += 1
    before = _tree()
    r = run_claude(f"Los tests de aceptacion fallan. Arregla el codigo en {state['plan_files']} hasta que "
                   f"`pytest {' '.join(_accept_paths())} -q` pase. NO toques tests ni docs. Al final responde en una linea que cambiaste.\n\n"
                   f"TAREA:\n{TASK.task}\n\nSALIDA:\n{log}", cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
    write_supervision(f"nivel 3 Claude (rc={r.get('returncode')}): {(r.get('result') or '')[:2000]}")
    gate_alcance("claude-fix-alcance", before, tuple(state["plan_files"]))
    if not gate_baseline()[0]:
        subprocess.run(["git", "checkout", "--", *SNAP], cwd=WT)
    ok, log = accept()
    if ok:
        state["suite_total"] = test_counts(log)
    return ok, "Claude verde" if ok else "Claude rojo: escala a humano"


def self_check() -> int:
    """Cero API: allowlist, novedad, conteo pytest."""
    fails = []
    need = _need_files()
    good = "## Archivos\n" + "".join(f"- `{f}`\n" for f in need) + "\n## Prueba\npytest\n"
    if not gate_plan_allowlist(good, need)[0]:
        fails.append("allowlist-good")
    if gate_plan_allowlist(f"## Archivos\n- {TESTS_PREFIX}test_x.py\n- {need[0]}", need)[0]:
        fails.append("allowlist-tests")
    if gate_plan_allowlist(good, need[:-1])[0]:
        fails.append("allowlist-omite")
    SEEN.clear()
    if _novelty({"A": "x\ny"}, {"A": "x\nz"}) != 1.0 or _novelty({"A": "x\nz"}, {"A": "x\ny"}) != 0.0:
        fails.append("novelty")
    if test_counts("2 failed, 1 passed in 0.1s") != {"run": 3, "failed": 2} or test_counts("3 passed in 0.1s") != {"run": 3, "failed": 0}:
        fails.append("counts")
    if one_file("# limiter/__init__.py\nfrom x import y\n\n# limiter/core.py\nclass TokenBucket: pass\n", "limiter/core.py") != "class TokenBucket: pass" \
            or one_file("class A: pass", "a.py") != "class A: pass":
        fails.append("un-archivo")
    tests = {"test_a", "test_b"}
    sp = "## Requisitos\n- R1: x\n- R2: y\n\n## Trazabilidad\n| ID | tests |\n|---|---|\n| R1 | test_a |\n| R2 | test_a, test_b |\n"
    if not gate_traza_spec(sp, tests)[0]:
        fails.append("traza-spec-good")
    if gate_traza_spec(sp.replace("| R2 | test_a, test_b |\n", ""), tests)[0] or gate_traza_spec(sp.replace("test_b", "test_zz"), tests)[0]:
        fails.append("traza-spec-bad")
    pb = "\n- `limiter/core.py` [R1] [P]\n- `limiter/multi.py` [R2]\n"
    if not gate_traza_plan(sp, pb, ["limiter/core.py", "limiter/multi.py"])[0]:
        fails.append("traza-plan-good")
    if gate_traza_plan(sp, pb.replace("[R2]", ""), ["limiter/core.py", "limiter/multi.py"])[0]:
        fails.append("traza-plan-bad")
    cl = "## Clarificaciones\n\n- P1: cap? | R: SUPUESTO: float | afecta: R1\n"
    if not gate_clarificaciones(cl)[0] or not gate_clarificaciones("## Clarificaciones\n- sin preguntas: la spec cubre el barrido.\n")[0] \
            or gate_clarificaciones("## Casos\n")[0] or gate_clarificaciones("## Clarificaciones\n- P1: cap? | R:  | afecta: R1\n")[0]:
        fails.append("clarificaciones")
    print("self-check", "FAIL" if fails else "PASS", fails)
    return 1 if fails else 0


@stage("2-spec")
def spec():
    tpl = (TPL / "spec-template.md").read_text(encoding="utf-8")
    tests = _test_names()
    ask = (f"Escribi spec.md siguiendo EXACTAMENTE esta plantilla (mismas secciones, IDs R<n> unicos, tabla de trazabilidad "
           f"que cita tests por su nombre exacto de entre {sorted(tests)}). Python 3.12, sin dependencias.\n\n"
           f"PLANTILLA:\n{tpl}\n\nTAREA:\n{TASK.task}\n\nTESTS DE ACEPTACION (no se modifican):\n{_tests_text()}\n\n"
           "Omiti las lineas de instruccion de la plantilla. Sin marcadores pendientes. Solo markdown.")
    out = llm(WRITER, "Sos un ingeniero de software. Escribis specs precisas en markdown, sin relleno.", ask)
    _write("docs/sdlc/spec.md", out)

    def _falla():
        missing = [s for s in CONTRACT if s not in out]
        tok, tnote = gate_traza_spec(out, tests)
        return (f"faltan tokens {missing}; " if missing else "") + ("TBD; " if "TBD" in out else "") + ("" if tok else tnote)

    why = _falla()
    if why:
        out = llm(WRITER, "Sos un ingeniero. Reescribís la spec completa. Los tokens y la tabla pedidos deben aparecer VERBATIM.",
                  f"Esta spec fallo el gate: {why}. Corregilo sin borrar el resto. Plantilla:\n{tpl}\n\nSPEC ACTUAL:\n{out}")
        _write("docs/sdlc/spec.md", out)
        why = _falla()
    if why:
        rec_gate("G1-spec-contrato", False, why)
        return False, f"G1 rechaza: {why}"
    rec_gate("G1-spec-contrato", True, "contrato cubierto, sin TBD, IDs trazados")
    return True, "G1 + trazabilidad-spec ok"


@stage("2b-spec-review")
def spec_review():
    """Claude revisa la spec con spec-review.md (de clarify.md): <= 5 preguntas, respondidas en la spec."""
    from mmorch.claude_exec import run_claude
    os.environ.pop("CLAUDECODE", None)
    guide = (TPL / "spec-review.md").read_text(encoding="utf-8")
    state["claude_calls"] += 1
    before = _tree()
    r = run_claude(f"{guide}\n\nTAREA:\n{TASK.task}\n\nEdita SOLO docs/sdlc/spec.md segun estas reglas. NO escribas codigo ni crees "
                   "ni toques ningun otro archivo: la implementacion la hace otra etapa. Responde en una linea cuantas preguntas hiciste.",
                   cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
    write_supervision(f"revision de spec (Claude, rc={r.get('returncode')}): {(r.get('result') or '')[:1500]}")
    gate_alcance("spec-review-alcance", before, ("docs/sdlc/spec.md",))
    if not gate_baseline()[0]:
        subprocess.run(["git", "checkout", "--", *SNAP], cwd=WT)
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    ok, note = gate_clarificaciones(spec_md)
    if ok:
        ok, note = gate_traza_spec(spec_md, _test_names())  # la revision no puede romper la trazabilidad
    return ok, note


@stage("3-plan")
def plan():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    ask = (f"Escribi plan.md. Formato OBLIGATORIO: seccion '## Archivos' con un item por archivo a escribir, asi:\n"
           f"- `path.py` [R1, R3] [P]\n"
           f"Cada item cita los IDs R<n> de la spec que cubre. Todo R<n> de la spec aparece en algun item. "
           f"[P] marca archivos que NO importan a otros del plan (paralelizables); los demas van en orden de dependencia. "
           f"'## Prueba' = `python -m pytest {' '.join(_accept_paths())} -q`.\n\nSPEC:\n{spec_md}")
    out = llm(WRITER, "Sos un tech lead. Escribis planes ejecutables. NO regeneres los tests.", ask)
    for intento in range(2):  # D2 r1: una reescritura con el motivo del gate, como en spec
        _write("docs/sdlc/plan.md", out)
        m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", out, re.S | re.I)
        block = m.group(1) if m else out
        files = _plan_files(block)
        state["plan_files"] = files
        state["plan_parallel"] = [f for f in files if re.search(rf"{re.escape(f)}`?[^\n]*\[P\]", block)]  # ticket 05 lo ejecuta en paralelo
        ok, note = gate_plan_allowlist(out, files)
        if ok:
            ok, note = gate_traza_plan(spec_md, block, files)
        if ok or intento:
            return ok, note
        out = llm(WRITER, "Sos un tech lead. Reescribis el plan completo con el formato obligatorio.",
                  f"Este plan fallo el gate: {note}. Corregilo.\n\nPLAN ACTUAL:\n{out}\n\nSPEC:\n{spec_md}")
    return ok, note


def _plan_files(block: str) -> list[str]:
    """Archivos del plan = SOLO los items de la lista (D2 r1: una mencion en prosa 'sin tocar X.py' se colaba)."""
    items = re.findall(r"(?m)^\s*[-*]\s*`?((?:\w+/)*\w+\.py)`?", block)
    return [f for f in dict.fromkeys(items) if not f.startswith(TESTS_PREFIX)]


@stage("4-build")
def build():
    spec_md = (WT / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
    written = {}
    docs_before = _docstrings(state["plan_files"])  # gate docstring-intacto (D4)
    _orig_files.clear()
    _orig_files.update({f: (WT / f).read_text(encoding="utf-8") for f in state["plan_files"] if (WT / f).exists()})
    for f in state["plan_files"]:
        cur = (WT / f).read_text(encoding="utf-8") if (WT / f).exists() else ""
        cur_ctx = f"ARCHIVO ACTUAL {f} (devolvelo COMPLETO con el cambio minimo):\n{cur}\n\n" if cur.strip() else ""
        out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el codigo del archivo pedido, sin explicacion ni markdown.",
                  f"Escribi {f}. Tiene que importar con los archivos ya escritos y pasar los tests de aceptacion.\n\n{cur_ctx}"
                  f"PLAN:\n{plan_md}\n\nSPEC:\n{spec_md}\n\nTAREA:\n{TASK.task}\n\nTESTS:\n{_tests_text()}\n\n"
                  f"ARCHIVOS YA ESCRITOS:\n{_joined(written) or '(ninguno)'}")
        code = one_file(strip_fence(out), f)
        _write(f, code)
        written[f] = code
        bok, bnote = gate_baseline()
        if not bok:
            return False, bnote
    def _compiles():
        ok, log = gate_compile(state["plan_files"])
        return (ok, log) if not ok else gate_test_compile()

    ok, log = _compiles()
    for _ in range(3):  # r1 2026-09-11: test-compile tambien entra al fix loop, antes cortaba sin vuelta
        if ok:
            break
        for f in state["plan_files"]:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el codigo corregido del archivo pedido, un solo archivo.",
                      f"py_compile o la importacion de los tests fallo. Corregi {f}.\n\nERROR:\n{log}\n\nARCHIVOS:\n{_joined(written)}")
            code = one_file(strip_fence(out), f)
            if code and code != written[f]:
                _write(f, code); written[f] = code
        ok, log = _compiles()
        if not gate_baseline()[0]:
            return False, "baseline roto en compile-fix"
    if not ok:
        return False, f"G3 rechaza tras 3 vueltas: {log[-300:]}"
    cok, cnote = gate_clones(written)
    if not cok:
        return False, cnote
    mok, mnote = gate_cambio_minimo(docs_before, written)
    if not mok:  # D11: el coder borro 216 lineas de project_loop.py para agregar 10; una vuelta de re-pedido
        for f in [f for f in written if f in mnote]:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"Tu version de {f} borro codigo existente que la tarea NO pide tocar ({mnote}). "
                      f"Parti del ARCHIVO ORIGINAL y aplica SOLO el cambio pedido, conservando todo lo demas.\n\n"
                      f"TAREA:\n{TASK.task}\n\nARCHIVO ORIGINAL:\n{_orig_files.get(f, '')}\n\nTU VERSION:\n{written[f]}")
            code = one_file(strip_fence(out), f)
            if code:
                _write(f, code); written[f] = code
        mok, mnote = gate_cambio_minimo(docs_before, written)
        if not mok or not _compiles()[0]:
            return False, f"cambio-minimo: {mnote}"
    dok, dnote = gate_docstring(docs_before)
    if not dok:  # una vuelta del coder para restaurarlo; si insiste, la etapa falla
        for f in [f for f, d in docs_before.items() if d and not _docstrings([f]).get(f)]:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"Restaura el docstring de modulo original de {f} al inicio del archivo, sin cambiar nada mas.\n\n"
                      f"DOCSTRING ORIGINAL:\n\"\"\"{docs_before[f]}\"\"\"\n\nARCHIVO ACTUAL:\n{(WT / f).read_text(encoding='utf-8')}")
            code = one_file(strip_fence(out), f)
            if code:
                _write(f, code); written[f] = code
        dok, dnote = gate_docstring(docs_before)
        if not dok or not _compiles()[0]:
            return False, f"docstring: {dnote}"
    return True, "G3 + test-compile + docstring ok"


@stage("5-test")
def test():
    ok, log = accept()
    state["test_rounds"] = state.get("test_rounds", []) + [{"round": 0, "tests": test_counts(log)}]
    state["stall"] = 0
    vueltas = 0
    while not ok and vueltas < MAX_FIX:
        vueltas += 1
        written = _all_code()
        pick = llm(CODER, 'Sos un programador Python senior. Respondes SOLO un JSON: {"files": [paths], "why": str}.',
                   f"Los tests fallan. Que archivos cambiar (los MENOS posibles)?\n\nSALIDA:\n{log}\n\n"
                   f"TAREA:\n{TASK.task}\n\nTESTS:\n{_tests_text()}\n\nARCHIVOS:\n{_joined(written)}")
        try:
            files = [f for f in json.loads(strip_fence(pick)).get("files", []) if f in written]
        except Exception:
            files = state["plan_files"][:1]
        if not files:
            files = state["plan_files"][:1]
        backup = {f: written[f] for f in files}
        for f in files:
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero. No toques los tests.",
                      f"Arregla {f}. Los tests NO se modifican.\n\nSALIDA:\n{log}\n\nTAREA:\n{TASK.task}\n\n"
                      f"TESTS:\n{_tests_text()}\n\nARCHIVOS:\n{_joined(written)}")
            code = one_file(strip_fence(out), f)
            if code and code != written[f]:
                _write(f, code)
        if not gate_baseline()[0] or not gate_compile(files)[0] or not gate_test_compile()[0]:
            _revert(backup)
            state["test_rounds"].append({"round": vueltas, "files": files, "reverted": "gate"})
            continue
        prev = state["test_rounds"][-1].get("tests") or {"failed": 10 ** 6}
        ok, log2 = accept()
        now = test_counts(log2) or {"failed": 10 ** 6, "run": 0}
        nov = _novelty(backup, {f: (WT / f).read_text(encoding="utf-8") for f in files})
        state["stall"] = state["stall"] + 1 if now["failed"] >= prev["failed"] else 0
        # Gate de REGRESION por unidad (ticket 13): si la vuelta hace fallar mas tests, se revierte.
        if now["failed"] > prev["failed"]:
            _revert(backup)
            rec_gate("regresion-unidad", False, f"{files} rompio {now['failed'] - prev['failed']} test(s) que pasaban")
            state["test_rounds"].append({"round": vueltas, "files": files, "reverted": "regresion", "tests": now, "novelty": nov})
        else:
            rec_gate("regresion-unidad", True, f"{files}: {prev['failed']} -> {now['failed']} fallos")
            ok, log = ok, log2
            state["test_rounds"].append({"round": vueltas, "files": files, "tests": now, "novelty": nov})
        # Topes por AVANCE (ticket 07): sin bajar fallos N vueltas, o diff sin novedad -> escala un nivel.
        if not ok and (state["stall"] >= CFG["stall_rounds"] or nov < CFG["diff_novelty_min"]):
            rec_gate("avance", False, f"stall={state['stall']} novedad={nov}: escala a reasoner")
            break
    state["test_fix_rounds"] = state.get("test_fix_rounds", 0) + vueltas
    ladder_note = ""
    if not ok:  # escalera: reasoner x2 -> Claude. D11: antes estos caminos saltaban lint y suite total.
        ok, ladder_note = reasoner_rounds(log)
        if not ok:
            ok, ladder_note = claude_fix(log)
        if not ok:
            rec_gate("G4-aceptacion", False, ladder_note)
            write_supervision("ESCALATE_HUMAN: fix 3 + reasoner 2 + Claude agotados.")
            return False, ladder_note
    if ok and FEAT:
        lok, lnote = gate_lint()
        if not lok:  # D2 r1: el pre-commit del repo rechazo 2 errores de mypy; ahora es gate con escalera
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"ruff/mypy reportan errores nuevos. Corregi {state['plan_files'][0]} sin cambiar comportamiento.\n\n"
                      f"ERRORES:\n{lnote}\n\nARCHIVOS:\n{_joined(_all_code())}")
            code = one_file(strip_fence(out), state["plan_files"][0])
            if code:
                _write(state["plan_files"][0], code)
            lok, lnote = gate_lint()
            aok, _ = accept()
            if not (lok and aok):
                cok, cnote = claude_fix(f"ruff/mypy nuevos deben ser 0 y la aceptacion verde:\n{lnote}")
                lok, lnote = gate_lint()
                if not (cok and lok):
                    write_supervision(f"ESCALATE_HUMAN: lint nuevo tras coder y Claude\n{lnote}")
                    return False, f"lint: {lnote[:200]}"
        sok, snote = gate_suite_total()
        if not sok and state.get("suite_total", {}).get("new_failures"):
            # D11: la regresion de suite iba directo a humano; ahora sigue la escalera (coder -> Claude -> humano)
            new = state["suite_total"]["new_failures"][:20]
            _, flog = sh([PY, "-m", "pytest", *new, "-q", "-x", "-p", "no:cacheprovider"])
            out = llm(CODER, "Sos un programador Python senior. Devolves SOLO el archivo entero.",
                      f"Tu cambio en {state['plan_files'][0]} rompio tests existentes que antes pasaban. Corregilo conservando "
                      f"la feature nueva y el comportamiento previo.\n\nTESTS ROTOS:\n{flog[-4000:]}\n\nARCHIVOS:\n{_joined(_all_code())}")
            code = one_file(strip_fence(out), state["plan_files"][0])
            if code:
                _write(state["plan_files"][0], code)
            aok, _ = accept()
            sok, snote = gate_suite_total() if aok else (False, "aceptacion rota tras el fix de regresion")
            if not sok:
                cok, _ = claude_fix(f"Tests existentes rotos por el cambio (deben volver a pasar sin perder la feature):\n{flog[-4000:]}")
                sok, snote = gate_suite_total() if cok else (False, "Claude no dejo verde la aceptacion")
        if not sok:
            write_supervision(f"ESCALATE_HUMAN: aceptacion verde pero suite total con regresion\n{snote}")
            return False, f"suite total: {snote[:200]}"
        rec_gate("G4-aceptacion", True, f"verde en {vueltas} vueltas {ladder_note} + lint + suite total")
        return True, f"G4 + suite ok ({vueltas} vueltas {ladder_note})".strip()
    state["suite_total"] = test_counts(log)
    rec_gate("G4-aceptacion", True, f"verde en {vueltas} vueltas {ladder_note}".strip())
    return True, f"G4 ok ({vueltas} vueltas {ladder_note})".strip()


@stage("5b-review")
def review():
    """Claude revisa el diff. Bloquea SOLO si deja un test_review.py que falla (ticket 07)."""
    from mmorch.claude_exec import run_claude
    os.environ.pop("CLAUDECODE", None)
    subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
    diff = subprocess.run(["git", "diff", "--cached", "--", *state["plan_files"]], cwd=WT,
                          capture_output=True, text=True, encoding="utf-8").stdout
    state["claude_calls"] += 1
    before = _tree()
    r = run_claude(
        "Revisa este diff contra docs/sdlc/spec.md y la TAREA. "
        f"Si encontras un defecto REAL, escribi UN test pytest que falle en {REVIEW_REL} "
        "(una funcion test_*) y responde 'BLOCK: <defecto>'. No toques ningun otro archivo. "
        "Si no hay defecto demostrable con test, no escribas nada y responde 'OK' o 'NOTE: <observacion>'.\n\n"
        f"TAREA:\n{TASK.task}\n\nDIFF:\n{diff[:60000]}", cwd=str(WT), mode="edit", timeout=CFG["cmd_timeout_s"])
    verdict = (r.get("result") or "").strip()
    write_supervision(f"revision del diff (Claude, rc={r.get('returncode')}): {verdict[:2000]}")
    gate_alcance("diff-review-alcance", before, (REVIEW_REL,))
    if not gate_baseline()[0]:
        subprocess.run(["git", "checkout", "--", *SNAP], cwd=WT)
    if not (WT / REVIEW_REL).exists():
        return rec_gate("claude-diff-review", True, f"sin test nuevo: {verdict[:120]}")
    SNAP[REVIEW_REL] = sha_file(REVIEW_REL)
    ok, _ = accept()
    if ok:
        return rec_gate("claude-diff-review", True, "test_review pasa: sin evidencia, PR sigue")
    state["review_block"] = True
    rec_gate("claude-diff-review", False, f"test_review falla: vuelve a build. {verdict[:120]}")
    return True, "bloqueo con evidencia"


@stage("6-pr")
def pr():
    subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
    subprocess.run(["git", "-c", "user.name=map12", "-c", "user.email=map12082004@gmail.com", "commit", "-q", "-m",
                    f"sdlc: {TASK_NAME} - driver_py, gates + escalera + revision Claude"], cwd=WT, check=True)
    base = state.get("base_sha", "HEAD~1")
    ds = subprocess.run(["git", "diff", "--stat", base], cwd=WT, capture_output=True, text=True).stdout
    state["diffstat"] = ds.strip().splitlines()[-1] if ds.strip() else ""
    ns = subprocess.run(["git", "diff", "--numstat", base, "--", *state["plan_files"]], cwd=WT, capture_output=True, text=True).stdout
    rows = [l.split("\t") for l in ns.splitlines() if l.count("\t") == 2 and l.split("\t")[0].isdigit()]
    state["lines"] = {"added": sum(int(a) for a, _, _ in rows), "deleted": sum(int(d) for _, d, _ in rows)}
    gate_lint()
    state["mutation_score"] = None  # ponytail: mutmut queda para el ticket 13
    return True, state["diffstat"]


def gate_lint() -> tuple[bool, str]:
    """ruff + mypy sobre los archivos del plan. Bench: paquete nuevo, baseline 0. Repo: el hook exige 0."""
    files = state["plan_files"]
    _, rl = sh([PY, "-m", "ruff", "check", "--output-format", "concise", *files])
    _, ml = sh([PY, "-m", "mypy", "--ignore-missing-imports", *files])
    ruff = len(re.findall(r"^\S+:\d+:\d+: ", rl, re.M))
    mypy = int((re.search(r"Found (\d+) error", ml) or [0, 0])[1])
    state["lint_new"] = {"ruff": ruff, "mypy": mypy}
    detail = "0/0" if not (ruff or mypy) else (rl + "\n" + ml)[-1500:]
    return rec_gate("lint", not (ruff or mypy), detail)


if __name__ == "__main__":
    for _s in (sys.stdout, sys.stderr):  # D10: la consola cp1252 tiro UnicodeEncodeError por un "->" de Claude
        _s.reconfigure(encoding="utf-8", errors="replace")
    if FEAT:
        import types
        TASK = types.SimpleNamespace(name=TASK_NAME, task=FEAT["task"],
                                    accept_files={rel: pathlib.Path(src).read_text(encoding="utf-8") for rel, src in FEAT["accept"].items()})
        CONTRACT = FEAT["contract"]
    else:
        TASK = bench.get_task(TASK_NAME)
        CONTRACT = CONTRACTS.get(TASK_NAME, [])
    if "--self-check" in sys.argv:
        sys.exit(self_check())
    # D3 r1: el pre-commit de mmorch corre `python -m ruff` y resolvia al Python del sistema (sin ruff).
    os.environ["PATH"] = os.path.dirname(PY) + os.pathsep + os.environ.get("PATH", "")
    if not (WT / ".git").exists():
        WT.parent.mkdir(parents=True, exist_ok=True)
        if FEAT:
            br = f"sdlc/{PHASE}"
            exists = subprocess.run(["git", "-C", FEAT["repo"], "rev-parse", "--verify", "-q", br], capture_output=True).returncode == 0
            # D13/D14: un intento anterior dejo la branch creada -> `-b` falla (255). Se reusa la branch existente.
            args = ["worktree", "add", "-q", str(WT), br] if exists else ["worktree", "add", "-q", "-b", br, str(WT), "HEAD"]
            subprocess.run(["git", "-C", FEAT["repo"], *args], check=True)
            for rel, content in TASK.accept_files.items():
                _write(rel, content)
            subprocess.run(["git", "add", "-A"], cwd=WT, check=True)
            staged = subprocess.run(["git", "diff", "--cached", "--quiet"], cwd=WT).returncode != 0
            if staged:  # D4: el test de aceptacion puede ser uno que ya vive en el repo -> nada que commitear
                subprocess.run(["git", "-c", "user.name=map12", "-c", "user.email=map12082004@gmail.com", "commit", "-q", "-m",
                                f"sdlc: test de aceptacion {TASK_NAME} (rojo por diseño)"], cwd=WT, check=True)
        else:
            bench.materialize(TASK, str(WT))
        print("materializado", WT, flush=True)
    state["base_sha"] = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=WT, capture_output=True, text=True).stdout.strip() \
        if float(_arg("--from-stage", "2")) <= 3 else "HEAD~1"  # en resume no se conoce: HEAD~1 como antes
    gi = WT / ".gitignore"
    if not gi.exists():
        gi.write_text("__pycache__/\n", encoding="utf-8")  # r1-r3: la review hace git add -A antes del pr
    snapshot_baseline()
    state["t0"] = time.strftime("%Y-%m-%d %H:%M:%S")
    state["t0_epoch"] = time.time()
    from_stage = float(_arg("--from-stage", "2"))
    if from_stage > 3:
        plan_md = (WT / "docs/sdlc/plan.md").read_text(encoding="utf-8")
        m = re.search(r"## Archivos\b(.*?)(?:\n## |\Z)", plan_md, re.S | re.I)
        state["plan_files"] = _plan_files(m.group(1) if m else plan_md)
    for t in TPL.glob("*.md"):  # convencion por repo (ticket 11): las plantillas viajan con el repo
        _write(f"docs/sdlc/{t.name}", t.read_text(encoding="utf-8"))
    stages = {2: spec, 2.5: spec_review, 3: plan, 4: build, 5: test, 5.5: review, 6: pr}
    for k in sorted(stages):
        if k >= from_stage:
            stages[k]()
            if k == 5.5 and state.get("review_block"):
                test()  # una vuelta mas con el test de Claude; si sigue rojo, escala
    state["minutes_total"] = round((time.time() - state["t0_epoch"]) / 60, 2)
    state["usd"] = ledger_usd()
    _flush()
    here = pathlib.Path(__file__).resolve().parent / f"run-log-{PHASE}.json"
    here.write_text(LOG.read_text(encoding="utf-8"), encoding="utf-8")
    print("DRIVER_PY TERMINO", json.dumps({k: state.get(k) for k in
          ("task", "calls", "minutes_total", "usd", "usd_by_family", "human_interventions", "claude_calls", "review_block",
           "escalated_to_claude", "gate_rejects", "diffstat", "lines", "suite_total", "lint_new")}, ensure_ascii=False))
