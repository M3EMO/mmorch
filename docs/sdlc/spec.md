# Spec: Cableo de `megasource` al nightly (price check mensual)

Fuente de la estructura: spec-kit `templates/spec-template.md` (MIT), recortada a lo que un gate puede verificar.

## Contrato

Único archivo modificado: `mmorch/nightly.py`. No se tocan tests ni otros archivos. Se conserva el docstring del módulo. Python 3.12, sin dependencias (solo stdlib: `json`, `pathlib`, `time`, `typing`).

Constants en `mmorch/nightly.py`:

```python
PRICE_CHECK_INTERVAL_S: int = 30 * 86400
PRICE_CHECK_STATE_FILENAME: str = "nightly_prices.json"
```

Firma exacta:

```python
def run_price_check(
    now: float | None = None,
    propose: Callable[[], dict] | None = None,
    state_path: Path | None = None,
) -> dict:
```

- `now`: default `time.time()`.
- `state_path`: default `mmorch.paths.home() / PRICE_CHECK_STATE_FILENAME`.
- `propose`: default import local `from mmorch.megasource import propose_price_update`.
- Estado JSON: `{"last_check_ts": <float>}`.
- Orden de evaluación: resolver `now` y `state_path`; leer estado si existe; comparar antigüedad; cortocircuito `reciente` o llamar `propose`; en éxito escribir estado; retornar.
- `run_price_check` nunca escribe `prices.json` ni otro archivo fuera de `state_path`.

`main()` existente conserva su firma `def main() -> None`. Después del bloque del digest, invoca `run_price_check()` dentro de su propio `try/except`:

```python
try:
    rec = {"step": "price_check", **run_price_check()}
except Exception as e:
    rec = {"step": "price_check", "error": f"{type(e).__name__}: {str(e)[:200]}"}
_log(rec)
```

`_log(rec: dict) -> None` no cambia de firma.

## Requisitos

- R1: si `state_path` existe y `now - last_check_ts < PRICE_CHECK_INTERVAL_S`, retorna `{"ran": False, "reason": "reciente"}` sin llamar a `propose`. Orden: leer estado -> validar antigüedad -> cortocircuito antes de `propose`.
- R2: si `state_path` no existe o `now - last_check_ts >= PRICE_CHECK_INTERVAL_S`, llama `propose()`, escribe `state_path` con `{"last_check_ts": now}` y retorna `{"ran": True, "n_changed": r.get("n_changed", 0), "diff": r.get("diff", {})}`. Orden: `propose` -> escritura de estado -> retorno.
- R3: si `state_path` no existe, se trata como vencido: corre `propose`, crea `state_path` y retorna `ran=True`. Orden: ausencia de archivo -> rama vencida de R2.
- R4: si `propose()` levanta, captura la excepción, retorna `{"ran": True, "error": f"{type(e).__name__}: {str(e)[:200]}"}` y no crea, escribe ni modifica `state_path`. Orden: `try propose` -> `except` -> retorno sin escritura.

## Casos borde

- R1: `now - last_check_ts` fraccionario y menor a `30 * 86400` -> `reciente`. `last_check_ts > now` (tiempo negativo) -> diferencia negativa, `reciente`. `last_check_ts` ausente o no numérico -> tratar como sin estado previo (R3).
- R2: límite exacto `now - last_check_ts == 30 * 86400` -> corre. `propose()` retorna dict vacío o sin claves `n_changed`/`diff` -> defaults `0` y `{}`. `diff` con clave nueva `"deepseek-chat"` se propaga sin filtrar.
- R3: `state_path` inexistente en `home()` -> corre y crea el archivo con `last_check_ts == now`.
- R4: `str(e)` de más de 200 caracteres -> truncado a 200. `str(e)` vacío -> `error == "RuntimeError: "`. `state_path` preexistente vencido -> queda intacto tras la excepción.

## Criterios de éxito

- R1: `state_path` con `last_check_ts = now - 5 * 86400`, `propose` espía -> retorna `{"ran": False, "reason": "reciente"}` y el espía no registra llamadas.
- R2: `state_path` con `last_check_ts = now - 40 * 86400`, `propose` devuelve `{"n_changed": 2, "diff": {"deepseek-chat": {"price_in": [1, 2]}}}` -> retorna `ran=True`, `n_changed=2`, `diff` contiene `"deepseek-chat"`; el JSON de estado tiene `last_check_ts == now` con tolerancia menor a 1 segundo.
- R3: sin `state_path`, `propose` devuelve `{"n_changed": 0, "diff": {}}` -> retorna `ran=True` y `state_path` existe.
- R4: `propose` lanza `RuntimeError("sin red")` -> retorna `{"ran": True, "error": "RuntimeError: sin red"}` y `state_path` no existe.

## Trazabilidad

| ID | tests |
|---|---|
| R1 | test_R1_reciente_no_corre |
| R2 | test_R2_vencido_propone_y_guarda_estado |
| R3 | test_R3_sin_estado_previo_corre |
| R4 | test_R4_propose_que_levanta_no_rompe_ni_avanza_estado |

## Clarificaciones

- sin preguntas: la spec cubre el barrido.