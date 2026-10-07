"""orchestration-ayz: despachador de Cursor. Cero Cursor real: run_agent es un doble que edita el worktree como lo
haria Cursor y devuelve su stream-json."""
import json
import subprocess
from pathlib import Path

import pytest

import mmorch.cursor_worker as W


def _git(d, *a):
    return subprocess.run(["git", "-C", str(d), *a], capture_output=True, text=True, check=True).stdout


def _repo(tmp_path) -> Path:
    r = tmp_path / "repo"
    r.mkdir()
    (r / "a.py").write_text("x = 1\n", encoding="utf-8")
    (r / "viejo.py").write_text("y = 2\n", encoding="utf-8")
    (r / "sdlc.toml").write_text('accept_cmd = "python -m pytest tests -q"\n', encoding="utf-8")
    _git(r, "init", "-q")
    _git(r, "add", "-A")
    _git(r, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "base")
    (r / "a.py").write_text("x = 1\nz = 3\n", encoding="utf-8")       # cambio sin commitear: viaja al worktree
    (r / "nuevo.py").write_text("w = 4\n", encoding="utf-8")           # sin seguimiento: viaja
    (r / ".env").write_text("TOKEN=secreto\n", encoding="utf-8")       # secreto: no viaja
    return r


def _stream(result="RESUMEN: listo.\nARCHIVOS: c.py\nACCIONES BLOQUEADAS: ninguna", extra=()):
    ev = [{"type": "system", "session_id": "s-123"}, *extra,
          {"type": "result", "result": result, "usage": {"inputTokens": 100, "outputTokens": 20}}]
    return "\n".join(json.dumps(e) for e in ev)


@pytest.fixture
def entorno(tmp_path, monkeypatch):
    monkeypatch.setenv("MMORCH_HOME", str(tmp_path / "home"))
    monkeypatch.setattr(W, "wt_root", lambda: tmp_path / "wts")
    monkeypatch.setattr(W, "cursor_argv", lambda: ["node", str(tmp_path / "2026.10.01-abc" / "index.js")])
    sens = tmp_path / "sensible.txt"
    sens.write_text("original", encoding="utf-8")
    monkeypatch.setattr(W, "sensibles", lambda repo: [sens])
    monkeypatch.setattr(W, "arboles", lambda repo: [])
    monkeypatch.setattr(W, "claves_json", lambda: [])
    import mmorch.metrics as M
    monkeypatch.setattr(M, "log_event", lambda **k: None)
    return {"sens": sens, "llamadas": []}


def _agente(entorno, efecto, salida=None):
    def fake(argv, cwd, prompt, env, timeout):
        entorno["llamadas"].append({"argv": argv, "cwd": Path(cwd), "env": env})
        efecto(Path(cwd))
        return salida or _stream()
    return fake


def test_repo_prohibido_y_no_git(tmp_path, entorno):
    p = tmp_path / "Portfolio financiero"
    p.mkdir()
    with pytest.raises(W.Rechazo, match="prohibido"):
        W.correr(p, "pedido")
    with pytest.raises(W.Rechazo, match="no es un repo git"):
        W.correr(tmp_path, "pedido")


def test_correr_y_aplicar(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)
    vistos = {}

    def efecto(wt):
        vistos["cli"] = json.loads((wt / ".cursor" / "cli.json").read_text(encoding="utf-8"))
        vistos["a"] = (wt / "a.py").read_text(encoding="utf-8")
        vistos["nuevo"] = (wt / "nuevo.py").exists()
        vistos["env"] = (wt / ".env").exists()
        (wt / "c.py").write_text("c = 5\n", encoding="utf-8")
    monkeypatch.setattr(W, "run_agent", _agente(entorno, efecto))
    r = W.correr(repo, "Crea c.py")
    llamada = entorno["llamadas"][0]
    assert "--force" not in llamada["argv"] and "SHELL" not in llamada["env"] and "CLAUDECODE" not in llamada["env"]
    assert vistos["a"] == "x = 1\nz = 3\n" and vistos["nuevo"] and not vistos["env"]
    assert vistos["cli"]["permissions"]["deny"] == ["Mcp(*:*)"]
    assert "Shell(python -m pytest tests -q*)" in vistos["cli"]["permissions"]["allow"]
    assert r["estado"] == "listo" and r["archivos"] == ["c.py"] and r["session_id"] == "s-123" and r["tokens"] == 120
    patch = Path(r["diff"]).read_text(encoding="utf-8")
    assert "c.py" in patch and "cli.json" not in patch and "z = 3" not in patch
    fila = json.loads(W.log_path().read_text(encoding="utf-8").splitlines()[-1])
    assert fila["evento"] == "correr" and fila["cli_version"] == "2026.10.01-abc"
    r = W.aplicar(r["run_id"])
    assert (repo / "c.py").read_text(encoding="utf-8") == "c = 5\n" and r["estado"] == "aplicado"
    assert not Path(r["wt"]).exists() and (repo / "a.py").read_text(encoding="utf-8") == "x = 1\nz = 3\n"


