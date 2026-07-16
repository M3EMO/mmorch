"""score_mutation — scorer FROZEN para autoresearch: mutation_score de un módulo PURO contra
un archivo de asserts que el loop fortalece (target con headroom real, reemplaza el viejo
code_quality-de-archivo-grande que no movía la aguja).

Gate + métrica, determinista (cero LLM, anti-reward-hacking):
  1. GATE: los asserts DEBEN pasar sobre el código real — si no, score 0.0 (un test "fuerte"
     pero roto no vale; evita que el loop suba mutation_score rompiendo los asserts).
  2. MÉTRICA: checkers.mutation_score(code=módulo, tests=asserts) — % de mutantes matados.
     Tests más fuertes matan más mutantes -> score sube. Ahí está el headroom.

Uso:  python scripts/score_mutation.py <archivo_de_asserts.py>   (imprime "score: <n>")
El módulo bajo prueba es mmorch/signature.py (PURO: regex, sin llamadas a API -> mutantes
rápidos y deterministas). Corre relativo al CWD (worktree aislado).
"""
import os
import pathlib
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

# módulo bajo prueba (el que mutation_score muta) — override por env para otros targets.
_MODULE = os.getenv("MMORCH_MUT_MODULE", "mmorch/signature.py")


def _asserts_only(src: str) -> str:
    """Cuerpo de asserts sin el import del header (mutation_score inyecta 'from candidate
    import *' — candidate.py = el módulo mutado, así que los símbolos ya quedan en scope).
    Toma desde la primera línea que no es import/comentario/vacía."""
    out, started = [], False
    for ln in src.splitlines():
        if not started and (ln.startswith(("from ", "import ", "#")) or not ln.strip()):
            continue
        started = True
        out.append(ln)
    return "\n".join(out)


def main() -> None:
    target = pathlib.Path(sys.argv[1])
    asserts_src = target.read_text(encoding="utf-8")
    # GATE: los asserts (nivel-módulo) pasan sobre el código real -> ejecutar el archivo directo
    # (pytest NO colecta mut_*.py; los asserts corren al importar). rc!=0 = algún assert falló.
    g = subprocess.run([sys.executable, str(target)], capture_output=True, text=True, timeout=120)
    if g.returncode != 0:
        print("score: 0.0")
        return
    from mmorch.checkers import check
    mod = pathlib.Path(_MODULE).read_text(encoding="utf-8")
    body = _asserts_only(asserts_src)
    r = check("mutation_score", code=mod, tests=body, min_score=0.0, max_mutants=12, timeout=60.0)
    print(f"score: {r.got}")


if __name__ == "__main__":
    main()
