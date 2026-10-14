"""orchestration-4rf (notas de campo en el prompt del coder) y orchestration-s8w (comandos del repo en un contenedor).
Cero API. El test de integracion corre solo si hay docker/podman y la imagen ya esta local (no descarga nada)."""
import shutil
import subprocess
import types

import pytest

import mmorch.sandbox as SB
import mmorch.sdlc as S

IMG = "docker.io/library/python:3.12-slim"


def _wt(tmp_path, **cfg):
    t = types.SimpleNamespace(name="c", task="t", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(tmp_path), "files": ["a.py"], "suite": []}, wt=tmp_path,
                accept_cmd="python -c pass")
    S.CFG.update(cfg)
    S.state["plan_files"] = ["a.py"]


def test_rol_sin_notas_queda_igual(tmp_path, monkeypatch):
    _wt(tmp_path)
    monkeypatch.delenv("SDLC_NOTAS", raising=False)
    assert "Notas de campo" not in S._rol("a.py")


def test_rol_con_notas_agrega_el_bloque_congelado(tmp_path, monkeypatch):
    _wt(tmp_path)
    notas = tmp_path / "notas.txt"
    notas.write_text("Tarea: X\nProblemas: G3-compile → import faltante → agregar import\n", encoding="utf-8")
    monkeypatch.setenv("SDLC_NOTAS", str(notas))
    rol = S._rol("a.py")
    assert rol.startswith("Sos un programador Python senior")
    assert "<<<\nTarea: X\nProblemas: G3-compile → import faltante → agregar import\n>>>" in rol
    assert rol.rstrip().endswith("Pueden no aplicar a este archivo.")


def test_container_argv_monta_en_la_misma_ruta_y_sin_red():
    a = SB.container_argv(["python", "-m", "pytest"], image="img", cwd="/w", mounts=["/w", "/tmp/x"], name="n", eng="podman")
    assert a[:5] == ["podman", "run", "--rm", "--name", "n"]
    assert a[a.index("--network") + 1] == "none" and "--cap-drop" in a
    vols = [a[i + 1] for i, x in enumerate(a) if x == "-v"]
    assert [v.split(":")[0] == v.split(":")[1] for v in vols] == [True, True]
    assert a[-4:] == ["img", "python", "-m", "pytest"]
    assert SB.container_argv("echo hi", image="img", cwd="/w", mounts=[], name="n", eng="docker")[-3:] == ["sh", "-c", "echo hi"]


def test_sh_en_modo_contenedor_reemplaza_el_python_del_host(tmp_path, monkeypatch):
    _wt(tmp_path, contenedor="img")
    monkeypatch.setattr(SB, "engine", lambda: "podman")
    visto = {}

    def fake_run(cmd, **kw):
        visto["cmd"] = cmd
        return subprocess.CompletedProcess(cmd, 0, "ok", "")

    monkeypatch.setattr(S.subprocess, "run", fake_run)
    ok, _ = S.sh([S.PY, "-m", "pytest", "-q"])
    assert ok and visto["cmd"][0] == "podman" and visto["cmd"][-4:] == ["python", "-m", "pytest", "-q"]
    assert S.PY not in visto["cmd"]
    S.sh(["ruff", "check"], host=True)
    assert visto["cmd"] == ["ruff", "check"]  # host=True no entra al contenedor


def test_sh_sin_motor_falla_la_etapa(tmp_path, monkeypatch):
    _wt(tmp_path, contenedor="img")
    monkeypatch.setattr(SB, "engine", lambda: None)
    with pytest.raises(S.StageFailed, match="docker ni podman"):
        S.sh("true")


def _imagen_local() -> bool:
    eng = SB.engine()
    return bool(eng) and subprocess.run([eng, "image", "exists" if eng == "podman" else "inspect", IMG],
                                        capture_output=True).returncode == 0


@pytest.mark.skipif(not _imagen_local(), reason=f"sin docker/podman o sin la imagen {IMG} local")
def test_integracion_comando_real_en_el_contenedor(tmp_path):
    _wt(tmp_path, contenedor=IMG)
    (tmp_path / "a.py").write_text("print(6 * 7)\n", encoding="utf-8")
    ok, out = S.sh([S.PY, "a.py"])
    assert ok and out.strip() == "42"
    ok, out = S.sh("python -c \"import urllib.request as u; u.urlopen('http://1.1.1.1', timeout=3)\"")
    assert not ok  # --network none
    ok, _ = S.sh("touch creado.txt")
    assert ok and (tmp_path / "creado.txt").exists()  # escribe en el worktree del host
    assert shutil.which(SB.engine())
