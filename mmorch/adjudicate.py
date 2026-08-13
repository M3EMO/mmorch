"""Adjudication module for matching notes to projects."""

from pathlib import Path
import hashlib
import yaml


def adjudicate(note, project_name, project_path, generator, verifier, *, logs_dir='logs'):
    """Adjudicate a single note against a project."""
    logs_path = Path(logs_dir)
    if (logs_path / 'loop_paused').exists():
        return {'skipped': True}

    codegraph_dir = Path(project_path) / '.codegraph'
    codegraph = (
        [str(p) for p in sorted(codegraph_dir.rglob('*.py'))]
        if codegraph_dir.exists()
        else None
    )

    payload = {
        'note': note,
        'project': project_name,
        'project_path': project_path,
        'codegraph': codegraph,
    }

    proposal = generator.propose(payload)
    score = float(proposal['score'])
    justification = proposal['justification']
    cited_file = proposal.get('cited_file')

    refutation = verifier.refute(payload)
    refuted = refutation['refuted']

    strong = (score >= 0.7) and not refuted

    return {
        'note_path': note['path'],
        'project': project_name,
        'score': score,
        'justification': justification,
        'cited_file': cited_file,
        'strong': strong,
        'status': 'pendiente',
        'shown_count': 0,
        'id': f"{note['path']}|{project_name}",
    }


def run_incremental(notes_dir, projects, generator, verifier, *, logs_dir='logs'):
    """Run incremental adjudication over notes and projects."""
    from mmorch.iohelpers import load_json_tolerant, atomic_write_json

    logs_path = Path(logs_dir)
    logs_path.mkdir(parents=True, exist_ok=True)

    # Read notes from non-hidden subdirs
    notes = []
    notes_base = Path(notes_dir)
    for subdir in sorted(notes_base.iterdir()):
        if subdir.is_dir() and not subdir.name.startswith('.'):
            for md_file in sorted(subdir.glob('*.md')):
                content = md_file.read_text(encoding='utf-8')
                notes.append({
                    'path': str(md_file),
                    'content': content,
                    'hash': hashlib.sha256(content.encode('utf-8')).hexdigest(),
                })

    # Load previous state
    state_path = logs_path / 'adjudications.json'
    state = load_json_tolerant(state_path, {})
    if 'pairs' not in state:
        state['pairs'] = {}
    if 'by_project' not in state:
        state['by_project'] = {}

    judged = 0
    skipped_pairs = 0
    strong_results = []

    # Process each note-project pair
    for note in notes:
        note_path = note['path']
        for project_name, project_path in projects.items():
            key = f'{note_path}|{project_name}'
            current_hash = note['hash']

            if key in state['pairs'] and state['pairs'][key]['hash'] == current_hash:
                skipped_pairs += 1
                result = state['pairs'][key]['result']
            else:
                result = adjudicate(
                    note, project_name, project_path, generator, verifier,
                    logs_dir=logs_dir,
                )
                if result.get('skipped'):
                    skipped_pairs += 1
                    continue
                judged += 1
                state['pairs'][key] = {'hash': current_hash, 'result': result}

            if result.get('strong'):
                strong_results.append(result)
                state['by_project'].setdefault(project_name, []).append(result)

    # Update frontmatter for notes with strong matches
    for note in notes:
        note_path = note['path']
        strong_projects = [
            project_name
            for project_name in projects
            if any(
                r['note_path'] == note_path and r['project'] == project_name
                for r in strong_results
            )
        ]
        if strong_projects:
            _update_frontmatter(note_path, strong_projects)

    # Write state back
    atomic_write_json(state_path, state)

    return {
        'judged': judged,
        'skipped_pairs': skipped_pairs,
        'strong': len(strong_results),
    }


def _update_frontmatter(note_path, projects):
    """Update the applies_to key in frontmatter, preserving other content."""
    path = Path(note_path)
    text = path.read_text(encoding='utf-8')

    if text.startswith('---'):
        parts = text.split('---', 2)
        if len(parts) >= 3:
            frontmatter_text = parts[1]
            body = parts[2]
            try:
                frontmatter = yaml.safe_load(frontmatter_text) or {}
            except yaml.YAMLError:
                frontmatter = {}
            frontmatter['applies_to'] = projects
            new_frontmatter = yaml.dump(frontmatter, default_flow_style=False, allow_unicode=True)
            path.write_text(f'---\n{new_frontmatter}---{body}', encoding='utf-8')
    else:
        # No frontmatter, create one
        frontmatter = {'applies_to': projects}
        new_frontmatter = yaml.dump(frontmatter, default_flow_style=False, allow_unicode=True)
        path.write_text(f'---\n{new_frontmatter}---\n{text}', encoding='utf-8')
