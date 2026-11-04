"""Plugin de pytest de Root para la suite oculta (H1).

Root lo carga con `-p root_oculto` y con `PYTHONPATH=/salida/.root`.
Después de la colección borra el código de la copia de la suite. Los datos quedan.
"""

import os


def pytest_collection_finish(session):
    """Borra los `.py` y `.pyc` de `ROOT_OCULTO_DIR`. Los tests ya están cargados en memoria."""
    carpeta = os.environ.get("ROOT_OCULTO_DIR")
    if not carpeta:
        return
    for raiz, _carpetas, archivos in os.walk(carpeta):
        for nombre in archivos:
            if nombre.endswith((".py", ".pyc")):
                os.remove(os.path.join(raiz, nombre))
    # Después de borrar el código, las `__pycache__` vacías sobran.
    for raiz, _carpetas, _archivos in os.walk(carpeta, topdown=False):
        if os.path.basename(raiz) == "__pycache__" and not os.listdir(raiz):
            os.rmdir(raiz)
