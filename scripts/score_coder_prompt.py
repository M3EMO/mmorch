"""score_coder_prompt — scorer FROZEN para autoresearch con HEADROOM REAL: optimiza el
SYSTEM PROMPT del coder contra el pass-rate de una batería congelada de tasks algorítmicas
(verdad de ejecución, cero LLM-juez -> anti-reward-hacking). Prompt engineering nunca se
satura: un wording/instrucción mejor mueve el pass-rate de verdad, a diferencia de
code_quality/mutation_score sobre módulos (que daban 1.0 fijo — medido 2026-07).

autoresearch edita prompts/coder_prompt.txt (o MMORCH_CODER_PROMPT); este scorer lo lee, genera
cada solución con ESE system prompt (temperature=0 -> casi determinista, minimiza ruido), la
EJECUTA contra sus asserts, e imprime 'score: <pass_rate>'. Batería reusada de ab_intuition_router.

Uso:  python scripts/score_coder_prompt.py prompts/coder_prompt.txt
"""
import os
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

# batería congelada, EDGE-HEAVY a propósito (baseline bajo -> slope real; medido 2026-07: una
# batería fácil daba 0.9 plano, prompt-insensible). Cada task tiene un caso-borde donde el wording
# del prompt SÍ mueve la aguja (touching-vs-overlapping, división-entera-hacia-cero, puntuación,
# matriz no-cuadrada, RLE-solo-si-acorta). Cambiarla rompe la comparabilidad -> versionar, no editar.
TASKS = [
    ("def merge_intervals(intervals): fusioná intervalos [inicio,fin] que se solapan o TOCAN "
     "(ej [1,3] y [3,5] -> [1,5]). Devolvé la lista ordenada por inicio.",
     "assert merge_intervals([[1,3],[2,6],[8,10]])==[[1,6],[8,10]]\n"
     "assert merge_intervals([[1,3],[3,5]])==[[1,5]]\n"
     "assert merge_intervals([])==[]\nassert merge_intervals([[5,6],[1,2]])==[[1,2],[5,6]]"),
    ("def rpn_eval(tokens): evaluá notación polaca inversa (lista de tokens str). Operadores + - * / "
     "(división ENTERA truncada hacia cero). Devolvé el entero resultado.",
     "assert rpn_eval(['2','1','+','3','*'])==9\nassert rpn_eval(['4','13','5','/','+'])==6\n"
     "assert rpn_eval(['10','6','-'])==4\nassert rpn_eval(['-7','2','/'])==-3"),
    ("def roman_to_int(s): convertí un número romano (I,V,X,L,C,D,M) a entero, manejando la resta "
     "(IV=4, IX=9, XL=40, etc).",
     "assert roman_to_int('III')==3\nassert roman_to_int('IV')==4\nassert roman_to_int('IX')==9\n"
     "assert roman_to_int('LVIII')==58\nassert roman_to_int('MCMXCIV')==1994"),
    ("def word_freq(s): devolvé un dict palabra->conteo. Palabras separadas por cualquier whitespace, "
     "case-insensitive, ignorando signos de puntuación al inicio/fin de cada palabra.",
     "assert word_freq('The cat, the CAT!')=={'the':2,'cat':2}\n"
     "assert word_freq('')=={}\nassert word_freq('a a  a')=={'a':3}"),
    ("def spiral_order(matrix): devolvé los elementos de una matriz (lista de listas) en orden espiral "
     "horario, arrancando arriba-izquierda. La matriz puede no ser cuadrada.",
     "assert spiral_order([[1,2,3],[4,5,6],[7,8,9]])==[1,2,3,6,9,8,7,4,5]\n"
     "assert spiral_order([[1,2],[3,4]])==[1,2,4,3]\nassert spiral_order([])==[]\n"
     "assert spiral_order([[1,2,3,4]])==[1,2,3,4]"),
    ("def compress(s): run-length encode SOLO si acorta; si el resultado no es más corto que el "
     "original, devolvé el original. Ej 'aabcccccaaa'->'a2b1c5a3'; 'abc'->'abc'.",
     "assert compress('aabcccccaaa')=='a2b1c5a3'\nassert compress('abc')=='abc'\n"
     "assert compress('')==''\nassert compress('aa')=='aa'"),
]


def main() -> None:
    prompt = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").strip()
    from mmorch.providers import call
    from mmorch.textutil import extract_fence
    from mmorch.checkers import check
    from mmorch.config import DEFAULT_GENERATOR
    model = os.getenv("MMORCH_CODER_MODEL", DEFAULT_GENERATOR)

    passed = 0
    for task, tests in TASKS:
        try:
            out = call(model, [{"role": "system", "content": prompt},
                               {"role": "user", "content": task}],
                       pattern="score_coder", node="coder", temperature=0.0).text
            code = extract_fence(out)
            if check("python_exec", code=code + "\n" + tests, timeout=10).passed:
                passed += 1
        except Exception:
            pass   # una task que falla = 0 para esa task, no rompe la medición
    print(f"score: {round(passed / len(TASKS), 4)}")


if __name__ == "__main__":
    main()
