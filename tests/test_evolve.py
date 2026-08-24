"""Tests evolve (subset DGM gated): fitness, archive, propose. Subprocess/API mockeados."""
import sys, pathlib, types
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
import mmorch.evolve as EV
import mmorch.patterns as PAT
from mmorch.providers import CallResult


def _proc(out, rc=0):
    return types.SimpleNamespace(stdout=out, stderr="", returncode=rc)


def test_fitness_all_pass(monkeypatch):
    monkeypatch.setattr(EV.subprocess, "run", lambda *a, **k: _proc("30 passed in 1s", 0))
    f = EV.fitness()
    assert f["passed"] == 30 and f["failed"] == 0 and f["pass_rate"] == 1.0 and f["ok"] is True


def test_fitness_with_failures(monkeypatch):
    monkeypatch.setattr(EV.subprocess, "run", lambda *a, **k: _proc("28 passed, 2 failed in 1s", 1))
    f = EV.fitness()
    assert f["passed"] == 28 and f["failed"] == 2 and f["ok"] is False
    assert 0.9 < f["pass_rate"] < 0.94


def test_archive_roundtrip(tmp_path, monkeypatch):
    monkeypatch.setattr(EV, "_ARCHIVE", tmp_path / "arch.jsonl")
    EV.archive_variant("v1", {"pass_rate": 1.0, "ok": True}, notes="fix X")
    EV.archive_variant("v2", {"pass_rate": 0.5, "ok": False})
    arch = EV.read_archive()
    assert len(arch) == 2 and arch[0]["name"] == "v1" and arch[1]["fitness"]["ok"] is False


def test_propose_patch_is_readonly(monkeypatch, tmp_path):
    # propose NO debe escribir; solo devuelve texto.
    monkeypatch.setattr(EV, "ROOT", tmp_path)
    (tmp_path / "mod.py").write_text("# old", encoding="utf-8")
    monkeypatch.setattr(PAT, "fan_out",
        lambda prompts, **k: [CallResult("deepseek-chat", "deepseek", "# NUEVO", 1, 1, 0.0, 0.0)])
    out = EV.propose_patch("mod.py", "mejorar X")
    assert out == "# NUEVO"
    assert (tmp_path / "mod.py").read_text(encoding="utf-8") == "# old"  # NO se modifico


def test_target_test_file_encuentra_el_match(tmp_path):
    (tmp_path / "mmorch").mkdir()
    (tmp_path / "tests").mkdir()
    (tmp_path / "tests" / "test_repo_mining.py").write_text("", encoding="utf-8")
    assert EV._target_test_file("mmorch/repo_mining.py", root=tmp_path) == "tests/test_repo_mining.py"


def test_target_test_file_sin_match_devuelve_none(tmp_path):
    (tmp_path / "mmorch").mkdir()
    (tmp_path / "tests").mkdir()
    assert EV._target_test_file("mmorch/nada.py", root=tmp_path) is None


def test_target_test_file_fuera_de_mmorch_devuelve_none(tmp_path):
    assert EV._target_test_file("scripts/nightly.py", root=tmp_path) is None


def test_propose_with_fast_retry_sin_test_rapido_un_solo_tiro(tmp_path):
    """Sin tests/test_X.py para el target: un intento, sin feedback (mismo
    contrato viejo de propose_fn, backward-compat con self-checks/tests que
    inyectan un fake de 2 args)."""
    (tmp_path / "mmorch").mkdir()
    (tmp_path / "tests").mkdir()
    calls = []

    def fake_propose(target, finding):
        calls.append((target, finding))
        return "nuevo contenido"

    after, r = EV.propose_with_fast_retry("mmorch/nada.py", "algo", root=tmp_path,
                                          propose_fn=fake_propose)
    assert after == "nuevo contenido" and calls == [("mmorch/nada.py", "algo")]
    assert r == {"skipped": "sin test rapido"}


def test_propose_with_fast_retry_itera_con_feedback_hasta_pasar(tmp_path):
    """Con test rapido disponible: reintenta pasandole el detalle del fallo
    anterior, hasta que el sandbox rapido de ok."""
    (tmp_path / "mmorch").mkdir()
    (tmp_path / "tests").mkdir()
    (tmp_path / "tests" / "test_x.py").write_text("", encoding="utf-8")

    intentos = []

    def fake_propose(target, finding, feedback=""):
        intentos.append(feedback)
        return f"intento {len(intentos)}"

    resultados = iter([
        {"ok": False, "fitness": {"detail": "1 failed: assert False"}},
        {"ok": True, "fitness": {"detail": ""}},
    ])

    def fake_quick_sandbox(change, cmd):
        return next(resultados)

    after, r = EV.propose_with_fast_retry("mmorch/x.py", "algo", root=tmp_path,
                                          propose_fn=fake_propose,
                                          quick_sandbox_fn=fake_quick_sandbox)
    assert after == "intento 2" and r["ok"] is True
    assert intentos == ["", "1 failed: assert False"]  # el 2do intento vio el fallo del 1ro


