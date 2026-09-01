"""W6 ronda 2 — regresiones de los defectos confirmados por los verificadores.

Cada test nombra el defecto que clava: instalado-como-wheel (paths/goal/nightly),
budget est_cost pre-red, smoke que mentia en verde, secretos ENV-style, y el
path traversal del vault. Sin API real.
"""
from __future__ import annotations

import pathlib
import sys

import pytest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

REPO = pathlib.Path(__file__).resolve().parents[1]


# ---- D3 grupo A: home() default de un paquete INSTALADO no es site-packages ----
def test_home_instalado_no_usa_site_packages(monkeypatch, tmp_path):
    import mmorch.paths as P
    monkeypatch.delenv("MMORCH_HOME", raising=False)
    fake = tmp_path / "venv" / "Lib" / "site-packages" / "mmorch"
    monkeypatch.setattr(P, "_REPO_ROOT", fake.parent)
    h = P.home()
    assert "site-packages" not in h.parts
    assert h == pathlib.Path.home() / ".mmorch"


def test_home_checkout_sigue_siendo_el_repo(monkeypatch):
    import mmorch.paths as P
    monkeypatch.delenv("MMORCH_HOME", raising=False)
    assert P.home() == P.repo_root() == REPO


# ---- D2 grupo A: mmorch-nightly --help NO corre el goal gate -------------------
def test_nightly_help_sale_limpio_sin_goal_gate(monkeypatch, capsys):
    import mmorch.nightly as N
    # si el gate corriera, este stub lo delata (antes: HALT + exit 1 en install limpia)
    monkeypatch.setattr(N, "_goal_gate", lambda *a, **k: pytest.fail("goal gate no debe correr con --help"))
    monkeypatch.setattr(sys, "argv", ["mmorch-nightly", "--help"])
    N.main()   # ni SystemExit ni gate
    assert "mmorch-nightly" in capsys.readouterr().out


# ---- D4: est_cost peor-caso viaja al budget check ANTES de la red --------------
def test_call_pasa_est_cost_al_budget(monkeypatch):
    import mmorch.budget as B
    import mmorch.providers as PROV
    seen = {}

    def fake_check(*, est_cost=0.0, critical=False, override=False):
        seen["est"] = est_cost
        raise B.BudgetExceeded("stop")   # corta antes de crear cliente/red

    monkeypatch.setattr(B, "check", fake_check)
    with pytest.raises(B.BudgetExceeded):
        PROV.call("deepseek-chat", "hola " * 100, max_tokens=1000)
    # con ledger en 0, el gate solo muerde si la call trae su costo estimado
    assert seen["est"] > 0.0


# ---- D6: smoke_test NO imprime OK con el generador 100% caido ------------------
def test_smoke_falla_si_fan_out_no_genero(monkeypatch, capsys):
    import smoke_test as SM
    monkeypatch.setattr(SM, "fan_out", lambda *a, **k: [])   # 0/2 generados
    rc = SM.main()
    assert rc == 1
    assert "FAIL" in capsys.readouterr().out


# ---- AT-26 #1: identificadores ENV-style con secreto real son rojos ------------
def test_secret_assign_matchea_env_style():
    from mmorch.evolve import red_content_hits
    hits = red_content_hits('AWS_SECRET_ACCESS_KEY = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"')
    assert hits, "clave AWS literal en identificador ENV-style debe ser hit rojo"
    # el anti-falso-rojo sigue: palabra suelta / placeholder corto no bloquea
    assert red_content_hits('password = "changeme"') == []


# ---- D-TRAVERSAL: folder del vault clampeado -----------------------------------
