# Semilla DÉBIL de asserts para mutation-score de mmorch/signature.py (target autoresearch nocturno).
# NO es test de la suite (mut_*, no test_*) — es el material vivo que el loop FORTALECE cada noche
# para matar más mutantes. Arranca casi vacío A PROPÓSITO (headroom real): 1 assert flojo. El loop
# agrega asserts sobre op_type/grounding/bits de más casos, subiendo mutation_score.
# El scorer (scripts/score_mutation.py) ejecuta este archivo como gate + corre mutation_score.
from mmorch.signature import signature

_s = signature("resolvé def foo(x)")
assert _s.op_type == "GENERATE"   # 1 solo assert débil -> muchos mutantes sobreviven -> headroom
