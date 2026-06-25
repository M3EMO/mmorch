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
