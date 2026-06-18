import sys, pathlib, importlib
sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))
S = importlib.import_module("mmorch.sessions")


def test_redacts_api_key():
    red, _ = S.redact("my key is sk-ABCDEF0123456789abcdef0123 ok")
    assert "sk-ABCDEF0123456789abcdef0123" not in red and "[REDACTED]" in red


def test_redacts_key_value_secret():
    red, _ = S.redact("API_KEY=supersecretvalue123")
    assert "supersecretvalue123" not in red


def test_redacts_email_and_home_path():
    red, _ = S.redact(r"mail map12@gmail.com path C:\Users\map12\proj")
    assert "map12@gmail.com" not in red and r"C:\Users\map12" not in red


def test_residual_entropy_lowers_confidence():
    _, conf = S.redact("token " + "a1B2" * 10)   # 40 chars base64-ish
    assert conf == 0.0


def test_clean_text_high_confidence():
    red, conf = S.redact("arregla el bug en el parser")
    assert conf == 1.0 and red == "arregla el bug en el parser"


def test_redacts_jwt():
    # mmorch verify T3: JWT eyJ... es un secret de alto valor.
    jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N"
    red, _ = S.redact("token " + jwt)
    assert jwt not in red and "[REDACTED]" in red


def test_redacts_pem_private_key():
    pem = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEA\n-----END RSA PRIVATE KEY-----"
    red, _ = S.redact("clave: " + pem)
    assert "MIIEowIBAAKCAQEA" not in red


def test_hyphenated_high_entropy_lowers_confidence():
    # mmorch verify T3: token con guiones (UUID-ish) tambien debe bajar confidence.
    _, conf = S.redact("ref abc12-def34-ghi56-jkl78-mno90-pqr12-stu34")
    assert conf == 0.0
