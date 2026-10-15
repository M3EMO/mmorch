"""Oraculo agente minimo (ticket 05): maneja el sitio real con Playwright, junta evidencia y dicta un veredicto
determinista. Corre DENTRO del contenedor sin red: sirve el sitio en 127.0.0.1 y lo abre con Chromium headless.
Uso: python driver.py <dir_del_sitio> <casos.json>   -> imprime un JSON con la evidencia y el veredicto."""
import functools
import http.server
import json
import random
import re
import sys
import threading

from playwright.sync_api import sync_playwright

sitio, casos_path = sys.argv[1], sys.argv[2]
verdad = json.load(open(casos_path, encoding="utf-8"))
srv = http.server.ThreadingHTTPServer(
    ("127.0.0.1", 0), functools.partial(http.server.SimpleHTTPRequestHandler, directory=sitio))
threading.Thread(target=srv.serve_forever, daemon=True).start()
url = f"http://127.0.0.1:{srv.server_address[1]}/"
ev = {"consola": [], "externos": []}


def numero(texto):
    """'$ 1.245.000,00' (es-AR) o '1245000.00' -> float. Vacio o NaN -> None."""
    t = re.sub(r"[^\d,.\-]", "", texto)
    if "," in t:
        t = t.replace(".", "").replace(",", ".")
    try:
        return float(t)
    except ValueError:
        return None


with sync_playwright() as p:
    nav = p.chromium.launch(args=["--disable-dev-shm-usage"])
    pg = nav.new_page()
    pg.on("console", lambda m: m.type == "error" and ev["consola"].append(m.text))
    pg.on("pageerror", lambda e: ev["consola"].append(str(e)))
    pg.on("request", lambda r: r.url.startswith(url) or ev["externos"].append(r.url))
    pg.goto(url, wait_until="load")
    meta = pg.evaluate("""() => ({lang: document.documentElement.lang, title: document.title,
        desc: (document.querySelector('meta[name=description]') || {}).content || '',
        h1: document.querySelectorAll('h1').length, canonical: !!document.querySelector('link[rel=canonical]')})""")

    def calcular(g):
        pg.fill("#bruto", str(g))
        pg.click("[data-testid=calcular]")
        return numero(pg.inner_text("[data-testid=neto]"))

    fallos = []
    for c in verdad["casos"]:
        n = calcular(c["bruto"])
        if n is None or abs(n - c["neto"]) > 1:
            fallos.append({"bruto": c["bruto"], "esperado": c["neto"], "visto": n})
    rnd = random.Random(verdad["semilla_metamorfica"])
    gs = sorted(round(rnd.uniform(100_000, 30_000_000), 2) for _ in range(40))
    ns = [calcular(g) for g in gs]
    bajos = [round(rnd.uniform(100_000, verdad["umbral"] - 1), 2) for _ in range(10)]
    ns_bajos = [calcular(g) for g in bajos]
    nav.close()
srv.shutdown()

checks = {
    "casos_ocultos": not fallos,
    "monotono": None not in ns and all(b >= a - 0.01 for a, b in zip(ns, ns[1:])),
    "neto_083_bajo_umbral": all(x is not None and abs(x - 0.83 * g) <= 1 for g, x in zip(bajos, ns_bajos)),
    "consola_limpia": not ev["consola"],
    "sin_requests_externos": not ev["externos"],
    "metadatos": meta["lang"] == "es-AR" and 30 <= len(meta["title"]) <= 60 and 70 <= len(meta["desc"]) <= 160
    and meta["h1"] == 1 and meta["canonical"],
}
print(json.dumps({"veredicto": all(checks.values()), "checks": checks, "fallos": fallos[:3], "n_fallos": len(fallos),
                  "consola": ev["consola"][:3], "externos": ev["externos"][:3], "meta": meta}, ensure_ascii=False))