def test_alerta_impide_aplicar(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)

    def efecto(wt):
        (wt / "c.py").write_text("c = 5\n", encoding="utf-8")
        entorno["sens"].write_text("tocado por cursor", encoding="utf-8")   # escritura fuera del worktree
    monkeypatch.setattr(W, "run_agent", _agente(entorno, efecto))
    r = W.correr(repo, "Crea c.py")
    assert r["estado"] == "alerta" and r["alerta"] == [str(entorno["sens"])]
    with pytest.raises(W.Rechazo, match="ALERTA"):
        W.aplicar(r["run_id"])
    assert not (repo / "c.py").exists()
    W.descartar(r["run_id"], "alerta")
    assert not Path(r["wt"]).exists()


def test_borrados_y_protegidos_piden_permiso(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)

    def efecto(wt):
        (wt / "viejo.py").unlink()
        (wt / ".claude").mkdir()
        (wt / ".claude" / "settings.json").write_text("{}", encoding="utf-8")
    monkeypatch.setattr(W, "run_agent", _agente(entorno, efecto))
    r = W.correr(repo, "Limpia")
    assert r["borrados"] == ["viejo.py"] and r["protegidos"] == [".claude/settings.json"]
    with pytest.raises(W.Rechazo, match="borra"):
        W.aplicar(r["run_id"])
    with pytest.raises(W.Rechazo, match="credenciales o config"):
        W.aplicar(r["run_id"], permitir_borrados=True)
    W.aplicar(r["run_id"], permitir_borrados=True, permitir_protegidos=True)
    assert not (repo / "viejo.py").exists() and (repo / ".claude" / "settings.json").exists()


def test_reintento_y_modelo_de_respaldo(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)
    n = {"i": 0}

    def fake(argv, cwd, prompt, env, timeout):
        n["i"] += 1
        entorno["llamadas"].append(argv)
        if n["i"] < 3:
            raise RuntimeError("agente: rc=1 cupo")
        return _stream()
    monkeypatch.setattr(W, "run_agent", fake)
    r = W.correr(repo, "Crea c.py")
    modelos = [a[a.index("--model") + 1] for a in entorno["llamadas"]]
    assert modelos == [W.MODELO, W.MODELO, W.RESPALDO] and r["modelo_usado"] == W.RESPALDO and r["intentos"] == 3


def test_una_sola_correccion_con_resume(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)
    monkeypatch.setattr(W, "run_agent", _agente(entorno, lambda wt: (wt / "c.py").write_text("c = 5\n", encoding="utf-8")))
    r = W.correr(repo, "Crea c.py")
    monkeypatch.setattr(W, "run_agent", _agente(entorno, lambda wt: (wt / "c.py").write_text("c = 6\n", encoding="utf-8")))
    r = W.corregir(r["run_id"], "Usa 6")
    a = entorno["llamadas"][-1]["argv"]
    assert a[a.index("--resume") + 1] == "s-123" and "c = 6" in Path(r["diff"]).read_text(encoding="utf-8")
    with pytest.raises(W.Rechazo, match="una sola vuelta"):
        W.corregir(r["run_id"], "otra")


def test_acciones_bloqueadas_en_el_resumen(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)
    rechazo = {"type": "tool_call", "subtype": "completed",
               "tool_call": {"shellToolCall": {"result": {"rejected": {"command": "npm install left-pad"}}}}}
    monkeypatch.setattr(W, "run_agent", _agente(entorno, lambda wt: None, _stream(extra=[rechazo])))
    r = W.correr(repo, "Instala algo")
    assert r["bloqueadas"] == ["npm install left-pad"] and r["herramientas"] == {"shellToolCall": 1}


