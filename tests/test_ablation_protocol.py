"""El protocolo del verificador de las ablaciones tiene que seguir vivo.

Por que existe este test (medido 2026-09-04): la ablacion pareada se escribio el
2026-06-08 pidiendo el formato legacy {"passed": bool}. El 2026-08-28 se endurecio
_parse_verdict (commit 3897345) para que ese formato SOLO pueda refutar -- un fix de
seguridad correcto contra un verificador hijackeado. Nadie noto que rompia la ablacion:
al re-correrla dio especificidad 0.0 en AMBOS brazos y cero pares discordantes en 348
items, o sea el parser refutaba todo y el experimento medía el arnes, no a los modelos.

La leccion no es "persistir resultados" (eso ya se arreglo): es que un arnes de
experimentos que no corre seguido se pudre en silencio. Estos tests son $0 (sin API) y
fallan si el contrato entre el prompt de la ablacion y el parser vuelve a divergir.
"""
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

from mmorch.patterns import _parse_verdict  # noqa: E402


def _verify_sys():
    from ablation_paired import _VERIFY_SYS
    return _VERIFY_SYS


def test_el_prompt_pide_el_vocabulario_que_el_parser_acepta():
    """Si el prompt pide un formato que el parser no puede aprobar, el experimento
    devuelve especificidad 0 y parece un hallazgo sobre los modelos. No lo es."""
    sys_prompt = _verify_sys()
    assert "verdict" in sys_prompt, "el prompt debe pedir 'verdict' (vocabulario anclado)"
    for label in ("correcto", "incorrecto_menor", "incorrecto_grave"):
        assert label in sys_prompt, f"falta el ancla '{label}' en el prompt"
    assert '"passed"' not in sys_prompt, (
        "el prompt sigue pidiendo el formato legacy {passed: bool}, que _parse_verdict "
        "solo puede REFUTAR -> el verificador nunca podria aprobar")


def test_el_parser_puede_aprobar_y_refutar_con_ese_vocabulario():
    """El bug se veia como 'todo refutado'. Un parser que no puede decir True hace que
    cualquier ablacion de sensibilidad 1.0 / especificidad 0.0, sin importar el modelo."""
    ok, _, _ = _parse_verdict('{"verdict": "correcto", "refutations": []}')
    assert ok is True, "el parser tiene que poder APROBAR con el vocabulario anclado"

    for bad in ("incorrecto_menor", "incorrecto_grave"):
        no, _, _ = _parse_verdict(f'{{"verdict": "{bad}", "refutations": ["x"]}}')
        assert no is False, f"'{bad}' tiene que refutar"


def test_el_formato_legacy_sigue_sin_poder_aprobar():
    """Guarda el fix de seguridad del 2026-08-28: un verificador que emite el formato
    viejo NO puede aprobar. Este test protege la razon por la que el prompt cambio."""
    ok, _, _ = _parse_verdict('{"passed": true, "confidence": 0.9, "refutations": []}')
    assert ok is False, "el formato legacy jamas debe aprobar (anti-hijack)"
