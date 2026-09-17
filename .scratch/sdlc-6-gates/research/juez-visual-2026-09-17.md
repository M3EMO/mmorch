# Juez visual: que dice la evidencia (ticket 15, 2026-09-17)

Dos investigaciones web con subagentes, mas un prototipo medido en la maquina. Los numeros de abajo son de las fuentes;
la ultima seccion es lo que se midio aca.

## 1. Un juez VLM ordena, pero no puntua

| Medicion | Numero | Fuente |
|---|---|---|
| Acuerdo con humanos (scoring / pares / ranking), GPT-4V | 70% / 79.3% / 69% | MLLM-as-a-Judge, arXiv 2402.04788 (ICML 2024) |
| Idem, Gemini | 67.7% / 72% / 47% (Pearson 0.262) | idem |
| gemini-2.5-flash como juez | Pearson 0.459, Spearman 0.446, exacto 32.1% | arXiv 2604.25235 (2026-04-28) |
| Estetica con tolerancia +-1 (AesBench) | 88.3% | idem |
| Realismo contra voto de 3 humanos, gemini-2.5-flash | kappa 0.738, acierto 0.880 | arXiv 2603.04325 (2026) |
| Baseline sin LLM (HPSv2) | 83.3% en dominio, 65.3% fuera | HPSv3, arXiv 2508.03789 (2025-08) |

Un reward model chico iguala o supera al juez VLM. La escala importa menos que la especializacion, pero abajo de ~7B
se derrumba. No hay ningun numero publicado de flash-lite como juez visual.

## 2. Modos de falla que pegan justo en pixel art

- Puntaje absoluto inservible: los intervalos cubren 2.08 a 3.50 sobre una escala de 4 (2604.25235).
- Ceguera a degradaciones: pairwise falla 13-18%, scoring simple falla 32-54% (arXiv 2604.21523).
- Defectos finos: 67% en blur, 66% en ruido, menos de 27% en artefactos de denoising (DistortBench, 2604.19966).
- Resolucion: 59.63% de acierto a 384px contra 67.96% a 1536px (VLM-RobustBench, 2603.06148). Un sprite de 32px es el
  peor caso: hay que agrandarlo con vecino-mas-cercano antes de preguntar.
- Sesgo de posicion (88.2% de repeticion del orden en LLaVA) y de verbosidad (+0.6 a +0.75 puntos).
- Pixel art: CERO benchmarks de juez VLM. La legibilidad de silueta se verifica bajando escala, o sea determinista.

## 3. Cuantos ejemplos hacen falta

5 ejemplos por criterio suben entre 5.1 y 11.8 puntos en 3 de 4 jueces (arXiv 2605.24737). Para reemplazar a un humano,
la practica pide 50 a 100 items con 2 o 3 anotadores (alt-test, ACL 2025). No existe dataset publico de sprites
etiquetados por calidad: las etiquetas salen de los veredictos del usuario.

## 4. Capa determinista: herramientas y practica

- `pixellint` (Python, OSS) es el unico linter de pixel art con aserciones; sale con codigo distinto de cero.
- El CLI de Aseprite corre headless (`-b`) y exporta metadata JSON; no documenta exit codes.
- ImageMagick `compare -metric AE -fuzz X%` cuenta pixeles distintos; pixelmatch y odiff usan threshold 0.1 y en la
  practica se falla recien sobre 0.01% de pixeles distintos.
- Umbrales de pixellint para animacion (criterio del autor, no medicion): loop exacto, linea de piso estable, ritmo
  dentro de pocos puntos, separacion outline/relleno con deltaE 24.
- Nadie publica validacion de hitbox contra alfa: ese chequeo salio de aca.

## 5. Medido en esta maquina (prototipo `sprites_checks.py`)

10 defectos inyectados, 10 atrapados, 0 falsos positivos sobre el sprite bueno: tamano, paleta cerrada, tope de colores,
alfa binario, pixel huerfano, borde sucio, hitbox en el aire, drift entre frames, paleta inestable, loop abierto.
Corre con Pillow y numpy, sin red. Ese prototipo se convirtio en `mmorch/sprites.py`.
