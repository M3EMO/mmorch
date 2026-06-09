"""build_dataset_repos — escala el dataset de calidad de código minando N repos GitHub.

Corre el miner JIT-defect (mmorch.dataset) sobre una lista de repos populares de Python,
agrega + dedup, y guarda a logs/codequality_dataset.jsonl (resumible: salta repos ya
clonados, re-usa el jsonl). Más repos -> más señal (la spec v1.0 pide >=10k samples).

Uso: python build_dataset_repos.py [--max-commits 300] [--max-total 12000]
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import pathlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
from mmorch.dataset import build_dataset

ROOT = pathlib.Path(__file__).resolve().parent
REPOS_DIR = ROOT / ".dataset_repos"
OUT = ROOT / "logs" / "codequality_dataset.jsonl"

# repos medianos con historia rica de fixes (pure-ish python)
REPOS = {
    "yt-dlp": "https://github.com/yt-dlp/yt-dlp.git",   # densidad brutal de fix-commits
    "requests": "https://github.com/psf/requests.git",
    "flask": "https://github.com/pallets/flask.git",
    "click": "https://github.com/pallets/click.git",
    "werkzeug": "https://github.com/pallets/werkzeug.git",
    "httpx": "https://github.com/encode/httpx.git",
    "rich": "https://github.com/Textualize/rich.git",
    "black": "https://github.com/psf/black.git",
    "pydantic": "https://github.com/pydantic/pydantic.git",
}


def _clone(name: str, url: str) -> pathlib.Path:
    dest = REPOS_DIR / name
    if dest.exists():
        return dest
    REPOS_DIR.mkdir(parents=True, exist_ok=True)
    print(f"  clonando {name}...", flush=True)
    subprocess.run(["git", "clone", "--quiet", url, str(dest)], timeout=600)
    return dest


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--max-commits", type=int, default=300)
    ap.add_argument("--max-total", type=int, default=12000)
    args = ap.parse_args()

    seen, data = set(), []
    per_repo = {}
    for name, url in REPOS.items():
        try:
            repo = _clone(name, url)
        except Exception as e:
            print(f"  {name}: clone FALLO ({str(e)[:60]}) -> skip"); continue
        before = len(data)
        try:
            d = build_dataset(repo, max_commits=args.max_commits, max_samples=args.max_total)
        except Exception as e:
            print(f"  {name}: mina FALLO ({str(e)[:60]})"); continue
        for code, label in d:
            h = hash(code)
            if h in seen:
                continue
            seen.add(h)
            data.append((code, label))
        per_repo[name] = len(data) - before
        print(f"  {name}: +{per_repo[name]} (total {len(data)})", flush=True)
        if len(data) >= args.max_total:
            break

    OUT.parent.mkdir(parents=True, exist_ok=True)
    with open(OUT, "w", encoding="utf-8") as fh:
        for code, label in data:
            fh.write(json.dumps({"label": label, "code": code}, ensure_ascii=False) + "\n")
    n0 = sum(1 for _, l in data if l == 0)
    print("\n" + "=" * 50)
    print(f"DATASET: {len(data)} funciones | buggy(0)={n0} fixed(1)={len(data)-n0}")
    print(f"por repo: {per_repo}")
    print(f"guardado: {OUT}")


if __name__ == "__main__":
    main()
