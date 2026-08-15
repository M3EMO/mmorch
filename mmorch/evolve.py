"""evolve — subset DGM-inspirado, GATED (research: vault/research/
darwin-godel-machine-self-improving-agents). La critica cross-family marco que el
DGM completo (evolucion poblacional open-ended + auto-modificacion) es overreach
para mmorch. Aca solo el subset seguro:

- fitness(): corre el test suite (gate empirico) y devuelve pass-rate. Es la
  "performance empirica" del DGM, pero usando los tests propios como benchmark.
- archive: registro append-only de intentos de evolucion + su fitness (la
  "poblacion/archivo" del DGM, sin la evolucion automatica).
- propose_patch(): un modelo barato PROPONE un cambio (read-only, NO lo aplica).

NUNCA auto-modifica vivo. Aplicar un patch = sandbox + fitness verde + gate humano.
"""
from __future__ import annotations

import hashlib
import json
import logging
import re
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .iohelpers import atomic_write_json, load_json_tolerant, read_jsonl_tolerant

_log = logging.getLogger(__name__)

ROOT = Path(__file__).resolve().parent.parent
_ARCHIVE = ROOT / "logs" / "evolution_archive.jsonl"


# --------------------------------------------------------------------------- #
# FASE 3 — Change + rollback() + evaluate() (fitness compuesta, reversible)    #
# --------------------------------------------------------------------------- #
@dataclass
class Change:
    """Un cambio candidato. `before` = snapshot (la reversibilidad first-class: sin
    snapshot no se puede rollback -> no se auto-aplica)."""
    target: str            # path relativo a root
    after: str             # contenido nuevo
    before: str            # snapshot previo (para rollback)
    description: str       # para goal_aligned
    id: str = ""
    notes: str = ""

    def __post_init__(self):
        if not self.id:
            self.id = hashlib.sha256(
                f"{self.target}\x00{self.after}".encode("utf-8")).hexdigest()[:12]


def snapshot_change(target: str, after: str, description: str, *, root: Path = ROOT,
                    notes: str = "") -> Change:
    p = Path(root) / target
    before = p.read_text(encoding="utf-8") if p.exists() else ""
    return Change(target=target, after=after, before=before, description=description, notes=notes)


def apply_change(change: Change, *, root: Path = ROOT) -> None:
    p = Path(root) / change.target
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(change.after, encoding="utf-8")


def rollback(change: Change, *, root: Path = ROOT) -> bool:
    """Restaura el snapshot `before`. Devuelve True si el archivo quedó == before.
    Reversibilidad first-class: si esto no puede restaurar, el cambio nunca debió
    auto-aplicarse. (Tombstone de notas/episodios lo hace el caller vía memory.)"""
    p = Path(root) / change.target
    try:
        if change.before == "" and not _existed_before(change):
            if p.exists():
                p.unlink()           # era archivo nuevo -> borrarlo
            return not p.exists()
        p.write_text(change.before, encoding="utf-8")
        return p.read_text(encoding="utf-8") == change.before
    except Exception:
        return False


def _existed_before(change: Change) -> bool:
    return change.before != ""


def evaluate(change: Change, *, root: Path = ROOT, run_tests: bool = True,
             goal: bool = True, goal_fn=None, test_path: str = "tests",
             check_cost: bool = True, cost_fn=None,
             check_ensemble: bool = True, ensemble_fn=None,
             isolate: str = "branch") -> dict:
    """fitness() compuesta de Fase 3 — las 6 OBLIGATORIAS del GOAL (invariante 'Gate antes
    de aplicar'). Cualquiera que falle aborta:
      1. ast_valid del contenido nuevo (checker determinista).
      2. tests_green (si run_tests): aplica al repo, pytest, SIEMPRE revierte (finally).
      3. ensemble cross-family (escéptico de seguridad/no-regresión). Inyectable.
      4. rollback PROBADO: apply -> rollback restaura `before` bit-a-bit (sandbox).
      5. cost_ok: no-degradación de costo (≤10% verde / ≤20% amarillo). Inyectable.
      6. goal_aligned(description) — alineación con GOAL (cross-family). Inyectable.
    Devuelve {ok, checks, ensemble_degraded}. ensemble_degraded=True si hoy solo hay 1
    familia de verificador (Kimi inactivo) → el ensemble-AZUL de 2 cae al cross-family
    simple (honesto, no se finge)."""
    from .checkers import check as _check
    checks: dict[str, bool] = {}
    ensemble_degraded = False

    checks["ast_valid"] = _check("python_ast_valid", code=change.after).passed
    checks["rollback_works"] = _rollback_roundtrip(change)   # reversibilidad first-class

    if goal:
        # TAMPER-HALT (B1): goal_guard estaba MUERTO (nunca se llamaba). evaluate() consume
        # GOAL.md como rubrica via goal_aligned; si GOAL.md fue manipulado fuera de banda, esa
        # rubrica esta envenenada. goal_guard frena ANTES de usarla. GoalTampered PROPAGA
        # (no se swallowea): aborta el ciclo de auto-aplicacion entero.
        from .goal import goal_guard as _goal_guard
        _goal_guard()
        gf = goal_fn or _default_goal_fn()
        checks["goal_aligned"] = gf(change.description).passed

    if check_ensemble:
        ok, ensemble_degraded = _ensemble_check(change, ensemble_fn)
        checks["ensemble_xfamily"] = ok

    if check_cost:
        checks["cost_ok"] = _cost_check(change, cost_fn)        # no-degradación (relativo)
        checks["budget_ok"] = _budget_ok()                       # BudgetKeeper (absoluto)

    if run_tests:
        if isolate == "branch":
            # aislamiento REAL: git worktree, no muta el repo vivo. Verde deja la branch
            # (pa promote/PR). Es el reemplazo de la mutación in-place con finally-revert.
            res = sandbox_branch(change, root=root, test_path=test_path)
            checks["tests_green"] = res["ok"]
            sandbox = {"branch": res.get("branch"), "fitness": res.get("fitness")}
        else:
            checks["tests_green"] = _tests_with_autorevert(change, root=root, test_path=test_path)
            sandbox = {"mode": "inplace-autorevert"}
    else:
        sandbox = None

    return {"ok": all(checks.values()), "checks": checks, "change_id": change.id,
            "ensemble_degraded": ensemble_degraded, "sandbox": sandbox}


