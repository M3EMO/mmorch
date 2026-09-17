"""Prototipo del ticket 15: que parte de un sprite es CHECKEABLE sin LLM, medido sobre defectos inyectados.

No entra a mmorch/ todavia: sin un juego real no hay a quien cablearlo (el ratchet de capas pide entrada real).
Corre solo: `python sprites_checks.py` genera un sprite bueno, 7 variantes con UN defecto conocido cada una, y verifica
que cada chequeo atrape su defecto y ninguno se queje del sprite bueno. Solo Pillow + numpy.
"""
from __future__ import annotations

import numpy as np
from PIL import Image

PALETA = [(0, 0, 0, 255), (60, 60, 90, 255), (120, 200, 120, 255), (230, 230, 120, 255), (200, 60, 60, 255)]
LADO = 32


def _arr(img: Image.Image) -> np.ndarray:
    return np.array(img.convert("RGBA"))


# ---------------------------------------------------------------- chequeos

def tamano(img: Image.Image, lado: int = LADO) -> tuple[bool, str]:
    return img.size == (lado, lado), f"{img.size} (esperado {(lado, lado)})"


def paleta_cerrada(img: Image.Image, paleta=tuple(PALETA)) -> tuple[bool, str]:
    a = _arr(img).reshape(-1, 4)
    usados = {tuple(int(x) for x in p) for p in a[a[:, 3] > 0]}
    fuera = usados - set(paleta)
    return not fuera, f"{len(fuera)} colores fuera de paleta: {sorted(fuera)[:3]}"


def alfa_binario(img: Image.Image) -> tuple[bool, str]:
    alfa = _arr(img)[:, :, 3]
    medios = int(((alfa > 0) & (alfa < 255)).sum())
    return medios == 0, f"{medios} pixeles con alfa intermedio (antialias)"


def sin_huerfanos(img: Image.Image) -> tuple[bool, str]:
    """Un pixel opaco sin vecino opaco (4-conexo) es basura suelta, no dibujo."""
    op = _arr(img)[:, :, 3] > 0
    vecinos = np.zeros_like(op, dtype=int)
    for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)):
        vecinos += np.roll(np.roll(op, dy, 0), dx, 1).astype(int)
    huerfanos = int((op & (vecinos == 0)).sum())
    return huerfanos == 0, f"{huerfanos} pixeles opacos sueltos"


def hitbox_en_alfa(img: Image.Image, hitbox: tuple[int, int, int, int]) -> tuple[bool, str]:
    """La hitbox declarada (x0, y0, x1, y1) tiene que caer dentro del dibujo, no del aire."""
    op = _arr(img)[:, :, 3] > 0
    x0, y0, x1, y1 = hitbox
    if not (0 <= x0 < x1 <= img.size[0] and 0 <= y0 < y1 <= img.size[1]):
        return False, f"hitbox {hitbox} fuera del canvas {img.size}"
    dentro = op[y0:y1, x0:x1]
    fraccion = float(dentro.mean()) if dentro.size else 0.0
    return fraccion >= 0.5, f"solo {fraccion:.0%} de la hitbox cae sobre pixeles opacos"


def frames_anclados(frames: list[Image.Image], tope: int = 1) -> tuple[bool, str]:
    """El bounding box opaco no se corre mas de `tope` px entre frames: el personaje no salta solo."""
    cajas = []
    for f in frames:
        ys, xs = np.nonzero(_arr(f)[:, :, 3] > 0)
        cajas.append((int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())))
    drift = max(max(abs(b[i] - cajas[0][i]) for i in range(4)) for b in cajas)
    return drift <= tope, f"drift de {drift} px entre frames (tope {tope})"


def bordes_limpios(img: Image.Image) -> tuple[bool, str]:
    """El dibujo no toca el borde del canvas: si lo toca, al escalar o al hacer el atlas se corta."""
    a = _arr(img)[:, :, 3]
    tocan = int((a[0] > 0).sum() + (a[-1] > 0).sum() + (a[:, 0] > 0).sum() + (a[:, -1] > 0).sum())
    return tocan == 0, f"{tocan} pixeles opacos pegados al borde del canvas"


def cierra_el_loop(frames: list[Image.Image]) -> tuple[bool, str]:
    """Animacion ciclica: el ultimo frame vuelve al primero, pixel a pixel (pixellint lo exige exacto)."""
    iguales = np.array_equal(_arr(frames[0]), _arr(frames[-1]))
    return iguales, "el ultimo frame no es identico al primero"


def tope_de_colores(img: Image.Image, tope: int = len(PALETA)) -> tuple[bool, str]:
    a = _arr(img).reshape(-1, 4)
    usados = len({tuple(int(x) for x in p) for p in a[a[:, 3] > 0]})
    return usados <= tope, f"{usados} colores usados (tope {tope})"


def paleta_estable(frames: list[Image.Image]) -> tuple[bool, str]:
    sets = []
    for f in frames:
        a = _arr(f).reshape(-1, 4)
        sets.append({tuple(int(x) for x in p) for p in a[a[:, 3] > 0]})
    difs = set().union(*sets) - set.intersection(*sets)
    return not difs, f"{len(difs)} colores aparecen en unos frames y no en otros"


