"""Aristas indirectas (ticket 04): un caso por mecanismo medido, cache y union con el informe."""
import json

from mmorch import impacto as I
from mmorch import impacto_indirecto as II


def _repo(tmp_path, files):
    for rel, body in files.items():
        p = tmp_path / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(body, encoding="utf-8")
    return tmp_path


def _rep(root, writer):
    files = [p for p in I._source_files(root, (".py",)) if not I._is_test(p.relative_to(root).as_posix())]
    return II.report(II.all_facts(root, files, root / "cache.json"), writer)


def test_kwarg_sigue_un_salto_hasta_la_invocacion(tmp_path):
    root = _repo(tmp_path, {
        "signals.py": "def collect():\n    return 0.5\n",
        "promote.py": "def decide(signal_fn):\n    return signal_fn() > 0.3\n",
        "nightly.py": "from promote import decide\nfrom signals import collect\n\n\ndef run():\n    return decide(signal_fn=collect)\n"})
    assert "`collect` en nightly.py:6 se pasa a `decide` (`signal_fn=`); se invoca en promote.py:2" in _rep(root, "signals.py")


def test_registro_por_decorador(tmp_path):
    root = _repo(tmp_path, {
        "registry.py": "_R = {}\n\n\ndef check(name):\n    def deco(fn):\n        _R[name] = fn\n        return fn\n"
                       "    return deco\n\n\ndef run_all(ctx):\n    return {n: f(ctx) for n, f in _R.items()}\n",
        "checks.py": "from registry import check\n\n\n@check('saldo')\ndef saldo(ctx):\n    return True\n"})
    assert "`saldo` (linea 5) se registra con `@check` en `_R` (registry.py); el registro se usa en registry.py:12" \
        in _rep(root, "checks.py")


def test_getattr_con_prefijo_y_atributo_asignado(tmp_path):
    root = _repo(tmp_path, {
        "handlers.py": "def on_created(e):\n    return 1\n",
        "bus.py": "import handlers\n\n\ndef dispatch(e):\n    return getattr(handlers, 'on_' + e)(e)\n",
        "alerts.py": "def notify(exc):\n    return True\n",
        "server.py": "class S:\n    def handle(self, exc):\n        self.on_error(exc)\n",
        "wiring.py": "from alerts import notify\n\n\ndef build(s):\n    s.on_error = notify\n"})
    assert '`"on_"` en bus.py:5 arma nombres de funciones de handlers.py con `getattr`' in _rep(root, "handlers.py")
    assert "`notify` en wiring.py:5 se asigna a `.on_error`, que se invoca en server.py:3" in _rep(root, "alerts.py")


def test_default_con_alias_y_contenedor(tmp_path):
    root = _repo(tmp_path, {
        "clock.py": "def now():\n    return 1\n",
        "sched.py": "from clock import now as _now\n\n\ndef due(last, now_fn=_now):\n    return now_fn() > last\n",
        "fmt.py": "def to_csv(rows):\n    return ''\n",
        "export.py": "from fmt import to_csv\n\nFORMATS = {'csv': to_csv}\n\n\ndef out(r, f):\n    return FORMATS[f](r)\n"})
    assert "`now` (alias `_now`) en sched.py:4 valor por defecto de `now_fn`; se invoca en sched.py:5" in _rep(root, "clock.py")
    assert "`to_csv` en export.py:3 queda en `FORMATS`, que se usa en export.py:7" in _rep(root, "fmt.py")


def test_contenedor_local_lista_sus_usos(tmp_path):
    root = _repo(tmp_path, {
        "stages.py": "def build():\n    return 1\n",
        "run.py": "from stages import build\n\n\ndef go():\n    steps = [build]\n    return [s() for s in steps]\n"})
    assert "`build` en run.py:5 queda en `steps`, que se usa en run.py:6" in _rep(root, "stages.py")


def test_llamada_directa_y_parametro_sombreado_no_cuentan(tmp_path):
    root = _repo(tmp_path, {
        "a.py": "def run():\n    return 1\n\n\ndef other(run):\n    return run\n",
        "b.py": "from a import run\n\n\ndef go():\n    return run()\n"})
    assert _rep(root, "a.py") == ""


def test_cache_reusa_hechos_y_detecta_cambios(tmp_path, monkeypatch):
    root = _repo(tmp_path, {"a.py": "def f():\n    return 1\n", "b.py": "from a import f\n\nX = [f]\n"})
    antes = _rep(root, "a.py")
    assert set(json.loads((root / "cache.json").read_text(encoding="utf-8"))) == {"a.py", "b.py"}
    orig, parseados = II.facts, []
    monkeypatch.setattr(II, "facts", lambda src, rel: parseados.append(rel) or orig(src, rel))
    assert _rep(root, "a.py") == antes and parseados == []          # tibio: todo sale del cache
    (root / "b.py").write_text("from a import f\n\nXS = [f, f]\n", encoding="utf-8")
    assert "queda en `XS`" in _rep(root, "a.py") and parseados == ["b.py"]


def test_report_de_impacto_suma_literales_e_indirectas(tmp_path, monkeypatch):
    monkeypatch.setattr(I.tempfile, "gettempdir", lambda: str(tmp_path / "tmp"))
    (tmp_path / "tmp").mkdir()
    root = _repo(tmp_path / "repo", {
        "signals.py": 'PATH = "signals.jsonl"\n\n\ndef collect():\n    return 0.5\n',
        "reader.py": 'P = "signals.jsonl"\n',
        "promote.py": "def decide(signal_fn):\n    return signal_fn()\n",
        "nightly.py": "from promote import decide\nfrom signals import collect\n\nR = decide(signal_fn=collect)\n"})
    r = I.report(root, root / "signals.py")
    assert r.index("INFORME DE IMPACTO (automatico") < r.index("INFORME DE IMPACTO INDIRECTO")
    assert "reader.py" in r and "se invoca en promote.py:2" in r
    assert I.report(root, root / "reader.py").count("INDIRECTO") == 0
