"""D1 (SDLC ticket 06): `gen_model` llega al coder de las sub-unidades de la recursion.

Cero API: `providers.call` es un fake que anota el modelo pedido. El plan inyectado fuerza una
recursion (la unidad de nivel 0 devuelve un stub -> 'recurse' -> nivel 1 construye una hoja).
"""
import uuid

from mmorch.project_integrate import build_project


class _R:
    def __init__(self, text: str):
        self.text = text


def test_gen_model_llega_al_coder_del_nivel_2(monkeypatch, tmp_path):
    tag = uuid.uuid4().hex[:8]          # nombres unicos: el cache de codigo por unidad no debe pegar
    seen: list[str] = []

    def fake_call(model, msgs, **kw):
        seen.append(model)
        unit_line = msgs[1]["content"].split("\n", 1)[0]
        if unit_line.startswith(f"UNIT: big-{tag}"):
            return _R("```\npass\n```")                       # stub -> el driver recurre
        return _R("```\ndef f():\n    return 42\n```")

    monkeypatch.setattr("mmorch.providers.call", fake_call)

    def plan(task, ext):
        if task == "top":
            return [{"name": f"big-{tag}", "spec": f"big {tag}", "file": "big.py", "deps": [], "test_cmd": None}]
        return [{"name": f"leaf-{tag}", "spec": f"leaf {tag}", "file": "leaf.py", "deps": [], "test_cmd": None}]

    res = build_project("top", str(tmp_path), external_test=None,
                        gen_model="deepseek-v4-pro", verifier_model="gemini-2.5-flash",
                        plan=plan, run_test=lambda u, c, t: (True, "ok"),
                        propose_test=lambda c, s: "", run_snippet=lambda c, a: (True, ""),
                        commit=lambda n, r: None, write_file=lambda u, c: None)

    assert res["status"] == "built", res
    assert res["results"][0]["recursed"], res
    assert seen == ["deepseek-v4-pro", "deepseek-v4-pro"], seen   # nivel 0 y nivel 1, mismo modelo
