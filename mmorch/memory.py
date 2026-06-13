"""memory — memoria episodica + semantica para mmorch (DuckDB 2 capas).

Diseno sometido a verificacion cross-family (2026-06-07). Cambios aplicados tras
la refutacion del verificador Gemini, triados por Opus (single-user/localhost:
se descartan objeciones de threat-model SaaS multi-tenant):

  FIX A  coarse SIN keyword hard-gate. El keyword filter mata el punto de los
         embeddings (sinonimos). Coarse = scope-chain + ventana de recencia, solo.
         El rerank fino corre embeddings sobre TODO el in-scope window. (objecion 3)
  FIX B  fallback a episodic RAW. Si la nota destilada no alcanza, recall completa
         desde el log crudo inmutable. La destilacion lossy no pierde el hecho. (obj 5)
  FIX C  embeddings versionados: (emb_model, dim) por nota -> re-embed selectivo si
         cambia el modelo, sin corrupcion silenciosa. (objecion 6c)
  HARDEN brute-force cosine sobre 384d x ~10k = ms. Para >100k: extension `vss`
         (HNSW) documentada, no implementada aun. (objecion 1)

Capas:
  episodic  append-only INMUTABLE. El log de hechos. Nunca se edita.
  semantic  notas DESTILADAS (Thought-Retriever) + embedding. Editable, tombstone.

Embeddings LOCAL via fastembed (bge-small-en, 384d, ONNX/CPU, cero key, cero $).
Si fastembed no esta instalado -> degrade graceful: recall coarse-only (sin rerank),
write guarda embedding NULL. mmorch sigue andando (ethos graceful).
"""
from __future__ import annotations

import json
import time
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
_DB_PATH = ROOT / "logs" / "memory.duckdb"
_EMB_MODEL = "BAAI/bge-small-en-v1.5"
_EMB_DIM = 384

# Jerarquia de scopes, especifico -> general. Recall sube por la cadena.
SCOPE_ORDER = ["task_id", "subsector", "project_id", "mmorch_self", "global"]


# ---------------------------------------------------------------------------
# Embedding backend (pluggable, opcional)
# ---------------------------------------------------------------------------
_embedder = None
_embed_unavailable = False


def _get_embedder():
    """Lazy singleton de fastembed. None si no esta instalado (degrade graceful)."""
    global _embedder, _embed_unavailable
    if _embedder is not None or _embed_unavailable:
        return _embedder
    try:
        from fastembed import TextEmbedding
        _embedder = TextEmbedding(model_name=_EMB_MODEL)
    except Exception:
        _embed_unavailable = True
        _embedder = None
    return _embedder


def embed(text: str) -> list[float] | None:
    """Embedding 384d o None si fastembed no disponible."""
    emb = _get_embedder()
    if emb is None:
        return None
    vec = next(iter(emb.embed([text])))
    return [float(x) for x in vec]


def _cosine(a: list[float], b: list[float]) -> float:
    import numpy as np
    va, vb = np.asarray(a, dtype="float32"), np.asarray(b, dtype="float32")
    na, nb = np.linalg.norm(va), np.linalg.norm(vb)
    if na == 0 or nb == 0:
        return 0.0
    return float(va.dot(vb) / (na * nb))


# ---------------------------------------------------------------------------
# DB
# ---------------------------------------------------------------------------
def _connect(path: Path = _DB_PATH):
    import duckdb
    path.parent.mkdir(parents=True, exist_ok=True)
    con = duckdb.connect(str(path))
    con.execute("""
        CREATE TABLE IF NOT EXISTS episodic (
            id BIGINT, ts DOUBLE, scope VARCHAR, kind VARCHAR,
            actor VARCHAR, payload VARCHAR
        );
        CREATE SEQUENCE IF NOT EXISTS seq_episodic START 1;
        CREATE TABLE IF NOT EXISTS semantic (
            id BIGINT, ts DOUBLE, scope VARCHAR, text VARCHAR,
            embedding DOUBLE[], emb_model VARCHAR, dim INTEGER,
            source_ids VARCHAR, tombstone BOOLEAN DEFAULT FALSE
        );
        CREATE SEQUENCE IF NOT EXISTS seq_semantic START 1;
    """)
    # migracion: columna `verified` (verification coverage, Martin 2026) en DBs previas.
    # OJO: no usar ADD COLUMN IF NOT EXISTS — en DuckDB re-aplica el DEFAULT y PISA
    # los valores existentes cuando la columna ya esta. Chequear el schema primero.
    cols = {r[0] for r in con.execute(
        "SELECT column_name FROM information_schema.columns "
        "WHERE table_name = 'semantic'").fetchall()}
    if "verified" not in cols:
        con.execute("ALTER TABLE semantic ADD COLUMN verified BOOLEAN DEFAULT FALSE")
    return con


