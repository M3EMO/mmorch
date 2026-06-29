"""curiosity — deteccion de TENSION en la memoria (modulo cognitivo #3).

El punto fino: los embeddings dan SIMILITUD DE TEMA, no contradiccion logica. Cosine
alto no distingue "acuerda" de "contradice". Por eso v1 NO usa LLM-judge: surfacea
CANDIDATOS deterministas y el humano/orquestador juzga (merge / flag_contradiction /
dejar). La banda interesante es la que consolidate DEJA pasar:

    cosine >= 0.92   -> consolidate ya las mergea (dup)
    0.82 <= cosine < 0.92  -> TENSION: muy parecidas pero no auto-merge. Aca se
                              esconde redundancia o contradiccion. <- esto surfaceamos
    cosine < 0.82    -> temas distintos, sin tension

La capa LLM-clasificadora ("¿esto contradice?") se DIFIERE hasta que los candidatos
deterministas no alcancen (ponytail). La pregunta se arma por template, cero LLM.
Requiere embeddings (fastembed); sin ellos degrada a [] (no puede medir cosine).
"""
from __future__ import annotations

from pathlib import Path

from .memory import _DB_PATH, _connect, _cosine

LO = 0.82   # piso de la banda de tension
HI = 0.92   # techo = umbral de auto-merge de consolidate (arriba de esto, es dup)


MAX_PER_SCOPE = 500   # guardrail O(n^2): arriba de esto el scope se saltea (reportado)


def find_tension(scope: str | None = None, *, lo: float = LO, hi: float = HI,
                 max_per_scope: int = MAX_PER_SCOPE,
                 path: Path = _DB_PATH) -> dict:
    """Pares de notas vivas en el mismo scope con lo <= cosine < hi: muy parecidas
    pero bajo el umbral de merge. Candidatos a redundancia/contradiccion para que el
    caller decida. Devuelve {pairs:[{a,b,scope,cosine,question}], skipped:[{scope,n}]}.
    pairs ordenado por cosine desc.

    ponytail: O(n^2) por scope. Bien para single-user (scopes chicos). Un scope con
    > max_per_scope notas se SALTEA y se reporta en `skipped` (sin drop silencioso);
    para esa escala usar la extension vss/HNSW ya documentada en memory."""
    con = _connect(path)
    try:
        q = ("SELECT id, scope, text, embedding FROM semantic "
             "WHERE NOT tombstone AND NOT needs_review AND embedding IS NOT NULL")
        params: list = []
        if scope:
            q += " AND scope = ?"
            params.append(scope)
        rows = con.execute(q, params).fetchall()
    finally:
        con.close()

    by_scope: dict[str, list] = {}
    for rid, sc, text, emb in rows:
        by_scope.setdefault(sc, []).append((rid, text, list(emb)))

    out: list[dict] = []
    skipped: list[dict] = []
    for sc, items in by_scope.items():
        if len(items) > max_per_scope:
            skipped.append({"scope": sc, "n": len(items)})
            continue
        for i in range(len(items)):
            for j in range(i + 1, len(items)):
                c = _cosine(items[i][2], items[j][2])
                if lo <= c < hi:
                    a_id, a_txt, _ = items[i]
                    b_id, b_txt, _ = items[j]
                    out.append({
                        "a": int(a_id), "b": int(b_id), "scope": sc,
                        "cosine": round(c, 4),
                        "question": (f"Notas {a_id} y {b_id} (scope {sc}) son muy "
                                     f"parecidas (cosine {round(c, 3)}). ¿Redundantes "
                                     f"(merge), una supersede a la otra (flag), o ambas "
                                     f"validas?\n  [{a_id}] {a_txt}\n  [{b_id}] {b_txt}"),
                    })
    out.sort(key=lambda d: -d["cosine"])
    return {"pairs": out, "skipped": skipped}
