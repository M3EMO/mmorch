"""D12 (cableo aprobado 2026-09-14): `megasource` corre desde el nightly, mensual, reversible, sin escribir prices.json.

Contrato:
- `nightly.run_price_check(now=None, propose=None, state_path=None) -> dict`.
- Estado en `paths.home()/nightly_prices.json` con `last_check_ts`. Si la ultima corrida tiene menos de 30 dias,
  devuelve {"ran": False, "reason": "reciente"} y NO llama a `propose`.
- Si corresponde, llama `propose()` (default `megasource.propose_price_update`), guarda `last_check_ts` y devuelve
  {"ran": True, "n_changed": <int>, "diff": <dict>}. Nunca modifica prices.json: solo propone (zona amarilla).
- Si `propose` levanta, devuelve {"ran": True, "error": "<tipo>: <msg>"} sin excepcion y sin actualizar el estado.
- `main()` la invoca dentro de su propio try, despues del digest, con logging via `_log`.
Cero red: propose inyectado.
"""
import json
import time

import mmorch.nightly as N


def test_R1_reciente_no_corre(tmp_path):
    st = tmp_path / "nightly_prices.json"
    st.write_text(json.dumps({"last_check_ts": time.time() - 5 * 86400}), encoding="utf-8")
    calls = []
    r = N.run_price_check(now=time.time(), propose=lambda: calls.append(1) or {"n_changed": 0, "diff": {}}, state_path=st)
    assert r["ran"] is False and calls == [], r


def test_R2_vencido_propone_y_guarda_estado(tmp_path):
    st = tmp_path / "nightly_prices.json"
    st.write_text(json.dumps({"last_check_ts": time.time() - 40 * 86400}), encoding="utf-8")
    now = time.time()
    r = N.run_price_check(now=now, propose=lambda: {"change": "x", "n_changed": 2, "diff": {"deepseek-chat": {"price_in": [1, 2]}}},
                          state_path=st)
    assert r["ran"] is True and r["n_changed"] == 2 and "deepseek-chat" in r["diff"], r
    assert abs(json.loads(st.read_text(encoding="utf-8"))["last_check_ts"] - now) < 1


def test_R3_sin_estado_previo_corre(tmp_path):
    st = tmp_path / "nightly_prices.json"
    r = N.run_price_check(now=time.time(), propose=lambda: {"n_changed": 0, "diff": {}}, state_path=st)
    assert r["ran"] is True and st.exists(), r


def test_R4_propose_que_levanta_no_rompe_ni_avanza_estado(tmp_path):
    st = tmp_path / "nightly_prices.json"

    def boom():
        raise RuntimeError("sin red")
    r = N.run_price_check(now=time.time(), propose=boom, state_path=st)
    assert r["ran"] is True and r["error"].startswith("RuntimeError: sin red"), r
    assert not st.exists()
