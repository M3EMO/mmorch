"""SDD fase 2, rama de deltas (tickets 01 a 06 de .scratch/sdd-fase-2/): spec viva por area, IDs locales al area,
caracterizacion, delta ADDED/MODIFIED/REMOVED con gate y fusion por codigo. Cada test es el aprobado del ticket; los
comentarios `# n.` son sus casos numerados. Cero API: Claude, el writer y bd simulados."""
import subprocess

import pytest

import sdlc.areas as A
import sdlc.pipeline as S
import sdlc_juguete as J

CALC = "docs/specs/calc/spec.md"


def _resolver(repo, monkeypatch, files, task="tarea"):
    J.conf(repo, monkeypatch, files=files, task=task)
    return S._resolver_area()


# ---------------------------------------------------------------- ticket 01


def test_spec_base_de_una_tarea(tmp_path, monkeypatch):
    repo = J.repo(tmp_path)
    # 1. files dentro de un area
    assert _resolver(repo, monkeypatch, ["pkg/a.py"]) == CALC and S.state["area"] == "calc"
    # 2. sin files: camino de hoy
    assert _resolver(repo, monkeypatch, []) is None and "area" not in S.state
    # 3. files que no calzan con ningun glob
    assert _resolver(repo, monkeypatch, ["otro/x.py"]) is None
    # 4. files en dos areas: corte con division.md
    with pytest.raises(S.StageFailed, match="areas"):
        _resolver(repo, monkeypatch, ["pkg/a.py", "lib/b.py"])
    div = (repo / "docs/sdlc/division.md").read_text(encoding="utf-8")
    assert "## Area calc" in div and "`pkg/a.py`" in div and "## Area libs" in div and "`lib/b.py`" in div
    # 8. otro orden, el mismo resultado
    (repo / "docs/sdlc/division.md").unlink()
    with pytest.raises(S.StageFailed, match="areas"):
        _resolver(repo, monkeypatch, ["lib/b.py", "pkg/a.py"])
    assert (repo / "docs/sdlc/division.md").read_text(encoding="utf-8") == div
    # 7. un area mas un archivo suelto
    assert _resolver(repo, monkeypatch, ["pkg/a.py", "otro/x.py"]) == CALC
    # 9. sin files, ruta en el texto de la tarea
    assert _resolver(repo, monkeypatch, [], task="cambiar sdlc/pipeline.py para que lea areas") == "docs/specs/builder/spec.md"
    # 10. files en X y una ruta de Y en el texto: gana X
    assert _resolver(repo, monkeypatch, ["pkg/a.py"], task="igual que lib/b.py") == CALC
    # 11. sin files, rutas de dos areas en el texto
    with pytest.raises(S.StageFailed, match="areas"):
        _resolver(repo, monkeypatch, [], task="tocar pkg/a.py y lib/b.py")
    # [spec_de_area]: la spec de un area puede vivir fuera del default (Adepor: specs planas)
    (repo / "sdlc.toml").write_text(J.TOML + '\n[spec_de_area]\ncalc = "docs/specs/01-calc.md"\n', encoding="utf-8")
    assert _resolver(repo, monkeypatch, ["pkg/a.py"]) == "docs/specs/01-calc.md"


def test_spec_base_dos_areas_corta_sin_llm(tmp_path, monkeypatch):
    # 5. (de test_spec_base_de_una_tarea) el corte ocurre antes de cualquier llamada al LLM
    repo = J.repo(tmp_path)
    monkeypatch.setattr(S, "RUNS", tmp_path / "runs")
    llamadas = []
    monkeypatch.setattr(S, "llm", lambda *a, **k: llamadas.append(a) or "")
    monkeypatch.setattr(S, "_revisor", lambda *a, **k: llamadas.append(a) or {"ok": True, "result": ""})
    r = S.build_feature("doble-area", "tarea", str(repo), files=["pkg/a.py", "lib/b.py"], wt=tmp_path / "wt")
    assert r["status"] == "escalate" and r["failed_stage"] == "0-areas" and llamadas == []
    assert "lib/b.py" in (tmp_path / "wt/docs/sdlc/division.md").read_text(encoding="utf-8")


