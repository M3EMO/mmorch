# supervision.md — driver_py


## candidato 13:30:35
revision de spec (Claude, rc=0): 0 preguntas.

## candidato 13:42:30
revision del diff (Claude, rc=0): All tests pass — the diff matches the spec exactly (order R4→R2→R3, exact signatures, error message contents, self-check intact). No demonstrable defect found.

OK

## candidato 13:48:30
revision del diff (Claude, rc=0): BLOCK: `validate_steps` levanta `TypeError`, no `ValueError`. Pasa cuando un step trae `"loop_back": None`. La clave existe. R3 debe aplicar igual. Falta chequear el tipo antes de comparar `lb < 0`.
