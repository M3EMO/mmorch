"""Conversion RICA de un PDF a markdown (docling completo) — manual, a demanda.

Uso:
    python scripts/convertir_pdf.py "C:\\ruta\\al\\archivo.pdf"
    python scripts/convertir_pdf.py "C:\\ruta\\al\\archivo.pdf" salida.md
    python scripts/convertir_pdf.py "C:\\ruta\\al\\archivo.pdf" salida.md --imagenes

`--imagenes`: ademas extrae las figuras/diagramas reales del PDF como PNG
(en vez de dejarlas como placeholder `<!-- image -->` sin contenido), en
`<salida sin extension>_artifacts/`. Mismo tiempo que sin el flag (docling ya
corre layout detection igual). Las imagenes extraidas hay que LEERLAS aparte
(son recortes sin interpretar) para escribir su descripcion.

Minutos de espera, pico de RAM alto (medido: 297MB libres de 7.8GB en un PDF
de 55 paginas) — por eso NUNCA corre desatendido, solo por linea de comando.
Requiere: pip install -e ".[docs-rico]"
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))


def main() -> None:
    if len(sys.argv) < 2:
        print(__doc__)
        raise SystemExit(1)
    pdf = Path(sys.argv[1])
    if not pdf.exists():
        print(f"no existe: {pdf}")
        raise SystemExit(1)
    args = [a for a in sys.argv[2:] if not a.startswith("--")]
    con_imagenes = "--imagenes" in sys.argv[2:]
    salida = Path(args[0]) if args else pdf.with_suffix(".md")

    print(f"convirtiendo {pdf.name} (esto tarda minutos, es normal)...")
    try:
        if con_imagenes:
            from mmorch.docs_extract import extract_rich_images
            md, imagenes = extract_rich_images(pdf, salida)
            print(f"listo: {salida} ({len(md)} chars), {len(imagenes)} imagen(es) en {salida.stem}_artifacts/")
            for img in imagenes:
                print(f"  {img}")
        else:
            from mmorch.docs_extract import extract_rich
            md = extract_rich(pdf)
            salida.write_text(md, encoding="utf-8")
            print(f"listo: {salida} ({len(md)} chars)")
    except RuntimeError as e:
        print(str(e))
        raise SystemExit(1) from e


if __name__ == "__main__":
    main()