def test_plan_fuera_de_area(tmp_path, monkeypatch):
    repo = J.repo(tmp_path)
    J.conf(repo, monkeypatch, files=["pkg/a.py"])
    S._resolver_area()
    bd = J.Bd()
    monkeypatch.setattr(S, "_bd", bd)
    ok, note = S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `lib/b.py` [R1]\n", ["pkg/a.py", "lib/b.py"])
    assert not ok and "lib/b.py" in note and "libs" in note
    assert bd.issues[0]["labels"] == ["sdlc-fuera-de-area:libs"] and "lib/b.py" in bd.issues[0]["notes"]
    # repo sin .beads: el bd de orchestration
    assert all(cwd == str(S._ORCH) for _, cwd in bd.llamadas)
    # segundo archivo de la misma area: append-notes sobre el mismo issue
    S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `lib/c.py` [R1]\n", ["pkg/a.py", "lib/c.py"])
    assert len(bd.issues) == 1 and "lib/c.py" in bd.issues[0]["notes"]
    assert any(a[0] == "update" and "--append-notes" in a for a, _ in bd.llamadas)
    # un archivo rechazado dos veces en la misma corrida se suma una sola vez
    n = len(bd.llamadas)
    S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `lib/c.py` [R1]\n", ["pkg/a.py", "lib/c.py"])
    assert len(bd.llamadas) == n
    assert S.state["fuera_de_area"] == {"libs": ["lib/b.py", "lib/c.py"]}
    assert "`lib/c.py` (area libs)" in (repo / "docs/sdlc/division.md").read_text(encoding="utf-8")
    # un plan con un archivo del area y un archivo sin area (dentro del techo) pasa
    (repo / "sdlc.toml").write_text(J.TOML.replace('"sdlc/**/*.py"]', '"sdlc/**/*.py", "suelto/*.py"]'), encoding="utf-8")
    J.conf(repo, monkeypatch, files=["pkg/a.py"])
    S._resolver_area()
    monkeypatch.setattr(S, "_bd", J.Bd())
    assert S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `suelto/x.py` [R1]\n", ["pkg/a.py", "suelto/x.py"])[0]
    # repo con .beads: el bd del repo destino
    (repo / ".beads").mkdir()
    bd2 = J.Bd()
    monkeypatch.setattr(S, "_bd", bd2)
    S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `lib/b.py` [R1]\n", ["pkg/a.py", "lib/b.py"])
    assert bd2.llamadas and all(cwd == str(repo) for _, cwd in bd2.llamadas)
    # un issue cerrado con la misma label no recibe notas: se crea uno nuevo
    J.conf(repo, monkeypatch, files=["pkg/a.py"])
    S._resolver_area()
    bd3 = J.Bd()
    bd3.issues.append({"id": "x-viejo", "labels": ["sdlc-fuera-de-area:libs"], "status": "closed", "notes": ""})
    monkeypatch.setattr(S, "_bd", bd3)
    S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `lib/b.py` [R1]\n", ["pkg/a.py", "lib/b.py"])
    assert bd3.issues[0]["notes"] == "" and len(bd3.issues) == 2
    # un bd que falla no rompe la corrida; division.md y fuera_de_area quedan escritos
    J.conf(repo, monkeypatch, files=["pkg/a.py"])
    S._resolver_area()
    (repo / "docs/sdlc/division.md").unlink()
    monkeypatch.setattr(S, "_bd", J.Bd(falla=True))
    ok, _ = S.gate_plan_allowlist("## Archivos\n- `pkg/a.py` [R1]\n- `lib/b.py` [R1]\n", ["pkg/a.py", "lib/b.py"])
    assert not ok and S.state["fuera_de_area"] == {"libs": ["lib/b.py"]}
    assert "lib/b.py" in (repo / "docs/sdlc/division.md").read_text(encoding="utf-8")


def test_chequeo_de_solapamiento(tmp_path):
    toml = 'accept_cmd = "x"\nfiles = ["a/**/*.py"]\n\n[areas]\na = ["a/*.py"]\nb = ["a/sub/*.py"]\n'
    repo = J.repo(tmp_path, toml=toml, archivos={"a/x.py": "", "a/sub/x.py": ""})
    assert A.solapados(repo, A.areas(S._toml(repo))) == {"a/sub/x.py": ["a", "b"]}


def _plan(files):
    return "## Archivos\n" + "".join(f"- `{f}` [R1]\n" for f in files) + "\n## Prueba\npytest\n"


def _area_lista_para_pr(repo, monkeypatch, *, base=None, delta=None, tests=None, name="feat x"):
    """Deja el worktree como al final de la etapa 5 en modo area: spec base, delta, plan y tests del area."""
    J.escribir(repo, {CALC: base or J.spec_area({"R1": "suma(a, b) devuelve a + b."}),
                      **dict([J.test_de("R1")]), **(tests or {})})
    J.commit(repo, "area calc")
    J.conf(repo, monkeypatch, files=["pkg/a.py"], name=name)
    S._resolver_area()
    S.state["base_sha"] = J.git(repo, "rev-parse", "--short", "HEAD").strip()
    S.state["plan_files"] = ["pkg/a.py"]
    J.escribir(repo, {"docs/sdlc/delta.md": delta or "# Delta\n\n## ADDED\n\n- R2: es_par(n) dice si n es par.\n",
                      "docs/sdlc/plan.md": _plan(["pkg/a.py"]), "docs/sdlc/spec.md": "mesa\n",
                      **dict([J.test_de("R2")]), "pkg/a.py": J.A + "\n\nX = 1\n"})
    monkeypatch.setattr(S, "gate_lint", lambda: (True, "0"))
    return S.state["base_sha"]


def test_archivo_con_area(tmp_path, monkeypatch):
    repo = J.repo(tmp_path)
    sha = _area_lista_para_pr(repo, monkeypatch)
    S.pr()
    assert (repo / f"docs/specs/calc/changes/feat_x-{sha}.md").exists()
    assert (repo / f"docs/specs/calc/changes/feat_x-{sha}.plan.md").exists()
    assert not (repo / "docs/specs/feat_x").exists()
    # un sdlc.toml sin areas mantiene el camino de hoy (test_sdlc_sdd.py::test_dos_features_dejan_dos_specs)
    otro = tmp_path / "sin"
    otro.mkdir()
    import types
    t = types.SimpleNamespace(name="Tope de Retiro", task="x", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(otro), "files": [], "suite": []}, wt=otro)
    S._write("docs/sdlc/spec.md", "spec A")
    S._archivar_spec()
    assert (otro / "docs/specs/tope_de_retiro/spec.md").read_text(encoding="utf-8") == "spec A\n"


