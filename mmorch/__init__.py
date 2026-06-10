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
                       calibration, read_outcomes,
                       calibrate_conf, reliability_bins,  # #3 calibrated gating
                       contextual_arm)                    # #4 contextual bandit key
from .memory import (write_episode, write_note, recall,   # memoria episodica+semantica
                     tombstone_note, embed, Note)
from .tournament import tournament, TournamentResult        # backlog: best-of-N pairwise
from .bucketrank import bucket_rank, BucketRankResult        # backlog: graduar set en tiers
from .loop import loop_until_done, LoopResult                # backlog: loop-until-dry
from .schema import (gated_json, validate, extract_json,     # §9 schema-gates
                     SchemaGateError)
from .classify import classify, classify_and_act, ClassifyResult  # classify-and-act (front-door)
from .checkers import (check, register_checker, CheckResult,         # tool-verify determinista
                       safe_arith, available as checkers_available)
from .goal import (load_goal, goal_hash, goal_aligned,               # ancla anti-goal-drift
                   authorize_goal, goal_guard, pursue_goal, GoalTampered)
from .budget import (BudgetExceeded, monthly_spend, remaining,       # BudgetKeeper (techo $)
                     check as budget_check, status as budget_status)
from .predict import Predictor, train as train_predictor, cross_val_error  # v0.1 cost/lat predictor
from .evolve import (Change, snapshot_change, apply_change, rollback,      # Fase 3+4 motor
                     evaluate, zone_of, self_evolve, red_content_hits,
                     sandbox_branch, promote_branch, open_pr_branch)        # git-isolated promote
from .prices import effective_prices, load_overrides                       # Fase 2 override precios
from .megasource import fetch_prices, diff_prices, propose_price_update     # Fase 2 megafuente
from .nodes import orchestra, members, conductor as orchestra_conductor, Node  # registry orquesta
from .factory import (featurize_code, train_logreg, train_code_quality,     # fábrica de modelos
                      emit_training_job, predict_proba, accuracy)
from .shadow_prior import (ShadowPrior, offline_improvement,                # Fase 5 NN shadow prior
                           auto_scale as shadow_auto_scale)

__all__ = [
    "REGISTRY", "family_of", "ModelSpec", "call",
    "fan_out", "adversarial_verify",
    "route", "RouteResult",
    "cascade", "CascadeResult",
    "ensemble_verify", "EnsembleVerdict",
    "Memo", "memoized_verify", "key_of",
    "ideate", "screen", "ideate_and_screen",
    "record_outcome", "ThompsonBandit", "calibration", "read_outcomes",
    "calibrate_conf", "reliability_bins", "contextual_arm",
    "write_episode", "write_note", "recall", "tombstone_note", "embed", "Note",
    "tournament", "TournamentResult", "bucket_rank", "BucketRankResult",
    "loop_until_done", "LoopResult",
    "gated_json", "validate", "extract_json", "SchemaGateError",
    "classify", "classify_and_act", "ClassifyResult",
    "check", "register_checker", "CheckResult", "safe_arith", "checkers_available",
    "load_goal", "goal_hash", "goal_aligned",
    "authorize_goal", "goal_guard", "pursue_goal", "GoalTampered",
    "BudgetExceeded", "monthly_spend", "remaining", "budget_check", "budget_status",
    "Predictor", "train_predictor", "cross_val_error",
    "orchestra", "members", "orchestra_conductor", "Node",
    "Change", "snapshot_change", "apply_change", "rollback", "evaluate", "zone_of",
    "self_evolve", "red_content_hits", "sandbox_branch", "promote_branch", "open_pr_branch",
    "effective_prices", "load_overrides", "fetch_prices", "diff_prices", "propose_price_update",
    "featurize_code", "train_logreg", "train_code_quality", "emit_training_job",
    "predict_proba", "accuracy",
    "ShadowPrior", "offline_improvement", "shadow_auto_scale",
]