@dataclass
class Note:
    id: int
    ts: float
    scope: str
    text: str
    score: float = 0.0
    layer: str = "semantic"   # semantic | episodic (fallback)


# ---------------------------------------------------------------------------
# Write
# ---------------------------------------------------------------------------
def write_episode(scope: str, kind: str, payload: dict | str, *,
                  actor: str = "mmorch", path: Path = _DB_PATH) -> int:
    """Append inmutable al log de hechos. Devuelve el id."""
    con = _connect(path)
    try:
        eid = con.execute("SELECT nextval('seq_episodic')").fetchone()[0]
        pl = payload if isinstance(payload, str) else json.dumps(payload, ensure_ascii=False)
        con.execute("INSERT INTO episodic VALUES (?,?,?,?,?,?)",
                    [eid, time.time(), scope, kind, actor, pl])
        return int(eid)
    finally:
        con.close()


def write_note(scope: str, text: str, *, source_ids: list[int] | None = None,
               verified: bool = False, path: Path = _DB_PATH) -> int:
    """Persiste una nota destilada en la capa semantica + embedding (FIX C versionado).
    Si fastembed no esta -> embedding NULL, recall cae a coarse-only para esa nota.
    `verified=True` marca la nota como validada independientemente (cross-family o
    checker) — alimenta verification_coverage en stats()."""
    con = _connect(path)
    try:
        sid = con.execute("SELECT nextval('seq_semantic')").fetchone()[0]
        vec = embed(text)
        con.execute(
            "INSERT INTO semantic (id, ts, scope, text, embedding, emb_model, dim, "
            "source_ids, tombstone, verified) VALUES (?,?,?,?,?,?,?,?,FALSE,?)",
            [sid, time.time(), scope, text, vec,
             _EMB_MODEL if vec else None, _EMB_DIM if vec else None,
             json.dumps(source_ids or []), bool(verified)])
        return int(sid)
    finally:
        con.close()


def tombstone_note(note_id: int, *, path: Path = _DB_PATH) -> None:
    """Soft-delete (FIX/HARDEN): la nota deja de recuperarse. El episodic raw queda."""
    con = _connect(path)
    try:
        con.execute("UPDATE semantic SET tombstone=TRUE WHERE id=?", [note_id])
    finally:
        con.close()


# ---------------------------------------------------------------------------
# Recall (FIX A coarse sin keyword-gate; FIX B fallback episodic)
# ---------------------------------------------------------------------------
def _scope_chain(scope: str) -> list[str]:
    """Cadena especifico->general desde un scope dado. Si scope no esta en el orden
    conocido, se usa tal cual + global."""
    if scope in SCOPE_ORDER:
        i = SCOPE_ORDER.index(scope)
        return SCOPE_ORDER[i:]
    return [scope, "global"]


