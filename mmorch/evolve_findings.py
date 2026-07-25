"""evolve_findings — fuente automática de hallazgos para el loop nocturno de auto-evolve
(pedido usuario 2026-07: "habría que armarle una fuente automática, no?").

No inventa un mecanismo nuevo — recicla `code_review.review()` (cero-cupo, cross-family
refutado, ya usado manualmente hoy sobre 5 servers reales) y lo aplica a los archivos que
CAMBIARON recientemente (git log), no al repo entero — un scan de 13k LOC cada noche sería
caro y en su mayoría redundante con la noche anterior. Cada finding confirmado (sobrevivió
el refute) se convierte en un candidato de evolve: (target_file, texto del hallazgo).

Cota de costo explícita (mismo patrón que el call-breaker de project_integrate.py): tope de
archivos Y de findings totales por corrida — una noche con muchos cambios no debe quemar
presupuesto sin límite.
"""
from __future__ import annotations

import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def _git_changed_py_files(*, days: int, root: Path) -> list[str]:
    """Archivos .py bajo mmorch/ modificados en los últimos `days` días (git log), más
    recientes primero. Vacío si no es un repo git o no hubo cambios."""
    try:
        p = subprocess.run(
            ["git", "log", f"--since={days}.days", "--name-only", "--pretty=format:",
             "--", "mmorch/*.py"],
            cwd=str(root), capture_output=True, text=True, timeout=30)
        if p.returncode != 0:
            return []
        seen, out = set(), []
        for ln in p.stdout.splitlines():
            ln = ln.strip()
            if ln and ln not in seen and Path(root, ln).is_file():
                seen.add(ln)
                out.append(ln)
        return out
    except Exception:
        return []


_FINDINGS_LOG = Path(__file__).resolve().parents[1] / "logs" / "evolve_findings.jsonl"


def harvest_findings(files: list[str] | None = None, *, days: int = 3, max_files: int = 5,
                     max_findings: int = 8, root: Path = ROOT,
                     changed_files_fn=None, review_fn=None) -> list[dict]:
    """Corre code_review.review() sobre hasta `max_files` archivos recientemente cambiados
    (o `files` si se pasa explícito — seam de test/uso puntual), devuelve hasta
    `max_findings` hallazgos CONFIRMADOS (ya refutados cross-family) como candidatos de
    evolve: [{target, finding, severity}], severidad-alta primero. `changed_files_fn`/
    `review_fn` inyectables (default = git log real / code_review.review real)."""
    changed_files_fn = changed_files_fn or (lambda: _git_changed_py_files(days=days, root=root))
    review_fn = review_fn or (lambda code, path: __import__("mmorch.code_review",
                              fromlist=["review"]).review(code, path=path))
    targets = (files if files is not None else changed_files_fn())[:max_files]
    out: list[dict] = []
    for rel in targets:
        fpath = Path(root, rel)
        try:
            code = fpath.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        try:
            r = review_fn(code, rel)
        except Exception:
            continue   # un archivo que no se puede revisar no debe frenar la cosecha entera
        for f in r.get("findings", []):
            out.append({"target": rel, "severity": f.get("severity", "low"),
                        "finding": f"{f.get('principle', '')}: {f.get('problem', '')} "
                                  f"— fix sugerido: {f.get('fix', '')}"})
    order = {"high": 0, "med": 1, "medium": 1, "low": 2}
    out.sort(key=lambda x: order.get(x["severity"], 3))
    kept = out[:max_findings]
    # PERSISTIR (2026-07, pedido explicito: el nightly solo guardaba los NOMBRES de archivo
    # en el log -- el contenido del hallazgo se perdia y habia que re-cosechar para verlo).
    # Append-only jsonl; quien cosecha, loguea. Best-effort: un fallo de log no rompe la cosecha.
    try:
        import time as _time
        _FINDINGS_LOG.parent.mkdir(parents=True, exist_ok=True)
        with open(_FINDINGS_LOG, "a", encoding="utf-8") as fh:
            ts = _time.time()
            for k in kept:
                fh.write(json.dumps({"ts": ts, **k}, ensure_ascii=False) + "\n")
    except OSError:
        pass   # side-channel: el log nunca frena la cosecha
    return kept


