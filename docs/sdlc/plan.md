## Archivos

- `mmorch/nightly.py` [R1, R2, R3, R4] [P]

  Detalle de implementación (único archivo tocado; no se tocan tests ni otros archivos; se conserva el docstring del módulo):

  1. **Constants a nivel de módulo** (junto a las existentes, sin romper imports):
     - `PRICE_CHECK_INTERVAL_S: int = 30 * 86400`
     - `PRICE_CHECK_STATE_FILENAME: str = "nightly_prices.json"`
     - Imports stdlib requeridos: `json`, `pathlib.Path`, `time`, `typing.Callable`. Reusar `mmorch.paths.home()` (ya importado en el módulo) para el default de `state_path`.

  2. **`run_price_check`** [R1, R2, R3, R4] con firma exacta:
     ```python
     def run_price_check(
         now: float | None = None,
         propose: Callable[[], dict] | None = None,
         state_path: Path | None = None,
     ) -> dict:
     ```
     Orden de evaluación interno (contrato):
     - Resolver `now` (`time.time()` si es `None`) y `state_path` (`mmorch.paths.home() / PRICE_CHECK_STATE_FILENAME` si es `None`).
     - Resolver `propose` con import local (`from mmorch.megasource import propose_price_update`) si es `None`.
     - **Leer estado**: si `state_path` existe, parsear `{"last_check_ts": <float>}`.
       - **R1 / R3 (validación de `last_check_ts`)**: si el JSON no tiene `last_check_ts` o no es numérico, tratar como sin estado previo (rama R3).
     - **Cortocircuito R1**: si `last_check_ts` es válido y `now - last_check_ts < PRICE_CHECK_INTERVAL_S` (incluye diferencias negativas por `last_check_ts > now` y diferencias fraccionarias), retornar `{"ran": False, "reason": "reciente"}` **sin** llamar a `propose` y **sin** escribir `state_path`.
     - **Rama vencida R2 / R3**: si `state_path` no existe, o `now - last_check_ts >= PRICE_CHECK_INTERVAL_S` (límite exacto incluido), ejecutar:
       - **R2 / R4 (try alrededor de `propose`)**: `try: r = propose()`.
         - En éxito: escribir `state_path` con `{"last_check_ts": now}` (modo texto JSON, `Path.write_text`), y retornar `{"ran": True, "n_changed": r.get("n_changed", 0), "diff": r.get("diff", {})}`. `diff` se propaga sin filtrar claves (p.ej. `"deepseek-chat"`). `n_changed`/`diff` ausentes o `r` vacío -> defaults `0` y `{}`.
         - **R4 (except)**: capturar `Exception`, retornar `{"ran": True, "error": f"{type(e).__name__}: {str(e)[:200]}"}` (truncado a 200 chars; `str(e)` vacío -> `"RuntimeError: "`) y **no** crear/escribir/modificar `state_path` (si el archivo preexistía vencido, queda intacto).
     - `run_price_check` **nunca** escribe `prices.json` ni ningún path fuera de `state_path`.

  3. **`main()`** [R2] conserva firma `def main() -> None`. Después del bloque del digest, agregar dentro de su propio `try/except`:
     ```python
     try:
         rec = {"step": "price_check", **run_price_check()}
     except Exception as e:
         rec = {"step": "price_check", "error": f"{type(e).__name__}: {str(e)[:200]}"}
     _log(rec)
     ```
     `_log(rec: dict) -> None` no cambia de firma.

  Notas de paralelización: es el único archivo del plan, por lo que no importa a otros y va marcado `[P]`.

## Prueba

`python -m pytest tests/test_sdlc_d12_megasource_nightly.py -q`