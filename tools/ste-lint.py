import re, sys, json, glob, os

# Vendored from woosal1337/blog videos/ep01-the-cure-for-ai-slop (score v2),
# extended with --lang es: Spanish wordlists + Spanish passive/hedge regex.
# Language-agnostic checks (sentence length, paragraphs, semicolons, em-dash)
# are shared across languages.
SCORE_VERSION = 2

MARKETING = ["seamless","seamlessly","robust","powerful","cutting-edge","effortless","effortlessly",
    "world-class","next-generation","revolutionary","blazing","lightning-fast","elegant","delightful",
    "turnkey","best-in-class","state-of-the-art","game-changing","first-class","battle-tested",
    "enterprise-grade","supercharge","unlock","unleash","empower","empowers"]
BANNED = ["begin","begins","commence","commences","initiate","initiates","originate",
    "utilize","utilizes","utilizing","leverage","leverages","leveraging","facilitate","facilitates",
    "ensure","ensures","ensuring","prior to","subsequent to","obtain","obtains","acquire","acquires",
    "demonstrate","demonstrates","additionally","furthermore","moreover","comprehensive","comprehensively",
    "utilization","aforementioned","henceforth","therein","whilst","amongst","numerous","myriad","plethora",
    "provide","provides","provided",
    "in order to","a variety of","in the event that","due to the fact that","it is important to note"]
# STE's own recurring-errors list. Counted only with --strict: these are
# correct STE but would flag normal prose in docs.
STRICT_BANNED = ["however","since","should","shall","using","follow","follows","followed"]
PHRASAL = ["spin up","spin down","reach out","dive into","dives into","diving into","kick off","kicks off",
    "roll out","rolls out","tear down","ramp up","circle back","drill down","spun up","reaching out"]
MODAL_HEDGE = ["it is important to note","it should be noted","it is worth noting","please note that",
    "as mentioned","as noted above"]

# --- Spanish rule set (same categories, ES slop) ---
MARKETING_ES = ["robusto","robusta","potente","revolucionario","revolucionaria","de vanguardia",
    "sin fisuras","elegante","innovador","innovadora","de primer nivel","de clase mundial",
    "sin esfuerzo","desbloquear","desbloquea","potenciar","potencia al maximo","supercargar"]
BANNED_ES = ["utilizar","utiliza","utilizan","utilizando","utilizacion","utilización",
    "aprovechar","aprovecha","facilitar","facilita","garantizar","garantiza",
    "asimismo","adicionalmente","por consiguiente","no obstante","sin embargo cabe",
    "previo a","posteriormente a","con el fin de","con el objetivo de","a fin de",
    "una variedad de","en el caso de que","debido al hecho de que","dicho","dicha","dichos","dichas",
    "el mismo","la misma","los mismos","las mismas","exhaustivo","exhaustiva","integral",
    "llevar a cabo","lleva a cabo","llevan a cabo","realizar la","realizar el","realizar una","realizar un"]
MODAL_HEDGE_ES = ["cabe destacar","cabe mencionar","cabe señalar","cabe senalar","es importante notar",
    "es importante destacar","es importante mencionar","vale la pena","como se menciono","como se mencionó",
    "como ya se dijo","tal como se indico","tal como se indicó"]

BE = r"(?:am|is|are|was|were|be|been|being)"
PP_IRREG = r"(?:done|made|sent|read|built|kept|held|set|put|run|written|shown|given|taken|found|got|gotten|seen|known|thrown|drawn)"
# Rule 3.3: a past participle used as an adjective is not passive. These
# stative participles only count as passive when a by-agent follows.
STATIVE = r"(?:closed|opened?|damaged|completed?|installed|connected|required|expected|configured|enabled|disabled|deprecated|supported)"
BE_ES = r"(?:es|son|era|eran|fue|fueron|sera|será|seran|serán|ha sido|han sido|habia sido|había sido|habian sido|habían sido|esta siendo|está siendo|estan siendo|están siendo)"
PP_ES = r"\w+(?:ado|ada|ados|adas|ido|ida|idos|idas|to|ta|tos|tas|cho|cha|chos|chas)"
FUNC_WORDS = set("""a an the this that these those of for to in on at by with from as and or but if
when then than not no is are was were be been being am do does did has have had will would can could
may might must should shall it its their your our his her they we you i""".split())
FUNC_WORDS_ES = set("""el la los las un una unos unas de del al a en con por para sin sobre entre
y o u e pero si no que como cuando donde este esta estos estas ese esa esos esas su sus se lo le les
es son fue era ser estar esta estan hay ya mas más muy tambien también""".split())

