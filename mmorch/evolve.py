"""evolve — subset DGM-inspirado, GATED (research: vault/research/
darwin-godel-machine-self-improving-agents). La critica cross-family marco que el
DGM completo (evolucion poblacional open-ended + auto-modificacion) es overreach
para mmorch. Aca solo el subset seguro:

- evaluate(): fitness compuesta (ast + ensemble + costo/budget + goal_aligned),
  cableada como gate pre-PR del loop nocturno (W4.3).
- propose_patch(): un modelo barato PROPONE un cambio (read-only, NO lo aplica).
- sandbox_branch(): tests reales en git worktree aislado; la reversibilidad la
  da git (branch + revert del carril automerge), no un snapshot estructural.

NUNCA auto-modifica vivo. Aplicar un patch = sandbox + gates verdes + gate humano.
(El motor self_evolve/promote/archive de F3-F4 se borro en W4.3: era museo —
implementado y testeado con cero callers vivos; 00-canonical-matrix §4.)
"""
from __future__ import annotations

import hashlib
import json
import logging
import math
import re
import subprocess
import sys
import time
from collections import Counter
from dataclasses import dataclass
from pathlib import Path

from . import paths

from .iohelpers import atomic_write_json, load_json_tolerant

_log = logging.getLogger(__name__)

ROOT = Path(__file__).resolve().parent.parent


# --------------------------------------------------------------------------- #
# Change + evaluate() (fitness compuesta)                                      #
# --------------------------------------------------------------------------- #
@dataclass
class Change:
    """Un cambio candidato. `before` = snapshot previo: alimenta el delta del
    red-scan (zone_of) y el diff que juzga el gate pre-PR."""
    target: str            # path relativo a root
    after: str             # contenido nuevo
    before: str            # snapshot previo (baseline del delta red-scan y del diff)
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


def _existed_before(change: Change) -> bool:
    return change.before != ""


def evaluate(change: Change, *,
             goal: bool = True, goal_fn=None,
             check_cost: bool = True, cost_fn=None,
             check_ensemble: bool = True, ensemble_fn=None) -> dict:
    """fitness() compuesta — cableada al camino VIVO como gate pre-PR de
    coordinated_evolve_round (W4.3; antes solo la corria mmorch_evolve_self en DRY).
    Cualquier check que falle aborta:
      1. ast_valid del contenido nuevo (checker determinista).
      2. ensemble cross-family (escéptico de seguridad/no-regresión). Inyectable.
      3. cost_ok: no-degradación de costo (inyectable) + budget_ok (BudgetKeeper absoluto).
      4. goal_aligned(description) — alineación con GOAL (cross-family). Inyectable.
    Tests reales y reversibilidad NO viven acá: los garantiza git (sandbox_branch en
    worktree + revert del carril automerge, W4.2). El rollback estructural por snapshot
    y el check tests_green in-place se borraron (museo, 00-canonical-matrix §4).
    Devuelve {ok, checks, ensemble_degraded}. ensemble_degraded=True si hoy solo hay 1
    familia de verificador (Kimi inactivo) → el ensemble-AZUL de 2 cae al cross-family
    simple (honesto, no se finge)."""
    from .checkers import check as _check
    checks: dict[str, bool] = {}
    ensemble_degraded = False

    checks["ast_valid"] = _check("python_ast_valid", code=change.after).passed

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

    return {"ok": all(checks.values()), "checks": checks, "change_id": change.id,
            "ensemble_degraded": ensemble_degraded}


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


def _count(text: str, pat: str) -> int:
    # la ULTIMA aparicion, no la primera: la linea de resumen de pytest va al
    # final, y cualquier "1 failed" impreso ANTES por la salida capturada de un
    # test mentia el conteo (medido: sandbox con 9 failed registrado como 1).
    ms = re.findall(pat, text)
    return int(ms[-1]) if ms else 0


