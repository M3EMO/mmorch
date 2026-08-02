# Lexicon babel — DICCIONARIO VIVO

Clave de decodificación para archivos `*.babel.md` del vault. Los `.babel.md`
son derivados comprimidos model-native (paper 2606.19857) del archivo original
homónimo — el original es SIEMPRE la fuente de verdad; el babel es para que los
modelos lean con menos tokens. Un modelo que lee un babel debe recibir este
lexicon como decoder key.

**Este doc se actualiza siempre**: `python -m mmorch.babel --mine` escanea los
babels existentes y lista shorthand usado pero no documentado → se agrega acá
(sección Candidatos → promovido a tabla), sube la versión, y cada `.babel.md`
registra en frontmatter con qué versión se escribió. Editable también a mano
desde Obsidian (este archivo vive en el vault).

`version: 2`

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
| `vs` | comparado con |
| `+` | y / además |

## Términos del dominio (abreviatura → expansión)

| Abrev | Expansión |
|-------|-----------|
| `xfam` | cross-family (generador y verificador de familias distintas) |
| `WT` | worktree aislado |
| `fid` | fidelidad (score QA del gate babel) |
| `AR` | autoresearch (hillclimb nocturno) |
| `HITL` | human-in-the-loop |

## Candidatos (minados, sin promover)

_Vacío. `--mine` apendea acá; un humano (u Opus) promueve a las tablas de arriba
y sube la versión._

## Convenciones

- Sin artículos, cortesía ni hedging; fragmentos telegráficos.
- Números exactos SIEMPRE intactos (medidas, versiones, ratios, hashes).
- Nombres propios, paths, identificadores de código intactos.
- Bloques de código intactos (no se comprimen).
- Estructura de headers del original se preserva (navegabilidad).

## Regla crítica (medida 2026-08-02)

Este lexicon NUNCA va en el prompt del ENCODER: una sola línea de símbolos en
ese prompt rompe la compresión (ratio 0.52 → 1.02 medido). Es exclusivamente
decoder key del LECTOR.

## Versionado

- v1 (2026-08-02): seed inicial, símbolos + convenciones.
- v2 (2026-08-02): diccionario vivo — términos del dominio, sección candidatos,
  proceso `--mine`, regla encoder-nunca.
