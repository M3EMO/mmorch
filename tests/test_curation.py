

def test_pre_veredicto_cachea_por_id(tmp_path):
    """Cada candidata se juzga UNA vez en su vida — el cache por id evita
    re-gastar API en cada vistazo de la pila."""
    from mmorch.curation import pre_veredicto
    calls = []

    def fake_llm(prompt, schema):
        calls.append(1)
        return {"puntaje": 0.3, "razon": "vaga, sin necesidad medida"}

    e = {"id": "2026-08-21-01", "lente": "capacidad", "gist": "canary semantico"}
    v1 = pre_veredicto(e, logs_dir=str(tmp_path), llm_fn=fake_llm)
    v2 = pre_veredicto(e, logs_dir=str(tmp_path), llm_fn=fake_llm)
    assert v1 == v2 == {"puntaje": 0.3, "razon": "vaga, sin necesidad medida"}
    assert len(calls) == 1                      # 2do vistazo: cache, no API


def test_pre_veredicto_fail_soft(tmp_path):
    from mmorch.curation import pre_veredicto

    def boom(prompt, schema):
        raise RuntimeError("api caida")

    assert pre_veredicto({"id": "x", "gist": "g"}, logs_dir=str(tmp_path),
                         llm_fn=boom) == {}


def test_pre_veredicto_clampa_puntaje(tmp_path):
    from mmorch.curation import pre_veredicto
    v = pre_veredicto({"id": "y", "gist": "g"}, logs_dir=str(tmp_path),
                      llm_fn=lambda p, s: {"puntaje": 7.5, "razon": "r"})
    assert v["puntaje"] == 1.0
