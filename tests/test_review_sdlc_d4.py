"""Review D4-r2: gate docstring-intacto sobre mmorch/synth_store.py."""
import mmorch.synth_store as s


def test_docstring_de_modulo_intacto():
    """La TAREA solo pide cambiar el import y STORE_PATH; el docstring de modulo
    no es parte del contrato y debe seguir terminando con salto de linea antes
    del cierre (tal como en la version previa al diff)."""
    assert s.__doc__.endswith("tipos.\n")
