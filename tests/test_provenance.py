"""Tests de provenance (outcomes retroactivos por supervivencia de branch)."""

import time

from mmorch.provenance import on_merge, record_branch, sweep_expired


def test_merge_da_reward_completo_al_brazo(tmp_path):
    rewards = []

    def rec(arm, reward, **kw):
        rewards.append((arm, reward, kw.get("source")))

    record_branch("mmorch-sbx-a", arm="deepseek-chat#evolve", origin="evolve",
                  target="mmorch/x.py", logs_dir=str(tmp_path))
    assert on_merge("mmorch-sbx-a", logs_dir=str(tmp_path), record_fn=rec)
    assert rewards == [("deepseek-chat#evolve", 1.0, "merge")]


def test_merge_es_idempotente_y_fail_soft(tmp_path):
    rewards = []

    def rec(arm, reward, **kw):
        rewards.append(arm)

    record_branch("b", arm="x#y", origin="y", logs_dir=str(tmp_path))
    on_merge("b", logs_dir=str(tmp_path), record_fn=rec)
    assert not on_merge("b", logs_dir=str(tmp_path), record_fn=rec)   # 2da vez: no
    assert not on_merge("nunca-registrada", logs_dir=str(tmp_path), record_fn=rec)
    assert len(rewards) == 1


def test_sweep_expira_una_sola_vez_con_rechazo_blando(tmp_path):
    rewards = []

    def rec(arm, reward, **kw):
        rewards.append((arm, reward, kw.get("source")))

    record_branch("vieja", arm="glm-4.6#slim", origin="slim",
                  logs_dir=str(tmp_path))
    record_branch("nueva", arm="glm-4.6#slim", origin="slim",
                  logs_dir=str(tmp_path))
    futuro = time.time() + 15 * 86400
    # 'nueva' re-registrada con ts futuro para que no expire... no se puede
    # via API — en cambio: sweep con now apenas 1 dia despues no expira nada
    assert sweep_expired(logs_dir=str(tmp_path),
                         now=time.time() + 86400, record_fn=rec) == []
    swept = sweep_expired(logs_dir=str(tmp_path), now=futuro, record_fn=rec)
    assert sorted(swept) == ["nueva", "vieja"]
    assert all(r == ("glm-4.6#slim", 0.2, "branch_expirada") for r in rewards)
    # idempotente: la proxima corrida no re-castiga
    assert sweep_expired(logs_dir=str(tmp_path), now=futuro, record_fn=rec) == []


def test_branch_mergeada_no_expira(tmp_path):
    rewards = []

    def rec(arm, reward, **kw):
        rewards.append((reward, kw.get("source")))

    record_branch("m", arm="a#b", origin="b", logs_dir=str(tmp_path))
    on_merge("m", logs_dir=str(tmp_path), record_fn=rec)
    assert sweep_expired(logs_dir=str(tmp_path),
                         now=time.time() + 30 * 86400, record_fn=rec) == []
    assert rewards == [(1.0, "merge")]
