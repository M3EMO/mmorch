"""Tests de stuck_detector (reglas deterministas, cero LLM, cero IO)."""

from mmorch.stuck_detector import stuck_findings

MUERTO = {"evolve": {"findings": 3, "opened": [], "red": ["mmorch/a.py"]}}
VIVO = {"evolve": {"findings": 3, "opened": ["mmorch/a.py"], "red": []}}
PLANO = {"autoresearch": {"target": "prompts/x.txt", "improved": False}}
MEJORO = {"autoresearch": {"target": "prompts/x.txt", "improved": True}}


def test_evolve_bucle_muerto_tras_n_noches():
    out = stuck_findings([MUERTO] * 6)
    assert any("bucle muerto" in f and "6 noches" in f for f in out)


def test_racha_corta_no_dispara():
    assert stuck_findings([MUERTO] * 4) == []


def test_pr_abierto_corta_la_racha_y_desmarca_el_modulo():
    # la noche final abre PR para a.py: ni bucle muerto ni modulo cronico
    assert stuck_findings([MUERTO] * 6 + [VIVO]) == []


def test_autoresearch_plano_dispara_y_mejora_resetea():
    assert any("autoresearch plano" in f for f in stuck_findings([PLANO] * 5))
    assert stuck_findings([PLANO] * 5 + [MEJORO]) == []


def test_modulo_cronico_no_consecutivo():
    """El modulo cronico cuenta apariciones, no racha: 5 de 8 noches alcanza
    aunque haya noches limpias en el medio (y sin bucle muerto: hay PRs de
    OTRO modulo, asi que la regla 1 no aplica pero la 3 si)."""
    rojo = {"evolve": {"findings": 2, "opened": ["mmorch/otro.py"],
                       "red": ["mmorch/b.py"]}}
    limpio = {"evolve": {"findings": 1, "opened": ["mmorch/otro.py"], "red": []}}
    out = stuck_findings([rojo, limpio, rojo, rojo, limpio, rojo, rojo, limpio])
    assert any("cronico mmorch/b.py" in f for f in out)


def test_historia_vacia_y_records_sin_campos():
    assert stuck_findings([]) == []
    assert stuck_findings([{}, {"ts": 1}, {"otro": {}}] * 4) == []


def test_prefijo_estable_para_la_firma_de_retry():
    """auto_repair._sig agrupa por source + detail[:80]: el numero de noches
    (que cambia cada dia) NO debe caer dentro de los primeros 80 chars del
    finding de bucle muerto... o al menos el prefijo debe ser identico entre
    noches consecutivas para que el retry window de 5 dias funcione."""
    f6 = next(f for f in stuck_findings([MUERTO] * 6) if "bucle muerto" in f)
    f7 = next(f for f in stuck_findings([MUERTO] * 7) if "bucle muerto" in f)
    assert f6[:80] == f7[:80]
