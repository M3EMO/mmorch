"""Un halt de auto-apply debe pausar a TODOS los loops (un solo kill-switch)."""

from mmorch.adjudicate import adjudicate
from mmorch.auto_apply import _pause
from mmorch.killswitch import paused
from mmorch.promotion import PromotionStore
from mmorch.proposals import compose_cards


def test_auto_apply_pause_is_seen_by_other_loops(tmp_path):
    logs = tmp_path / "logs"
    # Mismo layout que auto_apply_nightly: el store vive en logs/auto_apply.
    _pause(PromotionStore(logs / "auto_apply"))

    assert paused(logs)
    assert compose_cards(logs_dir=str(logs)) == {"skipped": True}
    assert adjudicate(None, "p", str(tmp_path), None, None, logs_dir=str(logs)) == {"skipped": True}
