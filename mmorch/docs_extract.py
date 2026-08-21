"""Extracción de texto de documentos (PDF hoy) — dos niveles, medidos en
vivo, no supuestos.

LIVIANO (extract_text, default, automatico): pypdfium2 (4MB, sin ML). Texto
plano, sin tablas ni reading-order. Es lo que usa repo_mining._collect_context
en el pipeline NOCTURNO desatendido.

RICO (extract_rich, manual/opt-in): docling completo (layout+tablas+OCR via
torch). Probado en vivo (2026-08-19, PDF real de 55 paginas): funciona, pero
327s (~5.4 min) y el pico de RAM libre en la maquina bajo a 297MB de 7.8GB
totales — corriendo SOLO, atendido. Metido en el loop nocturno junto con
evolve/autoresearch/self_audit/etc ese margen es real riesgo de OOM/swap.
Por eso NO esta wireado a repo_mining ni a nada automatico — es una funcion
que Mateo (o un script manual) llama a demanda, cuando quiere una conversion
rica de un documento puntual y puede esperar los minutos. Requiere
`pip install -e ".[docs-rico]"` (torch+torchvision+accelerate+docling-slim,
~1.1GB) — no se instala por default ni con el extra `docs` liviano.

Interfaz estable a propósito (inyeccion de deps, OCP-por-adicion): el
llamador no sabe ni le importa cual de las dos corre por dentro."""

from __future__ import annotations

from pathlib import Path

_MAX_PAGES = 30      # cap: un PDF de 300 paginas no debe inflar el contexto
_MAX_CHARS = 20000   # mismo orden que _collect_context de repo_mining


def extract_text(path: Path, *, max_pages: int = _MAX_PAGES,
                 max_chars: int = _MAX_CHARS) -> str:
    """Texto plano de un PDF via pypdfium2 (sin modelos, sin torch). Vacio
    si el archivo no es PDF valido o pypdfium2 no esta instalado — fail-soft,
    nunca rompe al llamador (docs faltantes = menos contexto, no un crash)."""
    try:
        import pypdfium2 as pdfium
    except ImportError:
        return ""
    try:
        pdf = pdfium.PdfDocument(str(path))
    except Exception:
        return ""
    partes = []
    total = 0
    try:
        for i in range(min(len(pdf), max_pages)):
            texto = pdf[i].get_textpage().get_text_range()
            partes.append(texto)
            total += len(texto)
            if total >= max_chars:
                break
    finally:
        pdf.close()
    return "\n".join(partes)[:max_chars]


def extract_rich(path: Path, *, converter_fn=None) -> str:
    """Markdown ESTRUCTURADO (headers/tablas/listas reales) via docling
    completo — MANUAL, nunca la llama el pipeline nocturno. Minutos de
    espera y GBs de RAM en el pico; uso a demanda, atendido.

    `converter_fn`: seam de test / inyeccion — por default construye un
    docling.document_converter.DocumentConverter real. RuntimeError con
    mensaje claro si torch/docling no estan instalados (no falla silencioso:
    a diferencia de extract_text, esta funcion la llama un humano esperando
    resultado, no un job desatendido — el silencio ahi confundiria mas de lo
    que ayuda)."""
    if converter_fn is None:
        try:
            from docling.document_converter import DocumentConverter
        except ImportError as e:
            raise RuntimeError(
                "extract_rich necesita docling completo (torch incluido) — "
                'instalar con: pip install -e ".[docs-rico]" '
                "(~1.1GB, minutos de conversion, pico de RAM alto — ver "
                "docs_extract.py y vault/research/docling-vs-pypdfium2-*.md)"
            ) from e
        converter_fn = DocumentConverter().convert
    res = converter_fn(str(path))
    return res.document.export_to_markdown()


def extract_rich_images(pdf: Path, out_md: Path, *, converter_fn=None) -> tuple[str, list[Path]]:
    """Como extract_rich, pero además extrae las FIGURAS/DIAGRAMAS reales del
    PDF como PNG (docling picture extraction, images_scale=2.0) en vez de
    dejarlas como placeholder `<!-- image -->` sin contenido. Mismo costo que
    extract_rich (docling ya corre layout detection igual; guardar el recorte
    no agrega minutos) — probado en vivo 2026-08-20, ~2 min para un capitulo
    de 12 paginas / 2 figuras, sin GPU. MANUAL/atendido igual que extract_rich.

    A diferencia de extract_rich (que devuelve el markdown como string),
    ESTA funcion escribe directo a `out_md` porque docling necesita saber la
    ruta de salida para nombrar la carpeta de artifacts (`<out_md.stem>_artifacts/`)
    donde caen los PNG — no hay forma limpia de devolver "markdown + imagenes"
    sin fijar esa ruta primero.

    Devuelve (markdown, lista de paths a los PNG extraidos) — las imagenes
    hay que leerlas (via el tool Read u otro lector visual) para escribir su
    descripcion; esta funcion solo las saca del PDF, no las interpreta.

    `converter_fn`: mismo seam de test/inyeccion que extract_rich — por
    default construye el DocumentConverter real con picture-extraction
    prendida; un test puede inyectar un doc fake con `.save_as_markdown`."""
    if converter_fn is None:
        try:
            from docling.document_converter import DocumentConverter, PdfFormatOption
            from docling.datamodel.pipeline_options import PdfPipelineOptions
            from docling.datamodel.base_models import InputFormat
        except ImportError as e:
            raise RuntimeError(
                "extract_rich_images necesita docling completo (torch incluido) — "
                'instalar con: pip install -e ".[docs-rico]"'
            ) from e
        opts = PdfPipelineOptions()
        opts.generate_picture_images = True
        opts.images_scale = 2.0
        conv = DocumentConverter(
            format_options={InputFormat.PDF: PdfFormatOption(pipeline_options=opts)})
        converter_fn = conv.convert

    from docling_core.types.doc import ImageRefMode
    doc = converter_fn(str(pdf)).document
    doc.save_as_markdown(out_md, image_mode=ImageRefMode.REFERENCED)

    artifacts_dir = out_md.parent / f"{out_md.stem}_artifacts"
    images = sorted(artifacts_dir.glob("*.png")) if artifacts_dir.exists() else []
    return out_md.read_text(encoding="utf-8"), images


def collect_pdfs(repo_dir: Path, *, max_files: int = 3,
                 max_chars_each: int = 4000) -> str:
    """PDFs sueltos de un repo clonado (whitepaper.pdf, docs/architecture.pdf,
    etc.) — hoy _collect_context() los ignora por completo. Ordena por tamaño
    ascendente (los chicos suelen ser mas señal por byte que un manual de
    500 paginas) y cap por archivo para no comerse todo el presupuesto de
    contexto con uno solo."""
    pdfs = sorted((f for f in repo_dir.rglob("*.pdf") if ".git" not in f.parts),
                  key=lambda f: f.stat().st_size)[:max_files]
    if not pdfs:
        return ""
    partes = []
    for f in pdfs:
        texto = extract_text(f, max_chars=max_chars_each)
        if texto.strip():
            partes.append(f"== {f.name} (PDF) ==\n{texto}")
    return "\n\n".join(partes)


def _demo() -> None:
    """Self-check: PDF inexistente no rompe; texto real trae contenido."""
    assert extract_text(Path("no-existe.pdf")) == ""
    print("docs_extract ok (self-check basico; probar con PDF real aparte)")


if __name__ == "__main__":
    _demo()
