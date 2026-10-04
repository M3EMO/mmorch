---
title: "Storyboard Sim: poses y animación por huesos desde el JSON de escena"
created: 2026-10-01
tags: [research, ssb, storyboard-sim, mmd, pmx, three, huesos, ik, fisica, morphs]
status: seed
confidence: alta en código y PMX; media en el comportamiento visual (no se corrió la app)
sources: [Storyboard Sim 2.1.1 resources/app/dist/assets/index-BRNPQ79Q.js (Ms, N9, H9, GE, p3, m3, A9, MMDAnimationHelper, GrantSolver, MMDPhysics, MMDLoader), resources/app/dist/model.pmx (onesan03, parseado con three 0.164.1 mmdparser parsePmx(buf, true)), ssb/fixtures/03-escena.json]
---
# Storyboard Sim: poses y animación por huesos

Ticket: `ssb/.scratch/goal-corto/issues/02-poses-por-huesos.md`.
Método: lectura del bundle `index-BRNPQ79Q.js` con node `indexOf`/`slice`.
Método: parseo de `dist/model.pmx` con `MMDParser.parsePmx(buffer, true)` de three 0.164.1.
Método: cinemática directa con `THREE.Bone` para verificar signos.
Nadie corrió la app real. La sección final lista lo que hay que validar.

## Resumen

- Todo `quaternion` de hueso es la rotación LOCAL de `THREE.Bone`, relativa al padre.
- En reposo cada hueso MMD tiene quaternion identidad. Por eso local al padre = relativo al reposo.
- `position` REEMPLAZA a la posición local. La posición de reposo es el offset al padre, no cero.
- Sin VMD cargado, la app NO resuelve IK ni 付与 (grant). Solo corre la física.
- En `onesan03` las piernas de la malla cuelgan de `左足D/左ひざD/左足首D`. Sin VMD hay que rotar también los huesos D.
- `onesan03` tiene 29 cuerpos rígidos, todos tipo 0, y 0 joints. La física no pisa ningún hueso. Pelo y falda no tienen física.
- La línea de tiempo no tiene pistas de morphs. Los morphs son estáticos por escena.

## 1. Espacio de los quaternions

### `models[].bones` (serializar y aplicar)

`N9` serializa y `Ms` (export `applyBoneStates`) aplica:

```js
function oi(r){return Z1(r)?.skeleton?.bones??[]}   // huesos del SkinnedMesh más grande
function N9(r){const e=oi(r);if(e.length!==0)return e.map((t,n)=>({index:n,name:t.name,
  position:t.position.clone(),
  quaternion:{x:t.quaternion.x,y:t.quaternion.y,z:t.quaternion.z,w:t.quaternion.w},
  scale:t.scale.clone()}))}
function Ms(r,e){if(!e)return;const t=oi(r);t.length!==0&&(e.forEach(n=>{
  const i=t[n.index],s=i?.name===n.name?i:t.find(o=>o.name===n.name);
  s&&(rs(s.position,n.position),s.quaternion.copy(k9(n.quaternion)),rs(s.scale,n.scale))}),
  r.updateMatrixWorld(!0))}
function rs(r,e){c5(e)?r.copy(e):Array.isArray(e)?r.fromArray(e):r.set(e.x,e.y,e.z)}
function k9(r){return new Ye(r.x,r.y,r.z,r.w)}      // Ye = THREE.Quaternion
```

- `Ms` busca el hueso por `index`. Si el nombre no coincide, busca por `name`.
- `Ms` escribe `bone.position`, `bone.quaternion` y `bone.scale`. Esos campos son locales al padre en three.
- `quaternion` debe ser objeto `{x,y,z,w}`. `k9` no acepta arrays: un array da NaN.
- `position` y `scale` aceptan objeto `{x,y,z}` o array.
- Los huesos ausentes de la lista quedan como están.

### `timeline.bones[uuid][hueso]`