def recall(query: str, scope: str = "global", *, k: int = 5,
           window_days: float | None = None, path: Path = _DB_PATH) -> list[Note]:
    """Recall clinico 2-stage:
      COARSE  scope-chain + (opcional) ventana de recencia. SIN keyword-gate (FIX A).
      FINE    rerank por cosine del embedding sobre el candidate set del coarse.
              Si no hay embeddings (fastembed ausente / notas viejas) -> orden por
              recencia (coarse-only).
      FALLBACK si la capa semantica devuelve < k, completa desde episodic RAW (FIX B).
    """
    con = _connect(path)
    try:
        chain = _scope_chain(scope)
        placeholders = ",".join("?" for _ in chain)
        cutoff = time.time() - window_days * 86400 if window_days else 0.0
        # COARSE: solo scope + recencia. Nada de keyword.
        rows = con.execute(
            f"""SELECT id, ts, scope, text, embedding FROM semantic
                WHERE scope IN ({placeholders}) AND ts >= ? AND NOT tombstone
                ORDER BY ts DESC""",
            [*chain, cutoff]).fetchall()

        qvec = embed(query)
        notes: list[Note] = []
        if qvec is not None:
            # FINE: rerank por cosine sobre candidatos con embedding.
            scored = []
            for rid, ts, sc, text, emb in rows:
                if emb:
                    scored.append(Note(rid, ts, sc, text, _cosine(qvec, list(emb)), "semantic"))
            scored.sort(key=lambda n: -n.score)
            notes = scored[:k]
        else:
            # coarse-only: mas recientes primero.
            notes = [Note(rid, ts, sc, text, 0.0, "semantic")
                     for (rid, ts, sc, text, _emb) in rows[:k]]

        # FIX B: completar desde episodic raw si falta.
        if len(notes) < k:
            need = k - len(notes)
            erows = con.execute(
                f"""SELECT id, ts, scope, kind, payload FROM episodic
                    WHERE scope IN ({placeholders}) AND ts >= ?
                    ORDER BY ts DESC LIMIT ?""",
                [*chain, cutoff, need]).fetchall()
            for rid, ts, sc, kind, payload in erows:
                notes.append(Note(rid, ts, sc, f"[{kind}] {payload}", 0.0, "episodic"))
        return notes
    finally:
        con.close()


_TOKEN_RE = __import__("re").compile(r"[a-z0-9_]+")


def _tokens(text: str) -> list[str]:
    return _TOKEN_RE.findall(text.lower())


def recall_keyword(query: str, scope: str = "global", *, k: int = 5,
                   window_days: float | None = None, path: Path = _DB_PATH) -> list[Note]:
    """Recall por KEYWORD (idea Hermes FTS5): BM25-lite sobre las notas in-scope. Atrapa
    el termino EXACTO que el embedding difumina (nombres, flags, ids) y anda sin fastembed.
    Cero dep (tokenizador propio, no necesita la extension FTS de DuckDB)."""
    import math
    con = _connect(path)
    try:
        chain = _scope_chain(scope)
        ph = ",".join("?" for _ in chain)
        cutoff = time.time() - window_days * 86400 if window_days else 0.0
        rows = con.execute(
            f"""SELECT id, ts, scope, text FROM semantic
                WHERE scope IN ({ph}) AND ts >= ? AND NOT tombstone""",
            [*chain, cutoff]).fetchall()
        if not rows:
            return []
        qtok = set(_tokens(query))
        if not qtok:
            return []
        docs = [(rid, ts, sc, text, _tokens(text)) for rid, ts, sc, text in rows]
        N = len(docs)
        df = {t: 0 for t in qtok}
        for *_, toks in docs:
            s = set(toks)
            for t in qtok:
                if t in s:
                    df[t] += 1
        avgdl = sum(len(toks) for *_, toks in docs) / N
        k1, b = 1.5, 0.75
        scored = []
        for rid, ts, sc, text, toks in docs:
            dl = len(toks) or 1
            score = 0.0
            for t in qtok:
                if df[t] == 0:
                    continue
                tf = toks.count(t)
                if not tf:
                    continue
                idf = math.log(1 + (N - df[t] + 0.5) / (df[t] + 0.5))
                score += idf * (tf * (k1 + 1)) / (tf + k1 * (1 - b + b * dl / avgdl))
            if score > 0:
                scored.append(Note(rid, ts, sc, text, round(score, 4), "semantic"))
        scored.sort(key=lambda n: -n.score)
        return scored[:k]
    finally:
        con.close()


