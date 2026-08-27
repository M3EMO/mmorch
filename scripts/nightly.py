"""Shim compat: el Task Scheduler apunta aca; el driver real vive en mmorch/nightly.py."""
from mmorch.nightly import main

if __name__ == "__main__":
    main()