def strip_code(t):
    t = re.sub(r"```.*?```", " ", t, flags=re.S)
    t = re.sub(r"`[^`]*`", " ", t)
    return t

def sentences(text):
    out = []
    for line in text.split("\n"):
        s = line.strip()
        if not s: continue
        s = re.sub(r"^\s*#{1,6}\s*", "", s)
        s = re.sub(r"^\s*(?:[-*+]|\d+[.)])\s+", "", s)
        if not s: continue
        parts = re.split(r"(?<=[.!?:])\s+(?=[A-Z0-9\"'\-¿¡])", s)
        for p in parts:
            p = p.strip()
            if p: out.append(p)
    return out

def wc(s):
    return len([w for w in re.findall(r"[A-Za-zÁÉÍÓÚÑáéíóúñü0-9][A-Za-zÁÉÍÓÚÑáéíóúñü0-9'\-/]*", s)])

def count_ci(text, phrases):
    n = 0; hits = []
    low = text.lower()
    for ph in phrases:
        for _m in re.finditer(r"(?<![a-záéíóúñ])" + re.escape(ph) + r"(?![a-záéíóúñ])", low):
            n += 1; hits.append(ph)
    return n, hits

def noun_trains(text, func_words):
    """Runs of 4+ consecutive non-function lowercase words (Rule 2.1 proxy).
    Heuristic marker only - proper nouns break a run, the leading word of each
    sentence is skipped, and the count stays out of the total."""
    hits = []
    for s in sentences(text):
        words = re.findall(r"[A-Za-zÁÉÍÓÚÑáéíóúñü][A-Za-zÁÉÍÓÚÑáéíóúñü'\-]*", s)[1:]
        run = []
        for w in words + [""]:
            if w and w.lower() not in func_words and not w[0].isupper():
                run.append(w)
            else:
                if len(run) >= 4: hits.append(" ".join(run))
                run = []
    return hits

