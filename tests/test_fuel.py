"""Tests F4 fuel (contrato .scratch/loop-cerrado/spec.md)."""

import time

from mmorch.fuel import (
    ARM_PREFIX,
    LENTES,
    detect_promotions,
    expire_candidates,
    generate_candidates,
    has_new_fuel,
    parse_candidatos,
    render_candidatos,
)


def entry(id_="2026-08-14-01", lente="deuda", gist="consolidar bandits",
          estado="pendiente", vence="2026-08-28"):
    return {"id": id_, "fecha": "2026-08-14", "vence": vence, "lente": lente,
            "gist": gist, "estado": estado}


class FakeGen:
    def __init__(self, gists):
        self.gists = dict(gists)  # lente -> gist | None
        self.payloads = []

    def propose(self, payload):
        self.payloads.append(payload)
        return {"gist": self.gists.get(payload["lente"]), "justification": "j"}


class FakeVer:
    def __init__(self, refute_gists=()):
        self.refute_gists = set(refute_gists)

    def refute(self, payload):
        return {"refuted": payload["gist"] in self.refute_gists}


class Recorder:
    def __init__(self):
        self.calls = []

    def __call__(self, arm, reward, *, pattern="", source=""):
        self.calls.append((arm, reward, source))


def test_roundtrip_preserves_fields():
    vig = [entry(), entry(id_="2026-08-14-02", lente="integracion",
                          gist="recall federado")]
    arch = [entry(id_="2026-08-01-01", estado="expirada", vence="2026-08-10")]
    md = render_candidatos(vig, arch)
    parsed = parse_candidatos(md)
    assert [e["id"] for e in parsed] == ["2026-08-14-01", "2026-08-14-02"]
    assert parsed[0]["vence"] == "2026-08-28"
    assert parsed[1]["lente"] == "integracion"
    assert parsed[0]["estado"] == "pendiente"
    # archivadas no aparecen en vigentes pero conservan estado en el texto
    assert "estado: expirada" in md


def test_parse_ignores_garbage():
    md = "# x\n\n## Vigentes\n\n- **cand-roto sin formato\n- texto suelto\n"
    assert parse_candidatos(md) == []


def test_has_new_fuel(tmp_path):
    f = tmp_path / "a.txt"
    f.write_text("x")
    old = time.time() - 1000
    assert has_new_fuel(old, [str(f)]) is True
    assert has_new_fuel(time.time() + 1000, [str(f)]) is False
    assert has_new_fuel(old, [str(tmp_path / "no-existe")]) is False


def test_generate_lentes_and_dedup(tmp_path):
    cand = tmp_path / "candidatos.md"
    road = tmp_path / "roadmap.md"
    road.write_text("direccion: unificar recall federado en mmorch",
                    encoding="utf-8")
    gen = FakeGen({"deuda": "consolidar bandits",
                   "capacidad": None,
                   "integracion": "unificar recall federado en mmorch",  # ya en roadmap
                   "notas-huerfanas": "adjudicar notas viejas"})
    result = generate_candidates("ctx", gen, FakeVer(),
                                 candidatos_path=str(cand),
                                 roadmap_path=str(road), today="2026-08-14")
    assert result == {"nuevas": 2}  # None no produce; la del roadmap dedupea
    parsed = parse_candidatos(cand.read_text(encoding="utf-8"))
    assert {e["lente"] for e in parsed} == {"deuda", "notas-huerfanas"}
    assert all(e["vence"] == "2026-08-28" for e in parsed)
    # el generator recibio ya_visto
    assert all("ya_visto" in p for p in gen.payloads)


def test_refuted_does_not_enter(tmp_path):
    cand = tmp_path / "candidatos.md"
    gen = FakeGen({"deuda": "idea refutable", "capacidad": None,
                   "integracion": None, "notas-huerfanas": None})
    result = generate_candidates("ctx", gen, FakeVer(refute_gists={"idea refutable"}),
                                 candidatos_path=str(cand),
                                 roadmap_path=str(tmp_path / "no.md"),
                                 today="2026-08-14")
    assert result == {"nuevas": 0}


def test_sequential_ids_same_day(tmp_path):
    cand = tmp_path / "candidatos.md"
    gen1 = FakeGen({"deuda": "idea uno", "capacidad": None,
                    "integracion": None, "notas-huerfanas": None})
    generate_candidates("ctx", gen1, FakeVer(), candidatos_path=str(cand),
                        roadmap_path=str(tmp_path / "no.md"), today="2026-08-14")
    gen2 = FakeGen({"deuda": "idea dos", "capacidad": None,
                    "integracion": None, "notas-huerfanas": None})
    generate_candidates("ctx", gen2, FakeVer(), candidatos_path=str(cand),
                        roadmap_path=str(tmp_path / "no.md"), today="2026-08-14")
    ids = [e["id"] for e in parse_candidatos(cand.read_text(encoding="utf-8"))]
    assert ids == ["2026-08-14-01", "2026-08-14-02"]


def test_expire_moves_and_records(tmp_path):
    cand = tmp_path / "candidatos.md"
    cand.write_text(render_candidatos(
        [entry(vence="2026-08-10", lente="deuda"),
         entry(id_="2026-08-14-02", vence="2026-08-28", lente="capacidad",
               gist="otra")], []), encoding="utf-8")
    rec = Recorder()
    result = expire_candidates(candidatos_path=str(cand), today="2026-08-14",
                               record_fn=rec)
    assert result == {"expired": 1}
    md = cand.read_text(encoding="utf-8")
    assert len(parse_candidatos(md)) == 1
    assert "estado: expirada" in md
    assert rec.calls == [(f"{ARM_PREFIX}deuda", 0.2, "soft_reject")]


def test_detect_promotions(tmp_path):
    cand = tmp_path / "candidatos.md"
    road = tmp_path / "roadmap.md"
    cand.write_text(render_candidatos(
        [entry(gist="consolidar los tres bandits en uno", lente="deuda"),
         entry(id_="2026-08-14-02", gist="otra idea distinta",
               lente="capacidad")], []), encoding="utf-8")
    road.write_text("## Direcciones\n- Consolidar los tres bandits en uno\n",
                    encoding="utf-8")
    rec = Recorder()
    result = detect_promotions(candidatos_path=str(cand),
                               roadmap_path=str(road), record_fn=rec)
    assert result == {"promoted": 1}
    md = cand.read_text(encoding="utf-8")
    assert "estado: promovida" in md
    assert len(parse_candidatos(md)) == 1
    assert rec.calls == [(f"{ARM_PREFIX}deuda", 1.0, "roadmap_promotion")]


def test_promotions_without_roadmap(tmp_path):
    cand = tmp_path / "candidatos.md"
    cand.write_text(render_candidatos([entry()], []), encoding="utf-8")
    result = detect_promotions(candidatos_path=str(cand),
                               roadmap_path=str(tmp_path / "no.md"),
                               record_fn=Recorder())
    assert result == {"promoted": 0}


def test_lentes_constant():
    assert LENTES == ("deuda", "capacidad", "integracion", "notas-huerfanas")