`GE` crea el controlador de timeline. Su función interna `C` aplica las claves de huesos:

```js
function C(q,ie){const he=u().bones[q.uuid];if(!he)return;const N=oi(q);if(N.length===0)return;let P=!1;
  for(const[se,ce]of Object.entries(he)){if(!ce?.length)continue;
    const we=N.find(Qe=>Qe.name===se),Se=we?p3(ce,ie):null;
    !we||!Se||r.transformControls?.dragging&&r.transformControls.object===we||(m3(we,Se),P=!0)}
  P&&q.updateMatrixWorld(!0)}
function p3(r,e){const t=r?Op(r,e):null;if(!t)return null;const{a:n,b:i,t:s}=t;
  return{position:No(n.position,i.position,s),quaternion:i5(n.quaternion,i.quaternion,s),scale:No(n.scale,i.scale,s)}}
function m3(r,e){r.position.fromArray(e.position),r.quaternion.fromArray(e.quaternion),r.scale.fromArray(e.scale)}
function Ba(r,e,t){return{frame:e,position:r.position.toArray(),quaternion:r.quaternion.toArray(),scale:r.scale.toArray(),interp:t}}
function A9(r,e){return r==="step"?0:r==="ease"?e*e*(3-2*e):e}
```

- La pista busca el hueso por NOMBRE, no por índice.
- Cada clave es `{frame, position:[x,y,z], quaternion:[x,y,z,w], scale:[x,y,z], interp}`. Aquí son arrays.
- `m3` reemplaza position, quaternion y scale locales. Es el mismo espacio que `models[].bones`.
- `Op` hace búsqueda binaria. Antes de la primera clave, la app usa la primera clave. Después de la última, la app mantiene la última.
- La curva del tramo usa el `interp` de la clave ANTERIOR: `linear`, `ease` (smoothstep) o `step`.
- La posición y la escala interpolan lineal. El quaternion interpola con `slerp`.
- La app no aplica la clave del hueso que el usuario arrastra con el gizmo.
- Los botones "key" y "pose" del UI guardan con `Ba` el estado local actual del hueso.

### Orden de carga

- `H9` (restoreState) aplica `Ms(m, y.bones)` y los morphs. Después llama `evaluateTimeline`.
- Resultado: una pista de timeline pisa a `models[].bones` para ese hueso. Los huesos sin pista conservan `models[].bones`.
- Al cargar un modelo, el orden es este: `helper.add(physics, warmup 60)`, VMD, VPD, `Ms`, morphs.

## 2. `position`: reemplaza

- `MMDLoader` crea cada hueso con `pos = posPMX - posPMX(padre)` y `rotq = [0,0,0,1]`.
- `Ms` y `m3` reemplazan ese valor con `copy`/`fromArray`. No suman.
- El fixture `fixtures/03-escena.json` lo confirma: センター guarda `{x:0, y:8.3148, z:0.0318}` y todos los quaternions son identidad.
- Regla: posición nueva = offset de reposo + delta. Para bajar la cadera 1 unidad, センター usa `{x:0, y:7.3148, z:0.0318}`.
- Un `position` en cero colapsa el hueso sobre su padre.
- Contraste: el pose VPD de la app (`helper.pose`) sí suma la traslación y multiplica el quaternion. El JSON no hace eso.

## 3. IK, grant y física

`helper` es el `MMDAnimationHelper` de three r164 incluido en el bundle. La app llama `helper.update(min(dt,0.1))` en cada tick (`F` de `GE`):

```js
_animateMesh(e,t){const n=this.objects.get(e),i=n.mixer,s=n.ikSolver,o=n.grantSolver,a=n.physics,l=n.looped;
 i&&this.enabled.animation&&(this._restoreBones(e),i.update(t),this._saveBones(e),
   /* pmxAnimation=false */ (s&&this.enabled.ik&&(e.updateMatrixWorld(!0),s.update()),o&&this.enabled.grant&&o.update())),
 l===!0&&this.enabled.physics&&(a&&this.configuration.resetPhysicsOnLoop&&a.reset(),n.looped=!1),
 a&&this.enabled.physics&&!this.sharedPhysics&&(this.onBeforePhysics(e),a.update(t))}
```