def recall_hybrid(query: str, scope: str = "global", *, k: int = 5,
                  window_days: float | None = None, path: Path = _DB_PATH) -> list[Note]:
    """Fusion de recall semantico (embedding) + keyword (BM25-lite) por Reciprocal Rank
    Fusion (RRF). Lo mejor de los dos: sinonimos del embedding + termino exacto del keyword.
    Si fastembed falta, degrada a keyword-only sin romper."""
    sem = recall(query, scope, k=k * 2, window_days=window_days, path=path)
    kw = recall_keyword(query, scope, k=k * 2, window_days=window_days, path=path)
    C = 60.0
    fused: dict[int, list] = {}
    for rank, n in enumerate(sem):
        fused.setdefault(n.id, [n, 0.0])[1] += 1.0 / (C + rank)
    for rank, n in enumerate(kw):
        fused.setdefault(n.id, [n, 0.0])[1] += 1.0 / (C + rank)
    out = sorted(fused.values(), key=lambda nv: -nv[1])
    return [Note(n.id, n.ts, n.scope, n.text, round(sc, 5), n.layer) for n, sc in out[:k]]


# ---------------------------------------------------------------------------
# Distillation (Thought-Retriever): condensar episodio -> nota durable
# ---------------------------------------------------------------------------
_DISTILL_SYS = (
    "Sos un destilador de memoria. Te doy un episodio (hecho/decision/resultado). "
    "Devolve UNA nota durable de 1-2 frases: SOLO lo que vale recordar a futuro "
    "(la decision, el por que, el resultado, la leccion). Sin relleno, sin fecha, "
    "sin meta-comentario. Si no hay nada digno de recordar, devolve exactamente: SKIP."
)


def distill(episode_text: str, *, gen_model=None, phase: str = "memory") -> str:
    """Condensa un episodio en una nota durable via modelo barato. 'SKIP' si nada
    vale la pena. (Thought-Retriever: se guarda el pensamiento destilado, no el raw.)"""
    from .config import DEFAULT_GENERATOR
    from .providers import call
    gen_model = gen_model or DEFAULT_GENERATOR
    res = call(gen_model,
               [{"role": "system", "content": _DISTILL_SYS},
                {"role": "user", "content": episode_text}],
               pattern="distill", node="distiller", phase=phase, temperature=0.0)
    return res.text.strip()


def remember(scope: str, episode_text: str, *, kind: str = "note", actor: str = "mmorch",
             verify: bool = False, gen_model=None, path: Path = _DB_PATH) -> dict:
    """Pipeline completo (invariante 7 del diseno):
      1. write_episode  -> log crudo INMUTABLE (siempre, nunca se pierde el hecho).
      2. distill        -> nota durable via modelo barato.
      3. (verify=True)  -> verificacion cross-family: la nota es FIEL al episodio?
                           Si el esceptico la refuta (nota lossy/infiel) -> NO se
                           persiste la nota; queda solo el raw. Anti-sicofancia.
      4. write_note     -> persiste la nota destilada + embedding, linkeada al raw.
    Devuelve {episode_id, note_id|None, distilled, persisted, refutations}."""
    eid = write_episode(scope, kind, episode_text, actor=actor, path=path)
    note = distill(episode_text, gen_model=gen_model)
    out = {"episode_id": eid, "note_id": None, "distilled": note,
           "persisted": False, "refutations": []}
    if note.strip().upper() == "SKIP" or not note.strip():
        return out
    if verify:
        from .patterns import adversarial_verify
        v = adversarial_verify(
            f"EPISODIO:\n{episode_text}\n\nNOTA DESTILADA:\n{note}",
            rubric=("La NOTA es un resumen FIEL y no-lossy del EPISODIO? Refuta si "
                    "omite un hecho critico, agrega algo que no estaba, o tergiversa "
                    "la decision/resultado. passed=true solo si es fiel y util."),
            gen_model=gen_model or _default_gen(), phase="memory")
        out["refutations"] = v.refutations
        if not v.passed:
            return out  # nota infiel -> solo queda el raw (FIX B la cubre en recall)
    # verified=True solo si la nota PASO la verificacion cross-family (verify=True
    # y no refutada — si refutada ya retornamos arriba). Sin verify: queda UNVERIFIED.
    nid = write_note(scope, note, source_ids=[eid], verified=verify, path=path)
    out["note_id"] = nid
    out["persisted"] = True
    return out


def _default_gen():
    from .config import DEFAULT_GENERATOR
    return DEFAULT_GENERATOR


