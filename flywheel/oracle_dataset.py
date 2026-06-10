"""oracle_dataset — la label CORRECTA pal flywheel: EJECUCION.
DeepSeek (fan_out) genera K soluciones por spec a temp alta -> mix natural de correctas/
buggy. checkers.unit_test corre pytest aislado -> label = pasa(1)/falla(0). Esta label SI
esta en el texto (la correctitud es funcion del codigo), a diferencia de JIT-defect.

Eval honesto: GroupKFold por SPEC (test = specs no vistos) -> mide si el encoder generaliza
'que hace correcto a un codigo', no memoriza specs. Esa es la prueba de que el cuello era la
LABEL, no la representacion.

Costo: ~N_SPECS*K gens cortas DeepSeek (centavos). Cap via MMORCH_MAX_MONTHLY_USD.
"""
from __future__ import annotations
import json, re, sys, pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
OUT = ROOT / "logs" / "oracle_dataset.jsonl"

# (nombre, spec-en-prosa, tests-pytest). Tests importan `from candidate import *`.
SPECS = [
    ("two_sum", "two_sum(nums, target) returns indices [i,j] of the two numbers that add to target (i<j).",
     "assert sorted(two_sum([2,7,11,15],9))==[0,1]\nassert sorted(two_sum([3,2,4],6))==[1,2]\nassert sorted(two_sum([3,3],6))==[0,1]"),
    ("is_palindrome", "is_palindrome(s) returns True if s reads the same forwards and backwards ignoring case and non-alphanumerics.",
     "assert is_palindrome('A man, a plan, a canal: Panama')\nassert not is_palindrome('race a car')\nassert is_palindrome('')"),
    ("fib", "fib(n) returns the n-th Fibonacci number, fib(0)=0, fib(1)=1.",
     "assert fib(0)==0\nassert fib(1)==1\nassert fib(10)==55\nassert fib(20)==6765"),
    ("gcd", "gcd(a,b) returns the greatest common divisor of two positive integers.",
     "assert gcd(12,8)==4\nassert gcd(17,5)==1\nassert gcd(100,75)==25"),
    ("flatten", "flatten(lst) returns a flat list from an arbitrarily nested list of integers.",
     "assert flatten([1,[2,[3,4]],5])==[1,2,3,4,5]\nassert flatten([])==[]\nassert flatten([[1],[2],[3]])==[1,2,3]"),
    ("rle", "rle(s) run-length-encodes a string: 'aaabb' -> 'a3b2'. Single chars get count 1.",
     "assert rle('aaabb')=='a3b2'\nassert rle('abc')=='a1b1c1'\nassert rle('')==''"),
    ("rotate", "rotate(lst,k) rotates a list right by k positions (k may exceed len).",
     "assert rotate([1,2,3,4,5],2)==[4,5,1,2,3]\nassert rotate([1,2,3],3)==[1,2,3]\nassert rotate([1,2,3],4)==[3,1,2]"),
    ("anagram", "anagram(a,b) returns True if the two strings are anagrams (same letters, case-insensitive, ignore spaces).",
     "assert anagram('listen','silent')\nassert anagram('Dormitory','Dirty Room')\nassert not anagram('hello','world')"),
    ("primes_up_to", "primes_up_to(n) returns a sorted list of all primes <= n.",
     "assert primes_up_to(10)==[2,3,5,7]\nassert primes_up_to(2)==[2]\nassert primes_up_to(1)==[]"),
    ("merge_sorted", "merge_sorted(a,b) merges two sorted lists into one sorted list.",
     "assert merge_sorted([1,3,5],[2,4,6])==[1,2,3,4,5,6]\nassert merge_sorted([],[1])==[1]\nassert merge_sorted([1,1],[1])==[1,1,1]"),
    ("balanced", "balanced(s) returns True if brackets ()[]{} in s are balanced and properly nested.",
     "assert balanced('([]{})')\nassert not balanced('([)]')\nassert balanced('')\nassert not balanced('(')"),
    ("word_count", "word_count(s) returns a dict mapping each lowercased word to its count, splitting on whitespace.",
     "assert word_count('a b a')=={'a':2,'b':1}\nassert word_count('')=={}\nassert word_count('Hi hi')=={'hi':2}"),
    ("max_subarray", "max_subarray(nums) returns the maximum sum of any contiguous non-empty subarray.",
     "assert max_subarray([-2,1,-3,4,-1,2,1,-5,4])==6\nassert max_subarray([1])==1\nassert max_subarray([-1,-2])==-1"),
    ("title_case", "title_case(s) capitalizes the first letter of each word, lowercasing the rest.",
     "assert title_case('hello world')=='Hello World'\nassert title_case('aBC dEF')=='Abc Def'\nassert title_case('')==''"),
    ("dedup", "dedup(lst) removes duplicates from a list preserving first-seen order.",
     "assert dedup([1,2,1,3,2])==[1,2,3]\nassert dedup([])==[]\nassert dedup(['a','a','b'])==['a','b']"),
    ("roman", "roman(n) converts an integer 1..3999 to a Roman numeral string.",
     "assert roman(4)=='IV'\nassert roman(9)=='IX'\nassert roman(58)=='LVIII'\nassert roman(1994)=='MCMXCIV'"),
    ("chunk", "chunk(lst,n) splits a list into consecutive chunks of size n (last may be shorter).",
     "assert chunk([1,2,3,4,5],2)==[[1,2],[3,4],[5]]\nassert chunk([],3)==[]\nassert chunk([1,2,3],3)==[[1,2,3]]"),
    ("count_vowels", "count_vowels(s) returns the number of vowels (aeiou, case-insensitive) in s.",
     "assert count_vowels('Hello')==2\nassert count_vowels('xyz')==0\nassert count_vowels('AEIOU')==5"),
    ("is_sorted", "is_sorted(lst) returns True if the list is sorted in non-decreasing order.",
     "assert is_sorted([1,2,2,3])\nassert not is_sorted([3,1,2])\nassert is_sorted([])"),
    ("digit_sum", "digit_sum(n) returns the sum of decimal digits of a non-negative integer.",
     "assert digit_sum(0)==0\nassert digit_sum(123)==6\nassert digit_sum(9999)==36"),
]