### Modelo SIN VMD (el caso normal al armar desde JSON)

- La app agrega el modelo con `n.add(mesh,{physics:!0,warmup:60})`. Esa llamada no crea `mixer`.
- Sin `mixer`, el helper NO ejecuta `ikSolver.update()` ni `grantSolver.update()`.
- Una clave en `左足ＩＫ` o `左つま先ＩＫ` solo mueve el hueso IK. Ese hueso no tiene pesos de vértice, así que la malla no cambia.
- Las piernas se posan por FK: rotación directa de 足, ひざ y 足首.
- Los huesos 付与 no siguen a su padre de grant. Esto afecta a `onesan03` así:
  - La malla de la pierna cuelga de `左足D/左ひざD/左足首D` (grant ratio 1 de 足/ひざ/足首, padre `腰キャンセル左`). Hay que escribir el MISMO quaternion en `左足` y en `左足D`, y así en cada par.
  - Los ojos de la malla cuelgan de `左目/右目`. `両目` no tiene pesos. Hay que rotar `左目` y `右目` directamente.
  - `上半身3` no recibe el 40% extra de `上半身2`. El pecho gira un poco menos que en MMD.
  - `左腕捩1..3` no siguen a `左腕捩`. El giro del antebrazo no se reparte; evitar `腕捩` y `手捩`.
  - `腰キャンセル` no anula `腰`. Si rotás `腰`, también rotan las piernas.
- Los brazos funcionan bien por FK. `左腕捩1..3` son hijos de `左腕` y heredan su rotación.
- La app aplica las claves de timeline solo cuando cambia el frame (`R`). Entre frames, los huesos quedan como están.

### Modelo CON VMD (mixer presente)

- `GE.x` envuelve `mixer.update`: evalúa el VMD con delta 0 y después aplica las claves (`C`).
- Orden por tick: restaurar backup, VMD, claves de timeline, guardar backup, IK, grant, física.
- Las claves de huesos actúan como una capa de animación encima del VMD.
- El IK corre después de las claves. El IK pisa las rotaciones de `ひざ` y `足` que vienen de claves. Una clave en `足ＩＫ` sí mueve la pierna.
- El grant corre y MULTIPLICA el quaternion del hueso D (`bone.quaternion.multiply(slerp(id, padre.q, ratio))`). Si además hay clave en `左足D`, la rotación se aplica dos veces. Con VMD hay que poner claves solo en los huesos no-D.

### Física ammo

- `MMDPhysics.updateBone` pisa el hueso solo para cuerpos tipo 1 (rotación y posición) y tipo 2 (rotación). Tipo 0 sigue al hueso.
- La física corre en cada tick con o sin VMD. La app nunca llama `helper.enable(...)`. No hay un interruptor de física en el UI ni en el JSON.
- La física se resetea al saltar más de 2 frames, al ir hacia atrás y al hacer loop. El export reset hace 30 pasos.
- `_optimizeIK(mesh, true)` desactiva los eslabones IK de huesos con cuerpo tipo 1 o 2.
- En `onesan03`: 29 cuerpos, TODOS tipo 0, 0 joints. Ningún hueso de pelo o falda tiene cuerpo rígido. La física no pisa nada.
- Pelo (`髪_*`) y falda (`装飾_*`) son FK puro en este modelo. Una pose en esos huesos queda fija.
- Otro modelo con cuerpos tipo 1/2 perdería las claves de esos huesos en cada tick. El único arreglo sería cambiar el PMX.

### Para que una pose quede fija (sin VMD)