def test_propose_with_fast_retry_agota_intentos_y_devuelve_el_ultimo(tmp_path):
    (tmp_path / "mmorch").mkdir()
    (tmp_path / "tests").mkdir()
    (tmp_path / "tests" / "test_x.py").write_text("", encoding="utf-8")

    def fake_propose(target, finding, feedback=""):
        return "siempre falla"

    def fake_quick_sandbox(change, cmd):
        return {"ok": False, "fitness": {"detail": "still red"}}

    after, r = EV.propose_with_fast_retry("mmorch/x.py", "algo", root=tmp_path,
                                          max_attempts=2, propose_fn=fake_propose,
                                          quick_sandbox_fn=fake_quick_sandbox)
    assert after == "siempre falla" and r["ok"] is False


def test_sandbox_branch_test_cmd_custom_tambien_lleva_basetemp(monkeypatch, tmp_path):
    """Bug real encontrado escribiendo propose_with_fast_retry: un test_cmd
    inyectado se saltaba el --basetemp por completo (mismo bug de raiz que
    ya se habia arreglado para el cmd default)."""
    monkeypatch.setattr(EV, "_git", lambda *a, **k: types.SimpleNamespace(
        returncode=0, stdout="", stderr=""))
    seen_cmd = {}

    def fake_run(cmd, **kw):
        seen_cmd["cmd"] = cmd
        return _proc("1 passed", 0)

    monkeypatch.setattr(EV.subprocess, "run", fake_run)
    change = EV.Change(id="x1", target="mmorch/x.py", before="a", after="b",
                       description="d")
    EV.sandbox_branch(change, root=tmp_path,
                      test_cmd=[sys.executable, "-m", "pytest", "tests/test_x.py", "-q"])
    assert any("--basetemp" in c for c in seen_cmd["cmd"])


def test_propose_patch_extrae_el_fence(monkeypatch, tmp_path):
    """El bug mas caro del sistema: el ```python del modelo viajaba ADENTRO
    del .py -> SyntaxError -> suite roja -> 12+ noches con 0 PRs. Confirmado
    por aritmetica (+13 chars exactos en slim dos noches = overhead del
    fence) y por los commits corruptos de las sandbox-branches."""
    monkeypatch.setattr(EV, "ROOT", tmp_path)
    (tmp_path / "mod.py").write_text("# old", encoding="utf-8")
    monkeypatch.setattr(PAT, "fan_out",
        lambda prompts, **k: [CallResult("deepseek-chat", "deepseek",
                                         "```python\nx = 1\n```", 1, 1, 0.0, 0.0)])
    out = EV.propose_patch("mod.py", "mejorar X")
    assert out == "x = 1"                    # fence FUERA
    assert "```" not in out


def test_propose_patch_sin_fence_devuelve_texto_limpio(monkeypatch, tmp_path):
    monkeypatch.setattr(EV, "ROOT", tmp_path)
    (tmp_path / "mod.py").write_text("# old", encoding="utf-8")
    monkeypatch.setattr(PAT, "fan_out",
        lambda prompts, **k: [CallResult("deepseek-chat", "deepseek",
                                         "y = 2\n", 1, 1, 0.0, 0.0)])
    assert EV.propose_patch("mod.py", "z") == "y = 2"


def test_pr_lock_muere_cuando_el_trabajo_ya_esta_en_head(monkeypatch, tmp_path):
    """Branch sandbox que sigue existiendo pero cuyo commit YA es ancestro de HEAD:
    el lock por archivo tiene que soltarse igual (sin gh, pr_number es None y el
    lock era permanente -> evolve salteaba el archivo TODAS las noches)."""
    seen = []

    def fake_git(*args, cwd):
        seen.append(args)
        if args[0] == "merge-base":          # es ancestro de HEAD -> mergeado
            return _proc("", rc=0)
        return _proc("", rc=0)               # la branch existe

    monkeypatch.setattr(EV, "_git", fake_git)
    entry = {"branch": "mmorch-sbx-abc", "head_sha": "deadbeef", "pr_number": None}
    assert EV._pr_still_open(entry, root=tmp_path) is False


def test_pr_lock_sigue_vivo_si_el_trabajo_no_llego_a_head(monkeypatch, tmp_path):
    def fake_git(*args, cwd):
        if args[0] == "merge-base":
            return _proc("", rc=1)           # NO es ancestro
        return _proc("", rc=0)               # la branch existe

    monkeypatch.setattr(EV, "_git", fake_git)
    entry = {"branch": "mmorch-sbx-abc", "head_sha": "deadbeef", "pr_number": None}
    assert EV._pr_still_open(entry, root=tmp_path) is True
