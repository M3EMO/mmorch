"""sdlc — el builder: construye features por un pipeline de 6 etapas con gates deterministas (spec -> plan -> build
-> test -> review -> pr). Usa a mmorch como biblioteca (proveedores, sandbox, checkers); mmorch mide, sdlc construye.

    python -m sdlc init <repo>                          adopta un repo (escribe sdlc.toml)
    python -m sdlc doctor <repo> | --todos [raiz]       oraculo de convenciones (JSON; sin red, LLM ni escrituras)
    python -m sdlc init --upgrade <repo>                arreglos seguros de convenciones; commit y push
    python -m sdlc areas <repo> [--aprobar]             propuesta de areas para la spec viva
    python -m sdlc propiedades <repo> [--aprobar]       Hypothesis / fast-check en el repo
    python -m sdlc bd <repo>                            reglas y hooks de bd por append (AGENTS.md fuente)
    python -m sdlc lint-md <repo>                       contrato de AGENTS.md/CLAUDE.md (docs/agents/estructura-md.md)
    python -m sdlc.builder <plan.json> <carpeta>        arma un entregable modulo por modulo
Biblioteca: `from sdlc.pipeline import build_feature`.
"""