def test_archivo_de_la_ficha(tmp_path, monkeypatch):
    # comentario del ticket 06 (orchestration-4u6): la etapa 6 archiva la ficha de la feature si existe
    repo = J.repo(tmp_path, archivos={"docs/sdlc/features/F1-escena.md": "ficha\n"})
    sha = _area_lista_para_pr(repo, monkeypatch, name="F1 escena")
    S.pr()
    assert (repo / f"docs/specs/calc/changes/f1_escena-{sha}.ficha.md").read_text(encoding="utf-8") == "ficha\n"
    sin = J.repo(tmp_path / "sin_area", toml='accept_cmd = "x"\nfiles = ["pkg/*.py"]\n',
                 archivos={"docs/sdlc/features/F1-escena.md": "ficha\n"})
    import types
    t = types.SimpleNamespace(name="F1 escena", task="x", accept_files={})
    S.configure(t, contract=[], feat={"repo": str(sin), "files": [], "suite": []}, wt=sin)
    S._archivar_spec()
    assert (sin / "docs/specs/f1_escena/ficha.md").read_text(encoding="utf-8") == "ficha\n"


# ---------------------------------------------------------------- ticket 02


def test_ids_sin_colision(tmp_path, monkeypatch):
    # 1. un ADDED en un area con R1, R2 y R4 toma R3
    assert A.libres({1, 2, 4}, 1) == [3]
    # 2. un area con R1 a R3 toma R4
    assert A.libres({1, 2, 3}, 1) == [4]
    # 3. dos ADDED sobre R1, R2, R4 y R5 toman R3 y R6
    assert A.libres({1, 2, 4, 5}, 2) == [3, 6]
    repo = J.repo(tmp_path)
    r1, t1 = J.test_de("R1", "from pkg.a import suma\n    assert suma(1, 2) == 3", slug="suma")
    r2, t2 = J.test_de("R2", slug="viejo")
    J.escribir(repo, {r1: t1, r2: t2})
    spec = A.regenerar(J.spec_area({"R1": "suma(a, b) devuelve a + b.", "R2": "otra regla."}), repo, "calc")
    # 4. un MODIFIED R1 resuelve el requisito de la spec del area y su test por ruta::test
    bloque, citas = A.resolver(spec, "R1")
    assert bloque == "- R1: suma(a, b) devuelve a + b." and citas == [f"{r1}::test_R1_suma"]
    # 5. una tabla que cita un test sin ruta falla y nombra el test
    errores = A.gate_traza_area(spec.replace(f"{r1}::test_R1_suma", "test_R1_suma"), repo)
    assert any("test_R1_suma" in e and "sin ruta" in e for e in errores)
    # 6. una cita ruta::test a un test inexistente falla y nombra la ruta
    errores = A.gate_traza_area(spec.replace(f"{r1}::test_R1_suma", "tests/sdlc/calc/no.py::test_R1_x"), repo)
    assert any("tests/sdlc/calc/no.py" in e for e in errores)
    assert A.gate_traza_area(spec, repo) == []
    # 7. un REMOVED R2 mueve su test a removidos/ y suma R2 al README.md
    J.commit(repo, "area")
    dst = A.mover(repo, "calc", r2, "R2", "REMOVED", "ya no aplica", "feat-abc1234")
    assert dst == "docs/specs/calc/removidos/test_R2_viejo.py" and not (repo / r2).exists()
    readme = (repo / "docs/specs/calc/removidos/README.md").read_text(encoding="utf-8")
    assert "- R2 (REMOVED, feat-abc1234): ya no aplica" in readme
    # 8. la suite `pytest tests` no recolecta el test movido
    out = subprocess.run([S.PY, "-m", "pytest", "tests", "--collect-only", "-q", "-p", "no:cacheprovider"], cwd=repo,
                         capture_output=True, text=True).stdout
    assert "test_R2_viejo" not in out and "test_R1_suma" in out
    # 9. un R2 reusado cita su test nuevo; el viejo sigue en removidos/
    r2b, t2b = J.test_de("R2", slug="nuevo")
    J.escribir(repo, {r2b: t2b})
    assert A.tests_por_id(repo, "calc")["R2"] == [f"{r2b}::test_R2_nuevo"]
    assert (repo / dst).exists()
    # 10. un R2 eliminado dos veces conserva los dos tests movidos
    J.escribir(repo, {r2: t2})
    dst2 = A.mover(repo, "calc", r2, "R2", "REMOVED", "otra vez", "feat-def5678")
    assert dst2 != dst and (repo / dst).read_text(encoding="utf-8") == t2 and (repo / dst2).exists()
    # 11. una corrida sin area mantiene los IDs por feature (los gates de hoy no cambian)
    assert S.gate_traza_spec("## Requisitos\n- R1: x\n\n## Trazabilidad\n| ID | tests |\n|---|---|\n| R1 | test_a |\n",
                             {"test_a"})[0]


# ---------------------------------------------------------------- ticket 03

CHAR_BIEN = "from pkg.a import suma, es_par\n\n\ndef test_R1_suma():\n    assert suma(1, 2) == 3\n    assert es_par(4) and not es_par(3)\n"


def _char(repo, monkeypatch, *cola, files=("pkg/a.py",), approve=None):
    J.conf(repo, monkeypatch, files=list(files), approve=approve)
    S._resolver_area()
    claude = J.Claude(repo, *cola)
    monkeypatch.setattr(S, "_revisor", claude)
    return claude


