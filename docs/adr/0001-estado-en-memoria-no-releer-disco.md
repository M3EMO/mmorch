# ADR 0001 — el llamador que YA tiene el estado se lo pasa; el callee no relee disco

Status: accepted (2026-08-19)

## Contexto

`auto_repair.py`, `merge_train.py` y `project_repair.py` fueron escritos en
sesiones distintas. Los tres necesitaban leer `logs/nightly.jsonl` (el record
de la corrida nocturna) para decidir sobre qué actuar. Los tres, de forma
independiente, escribieron:

```python
lines = (logs / "nightly.jsonl").read_text(encoding="utf-8").splitlines()
rec = json.loads(lines[-1])
```

Nadie copió el código del otro — cada uno lo reinventó porque era la forma
obvia de "conseguir el record actual" sin pensar en quién más ya lo tenía en
memoria en ese mismo momento. `scripts/nightly.py` (el composition root)
YA construye ese `rec` paso a paso durante la corrida — pero como cada
módulo releía el archivo en vez de recibirlo, actuaban sobre la última línea
ESCRITA hasta ese momento, no sobre el estado actual de la corrida. Como
`_log(rec)` corría a mitad del script, eso significaba: actuar con un día
de retraso, silenciosamente, sin error.

Esto tiene nombre: violación del **Common Closure Principle** (Robert
Martin, *Agile Software Development*, cap. 20) — tres módulos que
deberían cambiar juntos (cualquier fix a "cómo se lee el estado nocturno")
no estaban conectados por nada que lo impusiera, así que cambiaron
(mal) por separado.

## Decisión

**Regla**: si el composition root (`scripts/nightly.py`) ya tiene un dato en
memoria durante la corrida, todo módulo invocado en esa misma corrida lo
RECIBE como parámetro. Nunca relee del disco lo que el llamador ya tiene.

Patrón concreto (el que ya se aplicó hoy en los tres módulos):

```python
def repair_projects(orch_root: str, *, rec: dict | None = None, ...) -> dict:
    """`rec`: record EN MEMORIA de esta misma corrida. Sin esto, releia
    nightly.jsonl y actuaba sobre la corrida ANTERIOR."""
    if rec is None:
        rec = json.loads((logs / "nightly.jsonl").read_text(...).splitlines()[-1])
    ...
```

`rec=None` como default preserva el modo standalone (tests, `scripts/minar.py`
a demanda, uso manual) — el parámetro es aditivo, no rompe nada existente.

## Consecuencias

- Todo modulo nuevo que necesite el record nocturno DEBE aceptar `rec: dict
  | None = None` con ese mismo patron — no re-inventar la lectura de disco.
- `mmorch/architecture.py` (`co_change_pairs`) detecta esta clase de bug
  RETROACTIVAMENTE: dos archivos que cambian juntos en git sin estar
  conectados por import son candidatos a la misma violacion. Corre los
  domingos, sin LLM, resultado en `logs/architecture.jsonl`.
- `mmorch/self_audit.py` tiene la categoria de finding "estructural"
  dedicada a esto — el juez que audita cada modulo por noche la busca
  explicitamente, citando este ADR como ejemplo.
- Quien vea `logs/architecture.jsonl` con `co_change_sin_import` no vacio:
  el primer chequeo es "¿alguno de estos dos deberia recibir el dato del
  otro en vez de leerlo por su cuenta?" — no siempre aplica (a veces el
  co-cambio es por otra razon), pero es la primera hipotesis a descartar.
