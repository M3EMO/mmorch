"""Oraculo visual (ticket 15): la capa determinista atrapa defectos inyectados; el juez visual solo observa."""
import json
import types

from PIL import Image

import mmorch.sprites as S

PALETA = ["#000000", "#3c3c5a", "#78c878", "#e6e678"]
LADO = 16


def sprite(desplazado=0, color_cuerpo="#78c878") -> Image.Image:
    img = Image.new("RGBA", (LADO, LADO), (0, 0, 0, 0))
    px = img.load()
    for y in range(4, 13):
        for x in range(4, 12):
            borde = x in (4, 11) or y in (4, 12)
            px[x + desplazado, y] = S._hex("#000000" if borde else color_cuerpo)
    px[6 + desplazado, 6] = S._hex("#e6e678")
    return img


def _malos(res):
    return {n for n, ok, _ in res if not ok}


def test_capa_determinista_atrapa_cada_defecto_y_no_molesta_al_sprite_bueno():
    bueno = sprite()
    assert not _malos(S.chequeos(bueno, lado=LADO, paleta=PALETA, tope_colores=4, hitbox=[5, 5, 10, 11]))

    fuera = sprite(); fuera.load()[7, 7] = (7, 200, 250, 255)
    assert "paleta-cerrada" in _malos(S.chequeos(fuera, paleta=PALETA))
    assert "tope-colores" in _malos(S.chequeos(fuera, tope_colores=3))

    antialias = sprite(); antialias.load()[6, 8] = (120, 200, 120, 128)
    assert "alfa-binario" in _malos(S.chequeos(antialias))

    huerfano = sprite(); huerfano.load()[1, 1] = S._hex("#000000")
    assert "sin-huerfanos" in _malos(S.chequeos(huerfano))

    pegado = Image.new("RGBA", (LADO, LADO), (0, 0, 0, 0)); pegado.paste(sprite(), (-4, 0))
    assert "bordes-limpios" in _malos(S.chequeos(pegado))
    assert "tamano" in _malos(S.chequeos(bueno.crop((0, 0, 15, 15)), lado=LADO))
    assert "hitbox-en-alfa" in _malos(S.chequeos(bueno, hitbox=[0, 0, 3, 3]))


def test_chequeos_de_animacion():
    ok = [sprite(), sprite(), sprite()]
    assert not _malos(S.chequeos_animacion(ok))
    assert "frames-anclados" in _malos(S.chequeos_animacion([sprite(), sprite(desplazado=3), sprite()]))
    assert "paleta-estable" in _malos(S.chequeos_animacion([sprite(), sprite(color_cuerpo="#0a0ac8")]))
    assert "cierra-loop" in _malos(S.chequeos_animacion([sprite(), sprite(desplazado=1), sprite(desplazado=1)]))


def test_revisar_usa_la_config_del_repo(tmp_path):
    (tmp_path / "assets").mkdir()
    sprite().save(tmp_path / "assets" / "heroe.png")
    sprite(desplazado=2).save(tmp_path / "assets" / "heroe_1.png")
    cfg = {"dir": "assets", "lado": LADO, "paleta": PALETA, "hitboxes": {"heroe.png": [5, 5, 10, 11]},
           "animaciones": {"correr": ["heroe.png", "heroe_1.png", "heroe.png"]}}
    res = S.revisar(cfg, tmp_path)
    assert {n for n, ok, _ in res if not ok} == {"correr: frames-anclados"}  # el resto verde


def test_juez_visual_solo_observa_y_queda_en_sombra(tmp_path, monkeypatch):
    monkeypatch.setattr(S, "logs_dir", lambda: tmp_path)
    sprite().save(tmp_path / "ref.png")
    sprite(color_cuerpo="#e6e678").save(tmp_path / "nuevo.png")
    visto = {}

    def fake(modelo, mensaje, **kw):
        visto["modelo"], visto["mensaje"] = modelo, mensaje
        return types.SimpleNamespace(text='ruido {"mejor": "candidata", "criterios": {"silueta legible al tamano '
                                          'objetivo": true}, "motivo": "lee bien"} mas ruido')

    d = S.juez_visual(tmp_path / "nuevo.png", tmp_path / "ref.png", llamar=fake)
    assert d["mejor"] == "candidata" and d["motivo"] == "lee bien"
    partes = visto["mensaje"][0]["content"]
    assert [p["type"] for p in partes] == ["text", "image_url", "image_url"]  # referencia primero, candidata despues
    assert "puntajes numericos" in partes[0]["text"] and partes[1]["image_url"]["url"].startswith("data:image/png;base64,")
    import base64, io  # la imagen viaja agrandada con vecino-mas-cercano, no el sprite de 16 px
    crudo = base64.b64decode(partes[1]["image_url"]["url"].split(",", 1)[1])
    assert Image.open(io.BytesIO(crudo)).size == (LADO * S.ESCALA, LADO * S.ESCALA)
    linea = json.loads((tmp_path / "sdlc" / "juez_visual.jsonl").read_text(encoding="utf-8").strip())
    assert linea["sprite"].endswith("nuevo.png") and linea["modelo"] == S.JUEZ


def test_kappa_y_salida_de_sombra(tmp_path, monkeypatch):
    assert S.kappa([("a", "a")] * 5 + [("b", "b")] * 5) == 1.0
    assert S.kappa([("a", "b")] * 5 + [("b", "a")] * 5) < 0
    sombra = [{"sprite": f"s{i}.png", "mejor": "candidata"} for i in range(S.MIN_ITEMS)]
    humanos = [{"kind": "sprite", "sprite": f"s{i}.png", "label": "candidata" if i % 10 else "referencia"}
               for i in range(S.MIN_ITEMS)]
    ok, nota = S.confiable(humanos, sombra)
    assert not ok and "kappa" in nota  # acuerda casi siempre, pero sin varianza kappa no sube
    ok, nota = S.confiable(humanos[:10], sombra)
    assert not ok and f"10 de {S.MIN_ITEMS}" in nota