def test_rename_cuenta_como_borrado_y_protegido(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)
    (repo / "CLAUDE.md").write_text("reglas\n", encoding="utf-8")
    _git(repo, "add", "CLAUDE.md")
    _git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "claude")

    def efecto(wt):
        (wt / "CLAUDE.md").rename(wt / "docs.md")   # git lo veria como rename R100
    monkeypatch.setattr(W, "run_agent", _agente(entorno, efecto))
    r = W.correr(repo, "Mueve")
    assert "CLAUDE.md" in r["borrados"] and "CLAUDE.md" in r["protegidos"]
    with pytest.raises(W.Rechazo):
        W.aplicar(r["run_id"])


def test_alerta_persiste_y_corregir_se_niega(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)

    def efecto(wt):
        entorno["sens"].write_text("tocado", encoding="utf-8")
    monkeypatch.setattr(W, "run_agent", _agente(entorno, efecto))
    r = W.correr(repo, "Algo")
    with pytest.raises(W.Rechazo, match="ALERTA"):
        W.corregir(r["run_id"], "otra cosa")
    assert W._cargar(r["run_id"])["estado"] == "alerta"


def test_falla_tecnica_calcula_igual_la_alerta(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)

    def fake(argv, cwd, prompt, env, timeout):
        entorno["sens"].write_text("tocado y colgado", encoding="utf-8")
        raise RuntimeError("agente: timeout")
    monkeypatch.setattr(W, "run_agent", fake)
    with pytest.raises(RuntimeError):
        W.correr(repo, "Algo")
    run_id = sorted(p.name for p in W.runs_dir().iterdir())[-1]
    r = W._cargar(run_id)
    assert r["estado"] == "alerta" and r["alerta"] == [str(entorno["sens"])]
    W.descartar(run_id, "colgado")


def test_junction_de_node_modules_no_se_borra_y_se_vigila(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)
    (repo / ".gitignore").write_text("node_modules/\n", encoding="utf-8")
    (repo / "sdlc.toml").write_text('accept_cmd = "node --test"\nseed_globs = ["node_modules"]\n', encoding="utf-8")
    pkg = repo / "node_modules" / "pkg" / "index.js"
    pkg.parent.mkdir(parents=True)
    pkg.write_text("module.exports = 1\n", encoding="utf-8")
    monkeypatch.setattr(W, "run_agent", _agente(entorno, lambda wt: (wt / "c.py").write_text("c\n", encoding="utf-8")))
    r = W.correr(repo, "Algo")
    assert r["estado"] == "listo" and r["sembrados_reales"] == [str(repo / "node_modules")]
    W.descartar(r["run_id"])
    assert pkg.read_text(encoding="utf-8") == "module.exports = 1\n"   # el destino de la junction sigue intacto

    def inyecta(wt):
        (wt / "node_modules" / "pkg" / "index.js").write_text("malo\n", encoding="utf-8")
    monkeypatch.setattr(W, "run_agent", _agente(entorno, inyecta))
    r = W.correr(repo, "Algo")
    assert r["estado"] == "alerta" and str(repo / "node_modules") in r["alerta"]
    W.descartar(r["run_id"])


def test_protegidos_anidados_y_repo_prohibido_por_remoto(tmp_path, entorno):
    assert W._protegido("apps/web/.claude/settings.json") and W._protegido("sub/.cursor/rules/a.mdc")
    assert W._protegido("sdlc.toml") and W._protegido(".mcp.json") and W._protegido("conf/.NPMRC")
    assert not W._protegido("src/claude_client.py")
    repo = _repo(tmp_path)
    _git(repo, "remote", "add", "origin", "https://github.com/x/Portfolio-financiero.git")
    with pytest.raises(W.Rechazo, match="prohibido"):
        W.correr(repo, "Algo")


def test_descartar_no_acepta_una_corrida_aplicada(tmp_path, entorno, monkeypatch):
    repo = _repo(tmp_path)
    monkeypatch.setattr(W, "run_agent", _agente(entorno, lambda wt: (wt / "c.py").write_text("c\n", encoding="utf-8")))
    r = W.correr(repo, "Algo")
    W.aplicar(r["run_id"], motivo="tests verdes")
    with pytest.raises(W.Rechazo, match="ya se aplico"):
        W.descartar(r["run_id"])
    fila = json.loads(W.log_path().read_text(encoding="utf-8").splitlines()[-1])
    assert fila["evento"] == "aplicar" and fila["motivo"] == "tests verdes" and fila["modelo_usado"] == W.MODELO