def test_caracterizacion(tmp_path, monkeypatch):
    repo = J.repo(tmp_path)
    rel = "tests/sdlc/calc/test_R1_base.py"
    # 1. pasa en HEAD y mata un mutante
    _char(repo, monkeypatch, {rel: CHAR_BIEN})
    S.caracterizar()
    assert S.state["caracterizacion"]["score"] > 0 and S.state["caracterizacion"]["tests"] == [rel]
    assert rel in S.TASK.accept_files
    # 11. los mutantes salen solo del archivo caracterizado (pkg/c.py tambien es del area)
    assert S.state["caracterizacion"]["archivos"] == ["pkg/a.py"]
    assert all(m["archivo"] == "pkg/a.py" for m in S.state["caracterizacion"]["mutantes"])
    # 12. tras la aprobacion, un coder que edita un test de caracterizacion falla gate_baseline
    S.snapshot_baseline()
    (repo / rel).write_text(CHAR_BIEN + "\n# tocado\n", encoding="utf-8")
    assert not S.gate_baseline()[0]
    (repo / rel).unlink()
    # 2. un test que falla en HEAD se rechaza y el gate nombra el test
    _char(repo, monkeypatch, {rel: "def test_R1_mal():\n    assert False\n"})
    with pytest.raises(S.StageFailed, match="test_R1_mal"):
        S.caracterizar()
    (repo / rel).unlink()
    # 3. un test vacuo no mata ningun mutante
    _char(repo, monkeypatch, {rel: "def test_R1_vacuo():\n    assert True\n"})
    with pytest.raises(S.StageFailed, match="no mata"):
        S.caracterizar()
    (repo / rel).unlink()
    # 4. con caracterizacion_min = 0.5, un puntaje de 0.3 (2 de 7) se rechaza
    (repo / "sdlc.toml").write_text(J.TOML.replace("approve_accept", "caracterizacion_min = 0.5\napprove_accept"), encoding="utf-8")
    _char(repo, monkeypatch, {rel: "from pkg.a import suma\n\n\ndef test_R1_suma():\n    assert suma(1, 2) == 3\n"})
    with pytest.raises(S.StageFailed, match="0.286"):
        S.caracterizar()
    (repo / rel).unlink()
    (repo / "sdlc.toml").write_text(J.TOML, encoding="utf-8")
    # 10. Claude que toca codigo u otro test en la etapa 0 rechaza la etapa (y se revierte)
    _char(repo, monkeypatch, {rel: CHAR_BIEN, "pkg/a.py": J.A + "Y = 2\n"})
    with pytest.raises(S.StageFailed, match="alcance"):
        S.caracterizar()
    assert (repo / "pkg/a.py").read_text(encoding="utf-8") == J.A
    _char(repo, monkeypatch, {rel: CHAR_BIEN, "tests/test_viejo.py": "def test_viejo():\n    assert 1\n"})
    with pytest.raises(S.StageFailed, match="alcance"):
        S.caracterizar()
    (repo / rel).unlink()
    # 13. los requisitos base toman el menor numero libre del area
    J.escribir(repo, {CALC: J.spec_area({"R1": "a.", "R2": "b."}, rutas=("pkg/c.py",)),
                      **dict([J.test_de("R1", slug="c1"), J.test_de("R2", slug="c2")])})
    J.commit(repo, "area con R1 y R2")
    claude = _char(repo, monkeypatch, {"tests/sdlc/calc/test_R4_base.py": CHAR_BIEN.replace("R1", "R4")})
    with pytest.raises(S.StageFailed, match="R3"):
        S.caracterizar()
    assert "R3" in claude.prompts[0]
    (repo / "tests/sdlc/calc/test_R4_base.py").unlink()
    _char(repo, monkeypatch, {"tests/sdlc/calc/test_R3_base.py": CHAR_BIEN.replace("R1", "R3")})
    S.caracterizar()
    assert S.state["caracterizacion"]["ids"] == ["R3"]
    (repo / "tests/sdlc/calc/test_R3_base.py").unlink()
    # 5. un archivo que Contrato ya nombra no se vuelve a caracterizar
    claude = _char(repo, monkeypatch, files=("pkg/c.py",))
    S.caracterizar()
    assert claude.prompts == [] and "caracterizacion" not in S.state
    # 9. sin area, la etapa 0 no corre
    J.conf(repo, monkeypatch, files=[])
    S._resolver_area()
    monkeypatch.setattr(S, "_revisor", claude)
    S.caracterizar()
    assert claude.prompts == []


ACEPT = "def test_R2_resta():\n    from pkg.a import resta\n    assert resta(3, 1) == 2\n"


def test_caracterizacion_una_parada(tmp_path, monkeypatch):
    # 6. (de test_caracterizacion) una sola parada aprueba caracterizacion y aceptacion juntas
    repo = J.repo(tmp_path)
    char, acc = "tests/sdlc/calc/test_R1_base.py", "tests/sdlc/calc/test_R2_resta.py"
    _char(repo, monkeypatch, {char: CHAR_BIEN}, {acc: ACEPT}, approve=True)
    r = S.run(1, 1)
    assert r["status"] == "awaiting_approval" and [s["stage"] for s in r["stages"]] == ["0-caracterizar", "1-aceptacion"]
    sup = (repo / "docs/sdlc/supervision.md").read_text(encoding="utf-8")
    assert sup.count("ESPERA_HUMANO") == 1 and char in sup and acc in sup
    # 8. tras la aprobacion, el writer escribe la spec base y cita cada test de caracterizacion con ruta::test
    J.conf(repo, monkeypatch, files=["pkg/a.py"], approve=True)
    escritos = []

    def writer(model, system, user, timeout=400):
        escritos.append(user)
        if "requisitos base" in user:
            return "- R1: suma(a, b) devuelve a + b; es_par(n) dice si n es par."
        return "# Delta\n\n## ADDED\n\n- R2: resta(a, b) devuelve a - b.\n"
    monkeypatch.setattr(S, "llm", writer)
    S.run(2, 2)
    base = (repo / CALC).read_text(encoding="utf-8")
    assert f"| R1 | {char}::test_R1_suma |" in base and "- `pkg/a.py`" in base
    assert "| R2 |" not in base  # el R2 del delta entra a la spec viva recien en la etapa 6
    assert "R2" in (repo / "docs/sdlc/spec.md").read_text(encoding="utf-8")  # spec fusionada para plan y build
    # 7. con approve_accept = false la corrida sigue sin parada
    repo2 = J.repo(tmp_path / "otro")
    _char(repo2, monkeypatch, {char: CHAR_BIEN}, {acc: ACEPT}, approve=False)
    r = S.run(1, 1)
    assert "awaiting_approval" not in r and all(s["ok"] for s in r["stages"])


