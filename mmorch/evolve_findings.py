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
    return out[:max_findings]


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
