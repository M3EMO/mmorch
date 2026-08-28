"""Tests de docs_extract (pypdfium2, sin torch — fail-soft si no esta instalado)."""

from pathlib import Path

from mmorch.docs_extract import collect_pdfs, extract_text

# PDF minimo valido armado a mano (sintaxis PDF-1.4 cruda) — evita sumar
# reportlab como dependencia solo para generar un PDF de 1 pagina en tests.
_PDF_TEMPLATE = """%PDF-1.4
1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj
2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj
3 0 obj<</Type/Page/Parent 2 0 R/Resources<</Font<</F1 4 0 R>>>>/MediaBox[0 0 300 150]/Contents 5 0 R>>endobj
4 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj
5 0 obj<</Length {length}>>stream
BT /F1 14 Tf 20 100 Td ({text}) Tj ET
endstream
endobj
xref
0 6
trailer<</Size 6/Root 1 0 R>>
startxref
0
%%EOF"""


def _make_pdf(path: Path, text: str) -> None:
    """PDF minimo real (1 pagina, texto Helvetica) — pypdfium2 lo lee sin
    problema aunque le falte la tabla xref completa (motor tolerante)."""
    stream = f"BT /F1 14 Tf 20 100 Td ({text}) Tj ET"
    body = _PDF_TEMPLATE.format(length=len(stream), text=text)
    path.write_bytes(body.encode("latin-1"))


def test_extract_text_pdf_inexistente_no_rompe(tmp_path):
    assert extract_text(tmp_path / "no-existe.pdf") == ""


def test_extract_text_archivo_no_es_pdf_no_rompe(tmp_path):
    basura = tmp_path / "no-es-pdf.pdf"
    basura.write_text("esto no es un pdf", encoding="utf-8")
    assert extract_text(basura) == ""


def test_extract_text_pdf_real(tmp_path):
    p = tmp_path / "doc.pdf"
    _make_pdf(p, "contenido de prueba mmorch")
    texto = extract_text(p)
    assert "contenido de prueba mmorch" in texto


def test_extract_text_respeta_cap_de_chars(tmp_path):
    p = tmp_path / "doc.pdf"
    _make_pdf(p, "x" * 50)
    texto = extract_text(p, max_chars=10)
    assert len(texto) <= 10


def test_collect_pdfs_junta_varios_y_cap_por_archivo(tmp_path):
    repo = tmp_path / "repo"
    repo.mkdir()
    _make_pdf(repo / "a.pdf", "contenido A")
    (repo / "docs").mkdir()
    _make_pdf(repo / "docs" / "b.pdf", "contenido B")
    out = collect_pdfs(repo)
    assert "a.pdf" in out and "contenido A" in out
    assert "b.pdf" in out and "contenido B" in out


def test_collect_pdfs_sin_pdfs_devuelve_vacio(tmp_path):
    repo = tmp_path / "repo"
    repo.mkdir()
    (repo / "README.md").write_text("hola", encoding="utf-8")
    assert collect_pdfs(repo) == ""


def test_collect_pdfs_respeta_max_files(tmp_path):
    repo = tmp_path / "repo"
    repo.mkdir()
    for i in range(5):
        _make_pdf(repo / f"doc{i}.pdf", f"contenido {i}")
    out = collect_pdfs(repo, max_files=2)
    assert out.count("== doc") == 2


def test_extract_rich_usa_converter_inyectado(tmp_path):
    """El seam converter_fn permite testear sin docling/torch instalados."""
    from mmorch.docs_extract import extract_rich

    class FakeDoc:
        def export_to_markdown(self):
            return "# Titulo\n\n- item real de docling"

    class FakeResult:
        document = FakeDoc()

    p = tmp_path / "algo.pdf"
    p.write_bytes(b"%PDF-1.4 fake")
    md = extract_rich(p, converter_fn=lambda path: FakeResult())
    assert "item real de docling" in md


def test_extract_rich_sin_docling_da_mensaje_claro(tmp_path, monkeypatch):
    """Sin converter_fn Y sin docling instalado: RuntimeError con el comando
    de instalacion, no un ImportError crudo (esto lo llama un humano)."""
    import builtins
    from mmorch.docs_extract import extract_rich

    real_import = builtins.__import__

    def fake_import(name, *a, **kw):
        if name == "docling.document_converter" or name.startswith("docling"):
            raise ImportError("no module named docling")
        return real_import(name, *a, **kw)

    monkeypatch.setattr(builtins, "__import__", fake_import)
    p = tmp_path / "algo.pdf"
    p.write_bytes(b"%PDF-1.4 fake")
    try:
        extract_rich(p)
        raise AssertionError("debia levantar RuntimeError")
    except RuntimeError as e:
        assert "docs-rico" in str(e)


def test_extract_rich_images_usa_converter_inyectado(tmp_path):
    """Camino feliz sin torch: converter_fn fake que escribe markdown+PNG
    como haria el DocumentConverter real con generate_picture_images=True."""
    from mmorch.docs_extract import extract_rich_images

    class FakeDoc:
        def save_as_markdown(self, out_md, *, image_mode=None):
            artifacts = out_md.parent / f"{out_md.stem}_artifacts"
            artifacts.mkdir()
            (artifacts / "img_1.png").write_bytes(b"\x89PNG fake")
            out_md.write_text("# Capitulo\n\n![figura](img_1.png)", encoding="utf-8")

    class FakeResult:
        document = FakeDoc()

    pdf = tmp_path / "cap.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    out_md = tmp_path / "cap_rico.md"
    md, images = extract_rich_images(pdf, out_md, converter_fn=lambda p: FakeResult())
    assert "Capitulo" in md
    assert len(images) == 1 and images[0].name == "img_1.png"


def test_extract_rich_images_sin_pngs_devuelve_lista_vacia(tmp_path):
    """Si docling no genero artifacts (PDF sin imagenes), no debe romper —
    lista vacia, no excepcion."""
    from mmorch.docs_extract import extract_rich_images

    class FakeDoc:
        def save_as_markdown(self, out_md, *, image_mode=None):
            out_md.write_text("# Solo texto, sin figuras", encoding="utf-8")

    class FakeResult:
        document = FakeDoc()

    pdf = tmp_path / "cap.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    out_md = tmp_path / "cap_rico.md"
    md, images = extract_rich_images(pdf, out_md, converter_fn=lambda p: FakeResult())
    assert "Solo texto" in md
    assert images == []


def test_extract_rich_images_sin_docling_da_mensaje_claro(tmp_path, monkeypatch):
    """Mismo contrato que extract_rich: sin docling instalado y sin
    converter_fn, RuntimeError con el comando de instalacion — no un
    ImportError crudo que confunda a quien lo esta corriendo a mano."""
    import builtins
    from mmorch.docs_extract import extract_rich_images

    real_import = builtins.__import__

    def fake_import(name, *a, **kw):
        if name.startswith("docling"):
            raise ImportError("no module named docling")
        return real_import(name, *a, **kw)

    monkeypatch.setattr(builtins, "__import__", fake_import)
    pdf = tmp_path / "cap.pdf"
    pdf.write_bytes(b"%PDF-1.4 fake")
    try:
        extract_rich_images(pdf, tmp_path / "out.md")
        raise AssertionError("debia levantar RuntimeError")
    except RuntimeError as e:
        assert "docs-rico" in str(e)
