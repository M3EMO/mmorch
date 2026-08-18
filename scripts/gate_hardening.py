"""Gate del hardening loop: exit 0 SOLO si los tests nuevos matan mutantes.

Uso: python scripts/gate_hardening.py <module_rel> <baseline_survived>
Corre en el WORKTREE del build (cwd = repo del worktree): re-caza el modulo y
exige survived < baseline, y despues la suite completa verde. Verdad de
ejecucion pura — un test que no mata mutantes no pasa este gate aunque este
verde."""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))


def main() -> int:
    module_rel, baseline = sys.argv[1], int(sys.argv[2])
    from mmorch.bughunt import survivors_for
    test_rel = f"tests/test_{Path(module_rel).stem}.py"
    r = survivors_for(module_rel, test_rel, repo_dir=str(ROOT), max_mutants=12)
    survived = r.get("survived")
    if r.get("skipped") or survived is None:
        print(f"gate: FAIL ({r.get('skipped', 'sin resultado')})")
        return 1
    if survived >= baseline:
        print(f"gate: FAIL (survived {survived} >= baseline {baseline})")
        return 1
    import tempfile
    bt = tempfile.mkdtemp(prefix="mmorch_bt_")
    suite = subprocess.run([sys.executable, "-m", "pytest", "-q",
                            f"--basetemp={bt}"], cwd=ROOT,
                           capture_output=True, timeout=1800)
    if suite.returncode != 0:
        print("gate: FAIL (suite completa roja)")
        return 1
    print(f"gate: OK (survived {baseline} -> {survived}, suite verde)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