- No hace falta desactivar nada en `onesan03`.
- Hay que evitar IK y grant: posar por FK y duplicar en huesos D.
- Hay que poner la pose en `models[].bones`. También sirve una clave única en `timeline.bones`.

## 4. Jerarquía de `onesan03`

Datos: Vroid2Pmx 2.01.06, 193 huesos, 171 morphs, 29 cuerpos rígidos, 0 joints.
Convención three (después de `leftToRight`): Y arriba, el modelo mira hacia **+Z**, su IZQUIERDA es **+X**.
`pos` es la posición de mundo en reposo. La posición local del JSON es `pos - pos(padre)`.
Leyenda: ★ = útil para posar. "pesos" = el hueso tiene vértices.

| idx | nombre | padre | pos reposo (mundo) | notas |
|---|---|---|---|---|
| 0 | 全ての親 | - | 0, 0, 0 | raíz, movible |
| 1 | ★センター | 0 | 0, 8.31, 0.03 | movible, traslación del cuerpo |
| 2 | グルーブ | 1 | 0, 9.50, 0.04 | movible |
| 3 | 腰 | 2 | 0, 11.88, 0.05 | movible |
| 4 | ★下半身 | 3 | 0, 12.64, 0.21 | pesos |
| 5 | ★上半身 | 3 | 0, 12.64, 0.21 | pesos |
| 6 | ★上半身2 | 5 | 0, 14.10, 0.26 | pesos |
| 7 | 上半身3 | 6 | 0, 15.48, 0.08 | grant 0.4 de 上半身2, muchos pesos |
| 8 | ★首 | 7 | 0, 17.10, -0.39 | pesos |
| 9 | ★頭 | 8 | 0, 18.20, -0.25 | pesos |
| 10 | 両目 | 9 | 0, 18.89, -0.03 | sin pesos; solo sirve vía grant |
| 11 / 12 | ★左目 / 右目 | 9 | ±0.18, 18.89, -0.03 | grant 0.3 de 両目; rotar directo |
| 16-19 | 舌1-4 | 9→ | - | lengua |
| 20 / 22 | 左胸 / 右胸 | 7 | ±0.67, 15.39, 0.92 | |
| 24 / 58 | 左肩P / 右肩P | 7 | ±0.26, 16.77, -0.29 | |
| 25 / 59 | ★左肩 / 右肩 | 24 / 58 | ±0.26, 16.77, -0.29 | pesos |
| 26 / 60 | 左肩C / 右肩C | 25 / 59 | ±1.34, 16.61, -0.29 | grant -1 de 肩P, oculto |
| 27 / 61 | ★左腕 / 右腕 | 26 / 60 | ±1.34, 16.61, -0.29 | reposo 35° bajo la horizontal (pose A) |
| 28 / 62 | 左腕捩 / 右腕捩 | 27 / 61 | ±2.48, 15.81, -0.29 | eje fijo; evitar sin VMD |
| 29-31 / 63-65 | 腕捩1-3 | 27 / 61 | - | grant de 腕捩, ocultos, pesos |
| 32 / 66 | ★左ひじ / 右ひじ | 28 / 62 | ±3.62, 15.01, -0.29 | |
| 33 / 67 | 左手捩 / 右手捩 | 32 / 66 | ±4.69, 14.27, -0.29 | eje fijo |
| 34-36 / 68-70 | 手捩1-3 | 32 / 66 | - | grant, ocultos, pesos |
| 37 / 71 | ★左手首 / 右手首 | 33 / 67 | ±5.75, 13.52, -0.29 | |
| 38-57 / 72-91 | ★dedos | 37 / 71 | - | 親指０,１,２; 人指１-３; 中指１-３; 薬指１-３; 小指１-３ (+ 先) |
| 92 / 100 | 腰キャンセル左 / 右 | 4 | ±0.91, 11.39, -0.10 | grant -1 de 腰, oculto |
| 93 / 101 | ★左足 / 右足 | 92 / 100 | ±0.91, 11.39, -0.10 | eslabón IK; SIN pesos |
| 94 / 102 | ★左ひざ / 右ひざ | 93 / 101 | ±0.91, 6.67, -0.02 | eslabón IK con límite; sin pesos |
| 95 / 103 | ★左足首 / 右足首 | 94 / 102 | ±0.91, 1.30, -0.42 | efector IK; sin pesos |
| 96 / 104 | 左つま先 / 右つま先 | 95 / 103 | ±0.90, 0.00, 1.27 | |
| 97 / 105 | 左足IK親 / 右足IK親 | 0 | ±0.91, 0.00, -0.42 | movible |
| 98 / 106 | ★左足ＩＫ / 右足ＩＫ | 97 / 105 | ±0.91, 1.30, -0.42 | IK: efector 足首, links ひざ (límite X -180°..-0.5°), 足; 40 iter |
| 99 / 107 | 左つま先ＩＫ / 右つま先ＩＫ | 98 / 106 | ±0.90, 0.00, 1.27 | IK: efector つま先, link 足首 |
| 108 / 112 | ★左足D / 右足D | 92 / 100 | ±0.91, 11.39, -0.10 | grant 1.0 de 足; tc=1; PESOS del muslo |
| 109 / 113 | ★左ひざD / 右ひざD | 108 / 112 | ±0.91, 6.67, -0.02 | grant 1.0 de ひざ; PESOS |
| 110 / 114 | ★左足首D / 右足首D | 109 / 113 | ±0.91, 1.30, -0.42 | grant 1.0 de 足首; PESOS |
| 111 / 115 | 左足先EX / 右足先EX | 110 / 114 | - | pesos de la punta del pie |
| 116-161 | 髪_01..12-* | 9 (頭) | - | 12 mechones de 3-5 huesos; sin física |
| 162-189 | 装飾_* (CoatSkirt) | 94 / 102 (ひざ) | - | falda del abrigo; cuelga de ひざ, no de 下半身; sin física |
| 190-192 | Face, Body, Hair | - | 0, 0, 0 | ocultos |