def _default_goal_fn():
    from .goal import goal_aligned
    return goal_aligned


def _ensemble_check(change: Change, ensemble_fn):
    """Escéptico cross-family de seguridad/no-regresión. Si hay ≥2 familias de verificador
    -> ensemble-AZUL real. Hoy (Kimi inactivo) solo Google activa como verificador ->
    degrada a UN cross-family verify (honesto: marca ensemble_degraded). Inyectable."""
    if ensemble_fn is not None:
        return bool(ensemble_fn(change)), False
    from .config import REGISTRY
    import os
    verifier_families = {s.family for k, s in REGISTRY.items()
                         if s.family != "deepseek" and os.getenv(s.api_key_env)}
    rubric = ("¿Este cambio es SEGURO y NO introduce regresión ni viola invariantes "
              "(reversibilidad, zona roja, OneFlow)? Refutá si hay riesgo.")
    if len(verifier_families) >= 2:
        from .ensemble import ensemble_verify
        ev = ensemble_verify(change.after, rubric=rubric, phase="evolve")
        return ev.passed, False
    # degradado: un solo cross-family verify
    from .patterns import adversarial_verify
    v = adversarial_verify(change.after, rubric=rubric, phase="evolve", task_kind="subjective")
    return v.passed, True


def _cost_check(change: Change, cost_fn) -> bool:
    """No-degradación de costo. cost_fn(change)->bool si se inyecta. Default conservador:
    un cambio que SOLO agrega archivo nuevo (before=='') no toca el hot-path -> no degrada;
    un cambio que MODIFICA hot-path requiere medición explícita (cost_fn) o falla cerrado."""
    if cost_fn is not None:
        return bool(cost_fn(change))
    if not _existed_before(change):
        return True                      # archivo nuevo aislado: no degrada el costo existente
    # modifica algo existente sin medición -> fail-closed (exige cost_fn que mida)
    return False


def _budget_ok() -> bool:
    """Invariante 'Costo acotado' ABSOLUTO: respeta el BudgetKeeper. No auto-aplicar si el
    gasto del mes ya superó el límite (sin límite configurado = ilimitado = OK)."""
    from .budget import max_monthly_usd, remaining
    lim = max_monthly_usd()
    return lim is None or (remaining() or 0) > 0


def _rollback_roundtrip(change: Change) -> bool:
    """En un dir temporal: simula before, aplica after, rollback, verifica == before."""
    import tempfile
    with tempfile.TemporaryDirectory(prefix="mmorch_rb_") as td:
        troot = Path(td)
        tgt = troot / change.target
        tgt.parent.mkdir(parents=True, exist_ok=True)
        existed = _existed_before(change)
        if existed:
            tgt.write_text(change.before, encoding="utf-8")
        apply_change(change, root=troot)
        ok = rollback(change, root=troot)
        if existed:
            ok = ok and tgt.exists() and tgt.read_text(encoding="utf-8") == change.before
        else:
            ok = ok and not tgt.exists()
        return ok


