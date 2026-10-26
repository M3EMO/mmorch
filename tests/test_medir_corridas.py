"""scripts/medir_corridas.py: tramos de tamano-plan y union veredicto -> run-log (orchestration-amp, orchestration-1yu)."""
import importlib.util
import json
import pathlib

_spec = importlib.util.spec_from_file_location("medir_corridas", pathlib.Path(__file__).parents[1] / "scripts/medir_corridas.py")
M = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(M)

OK = [{"stage": "6-pr", "ok": True}]


def test_tamano_cuenta_exito_sin_claude_por_tramo():
    logs = [{"plan_tamano": {"archivos": 1}, "stages": OK},
            {"plan_tamano": {"archivos": 1}, "stages": OK, "escalated_to_claude": True},
            {"plan_tamano": {"archivos": 5}, "stages": OK},
            {"plan_tamano": {"archivos": 9}, "stages": [], "failed_stage": "4-build"},
            {"stages": OK}]
    t = M.tamano(logs)
    assert t["tramos"] == {"1": [1, 2], "2-3": [0, 0], "4-7": [1, 1], "8+": [0, 1]} and t["grandes"] == 2


def test_veredictos_se_unen_por_branch_y_cobertura_usa_el_peor(tmp_path, capsys):
    (tmp_path / "run-log-f1.json").write_text(json.dumps({"mutation_score": 1.0, "cobertura_nueva": {"a.py": 1.0, "b.py": 0.5}}))
    (tmp_path / "run-log-f2.json").write_text(json.dumps({"mutation_score": 0.4}))
    (tmp_path / "veredictos.jsonl").write_text("\n".join(json.dumps(v) for v in (
        {"branch": "sdlc/f1", "label": "aprobado"}, {"branch": "sdlc/f2", "label": "rechazado"},
        {"branch": "sdlc/sin-log", "label": "aprobado"})))
    por_fase = {"f1": json.loads((tmp_path / "run-log-f1.json").read_text()), "f2": json.loads((tmp_path / "run-log-f2.json").read_text())}
    v = [json.loads(x) for x in (tmp_path / "veredictos.jsonl").read_text().splitlines()]
    assert M.pares_veredicto(v, por_fase, "mutation_score") == [(1.0, True), (0.4, False)]
    assert M.pares_veredicto(v, por_fase, "cobertura_nueva") == [(0.5, True)]
    assert M.precision_por_umbral([(1.0, True), (0.4, False)], (0.5, 1.0)) == {0.5: (1.0, 1.0), 1.0: (1.0, 1.0)}
    assert M.main([str(tmp_path)]) == 0 and "faltan 20 corridas" in capsys.readouterr().out
