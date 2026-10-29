# Hooks propios de la rama sistema-nuevo.
# Los gates del sistema se agregan aca; los de mmorch (ruff, bd) no aplican.
# En una sesión sin podman (nube): `export ROOT_SIN_PODMAN=1` y `git config core.hooksPath .githooks`.
# Con ROOT_SIN_PODMAN=1, los tests que necesitan podman se saltean y lo avisan en stderr.
# Sin la variable, todos corren: en la laptop nunca hay un salteo silencioso.
