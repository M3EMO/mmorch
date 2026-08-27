"""Shim compat: ~/.claude.json apunta aca; el server real vive en mmorch/mcp_server.py."""
from mmorch.mcp_server import main

if __name__ == "__main__":
    main()
