"""CLI minimo instalable (`mmorch`): status, health, canary y refutacion desde la terminal.

Reusa lo que ya existe — metrics.summary() y health.report() — sin logica
propia: el CLI es una vista, la semantica vive en la libreria (contrato W5:
una sola semantica por operacion). Salida JSON para que sea parseable por
scripts igual que por humanos.
"""
from __future__ import annotations

import argparse
import json
import sys


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="mmorch", description="mmorch — orquestacion multi-modelo (status/health)")
    sub = parser.add_subparsers(dest="cmd", required=True)
    sub.add_parser("status", help="resumen de metrics (costos, llamadas, cache)")
    sub.add_parser("health", help="latidos + errores recientes; exit 1 si unhealthy")
    pc = sub.add_parser("canary", help="corre el canary set contra los modelos activos "
                        "y compara pass-rate vs baseline (detecta drift de provider)")
    pc.add_argument("--models", nargs="*", default=None,
                    help="model keys a testear (default: gen/verifier/router activos)")
    pc.add_argument("--update-baseline", action="store_true",
                    help="persiste los pass-rates medidos como nuevo baseline")
    pr = sub.add_parser("refutacion", help="clasifica un test propuesto contra un caso del banco de refutacion "
                        "(acierto / falsa_alarma / neutro / invalido); exit 0 solo si acierta")
    pr.add_argument("caso", help="id del caso en logs/refutacion/banco.json")
    pr.add_argument("test", help="archivo con el test propuesto, en el formato del caso")
    args = parser.parse_args(argv)

    if args.cmd == "refutacion":  # sin watchdog: es una medicion puntual, no una vista del estado
        from pathlib import Path

        from mmorch.refutacion import Banco, cargar_banco
        with Banco(cargar_banco()) as banco:
            r = banco.evaluar(args.caso, Path(args.test).read_text(encoding="utf-8"))
        print(json.dumps(r, ensure_ascii=False, indent=2))
        return 0 if r["clase"] == "acierto" else 1

    # dead-man visible (W4.4): nightly vencido grita en stderr aca mismo,
    # ademas del JSON de `health` — status solo muestra metrics y sin esto
    # un nightly muerto pasaba inadvertido en el comando mas usado
    from mmorch.health import nightly_watchdog
    nightly_watchdog()

    if args.cmd == "status":
        from mmorch.metrics import summary
        print(json.dumps(summary(), ensure_ascii=False, indent=2, default=str))
        return 0

    if args.cmd == "canary":
        # exit code = señal: 0 sano, 1 drift o sin modelos medibles (cron/CI-friendly)
        from mmorch.canary import compare_baseline, run_canary
        rep = compare_baseline(run_canary(models=args.models or None),
                               update=args.update_baseline)
        print(json.dumps(rep, ensure_ascii=False, indent=2))
        medidos = [m for m, r in rep["models"].items() if "pass_rate" in r]
        if not medidos:
            print("canary: ningun modelo medible — faltan API keys; setealas en "
                  "el .env del home de mmorch para correr el canario", file=sys.stderr)
            return 1
        if rep["drift"]:
            print(f"canary: DRIFT detectado en {', '.join(rep['drift'])} "
                  "(pass-rate cayo vs baseline)", file=sys.stderr)
            return 1
        return 0

    # health: exit code = señal (0 sano, 1 no) para poder usarlo en cron/CI
    from mmorch.health import report
    from mmorch.paths import logs_dir
    rep = report(logs_dir=str(logs_dir()))
    print(json.dumps(rep, ensure_ascii=False, indent=2, default=str))
    return 0 if rep.get("healthy") else 1


if __name__ == "__main__":
    sys.exit(main())