# ---------------------------------------------------------------- ticket 04

BASE4 = J.spec_area({"R1": "suma(a, b) devuelve a + b.", "R2": "resta(a, b) devuelve a - b.", "R4": "es_par(n)."})
TESTS4 = {"R1": ["t.py::test_R1_x"], "R3": ["t.py::test_R3_x"]}


def _delta(added="", modified="", removed="", extra=""):
    out = "# Delta: feat\n\n"
    for sec, txt in (("ADDED", added), ("MODIFIED", modified), ("REMOVED", removed)):
        if txt:
            out += f"## {sec}\n\n{txt}\n"
    return out + extra


VALIDO = _delta(added="- R3: doble(x) devuelve 2x.", modified="- R1: suma(a, b) devuelve a + b, tambien con floats.",
                removed="- R2: la resta se mudo a otra area.")


def test_gate_delta(tmp_path, monkeypatch):
    # 1. delta valido
    assert A.gate_delta(BASE4, VALIDO, TESTS4) == []
    # 2. MODIFIED R9 inexistente
    assert any("R9" in e and "no existe" in e for e in A.gate_delta(BASE4, _delta(modified="- R9: x."), {}))
    # 3. ADDED R5: el menor numero libre es R3
    assert any("R3" in e for e in A.gate_delta(BASE4, _delta(added="- R5: x."), {"R5": ["t.py::test_R5_x"]}))
    # 4. un R1 en dos secciones
    assert any("dos secciones" in e for e in A.gate_delta(BASE4, _delta(modified="- R1: y.", removed="- R1: z."), TESTS4))
    # 5. ADDED R3 sin test ruta::test
    assert any("R3" in e and "sin test" in e for e in A.gate_delta(BASE4, _delta(added="- R3: x."), {}))
    # 6. REMOVED R2 sin motivo
    assert any("R2" in e and "motivo" in e for e in A.gate_delta(BASE4, _delta(removed="- R2:"), {}))
    # 7. el diff entre la base y la fusionada nombra exactamente R1, R2 y R3
    fus = A.fusionar(BASE4, VALIDO)
    assert A.ids_cambiados(BASE4, fus) == {"R1", "R2", "R3"} and A.chequeo_diff(BASE4, fus, VALIDO) == []
    # 8. una fusion que cambia R4 por error falla el chequeo
    rota = fus.replace("- R4: es_par(n).", "- R4: es_par(n) cambiado.")
    assert any("R4" in e for e in A.chequeo_diff(BASE4, rota, VALIDO))
    # 11. un encabezado desconocido falla
    assert any("MODIFED" in e for e in A.gate_delta(BASE4, "## MODIFED\n\n- R1: x.\n", TESTS4))
    # 15. MODIFIED con el mismo texto que la base
    assert any("MODIFIED sin cambio" in e for e in A.gate_delta(BASE4, _delta(modified="- R1: suma(a, b) devuelve a + b."), TESTS4))
    # 10 (ticket 07). un MODIFIED que solo agrega o quita la marca pasa
    assert A.gate_delta(BASE4, _delta(modified="- R1 [prop]: suma(a, b) devuelve a + b."), TESTS4) == []
    # 13. una corrida sin area no escribe delta ni corre este gate
    J.conf(J.repo(tmp_path / "sin", toml='accept_cmd = "x"\nfiles = ["pkg/*.py"]\n'), monkeypatch, files=["pkg/a.py"])
    S._resolver_area()
    monkeypatch.setattr(S, "llm", lambda *a, **k: "## Requisitos\n- R1: x\n\n## Trazabilidad\n| ID | tests |\n|---|---|\n| R1 | test_R1_a |\n")
    monkeypatch.setattr(S, "_test_names", lambda: {"test_R1_a"})
    S.spec()
    assert not (S.WT / "docs/sdlc/delta.md").exists()


def _area_tras_etapa1(repo, monkeypatch, ids_etapa1=("R3",), bajas=None):
    """Area calc con R1, R2 y R4; la etapa 1 dejo los tests de `ids_etapa1` y la lista de bajas."""
    J.escribir(repo, {CALC: BASE4, **dict(J.test_de(i) for i in ("R1", "R2", "R4"))})
    J.commit(repo, "area")
    J.conf(repo, monkeypatch, files=["pkg/a.py"])
    S._resolver_area()
    J.escribir(repo, dict(J.test_de(i, slug="nuevo") for i in ids_etapa1))
    S.state["ids_etapa1"] = list(ids_etapa1)
    S.state["bajas"] = dict(bajas or {})