_FENCE = re.compile(r"```(?:python)?\s*(.*?)```", re.DOTALL)


def extract_code(text: str) -> str:
    m = _FENCE.search(text)
    code = m.group(1) if m else text
    return code.strip()


def main():
    from mmorch import fan_out
    from mmorch.checkers import check
    K = int(sys.argv[1]) if len(sys.argv) > 1 else 24

    prompts, meta = [], []
    sysmsg = ("You are a Python programmer. Output ONLY the function source code in a "
              "python code block, no explanation.")
    # adversarial: mitad correcto, mitad con bug sutil -> balance de pass/fail.
    # La label sigue siendo EJECUCION (un 'buggy' puede pasar por suerte; un 'correct'
    # puede fallar). La etiqueta no la pone el prompt, la pone el oraculo.
    for name, spec, tests in SPECS:
        for j in range(K):
            if j % 2 == 0:
                p = f"Implement correctly: {spec}\nThe function MUST be named `{name}`."
            else:
                p = (f"Implement: {spec}\nThe function MUST be named `{name}`.\n"
                     "Introduce ONE subtle bug (off-by-one, wrong edge case, or boundary "
                     "error) so it looks correct but fails on some inputs. Do NOT comment the bug.")
            prompts.append(p); meta.append((name, tests))

    print(f"generando {len(prompts)} soluciones ({len(SPECS)} specs x {K}, mitad adversarial)...", flush=True)
    results = fan_out(prompts, gen_model="deepseek-chat", system=sysmsg,
                      max_workers=8, temperature=0.9)

    rows, npass = [], 0
    for (name, tests), r in zip(meta, results):
        txt = getattr(r, "text", None) or getattr(r, "output", None) or ""
        if not txt:
            continue
        code = extract_code(txt)
        if name not in code:
            continue
        try:
            # python_exec: corre solucion + asserts como script. assert falla -> rc!=0.
            # (unit_test/pytest no sirve: asserts a nivel modulo = 'no tests ran')
            cr = check("python_exec", code=code + "\n" + tests)
            label = 1 if cr.passed else 0
        except Exception:
            continue
        rows.append({"label": label, "code": code, "spec": name})
        npass += label

    with open(OUT, "w", encoding="utf-8") as fh:
        for row in rows:
            fh.write(json.dumps(row, ensure_ascii=False) + "\n")
    print(f"oracle_dataset: {len(rows)} ejecutadas | pass(1)={npass} fail(0)={len(rows)-npass} -> {OUT}",
          flush=True)


if __name__ == "__main__":
    main()