# --------------------------------------------------------------------------- #
# Zona roja (paths + contenido) — el semáforo que comparte todo el pipeline     #
# --------------------------------------------------------------------------- #
# Zona ROJA por path: nunca auto-aplicar (gate humano). Coincide con GOAL.md cat.4.
# W4.1 (research 07 #1): el harness de EVALUACIÓN también es zona roja — un pipeline que
# puede editar sus propios gates (gates/smoke, los tests del fitness, esta misma allowlist)
# puede aflojarlos para pasar. Por eso mmorch/evolve.py (contiene _RED_PATHS y zone_of)
# está acá adentro: la lista se protege a sí misma.
_RED_PATHS = ("GOAL.md", "GOAL.hash", ".env", "mmorch/goal.py", "mmorch/budget.py",
              "mmorch/config.py", "mmorch/evolve.py", "scripts/gates.py",
              "scripts/smoke.py", "tests/test_evolve_motor.py", "tests/test_goal.py",
              "tests/test_evolve_goal_guard.py")


# Firmas de ACCIONES zona-roja en el CONTENIDO generado (no solo el path): un cambio de
# código que INTRODUCE estas capacidades es rojo aunque el archivo sea nuevo/aislado.
_RED_CONTENT = re.compile(
    r"\b(os\.system|subprocess\.(?:run|Popen|call)|shutil\.rmtree|os\.remove|os\.unlink|"
    r"\beval\s*\(|\bexec\s*\(|__import__|rm\s+-rf|DROP\s+TABLE|TRUNCATE|"
    r"requests\.(?:post|put|delete|patch)|socket\.|"
    r"transfer|withdraw|wallet|exchange|stripe|paypal|place_order|send_money)\b", re.I)


# Palabras de CREDENCIAL separadas de las ACCIONES: su mera presencia en identificadores,
# tests o docs no es peligrosa (falso rojo medido: fixtures con "password"/"secret_key"
# bloqueaban merges verdes legitimos — defecto 05 #7). Lo rojo es un VALOR real asignado:
# `password = "<literal con entropia>"`. Conservador a proposito: umbrales bajos, ante
# duda rojo; solo la palabra suelta o un placeholder corto dejan de bloquear.
# \w* a ambos lados: los \b no cortan en underscore, asi que un identificador ENV-style
# (AWS_SECRET_ACCESS_KEY = "...") NO matcheaba y la clave literal pasaba el gate de zona
# (AT-26 defecto #1). El anti-falso-rojo sigue siendo el filtro de VALOR en _secret_hits
# (entropia/forma), no la angostura del identificador.
_SECRET_ASSIGN = re.compile(
    r"\b\w*(?:password|passwd|secret|private_key|api_key|access_key|access_token|"
    r"auth_token|seed_phrase)\w*\s*[:=]\s*[\"']([^\"']{8,})[\"']", re.I)


def _char_entropy(s: str) -> float:
    """Entropia de Shannon por caracter (bits) — proxy barato de 'parece clave real'."""
    if not s:
        return 0.0
    counts = Counter(s)
    n = len(s)
    return -sum(c / n * math.log2(c / n) for c in counts.values())


def _secret_hits(text: str) -> set[str]:
    """Asignaciones de credencial cuyo VALOR parece secreto real: literal largo con
    entropia alta, o >=20 chars con forma hex/base64 (aunque la entropia sea baja).
    El hit incluye el valor -> el delta vs baseline funciona igual que con acciones."""
    hits = set()
    for m in _SECRET_ASSIGN.finditer(text or ""):
        val = m.group(1)
        if (len(val) >= 12 and _char_entropy(val) >= 3.0) or (
                len(val) >= 20 and re.fullmatch(r"[A-Za-z0-9+/=_\-]+", val)):
            hits.add(m.group(0))
    return hits


def red_content_hits(text: str, *, baseline: str | None = None) -> list[str]:
    """Firmas de acción zona-roja en el contenido (vacío = limpio). Con `baseline`, devuelve
    solo las firmas NUEVAS (delta): las que ya estaban en baseline no cuentan. Esto arregla el
    auto-lock medido (2026-07: evolve.py CONTIENE la regex de firmas como strings -> se marcaba
    rojo a sí mismo). El escudo real queda: un self-edit que INTRODUCE una capacidad peligrosa
    nueva (que no estaba en baseline) sigue siendo rojo. No es caso-especial de 'self' —
    principiado, aplica a cualquier archivo que ya mencione firmas legítimamente (regex, docs)."""
    hits = set(m.group(0) for m in _RED_CONTENT.finditer(text or "")) | _secret_hits(text or "")
    if baseline is not None:
        hits -= set(m.group(0) for m in _RED_CONTENT.finditer(baseline)) | _secret_hits(baseline)
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