def _tests_with_autorevert(change: Change, *, root: Path = ROOT, test_path: str = "tests") -> bool:
    """Aplica el cambio al repo, corre pytest, SIEMPRE revierte (finally). Nunca deja
    el repo mutado. (Solo se usa si el cambio toca el repo vivo.)"""
    p = Path(root) / change.target
    original = p.read_text(encoding="utf-8") if p.exists() else None
    try:
        apply_change(change, root=root)
        return fitness(test_path=test_path)["ok"]
    finally:
        if original is None:
            if p.exists():
                p.unlink()
        else:
            p.write_text(original, encoding="utf-8")


def fitness(test_path: str = "tests", timeout: int = 1800) -> dict:
    """Corre pytest y devuelve {passed, failed, total, pass_rate, ok}. Gate empirico."""
    proc = subprocess.run(
        [sys.executable, "-m", "pytest", test_path, "-q", "--no-header"],
        cwd=str(ROOT), capture_output=True, text=True, timeout=timeout)
    out = (proc.stdout or "") + (proc.stderr or "")
    passed = _count(out, r"(\d+) passed")
    failed = _count(out, r"(\d+) failed")
    total = passed + failed
    return {
        "passed": passed, "failed": failed, "total": total,
        "pass_rate": round(passed / total, 4) if total else 0.0,
        "ok": proc.returncode == 0 and failed == 0 and passed > 0,
    }


def _count(text: str, pat: str) -> int:
    m = re.search(pat, text)
    return int(m.group(1)) if m else 0


def archive_variant(name: str, fit: dict, notes: str = "", applied: bool = False) -> None:
    """Registra un intento de evolucion + su fitness (append-only)."""
    rec = {"ts": time.time(),
           "iso": time.strftime("%Y-%m-%dT%H:%M:%S", time.localtime()),
           "name": name, "fitness": fit, "applied": applied, "notes": notes}
    _ARCHIVE.parent.mkdir(parents=True, exist_ok=True)
    with open(_ARCHIVE, "a", encoding="utf-8") as fh:
        fh.write(json.dumps(rec, ensure_ascii=False) + "\n")


def read_archive() -> list[dict]:
    return read_jsonl_tolerant(_ARCHIVE)


# --------------------------------------------------------------------------- #
# FASE 4 — self_evolve(): el motor (ideate -> fitness -> zona -> apply -> audit) #
# --------------------------------------------------------------------------- #
# Zona ROJA por path: nunca auto-aplicar (gate humano). Coincide con GOAL.md cat.4.
_RED_PATHS = ("GOAL.md", "GOAL.hash", ".env", "mmorch/goal.py", "mmorch/budget.py",
              "mmorch/config.py")


# Firmas de ACCIONES zona-roja en el CONTENIDO generado (no solo el path): un cambio de
# código que INTRODUCE estas capacidades es rojo aunque el archivo sea nuevo/aislado.
_RED_CONTENT = re.compile(
    r"\b(os\.system|subprocess\.(?:run|Popen|call)|shutil\.rmtree|os\.remove|os\.unlink|"
    r"\beval\s*\(|\bexec\s*\(|__import__|rm\s+-rf|DROP\s+TABLE|TRUNCATE|"
    r"requests\.(?:post|put|delete|patch)|socket\.|"
    r"transfer|withdraw|wallet|exchange|stripe|paypal|place_order|send_money|private_key|"
    r"secret_key|seed_phrase)\b", re.I)


def red_content_hits(text: str, *, baseline: str | None = None) -> list[str]:
    """Firmas de acción zona-roja en el contenido (vacío = limpio). Con `baseline`, devuelve
    solo las firmas NUEVAS (delta): las que ya estaban en baseline no cuentan. Esto arregla el
    auto-lock medido (2026-07: evolve.py CONTIENE la regex de firmas como strings -> se marcaba
    rojo a sí mismo). El escudo real queda: un self-edit que INTRODUCE una capacidad peligrosa
    nueva (que no estaba en baseline) sigue siendo rojo. No es caso-especial de 'self' —
    principiado, aplica a cualquier archivo que ya mencione firmas legítimamente (regex, docs)."""
    hits = set(m.group(0) for m in _RED_CONTENT.finditer(text or ""))
    if baseline is not None:
        hits -= set(m.group(0) for m in _RED_CONTENT.finditer(baseline))
    return sorted(hits)


def zone_of(change: Change, *, root: Path = ROOT) -> str:
    """Clasifica por reversibilidad x blast-radius. ROJA = path prohibido, escapa del repo,
    O el CONTENIDO introduce una acción zona-roja (dinero/borrado/SO/red/claves) — un
    sistema que auto-genera código DEBE screenear capacidades peligrosas, no solo paths.
    AMARILLA = modifica archivo existente (reversible); VERDE = archivo nuevo aislado."""
    tgt = change.target.replace("\\", "/")
    if tgt.startswith("/") or tgt.startswith("..") or ":" in tgt:
        return "red"
    if any(tgt == r or tgt.endswith("/" + r) for r in _RED_PATHS):
        return "red"
    # delta: solo firmas NUEVAS vs el archivo previo -> un refactor que preserva firmas
    # pre-existentes (regex, docs) no se auto-bloquea; capacidad peligrosa NUEVA sí.
    if red_content_hits(change.after, baseline=change.before):
        return "red"
    return "yellow" if _existed_before(change) else "green"


