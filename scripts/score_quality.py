"""score_quality — scorer FROZEN para autoresearch (anti-reward-hacking: determinista, cero LLM).

Gate + métrica en un solo número:
  1. GATE: los tests de evolve deben pasar — si no, score: 0.0 (piso). Sin esto, autoresearch
     podría "mejorar" la mantenibilidad vaciando lógica real (el score de radon sube si borrás todo).
  2. MÉTRICA: checkers.code_quality (radon cc + maintainability + smells AST, 0..1).

Uso:  python scripts/score_quality.py <target.py>       (imprime "score: <n>")
Corre relativo al CWD (pensado para ejecutarse DENTRO de un worktree aislado).
"""
import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

TESTS = ["tests/test_evolve.py", "tests/test_evolve_branch.py",
         "tests/test_evolve_goal_guard.py", "tests/test_evolve_motor.py"]


def main() -> None:
    target = sys.argv[1]
    p = subprocess.run([sys.executable, "-m", "pytest", *TESTS, "-q", "--no-header"],
                       capture_output=True, text=True, timeout=600)
    if p.returncode != 0:
        print("score: 0.0")          # gate rojo -> piso; keep/discard descarta este intento
        return
    from mmorch.checkers import check
    r = check("code_quality", code=open(target, encoding="utf-8").read(), min_score=0.0)
    print(f"score: {r.got}")


if __name__ == "__main__":
    main()
