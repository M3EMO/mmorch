# Lexicon babel v1

Clave de decodificación para archivos `*.babel.md` del vault. Los `.babel.md`
son derivados comprimidos model-native (paper 2606.19857) del archivo original
homónimo — el original es SIEMPRE la fuente de verdad; el babel es para que los
modelos lean con menos tokens. Un modelo que lee un babel debe recibir este
lexicon como decoder key.

## Símbolos

| Símbolo | Significado |
|---------|-------------|
| `→` | causa / lleva a / produce |
| `←` | derivado de / viene de |
| `∴` | por lo tanto |
| `¬` | no / negación |
| `∀` | todos / siempre |
| `∃` | existe / hay al menos uno |
| `Δ` | cambio / diferencia |
| `≈` | aproximadamente |
| `⊕` | combinar / merge |
| `✓` | verificado / medido con ejecución |
| `✗` | refutado / falló |
| `!` | invariante / importante |
| `?` | incierto / pendiente |

## Convenciones

- Sin artículos, cortesía ni hedging; fragmentos telegráficos.
- Números exactos SIEMPRE intactos (medidas, versiones, ratios, hashes).
- Nombres propios, paths, identificadores de código intactos.
- Bloques de código intactos (no se comprimen).
- Estructura de headers del original se preserva (navegabilidad).

## Versionado

Cambios al lexicon suben la versión (v1 → v2) y se documentan acá. Cada
`.babel.md` registra `lexicon: v1` en su frontmatter — un babel viejo se
decodifica con la versión del lexicon con la que fue escrito.
