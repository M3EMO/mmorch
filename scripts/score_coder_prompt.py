"""score_coder_prompt — scorer FROZEN para autoresearch con HEADROOM REAL: optimiza el
SYSTEM PROMPT del coder contra el pass-rate de una batería congelada de tasks algorítmicas
(verdad de ejecución, cero LLM-juez -> anti-reward-hacking). Prompt engineering nunca se
satura: un wording/instrucción mejor mueve el pass-rate de verdad, a diferencia de
code_quality/mutation_score sobre módulos (que daban 1.0 fijo — medido 2026-07).

autoresearch edita mmorch/prompts/coder_prompt.txt (o MMORCH_CODER_PROMPT); este scorer lo lee, genera
cada solución con ESE system prompt (temperature=0 -> casi determinista, minimiza ruido), la
EJECUTA contra sus asserts, e imprime 'score: <pass_rate>'. Batería reusada de ab_intuition_router.

Uso:  python scripts/score_coder_prompt.py mmorch/prompts/coder_prompt.txt
"""
import os
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

# batería congelada, EDGE-HEAVY a propósito (baseline bajo -> slope real; medido 2026-07: una
# batería fácil daba 0.9 plano, prompt-insensible). Cada task tiene un caso-borde donde el wording
# del prompt SÍ mueve la aguja (touching-vs-overlapping, división-entera-hacia-cero, puntuación,
# matriz no-cuadrada, RLE-solo-si-acorta). Cambiarla rompe la comparabilidad -> versionar, no editar.
# NOTA (2026-07, 2do audit): la v1 daba los hints de borde EN el enunciado de la task ("TOCAN",
# "hacia cero", etc) -> confound medido: deepseek-chat a temp=0 sigue el spec explícito sin
# importar el prompt de sistema (score=1.0 con prompt mínimo, en las 6 + en 3 complejas nuevas
# con estado -LRU eviction, ciclo dirigido, Bellman-Ford ciclo negativo-). El enunciado spoileaba
# el borde -> el system-prompt no tenía nada que aportar. v2: spec pelado, SIN nombrar el borde;
# el system prompt es lo único que puede instruir "pensá los bordes" -> ahí vive el headroom real.
TASKS = [
    ("def merge_intervals(intervals): fusioná los intervalos [inicio,fin] que se superponen. "
     "Devolvé la lista ordenada por inicio.",
     "assert merge_intervals([[1,3],[2,6],[8,10]])==[[1,6],[8,10]]\n"
     "assert merge_intervals([[1,3],[3,5]])==[[1,5]]\n"
     "assert merge_intervals([])==[]\nassert merge_intervals([[5,6],[1,2]])==[[1,2],[5,6]]"),
    ("def rpn_eval(tokens): evaluá notación polaca inversa (lista de tokens str). Operadores + - * /. "
     "Devolvé el resultado como entero.",
     "assert rpn_eval(['2','1','+','3','*'])==9\nassert rpn_eval(['4','13','5','/','+'])==6\n"
     "assert rpn_eval(['10','6','-'])==4\nassert rpn_eval(['-7','2','/'])==-3"),
    ("def roman_to_int(s): convertí un número romano a entero.",
     "assert roman_to_int('III')==3\nassert roman_to_int('IV')==4\nassert roman_to_int('IX')==9\n"
     "assert roman_to_int('LVIII')==58\nassert roman_to_int('MCMXCIV')==1994"),
    ("def word_freq(s): devolvé un dict palabra->conteo de las palabras en el texto.",
     "assert word_freq('The cat, the CAT!')=={'the':2,'cat':2}\n"
     "assert word_freq('')=={}\nassert word_freq('a a  a')=={'a':3}"),
    ("def spiral_order(matrix): devolvé los elementos de una matriz (lista de listas) en orden "
     "espiral horario, arrancando arriba-izquierda.",
     "assert spiral_order([[1,2,3],[4,5,6],[7,8,9]])==[1,2,3,6,9,8,7,4,5]\n"
     "assert spiral_order([[1,2],[3,4]])==[1,2,4,3]\nassert spiral_order([])==[]\n"
     "assert spiral_order([[1,2,3,4]])==[1,2,3,4]"),
    ("def compress(s): comprimí el string por run-length encoding (letra+conteo consecutivo).",
     "assert compress('aabcccccaaa')=='a2b1c5a3'\nassert compress('abc')=='abc'\n"
     "assert compress('')==''\nassert compress('aa')=='aa'"),
    ("class LRUCache: constructor(capacity). get(key)->valor o -1 si no está. put(key,valor): "
     "inserta o actualiza, respetando la capacidad.",
     "c=LRUCache(2)\nc.put(1,1); c.put(2,2)\nassert c.get(1)==1\n"
     "c.put(3,3)  # evict 2 (1 fue usado hace poco por el get)\n"
     "assert c.get(2)==-1\nc.put(4,4)  # evict 1\nassert c.get(1)==-1\n"
     "assert c.get(3)==3\nassert c.get(4)==4"),
    ("def has_cycle(graph): graph es dict nodo->lista de vecinos. Devolvé True si el grafo tiene "
     "un ciclo, False si no.",
     "assert has_cycle({1:[2],2:[3],3:[]})==False\n"
     "assert has_cycle({1:[2],2:[3],3:[1]})==True\n"
     "assert has_cycle({1:[2],2:[]},)==False\n"
     "assert has_cycle({1:[2],2:[1],3:[4],4:[]})==True\nassert has_cycle({})==False"),
    ("def bellman_ford(edges, n, src): edges=lista de (u,v,peso), n=cantidad de nodos (0..n-1). "
     "Devolvé dict nodo->distancia mínima desde src, o None si el grafo tiene un problema que "
     "hace que 'distancia mínima' no esté bien definida.",
     "assert bellman_ford([(0,1,4),(0,2,1),(2,1,2),(1,3,1),(2,3,5)],4,0)=="
     "{0:0,1:3,2:1,3:4}\n"
     "assert bellman_ford([(0,1,1)],3,0)=={0:0,1:1,2:float('inf')}\n"
     "assert bellman_ford([(0,1,1),(1,2,-1),(2,0,-1)],3,0) is None"),
]


def main() -> None:
    prompt = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").strip()
    from mmorch.providers import call
    from mmorch.textutil import extract_fence
    from mmorch.checkers import check
    from mmorch.config import DEFAULT_GENERATOR
    model = os.getenv("MMORCH_CODER_MODEL", DEFAULT_GENERATOR)

    passed = 0
    for i, (task, tests) in enumerate(TASKS):
        try:
            out = call(model, [{"role": "system", "content": prompt},
                               {"role": "user", "content": task}],
                       pattern="score_coder", node="coder", temperature=0.0).text
            code = extract_fence(out)
            r = check("python_exec", code=code + "\n" + tests, timeout=10)
            if r.passed:
                passed += 1
            else:
                # detalle por tarea, no solo el score agregado (medido: sin esto
                # autoresearch optimiza a ciegas — 15+ noches sin poder ver CUAL
                # tarea fallaba, solo un numero. score() de autoresearch.py
                # captura este stdout completo como feedback de la proxima ronda)
                print(f"FAIL tarea {i} ({task[:50]}): {r.detail[:200]}")
        except Exception as e:
            print(f"FAIL tarea {i} ({task[:50]}): excepcion {type(e).__name__}: {e}")
    print(f"score: {round(passed / len(TASKS), 4)}")


if __name__ == "__main__":
    main()
