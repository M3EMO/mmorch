"""Ticket 07 de builder-e2e: la etapa 4 separa una dependencia que el ENTORNO no trae de un error del codigo. Cero API."""
import json

import sdlc.pipeline as S

CASO_REAL = ("tests/parametros.contract.test.ts(1,22): error TS2307: Cannot find module 'node:test' or its corresponding "
             "type declarations.\n")   # logs/sdlc/diag-cases de la corrida del 2026-10-15: 3 vueltas y 2 escaladas en vano


def _repo(tmp_path, monkeypatch, **manifiestos):
    for nombre, texto in manifiestos.items():
        (tmp_path / nombre.replace("_", ".", 1)).write_text(texto, encoding="utf-8")
    monkeypatch.setattr(S, "WT", tmp_path)


def test_builtin_de_node_es_del_entorno(tmp_path, monkeypatch):
    _repo(tmp_path, monkeypatch)
    assert S._toolchain_missing(CASO_REAL)
    assert S._toolchain_missing("error TS2688: Cannot find type definition file for 'node'.")


def test_import_relativo_roto_es_del_codigo(tmp_path, monkeypatch):
    _repo(tmp_path, monkeypatch)
    assert not S._toolchain_missing("src/app.ts(1,1): error TS2307: Cannot find module './calculo.ts' or its types.")


def test_paquete_inventado_es_del_codigo_y_declarado_es_del_entorno(tmp_path, monkeypatch):
    log = "src/a.ts(1,1): error TS2307: Cannot find module 'lodash/fp' or its corresponding type declarations."
    _repo(tmp_path, monkeypatch, package_json=json.dumps({"dependencies": {"zod": "^3"}}))
    assert not S._toolchain_missing(log)   # nadie declaro lodash: el coder lo invento
    _repo(tmp_path, monkeypatch, package_json=json.dumps({"dependencies": {"lodash": "^4"}}))
    assert S._toolchain_missing(log)       # declarado pero no instalado: falta en la imagen
    _repo(tmp_path, monkeypatch, package_json=json.dumps({"devDependencies": {"@types/node": "^22"}}))
    assert S._toolchain_missing("error TS2307: Cannot find module '@types/node/test'.")


def test_python_paquete_declarado_contra_modulo_propio(tmp_path, monkeypatch):
    _repo(tmp_path, monkeypatch, pyproject_toml='[project]\ndependencies = ["requests>=2"]\n')
    assert S._toolchain_missing("ModuleNotFoundError: No module named 'requests'")
    assert not S._toolchain_missing("ModuleNotFoundError: No module named 'mipaquete.utils'")


def test_maven_y_cargo_sin_red_son_del_entorno(tmp_path, monkeypatch):
    _repo(tmp_path, monkeypatch)
    assert S._toolchain_missing("[ERROR] Failed to execute goal on project app: Could not resolve dependencies for project com.x:app")
    assert S._toolchain_missing("error: no matching package named `serde` found")


def test_errores_de_codigo_siguen_yendo_al_coder(tmp_path, monkeypatch):
    _repo(tmp_path, monkeypatch)
    assert not S._toolchain_missing("src/a.ts(3,1): error TS2322: Type 'string' is not assignable to type 'number'.")
    assert not S._toolchain_missing("src/a.ts(1,10): error TS2305: Module './tipos.ts' has no exported member 'X'.")
