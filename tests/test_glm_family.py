"""j76: GLM (Zhipu) como 3ra familia. Verifica que el registro la expone como familia
'zhipu' distinta de deepseek/google -> family_of() la habilita como cross-family valida
automaticamente, y rechaza zhipu<->zhipu. No corre API (inactivo sin key)."""
import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
cfg = importlib.import_module("mmorch.config")


def test_glm_registered_as_zhipu():
    assert "glm-4.5-air" in cfg.REGISTRY
    assert cfg.family_of("glm-4.5-air") == "zhipu"


def test_defaults_are_cross_family():
    # 2026-09-19: verifier default = glm (zhipu), generador = deepseek. El invariante que importa
    # es generador != verificador; GLM ya no es "tercera familia" respecto del verificador.
    assert cfg.family_of(cfg.DEFAULT_GENERATOR) != cfg.family_of(cfg.DEFAULT_VERIFIER)
    assert cfg.family_of("glm-4.5-air") != cfg.family_of(cfg.DEFAULT_GENERATOR)
    assert cfg.family_of(cfg.DEFAULT_VERIFIER) == "zhipu"


def test_glm_same_family_with_itself():
    # zhipu<->zhipu NO es cross-family (lo rechazaria adversarial_verify/ensemble)
    assert cfg.family_of("glm-4.5-air") == cfg.family_of("glm-4.5-air")


def test_three_distinct_active_families():
    fams = {cfg.family_of(k) for k in ("deepseek-chat", "gemini-3.1-flash-lite", "glm-4.5-air")}
    assert fams == {"deepseek", "google", "zhipu"}


def test_glm46_coder_registered_cross_family_vs_deepseek():
    # bof: GLM-4.6 coder, rival cross-family de deepseek-v4-pro
    assert cfg.family_of("glm-4.6") == "zhipu"
    assert cfg.family_of("glm-4.6") != cfg.family_of("deepseek-v4-pro")
