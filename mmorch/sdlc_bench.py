"""sdlc_bench — mmorch mide el builder `sdlc` sobre las tasks congeladas del bench (el SDLC ya no trae modo bench).

    python -m mmorch.sdlc_bench --task <bench> [--wt DIR] [--phase P] [--from-stage N] [--max-fix N] [--self-check]
"""
from __future__ import annotations

import sys

from sdlc import pipeline

from . import bench

# Tokens que la spec tiene que nombrar verbatim (G1) para las tasks del bench.
BENCH_CONTRACTS = {
    "rate-limiter": ["TokenBucket", "MultiLimiter", "allow", "capacity", "refill_per_s", "now", "key",
                     "limiter/core.py", "limiter/multi.py", "limiter/__init__.py"],
    "etl-pipeline": ["parse_lines", "normalize", "to_summary", "run", "maxsplit", "count", "total", "by_name",
                     "etl/extract.py", "etl/transform.py", "etl/load.py", "etl/__init__.py"],
    "lru-ttl-cache": ["LRUCache", "maxsize", "ttl_s", "get", "put", "now", "None", "cache/core.py", "cache/__init__.py"],
}


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)

    def arg(flag, default):
        return argv[argv.index(flag) + 1] if flag in argv else default

    for _s in (sys.stdout, sys.stderr):  # D10: la consola cp1252 tiro UnicodeEncodeError por un "->" de Claude
        getattr(_s, "reconfigure", lambda **k: None)(encoding="utf-8", errors="replace")
    name = arg("--task", "rate-limiter")
    task = bench.get_task(name)
    pipeline.configure(task, contract=BENCH_CONTRACTS.get(name, []), wt=arg("--wt", None), phase=arg("--phase", None),
                       max_fix=arg("--max-fix", 3))
    if "--self-check" in argv:
        return pipeline.self_check()
    if not (pipeline.WT / ".git").exists():  # el pipeline recibe un directorio plano y le hace git init
        bench.materialize(task, str(pipeline.WT))
    return 0 if pipeline.run(float(arg("--from-stage", "2"))).get("status") == "built" else 1


if __name__ == "__main__":
    sys.exit(main())