def test_gate_delta_en_la_corrida(tmp_path, monkeypatch):
    repo = J.repo(tmp_path)
    _area_tras_etapa1(repo, monkeypatch, ids_etapa1=("R1", "R3"), bajas={"R2": "se mudo"})
    assert S.gate_delta(VALIDO)[0]
    # 14. cada test_R<n> de la etapa 1 corresponde a un ADDED o MODIFIED del delta
    ok, note = S.gate_delta(_delta(added="- R3: doble(x) devuelve 2x.", removed="- R2: se mudo."))
    assert not ok and "R1" in note
    # 9. la etapa 2b que edita la spec fusionada o la base rechaza la etapa
    S._write("docs/sdlc/delta.md", VALIDO)
    S._write("docs/sdlc/spec.md", A.fusionar(BASE4, VALIDO))
    J.commit(repo, "mesa")
    monkeypatch.setattr(S, "_revisor", J.Claude(repo, {"docs/sdlc/spec.md": "editada\n"}))
    with pytest.raises(S.StageFailed, match="alcance"):
        S.spec_review()
    monkeypatch.setattr(S, "_revisor", J.Claude(repo, {CALC: "editada\n"}))
    with pytest.raises(S.StageFailed, match="alcance"):
        S.spec_review()
    # 10. gate_clarificaciones encuentra ## Clarificaciones en el delta
    clar = VALIDO + "\n## Clarificaciones\n\n- P1: floats? | R: si | afecta: R1\n"
    monkeypatch.setattr(S, "_revisor", J.Claude(repo, {"docs/sdlc/delta.md": clar}))
    S.spec_review()
    assert "R3" in (repo / "docs/sdlc/spec.md").read_text(encoding="utf-8")
    # 12. el gate vuelve a correr tras la etapa 2b: un delta roto por la revision falla
    roto = clar.replace("## MODIFIED", "## MODIFED")
    monkeypatch.setattr(S, "_revisor", J.Claude(repo, {"docs/sdlc/delta.md": roto}, {"docs/sdlc/delta.md": roto}))
    with pytest.raises(S.StageFailed, match="MODIFED"):
        S.spec_review()


def test_etapa1_numeros_ofrecidos(tmp_path, monkeypatch):
    # 16 (ticket 04). la etapa 1 que nombra un test con un numero fuera de la lista ofrecida falla en su propio gate
    repo = J.repo(tmp_path)
    J.escribir(repo, {CALC: BASE4, **dict(J.test_de(i) for i in ("R1", "R2", "R4"))})
    J.commit(repo, "area")
    J.conf(repo, monkeypatch, files=["pkg/c.py"])
    S._resolver_area()
    rel, txt = J.test_de("R40", "assert False", slug="lejos")
    claude = J.Claude(repo, {rel: txt})
    monkeypatch.setattr(S, "_revisor", claude)
    with pytest.raises(S.StageFailed, match="R40"):
        S.aceptacion()
    assert "R3" in claude.prompts[0] and "R5" in claude.prompts[0]


# ---------------------------------------------------------------- ticket 05


def _area_r1245(repo, monkeypatch, approve=False):
    J.escribir(repo, {CALC: J.spec_area({f"R{i}": f"regla {i}." for i in (1, 2, 4, 5)}),
                      **dict(J.test_de(f"R{i}") for i in (1, 2, 4, 5))})
    J.commit(repo, "area")
    J.conf(repo, monkeypatch, files=["pkg/a.py"], approve=approve)
    S._resolver_area()


def test_suite_total_con_delta(tmp_path, monkeypatch):
    repo = J.repo(tmp_path)
    _area_r1245(repo, monkeypatch, approve=True)
    nuevo_r1 = J.test_de("R1", "from pkg.a import suma\n    assert suma(0.5, 0.5) == 1 and isinstance(suma(1, 1), float)", slug="floats")
    claude = J.Claude(repo, {nuevo_r1[0]: nuevo_r1[1], "docs/sdlc/bajas.md": "- R2: la regla 2 ya no aplica.\n"})
    monkeypatch.setattr(S, "_revisor", claude)
    with pytest.raises(S.StageFailed, match="aprobacion"):
        S.aceptacion()
    rem = repo / "docs/specs/calc/removidos"
    # 1. el test_R1 nuevo mueve el archivo viejo de R1 a removidos/ antes de la parada
    assert (rem / "test_R1_x.py").exists() and not (repo / "tests/sdlc/calc/test_R1_x.py").exists()
    # 2. la lista de bajas nombra R2 con su motivo: el codigo mueve su archivo y suma el motivo al README
    assert (rem / "test_R2_x.py").exists() and "la regla 2 ya no aplica." in (rem / "README.md").read_text(encoding="utf-8")
    sup = (repo / "docs/sdlc/supervision.md").read_text(encoding="utf-8")
    assert "test_R1_x.py" in sup and "R2" in sup  # la parada muestra lo que sale y los motivos
    # 3. un delta con un REMOVED fuera de la lista de bajas falla
    ok, note = S.gate_delta(_delta(modified="- R1: regla 1, con floats.", removed="- R2: x.\n- R4: y."))
    assert not ok and "R4" in note
    # 4. una baja de la lista que falta en el delta tambien falla
    ok, note = S.gate_delta(_delta(modified="- R1: regla 1, con floats."))
    assert not ok and "R2" in note
    assert S.gate_delta(_delta(modified="- R1: regla 1, con floats.", removed="- R2: ya no aplica."))[0]
    # 13. una corrida reanudada desde la etapa 2 no repite los movimientos de la etapa 1
    movs = S.state["movimientos"]
    J.conf(repo, monkeypatch, files=["pkg/a.py"])
    monkeypatch.setattr(S, "llm", lambda *a, **k: _delta(modified="- R1: regla 1, con floats.", removed="- R2: ya no aplica."))
    monkeypatch.setattr(S, "_revisor", J.Claude(repo))
    S.run(2, 2)
    assert S.state["movimientos"] == movs and sorted(p.name for p in rem.iterdir()) == ["README.md", "test_R1_x.py", "test_R2_x.py"]
    # 5. la suite no cuenta los archivos movidos como fallo ni como test faltante
    S.state["plan_files"] = ["pkg/a.py"]
    J.escribir(repo, {"pkg/a.py": J.A.replace("return a + b", "return float(a + b)")})
    assert S.gate_suite_total()[0]
    # 6. un coder que borra el archivo de R4 produce la regresion "test desaparecido"
    (repo / "tests/sdlc/calc/test_R4_x.py").unlink()
    ok, note = S.gate_suite_total()
    assert not ok and "test desaparecido" in note and "test_R4_x.py" in note
    J.git(repo, "checkout", "--", "tests/sdlc/calc/test_R4_x.py")
    # 7. un fallo nuevo en R5, sin relacion con el delta, cuenta como regresion
    J.escribir(repo, dict([J.test_de("R5", "assert False")]))
    ok, note = S.gate_suite_total()
    assert not ok and "test_R5_x" in note
    J.git(repo, "checkout", "--", "tests/sdlc/calc/test_R5_x.py")
    # 8. un plan que nombra un archivo bajo tests/ falla gate_plan_allowlist
    assert not S.gate_plan_allowlist(_plan(["pkg/a.py", "tests/sdlc/calc/test_R5_x.py"]), ["pkg/a.py"])[0]
    # 9. una tabla que cita el mismo test en dos filas falla
    t = "tests/sdlc/calc/test_R4_x.py::test_R4_x"
    spec = J.spec_area({"R4": "a.", "R5": "b."}) + f"| R4 | {t} |\n| R5 | {t} |\n"
    assert any("dos filas" in e for e in A.gate_traza_area(spec, repo))
    # 10. una corrida sin area deja gate_suite_total igual que hoy (un archivo borrado no es regresion)
    sin = J.repo(tmp_path / "sin", toml='accept_cmd = "python -m pytest -q"\nfiles = ["pkg/*.py"]\n',
                 archivos={"tests/test_otro.py": "def test_otro():\n    assert True\n"})
    J.conf(sin, monkeypatch, files=["pkg/a.py"])
    S.state["plan_files"], S.state["base_sha"] = ["pkg/a.py"], "HEAD"
    (sin / "tests/test_otro.py").unlink()
    assert S.gate_suite_total()[0]
    # 11. un `files` del llamador bajo tests/sdlc/<area>/ se rechaza en modo area
    J.conf(repo, monkeypatch, files=["pkg/a.py", "tests/sdlc/calc/test_R4_x.py"])
    with pytest.raises(S.StageFailed, match="tests/sdlc/"):
        S._resolver_area()


