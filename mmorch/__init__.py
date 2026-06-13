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
                     tombstone_note, embed, Note, consolidate)
from .tournament import tournament, TournamentResult        # backlog: best-of-N pairwise
from .bucketrank import bucket_rank, BucketRankResult        # backlog: graduar set en tiers
from .loop import loop_until_done, LoopResult                # backlog: loop-until-dry
from .hillclimb import hillclimb, ClimbResult, ClimbCtx, ClimbStep  # goal+rubric loop (Martin 2026)
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
from .code_embedder import embed_code, available as code_embedder_available  # flywheel asset (numpy)
from .code_loop import run_code_task, CodeTaskResult                          # Fase 5 wire (lazo cerrado)
from .rubric_loop import (start_rubric, next_action as rubric_next,           # loop autocorreccion
                          submit as rubric_submit, run_rubric_loop)            # (plan o API)
from .trajectory import (record_trajectory, trajectory_dataset,               # Hermes: trajectory
                         distill_skill, load_trajectories,                     # compression + skills
                         stats as trajectory_stats)
from .memory import recall_keyword, recall_hybrid                             # Hermes: FTS keyword
from .nudge import tick as nudge_tick, status as nudge_status                 # Hermes: memory nudge
from .sandbox import policy_violations, docker_available                      # Hermes: exec policy
from .prompts import cacheable_messages, prefix_signature, shares_prefix      # prefix-stable cache
from .schedule import is_off_peak, advisory as offpeak_advisory, spend_by_period  # off-peak advisory
from .effort import model_for_effort, effort_steps, escalation_models         # effort-routing knob
from .scout import (scout as run_scout, gather_environment,                    # Fable: entorno-primero
                    scout_delta)   # alias run_scout: NO shadowear el submodulo mmorch.scout
from .events import emit as emit_event, bus as event_bus, Event                # nivel 3: bus live
from .enrich import enrich_prompt, enrich_delta                                # Fable: intent enrich

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
    "consolidate",
    "tournament", "TournamentResult", "bucket_rank", "BucketRankResult",
    "loop_until_done", "LoopResult",
    "hillclimb", "ClimbResult", "ClimbCtx", "ClimbStep",
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
    "embed_code", "code_embedder_available",
    "run_code_task", "CodeTaskResult",
    "start_rubric", "rubric_next", "rubric_submit", "run_rubric_loop",
    "record_trajectory", "trajectory_dataset", "distill_skill", "load_trajectories",
    "trajectory_stats",
    "recall_keyword", "recall_hybrid", "nudge_tick", "nudge_status",
    "policy_violations", "docker_available",
    "cacheable_messages", "prefix_signature", "shares_prefix",
    "is_off_peak", "offpeak_advisory", "spend_by_period",
    "model_for_effort", "effort_steps", "escalation_models",
    "run_scout", "gather_environment", "scout_delta",
    "emit_event", "event_bus", "Event",
    "enrich_prompt", "enrich_delta",
]