def self_evolve(*, candidates: list[Change] | None = None, generate_fn=None, n: int = 3,
                root: Path = ROOT, evaluate_fn=None, do_apply: bool = False,
                audit: bool = True) -> dict:
    """Motor de auto-evolución (1 mejora segura por ciclo). Pasos:
      IDEATE: usa `candidates` o `generate_fn() -> list[Change]`.
      FITNESS: `evaluate()` cada uno (inyectable vía evaluate_fn para tests).
      TOURNAMENT: entre los que pasan, gana el de más checks ok (desempate: id).
      ZONA: roja -> STOP (nunca aplica, gate humano). verde/amarilla -> aplica si do_apply.
      AUDIT: archive + episodio kind="auto_action". LEARN: record_outcome.
    Devuelve {evaluated, winner, applied, zone, blocked_red}. NO aplica rojo jamás."""
    ev = evaluate_fn or evaluate
    cands = candidates if candidates is not None else (generate_fn() if generate_fn else [])
    results: list[dict[str, Any]] = []
    for c in cands:
        z = zone_of(c, root=root)
        r = ev(c)
        results.append({"change": c, "zone": z, "eval": r, "ok": bool(r.get("ok"))})

    passing = [x for x in results if x["ok"] and x["zone"] != "red"]
    blocked_red = [x for x in results if x["zone"] == "red"]
    # tournament: más checks ok gana (proxy de "mejor"); determinista por id
    winner = max(passing, key=lambda x: (sum(x["eval"]["checks"].values()), x["change"].id),
                 default=None)

    applied = False
    if winner and do_apply and winner["zone"] in ("green", "yellow"):
        # defense-in-depth (B1): re-chequear tamper-halt JUSTO antes de mutar el repo, aunque
        # evaluate ya lo corrio — el apply es el momento irreversible. GoalTampered propaga.
        from .goal import goal_guard as _goal_guard
        _goal_guard()
        apply_change(winner["change"], root=root)
        applied = True

    if audit:
        for x in results:
            c = x["change"]
            archive_variant(c.id, x["eval"], notes=f"zone={x['zone']} ok={x['ok']}",
                            applied=(applied and winner is c))
            if x is winner and applied:
                _audit_episode(c, x["zone"], x["eval"])
        try:
            from .feedback import record_outcome
            for x in results:
                record_outcome(f"evolve:{x['zone']}", 1.0 if x["ok"] else 0.0,
                               pattern="evolve", source="self_evolve", context=x["change"].target)
        except Exception:
            pass

    return {"evaluated": len(results), "winner": winner["change"].id if winner else None,
            "applied": applied, "zone": winner["zone"] if winner else None,
            "blocked_red": [x["change"].id for x in blocked_red], "results": results}


# --------------------------------------------------------------------------- #
# Sandbox por BRANCH (git worktree) — aislamiento real, no muta el repo vivo    #
# --------------------------------------------------------------------------- #
def _git(*args, cwd) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=str(cwd), capture_output=True, text=True)


def sandbox_branch(change: Change, *, root: Path = ROOT, base: str = "HEAD",
                   run_tests: bool = True, test_path: str = "tests",
                   test_cmd: list[str] | None = None, keep_on_pass: bool = True,
                   timeout: int = 1800) -> dict:
    """Aísla en un git WORKTREE sobre una branch nueva `mmorch-sbx-<id>` (desde HEAD, NO
    incluye cambios sin commitear del árbol principal → no interfiere). Aplica el cambio,
    commitea, corre tests AHÍ. Verde → branch QUEDA (pa merge/PR). Rojo → branch borrada.
    El repo vivo NUNCA se toca. Devuelve {ok, branch, fitness, change_id}."""
    import tempfile
    bname = f"mmorch-sbx-{change.id}"
    wt = tempfile.mkdtemp(prefix="mmorch_wt_")
    _git("worktree", "remove", "--force", wt, cwd=root)         # limpiar stale
    _git("branch", "-D", bname, cwd=root)
    add = _git("worktree", "add", "-b", bname, wt, base, cwd=root)
    if add.returncode != 0:
        return {"ok": False, "error": add.stderr[:200], "branch": None, "change_id": change.id}
    fit, ok = {}, True
    try:
        tgt = Path(wt) / change.target
        tgt.parent.mkdir(parents=True, exist_ok=True)
        tgt.write_text(change.after, encoding="utf-8")
        _git("add", "-A", cwd=wt)
        _git("commit", "-m", f"sandbox {change.id}: {change.description[:60]}",
             "--no-verify", cwd=wt)
        if run_tests:
            cmd = test_cmd or [sys.executable, "-m", "pytest", test_path, "-q", "--no-header"]
            proc = subprocess.run(cmd, cwd=wt, capture_output=True, text=True, timeout=timeout)
            out = (proc.stdout or "") + (proc.stderr or "")
            fit = {"passed": _count(out, r"(\d+) passed"), "failed": _count(out, r"(\d+) failed"),
                   "rc": proc.returncode}
            ok = proc.returncode == 0 and fit["failed"] == 0
    finally:
        _git("worktree", "remove", "--force", wt, cwd=root)
    if ok and keep_on_pass:
        return {"ok": True, "branch": bname, "fitness": fit, "change_id": change.id}
    _git("branch", "-D", bname, cwd=root)
    return {"ok": ok, "branch": None, "fitness": fit, "change_id": change.id}


