"""autoresearch (r4a): hillclimb como job declarativo. gen_fn/run_fn inyectados (cero API).
Verifica optimizacion de un archivo hacia una metrica, journal, keep del best, parse y resume."""
import sys, pathlib, json
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
from mmorch.autoresearch import run_autoresearch, parse_metric, resume_from_journal


def test_parse_metric_ok_and_raises():
    assert parse_metric("blah score: 3.5 end", r"score[:=]\s*([-\d.]+)") == 3.5
    assert parse_metric("val=42", r"val=([-\d.]+)") == 42.0
    try:
        parse_metric("no number here", r"score[:=]\s*([-\d.]+)"); assert False
    except ValueError:
        pass


def test_optimizes_file_keeps_best_and_journals(tmp_path):
    f = tmp_path / "x.txt"
    f.write_text("100", encoding="utf-8")
    jp = tmp_path / "exp.jsonl"
    seq = iter(["50", "30", "80", "20"])               # 80 es peor: debe descartarse
    def gen(model, prompt):
        return "```\n" + next(seq) + "\n```"
    def run(cmd):
        return f"score: {f.read_text(encoding='utf-8').strip()}"
    r = run_autoresearch("bajá el número", "x.txt", "scorer", cwd=str(tmp_path),
                         maximize=False, max_rounds=4, patience=9,
                         journal_path=str(jp), gen_fn=gen, run_fn=run)
    assert r.baseline == 100.0
    assert r.best_score == 20.0                          # el menor visto
    assert f.read_text(encoding="utf-8").strip() == "20"  # best vuelve al archivo (keep)
    lines = [json.loads(l) for l in jp.read_text(encoding="utf-8").splitlines() if l.strip()]
    assert len(lines) == 4
    assert [x["improved"] for x in lines] == [True, True, False, True]  # 80 no mejoró


def test_resume_from_journal(tmp_path):
    jp = tmp_path / "j.jsonl"
    jp.write_text(
        json.dumps({"round": 1, "best_score": 100, "improved": True}) + "\n" +
        json.dumps({"round": 2, "best_score": 60, "improved": True}) + "\n",
        encoding="utf-8")
    rounds, best = resume_from_journal(jp)
    assert rounds == 2 and best == 60


def test_resume_no_journal(tmp_path):
    rounds, best = resume_from_journal(tmp_path / "nope.jsonl")
    assert rounds == 0 and best is None


def test_feedback_de_fallos_especificos_llega_a_la_proxima_ronda(tmp_path):
    """Antes: 'detail' solo se poblaba si score() TIRABA excepcion — un score
    valido pero imperfecto (el caso normal, 0.8889 por ej) nunca le daba
    contexto a la siguiente ronda. Medido: autoresearch optimizaba a ciegas
    15+ noches. Ahora score() captura las lineas FAIL del output crudo del
    scorer y se las pasa a la siguiente propuesta."""
    f = tmp_path / "prompt.txt"
    f.write_text("prompt v1", encoding="utf-8")
    prompts_vistos = []

    def gen(model, prompt):
        prompts_vistos.append(prompt)
        return "```\nprompt v2\n```"

    salidas = iter([
        "FAIL tarea 5 (compress): solo comprime si acorta\nscore: 0.8889",
        "score: 1.0",
    ])

    def run(cmd):
        return next(salidas)

    run_autoresearch("mejora el prompt", "prompt.txt", "scorer", cwd=str(tmp_path),
                     maximize=True, max_rounds=2, patience=9,
                     gen_fn=gen, run_fn=run)
    # la 2da ronda (unico prompt propuesto tras la 1ra, ya que max_rounds=2)
    # debe ver el detalle especifico de la tarea que fallo en la 1ra
    assert any("compress" in p and "solo comprime si acorta" in p
              for p in prompts_vistos), prompts_vistos
