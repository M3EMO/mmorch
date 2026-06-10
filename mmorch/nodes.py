"""nodes — el registry de la ORQUESTA: nombra a cada miembro que mmorch (el DIRECTOR)
conduce, su sección, qué algoritmo lo construye, y su estado.

Metáfora (de SELF-EVOLUTION-PLAN.md): mmorch+Opus = director determinista (NO músico).
Los nodos son los músicos. Secciones:
  - generator  (Voces)        : generan código/texto. Hoy LLMs; futuro gen:synth (no-LLM).
  - verifier   (Críticos)     : LLM-skeptics cross-family + CHECKERS deterministas.
  - router     (Primer violín): el cerebro que elige la jugada (bandit -> MLP).
  - soloist    (Especialistas): modelos que la FÁBRICA entrena en WSL (embedder, clasificadores).
  - memory     (Partitura)    : episodic + semantic, el score compartido.

Compone los registries que YA existen (config.REGISTRY de modelos + checkers.available())
en vez de duplicar — fuente única. Consultable: orchestra(), members(section), conductor().
"""
from __future__ import annotations

from dataclasses import dataclass, asdict

from .config import REGISTRY
from .checkers import available as _checkers


@dataclass
class Node:
    handle: str       # "<seccion>:<nombre>", ej "gen:deepseek-chat"
    section: str      # generator|verifier|router|soloist|memory
    kind: str         # llm | checker | bayesian | model | store | synth
    builder: str      # algoritmo/medio que lo construye ('-' si no aplica)
    status: str       # active | planned
    family: str = ""  # familia de modelo si aplica (deepseek|google|moonshot|...)
    note: str = ""


def conductor() -> dict:
    return {"name": "mmorch + Opus", "role": "orquestador determinista + tie-break",
            "es_musico": False}


# Solistas (modelos de la fábrica) y router — explícitos (no salen de un registry hoy).
_SOLOISTS = [
    Node("router:bandit", "router", "bayesian", "Thompson sampling (#9)", "active",
         note="elige brazo modelo@umbral; feedback.ThompsonBandit"),
    Node("router:mlp_v1", "router", "model", "DNN/MLP (#1)", "planned",
         note="Capa A v1.0; entrena en WSL+torch"),
    Node("model:cost_predictor", "soloist", "model", "cuantil -> GBRT (#2)", "active",
         note="predict.py; out_tokens/latencia, mejora a GBRT"),
    Node("model:code_embedder", "soloist", "model", "Contrastive/SimCLR (#26)", "active",
         note="GANA a bge-small en code-quality (radon AUC 0.88 vs 0.80); inferencia numpy "
              "pura (code_embedder.py), pesos en flywheel/code_embedder.npz"),
    Node("model:code_quality_clf", "soloist", "model", "RandomForest/GBRT (#3/#2)", "planned",
         note="clasificador entrenado con labels de EJECUCION (checkers)"),
    Node("memory:episodic", "memory", "store", "DuckDB append-only", "active"),
    Node("memory:semantic", "memory", "store", "bge-small embed (#13 k-NN recall)", "active"),
    Node("gen:synth", "generator", "synth", "program-synthesis / GNN-AST", "planned",
         note="generador NO-LLM: enumera+ejecuta vs tests; entiende ejecucion, no sintaxis"),
]


def orchestra() -> list[Node]:
    """Roster completo: modelos (config.REGISTRY) + checkers + solistas/router/memoria."""
    nodes: list[Node] = []
    for k, s in REGISTRY.items():
        is_verifier = "verifier" in s.role.lower()
        section = "verifier" if is_verifier else ("router" if "rout" in s.role.lower()
                                                  or "classif" in s.role.lower() else "generator")
        prefix = {"verifier": "verify", "router": "route", "generator": "gen"}[section]
        # activo si la familia tiene key configurada (kimi/moonshot = planned)
        status = "planned" if s.family == "moonshot" else "active"
        nodes.append(Node(f"{prefix}:{k}", section, "llm", "pre-entrenado (lenguaje)", status,
                          family=s.family, note=s.role))
    for c in _checkers():
        nodes.append(Node(f"check:{c}", "verifier", "checker", "determinista (cero API)", "active"))
    nodes.extend(_SOLOISTS)
    return nodes


def members(section: str | None = None) -> list[Node]:
    return [n for n in orchestra() if section is None or n.section == section]


def sections() -> dict[str, int]:
    out: dict[str, int] = {}
    for n in orchestra():
        out[n.section] = out.get(n.section, 0) + 1
    return out


def summary() -> dict:
    """Vista compacta pa observabilidad/MCP."""
    return {"conductor": conductor(), "sections": sections(),
            "roster": [asdict(n) for n in orchestra()]}
