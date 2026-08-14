"""Proposals module: compose and pick proposal cards."""

from mmorch.iohelpers import load_json_tolerant, atomic_write_json
from mmorch.adjudicate import ADJUDICATIONS_PATH, STRONG_MATCH_THRESHOLD


def compose_cards():
    """Add 'card' to best pending strong match per project without card."""
    data = load_json_tolerant(ADJUDICATIONS_PATH)
    if data is None:
        return {"cards": 0}

    if "loop_paused" in data:
        return {"skipped": True}

    projects = data.get("projects", {})
    cards_added = 0

    for project in projects.values():
        # Skip if project already has a card
        if any("card" in match for match in project.get("matches", [])):
            continue

        # Find best pending strong match
        best_match = None
        best_score = -1.0
        for match in project.get("matches", []):
            if match.get("status") == "pending" and match.get("score", 0) >= STRONG_MATCH_THRESHOLD:
                if match.get("score", 0) > best_score:
                    best_score = match["score"]
                    best_match = match

        if best_match is not None:
            best_match["card"] = True
            cards_added += 1

    if cards_added > 0:
        atomic_write_json(ADJUDICATIONS_PATH, data)

    return {"cards": cards_added}


def pick_card(cwd):
    """Pick highest-score pending strong card with shown_count < 5."""
    try:
        data = load_json_tolerant(ADJUDICATIONS_PATH)
        if data is None:
            return None

        projects = data.get("projects", {})

        # Resolve project by longest cwd prefix
        best_project = None
        best_prefix_len = -1
        for _project_id, project in projects.items():
            project_cwd = project.get("cwd", "")
            if cwd.startswith(project_cwd) and len(project_cwd) > best_prefix_len:
                best_prefix_len = len(project_cwd)
                best_project = project

        if best_project is None:
            return None

        # Find best card candidate
        best_match = None
        best_score = -1.0
        for match in best_project.get("matches", []):
            if (match.get("status") == "pending"
                    and match.get("card") is True
                    and match.get("shown_count", 0) < 5
                    and match.get("score", 0) >= STRONG_MATCH_THRESHOLD):
                if match.get("score", 0) > best_score:
                    best_score = match["score"]
                    best_match = match

        if best_match is None:
            return None

        # Increment shown_count and persist
        best_match["shown_count"] = best_match.get("shown_count", 0) + 1
        atomic_write_json(ADJUDICATIONS_PATH, data)

        return best_match.get("text")
    except Exception:
        return None
