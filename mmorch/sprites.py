"""sprites — oraculo visual del pipeline (ticket 15 del mapa sdlc-6-gates).

Dos capas, con roles distintos:

- DETERMINISTA (bloquea): tamano, paleta cerrada, tope de colores, alfa binario, pixeles huerfanos, bordes limpios,
  hitbox dentro del alfa, y por animacion: frames anclados, paleta estable, loop cerrado. Son reglas exactas sobre los
  pixeles, asi que no las decide ningun modelo.
- JUICIO (solo observa): `juez_visual` compara el sprite nuevo contra una referencia aprobada (de a pares) y contesta
  una rubrica binaria por criterio. Queda EN SOMBRA hasta medir kappa >= 0.6 contra >= 50 veredictos humanos
  (protocolo del ticket 08). Medido 2026-09-17: un juez VLM ordena (Pearson ~0.46) pero puntua mal (32% exacto), pierde
  acierto con imagenes chicas (59.6% a 384px vs 67.9% a 1536px) y no tiene benchmark publicado en pixel art. Por eso:
  nunca puntaje absoluto, y el sprite se agranda con vecino-mas-cercano antes de mandarlo.
"""
from __future__ import annotations

import base64
import io
import json
import time
from pathlib import Path

import numpy as np
from PIL import Image
from typing import Sequence

from .paths import logs_dir

CRITERIOS = ("silueta legible al tamano objetivo", "paleta coherente con la referencia",
             "luz y sombra consistentes", "lectura clara a la distancia de juego")
JUEZ = "gemini-2.5-flash"
ESCALA = 8          # vecino-mas-cercano: un sprite de 32px viaja como 256px
MIN_ITEMS = 50      # ticket 15 D3: antes de confiar en el juez
KAPPA_MIN = 0.6


def _arr(img: Image.Image) -> np.ndarray:
    return np.array(img.convert("RGBA"))


def _colores(img: Image.Image) -> set[tuple]:
    a = _arr(img).reshape(-1, 4)
    return {tuple(int(x) for x in p) for p in a[a[:, 3] > 0]}


def _hex(color: str) -> tuple[int, int, int, int]:
    c = color.lstrip("#")
    return (int(c[0:2], 16), int(c[2:4], 16), int(c[4:6], 16), int(c[6:8], 16) if len(c) == 8 else 255)


# ----------------------------------------------------------------- capa determinista

def chequeos(img: Image.Image, *, lado: int | None = None, paleta: list[str] | None = None,
             tope_colores: int | None = None, hitbox: list[int] | None = None) -> list[tuple[str, bool, str]]:
    """Los chequeos que aplican a UN sprite, como (nombre, ok, detalle)."""
    a = _arr(img)
    alfa = a[:, :, 3]
    op = alfa > 0
    out: list[tuple[str, bool, str]] = []
    if lado:
        out.append(("tamano", img.size == (lado, lado), f"{img.size} (esperado {(lado, lado)})"))
    if paleta:
        fuera = _colores(img) - {_hex(c) for c in paleta}
        out.append(("paleta-cerrada", not fuera, f"{len(fuera)} colores fuera de paleta: {sorted(fuera)[:3]}"))
    if tope_colores:
        n = len(_colores(img))
        out.append(("tope-colores", n <= tope_colores, f"{n} colores (tope {tope_colores})"))
    medios = int(((alfa > 0) & (alfa < 255)).sum())
    out.append(("alfa-binario", medios == 0, f"{medios} pixeles con alfa intermedio (antialias)"))
    vecinos = sum(np.roll(np.roll(op, dy, 0), dx, 1).astype(int) for dy, dx in ((1, 0), (-1, 0), (0, 1), (0, -1)))
    huerfanos = int((op & (vecinos == 0)).sum())
    out.append(("sin-huerfanos", huerfanos == 0, f"{huerfanos} pixeles opacos sueltos"))
    borde = int((alfa[0] > 0).sum() + (alfa[-1] > 0).sum() + (alfa[:, 0] > 0).sum() + (alfa[:, -1] > 0).sum())
    out.append(("bordes-limpios", borde == 0, f"{borde} pixeles opacos pegados al borde"))
    if hitbox:
        x0, y0, x1, y1 = hitbox
        dentro = op[y0:y1, x0:x1]
        frac = float(dentro.mean()) if dentro.size else 0.0
        out.append(("hitbox-en-alfa", frac >= 0.5, f"{frac:.0%} de la hitbox cae sobre pixeles opacos"))
    return out


def chequeos_animacion(frames: Sequence[Image.Image], *, drift_max: int = 1, cierra_loop: bool = True) -> list[tuple[str, bool, str]]:
    """Los chequeos que solo tienen sentido sobre una secuencia."""
    cajas = []
    for f in frames:
        ys, xs = np.nonzero(_arr(f)[:, :, 3] > 0)
        cajas.append((int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())) if len(xs) else (0, 0, 0, 0))
    drift = max(max(abs(b[i] - cajas[0][i]) for i in range(4)) for b in cajas)
    sets: list[set] = [_colores(f) for f in frames]
    difs = set().union(*sets) - set.intersection(*sets)
    out = [("frames-anclados", drift <= drift_max, f"drift de {drift} px (tope {drift_max})"),
           ("paleta-estable", not difs, f"{len(difs)} colores aparecen en unos frames y no en otros")]
    if cierra_loop:
        igual = np.array_equal(_arr(frames[0]), _arr(frames[-1]))
        out.append(("cierra-loop", igual, "el ultimo frame no es identico al primero"))
    return out


