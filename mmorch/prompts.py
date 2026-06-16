"""prompts — construccion de mensajes PREFIX-STABLE pa maximizar el cache-hit de DeepSeek.

DeepSeek (y otros) cachean por PREFIJO: si el comienzo del request es byte-identico a uno
previo, esos tokens se cobran al precio cache (~50x mas barato, ver cost.py). La palanca:
poner TODO lo estable (system, contexto compartido, few-shots) PRIMERO y byte-identico entre
calls, y solo lo volatil (la query puntual) AL FINAL. Asi N calls que comparten contexto
pagan el prefijo una vez.

cacheable_messages() canonicaliza el bloque estable (orden fijo, serializacion estable) pa
que el prefijo NO cambie por reordenamiento de dicts. prefix_signature() permite medir/test
que dos calls comparten prefijo. Observabilidad real via metrics.cache_stats (hit-rate).
"""
from __future__ import annotations

import hashlib
import json


def _canon(block) -> str:
    """Serializacion ESTABLE: dict -> JSON con keys ordenadas; lista -> join; str -> tal cual.
    Garantiza que el mismo contenido produzca SIEMPRE los mismos bytes (clave del cache-hit)."""
    if isinstance(block, str):
        return block
    if isinstance(block, dict):
        return json.dumps(block, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    if isinstance(block, (list, tuple)):
        return "\n".join(_canon(b) for b in block)
    return str(block)


def cacheable_messages(system: str, shared: list | dict | str | None,
                       query: str, *, examples: list[tuple[str, str]] | None = None) -> list[dict]:
    """Arma messages prefix-stable: [system+shared canonico] + few-shots + [query volatil].
    El prefijo (todo menos la query) es byte-identico mientras system/shared/examples no cambien
    -> calls sucesivas pegan el cache. SOLO `query` debe variar entre calls del mismo contexto."""
    msgs: list[dict] = []
    sys_parts = [system.strip()] if system else []
    if shared is not None:
        c = _canon(shared).strip()
        if c:
            sys_parts.append("CONTEXTO ESTABLE:\n" + c)
    if sys_parts:
        msgs.append({"role": "system", "content": "\n\n".join(sys_parts)})
    for u, a in (examples or []):
        msgs.append({"role": "user", "content": u})
        msgs.append({"role": "assistant", "content": a})
    msgs.append({"role": "user", "content": query})   # <- lo VOLATIL, siempre al final
    return msgs


def prefix_signature(messages: list[dict]) -> str:
    """Hash del prefijo (todo menos el ultimo mensaje = la query volatil). Dos calls con la
    misma firma comparten prefijo cacheable. Util pa test/observabilidad del cache-hit."""
    prefix = messages[:-1] if len(messages) > 1 else messages
    blob = "␟".join(f"{m.get('role')}:{m.get('content','')}" for m in prefix)
    return hashlib.sha256(blob.encode("utf-8")).hexdigest()[:16]


def shares_prefix(a: list[dict], b: list[dict]) -> bool:
    return prefix_signature(a) == prefix_signature(b)


# System prefix LAZY (minimal-code) pal path de EDICION (project_loop). Es un PREFIJO ESTABLE
# -> DeepSeek lo cachea entre las K iteraciones (se paga una vez). SEGURO de empujar fuerte
# porque mmorch verifica por EJECUCION: lo minimal-pero-roto lo filtra el gate de tests.
# Adaptado de Ponytail (DietrichGebert, MIT). Tweak propio (medido): el fail de `roman` fue un
# one-liner over-clever -> reforzamos "boring over clever, nada de one-liners cripticos".
LAZY_SYSTEM = """You are a lazy senior developer: lazy means efficient, not careless. The best \
code is the code never written. Follow this ladder and STOP at the first rung that holds:
1. Does it need to exist at all? Speculative need -> skip it (YAGNI).
2. Does the stdlib do it? Use it.
3. Does a native platform feature cover it? Use it.
4. Does an already-installed dependency solve it? Use it; never add a new one for a few lines.
5. Can it be one READABLE line? One line.
6. Otherwise: the minimum code that works.
Rules: no unrequested abstractions, no scaffolding "for later", deletion over addition, fewest \
files, shortest working diff. BORING over CLEVER -- never a cryptic one-liner that is hard to \
verify; a plain loop beats a bug-prone trick. Two options the same size -> the one that is \
correct on edge cases (lazy means less code, not a flimsier algorithm).
Never simplify away: input validation at trust boundaries, error handling that prevents data \
loss, security, accessibility, or anything explicitly requested."""
