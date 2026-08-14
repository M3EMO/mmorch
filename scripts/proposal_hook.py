import json
import sys
from pathlib import Path

from card_picker import pick_card


def main() -> None:
    # Read JSON from stdin
    try:
        data = json.load(sys.stdin)
    except json.JSONDecodeError:
        data = {}

    # Load projects.json from repo root
    projects_path = Path(__file__).resolve().parents[1] / 'projects.json'
    try:
        with open(projects_path, 'r') as f:
            projects = json.load(f)
    except (FileNotFoundError, json.JSONDecodeError):
        projects = {}

    # Call pick_card with cwd from data (or current directory)
    cwd = data.get('cwd', str(Path.cwd()))
    result = pick_card(cwd, projects)

    # Print result if not None
    if result is not None:
        print(result)

    # Always exit 0
    sys.exit(0)


if __name__ == '__main__':
    main()