# --------------------------------------------------------------------------- #
# Sandbox por BRANCH (git worktree) — aislamiento real, no muta el repo vivo    #
# --------------------------------------------------------------------------- #
def _git(*args, cwd) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *args], cwd=str(cwd), capture_output=True, text=True)


def sandbox_branch(change: Change, *, root: Path = ROOT, base: str = "HEAD",
                   run_tests: bool = True, test_path: str = "tests",
                   test_cmd: list[str] | None = None, keep_on_pass: bool = True,
                   timeout: int = 1800, origin: str = "evolve") -> dict:
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
            # --basetemp propio: el pytest-current global del user esta roto de
            # permisos (medido 2026-08: TODOS los sandboxes daban rojo por el
            # cleanup, no por los tests -> 0 PRs abiertos durante semanas). Se
            # agrega SIEMPRE, tambien si test_cmd viene inyectado (bug real
            # encontrado escribiendo propose_with_fast_retry: un test_cmd
            # custom se saltaba el basetemp por completo)
            bt = tempfile.mkdtemp(prefix="mmorch_bt_")
            cmd = list(test_cmd) if test_cmd else [
                sys.executable, "-m", "pytest", test_path, "-q", "--no-header"]
            if not any("--basetemp" in c for c in cmd):
                cmd.append(f"--basetemp={bt}")
            proc = subprocess.run(cmd, cwd=wt, capture_output=True, text=True, timeout=timeout)
            out = (proc.stdout or "") + (proc.stderr or "")
            fit = {"passed": _count(out, r"(\d+) passed"), "failed": _count(out, r"(\d+) failed"),
                   "rc": proc.returncode, "detail": out[-1200:]}
            ok = proc.returncode == 0 and fit["failed"] == 0
    finally:
        _git("worktree", "remove", "--force", wt, cwd=root)
    if ok and keep_on_pass:
        # provenance: la branch nace atribuida a su brazo — al mergearse (o
        # expirar) el bandit recibe el outcome retroactivo sin ningun clic
        # extra del humano. Fail-soft: un ledger roto no frena el sandbox.
        try:
            from .config import DEFAULT_GENERATOR
            from .provenance import record_branch
            record_branch(bname, arm=f"{DEFAULT_GENERATOR}#{origin}",
                          origin=origin, target=change.target,
                          logs_dir=str(root / "logs"))
        except Exception:
            pass
        return {"ok": True, "branch": bname, "fitness": fit, "change_id": change.id}
    _git("branch", "-D", bname, cwd=root)
    return {"ok": ok, "branch": None, "fitness": fit, "change_id": change.id}


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
_PR_STATE = paths.logs_dir() / "evolve_open_prs.json"


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
    # el trabajo YA esta en HEAD (lo mergeo el tren o el humano) -> el lock esta
    # muerto aunque la branch siga existiendo. Sin esto, con gh ausente (pr_number
    # None) y nadie borrando branches sandbox, el lock era PERMANENTE: medido
    # 2026-08-24, 4 archivos bloqueados y 6 de 8 hallazgos salteados por noche.
    sha = entry.get("head_sha")
    if sha and _git("merge-base", "--is-ancestor", sha, "HEAD",
                    cwd=root).returncode == 0:
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


def _diff_goal_aligned(c: Change):
    """goal_aligned() sobre el DIFF del candidato (no el archivo entero: el verificador
    juzga QUÉ cambió, con menos ruido y menos tokens). W4.1: el ancla anti-drift pasa
    de museo (solo evaluate(), que nadie llama) al camino vivo pre-PR."""
    import difflib
    from .goal import goal_aligned
    diff = "".join(difflib.unified_diff(
        c.before.splitlines(keepends=True), c.after.splitlines(keepends=True),
        fromfile=f"a/{c.target}", tofile=f"b/{c.target}"))
    return goal_aligned(f"{c.description}\n\n{diff[:8000]}", phase="evolve_pr_gate")


