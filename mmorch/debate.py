"""Debate cross-family sobre una branch: refutador -> habilitador -> juez.

Por que existe (medido 2026-08-25): un solo refutador escéptico refuta 4 de 4,
incluidas branches que un humano mergeó y un commit que arregló una caída real.
No falla por decir mentiras — falla porque sus objeciones son CIERTAS e
INMATERIALES ("no explicás por qué 20 segundos y no otro número"). Lo que falta
no es más escepticismo: es una VARA DE MATERIALIDAD.

Los tres roles, cada uno en una familia distinta (y distinta de la que escribió
el parche, que es deepseek por defecto):

  refutador   busca puntos concretos en contra, con evidencia del diff.
  habilitador defiende el cambio punto por punto. No opina del conjunto:
              responde CADA objeción, que es lo que la hace refutable.
  juez        decide, por punto, una sola cosa: ¿esto justifica BLOQUEAR el
              merge? No si es cierto — si es material. Un punto cierto e
              inmaterial se descarta explícitamente.

Dinámico: el número de puntos lo fija el refutador, no una constante. El
habilitador responde exactamente esos N y el juez falla exactamente esos N. Un
diff sin nada en contra sale del paso 1 sin gastar los otros dos.

Costo: 3 llamadas por branch (~USD 0.004). Se corre SOLO sobre lo que el triage
mecánico dejó pasar.
"""

from __future__ import annotations

# Candidatos por rol, en orden de preferencia. Se elige el primero cuya familia
# no la haya tomado ya otro rol; si el proveedor falla en vivo, se cae al
# siguiente. Medido 2026-08-26: moonshot sin cuota (429) y glm tardando 13.6s en
# un "deci OK" tumbaron las 4 corridas de la primera version, que fijaba UN
# modelo por rol. Cuatro proveedores son cuatro puntos de falla.
#
# El juez va ultimo a proposito: su payload es el mas chico (puntos + defensas,
# sin el diff), asi que ahi duele menos un modelo lento.
ROLES = {
    "refutador": ["gemini-3.1-flash-lite", "deepseek-chat", "glm-4.5-air"],
    "habilitador": ["deepseek-chat", "glm-4.5-air", "gemini-2.5-flash-lite"],
    "juez": ["glm-4.6", "gemini-2.5-flash", "deepseek-reasoner", "kimi-k2.5"],
}
_MAX_DIFF = 12000
_MAX_PUNTOS = 10        # tope de costo; si se recorta, se REPORTA (nunca en silencio)

_PUNTOS_SCHEMA = {
    "type": "object",
    "properties": {"puntos": {"type": "array", "items": {
        "type": "object",
        "properties": {"titulo": {"type": "string"}, "evidencia": {"type": "string"}},
        "required": ["titulo"]}}},
    "required": ["puntos"],
}
_DEFENSAS_SCHEMA = {
    "type": "object",
    "properties": {"defensas": {"type": "array", "items": {
        "type": "object",
        "properties": {"i": {"type": "number"}, "respuesta": {"type": "string"}},
        "required": ["i", "respuesta"]}}},
    "required": ["defensas"],
}
_JUICIOS_SCHEMA = {
    "type": "object",
    "properties": {"veredictos": {"type": "array", "items": {
        "type": "object",
        "properties": {"i": {"type": "number"}, "material": {"type": "boolean"},
                       "razon": {"type": "string"}},
        "required": ["i", "material"]}}},
    "required": ["veredictos"],
}

_P_REFUTAR = """Sos el REFUTADOR. Este es el diff de un cambio propuesto por un
modelo sobre un repo de orquestacion. Paso la suite completa, asi que "los tests
pasan" NO es evidencia de nada.

Enumera los puntos CONCRETOS en contra, con la evidencia textual del diff (cita
la linea). Formas de fallar medidas en cambios reales de este repo:
- un nombre (parametro, flag, funcion) que promete una garantia que el codigo no da
- un borde que se mueve sin que el autor lo mencione (fechas, vacios, off-by-one)
- maquinaria agregada que no cambia QUE puede fallar
- un comentario o log borrado que explicaba una decision o un bug ya arreglado

Si no encontras nada concreto, devolve la lista VACIA. No inventes para llenar.

DIFF:
{diff}

Devolve EXACTAMENTE este JSON: {{"puntos": [{{"titulo": str, "evidencia": str}}]}}"""

_P_HABILITAR = """Sos el HABILITADOR. Defendes este cambio. Otro modelo levanto
objeciones; respondé UNA POR UNA, por indice.

Para cada una, la respuesta util es una de estas:
- la objecion es factualmente incorrecta (mostra por que, con el diff)
- es cierta pero INMATERIAL (el caso no ocurre, o si ocurre no hace daño)
- es cierta y material (admitila; defender lo indefendible te quita credibilidad)

DIFF:
{diff}

OBJECIONES:
{puntos}

Devolve EXACTAMENTE este JSON, un item por objecion y con su MISMO indice:
{{"defensas": [{{"i": int, "respuesta": str}}]}}"""