def revisar(cfg: dict, raiz: Path) -> list[tuple[str, bool, str]]:
    """Corre la capa determinista sobre los assets que declara `cfg` (bloque [sprites] de sdlc.toml)."""
    base = raiz / cfg.get("dir", ".")
    comun = {"lado": cfg.get("lado"), "paleta": cfg.get("paleta"), "tope_colores": cfg.get("tope_colores")}
    res: list[tuple[str, bool, str]] = []
    for rel in sorted(cfg.get("archivos") or [p.relative_to(base).as_posix() for p in base.glob("**/*.png")]):
        img = Image.open(base / rel)
        hitbox = (cfg.get("hitboxes") or {}).get(rel)
        res += [(f"{rel}: {n}", ok, d) for n, ok, d in chequeos(img, hitbox=hitbox, **comun)]
    for nombre, rels in (cfg.get("animaciones") or {}).items():
        frames = [Image.open(base / r) for r in rels]
        res += [(f"{nombre}: {n}", ok, d) for n, ok, d in chequeos_animacion(
            frames, drift_max=int(cfg.get("drift_max", 1)), cierra_loop=bool(cfg.get("cierra_loop", True)))]
    return res


# ----------------------------------------------------------------- capa de juicio (sombra)

def _parte_imagen(img: Image.Image, escala: int = ESCALA) -> dict:
    grande = img.resize((img.size[0] * escala, img.size[1] * escala), Image.Resampling.NEAREST)  # el VLM pierde acierto en chico
    buf = io.BytesIO()
    grande.save(buf, format="PNG")
    return {"type": "image_url", "image_url": {"url": "data:image/png;base64," + base64.b64encode(buf.getvalue()).decode()}}


def juez_visual(nuevo: Path, referencia: Path, *, criterios=CRITERIOS, modelo: str = JUEZ,
                llamar=None) -> dict:
    """Compara de a pares y contesta la rubrica binaria. NO decide nada: su salida va a sombra."""
    from .providers import call
    texto = ("Sos un revisor de arte de un juego 2D. La PRIMERA imagen es una referencia YA APROBADA; la SEGUNDA es la "
             "candidata. Las dos estan agrandadas con vecino-mas-cercano, asi que los bordes duros son correctos.\n"
             "Responde SOLO un JSON: {\"mejor\": \"referencia\"|\"candidata\"|\"empate\", \"criterios\": {"
             + ", ".join(f'"{c}": true|false' for c in criterios) + "}, \"motivo\": \"<una linea>\"}\n"
             "No pongas puntajes numericos.")
    mensaje = [{"role": "user", "content": [{"type": "text", "text": texto},
                                            _parte_imagen(Image.open(referencia)), _parte_imagen(Image.open(nuevo))]}]
    r = (llamar or call)(modelo, mensaje, pattern="sprite-juez", node=modelo, phase="visual", temperature=0.0,
                         max_tokens=1024, timeout=120)
    crudo = r.text if hasattr(r, "text") else str(r)
    try:
        d = json.loads(crudo[crudo.index("{"):crudo.rindex("}") + 1])
    except Exception:
        d = {"mejor": "", "criterios": {}, "motivo": f"respuesta no parseable: {crudo[:200]}"}
    d["sprite"] = str(nuevo)
    d["referencia"] = str(referencia)
    d["modelo"] = modelo
    d["ts"] = time.time()
    _sombra().parent.mkdir(parents=True, exist_ok=True)
    with _sombra().open("a", encoding="utf-8") as f:
        f.write(json.dumps(d, ensure_ascii=False) + "\n")
    return d


def _sombra() -> Path:
    return logs_dir() / "sdlc" / "juez_visual.jsonl"


def kappa(pares: list[tuple[str, str]]) -> float:
    """Cohen's kappa entre juez y humano sobre etiquetas discretas (ticket 08: >= 0.6 para salir de sombra)."""
    if not pares:
        return 0.0
    etiquetas = sorted({e for p in pares for e in p})
    n = len(pares)
    po = sum(a == b for a, b in pares) / n
    pe = sum((sum(a == e for a, _ in pares) / n) * (sum(b == e for _, b in pares) / n) for e in etiquetas)
    return 1.0 if pe == 1 else round((po - pe) / (1 - pe), 3)


def confiable(veredictos: list[dict], sombra: list[dict] | None = None) -> tuple[bool, str]:
    """El juez sale de sombra con >= MIN_ITEMS sprites etiquetados por el humano y kappa >= KAPPA_MIN."""
    sombra = sombra if sombra is not None else [json.loads(x) for x in
                                                _sombra().read_text(encoding="utf-8").splitlines() if x.strip()]
    por_sprite = {d.get("sprite"): d for d in sombra}
    pares = [(por_sprite[v["sprite"]].get("mejor", ""), v["label"]) for v in veredictos
             if v.get("kind") == "sprite" and v.get("sprite") in por_sprite]
    k = kappa(pares)
    if len(pares) < MIN_ITEMS:
        return False, f"sombra: {len(pares)} de {MIN_ITEMS} sprites etiquetados (kappa parcial {k})"
    return k >= KAPPA_MIN, f"kappa {k} sobre {len(pares)} sprites (minimo {KAPPA_MIN})"
