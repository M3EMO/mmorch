"""preparar_mapa — CLI del wayfinder-prep: investigación autónoma, decisión tuya.

Uso:
  python scripts/preparar_mapa.py <nombre> "pregunta 1" "pregunta 2" ...
  python scripts/preparar_mapa.py <nombre> --file tickets.txt   (una pregunta por línea, # comenta)

Escribe .scratch/<nombre>/prep.md con evidencia + opciones + recomendación por
ticket — SIN responder ninguno (HITL: el grilling se contesta con Mateo).
"""
import io
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))


def main() -> None:
    if sys.stdout.encoding and sys.stdout.encoding.lower() != "utf-8":
        sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8",
                                      errors="replace")
    args = sys.argv[1:]
    if len(args) < 2:
        print(__doc__)
        sys.exit(1)
    nombre = args[0]
    if args[1] == "--file":
        lines = pathlib.Path(args[2]).read_text(encoding="utf-8").splitlines()
        preguntas = [ln.strip() for ln in lines
                     if ln.strip() and not ln.strip().startswith("#")]
    else:
        preguntas = args[1:]

    from dotenv import load_dotenv
    load_dotenv(ROOT / ".env")
    from mmorch.wayfinder_prep import prep_map
    print(f"Investigando {len(preguntas)} tickets del mapa '{nombre}'...")
    path = prep_map(nombre, preguntas, orch_root=str(ROOT))
    print(f"Dossier listo: {path}")
    print("Ningún ticket respondido — el grilling es tuyo.")


if __name__ == "__main__":
    main()