def _pr_fitness(c: Change) -> dict:
    """W4.3: evaluate() (la fitness compuesta, museo desde F3) entra al camino vivo
    como check pre-PR, SIN duplicar lo que la ronda ya paga: goal_aligned lo cubre
    aligned_fn (W4.1, sobre el diff) => goal=False; los tests reales los corrió
    sandbox_branch. El costo RELATIVO no se mide en el nightly (no hay cost_fn con
    medición) => no gatea; budget_ok (absoluto, BudgetKeeper) sí."""
    return evaluate(c, goal=False, cost_fn=lambda ch: True)


def coordinated_evolve_round(candidates: list[Change], *, root: Path = ROOT,
                             sandbox_fn=None, pr_fn=None, gh_check_fn=None,
                             pr_title_fn=None, open_pr: bool = True,
                             aligned_fn=None, fitness_fn=None,
                             path: Path = _PR_STATE) -> dict:
    """1 ronda del loop nocturno, coordinada por archivo. `sandbox_fn`/`pr_fn` inyectables
    (default = sandbox_branch/open_pr_branch reales; seam de test sin git/gh real).

    1. reap_merged_prs(): libera archivos cuyo PR anterior ya se cerro.
    2. Por candidate: target_file YA con PR abierto -> SKIP esta ronda (no crea un branch
       competidor; se reintenta la proxima ronda, para entonces puede estar libre). Sin PR
       abierto -> zona roja bloquea siempre; verde/amarilla -> sandbox+test; verde -> abre
       PR + trackea; rojo/fitness-fail -> no trackea nada (proximo intento arranca limpio).

    W4.1: antes de abrir PR, `aligned_fn` (default goal_aligned sobre el diff) tiene que
    pasar — desalineado con GOAL.md => sin PR + motivo a evolve_red.jsonl. Error de infra
    del check => fail-OPEN (se abre el PR igual: el gate humano del PR sigue; CLOSED solo
    ante refutación explícita — mismo principio que el never-edit guard).

    W4.3: `fitness_fn` (default _pr_fitness = evaluate() sin goal/tests) es el segundo
    gate pre-PR: ast + ensemble cross-family + budget. Falla explícita => sin PR +
    checks a evolve_red.jsonl; error de infra => fail-OPEN, igual que aligned_fn.

    Devuelve {skipped_active_pr, opened, red, blocked_zone_red, blocked_fitness,
    blocked_goal}."""
    sandbox_fn = sandbox_fn or (lambda c: sandbox_branch(c, root=root))
    pr_fn = pr_fn or (lambda branch, title: open_pr_branch(branch, title=title, root=root))
    aligned_fn = aligned_fn or _diff_goal_aligned
    fitness_fn = fitness_fn or _pr_fitness
    reap_merged_prs(root=root, gh_check_fn=gh_check_fn, path=path)
    state = _load_pr_state(path)
    skipped, opened, red, blocked_zone_red, blocked_goal = [], [], [], [], []
    blocked_fitness: list[str] = []
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
            # persistir el POR QUE: fitness.detail se calculaba y se tiraba —
            # medido 2026-08-21: el stuck-finding le pedia al reparador "lee
            # fitness.detail" y ese dato no existia en ningun log. Sin esto,
            # diagnosticar el bucle muerto de evolve es adivinar.
            try:
                # zone + reason SIEMPRE (AT-29): habia lineas con fitness:{} y
                # error:"" — una linea roja sin porque no audita nada. reason se
                # deriva de lo mejor que haya; su ausencia total tambien se nombra.
                fit = r.get("fitness") or {}
                reason = (r.get("error")
                          or (f"suite roja: {fit['failed']} failed (rc {fit.get('rc')})"
                              if fit.get("failed") else "")
                          or fit.get("detail", "")[-300:]
                          or "sandbox_fail_sin_detalle")
                # mkdir: un root sin logs/ (instancia fresca) tiraba OSError y el
                # except lo tragaba — el rechazo desaparecia sin dejar rastro
                (root / "logs").mkdir(parents=True, exist_ok=True)
                with open(root / "logs" / "evolve_red.jsonl", "a",
                          encoding="utf-8") as fh:
                    fh.write(json.dumps(
                        {"ts": time.time(), "target": c.target,
                         "description": c.description[:120],
                         "zone": zone_of(c, root=root), "reason": reason,
                         "fitness": fit, "error": r.get("error", "")},
                        ensure_ascii=False) + "\n")
            except OSError:
                pass
            continue
        if open_pr:
            # gate de fitness pre-PR (W4.3): ast + ensemble + budget sobre el candidato
            # que YA pasó zona y sandbox — pocos, así el ensemble cross-family no se paga
            # por basura que igual moría. Mismo contrato fail-open que aligned_fn.
            try:
                fv = fitness_fn(c)
                fit_ok = bool(fv.get("ok", True))
                fit_checks = dict(fv.get("checks", {}))
            except Exception as e:   # infra caída => fail-open (el humano del PR gatea)
                fit_ok, fit_checks = True, {"error": f"fitness_fn (fail-open): {type(e).__name__}"}
            if not fit_ok:
                blocked_fitness.append(c.target)
                try:
                    with open(root / "logs" / "evolve_red.jsonl", "a",
                              encoding="utf-8") as fh:
                        fh.write(json.dumps(
                            {"ts": time.time(), "target": c.target, "kind": "fitness_fail",
                             "description": c.description[:120], "branch": r["branch"],
                             "checks": fit_checks}, ensure_ascii=False) + "\n")
                except OSError:
                    pass
                continue   # sin PR; la branch queda para autopsia humana
            # gate de alineación pre-PR (W4.1): tests verdes NO alcanza — el cambio
            # además tiene que alinear con GOAL.md. Solo acá (candidatos que ya pasaron
            # sandbox: pocos) para no pagar cross-family por basura que igual moría.
            try:
                v = aligned_fn(c)
                aligned = bool(getattr(v, "passed", True))
                refutations = list(getattr(v, "refutations", []))
            except Exception as e:   # infra caída => fail-open (el PR sigue gateado por humano)
                aligned, refutations = True, [f"aligned_fn error (fail-open): {type(e).__name__}"]
            if not aligned:
                blocked_goal.append(c.target)
                try:
                    with open(root / "logs" / "evolve_red.jsonl", "a",
                              encoding="utf-8") as fh:
                        fh.write(json.dumps(
                            {"ts": time.time(), "target": c.target, "kind": "goal_misaligned",
                             "description": c.description[:120], "branch": r["branch"],
                             "refutations": refutations[:5]}, ensure_ascii=False) + "\n")
                except OSError:
                    pass
                continue   # sin PR; la branch queda para autopsia humana, nada se trackea
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
            "blocked_zone_red": blocked_zone_red, "blocked_fitness": blocked_fitness,
            "blocked_goal": blocked_goal}