_P_JUZGAR = """Sos el JUEZ. Viste una objecion y su defensa. Por CADA punto
decidis UNA sola cosa: ¿esto justifica BLOQUEAR el merge?

No juzgues si la objecion es cierta — muchas lo son y no importan. Juzga si es
MATERIAL. Marca material=true solo si se cumple alguna:
- rompe o cambia comportamiento que alguien depende, sin que se haya declarado
- una garantia prometida por un nombre no se cumple (quien lea el codigo se
  confiara de algo que no pasa)
- se pierde informacion que no se puede recuperar (el porque de una decision)

Marca material=false para: estilo, gustos, "podria ser mas robusto", "no
explica por que ese numero", cobertura de tests faltante, y toda objecion
verdadera cuyo caso no ocurre en la practica.

OBJECIONES Y DEFENSAS:
{debate}

Devolve EXACTAMENTE este JSON, un item por punto y con su MISMO indice:
{{"veredictos": [{{"i": int, "material": bool, "razon": str}}]}}"""


def _familias(roles: dict) -> dict:
    from mmorch.config import family_of
    return {rol: family_of(m) for rol, m in roles.items()}


class _Mesa:
    """Elige un modelo por rol y lo reemplaza si el proveedor se cae.

    Invariante: mientras haya candidatos, dos roles NUNCA comparten familia —
    un punto ciego compartido no se arregla ocultando quien escribio que (los
    priors son los mismos igual). Si se agotan los candidatos de familias
    libres, se permite REPETIR familia y queda anotado en `degradado`: un
    veredicto de familia repetida vale menos que uno cruzado, pero vale mas
    que no tener veredicto."""

    def __init__(self, candidatos: dict, llm_fn):
        from mmorch.config import family_of
        self._family_of = family_of
        self.candidatos = {k: (list(v) if isinstance(v, (list, tuple)) else [v])
                           for k, v in candidatos.items()}
        self.llm_fn = llm_fn
        self.elegidos: dict[str, str] = {}
        self.degradado: list[str] = []

    def _tomadas(self, rol: str) -> set:
        return {self._family_of(m) for r, m in self.elegidos.items() if r != rol}

    def pedir(self, rol: str, prompt: str, schema: dict) -> dict:
        libres = [m for m in self.candidatos[rol]
                  if self._family_of(m) not in self._tomadas(rol)]
        orden = libres + [m for m in self.candidatos[rol] if m not in libres]
        ultimo: Exception | None = None
        for modelo in orden:
            try:
                out = self.llm_fn(modelo, prompt, schema=schema)
            except Exception as e:                 # proveedor caido/lento/sin cuota
                ultimo = e
                self.degradado.append(f"{rol}: {modelo} fallo ({type(e).__name__})")
                continue
            self.elegidos[rol] = modelo
            if modelo not in libres:
                self.degradado.append(f"{rol}: {modelo} REPITE familia "
                                      f"{self._family_of(modelo)}")
            return out
        raise ultimo or RuntimeError(f"sin candidatos para {rol}")


def debatir_branch(repo: str, branch: str, *, base: str, roles: dict | None = None,
                   git_fn=None, llm_fn=None) -> dict:
    """Debate de 3 roles sobre el diff de `branch`. Devuelve
    {refutado, puntos:[{titulo, defensa, material, razon}], costo_usd, recortados}.

    `refutado` = existe al menos UN punto que el juez marco material.
    Fail-open: cualquier error de infra devuelve refutado=False (un problema de
    API jamas descarta trabajo; se manda al humano igual)."""
    roles = roles or ROLES
    if git_fn is None:
        from mmorch.automerge import _git as git_fn
    if llm_fn is None:
        from mmorch.schema import gated_json

        def llm_fn(modelo, prompt, *, schema):
            return gated_json(modelo, [{"role": "user", "content": prompt}],
                              schema=schema, pattern="debate_branch",
                              phase="triage_branch", temperature=0.0)

    mesa = _Mesa(roles, llm_fn)
    vacio = {"refutado": False, "puntos": [], "costo_usd": 0.0, "recortados": 0}
    d = git_fn(repo, "diff", f"{base}..{branch}")
    if d.returncode != 0 or not d.stdout.strip():
        return {**vacio, "error": "diff vacio o git fallo"}
    diff = d.stdout[:_MAX_DIFF]
    costo = 0.0
    try:
        out = mesa.pedir("refutador", _P_REFUTAR.format(diff=diff),
                         _PUNTOS_SCHEMA)
        costo += float(out.get("_cost_usd") or 0.0)
        puntos = [p for p in (out.get("puntos") or []) if p.get("titulo")]
        recortados = max(0, len(puntos) - _MAX_PUNTOS)
        puntos = puntos[:_MAX_PUNTOS]
        if not puntos:
            # nada en contra: no se gastan los otros dos roles
            return {**vacio, "costo_usd": costo, "modelos": dict(mesa.elegidos),
                    "degradado": list(mesa.degradado)}

        listado = "\n".join(
            f"[{i}] {p['titulo']}\n    evidencia: {p.get('evidencia', '')}"
            for i, p in enumerate(puntos))
        out = mesa.pedir("habilitador",
                         _P_HABILITAR.format(diff=diff, puntos=listado),
                         _DEFENSAS_SCHEMA)
        costo += float(out.get("_cost_usd") or 0.0)
        defensas = {int(x["i"]): x["respuesta"] for x in (out.get("defensas") or [])
                    if str(x.get("i", "")).lstrip("-").isdigit()}

        debate = "\n".join(
            f"[{i}] OBJECION: {p['titulo']}\n    evidencia: {p.get('evidencia', '')}"
            f"\n    DEFENSA: {defensas.get(i, '(el habilitador no respondio este punto)')}"
            for i, p in enumerate(puntos))
        out = mesa.pedir("juez", _P_JUZGAR.format(debate=debate), _JUICIOS_SCHEMA)
        costo += float(out.get("_cost_usd") or 0.0)
        juicios = {int(x["i"]): x for x in (out.get("veredictos") or [])
                   if str(x.get("i", "")).lstrip("-").isdigit()}
    except Exception as e:
        return {**vacio, "costo_usd": costo, "modelos": dict(mesa.elegidos),
                "degradado": list(mesa.degradado),
                "error": f"{type(e).__name__}: {str(e)[:150]}"}

    detalle = []
    for i, p in enumerate(puntos):
        j = juicios.get(i, {})
        detalle.append({"titulo": p["titulo"], "evidencia": p.get("evidencia", ""),
                        "defensa": defensas.get(i, ""),
                        # un punto que el juez no fallo NO bloquea: sin veredicto
                        # explicito no hay materialidad demostrada
                        "material": bool(j.get("material")),
                        "razon": j.get("razon", "")})
    return {"refutado": any(p["material"] for p in detalle), "puntos": detalle,
            "costo_usd": round(costo, 6), "recortados": recortados,
            "modelos": dict(mesa.elegidos),
            "familias": _familias(mesa.elegidos),
            "degradado": list(mesa.degradado)}