def _sample_py_files(root: Path, cap: int) -> list[str]:
    """Muestra de .py de un repo (para repos EXTERNOS: no hay 'recién cambiado' útil, se toma
    una tajada estable). Salta vendored/tests/build. Orden determinista (por path)."""
    skip = ("/test", "/tests", "/.venv", "/venv", "/site-packages", "/node_modules",
            "/build", "/dist", "/__pycache__", "/migrations")
    out = []
    for p in sorted(root.rglob("*.py")):
        rel = "/" + str(p.relative_to(root)).replace("\\", "/").lower()
        if any(s in rel for s in skip):
            continue
        out.append(str(p.relative_to(root)))
        if len(out) >= cap:
            break
    return out


_EXT_LOG = Path(__file__).resolve().parents[1] / "logs" / "external_findings.jsonl"


def learn_from_repos(repo_urls: list[str], *, cap_files: int = 8, max_findings: int = 12,
                     workdir: Path | None = None, clone_fn=None, harvest_fn=None,
                     log_path: Path | None = None) -> dict:
    """READ-ONLY: clona repos PÚBLICOS, cosecha findings de cada uno (code_review), y los
    guarda a un corpus de aprendizaje (logs/external_findings.jsonl). NUNCA abre PR, NUNCA
    escribe en el repo ajeno, NUNCA importa apply/nightly_evolve — separación DURA
    aprender-vs-contribuir (repos ajenos = solo material; PRs solo en los tuyos).

    Licencia/etiqueta: leer código público + aprender patrones es análisis, no redistribución;
    esto no genera derivados publicados. `clone_fn`/`harvest_fn` inyectables (self-check sin red)."""
    import subprocess
    import tempfile
    import time

    log_path = log_path or _EXT_LOG
    workdir = workdir or Path(tempfile.mkdtemp(prefix="mmorch-learn-"))
    clone_fn = clone_fn or (lambda url, dst: subprocess.run(
        ["git", "clone", "--depth", "1", url, str(dst)], capture_output=True, timeout=180).returncode == 0)
    harvest_fn = harvest_fn or (lambda root, files: harvest_findings(
        files, root=root, max_files=cap_files, max_findings=max_findings))

    summary: dict = {"repos": 0, "findings": 0, "failed": []}
    log_path.parent.mkdir(parents=True, exist_ok=True)
    for url in repo_urls:
        name = url.rstrip("/").split("/")[-1].removesuffix(".git")
        dst = workdir / name
        if not clone_fn(url, dst):
            summary["failed"].append(url)
            continue
        files = _sample_py_files(dst, cap_files)
        try:
            findings = harvest_fn(dst, files)
        except Exception:
            summary["failed"].append(url)
            continue
        summary["repos"] += 1
        summary["findings"] += len(findings)
        with open(log_path, "a", encoding="utf-8") as f:
            for fd in findings:
                f.write(json.dumps({"ts": time.time(), "repo": name, **fd}, ensure_ascii=False) + "\n")
    return summary