Notas:
- La falda (`装飾_*`) cuelga de `ひざ`, que no tiene pesos. Sin VMD, rotar `左ひざD` no mueve la falda. Hay que rotar también `左ひざ`.
- Esa regla refuerza la anterior: siempre escribir el par `X` y `XD`.

## 5. Ejemplos verificados (three 0.164.1, FK sobre el PMX)

El reposo es pose A: el brazo está 35.00° bajo la horizontal, no en pose T.
Para llegar a 70° bajo la horizontal hacen falta 35° más.

| hueso | quaternion `{x,y,z,w}` | efecto medido |
|---|---|---|
| 左腕 | `{x:0, y:0, z:-0.30071, w:0.95372}` | −35° en Z; el brazo queda a 70.00° bajo la horizontal; la muñeca va a (3.18, 11.55) |
| 右腕 | `{x:0, y:0, z:0.30071, w:0.95372}` | +35° en Z; espejo exacto |
| 左腕 desde una T real | `{x:0, y:0, z:-0.57358, w:0.81915}` | −70° en Z; sirve solo si el reposo fuera T |
| 頭 | `{x:0, y:0.17365, z:0, w:0.98481}` | +20° en Y; la cara apunta a (0.342, 0, 0.940), hacia la IZQUIERDA del personaje (+X) |
| 首 + 頭 repartido | `{x:0, y:0.08716, z:0, w:0.99619}` en cada uno | 10° + 10° = 20°, más natural |

Convención de signos:
- La izquierda del personaje es +X. Los huesos y morphs `左` están en +X (`ウィンク２` tiene x media +0.45).
- Para bajar un brazo, el izquierdo usa ángulo negativo en Z y el derecho usa ángulo positivo.
- Girar la cabeza hacia la izquierda del personaje usa +Y. Hacia la derecha usa −Y.
- La cámara frontal mira desde +Z. Desde ahí, la izquierda del personaje aparece a la DERECHA de la pantalla.
- Estos quaternions valen con los ancestros en reposo. Con el torso rotado siguen siendo locales y siguen bien.

