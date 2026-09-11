"""Genera 300 frases de usuario (con sesiones) y las respuestas del oraculo Python demo.py.
Salida: diff_cases.json = [{sid, text, replies:[{text,image}], paused, order}] en orden."""
import json, random, sys, pathlib
sys.path.insert(0, r"C:\Users\map12\Desktop\QueTePario\ChatBot\backend\reference")
import demo  # noqa: E402

rng = random.Random(7)
catalog = demo.load_catalog(False)
names = [p["name"] for p in catalog]
colors = sorted({v["color"] for p in catalog for v in p["variants"]})
sizes = ["34", "35", "36", "37", "38", "39", "40", "41", "42", "43", "44", "45", "46", "39/40"]
frases = ["hola", "buenas tardes", "que horarios tienen", "hacen envios?", "abren los sabados?",
          "quiero hablar con una persona", "soy revendedor", "por mayor", "1", "2", "3", "4", "0", "menu",
          "que tenes para hombre", "que tenes para mujer", "modelos de invierno", "catalogo", "mostrame opciones",
          "asdf", "qwerty", "zzz", "precio?", "cuanto sale", "hay stock?", "que talle tenes", "que color hay",
          "si", "dale", "no", "retiro", "envio", "mercedes", "Buenos Aires", "gracias", "chau"]

def phrase():
    k = rng.random()
    if k < 0.25:
        return rng.choice(frases)
    if k < 0.45:
        n = rng.choice(names).lower()
        w = n.split()
        return " ".join(w[: rng.randint(1, len(w))])
    if k < 0.6:
        return f"{rng.choice(names).lower().split()[0]} en {rng.choice(sizes)}"
    if k < 0.75:
        return f"{rng.choice(names).lower().split()[0]} {rng.choice(colors).lower()}"
    if k < 0.85:
        return f"tenes {rng.choice(names).lower()} {rng.choice(colors).lower()} {rng.choice(sizes)}?"
    if k < 0.92:
        return f"y en {rng.choice(sizes)}?"
    return rng.choice(["1", "2", "5", "9", "12", "0"])

bots = {}
cases = []
for i in range(300):
    sid = f"s{rng.randint(1, 25)}"
    bot = bots.setdefault(sid, demo.Bot(catalog)) if False else None
    text = phrase()
    cases.append({"sid": sid, "text": text})

# un solo Bot con sesiones internas por sid, igual que Java
bot = demo.Bot(catalog)
for c in cases:
    r = bot.reply(c["sid"], c["text"])
    c["replies"] = [{"text": m.get("text", ""), "image": m.get("image")} for m in r["replies"]]
    c["paused"] = bool(r["paused"])
    c["order"] = r.get("order")
out = pathlib.Path(__file__).with_name("diff_cases.json")
out.write_text(json.dumps(cases, ensure_ascii=False, indent=0), encoding="utf-8")
print("casos:", len(cases), "| pausados:", sum(c["paused"] for c in cases), "| ordenes:", sum(1 for c in cases if c["order"]))
