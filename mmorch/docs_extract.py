"""Extracción de texto de documentos (PDF hoy) para enriquecer el contexto
que ve el juez — candidata "Docling" resuelta a lo que el hardware permite.

Docling completo (layout/tablas/reading-order via modelos ML) NECESITA torch
en tiempo de ejecución (el layout model hace `import torch` al inicializarse,
no hay pipeline liviano para PDF — probado en vivo: import ok sin torch,
conversión real revienta con ModuleNotFoundError). torch solo pesa ~527MB en
disco + baja pesos de modelos aparte — no corresponde en esta maquina (RAM
limitada, ver memoria de hardware). Via pypdfium2 (4MB, sin ML) se consigue
texto plano — sin estructura de tablas ni reading-order multi-columna, pero
suficiente para que el juez vea CONTENIDO que hoy no ve (ningun PDF entra a
_collect_context).

Interfaz estable a propósito (inyeccion de deps, OCP-por-adicion): quien
llame a `extract_text` no sabe ni le importa si por dentro es pypdfium2 o
docling completo. Cuando el hardware lo permita (ExpertBook 64GB, memoria
'hardware-plan'), el upgrade es cambiar EL CUERPO de esta funcion, no los
call-sites."""

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
