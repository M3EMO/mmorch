"""mmorch — multi-model orchestration harness (migrated patterns).

Cheap, deterministic orchestration over external model APIs (DeepSeek, Gemini, ...).
The orchestration is plain Python code; the models are nodes. This conserves Claude
cupo by moving bulk generation and cross-family verification off the plan.

Public API:
    from mmorch import (fan_out, adversarial_verify, call, REGISTRY,
                        route, ensemble_verify, Memo, memoized_verify,
                        ideate, ideate_and_screen)
    from mmorch.learn import analyze, recommend
"""
from .config import REGISTRY, family_of, ModelSpec
from .providers import call
from .patterns import fan_out, adversarial_verify
from .route import route, RouteResult                 # I-2
from .cascade import cascade, CascadeResult            # FrugalGPT cascade
from .ensemble import ensemble_verify, EnsembleVerdict  # I-3
from .cache import Memo, memoized_verify, key_of         # I-4
from .innovate import ideate, screen, ideate_and_screen  # I-5
from .feedback import (record_outcome, ThompsonBandit,   # feedback loop (keystone)
                       calibration, read_outcomes)
from .memory import (write_episode, write_note, recall,   # memoria episodica+semantica
                     tombstone_note, embed, Note)

__all__ = [
    "REGISTRY", "family_of", "ModelSpec", "call",
    "fan_out", "adversarial_verify",
    "route", "RouteResult",
    "cascade", "CascadeResult",
    "ensemble_verify", "EnsembleVerdict",
    "Memo", "memoized_verify", "key_of",
    "ideate", "screen", "ideate_and_screen",
    "record_outcome", "ThompsonBandit", "calibration", "read_outcomes",
    "write_episode", "write_note", "recall", "tombstone_note", "embed", "Note",
]