def test_bajas_con_id_inexistente(tmp_path, monkeypatch):
    # 12 (ticket 05). una lista de bajas que nombra un ID inexistente en el area falla
    repo = J.repo(tmp_path)
    _area_r1245(repo, monkeypatch)
    rel, txt = J.test_de("R3", "assert False", slug="nuevo")
    monkeypatch.setattr(S, "_revisor", J.Claude(repo, {rel: txt, "docs/sdlc/bajas.md": "- R9: no existe.\n"}))
    with pytest.raises(S.StageFailed, match="R9"):
        S.aceptacion()


# ---------------------------------------------------------------- ticket 06

BASE6 = J.spec_area({"R1": "suma(a, b) devuelve a + b.\n  Casos borde: floats.\n  Criterio: suma(1, 2) == 3.",
                     "R2": "resta(a, b) devuelve a - b.", "R3": "es_par(n)."})
DELTA6 = _delta(added="- R4: doble(x) devuelve 2x.", modified="- R1: suma(a, b) devuelve a + b redondeado.\n  Criterio: suma(0.1, 0.2) == 0.3.",
                removed="- R2: se mudo.")


def test_fusion_del_delta(tmp_path, monkeypatch):
    # 1. R1 nuevo, R3 byte a byte, R4, sin R2
    fus = A.fusionar(BASE6, DELTA6)
    req = A.requisitos(fus)
    assert list(req) == ["R1", "R3", "R4"] and "redondeado" in req["R1"] and "Casos borde" not in req["R1"]
    assert req["R3"] == A.requisitos(BASE6)["R3"] and "R2" not in fus.split("## Trazabilidad")[0]
    # 2. aplicar el mismo delta dos veces falla y no duplica R4
    with pytest.raises(ValueError, match="R4"):
        A.fusionar(fus, DELTA6)
    repo = J.repo(tmp_path)
    J.escribir(repo, dict(J.test_de(i) for i in ("R1", "R3", "R4")))
    J.escribir(repo, {"docs/specs/calc/changes/f-abc.plan.md": _plan(["pkg/a.py", "pkg/c.py"])})
    base_firmas = BASE6.replace("- `pkg/a.py`\n", "- `pkg/a.py`\n- suma(a: int, b: int) -> int\n")
    reg = A.regenerar(A.fusionar(base_firmas, DELTA6), repo, "calc")
    # 3. la Trazabilidad fusionada sale de tests/sdlc/<area>/ y cita ruta::test por ID
    assert "| R4 | tests/sdlc/calc/test_R4_x.py::test_R4_x |" in reg
    # 4. el Contrato fusionado lista solo rutas cubiertas, sin firmas
    contrato = reg.split("## Contrato")[1].split("## Requisitos")[0]
    assert "`pkg/a.py`" in contrato and "`pkg/c.py`" in contrato and "suma(a: int" not in contrato
    # 16. --regenerar dos veces seguidas no deja diferencias
    assert A.regenerar(reg, repo, "calc") == reg


