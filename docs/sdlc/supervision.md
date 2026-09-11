# supervision.md — driver_py


## candidato 17:13:26
revision de spec (Claude, rc=0): Hice 0 preguntas.

## candidato 17:17:01
ESCALATE_HUMAN: aceptacion verde pero suite total roja
py:6: StarletteDeprecationWarning: Using `httpx` with `starlette.testclient` is deprecated; install `httpx2` instead.
    from starlette.testclient import TestClient

-- Docs: https://docs.pytest.org/en/stable/how-to/capture-warnings.html
=========================== short test summary info ===========================
FAILED tests/test_cache_cost.py::test_repo_prices_json_has_deepseek_cache - A...
!!!!!!!!!!!!!!!!!!!!!!!!!! stopping after 1 failures !!!!!!!!!!!!!!!!!!!!!!!!!!
1 failed, 88 passed, 1 warning in 82.81s (0:01:22)
Exception ignored in atexit callback: <function cleanup_numbered_dir at 0x000002A0AF36BD80>
Traceback (most recent call last):
  File "C:\Users\map12\.claude\orchestration\.venv\Lib\site-packages\_pytest\pathlib.py", line 374, in cleanup_numbered_dir
    cleanup_dead_symlinks(root)
  File "C:\Users\map12\.claude\orchestration\.venv\Lib\site-packages\_pytest\pathlib.py", line 359, in cleanup_dead_symlinks
    if not left_dir.resolve().exists():
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File "C:\Users\map12\AppData\Local\Programs\Python\Python312\Lib\pathlib.py", line 860, in exists
    self.stat(follow_symlinks=follow_symlinks)
  File "C:\Users\map12\AppData\Local\Programs\Python\Python312\Lib\pathlib.py", line 840, in stat
    return os.stat(self, follow_symlinks=follow_symlinks)
           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
PermissionError: [WinError 5] Acceso denegado: 'C:\\Users\\map12\\AppData\\Local\\Temp\\pytest-of-map12\\pytest-current'


## candidato 18:27:43
ESCALATE_HUMAN: aceptacion verde pero suite total con regresion
fallos nuevos: ['tests/test_w6_ronda3.py::test_mcp_server_reporta_version_de_mmorch']

## candidato 18:49:56
revision del diff (Claude, rc=0): NOTE: El branch de excepcion de `gate_fn` (linea 73) guarda `previous_code` aunque el gate no rechazo, solo crasheo. Esto puede disparar 'atascado' comparando contra una vuelta sin veredicto real del gate. La spec no cubre excepciones de `gate_fn`, y los dos tests de aceptacion (`test_mismo_codigo_dos_veces_escala_en_la_segunda`, `test_codigos_distintos_no_escalan_antes_de_max_fix`) pasan sin problema. No escribo test: no logro demostrar violacion de un requisito R1-R5 con esto, es una zona gris fuera del contrato.

Corri los tests relevantes (`driver`, `atasco`, `project_build`, `project_integrate`, `fleet`, `w3*`): todos verdes, sin regresiones.