def promote_branch(branch: str, *, root: Path = ROOT, ff_only: bool = True) -> dict:
    """Mergea la branch sandbox a la actual. ff_only por default (no crea merge-commits
    raros). Esto es la PROMOCIÓN del pipeline 'sandbox→merge' (zona amarilla)."""
    args = ["merge", "--ff-only" if ff_only else "--no-ff", branch]
    r = _git(*args, cwd=root)
    return {"merged": r.returncode == 0, "detail": (r.stdout + r.stderr)[:300]}


def open_pr_branch(branch: str, *, title: str, body: str = "", root: Path = ROOT) -> dict:
    """Abre un PR de la branch sandbox vía `gh` (si está). Alternativa a merge directo
    cuando querés revisión humana (zona amarilla con gate). gh ausente → devuelve push-only."""
    push = _git("push", "-u", "origin", branch, cwd=root)
    try:
        gh = subprocess.run(["gh", "pr", "create", "--head", branch, "--title", title,
                             "--body", body or title], cwd=str(root),
                            capture_output=True, text=True)
        pr_created, detail = gh.returncode == 0, (gh.stdout + gh.stderr)[:300]
    except FileNotFoundError:
        # el docstring siempre prometio push-only sin gh; el crash rompia el
        # nightly entero (FileNotFoundError medido, corrida 2026-08-15)
        pr_created, detail = False, "gh no instalado -> push-only"
    return {"pushed": push.returncode == 0, "pr_created": pr_created,
            "detail": detail}


def _audit_episode(change: Change, zone: str, ev: dict) -> None:
    """Auditoría inmutable de la auto-acción (mejora #5 del usuario)."""
    try:
        from .memory import write_episode
        write_episode("mmorch_self", "auto_action", {
            "change_id": change.id, "target": change.target, "zone": zone,
            "checks": ev.get("checks"), "description": change.description})
    except Exception:
        pass


# --------------------------------------------------------------------------- #
# Coordinacion nocturna (usuario 2026-07): auto-correr el loop overnight es seguro
# (siempre worktree-aislado), auto-MERGEAR no (un test verde es proxy, no prueba —
# mutation_score/F4 lo midieron esta misma sesion) -> auto-run + auto-PR, merge manual.
#
# El riesgo nuevo que introduce correr VARIAS rondas overnight: dos rondas tocando el
# MISMO target_file terminarian con 2 branches que van a competir/conflictuar cuando
# alguna se mergee. Fix: LOCK por archivo. Mientras el archivo tenga un PR trackeado
# ABIERTO, ninguna ronda nueva genera un branch competidor para ese archivo -> se
# saltea y reintenta en la proxima ronda. Cuando el humano mergea/cierra ese PR, el
# archivo queda libre automaticamente (reap_merged_prs corre al principio de cada ronda).
# Alternativa descartada: forzar mas commits sobre el MISMO PR abierto (mas trabajo, y
# un force-push puede pisar contexto de una revision humana en curso).
# --------------------------------------------------------------------------- #
_PR_STATE = ROOT / "logs" / "evolve_open_prs.json"


def _load_pr_state(path: Path = _PR_STATE) -> dict:
    # NO tragar la corrupcion en silencio: un JSON truncado (crash nocturno mid-write,
    # ahora mitigado por el write atomico de _save_pr_state) hacia desaparecer los locks
    # por archivo -> coordinated_evolve_round podia abrir un branch competidor.
    return load_json_tolerant(path, {}, what="evolve_open_prs.json (PR locks)")


def _save_pr_state(state: dict, path: Path = _PR_STATE) -> None:
    atomic_write_json(path, state, indent=1)