def test_fusion_en_la_etapa_6(tmp_path, monkeypatch):
    repo = J.repo(tmp_path)
    sha = _area_lista_para_pr(repo, monkeypatch)
    S.pr()
    # 6. dos commits; el primero solo toca docs/specs/<area>/
    log = J.git(repo, "log", "--format=%s", f"{sha}..HEAD").splitlines()
    assert len(log) == 2
    primero = J.git(repo, "show", "--name-only", "--format=", "HEAD~1").split()
    assert primero and all(p.startswith("docs/specs/calc/") for p in primero)
    # 7. el delta queda en changes/<slug>-<sha>.md, con su .plan.md al lado
    assert f"docs/specs/calc/changes/feat_x-{sha}.md" in primero and f"docs/specs/calc/changes/feat_x-{sha}.plan.md" in primero
    viva = (repo / CALC).read_text(encoding="utf-8")
    assert "- R2: es_par(n) dice si n es par." in viva and "| R2 | tests/sdlc/calc/test_R2_x.py::test_R2_x |" in viva
    # 15. una etapa 6 reanudada tras una falla no vuelve a aplicar el delta y termina igual
    repo2 = J.repo(tmp_path / "r2")
    _area_lista_para_pr(repo2, monkeypatch)
    real, n = S._commit, []

    def falla_una_vez(msg, rutas=None):
        n.append(msg)
        if len(n) == 2:
            raise S.StageFailed("6-pr", "falla simulada")
        return real(msg, rutas)
    monkeypatch.setattr(S, "_commit", falla_una_vez)
    with pytest.raises(S.StageFailed):
        S.pr()
    S.pr()
    assert (repo2 / CALC).read_text(encoding="utf-8") == viva
    # 5. un requisito fusionado sin archivo de test corta la etapa 6
    repo3 = J.repo(tmp_path / "r3")
    _area_lista_para_pr(repo3, monkeypatch, delta="# Delta\n\n## ADDED\n\n- R2: x.\n- R3: y.\n")
    with pytest.raises(S.StageFailed, match="R3"):
        S.pr()
    # 13. una corrida sin area deja la etapa 6 igual que hoy: spec en docs/specs/<slug>/ y un commit
    sin = J.repo(tmp_path / "sin", toml='accept_cmd = "x"\nfiles = ["pkg/*.py"]\n')
    J.conf(sin, monkeypatch, files=["pkg/a.py"])
    S.state["plan_files"], S.state["base_sha"] = ["pkg/a.py"], J.git(sin, "rev-parse", "--short", "HEAD").strip()
    J.escribir(sin, {"docs/sdlc/spec.md": "spec\n", "docs/sdlc/plan.md": "plan\n", "pkg/a.py": J.A + "X = 1\n"})
    S.pr()
    assert (sin / "docs/specs/feat_x/spec.md").exists() and len(J.git(sin, "log", "--format=%s").splitlines()) == 2


def _precommit(repo):
    return subprocess.run([S.PY, "-m", "sdlc", "specs", "--chequear", str(repo)], capture_output=True, text=True,
                          cwd=S._ORCH)


def test_precommit_de_specs(tmp_path, monkeypatch):
    repo = J.repo(tmp_path)
    J.escribir(repo, dict(J.test_de(i) for i in ("R1", "R2")))
    J.escribir(repo, {CALC: A.regenerar(J.spec_area({"R1": "a.", "R2": "b."}), repo, "calc")})
    J.commit(repo, "area")
    assert _precommit(repo).returncode == 0
    # 8. un test nuevo agregado a mano deja la Trazabilidad vieja: el pre-commit falla con el comando que la regenera
    r2 = repo / "tests/sdlc/calc/test_R2_x.py"
    r2.write_text(r2.read_text(encoding="utf-8") + "\n\ndef test_R2_otro():\n    assert True\n", encoding="utf-8")
    antes = {p: p.read_bytes() for p in repo.rglob("*") if p.is_file() and ".git" not in p.parts}
    r = _precommit(repo)
    assert r.returncode == 1 and "python -m sdlc specs --regenerar" in r.stdout + r.stderr
    # 9. el pre-commit que falla no escribe ningun archivo
    assert {p: p.read_bytes() for p in repo.rglob("*") if p.is_file() and ".git" not in p.parts} == antes
    # 10. tras `python -m sdlc specs --regenerar`, el pre-commit pasa
    assert S.main(["specs", "--regenerar", str(repo)]) == 0
    assert _precommit(repo).returncode == 0
    J.commit(repo, "regenerada")
    # 11. dos archivos test_R3_* en la misma area hacen fallar el pre-commit
    J.escribir(repo, {"tests/sdlc/calc/test_R1_bis.py": "def test_R1_bis():\n    assert True\n"})
    r = _precommit(repo)
    assert r.returncode == 1 and "R1" in r.stdout and "un archivo por ID" in r.stdout
    (repo / "tests/sdlc/calc/test_R1_bis.py").unlink()
    # 12. un R3 repetido en la spec tambien falla
    viva = (repo / CALC).read_text(encoding="utf-8")
    J.escribir(repo, {CALC: viva.replace("- R2: b.", "- R2: b.\n- R2: c.")})
    r = _precommit(repo)
    assert r.returncode == 1 and "R2" in r.stdout and "repetido" in r.stdout
    # 14. un commit que cambia un requisito de spec.md sin sumar un archivo a changes/ falla
    J.escribir(repo, {CALC: viva.replace("- R2: b.", "- R2: b cambiada.")})
    J.git(repo, "add", CALC)
    r = _precommit(repo)
    assert r.returncode == 1 and "changes/" in r.stdout
    J.escribir(repo, {"docs/specs/calc/changes/a-mano-123.md": "## MODIFIED\n\n- R2: b cambiada.\n"})
    J.git(repo, "add", "docs/specs/calc/changes/a-mano-123.md")
    assert _precommit(repo).returncode == 0