if __name__ == "__main__":
    # cero-API: changed_files_fn/review_fn inyectados.
    FAKE_REVIEWS = {
        "mmorch/a.py": {"findings": [{"principle": "DRY", "severity": "low",
                                      "problem": "codigo duplicado", "fix": "extraer helper"}]},
        "mmorch/b.py": {"findings": [{"principle": "Security", "severity": "high",
                                      "problem": "path sin validar", "fix": "sanitizar"},
                                     {"principle": "Naming", "severity": "low",
                                      "problem": "var x", "fix": "renombrar"}]},
        "mmorch/c.py": {"findings": []},   # archivo limpio -> sin findings
    }

    def _fake_review(code, path):
        return FAKE_REVIEWS.get(path, {"findings": []})

    res = harvest_findings(["mmorch/a.py", "mmorch/b.py", "mmorch/c.py"],
                           review_fn=_fake_review,
                           changed_files_fn=lambda: ["mmorch/a.py", "mmorch/b.py", "mmorch/c.py"])
    # NOTA: harvest_findings intenta LEER el archivo real del disco (fpath.read_text) antes
    # de llamar review_fn -> con paths ficticios, todos se skipean silenciosamente (por
    # diseño: un archivo no-leible no frena la cosecha). Este self-check prueba el ORDEN Y
    # CAP con archivos REALES del propio repo (siempre existen, cero riesgo de flake).
    real_files = ["mmorch/evolve.py", "mmorch/evolve_findings.py"]
    res = harvest_findings(real_files, review_fn=lambda code, path: FAKE_REVIEWS.get(
        {"mmorch/evolve.py": "mmorch/b.py", "mmorch/evolve_findings.py": "mmorch/a.py"}[path],
        {"findings": []}))
    assert len(res) == 3, res                              # 2 de b.py (high+low) + 1 de a.py (low)
    assert res[0]["severity"] == "high", res                # high primero
    assert res[0]["target"] == "mmorch/evolve.py", res
    assert all("fix sugerido" in r["finding"] for r in res)

    # cap de findings: max_findings corta aunque haya mas
    res2 = harvest_findings(real_files, max_findings=1, review_fn=lambda code, path: FAKE_REVIEWS.get(
        {"mmorch/evolve.py": "mmorch/b.py", "mmorch/evolve_findings.py": "mmorch/a.py"}[path],
        {"findings": []}))
    assert len(res2) == 1 and res2[0]["severity"] == "high", res2

    # un archivo cuyo review_fn explota no frena la cosecha del resto
    def _boom_then_ok(code, path):
        if path.endswith("evolve.py"):
            raise RuntimeError("api caida")
        return {"findings": [{"principle": "X", "severity": "low", "problem": "p", "fix": "f"}]}
    res3 = harvest_findings(real_files, review_fn=_boom_then_ok)
    assert len(res3) == 1 and res3[0]["target"] == "mmorch/evolve_findings.py", res3

    # max_files corta la lista de targets ANTES de revisar (cota de costo real)
    calls: list = []

    def _counting_review(code, path):
        calls.append(path)
        return {"findings": []}
    harvest_findings(changed_files_fn=lambda: [f"mmorch/f{i}.py" for i in range(10)],
                     max_files=3, review_fn=_counting_review)
    # (los f0..f9 no existen en disco -> se skipean en el read, pero el CAP a targets[:max_files]
    # ya paso antes de leer -> 0 llamadas reales a review_fn, lo cual prueba que nunca se
    # itero mas alla de max_files; confirmamos indirectamente con un set real chico:)
    harvest_findings(real_files + ["mmorch/config.py"], max_files=1, review_fn=_counting_review)
    assert len(calls) == 1, calls   # solo el 1er archivo del recorte, pese a pasar 3

    print("evolve_findings OK — harvest ordena por severidad, respeta caps, resiliente a fallas por-archivo")

    # --- learn_from_repos: READ-ONLY, corpus de aprendizaje, cero PR/apply (cero-red) ---
    import tempfile as _tf
    wd = Path(_tf.mkdtemp())
    logp = wd / "ext.jsonl"

    def _fake_clone(url, dst):
        if "broken" in url:
            return False
        (dst / "pkg").mkdir(parents=True, exist_ok=True)
        (dst / "pkg" / "core.py").write_text("def f(): return 1", encoding="utf-8")
        (dst / "tests").mkdir(exist_ok=True)
        (dst / "tests" / "test_x.py").write_text("def test(): pass", encoding="utf-8")  # se saltea
        return True

    def _fake_harvest(root, files):
        assert "tests/test_x.py" not in files, "tests/ debe saltarse en el sampling"
        return [{"target": files[0], "severity": "high", "finding": "algo real"}]

    s = learn_from_repos(["https://github.com/u/repoA", "https://github.com/u/broken",
                          "https://github.com/u/repoB.git"],
                         workdir=wd, clone_fn=_fake_clone, harvest_fn=_fake_harvest, log_path=logp)
    assert s["repos"] == 2 and s["findings"] == 2 and s["failed"] == ["https://github.com/u/broken"], s
    corpus = [json.loads(ln) for ln in logp.read_text(encoding="utf-8").splitlines()]
    assert len(corpus) == 2 and {c["repo"] for c in corpus} == {"repoA", "repoB"}, corpus
    assert all("finding" in c and "ts" in c for c in corpus)

    # SEPARACIÓN DURA (verificada por AST, no por texto): ninguna función de este módulo LLAMA a
    # apply/PR — es read-only por construcción. Chequeo real: no hay Call a esos nombres.
    import ast as _ast
    tree = _ast.parse(Path(__file__).read_text(encoding="utf-8"))
    called = {n.func.id for n in _ast.walk(tree)
              if isinstance(n, _ast.Call) and isinstance(n.func, _ast.Name)}
    for forbidden in ("open_pr_branch", "coordinated_evolve_round", "nightly_evolve", "apply_change"):
        assert forbidden not in called, f"read-only: harvest no debe LLAMAR a {forbidden}"

    print("learn_from_repos OK — read-only multi-repo, salta tests/vendored, corpus jsonl, "
          "clone-fail resiliente, cero LLAMADA a apply/PR (separación dura por AST)")