def _pr_still_open(entry: dict, *, root: Path, gh_check_fn=None) -> bool:
    """Un PR trackeado sigue 'abierto' (bloqueando un branch nuevo) si su branch existe Y
    (no hay pr_number registrado, O gh dice que sigue abierto). gh ausente/falla -> el
    default es 'sigue abierto' (falla SEGURO: nunca pisa un merge que no pudo confirmar)."""
    branch = entry.get("branch")
    if not branch:
        return False
    if _git("rev-parse", "--verify", branch, cwd=root).returncode != 0:
        return False
    pr_num = entry.get("pr_number")
    if pr_num and gh_check_fn:
        try:
            return gh_check_fn(pr_num) == "OPEN"
        except Exception:
            pass
    return True


def reap_merged_prs(*, root: Path = ROOT, gh_check_fn=None, path: Path = _PR_STATE) -> dict:
    """Corre al empezar cada ronda: libera archivos cuyo PR trackeado ya se mergeo/cerro
    (la branch ya no existe, o gh dice closed/merged). Solo toca el tracking LOCAL —
    nunca borra nada de git. Devuelve {freed:[...], still_open:[...]}."""
    state = _load_pr_state(path)
    freed: list[str] = []
    merged: list[str] = []
    rejected: list[str] = []
    for target, entry in list(state.items()):
        if not _pr_still_open(entry, root=root, gh_check_fn=gh_check_fn):
            freed.append(target)
            # POST-MERGE OUTCOME (blind-spot #1 del audit: el loop aprendía del gate, nunca
            # del veredicto humano final). branch cerrada: si su commit es alcanzable desde
            # HEAD -> MERGEADO (reward 1.0); si no -> rechazado/descartado (0.0). Señal más
            # valiosa que el gate: es el juicio del humano sobre el trabajo completo.
            was_merged = False
            sha = entry.get("head_sha")
            if sha:
                was_merged = _git("merge-base", "--is-ancestor", sha, "HEAD",
                                  cwd=root).returncode == 0
            (merged if was_merged else rejected).append(target)
            try:
                from .feedback import record_outcome
                record_outcome("evolve:nightly", 1.0 if was_merged else 0.0,
                               pattern="evolve_pr", source="human_merge", context=target)
            except Exception:
                pass
            del state[target]
    _save_pr_state(state, path)
    return {"freed": freed, "merged": merged, "rejected": rejected,
            "still_open": list(state.keys())}


def coordinated_evolve_round(candidates: list[Change], *, root: Path = ROOT,
                             sandbox_fn=None, pr_fn=None, gh_check_fn=None,
                             pr_title_fn=None, open_pr: bool = True,
                             path: Path = _PR_STATE) -> dict:
    """1 ronda del loop nocturno, coordinada por archivo. `sandbox_fn`/`pr_fn` inyectables
    (default = sandbox_branch/open_pr_branch reales; seam de test sin git/gh real).

    1. reap_merged_prs(): libera archivos cuyo PR anterior ya se cerro.
    2. Por candidate: target_file YA con PR abierto -> SKIP esta ronda (no crea un branch
       competidor; se reintenta la proxima ronda, para entonces puede estar libre). Sin PR
       abierto -> zona roja bloquea siempre; verde/amarilla -> sandbox+test; verde -> abre
       PR + trackea; rojo/fitness-fail -> no trackea nada (proximo intento arranca limpio).

    Devuelve {skipped_active_pr, opened, red, blocked_zone_red}."""
    sandbox_fn = sandbox_fn or (lambda c: sandbox_branch(c, root=root))
    pr_fn = pr_fn or (lambda branch, title: open_pr_branch(branch, title=title, root=root))
    reap_merged_prs(root=root, gh_check_fn=gh_check_fn, path=path)
    state = _load_pr_state(path)
    skipped, opened, red, blocked_zone_red = [], [], [], []
    for c in candidates:
        if c.target in state:
            skipped.append(c.target)
            continue
        if zone_of(c, root=root) == "red":
            blocked_zone_red.append(c.target)
            continue
        r = sandbox_fn(c)
        if not r.get("ok"):
            red.append(c.target)
            continue
        entry = {"branch": r["branch"], "target": c.target, "change_id": c.id}
        head = _git("rev-parse", r["branch"], cwd=root)
        if head.returncode == 0:                      # pa distinguir merge de rechazo al reapear
            entry["head_sha"] = head.stdout.strip()
        if open_pr:
            title = pr_title_fn(c) if pr_title_fn else f"auto-evolve: {c.description[:60]}"
            pr = pr_fn(r["branch"], title)
            entry["pr_pushed"] = pr.get("pushed")
            entry["pr_number"] = pr.get("pr_number")
        state[c.target] = entry
        opened.append(c.target)
    _save_pr_state(state, path)
    return {"skipped_active_pr": skipped, "opened": opened, "red": red,
            "blocked_zone_red": blocked_zone_red}


