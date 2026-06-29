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
import re
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

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


def fitness(test_path: str = "tests", timeout: int = 300) -> dict:
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
    if not _ARCHIVE.exists():
        return []
    return [json.loads(ln) for ln in _ARCHIVE.read_text(encoding="utf-8").splitlines() if ln.strip()]


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


def red_content_hits(text: str) -> list[str]:
    """Firmas de acción zona-roja encontradas en el contenido (vacío = limpio)."""
    return sorted(set(m.group(0) for m in _RED_CONTENT.finditer(text or "")))


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
    if red_content_hits(change.after):              # acción peligrosa en el código generado
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
    results = []
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
                   timeout: int = 300) -> dict:
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
    gh = subprocess.run(["gh", "pr", "create", "--head", branch, "--title", title,
                         "--body", body or title], cwd=str(root), capture_output=True, text=True)
    return {"pushed": push.returncode == 0, "pr_created": gh.returncode == 0,
            "detail": (gh.stdout + gh.stderr)[:300]}


def _audit_episode(change: Change, zone: str, ev: dict) -> None:
    """Auditoría inmutable de la auto-acción (mejora #5 del usuario)."""
    try:
        from .memory import write_episode
        write_episode("mmorch_self", "auto_action", {
            "change_id": change.id, "target": change.target, "zone": zone,
            "checks": ev.get("checks"), "description": change.description})
    except Exception:
        pass


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
