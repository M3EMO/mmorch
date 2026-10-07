---
title: Vocabulario de planos de Storyboard Sim 2.1.1 (nombre de archivo al guardar)
created: 2026-09-27
tags: [research, ssb, research, storyboard-sim, camara, reverse-engineering]
status: seed
sources: [file:///C:/Users/map12/AppData/Local/Programs/Storyboard%20Sim/resources/app/dist/assets/index-BRNPQ79Q.js, file:///C:/Users/map12/AppData/Local/Programs/Storyboard%20Sim/resources/app/dist-electron/main.js, file:///C:/Users/map12/AppData/Local/Programs/Storyboard%20Sim/resources/app/definitions/model_legend.json]
---
mision: ¿Cómo calcula Storyboard Sim 2.1.1 el nombre del plano al guardar, y qué reglas exactas puede invertir SSB?

## Tronco

El nombre es `TAMAÑO_ANGULO_EJE_PERSONAJES.png` con códigos fijos en inglés; el tamaño sale del % de píxeles de silueta, el ángulo del pitch de cámara, y el OTS del yaw respecto del eje entre dos modelos.

## Qué es

- Fuente: bundle `dist/assets/index-BRNPQ79Q.js` (offsets de byte ~1521400–1526100) y `dist-electron/main.js`.
- Método: grep -o y dd por offsets. Lectura estática; no se ejecutó la app.
- Los códigos NO están en i18n. Ningún idioma traduce los nombres de plano. No existen cadenas "Primer plano", "Picado", "Medium Shot", アップ, あおり, 俯瞰 ni ロング en el bundle.
- La única cadena i18n relacionada es `bar.lineOfAction`: en "Line of action (imaginary line)", es "Línea de acción (eje imaginario)", ja "イマジナリーライン".
- La app NO usa huesos MMD ni humanoides para el nombre. Usa la silueta renderizada, la caja envolvente (Box3) y el origen del modelo.

## Evidencia / mecanismo

### Orquestador: `_E(r, opts)` → `extract(force)`
- `_E` crea un extractor. `extract` devuelve `{shotSize, angle, lineOfAction, charNames, ots, otsPair}`.
- El default es `xg()`: `NOSHOT`, `EL`, `NOLINE`, `NOCHARA`, `NO_OTS`.
- El tamaño se recalcula cada 200 ms (`mE=200`) y nunca durante un arrastre. Al guardar se fuerza (`extract(!0)`).
- `DE(...)` actualiza en vivo el elemento `#filename-preview` con el mismo nombre.
- Llamadores al guardar: `ME` (Ctrl+S, PNG 1920x1080) y `CE` (guardado por capas; usa el nombre sin `.png` como carpeta).

### Personajes en cuadro: `bE(camera, models)`
- `bE` arma un `Frustum` desde `projectionMatrix * matrixWorldInverse`.
- Un modelo está "en cuadro" si su `Box3.setFromObject(model)` intersecta el frustum.
- Recorre `r.models` completo. No filtra por `category` ("character"/"background"). No filtra por visibilidad.
- Consecuencia: un fondo .glb, un plano de imagen o un modelo oculto también cuentan como "personaje".
- `charNames` = nombres en cuadro unidos con `_`, en orden de carga (`r.models`). Si no hay ninguno: `NOCHARA`.
- Nombre del modelo: `cm(path, legend)` busca en `definitions/model_legend.json` por sufijo de filename (ej. `shonen.pmx` → `shonen`). Si no hay entrada, usa el nombre del header PMX (`metadata.name`) o `Model N`.
- `Wo(name)` desambigua duplicados con `_2`, `_3`, etc. Un duplicado manual recibe `<nombre>_copy`.

### Tamaño de plano: `SE(camera, inFrame, renderer, rt, r)`
- `SE` renderiza solo los modelos en cuadro, con material blanco (`yE`, color 16777215) sobre fondo negro.
- El render target mide como máximo 160 px en su lado mayor (`gE=160`). Con 16:9 el target mide 160x90 = 14400 px.
- Cuenta píxeles con R, G o B > 0. Calcula `b = encendidos / total * 100`.
- Umbrales (estrictos, de arriba hacia abajo):

| Código | Condición de cobertura b | Nombre usual |
|---|---|---|
| BCU | b > 70 % | big/extreme close-up |
| CU | 40 < b ≤ 70 | close-up |
| MCU | 25 < b ≤ 40 | medium close-up |
| MS | 15 < b ≤ 25 | medium shot |
| MLS | 5 < b ≤ 15 | medium long shot |
| LS | 0.1 < b ≤ 5 | long shot |
| ELS | b ≤ 0.1 (≤ 14 px) | extreme long shot |
| NOSHOT | ningún modelo en cuadro | — |

- La cobertura es la UNIÓN de todas las siluetas en cuadro, no por personaje. Dos personajes en MS pueden dar MCU.
- `SE` oculta grid, ejes y el gizmo. NO oculta `lineOfActionHelper` ni objetos `userData.editorOnly`. Si el plano de eje imaginario está visible, infla la cobertura (probable bug).

### Ángulo: `vE(camera)`
- `vE` toma `camera.getWorldDirection()` (vector unitario). Usa solo la componente y.
- `y < -0.3` → `HA` (high angle, picado). `y > 0.3` → `LA` (low angle, contrapicado). Resto → `EL` (eye level).
- En grados: pitch = asin(y). El umbral es asin(0.3) ≈ 17.46°. Picado si pitch < −17.46°. Contrapicado si pitch > +17.46°.
- No existen categorías cenital, nadir ni holandés. El roll no importa. La altura de cámara respecto del personaje no importa.

### Eje imaginario (lineOfAction): `AE(camera, inFrame, r)`
- `AE` exige exactamente 2 modelos en cuadro. Si no, devuelve `NOLINE`.
- A = primer modelo, B = segundo, en orden de carga. Usa `getWorldPosition()` del objeto raíz (el origen, normalmente los pies en MMD).
- `l = B − A`. Normal `d = l × (0,1,0) = (−l.z, 0, l.x)`. Plano vertical por A con esa normal.
- Distancia con signo de la cámara al plano ≥ 0 → `LINE_A`; < 0 → `LINE_B`. Si A y B están alineados en vertical → `NOLINE`.
- Lectura geométrica (derivada, no probada en runtime): en `LINE_A` la cámara ve a A a la izquierda y a B a la derecha.

### Sobre el hombro: `xE(camera, inFrame, line)`
- `xE` exige ≥ 2 modelos y el vector del eje de `AE`. En la práctica exige exactamente 2 en cuadro.
- `n` = dirección de cámara proyectada al piso (y=0), normalizada. `i` = dirección horizontal A→B.
- `d = acos(i·n)` en grados. `h = 180 − d`.
- `d ≤ 35°` (`Ag=35`) → candidato `A_OVER_B` (sobre el hombro de A, mirando a B).
- `h ≤ 35°` → candidato `B_OVER_A`.
- Filtro de distancia `bg = 1e6`: en la práctica no filtra nada.
- Gana el candidato con menor ángulo. `g` = dirección over→target.
- Lado: `g.x*n.z − g.z*n.x ≥ 0` → `OTS_LEFT` (`L`); si no → `OTS_RIGHT` (`R`).
- Lectura geométrica (derivada, no probada): `R` ocurre cuando la cámara gira hacia la izquierda respecto del eje over→target. Es el caso típico de cámara detrás del hombro derecho que converge hacia el target. Cámara paralela al eje exacto da `L` (empate ≥ 0).
- La app no mira hacia dónde mira el personaje. No usa la rotación del modelo ni huesos de hombro o cabeza.

### Formato del nombre: `um(spec)` y `wE(ots)`
- `wE`: `OTS_LEFT` → `L`, `OTS_RIGHT` → `R`, otro → null.
- `um`: base = `${shotSize}_${angle}_${lineOfAction}`.
- Con OTS: `${base}_${over}_OTS_${L|R}_${target}.png`. Sin OTS: `${base}_${charNames}.png`.
- Fallback si falla extract: `shot.png`.
- `main.js` `G(name)`: reemplaza `<>:"/\|?*` y controles por `_`. Recorta puntos y espacios finales. Si pasa 120 bytes UTF-8, trunca y agrega `_` + 10 hex de SHA-256.
- `main.js` `ra(path)`: con autoRename agrega `_ver001`, `_ver002`… si el archivo existe.

Ejemplos (construidos con las reglas):
- `MS_EL_NOLINE_shonen.png` — un personaje, plano medio, altura de ojos.
- `MCU_LA_LINE_A_shonen_onesan.png` — dos personajes, contrapicado, cámara del lado A.
- `CU_HA_LINE_B_onesan_OTS_R_shonen.png` — picado, sobre el hombro derecho de onesan, mirando a shonen.
- `NOSHOT_EL_NOLINE_NOCHARA.png` — nada en cuadro.
- `LS_EL_NOLINE_shonen_背景A.png` — el fondo cuenta como "personaje".

Vocabulario en español (propuesta para SSB; la app no lo trae):
- BCU = primerísimo primer plano. CU = primer plano. MCU = plano medio corto. MS = plano medio. MLS = plano americano / medio largo. LS = plano general. ELS = gran plano general.
- HA = picado. LA = contrapicado. EL = normal / altura de ojos.
- OTS_L / OTS_R = sobre el hombro izquierdo / derecho. LINE_A / LINE_B = lado A / lado B del eje imaginario.

## Aplicable a SSB (camino inverso)
- Ángulo: fijar pitch fuera de ±17.46° para HA/LA. Usar margen, por ejemplo −25° y +25°. EL = pitch cercano a 0.
- Lado del eje: poner la cámara en el semiespacio `sign(dot(C − A, (−l.z,0,l.x)))`.
- OTS: yaw de cámara a ≤ 35° del eje over→target horizontal. Elegir el signo de la desviación según L/R. Verificar que ambos modelos intersecten el frustum y que no haya un tercer modelo en cuadro.
- Tamaño: la cobertura no tiene fórmula cerrada respecto de huesos. SSB debe iterar distancia o focal y medir la máscara a 160x90, igual que `SE`. La cobertura crece de forma monótona al acercar o al subir la focal.
- Para chequear SSB contra la app sin LLM: reimplementar `vE`, `AE`, `xE`, `SE` y `um` como oráculo (checkers.py).

## Objeciones
- La lectura es estática. El sentido de `L`/`R` y de `LINE_A` sale de álgebra sobre la convención de three.js; falta una prueba en runtime con dos modelos.
- La relación "% cobertura ↔ encuadre corporal" depende del modelo y la pose. Falta medir, por ejemplo, qué % da un encuadre de cintura con shonen.pmx.
- `Box3.setFromObject` sobre SkinnedMesh (three r164 probable) puede cachear la caja de la primera pose. Una pose muy distinta puede fallar la detección en bordes. No verificado.

## Veredicto cross-family
- Sin verificar. Recomendación: verificar con un oráculo ejecutable, no con un LLM.

## Links
- [[ssb]]