def propose_patch(target_file: str, finding: str, *, gen_model: str | None = None,
                  feedback: str = "") -> str:
    """Un modelo barato PROPONE el contenido nuevo de target_file para resolver
    `finding`. READ-ONLY: devuelve el texto, NO escribe nada. Aplicar = gate aparte.

    `feedback`: salida de pytest del intento ANTERIOR (ver propose_with_fast_retry) —
    mismo patron que project_integrate.py's hot-coder loop. Sin esto, cada intento
    era ciego a por que el anterior fallo (medido: causa raiz de por que evolve
    llevaba 12+ noches con 0 PRs — un solo tiro contra la suite completa, sin
    iterar, es un piso demasiado alto para un cambio de archivo entero).
    """
    from .patterns import fan_out
    from .config import DEFAULT_GENERATOR
    src = (ROOT / target_file).read_text(encoding="utf-8") if (ROOT / target_file).exists() else ""
    fb = (f"\nEl intento anterior FALLO estos tests:\n{feedback[:1200]}\n"
         f"Arreglalo sin reintroducir el problema original.\n") if feedback else ""
    prompt = (
        f"Sos un mejorador de codigo Python. Resolve este hallazgo SIN romper la API publica "
        f"ni los invariantes (cross-family, OneFlow, anti-sicofancia, observabilidad).\n\n"
        f"HALLAZGO: {finding}\n\nARCHIVO {target_file}:\n{src}\n{fb}\n"
        f"Devolve el CONTENIDO COMPLETO nuevo del archivo, sin explicacion, en un bloque de codigo.")
    out = fan_out([prompt], gen_model=gen_model or DEFAULT_GENERATOR, phase="evolve")[0].text
    # extract_fence: SIN esto, el ```python del modelo viajaba ADENTRO del .py
    # -> SyntaxError -> suite entera roja en la coleccion -> 12+ noches con 0
    # PRs. Confirmado por aritmetica: slim reportaba 'no_adelgazo' con +13
    # chars exactos = el overhead del fence. project_integrate siempre lo
    # extrajo; evolve (y todo lo que reusa propose_patch: slim, hardening,
    # auto-findings) nunca. El bug mas caro del sistema costaba 13 chars.
    from .textutil import extract_fence
    return extract_fence(out)


