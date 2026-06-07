"""schema (§9) — structured-output gates. Hoy los parsers de mmorch son best-effort
(regex/JSON con fallback). Esto los gradua a 'validado-o-rechaza': el output del
modelo se valida contra un JSON Schema chico; si no parsea o no valida, se REINTENTA
una vez con el error como feedback; si se agota, tira SchemaGateError (no se devuelve
basura silenciosa).

Validador minimo embebido (sin dependencia jsonschema): cubre type/required/properties
/enum/items, que es lo que usan los shapes de mmorch (verdict, tier, etc.). Para
schemas complejos se puede swappear por la lib jsonschema sin tocar la interfaz.
"""
from __future__ import annotations

import json
from typing import Any

from .providers import call

_TYPE_MAP = {
    "object": dict, "array": list, "string": str,
    "number": (int, float), "integer": int, "boolean": bool, "null": type(None),
}


class SchemaGateError(ValueError):
    """El modelo no produjo JSON valido contra el schema tras los reintentos."""


def extract_json(text: str) -> Any | None:
    """Best-effort: saca el primer objeto/array JSON del texto (strip de code-fences)."""
    if not text:
        return None
    s = text.strip()
    if s.startswith("```"):
        s = s.strip("`")
        s = s.removeprefix("json").strip()
    # buscar el span {..} o [..] mas externo
    starts = [i for i in (s.find("{"), s.find("[")) if i != -1]
    if not starts:
        return None
    start = min(starts)
    end = max(s.rfind("}"), s.rfind("]"))
    if end <= start:
        return None
    try:
        return json.loads(s[start:end + 1])
    except Exception:
        return None


def validate(data: Any, schema: dict, _path: str = "$") -> list[str]:
    """Devuelve lista de errores ([] = valido). Soporta type/required/properties/
    enum/items. No-soportado se ignora (permisivo hacia adelante)."""
    errs: list[str] = []
    # enum
    if "enum" in schema and data not in schema["enum"]:
        errs.append(f"{_path}: {data!r} no esta en enum {schema['enum']}")
        return errs
    # type
    t = schema.get("type")
    if t:
        py = _TYPE_MAP.get(t)
        # bool es subclase de int: number/integer NO deben aceptar bool.
        if t in ("number", "integer") and isinstance(data, bool):
            errs.append(f"{_path}: se esperaba {t}, vino boolean")
            return errs
        if py and not isinstance(data, py):
            errs.append(f"{_path}: se esperaba {t}, vino {type(data).__name__}")
            return errs
    # object
    if t == "object" or isinstance(data, dict):
        for req in schema.get("required", []):
            if not isinstance(data, dict) or req not in data:
                errs.append(f"{_path}: falta campo requerido '{req}'")
        props = schema.get("properties", {})
        if isinstance(data, dict):
            for k, subschema in props.items():
                if k in data:
                    errs += validate(data[k], subschema, f"{_path}.{k}")
    # array
    if (t == "array" or isinstance(data, list)) and "items" in schema and isinstance(data, list):
        for idx, item in enumerate(data):
            errs += validate(item, schema["items"], f"{_path}[{idx}]")
    return errs


def gated_json(
    model: str,
    messages: list[dict],
    *,
    schema: dict,
    max_retries: int = 2,
    pattern: str = "schema_gate",
    node: str = "gen",
    phase: str = "",
    temperature: float = 0.0,
) -> dict:
    """Llama al modelo y EXIGE JSON valido contra `schema`. Reintenta con el error
    como feedback. Tira SchemaGateError si se agotan los reintentos. Devuelve el dict
    validado (+ clave privada '_cost_usd' con el costo acumulado)."""
    msgs = list(messages)
    last_err = "?"
    total_cost = 0.0
    for attempt in range(max_retries + 1):
        res = call(model, msgs, pattern=pattern, node=f"{node}#{attempt}",
                   phase=phase, temperature=temperature)
        total_cost += getattr(res, "cost_usd", 0.0)
        data = extract_json(res.text)
        if data is None:
            last_err = "no se encontro JSON en la respuesta"
        else:
            errs = validate(data, schema)
            if not errs:
                if isinstance(data, dict):
                    data["_cost_usd"] = round(total_cost, 6)
                return data
            last_err = "; ".join(errs)
        # feedback para el reintento
        msgs = list(messages) + [
            {"role": "assistant", "content": res.text},
            {"role": "user", "content": (
                f"Tu respuesta fallo la validacion de schema: {last_err}. "
                f"Devolve SOLO JSON valido que cumpla el schema. Nada de texto extra.")},
        ]
    raise SchemaGateError(
        f"{model} no produjo JSON valido tras {max_retries + 1} intentos. "
        f"Ultimo error: {last_err}")
