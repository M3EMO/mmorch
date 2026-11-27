"""A4: archivo manual de seeds que nadie leyo en 60 dias de transcripts (JSONL sinteticos)."""
from __future__ import annotations

import json
from datetime import date

import pytest

import mmorch.vault as vault_mod
from mmorch import vault_seeds as S

HOY = date(2026, 12, 20)  # corte = 2026-10-21


def _tu(ts, name, **inp):
    return {"timestamp": ts, "message": {"content": [
        {"type": "tool_use", "name": name, "input": inp}]}}


def _transcript(d, eventos, name="s.jsonl"):
    d.mkdir(parents=True, exist_ok=True)
    (d / name).write_text("\n".join(json.dumps(e) for e in eventos) + "\n", encoding="utf-8")


def _seed(vault, stem, created="2026-08-01", status="seed", tags="[p]"):
    p = vault / "research" / f"{stem}.md"
    p.parent.mkdir(parents=True, exist_ok=True)
    c = f"created: {created}\n" if created is not None else ""
    p.write_text(f"---\ntitle: {stem}\nstatus: {status}\ntags: {tags}\n{c}---\nx\n",
                 encoding="utf-8")
    return p


@pytest.fixture()
def vault(tmp_path, monkeypatch):
    v = tmp_path / "vault"
    v.mkdir()
    monkeypatch.setattr(vault_mod, "VAULT", v)
    return v


def _snap(root):
    return {str(p): p.read_bytes() for p in root.rglob("*") if p.is_file()}


def test_lecturas_cuenta_solo_nombre_exacto(tmp_path):
    t = tmp_path / "t"
    ok = "2026-11-01T10:00:00.000Z"
    _transcript(t, [
        _tu(ok, "Read", file_path="/v/research/leida-read.md"),
        _tu(ok, "Grep", pattern="x", path="/v/research/leida-grep.md"),
        _tu(ok, "Bash", command="cd /v && cat research/leida-cat.md | head"),
        _tu(ok, "Bash", command="sed -n 1,5p /v/research/leida-sed.md"),
        _tu(ok, "Bash", command="ls && grep foo research/leida-bash-grep.md"),
        _tu(ok, "Bash", command="cat 'research/con espacio.md'"),
        _tu(ok, "Bash", command="cat research/*.md"),
        _tu(ok, "Bash", command="cat research/glob-?.md"),
        _tu(ok, "Bash", command="grep -r x /v/research/"),
        _tu(ok, "Bash", command="echo leida-echo.md"),
        _tu(ok, "Bash", command="cat 'sin cerrar.md"),
        _tu(ok, "Grep", pattern="x", glob="*.md"),
        _tu("2026-09-01T10:00:00Z", "Read", file_path="/v/research/vieja-lectura.md"),
    ])
    assert S.lecturas(t, date(2026, 10, 21)) == {
        "leida-read.md", "leida-grep.md", "leida-cat.md", "leida-sed.md",
        "leida-bash-grep.md", "con espacio.md",
        "cerrar.md"}  # comillas rotas: cae a split() y no rompe (cuenta de mas, no de menos)


def test_freno_cobertura_menor_a_60_dias(vault, tmp_path):
    t = tmp_path / "t"
    _transcript(t, [_tu("2026-11-01T10:00:00Z", "Read", file_path="/x.md")])
    _seed(vault, "a")
    with pytest.raises(S.CoberturaInsuficiente, match="2026-12-31"):
        S.candidatas(vault, t, HOY)
    with pytest.raises(S.CoberturaInsuficiente):
        S.candidatas(vault, tmp_path / "vacio", HOY)


def test_candidatas_filtra_created_y_lecturas(vault, tmp_path):
    t = tmp_path / "t"
    _transcript(t, [_tu("2026-09-01T00:00:00Z", "Bash", command="ls"),
                    _tu("2026-11-05T00:00:00Z", "Read", file_path="/v/research/leida.md")])
    _seed(vault, "candidata")
    _seed(vault, "leida")
    _seed(vault, "reciente", created="2026-11-30")
    _seed(vault, "borde", created="2026-10-21")
    _seed(vault, "sin-created", created=None)
    _seed(vault, "created-roto", created="ayer")
    _seed(vault, "applied", status="applied")
    (vault / "archive").mkdir()
    (vault / "archive" / "ya.md").write_text(
        "---\nstatus: seed\ncreated: 2026-01-01\n---\n", encoding="utf-8")
    assert [p.stem for p in S.candidatas(vault, t, HOY)] == ["candidata"]


def test_sin_aplicar_no_mueve(vault, tmp_path, capsys):
    t = tmp_path / "t"
    _transcript(t, [_tu("2026-09-01T00:00:00Z", "Bash", command="ls")])
    _seed(vault, "candidata")
    antes = _snap(vault)
    assert S.main(["--transcripts", str(t)], hoy=HOY) == 0
    assert _snap(vault) == antes
    assert "candidata" in capsys.readouterr().out


def test_aplicar_archiva_y_moc_las_lista_en_archivadas(vault, tmp_path):
    t = tmp_path / "t"
    _transcript(t, [_tu("2026-09-01T00:00:00Z", "Bash", command="ls")])
    _seed(vault, "candidata")
    _seed(vault, "viva", created="2026-12-01")
    vault_mod.regenerate_moc("p")
    assert S.main(["--aplicar", "--transcripts", str(t)], hoy=HOY) == 0
    assert (vault / "archive" / "candidata.md").exists()
    assert not (vault / "research" / "candidata.md").exists()
    moc = (vault / "moc" / "p.md").read_text(encoding="utf-8")
    assert moc.split("## Archivadas")[1].strip() == "- [[candidata]]"
    assert "[[viva]]" in moc.split("## Archivadas")[0]
    assert "archive | candidata" in (vault / "log.md").read_text(encoding="utf-8")


def test_freno_sale_con_codigo_distinto_de_cero(vault, tmp_path, capsys):
    t = tmp_path / "t"
    _transcript(t, [_tu("2026-12-01T00:00:00Z", "Bash", command="ls")])
    _seed(vault, "candidata")
    assert S.main(["--aplicar", "--transcripts", str(t)], hoy=HOY) != 0
    assert (vault / "research" / "candidata.md").exists()
    assert "2027-01-30" in capsys.readouterr().err


def test_aplicar_con_choque_en_archive_no_mueve_nada(vault, tmp_path, capsys):
    t = tmp_path / "t"
    _transcript(t, [_tu("2026-09-01T00:00:00Z", "Bash", command="ls")])
    _seed(vault, "a-primera")
    _seed(vault, "b-choca")
    (vault / "archive").mkdir()
    (vault / "archive" / "b-choca.md").write_text("previo", encoding="utf-8")
    antes = _snap(vault)
    assert S.main(["--aplicar", "--transcripts", str(t)], hoy=HOY) != 0
    assert _snap(vault) == antes
    assert "b-choca" in capsys.readouterr().err


def test_candidatas_excluye_symlinks(vault, tmp_path):
    t = tmp_path / "t"
    _transcript(t, [_tu("2026-09-01T00:00:00Z", "Bash", command="ls")])
    real = _seed(vault, "real")
    fuera = tmp_path / "fuera.md"
    fuera.write_text(real.read_text(encoding="utf-8"), encoding="utf-8")
    (vault / "research" / "enlace.md").symlink_to(fuera)
    assert [p.stem for p in S.candidatas(vault, t, HOY)] == ["real"]
