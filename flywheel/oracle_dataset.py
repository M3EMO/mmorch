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
    # --- scale-up 2026-06-15: +20 specs (arity mix, floats, nested, tie-handling) --- #
    ("factorial", "factorial(n) returns n! (product of 1..n), with factorial(0)=1.",
     "assert factorial(0)==1\nassert factorial(1)==1\nassert factorial(5)==120\nassert factorial(6)==720"),
    ("reverse_string", "reverse_string(s) returns the string s reversed.",
     "assert reverse_string('abc')=='cba'\nassert reverse_string('')==''\nassert reverse_string('a')=='a'"),
    ("is_prime", "is_prime(n) returns True if n is a prime number (n>=2), else False.",
     "assert is_prime(2)\nassert is_prime(13)\nassert not is_prime(1)\nassert not is_prime(15)\nassert not is_prime(0)"),
    ("sum_list", "sum_list(lst) returns the sum of a list of numbers (empty list -> 0).",
     "assert sum_list([1,2,3])==6\nassert sum_list([])==0\nassert sum_list([-1,1])==0"),
    ("fizzbuzz", "fizzbuzz(n) returns a list for 1..n: 'Fizz' if divisible by 3, 'Buzz' if by 5, 'FizzBuzz' if both, else the number as a string.",
     "assert fizzbuzz(5)==['1','2','Fizz','4','Buzz']\nassert fizzbuzz(3)==['1','2','Fizz']\nassert fizzbuzz(15)[-1]=='FizzBuzz'"),
    ("caesar", "caesar(s,k) shifts each ASCII letter in s forward by k positions (wrapping within its case), leaving non-letters unchanged.",
     "assert caesar('abc',1)=='bcd'\nassert caesar('xyz',3)=='abc'\nassert caesar('Hello, World!',0)=='Hello, World!'"),
    ("all_unique", "all_unique(s) returns True if all characters in the string s are unique.",
     "assert all_unique('abc')\nassert not all_unique('aba')\nassert all_unique('')"),
    ("second_largest", "second_largest(lst) returns the second largest DISTINCT value in the list.",
     "assert second_largest([1,2,3])==2\nassert second_largest([5,5,4])==4\nassert second_largest([1,2,2,3,3])==2"),
    ("to_binary", "to_binary(n) returns the binary representation of a non-negative integer as a string without any prefix.",
     "assert to_binary(0)=='0'\nassert to_binary(5)=='101'\nassert to_binary(255)=='11111111'"),
    ("hamming", "hamming(a,b) returns the number of positions where two EQUAL-LENGTH strings differ.",
     "assert hamming('abc','abd')==1\nassert hamming('abc','abc')==0\nassert hamming('','')==0"),
    ("mode", "mode(lst) returns the most frequent element; on ties return the smallest such element.",
     "assert mode([1,2,2,3])==2\nassert mode([1,1,2,2])==1\nassert mode([5])==5"),
    ("is_leap", "is_leap(year) returns True if year is a leap year in the Gregorian calendar.",
     "assert is_leap(2000)\nassert not is_leap(1900)\nassert is_leap(2024)\nassert not is_leap(2023)"),
    ("clamp", "clamp(x,lo,hi) returns x bounded to the inclusive range [lo,hi].",
     "assert clamp(5,0,10)==5\nassert clamp(-3,0,10)==0\nassert clamp(99,0,10)==10"),
    ("transpose", "transpose(matrix) returns the transpose of a rectangular list-of-lists ([] -> []).",
     "assert transpose([[1,2,3],[4,5,6]])==[[1,4],[2,5],[3,6]]\nassert transpose([[1],[2]])==[[1,2]]\nassert transpose([])==[]"),
    ("running_max", "running_max(lst) returns the list of prefix maxima (running_max[i]=max(lst[:i+1])).",
     "assert running_max([1,3,2,5,4])==[1,3,3,5,5]\nassert running_max([])==[]\nassert running_max([2])==[2]"),
    ("count_substr", "count_substr(s,sub) returns the number of NON-overlapping occurrences of non-empty sub in s.",
     "assert count_substr('ababab','ab')==3\nassert count_substr('aaa','aa')==1\nassert count_substr('abc','x')==0"),
    ("to_snake", "to_snake(s) converts camelCase to snake_case: lowercase the first letter (no leading underscore) and prefix '_' before each subsequent uppercase letter, lowercased.",
     "assert to_snake('camelCase')=='camel_case'\nassert to_snake('HTTPServer')=='h_t_t_p_server'\nassert to_snake('x')=='x'"),
    ("digital_root", "digital_root(n) repeatedly sums the decimal digits of a non-negative integer until a single digit remains.",
     "assert digital_root(0)==0\nassert digital_root(38)==2\nassert digital_root(9999)==9"),
    ("median", "median(lst) returns the median of a non-empty list of numbers (average of the two middle values if the length is even).",
     "assert median([1,2,3])==2\nassert median([1,2,3,4])==2.5\nassert median([5])==5"),
    ("is_pangram", "is_pangram(s) returns True if s contains every letter of the English alphabet at least once, case-insensitive.",
     "assert is_pangram('The quick brown fox jumps over the lazy dog')\nassert not is_pangram('hello')\nassert is_pangram('abcdefghijklmnopqrstuvwxyz')"),
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
