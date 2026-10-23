"""sdlc — el builder: construye features por un pipeline de 6 etapas con gates deterministas (spec -> plan -> build
-> test -> review -> pr). Usa a mmorch como biblioteca (proveedores, sandbox, checkers); mmorch mide, sdlc construye.

    python -m sdlc init <repo>                          adopta un repo (escribe sdlc.toml)
    python -m sdlc.builder <plan.json> <carpeta>        arma un entregable modulo por modulo
Biblioteca: `from sdlc.pipeline import build_feature`.
"""
