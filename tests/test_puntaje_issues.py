"""scripts/puntaje_issues.py: escala de esfuerzo, formula de impacto y reparto de lineas entre issues."""
import importlib.util
import json
import pathlib

_spec = importlib.util.spec_from_file_location("puntaje_issues", pathlib.Path(__file__).parents[1] / "scripts/puntaje_issues.py")
P = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(P)


def test_escalas():
    assert P.esfuerzo(0) == 0 and P.esfuerzo(2500) == 5 and P.esfuerzo(10**6) == 5
    assert 1.3 < P.esfuerzo(50) < 1.5 and P.esfuerzo(50, extra=1.0) == round(P.esfuerzo(50) + 1, 2)
    assert P.impacto(5, 5, 5) == 5 and P.impacto(0, 0, 5) == 1 and P.impacto(4, 3, 2) == 3.2


def test_commit_compartido_reparte_y_el_id_es_palabra_entera():
    commits = [("a", "fix (orchestration-xdi, orchestration-eeq)", 100), ("b", "eeq: test", 10),
               ("c", "amplia el reporte", 50), ("d", "ver amp", 8)]
    assert P.lineas_por_issue(["xdi", "eeq", "amp"], commits) == {"xdi": 50, "eeq": 60, "amp": 8}


def test_puntajes_json_completo():
    for i, p in json.loads(P.PUNTAJES.read_text(encoding="utf-8")).items():
        assert {"titulo", "estado", "V", "A", "E"} <= set(p) and all(0 <= p[k] <= 5 for k in "VAE"), i
