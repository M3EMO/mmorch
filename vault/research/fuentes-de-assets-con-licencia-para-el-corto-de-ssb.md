---
title: Fuentes de assets con licencia para el corto de SSB
created: 2026-10-01
tags: [research, ssb, assets, licencias, pmx, vrm, glb, hdri, storyboard-sim, corto]
status: seed
confidence: media-alta
sources: [https://hub.vroid.com/en/characters/2843975675147313744/models/5644550979324015604, https://hub.vroid.com/en/characters/1248981995540129234/models/8640547963669442173, https://vroid.pixiv.help/hc/en-us/articles/4402394424089, https://3d.nicovideo.jp/works/td14712, https://3d.nicovideo.jp/alicia/rule.html, https://3d.nicovideo.jp/works/td81420, https://bowlroll.net/file/270618, https://sketchfab.com/3d-models/japanese-classroom-2a1e3b294c1e4e91bed794bfa520c4f4, https://sketchfab.com/3d-models/anime-stylized-room-free-223df82515de41e684720e2eaa02e93d, https://sketchfab.com/3d-models/stylized-little-japanese-town-street-200fc33b8a2b4da98e71590feeb255a8, https://quaternius.com/packs/ultimatehomeinterior.html, https://kenney.nl/assets/furniture-kit, https://polyhaven.com/license, https://api.polyhaven.com/files/lebombo, https://api.polyhaven.com/files/kloofendal_48d_partly_cloudy_puresky, https://github.com/miu200521358/vroid2pmx/releases]
---
# Fuentes de assets con licencia para el corto de SSB

Ticket: `ssb/.scratch/goal-corto/issues/01-fuentes-de-assets.md`.
Fecha de verificación: 2026-09-30 / 2026-10-01.
Método: lectura de las páginas fuente. No se descargó ningún archivo.

## Hallazgo clave sobre el pipeline

- SSB clasifica solo `.pmx` y `.pmd` como `personaje` (spec-v1, F2-R2).
- SSB clasifica `.glb .gltf .fbx .ply` como `set`.
- `construirEscena` y `verificarPlano` arman mallas solo desde PMX (F7a-R2, F7a-R9).
- Las poses por huesos (`models[].bones`) asumen un esqueleto MMD.
- Conclusión: un personaje VRM debe pasar a **PMX**, no a GLB.
- Un VRM → GLB carga en la app como set, sin el flujo de poses ni la verificación de encuadre.
- Herramienta de conversión: Vroid2Pmx de miu200521358 (MIT, Windows).
- La última versión es ver2.01.06 (2023-10-22). Esa versión solo acepta VRM 0.0 y da error con VRM 1.0.
- Los AvatarSample A/B/C de VRoid Hub son VRM 0.0. Son compatibles.
- Bajar Vroid2Pmx también requiere el permiso del usuario.

## Tabla de candidatos

| # | Tipo | Asset | Fuente y autor | Licencia o términos | Formato y tamaño | Carga en la app |
|---|---|---|---|---|---|---|
| P1 | Personaje | ニコニ立体ちゃん (Alicia Solid) | [3d.nicovideo.jp/works/td14712](https://3d.nicovideo.jp/works/td14712). Dwango. Diseño: 黒星紅白. Modelado: 雨刻 | Crédito no requerido. Comercial sí, excepto empresas (法人を除く). Modificar y redistribuir sí. Prohíbe: actos contra 公序良俗, violencia, fines antisociales, uso religioso o político, presentarlo como oficial, dañar la imagen. [Reglas](https://3d.nicovideo.jp/alicia/rule.html) | `Alicia.zip`, 83.35 MB. MMD + FBX + Unity. 31,866 polígonos. Login niconico | PMX directo |
| P2 | Personaje | AvatarSample_C (masculino) | [VRoid Hub](https://hub.vroid.com/en/characters/1248981995540129234/models/8640547963669442173). VRoid Project (pixiv) | Avatar, violencia, sexual, empresas, comercial individual, redistribución y alteración: Allow. Crédito: Not required. No es CC0. Prohíbe: redistribuir con cobro, marcarlo como CC0, fraude, discriminación, extremismo, actividad política excesiva. [Ayuda](https://vroid.pixiv.help/hc/en-us/articles/4402394424089) | VRM 0.0. Tamaño no publicado. Login pixiv y aceptar los ToS de VRoid Hub | VRM → PMX con Vroid2Pmx |
| P3 | Personaje | AvatarSample_A (femenino) | [VRoid Hub](https://hub.vroid.com/en/characters/2843975675147313744/models/5644550979324015604). VRoid Project | Las mismas condiciones que P2 | VRM 0.0. Tamaño no publicado | VRM → PMX con Vroid2Pmx |
| E1 | Escenario | 教室ステージMMDモデル | [BowlRoll 270618](https://bowlroll.net/file/270618) / [ニコニ立体 td81420](https://3d.nicovideo.jp/works/td81420). Conversión: 時の番人. Base: "Classroom" de Pino_156 (Sketchfab, CC BY 4.0) | Uso libre con crédito a Pino_156 y a 時の番人. El autor pide "商用利用はご遠慮ください". Redistribución permitida si hereda las condiciones | `Classroom.zip`, 4.28 MB, PMX, 60,691 vértices. Sin pasillo ni exterior | PMX directo (como set) |
| E2 | Escenario | Japanese Classroom | [Sketchfab](https://sketchfab.com/3d-models/japanese-classroom-2a1e3b294c1e4e91bed794bfa520c4f4). T I A N (@Tian96) | CC Attribution (CC BY 4.0) | 326k triángulos. Tamaño no publicado. Sketchfab ofrece glTF/GLB. Login | GLB directo |
| E3 | Escenario | Anime stylized room free (tatami, Showa) | [Sketchfab](https://sketchfab.com/3d-models/anime-stylized-room-free-223df82515de41e684720e2eaa02e93d). CG Lads | CC Attribution | 147.4k triángulos. Tamaño no publicado | GLB directo |
| E4 | Escenario | Stylized Little Japanese Town Street | [Sketchfab](https://sketchfab.com/3d-models/stylized-little-japanese-town-street-200fc33b8a2b4da98e71590feeb255a8). Michał Solarek (@misiek13) | CC Attribution | 234.7k triángulos. Tamaño no publicado. Es un diorama: revisar la escala | GLB directo |
| E5 | Kit | Ultimate House Interior Pack | [Quaternius](https://quaternius.com/packs/ultimatehomeinterior.html) | CC0 | 123 modelos. FBX, OBJ, Blend | FBX directo pieza por pieza. Una escena completa requiere armado (ticket 05 o Blender → GLB) |
| H1 | HDRI | Lebombo (living vacío con sol) | [Poly Haven](https://polyhaven.com/a/lebombo). Greg Zaal | CC0. [Licencia](https://polyhaven.com/license): sin crédito, uso comercial libre | 2K HDR 6.0 MB. 4K HDR 24.4 MB. 4K EXR 15.3 MB | HDR/EXR directo |
| H2 | HDRI | Kloofendal 48d Partly Cloudy (Pure Sky) | [Poly Haven](https://polyhaven.com/a/kloofendal_48d_partly_cloudy_puresky). Greg Zaal, Jarod Guest | CC0 | 2K HDR 5.5 MB. 4K HDR 20.7 MB. 4K EXR 75.6 MB | HDR/EXR directo |

Otros datos verificados:

- Kenney Furniture Kit es CC0 y trae 140 archivos. La página no lista los formatos.
- Quaternius Modular Streets Pack es CC0 y trae 25 modelos en FBX, OBJ y Blend.
- "Tokyo train platform" (Sketchfab, MTSU, CC BY) es un escaneo fotorrealista. Su look choca con el toon.
- Los Gaussian splats PLY con licencia CC existen. Su look fotorrealista choca con el toon. Esta nota no los prioriza.

## Top 3 para dos personajes en un lugar cotidiano

1. **Alicia Solid (P1)** es la protagonista. Carga como PMX directo y no exige crédito.
2. **AvatarSample_C (P2)** es el segundo personaje. Es masculino y contrasta con Alicia. Requiere VRM → PMX con Vroid2Pmx.
3. **Aula PMX de BowlRoll (E1)** es el escenario. Pesa 4.28 MB, ya está en PMX y trae morphs para quitar pupitres y la pared trasera. Para las ventanas, sumar el HDRI H2 como cielo.

Plan B de escenario: E3 (habitación tatami, CC BY, GLB directo) con el HDRI H1.

## Créditos a poner en el video

- E1: "Classroom" de Pino_156 (CC BY 4.0), conversión a MMD de 時の番人.
- E2/E3/E4: "Título" de Autor, Sketchfab, CC BY 4.0, con link.
- P1, P2, P3 y Poly Haven no exigen crédito. Conviene acreditarlos igual.

## Dudas de licencia abiertas

- E1: el convertidor pide no hacer uso comercial. La base es CC BY 4.0. Un video monetizado puede chocar con ese pedido.
- P1: la licencia excluye a empresas. Un futuro uso por una empresa necesita otro acuerdo con Dwango.
- P1: la licencia prohíbe violencia y mensajes políticos o religiosos. La historia debe respetar eso.
- P2/P3: pixiv avisa que puede cambiar las condiciones. Conviene guardar una captura de las condiciones al bajar.
- P2/P3: bajar el VRM exige cuenta pixiv y aceptar los ToS de VRoid Hub. El usuario hace ese paso.
- Sketchfab CC BY: nadie verifica que el autor tenga los derechos. Evitar recreaciones de animes o ripeos de juegos (por ejemplo "Naruto's Apartment" o "Ichigo Bedroom").
- Los tamaños de los VRM y de los GLB de Sketchfab no figuran en las páginas.