# ---------------------------------------------------------------------------
# Consolidacion periodica (Martin 2026: merge dups + compress, cada ~10 sesiones)
# ---------------------------------------------------------------------------
def _is_dup(a, b, thr: float) -> bool:
    """a/b = (id, ts, scope, text, embedding, verified). Dup si texto identico
    (normalizado) o cosine >= thr cuando ambos tienen embedding. Determinista,
    cero API."""
    if a[3].strip().lower() == b[3].strip().lower():
        return True
    if a[4] and b[4]:
        return _cosine(list(a[4]), list(b[4])) >= thr
    return False


def _pick_keeper(cluster: list) -> tuple:
    """Verificada > reciente: una nota validada no se pisa con una sin validar."""
    return sorted(cluster, key=lambda r: (bool(r[5]), r[1]), reverse=True)[0]


def consolidate(scope: str | None = None, *, sim_threshold: float = 0.92,
                max_bytes: int = 50_000, dry_run: bool = False,
                path: Path = _DB_PATH) -> dict:
    """Mantenimiento periodico de la capa semantica (correr cada ~10 sesiones):
    mergea near-duplicados POR scope (texto identico o cosine >= sim_threshold),
    tombstoneando los perdedores — keeper: verificada primero, despues la mas
    reciente. El episodic raw NUNCA se toca (invariante: inmutable) y la corrida
    queda auditada como evento episodico kind='consolidation'.

    No borra por tamano: si las notas vivas superan max_bytes solo flaggea
    over_budget=True (decidir que podar es juicio del caller — gated).
    dry_run=True reporta sin tocar nada."""
    con = _connect(path)
    try:
        q = ("SELECT id, ts, scope, text, embedding, verified FROM semantic "
             "WHERE NOT tombstone")
        params: list = []
        if scope:
            q += " AND scope = ?"
            params.append(scope)
        rows = con.execute(q + " ORDER BY ts", params).fetchall()
    finally:
        con.close()

    by_scope: dict[str, list] = {}
    for r in rows:
        by_scope.setdefault(r[2], []).append(r)

    merged: list[dict] = []
    tomb_ids: list[int] = []
    for sc, items in by_scope.items():
        used: set[int] = set()
        for i, anchor in enumerate(items):
            if anchor[0] in used:
                continue
            cluster = [anchor]
            for cand in items[i + 1:]:
                if cand[0] not in used and _is_dup(anchor, cand, sim_threshold):
                    cluster.append(cand)
            if len(cluster) > 1:
                used.update(c[0] for c in cluster)
                keep = _pick_keeper(cluster)
                drop = [c[0] for c in cluster if c[0] != keep[0]]
                merged.append({"kept": int(keep[0]), "tombstoned": [int(d) for d in drop]})
                tomb_ids.extend(drop)

    if not dry_run:
        for nid in tomb_ids:
            tombstone_note(int(nid), path=path)
        write_episode("mmorch_self", "consolidation",
                      {"scope": scope or "*", "clusters": len(merged),
                       "tombstoned": len(tomb_ids)}, path=path)

    live = [r for r in rows if r[0] not in set(tomb_ids)]
    nbytes = sum(len(r[3].encode("utf-8")) for r in live)
    return {"merged": merged, "tombstoned": len(tomb_ids),
            "live_notes": len(live), "bytes": nbytes,
            "over_budget": nbytes > max_bytes, "dry_run": dry_run}


def stats(path: Path = _DB_PATH) -> dict:
    con = _connect(path)
    try:
        ep = con.execute("SELECT count(*) FROM episodic").fetchone()[0]
        se = con.execute("SELECT count(*) FROM semantic WHERE NOT tombstone").fetchone()[0]
        embd = con.execute("SELECT count(*) FROM semantic WHERE embedding IS NOT NULL").fetchone()[0]
        ver = con.execute("SELECT count(*) FROM semantic WHERE verified AND NOT tombstone").fetchone()[0]
        # verification coverage (Martin 2026): % del aprendizaje vivo validado
        # independientemente. Predice utilidad de la memoria (73% Fable vs 17% Opus).
        return {"episodic": int(ep), "semantic": int(se), "embedded": int(embd),
                "verified": int(ver),
                "verification_coverage": (round(ver / se, 4) if se else None),
                "emb_backend": (None if _get_embedder() is None else _EMB_MODEL)}
    finally:
        con.close()