def lint(text, strict=False, lang="en"):
    raw = text
    text = strip_code(text)
    sents = sentences(text)
    words = sum(wc(s) for s in sents) or 1
    v = {}
    longs = [(wc(s), s) for s in sents if wc(s) > 20]
    v["long_sentence(>20w)"] = len(longs)
    v["semicolon"] = text.count(";")
    if lang == "es":
        v["passive_voice"] = len(re.findall(rf"\b{BE_ES}\s+{PP_ES}\b", text, re.I))
        # "se realiza / se ha realizado" impersonal-passive constructions
        v["se_passive"] = len(re.findall(r"\bse\s+(?:ha\s+|han\s+)?\w+(?:a|an|o|ó|aron|ado|ido)\b", text, re.I))
        v["nominalization"] = len(re.findall(r"\b(?:realizacion|realización|implementacion|implementación|utilizacion|utilización|ejecucion|ejecución|verificacion|verificación|configuracion|configuración|obtencion|obtención)\s+de\b", text, re.I))
        v["banned_word"], bh = count_ci(text, BANNED_ES)
        v["marketing_adjective"], mh = count_ci(text, MARKETING_ES)
        v["modal_hedge"], _ = count_ci(text, MODAL_HEDGE_ES)
        v["phrasal_verb"] = 0
        v["contraction"] = 0
        v["complex_tense"] = 0
        v["ing_main_verb"] = 0
        func_words = FUNC_WORDS_ES
    else:
        v["contraction"] = len(re.findall(r"\b\w+[''](?:t|re|ve|ll|d|s|m)\b", text))
        passive_parts = re.findall(rf"\b{BE}\s+(\w+ed|{PP_IRREG})\b", text, re.I)
        v["passive_voice"] = sum(1 for p in passive_parts if not re.fullmatch(STATIVE, p, re.I)) \
            + len(re.findall(rf"\b{BE}\s+{STATIVE}\s+by\b", text, re.I))
        v["complex_tense"] = len(re.findall(
            rf"\b(?:(?:may|might|could|would|should|must|will|shall|can)\s+)?(?:have|has|had)\s+(?:been\s+)?(?:\w+ed|{PP_IRREG})\b",
            text, re.I))
        v["ing_main_verb"] = len(re.findall(rf"\b{BE}\s+\w+ing\b", text, re.I))
        v["nominalization"] = len(re.findall(r"\b(?:perform(?:s|ed)?|conduct(?:s|ed)?|carry out|carries out|make use of|makes use of)\b", text, re.I)) + len(re.findall(r"\b\w{4,}(?:tion|ment|ance|ence)\s+of\b", text, re.I))
        v["phrasal_verb"], _ = count_ci(text, PHRASAL)
        v["banned_word"], bh = count_ci(text, BANNED)
        v["marketing_adjective"], mh = count_ci(text, MARKETING)
        v["modal_hedge"], _ = count_ci(text, MODAL_HEDGE)
        func_words = FUNC_WORDS
    paras = [p for p in re.split(r"\n\s*\n", raw) if p.strip()]
    v["long_paragraph(>6s)"] = sum(1 for p in paras if len(sentences(strip_code(p))) > 6)
    em = raw.count("—") + raw.count("–")
    trains = noun_trains(text, func_words)
    if strict and lang == "en":
        n_strict, sh = count_ci(text, STRICT_BANNED)
        # "may" is matched case-sensitively so the month "May" stays clean
        n_strict += len(re.findall(r"(?<![A-Za-z])may(?![a-z])", text))
        v["strict_banned_word"] = n_strict
        v["em_dash"] = em
    elif strict:
        v["em_dash"] = em
    total = sum(v.values())
    return {
        "score_version": SCORE_VERSION,
        "lang": lang,
        "mode": "strict" if strict else "flavored",
        "words": words, "sentences": len(sents),
        "violations": v, "total": total,
        "total_per100w": round(total*100.0/words, 2),
        "em_dash(slop-marker)": em,
        "noun_train(>=4w,marker)": len(trains),
        "longest_sentence_words": (max(longs)[0] if longs else max((wc(s) for s in sents), default=0)),
        "sample_marketing": list(dict.fromkeys(mh))[:6],
        "sample_banned": list(dict.fromkeys(bh))[:6],
        "sample_noun_train": trains[:3],
    }

if __name__ == "__main__":
    args = sys.argv[1:]
    strict = "--strict" in args
    as_json = "--json" in args
    lang = "en"
    if "--lang" in args:
        i = args.index("--lang")
        lang = args[i + 1]
        del args[i:i + 2]
    fail_over = None
    if "--fail-over" in args:
        i = args.index("--fail-over")
        fail_over = float(args[i + 1])
        del args[i:i + 2]
    files = [a for a in args if a not in ("--strict", "--json")]
    worst = 0.0
    if not files:
        sys.stdin.reconfigure(encoding="utf-8")
        r = lint(sys.stdin.read(), strict=strict, lang=lang)
        print(json.dumps(r, indent=2, ensure_ascii=False))
        worst = r["total_per100w"]
    else:
        exp = []
        for f in files: exp += sorted(glob.glob(f)) if any(c in f for c in "*?[") else [f]
        for f in exp:
            with open(f, encoding="utf-8") as fh: r = lint(fh.read(), strict=strict, lang=lang)
            worst = max(worst, r["total_per100w"])
            if as_json:
                print(json.dumps({"file": f, **r}, indent=2, ensure_ascii=False))
            else:
                print(f"{os.path.basename(f):32} words={r['words']:4d} total={r['total']:3d} per100w={r['total_per100w']:6.2f} em_dash={r['em_dash(slop-marker)']:2d}")
    if fail_over is not None and worst > fail_over:
        sys.exit(1)