def propose_patch(target_file: str, finding: str, *, gen_model: str | None = None) -> str:
    """Un modelo barato PROPONE el contenido nuevo de target_file para resolver
    `finding`. READ-ONLY: devuelve el texto, NO escribe nada. Aplicar = gate aparte.
    """
    from .patterns import fan_out
    from .config import DEFAULT_GENERATOR
    src = (ROOT / target_file).read_text(encoding="utf-8") if (ROOT / target_file).exists() else ""
    prompt = (
        f"Sos un mejorador de codigo Python. Resolve este hallazgo SIN romper la API publica "
        f"ni los invariantes (cross-family, OneFlow, anti-sicofancia, observabilidad).\n\n"
        f"HALLAZGO: {finding}\n\nARCHIVO {target_file}:\n{src}\n\n"
        f"Devolve el CONTENIDO COMPLETO nuevo del archivo, sin explicacion, en un bloque de codigo.")
    return fan_out([prompt], gen_model=gen_model or DEFAULT_GENERATOR, phase="evolve")[0].text


# --------------------------------------------------------------------------- #
# Loop nocturno end-to-end (pedido usuario 2026-07): cosecha -> propone -> sandbox+PR,
# coordinado. Entry point unico para el scheduled-task.
# --------------------------------------------------------------------------- #
def nightly_evolve(*, days: int = 3, max_files: int = 5, max_findings: int = 8,
                   root: Path = ROOT, harvest_fn=None, propose_fn=None,
                   **round_kwargs) -> dict:
    """1 corrida nocturna completa: harvest_findings() (code_review real sobre archivos
    recientemente cambiados) -> por cada hallazgo, propose_patch() genera el cambio ->
    coordinated_evolve_round() lo sandboxea + testea + abre PR (o saltea si el archivo ya
    tiene un PR pendiente). `harvest_fn`/`propose_fn` inyectables (self-check cero-API).
    Sin hallazgos -> no-op limpio (nunca genera ruido de PRs vacíos)."""
    if harvest_fn is None:
        from .evolve_findings import harvest_findings

        def harvest_fn():
            return harvest_findings(days=days, max_files=max_files,
                                    max_findings=max_findings, root=root)
    propose_fn = propose_fn or propose_patch

    findings = harvest_fn()
    if not findings:
        return {"findings": 0, "skipped_active_pr": [], "opened": [], "red": [],
                "blocked_zone_red": []}
    candidates = []
    for f in findings:
        try:
            after = propose_fn(f["target"], f["finding"])
        except Exception:
            continue   # un hallazgo que no se pudo proponer no debe frenar el resto
        candidates.append(snapshot_change(f["target"], after, f["finding"][:80], root=root))
    result = coordinated_evolve_round(candidates, root=root, **round_kwargs)
    result["findings"] = len(findings)
    return result