def _demo() -> None:
    """Self-check sin API: los 3 roles inyectados, el juez manda."""
    import types

    def _git(repo, *args):
        return types.SimpleNamespace(returncode=0, stdout="- a\n+ b\n", stderr="")

    def _llm(modelo, prompt, *, schema):
        if schema is _PUNTOS_SCHEMA:
            return {"puntos": [{"titulo": "cierto pero inmaterial", "evidencia": "l.1"},
                               {"titulo": "el flag miente", "evidencia": "l.2"}]}
        if schema is _DEFENSAS_SCHEMA:
            return {"defensas": [{"i": 0, "respuesta": "no ocurre"},
                                 {"i": 1, "respuesta": "admitido"}]}
        return {"veredictos": [{"i": 0, "material": False, "razon": "gusto"},
                               {"i": 1, "material": True, "razon": "promete y no cumple"}]}

    r = debatir_branch("r", "b", base="main", git_fn=_git, llm_fn=_llm)
    assert r["refutado"] is True
    assert [p["material"] for p in r["puntos"]] == [False, True]

    # sin puntos -> no se gastan los otros dos roles
    def _llm_limpio(modelo, prompt, *, schema):
        assert schema is _PUNTOS_SCHEMA, "no debe llamar al habilitador ni al juez"
        return {"puntos": []}

    assert debatir_branch("r", "b", base="main", git_fn=_git,
                          llm_fn=_llm_limpio)["refutado"] is False

    # el juez que no falla un punto no bloquea
    def _llm_mudo(modelo, prompt, *, schema):
        if schema is _PUNTOS_SCHEMA:
            return {"puntos": [{"titulo": "algo", "evidencia": "l.1"}]}
        if schema is _DEFENSAS_SCHEMA:
            return {"defensas": []}
        return {"veredictos": []}

    assert debatir_branch("r", "b", base="main", git_fn=_git,
                          llm_fn=_llm_mudo)["refutado"] is False

    # familia repetida: NO explota (un veredicto degradado vale mas que ninguno),
    # pero queda anotado — nunca se hace pasar por un debate cruzado
    r = debatir_branch("r", "b", base="main", git_fn=_git, llm_fn=_llm,
                       roles={"refutador": ["deepseek-chat"],
                              "habilitador": ["deepseek-reasoner"],
                              "juez": ["kimi-k2.5"]})
    assert any("REPITE familia" in d for d in r["degradado"]), r["degradado"]

    # proveedor caido: se cae al siguiente candidato en vez de quedarse sin veredicto
    caidos = {"n": 0}

    def _llm_flaky(modelo, prompt, *, schema):
        if modelo == "gemini-3.1-flash-lite":
            caidos["n"] += 1
            raise RuntimeError("429")
        return _llm(modelo, prompt, schema=schema)

    r = debatir_branch("r", "b", base="main", git_fn=_git, llm_fn=_llm_flaky)
    assert caidos["n"] == 1 and r["refutado"] is True
    assert r["modelos"]["refutador"] != "gemini-3.1-flash-lite"
    assert any("fallo" in d for d in r["degradado"])

    print("debate ok (5 casos)")


if __name__ == "__main__":
    _demo()
