# Inventario gates existentes
Type: research
Status: resolved
Blocked by: 
Map: ../map.md

## Question

Inventario de lo que mmorch YA tiene para gates, para que el contrato del ticket 02 componga y no reinvente: `gate_policy.py` (stage/policy/history), `checkers.py` (21 oráculos registrados), `synth_store.py` (checkers sintetizados, evidencia), los gates de `project_integrate.py` (unit gate, cold verifier, integration gate), `hardening.py`, y los hooks. Para cada uno: qué entrada recibe, qué decide, si es determinista/sintetizado/juicio, y si tiene número medido. Salida: tabla en `research/01-inventario-gates.md` y un párrafo con los huecos frente a las 6 etapas.

## Answer

mmorch tiene 20 gates o chequeos en código. 21 oráculos en `checkers.py`, todos deterministas y sin número propio. `synth_store` guarda 57 tipos promovidos, pero ningún gate del engine lo consume y la promoción vive en `ablation_synth_kinds.py`, no en el paquete. El engine `/project` cubre build (stub_check, gate frío, breakers) y test (gate de integración solo con `external_test`); su único juicio LLM es la sonda fría advisory, sin medir. Intent, spec y PR tienen cero gates; plan tiene solo estructura (`validate_worklist`, allowlist de `test_cmd`); `gate_policy` es aprobación humana sin evaluar contenido. Tabla completa y observaciones para el contrato: [research/01-inventario-gates.md](../research/01-inventario-gates.md).