if __name__ == "__main__":
    import tempfile

    # cero-git/gh: sandbox_fn/pr_fn/gh_check_fn inyectados (prueba la COORDINACION, no git real)
    tmp_state = Path(tempfile.mkdtemp()) / "pr_state.json"
    calls: list = []

    def _fake_sandbox_ok(c):
        calls.append(("sandbox", c.target))
        return {"ok": True, "branch": f"mmorch-sbx-{c.id}", "fitness": {}, "change_id": c.id}

    def _fake_sandbox_fail(c):
        calls.append(("sandbox", c.target))
        return {"ok": False, "branch": None, "fitness": {}, "change_id": c.id}

    def _fake_pr(branch, title):
        calls.append(("pr", branch))
        return {"pushed": True, "pr_created": True, "pr_number": 42}

    def _fake_git_exists_true(*a, cwd):
        class _R:
            returncode = 0
            stdout = "deadbeef123\n"
        return _R()

    def _fake_git_exists_false(*a, cwd):
        class _R:
            returncode = 1
            stdout = ""
        return _R()

    c1 = snapshot_change("a.py", "def a(): return 1", "fix a")
    c2 = snapshot_change("a.py", "def a(): return 2", "fix a, take 2")  # MISMO target que c1
    c3 = snapshot_change("b.py", "def b(): return 1", "fix b")

    # 1. ronda 1: 2 candidatos, uno para 'a.py' otro para 'b.py' -> ambos abren PR
    _git = globals()["_git"]
    globals()["_git"] = _fake_git_exists_true   # branch existe (recien creada)
    r1 = coordinated_evolve_round([c1, c3], sandbox_fn=_fake_sandbox_ok, pr_fn=_fake_pr,
                                  path=tmp_state)
    assert set(r1["opened"]) == {"a.py", "b.py"}, r1
    assert r1["skipped_active_pr"] == [], r1

    # 2. ronda 2: un candidato NUEVO para 'a.py' (mismo target que c1) -> SKIP, no compite
    calls.clear()
    r2 = coordinated_evolve_round([c2], sandbox_fn=_fake_sandbox_ok, pr_fn=_fake_pr,
                                  path=tmp_state)
    assert r2["skipped_active_pr"] == ["a.py"], r2
    assert calls == [], "no debio llamar sandbox_fn para un archivo con PR abierto"

    # 3. el PR de 'a.py' se mergea (la branch ya no existe) -> reap la libera -> ronda 3 la toma
    globals()["_git"] = _fake_git_exists_false
    r3 = coordinated_evolve_round([c2], sandbox_fn=_fake_sandbox_ok, pr_fn=_fake_pr,
                                  path=tmp_state)
    assert r3["opened"] == ["a.py"], r3          # liberada y re-tomada en la MISMA ronda

    # 4. sandbox que falla (rojo) -> nunca se trackea, no bloquea futuras rondas
    globals()["_git"] = _fake_git_exists_true
    _save_pr_state({}, tmp_state)
    r4 = coordinated_evolve_round([c1], sandbox_fn=_fake_sandbox_fail, pr_fn=_fake_pr,
                                  path=tmp_state)
    assert r4["red"] == ["a.py"] and r4["opened"] == [], r4
    assert _load_pr_state(tmp_state) == {}, "un intento fallido no debe quedar trackeado"

    globals()["_git"] = _git   # restaurar
    print("evolve coordination OK — lock por archivo, reap libera al mergear, rojo no trackea")

    # --- nightly_evolve: harvest/propose/round todos inyectados, cero-API/cero-git ---
    tmp_state2 = Path(tempfile.mkdtemp()) / "pr_state.json"
    globals()["_git"] = _fake_git_exists_true

    def _fake_harvest_some():
        return [{"target": "x.py", "severity": "high", "finding": "algo mal en x"},
                {"target": "y.py", "severity": "low", "finding": "algo mal en y"}]

    def _fake_propose(target, finding):
        return f"# fix aplicado a {target}\ndef f(): return 1"

    r_night = nightly_evolve(harvest_fn=_fake_harvest_some, propose_fn=_fake_propose,
                             root=Path(tempfile.mkdtemp()), sandbox_fn=_fake_sandbox_ok,
                             pr_fn=_fake_pr, path=tmp_state2)
    assert r_night["findings"] == 2, r_night
    assert set(r_night["opened"]) == {"x.py", "y.py"}, r_night

    # sin hallazgos -> no-op limpio, nunca llama propose/round
    def _fake_harvest_empty():
        return []
    r_empty = nightly_evolve(harvest_fn=_fake_harvest_empty,
                             propose_fn=lambda t, f: (_ for _ in ()).throw(
                                 AssertionError("no debio proponerse nada")))
    assert r_empty == {"findings": 0, "skipped_active_pr": [], "opened": [], "red": [],
                       "blocked_zone_red": []}, r_empty

    # un propose_fn que explota para UN finding no frena el resto
    def _propose_one_boom(target, finding):
        if target == "x.py":
            raise RuntimeError("modelo caido")
        return "def g(): return 2"
    r_partial = nightly_evolve(harvest_fn=_fake_harvest_some, propose_fn=_propose_one_boom,
                               root=Path(tempfile.mkdtemp()), sandbox_fn=_fake_sandbox_ok,
                               pr_fn=_fake_pr, path=Path(tempfile.mkdtemp()) / "s.json")
    assert r_partial["opened"] == ["y.py"], r_partial   # x.py se perdio, y.py sigue

    globals()["_git"] = _git
    print("nightly_evolve OK — harvest->propose->round encadenado, no-op limpio, resiliente")

    # --- delta red-scan: pre-existente NO bloquea, capacidad NUEVA SÍ (fix auto-lock 2026-07) ---
    _selfish = 'import os\nRX = "subprocess.run|os.remove"   # firmas como STRING, igual que evolve.py\n'
    c_refactor = Change("evolve.py", _selfish + "\ndef mejor(): return 1", _selfish, "refactor")
    assert zone_of(c_refactor) != "red", "un refactor que preserva firmas pre-existentes NO debe ser rojo"
    c_newdanger = Change("x.py", _selfish + "\nos.system(payload)\n", _selfish, "agrega os.system real")
    assert zone_of(c_newdanger) == "red", "capacidad peligrosa NUEVA (os.system no estaba antes) SÍ es roja"
    c_fresh = Change("nuevo.py", "eval(user_input)\n", "", "archivo nuevo con eval")
    assert zone_of(c_fresh) == "red", "archivo nuevo (before vacío) con eval = todo es nuevo = rojo"
    print("delta red-scan OK — self-lock roto sin apagar el escudo (firma nueva sigue bloqueando)")
