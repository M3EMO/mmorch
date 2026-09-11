# Spec: Detector de atasco en `build_unit`

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.
Cada requisito lleva un ID `R<n>` unico. El gate `trazabilidad` cruza esos IDs con `plan.md` y con los tests.

## Contrato

Unico archivo modificado: `mmorch/project_driver.py` (no se tocan tests ni otros archivos).

```python
from collections.abc import Callable

# mmorch/project_driver.py
def build_unit(
    unit: dict,
    *,
    build_fn: Callable[[dict], str],
    gate_fn: Callable[[dict, str], tuple[bool, str]],
    max_fix: int = 3,
    use_cache: bool = False,
) -> dict[str, object]:
    ...

# Retorno: {"status": str, "detail": str}
#   status == "escalate" en los dos caminos de rechazo cubiertos por esta spec.
#   detail: str; contiene "atascado" cuando el escalado es por atasco (R3).
```

Firma observable segun los tests de aceptacion: `unit` posicional, `build_fn`, `gate_fn`, `max_fix` y `use_cache` por keyword. Los parametros preexistentes que no aparecen en los tests conservan nombre y default.

`build_fn(unit)` devuelve el codigo generado como `str` (se compara byte a byte contra el de la vuelta anterior).
`gate_fn(unit, codigo)` devuelve `(ok: bool, motivo: str)`; `ok == False` es rechazo.

## Requisitos

- R1: `build_unit` evalua las vueltas en orden ascendente `1..max_fix`; dentro de cada vuelta el orden de evaluacion es (1) `build_fn(unit)`, (2) `gate_fn(unit, codigo)`, (3) decision de corte; en la vuelta 1 no existe codigo previo, por lo que el detector de atasco no puede dispararse.
- R2: Si en la vuelta `n > 1` `gate_fn` rechaza y el codigo devuelto por `build_fn` es byte-identico al codigo de la vuelta `n - 1` de la misma `unit`, `build_unit` devuelve `status == "escalate"` en esa misma vuelta.
- R3: Cuando el escalado ocurre por R2, el `detail` devuelto contiene la palabra `atascado` (el test la busca como subcadena de `detail.lower()`).
- R4: El corte por atasco se evalua antes que el agotamiento de `max_fix`: al dispararse R2, `build_unit` retorna de inmediato y no invoca `build_fn` en las vueltas restantes.
- R5: Si los codigos de vueltas consecutivas no son byte-identicos, el detector no se activa y `build_unit` agota `max_fix` vueltas (invoca `build_fn` exactamente `max_fix` veces) antes de devolver `status == "escalate"`.

## Casos borde

- R1: `max_fix = 1` con cualquier `build_fn`: hay una sola vuelta, sin codigo previo, y el detector no puede activarse.
- R2: `build_fn` devuelve `""` en dos vueltas consecutivas: la cadena vacia es byte-identica a si misma y cuenta como atasco.
- R2: codigos de igual longitud con un byte distinto (por ejemplo `"def f():\n    return 1\n"` vs `"def f():\n    return 2\n"`) no son byte-identicos: no hay atasco.
- R4: `max_fix = 3` y atasco en la vuelta 2: `build_fn` se invoca 2 veces, nunca 3.
- R5: `max_fix = 2` con codigos distintos en cada vuelta: 2 invocaciones y `status == "escalate"` por agotamiento, no por atasco.
- R5: el rechazo del gate es constante (`(False, "rojo")`) en todas las vueltas: el unico discriminante entre R2 y R5 es la igualdad byte a byte de los codigos.

## Criterios de exito

- R1: `build_fn` que incrementa un contador + `gate_fn` que incrementa otro, `max_fix = 3` -> en cada vuelta el contador de `build_fn` se incrementa antes que el de `gate_fn`.
- R2: `build_fn` que siempre devuelve `"def f():\n    return 1\n"` + `gate_fn = lambda u, c: (False, "rojo")` + `max_fix = 3` -> `status == "escalate"`.
- R3: mismo caso que R2 -> `"atasc" in detail.lower()` es verdadero.
- R4: mismo caso que R2 -> la cantidad de invocaciones de `build_fn` es exactamente `2`.
- R5: `build_fn` que devuelve `f"def f():\n    return {len(calls)}\n"` + `gate_fn = lambda u, c: (False, "rojo")` + `max_fix = 3` -> la cantidad de invocaciones de `build_fn` es exactamente `3` y `status == "escalate"`.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_mismo_codigo_dos_veces_escala_en_la_segunda |
| R2 | test_mismo_codigo_dos_veces_escala_en_la_segunda |
| R3 | test_mismo_codigo_dos_veces_escala_en_la_segunda |
| R4 | test_mismo_codigo_dos_veces_escala_en_la_segunda |
| R5 | test_codigos_distintos_no_escalan_antes_de_max_fix |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.