# ---------------------------------------------------------------- sprites de prueba

def sprite_bueno(desplazado: int = 0, paleta=PALETA) -> Image.Image:
    img = Image.new("RGBA", (LADO, LADO), (0, 0, 0, 0))
    px = img.load()
    for y in range(8, 26):
        for x in range(10, 22):
            borde = x in (10, 21) or y in (8, 25)
            px[x + desplazado, y] = paleta[0] if borde else paleta[2]
    for y in range(11, 15):          # "ojos" y detalle, dentro de la paleta
        px[13 + desplazado, y] = paleta[3]
        px[18 + desplazado, y] = paleta[1]
    return img


def variantes() -> dict[str, tuple[str, object]]:
    """{nombre: (chequeo que debe atrapar el defecto, imagen o lista de frames)}"""
    bueno = sprite_bueno()
    fuera_paleta = sprite_bueno()
    fuera_paleta.load()[15, 15] = (7, 200, 250, 255)
    antialias = sprite_bueno()
    antialias.load()[12, 20] = (120, 200, 120, 128)
    huerfano = sprite_bueno()
    huerfano.load()[2, 2] = PALETA[4]
    chico = bueno.crop((0, 0, 31, 30))
    frames_drift = [sprite_bueno(), sprite_bueno(desplazado=3), sprite_bueno()]
    otra = PALETA[:2] + [(10, 10, 200, 255)] + PALETA[3:]  # cambia el color DEL CUERPO, que el dibujo si usa
    frames_paleta = [sprite_bueno(), sprite_bueno(paleta=otra)]
    pegado = Image.new("RGBA", (LADO, LADO), (0, 0, 0, 0))
    pegado.paste(sprite_bueno(), (-10, 0))          # el dibujo toca el borde izquierdo
    loop_abierto = [sprite_bueno(), sprite_bueno(desplazado=1), sprite_bueno(desplazado=1)]
    muchos = sprite_bueno()          # el sprite bueno usa 4 colores; dos agregados pasan el tope de 5
    muchos.load()[15, 15] = (7, 200, 250, 255)
    muchos.load()[16, 15] = (250, 7, 200, 255)
    return {
        "bueno": ("", bueno),
        "pegado_al_borde": ("bordes_limpios", pegado),
        "loop_abierto": ("cierra_el_loop", loop_abierto),
        "demasiados_colores": ("tope_de_colores", muchos),
        "fuera_de_paleta": ("paleta_cerrada", fuera_paleta),
        "antialias": ("alfa_binario", antialias),
        "pixel_huerfano": ("sin_huerfanos", huerfano),
        "canvas_chico": ("tamano", chico),
        "hitbox_en_el_aire": ("hitbox_en_alfa", bueno),
        "frames_con_drift": ("frames_anclados", frames_drift),
        "frames_con_otra_paleta": ("paleta_estable", frames_paleta),
    }


def demo() -> None:
    v = variantes()
    bueno = v["bueno"][1]
    frames_ok = [sprite_bueno(), sprite_bueno(), sprite_bueno()]
    # el sprite bueno pasa TODO (hitbox declarada sobre el cuerpo)
    for nombre, (ok, detalle) in {
        "tamano": tamano(bueno), "paleta_cerrada": paleta_cerrada(bueno), "alfa_binario": alfa_binario(bueno),
        "sin_huerfanos": sin_huerfanos(bueno), "hitbox_en_alfa": hitbox_en_alfa(bueno, (12, 10, 20, 24)),
        "frames_anclados": frames_anclados(frames_ok), "paleta_estable": paleta_estable(frames_ok),
        "bordes_limpios": bordes_limpios(bueno), "cierra_el_loop": cierra_el_loop(frames_ok),
        "tope_de_colores": tope_de_colores(bueno),
    }.items():
        assert ok, f"falso positivo en el sprite bueno: {nombre} dijo {detalle}"
        print(f"bueno            | {nombre:16s} OK")

    esperado = {
        "fuera_de_paleta": lambda i: paleta_cerrada(i), "antialias": lambda i: alfa_binario(i),
        "pixel_huerfano": lambda i: sin_huerfanos(i), "canvas_chico": lambda i: tamano(i),
        "hitbox_en_el_aire": lambda i: hitbox_en_alfa(i, (0, 0, 8, 8)),
        "frames_con_drift": lambda f: frames_anclados(f), "frames_con_otra_paleta": lambda f: paleta_estable(f),
        "pegado_al_borde": lambda i: bordes_limpios(i), "loop_abierto": lambda f: cierra_el_loop(f),
        "demasiados_colores": lambda i: tope_de_colores(i),
    }
    for nombre, (chequeo, dato) in v.items():
        if not chequeo:
            continue
        ok, detalle = esperado[nombre](dato)
        assert not ok, f"{chequeo} NO atrapo el defecto {nombre}"
        print(f"{nombre:16s} | {chequeo:16s} atrapado: {detalle}")
    print("\n10 defectos, 10 atrapados, 0 falsos positivos. Lo que ningun chequeo mide: si el sprite 'queda bien'.")


if __name__ == "__main__":
    demo()