Fragmento JSON (sin VMD):

```json
{"index":27,"name":"左腕","position":{"x":0,"y":0,"z":0},"quaternion":{"x":0,"y":0,"z":-0.30071,"w":0.95372},"scale":{"x":1,"y":1,"z":1}}
```

La posición local de reposo de `左腕` es (0,0,0), porque coincide con `左肩C`. Otros huesos tienen offset distinto de cero. Conviene partir del JSON que guarda la app y cambiar solo `quaternion`.

Clave de timeline equivalente:

```json
"timeline":{"bones":{"<uuid>":{"左腕":[{"frame":0,"position":[0,0,0],"quaternion":[0,0,-0.30071,0.95372],"scale":[1,1,1],"interp":"ease"}]}}}
```

## 6. Morphs de `onesan03` para expresión

El JSON guarda `morphs: {nombre: peso}` con las claves de `mesh.morphTargetDictionary`.
`MMDLoader` crea un morph target por cada morph, pero solo los de tipo vértice tienen efecto.
Los morphs grupo suman solo sus hijos de vértice.
Los morphs hueso, material y UV no hacen nada: `照れ` no tiene efecto, y `はぅ/星目/はぁと/なごみ/はちゅ目` funcionan solo en parte.
`morphTargetsRelative = false`. Sumar morphs que tocan los mismos vértices puede deformar la cara.

| uso | nombre exacto | tipo |
|---|---|---|
| parpadeo | `まばたき` | vértice |
| parpadeo completo | `まばたき連動` | grupo; la parte de hueso no aplica |
| guiño izquierdo / derecho | `ウィンク２` / `ｳｨﾝｸ２右` (katakana de media anchura) | vértice |
| ojos sonrientes | `笑い`, `ウィンク` (izq), `ウィンク右` | vértice |
| ojos | `びっくり`, `じと目`, `ｷﾘｯ`, `目を細める`, `瞳小`, `瞳大`, `ナチュラル` | vértice/grupo |
| boca あいうえお | `あ`, `い`, `う`, `え`, `お` (grupo; la lengua no se mueve) o `あ頂点`, `い頂点`, `う頂点`, `え頂点`, `お頂点` | grupo/vértice |
| sonrisa | `にっこり`, `にこ`, `ワ`, `口角下げ` (triste), `ん` (neutra), `一文字` | vértice/grupo |
| cejas | `にこり`, `にこり2`, `困る`, `怒り`, `驚き`, `上`, `下`, `真面目`, `ひそめ` (con variantes `右`/`左`) | vértice/grupo |
| cara completa | `喜`, `楽`, `怒`, `哀`, `驚`, `ニュートラル` (Fcl_ALL_*) | vértice |

La línea de tiempo NO tiene pistas de morphs: solo `cameras`, `objects`, `folders`, `bones` y `motions`.
Un parpadeo animado pide un VMD con pista de morph o varias escenas.

## Licencia del modelo

El comentario del PMX declara una licencia VRoid restrictiva.
- Persona: OnlyAuthor.
- Redistribución: prohibida.
- Uso comercial: Disallow.
Esa licencia limita cualquier entrega de ssb que use `onesan03`.

## A validar en la app real

1. Sin VMD, una rotación solo en `左足` no mueve la pierna. Con `左足` + `左足D` la pierna sí se mueve.
2. Una clave en `左足ＩＫ` sin VMD no mueve la pierna.
3. Los quaternions de brazos y cabeza producen la pose esperada vista desde la cámara.
4. Una escena guardada desde el UI tras rotar 左足 con el gizmo: ¿la pierna se movió en pantalla?
5. Con VMD cargado, una clave en `左足D` duplica la rotación.
6. Pelo y falda quedan quietos y no oscilan.
