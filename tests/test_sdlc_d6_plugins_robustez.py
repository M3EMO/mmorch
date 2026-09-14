"""D6 (robustez por modulos: `plugins`): el host nunca crashea por lo que haga el worker o el plugin.

Modos de falla cubiertos, todos deterministas y sin red:
- R1 un plugin que imprime en stdout al IMPORTAR (antes del redirect del worker) no corrompe el protocolo.
- R2 un plugin colgado se corta por timeout y el error dice "timeout", no "exited".
- R3 un entry que no importa (SyntaxError) devuelve {ok: False, error: ...} con la causa, sin decir "timeout".
"""
import json
import textwrap
from pathlib import Path

from mmorch.plugins import invoke, load_manifest


def _plugin(tmp_path: Path, name: str, main_src: str) -> dict:
    d = tmp_path / name
    d.mkdir()
    (d / "plugin.json").write_text(json.dumps({
        "name": name, "version": "1", "entry": "main.py", "capabilities": [],
        "contributes": [{"kind": "pattern", "name": "go"}],
    }), encoding="utf-8")
    (d / "main.py").write_text(textwrap.dedent(main_src), encoding="utf-8")
    return load_manifest(d, allow=set())


def test_R1_print_al_importar_no_corrompe_el_protocolo(tmp_path):
    man = _plugin(tmp_path, "ruidoso", '''
        print("hola desde el import")      # va a stdout ANTES del redirect del worker
        def go(args, host):
            return {"ok": True, "n": args["n"] + 1}
    ''')
    res = invoke(man, "go", {"n": 1}, host_services={}, timeout=20)
    assert res["ok"] is True, res
    assert res["value"] == {"ok": True, "n": 2}, res


def test_R2_timeout_se_reporta_como_timeout(tmp_path):
    man = _plugin(tmp_path, "colgado", '''
        import time
        def go(args, host):
            time.sleep(30)
            return "nunca"
    ''')
    res = invoke(man, "go", {}, host_services={}, timeout=0.8)
    assert res["ok"] is False, res
    assert "timeout" in res["error"].lower() and "exited" not in res["error"].lower(), res


def test_R3_entry_roto_devuelve_error_con_causa(tmp_path):
    man = _plugin(tmp_path, "roto", '''
        def go(args, host)
            return 1
    ''')
    res = invoke(man, "go", {}, host_services={}, timeout=20)
    assert res["ok"] is False, res
    assert "timeout" not in res["error"].lower(), res
    assert "syntax" in res["error"].lower() or "load" in res["error"].lower() or "import" in res["error"].lower(), res