def _target_test_file(target_file: str, *, root: Path = ROOT) -> str | None:
    """mmorch/repo_mining.py -> tests/test_repo_mining.py si existe. El gate RAPIDO
    de iteracion usa esto (segundos, no minutos) en vez de la suite completa (10+
    min, 600+ tests) — coordinated_evolve_round sigue corriendo la suite entera
    como gate FINAL antes de abrir PR, esto solo acelera llegar ahi."""
    p = Path(target_file)
    if p.parts[:1] != ("mmorch",):
        return None
    cand = root / "tests" / f"test_{p.stem}.py"
    return str(cand.relative_to(root)).replace("\\", "/") if cand.exists() else None


def propose_with_fast_retry(target_file: str, finding: str, *, root: Path = ROOT,
                            max_attempts: int = 3, propose_fn=None,
                            quick_sandbox_fn=None) -> tuple[str, dict]:
    """Genera el patch iterando contra el test RAPIDO del modulo (si existe) antes
    de que coordinated_evolve_round gaste 10+ min corriendo la suite entera. Cada
    intento fallido le pasa el output real de pytest al siguiente — antes cada
    intento era ciego (un solo tiro, causa raiz medida del bucle muerto de evolve).

    Sin test rapido disponible (target fuera de mmorch/, o sin tests/test_X.py):
    un solo intento sin feedback — no hay gate barato contra el cual iterar.
    Devuelve (ultimo patch propuesto, resultado del ultimo intento rapido)."""
    propose_fn = propose_fn or propose_patch
    quick_sandbox_fn = quick_sandbox_fn or (
        lambda c, cmd: sandbox_branch(c, root=root, test_cmd=cmd, keep_on_pass=False))
    quick_test = _target_test_file(target_file, root=root)
    if quick_test is None:
        return propose_fn(target_file, finding), {"skipped": "sin test rapido"}

    feedback = ""
    last: dict = {}
    for _ in range(max_attempts):
        after = propose_fn(target_file, finding, feedback=feedback)
        change = snapshot_change(target_file, after, finding[:80], root=root)
        cmd = [sys.executable, "-m", "pytest", quick_test, "-q", "--no-header"]
        last = quick_sandbox_fn(change, cmd)
        if last.get("ok"):
            return after, last
        feedback = last.get("fitness", {}).get("detail", "")
    return after, last


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
                "blocked_zone_red": [], "blocked_fitness": [], "blocked_goal": []}
    candidates = []
    for f in findings:
        try:
            # antes: un solo tiro contra la suite completa (10+ min), ciego a
            # por que fallo el intento anterior — causa raiz medida de 12+
            # noches sin abrir un PR. Ahora itera rapido (segundos) contra el
            # test propio del modulo antes de llegar al gate caro de abajo.
            after, _ = propose_with_fast_retry(f["target"], f["finding"], root=root,
                                               propose_fn=propose_fn)
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

    from types import SimpleNamespace

    def _fake_aligned_ok(c):
        return SimpleNamespace(passed=True, refutations=[])

    def _fake_fitness_ok(c):
        return {"ok": True, "checks": {}}

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
        # rev-parse rc=0 (la branch existe) pero merge-base --is-ancestor rc=1
        # (NO mergeada aun): sin distinguirlos, el lock-release por commit-en-HEAD
        # (fix 2026-08-24) liberaba el target y el skip de ronda 2 jamas ocurria.
        class _R:
            returncode = 1 if a and a[0] == "merge-base" else 0
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
    r1 = coordinated_evolve_round([c1, c3], sandbox_fn=_fake_sandbox_ok, pr_fn=_fake_pr, aligned_fn=_fake_aligned_ok, fitness_fn=_fake_fitness_ok,
                                  path=tmp_state)
    assert set(r1["opened"]) == {"a.py", "b.py"}, r1
    assert r1["skipped_active_pr"] == [], r1

    # 2. ronda 2: un candidato NUEVO para 'a.py' (mismo target que c1) -> SKIP, no compite
    calls.clear()
    r2 = coordinated_evolve_round([c2], sandbox_fn=_fake_sandbox_ok, pr_fn=_fake_pr, aligned_fn=_fake_aligned_ok, fitness_fn=_fake_fitness_ok,
                                  path=tmp_state)
    assert r2["skipped_active_pr"] == ["a.py"], r2
    assert calls == [], "no debio llamar sandbox_fn para un archivo con PR abierto"

    # 3. el PR de 'a.py' se mergea (la branch ya no existe) -> reap la libera -> ronda 3 la toma
    globals()["_git"] = _fake_git_exists_false
    r3 = coordinated_evolve_round([c2], sandbox_fn=_fake_sandbox_ok, pr_fn=_fake_pr, aligned_fn=_fake_aligned_ok, fitness_fn=_fake_fitness_ok,
                                  path=tmp_state)
    assert r3["opened"] == ["a.py"], r3          # liberada y re-tomada en la MISMA ronda

    # 4. sandbox que falla (rojo) -> nunca se trackea, no bloquea futuras rondas
    globals()["_git"] = _fake_git_exists_true
    _save_pr_state({}, tmp_state)
    r4 = coordinated_evolve_round([c1], sandbox_fn=_fake_sandbox_fail, pr_fn=_fake_pr, aligned_fn=_fake_aligned_ok, fitness_fn=_fake_fitness_ok,
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
                             pr_fn=_fake_pr, aligned_fn=_fake_aligned_ok, fitness_fn=_fake_fitness_ok, path=tmp_state2)
    assert r_night["findings"] == 2, r_night
    assert set(r_night["opened"]) == {"x.py", "y.py"}, r_night

    # sin hallazgos -> no-op limpio, nunca llama propose/round
    def _fake_harvest_empty():
        return []
    r_empty = nightly_evolve(harvest_fn=_fake_harvest_empty,
                             propose_fn=lambda t, f: (_ for _ in ()).throw(
                                 AssertionError("no debio proponerse nada")))
    assert r_empty == {"findings": 0, "skipped_active_pr": [], "opened": [], "red": [],
                       "blocked_zone_red": [], "blocked_fitness": [],
                       "blocked_goal": []}, r_empty

    # un propose_fn que explota para UN finding no frena el resto
    def _propose_one_boom(target, finding):
        if target == "x.py":
            raise RuntimeError("modelo caido")
        return "def g(): return 2"
    r_partial = nightly_evolve(harvest_fn=_fake_harvest_some, propose_fn=_propose_one_boom,
                               root=Path(tempfile.mkdtemp()), sandbox_fn=_fake_sandbox_ok,
                               pr_fn=_fake_pr, aligned_fn=_fake_aligned_ok, fitness_fn=_fake_fitness_ok, path=Path(tempfile.mkdtemp()) / "s.json")
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
