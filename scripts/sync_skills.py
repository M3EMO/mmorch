"""sync_skills — el repo mmorch es la fuente de verdad de las skills vendorizadas.

`skills/pocock/` (clonadas de mattpocock/skills, ver UPSTREAM.txt) se copian a
`~/.claude/skills/` donde Claude Code las levanta. Editar SIEMPRE en el repo y
re-correr esto — nunca editar la copia instalada (se pisa).

Uso:
    python scripts/sync_skills.py            # sync repo -> ~/.claude/skills
    python scripts/sync_skills.py --check    # solo reporta drift, no copia
"""
from __future__ import annotations

import filecmp
import shutil
import sys
from pathlib import Path

REPO_SKILLS = Path(__file__).resolve().parent.parent / "skills" / "pocock"
INSTALLED = Path.home() / ".claude" / "skills"


def _drift(src: Path, dst: Path) -> list[str]:
    if not dst.exists():
        return [f"MISSING {dst.name}"]
    cmp = filecmp.dircmp(src, dst)
    out = [f"DIFF {dst.name}/{f}" for f in cmp.diff_files]
    out += [f"ONLY-REPO {dst.name}/{f}" for f in cmp.left_only]
    return out


def main() -> int:
    check_only = "--check" in sys.argv
    drift: list[str] = []
    for src in sorted(p for p in REPO_SKILLS.iterdir() if p.is_dir()):
        dst = INSTALLED / src.name
        d = _drift(src, dst)
        drift += d
        if d and not check_only:
            shutil.copytree(src, dst, dirs_exist_ok=True)
            print(f"synced {src.name} ({len(d)} cambios)")
    if not drift:
        print("sin drift: instaladas == repo")
    elif check_only:
        print("\n".join(drift))
    return 1 if (check_only and drift) else 0


if __name__ == "__main__":
    raise SystemExit(main())
