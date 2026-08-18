"""Minar un repo ajeno a demanda: aprende el jugo, borra la fruta.

Uso:  .venv/Scripts/python.exe scripts/minar.py <url-del-repo>
      .venv/Scripts/python.exe scripts/minar.py --cola <url>   (encolar p/ nightly)
"""

import io
import pathlib
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")


def main() -> None:
    if len(sys.argv) < 2:
        print(__doc__)
        return
    if sys.argv[1] == "--cola" and len(sys.argv) == 3:
        q = ROOT / "logs" / "repos_queue.txt"
        with open(q, "a", encoding="utf-8") as f:
            f.write(sys.argv[2].strip() + "\n")
        print(f"encolado para el nightly: {sys.argv[2]}")
        return
    from dotenv import load_dotenv
    load_dotenv(ROOT / ".env")
    from mmorch.repo_mining import mine_repo
    r = mine_repo(sys.argv[1], orch_root=str(ROOT),
                  today=time.strftime("%Y-%m-%d"))
    import json
    print(json.dumps(r, ensure_ascii=False, indent=1))


if __name__ == "__main__":
    main()
