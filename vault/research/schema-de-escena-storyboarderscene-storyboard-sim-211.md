---
title: Schema de escena storyboarder.scene (Storyboard Sim 2.1.1)
created: 2026-09-27
tags: [research, ssb, research, storyboard-sim, reverse-engineering, schema]
status: seed
confidence: 0.85
---
mision: Documentar el schema completo del JSON `storyboarder.scene` de Storyboard Sim 2.1.1, con tipos, defaults, rangos y campos obligatorios.

## Tronco

La escena es un snapshot de `Un()` con `formatVersion: 2`; el loader `r5()` + `n5()` + `Pl()` completa casi todo con defaults, y solo cámaras (position/rotation/target), modelos (modelPath) y carpetas (name) tienen campos obligatorios de facto.

## Qué es

- Fuente: lectura estática del bundle minificado. No hay ejecución.
- Renderer: `resources/app/dist/assets/index-BRNPQ79Q.js` (1.6 MB, una línea).
- Main: `resources/app/dist-electron/main.js`.
- Los offsets `@N` son posiciones de carácter en el bundle. Sirven para re-verificar con un script de ventana.
- El módulo de estado se expone como `window.__storyboardSim.StateManager` (`zc`). Sus exports son `captureState`=`Un`, `restoreState`=`H9`, `exportStateAsJson`=`G9`, `applyBoneStates`=`Ms`, `applyModelParts`=`l5`.

## Evidencia / mecanismo

### Mapa de funciones minificadas

| Función | Offset | Rol |
|---|---|---|
| `Un(r)` | ~776900 | Serializador (`captureState`). Arma el objeto raíz. |
| `c6(r)` | — | `Un()` + `savedAt` → `JSON.stringify`. Lo usa guardar PNG y PNG por capas. |
| `G9(r)` | — | `JSON.stringify(Un(r))` sin `savedAt`. |
| `KE(r,e)` | ~1555700 | Export de vídeo. Embebe `{...Un(r), savedAt}` si `render.embedProject`. |
| `D9(bytes)` | @772389 | Lee el chunk `tEXt` con clave `E9="storyboarder.scene"` y decodifica con `T9` (atob → UTF-8). |
| `r5(json)` | @772820 | Normalizador/migrador de la raíz. Aplica defaults. |
| `n5(tl)` | @767544 | Normalizador de la línea de tiempo. Default en `Bp()`. |
| `Pl(toon)` | @676432 | Normalizador toon v2 con clamps `Ct`/`zi`/`Zs`. Default en `fd()` @674463. |
| `ax(toon)` | ~675500 | Migrador de toon legacy (sin `version:2`). |
| `_(json,...)` (loadScene) | ~1489000 | Orquesta la carga: `r5` → `aE` (resuelve paths) → carpetas → `yg` por modelo → `H9`. |
| `aE(scene,base)` / `oE` | @1479737 | Resuelve refs de archivo vía IPC `resolveFileRefs`. |
| `yg(model,...)` | @1495060 | Carga cada modelo. Devuelve false si falta `modelPath`. |
| `Mo(obj,model)` | @1494798 | Aplica uuid, transform, folder, lock, visible, toon.lineSet. |
| `H9(scene,ctx,...)` | ~779000 | `restoreState`: luces, HDRI, timeline, render, carpetas (`V9`), cámaras, estado de modelos. |
| `VE(...)` / `createFromFile` | ~1519500 | Media manager. Defaults de planos de imagen/vídeo. |
| `_g()` | ~1514500 | Defaults de `media.video`. |
| `mS()` | @889592 | Defaults de `media.turntable`. |
| `md()` | @772498 | Defaults de `render`. |
| `s5()` | ~772700 | Defaults de `hdri`. |
| `e5`/`h9`/`y9`/`Dl` | ~765000 | Deformador de cabeza anime (solo `.pmx`). |
| `G1`/`Zo`/`ix`/`sx` | ~672800 | Apuntar cámara a objeto (`userData.aim`). |
| `GE(...)` | @1528715 | Controlador de timeline. Escribe las claves. |
| `A9(interp,t)` | ~768000 | Curvas: `step`→0, `ease`→smoothstep, otro→lineal. |
| main `$e`, `Ve`, `kt`, `Ht`, `Dt`, `Jt` | main.js | Inserción del chunk PNG, paths relativos, caja MP4 `uuid`, lectura desde vídeo. |

### Contenedor

- **PNG**: chunk `tEXt`, keyword `storyboarder.scene`, texto = base64 de los bytes UTF-8 del JSON (`_t` en main). Main quita cualquier chunk previo con la misma clave y lo inserta antes de `IEND` (`$e`). Antes de escribir, `Ve()` recalcula `relativePath` de cada ref de archivo respecto a la carpeta del PNG.
- **MP4/MOV** (`mp4-h264`, `mov-*`): main agrega al final del archivo una caja top-level `uuid` (`kt`). Layout: `u32 size` + `"uuid"` + UUID de 16 bytes `53 42 53 49 4D 2D 50 52 4F 4A 7A 3E 91 0C 5D A7` + `"SBSIM-PROJECT\0"` + `u32 version=1` + JSON UTF-8 **plano, sin base64**. El lector `Dt`/`Jt` acepta `.mp4/.m4v/.mov`.
- **Secuencia PNG**: solo el primer frame (`_00000.png`) lleva el chunk `tEXt`.
- **WebM**: no lleva proyecto (`Ht` no lo contempla).
- **JSON suelto**: "Save Scene (JSON)" usa el mismo objeto.

### Schema (TypeScript anotado)

Convenciones de tipos serializados por three.js:
- `Vec3Obj = {x:number,y:number,z:number}` (Vector3 sin `toJSON`).
- `EulerJSON = {isEuler:true,_x:number,_y:number,_z:number,_order:"XYZ"|...}` (Euler sin `toJSON`, se serializan campos propios). El loader (`bu`/`s6`) también acepta `{x,y,z,order?}` o `[x,y,z,order?]`.
- Vectores en claves de timeline: arrays `[x,y,z]`. Cuaterniones en claves: `[x,y,z,w]`.
- `FileRef = {path:string, relativePath:string|null, fileName:string}` (`Dn`). Resolución (`Yt` en main): `relativePath` relativo a la carpeta del proyecto → `path` absoluto → `fileName` en la carpeta del proyecto.

```ts
interface StoryboarderScene {
  formatVersion?: number;        // escribe 2 (F9). Default 1. Ningún código ramifica por versión.
  app?: "Storyboard Sim";        // informativo
  savedAt?: string;              // ISO. Solo en PNG/vídeo (c6, KE)
  toonSettings?: ToonSettings;   // ver abajo. Ausente → fd(). Objeto sin version:2 → migrador legacy ax()
  cameras?: Camera[];            // default []. En la práctica hace falta ≥1 (ver Objeciones)
  activeCameraName?: string|null;// default null → primera cámara
  lighting?: Lighting;
  hdri?: Hdri;
  timeline?: Timeline;           // ausente/no-objeto → Bp()
  render?: RenderSettings;       // merge {...md(), ...render}, sin validar
  models?: Model[];              // default []. Filtra no-objetos
  selectedMeshUUID?: string|null;// default null
  folders?: Folder[];            // default []. Filtra los que no tienen name
}

interface Camera {
  id?: string;          // = uuid de la cámara. Si falta, se genera uno y se pierden sus pistas y cortes
  name: string;         // clave de activeCameraName. UI rechaza duplicados
  position: Vec3Obj;    // OBLIGATORIO (rs() lanza TypeError si falta)
  rotation: EulerJSON;  // OBLIGATORIO (bu() lanza TypeError si falta). Es lo que se aplica al cargar
  quaternion?: {x,y,z,w}; // se escribe, el loader lo IGNORA
  near?: number;        // default 0.1
  far?: number;         // default 2000
  focalLength?: number; // mm. UI 10..200 paso 1. Falsy → fov 50° por defecto. Ver nota de aspect
  target: Vec3Obj;      // OBLIGATORIO (O9() lanza si falta). Punto de órbita
  aim?: {               // "Aim at Object". Solo se restaura si aim.objectId es truthy
    objectId: string;   // uuid del modelo
    anchor: Vec3Obj;    // punto mirado, en espacio local del objeto (centro del bbox)
    offset: Vec3Obj;    // posición de cámara en el espacio del objeto (sin escala)
    path?: "orbit"|"linear"; // interpolación entre claves. Default "orbit"
  };
}

interface Lighting {            // todos con default en r5; sin clamp
  ambient?: number;             // default 1.5. UI 0..2 paso 0.1
  directional?: number;         // default 1. UI 0..2
  directionalPos?: Vec3Obj;     // default {1,1,1}
  hdriRotation?: number;        // rad. default 0 (legacy)
  hdriPath?: string|null;       // legacy. default null
  hdriEnabled?: boolean;        // legacy. Sin default
}

interface Hdri {                // merge {...s5(), ...hdri}
  enabled: boolean;             // default false. Si falta `hdri`, se deriva de lighting.hdriEnabled
  showBackground: boolean;      // default false
  file: FileRef|null;           // default null. Si falta, se toma de lighting.hdriPath
  rotation: number;             // rad, default 0. UI 0..2π
  intensity: number;            // default 1. UI 0..4
  backgroundIntensity: number;  // default 1. UI 0..4
  backgroundBlurriness: number; // default 0. UI 0..1
}

interface Model {
  uuid?: string;               // default randomUUID (L9). Clave de timeline.objects/bones/motions
  name?: string;               // default "Model". MMD/legend puede renombrar; H9 reaplica name
  modelPath?: string;          // OBLIGATORIO de facto: yg() devuelve false sin él → modelo "missing".
                               // aE() lo rellena si files.model o media.file se resuelven en disco
  vmdPath?: string; vpdPath?: string; // solo MMD. Se cargan tras el modelo
  isMMD?: boolean;             // si falta, se infiere por /\.(pmx|pmd)$/
  visible?: boolean;           // default true
  lock?: boolean;              // default false
  folder?: string|null;        // nombre de carpeta. default null. Crea la carpeta si no existe (V9)
  position?: Vec3Obj;          // default {0,0,0}
  rotation?: EulerJSON;        // default {x:0,y:0,z:0}
  scale?: Vec3Obj;             // default {1,1,1}
  category?: string;           // default "background" (o model_legend.json)
  morphs?: Record<string,number>; // solo MMD. nombre de morph → influencia (0..1 típico, sin clamp)
  bones?: { index:number; name:string; position:Vec3Obj;
            quaternion:{x,y,z,w}; scale:Vec3Obj }[]; // pose. Match por index+name, luego por name (Ms)
  parts?: { path:number[]; position:Vec3Obj; rotation:EulerJSON;
            scale:Vec3Obj; visible:boolean }[]; // solo FBX/GLB/GLTF no-media. path = índices de children
  animeHeadDeformer?: {        // solo .pmx (pd). Normaliza h9
    enabled:boolean; amount:number /*0..100, def 75*/; falloff:number /*0..100, def 15*/;
    boneName:string; center:Vec3Obj; radius:Vec3Obj /*cada eje ≥0.001*/ };
  toon?: { lineSet:number };   // 0 = sin líneas, 1..8 = set. Ausente → 1
  media?: Media;               // presente ⇒ plano de imagen/vídeo
  files?: { model?:FileRef; vmd?:FileRef; vpd?:FileRef; textures?:FileRef[] };
                               // r5 lo completa desde modelPath/vmdPath/vpdPath. textures es informativo
  missing?: true;              // modelos que faltaban al guardar. Se reintentan al cargar
}
// Tipos de modelo (yg): .pmx/.pmd (MMD), .fbx, .ply (incl. Gaussian splat), resto → GLTFLoader (.glb/.gltf).

interface Media {              // createFromFile: {defaults, ...media guardado}; width/height/frame* se re-sondean
  kind: "image"|"video";       // el guardado PISA al inferido por extensión
  file: FileRef;               // path se fuerza a modelPath
  width:number; height:number; // se recalculan al cargar
  alphaMode: "auto"|"straight"|"premultiplied"|"opaque"; // default "auto"
  hasAlpha?: boolean; codec?: string; frameRate?: number; frameCount?: number;
  opacity: number;             // default 1. Clamp 0..1 al render. UI 0..1
  planeHeight: number;         // default 20. ≤0 → 20. UI 0.1..500. Ancho = alto × width/height
  doubleSided: boolean;        // default true
  layer: number;               // default 0. renderOrder. UI -100..100 entero
  followCamera?: string|null;  // null | "@render" (cámara activa) | uuid de cámara. Anula folder
  video?: {                    // solo kind "video". Defaults _g()
    startFrame:number /*0; UI ±1e5*/; trimIn:number /*0; 0..frameCount-1*/;
    speed:number /*1; UI 0.05..8*/; loop:boolean /*false*/; holdFrames:boolean /*true*/ };
  turntable?: {                // solo vídeo. Defaults mS()
    enabled:boolean /*false*/; frontFrame:number /*0*/; frames:number /*0 = todos*/;
    reverse:boolean /*false*/; billboard:boolean /*true*/; range:number /*360; UI 1..360 grados*/ };
}

interface Folder {
  name: string;               // OBLIGATORIO (se filtra sin él). Clave de timeline.folders
  visible?: boolean;          // default true
  position?: Vec3Obj; rotation?: EulerJSON; scale?: Vec3Obj; // opcionales, transform del grupo
}

interface Timeline {           // n5(); defaults Bp()
  version?: 1;                 // siempre se reescribe a 1
  fps: number;                 // default 24. ≤0 o no finito → 24. UI presets 12,15,23.976,24,25,29.97,30,48,50,60
  start: number;               // default 0. Math.round
  end: number;                 // default 143. Si end<start → end=start
  currentFrame: number;        // default 0
  loopPlayback: boolean;       // default true (solo false explícito lo apaga)
  cameraCuts: {frame:number; cameraId:string}[]; // cameraId = Camera.id. Se filtran sin cameraId string. Se ordenan
  cameras: Record<cameraUuid, CameraKey[]>;
  objects: Record<modelUuid, { transform?:TransformKey[]; visible?:BoolKey[]; opacity?:NumKey[] }>;
  folders: Record<folderName, { transform?:TransformKey[]; visible?:BoolKey[] }>;
  bones:   Record<modelUuid, Record<boneName, TransformKey[]>>;
  motions: Record<modelUuid, { startFrame:number /*0*/; speed:number /*1*/;
            enabled:boolean; clip:string|null; loop:boolean /*false*/ }>; // VMD mixer o clips FBX/GLTF
}
type Interp = "linear"|"ease"|"step"; // default "linear". UI: Linear/Ease/Step. No existe "smooth"
interface TransformKey { frame:number; position:[n,n,n]; quaternion:[n,n,n,n]; scale:[n,n,n]; interp?:Interp }
interface CameraKey { frame:number; position:[n,n,n]; quaternion:[n,n,n,n]; target:[n,n,n];
                      focalLength:number; interp?:Interp; aimOffset?:[n,n,n] }
interface BoolKey { frame:number; value:boolean }          // sin interp: escalón
interface NumKey  { frame:number; value:number; interp?:Interp } // opacidad de media
// n5 solo ordena por frame; NO valida el contenido de las claves.

interface RenderSettings {     // md(); merge sin validar
  format: "mp4-h264"|"mov-prores422"|"mov-prores4444"|"mov-qtrle"|"webm-vp9"|"png-sequence"; // default mp4-h264
  width: number;  height: number;       // 1920×1080. UI 16..8192 paso 2. Export fuerza ≥2
  transparentBackground: boolean;       // false. Solo aplica a formatos con alpha
  embedProject: boolean;                // true
  useCameraCuts: boolean;               // true
  quality: "high"|"draft";              // "high"
  renderElements: boolean;              // false (pases por separado)
}
// El rango y fps del export salen de timeline.start/end/fps.

interface ToonSettings {       // Pl(); Ct(v,def,min,max), zi(bool), Zs(#rrggbb)
  version: 2;                  // OBLIGATORIO para no pasar por el migrador legacy
  enabled: boolean;            // false
  zones: { position:number /*0..1; la 1ª se fuerza a 0*/; color:"#rrggbb";
           feather:number /*0..0.5, def .01*/; replace:boolean }[]; // máx 8, se ordenan. Default 2 zonas (#7d7a9c@0, #ffffff@.45)
  shift:number /*-1..1, 0*/; lightColorInfluence:number /*0..1, 0*/;
  subLightStrength:number /*0..4, .35*/; ambientInfluence:number /*0..2, 0*/;
  castShadows:boolean /*true*/; shadowRate:number /*0..1, 1*/;
  castShadowColorEnabled:boolean /*false*/; castShadowColor:"#rrggbb" /*#6a6488*/;
  highlight:{ enabled /*false*/; color /*#ffffff*/; size /*0..1, .25*/; softness /*0...5, .05*/; strength /*0..4, .6*/ };
  rim:{ enabled /*false*/; color /*#ffffff*/; width /*0..1, .25*/; softness /*0...5, .03*/; strength /*0..4, .4*/;
        mask:"all"|"lit"|"shadow" /*lit*/ };
  lines:{ enabled /*true*/; size /*0..32, 1.5*/; sizeMode:"relative"|"absolute"; opacity /*0..1, 1*/;
          normalAngle /*1..179, 40*/; intersectionTolerance /*0..0.2, .004*/; innerThreshold /*1e-4..1, .02*/;
          depthThreshold /*0..1, .01*/; attenuation:{ enabled /*false*/; near /*≥0, 20*/; far /*≥0, 300*/; minScale /*0..1, .4*/ };
          vertexColorWidth /*false*/;
          sets: { enabled /*true*/; name /*"Set N"*/; size /*0..20, 1*/;
                  edges: Record<"outline"|"intersection"|"material"|"normal"|"inner",
                                { enabled:boolean; color:"#rrggbb" /*#1a1822*/; size:number /*0..20*/ }> }[] }; // siempre 8 sets
  debugView: "none"|"lighting"|"zones"|"normal"|"depth"|"objectId"|"edges";
}
// Legacy (sin version:2) lee: bands(2..6,3), threshold(.1...9,.5), softness(0...15,.02), shadowColor,
// rimStrength(0..2,.25), rimColor, rimPower(1..12,4), outlineWidth(0..0.2,.02), outlineColor. Fuerza castShadows=false.
```

### Obligatorio vs default

- **Raíz**: solo debe ser un objeto JSON válido. Todo lo demás tiene default en `r5`.
- **Cámara**: `position`, `rotation` y `target` son obligatorios. Sin ellos, `H9` lanza TypeError y la carga aborta. `id` es obligatorio en la práctica si hay timeline o cortes.
- **Modelo**: `modelPath` es obligatorio en la práctica. Sin él (o sin un `files.model` resoluble), el modelo pasa a "missing files". El resto tiene default.
- **Media**: `media.kind` y `modelPath` (ruta al archivo). El resto sale de `createFromFile`.
- **Carpeta**: `name`.
- **Claves de timeline**: `n5` no las valida. Una `CameraKey` sin `position/quaternion/target/focalLength` rompe la evaluación (`No`, `i5`, `Wa` con undefined).
- **Toon**: si se incluye, debe llevar `version: 2`. Sin ese campo se interpreta como legacy.

### Nota de cámara: focal en mm

- `focalLength` usa `PerspectiveCamera.getFocalLength()` con `filmGauge = 35`.
- La altura de film es `35 / max(aspect, 1)`. El valor en mm depende del aspect de la ventana al guardar.
- Al cargar, `setFocalLength` usa el aspect de la ventana actual. El FOV vertical cambia si el aspect cambia.
- La cámara nueva por defecto está en `(0,20,50)`, mira a `(0,0,0)` y tiene fov 50°.

### Ejemplo mínimo válido

```json
{
  "formatVersion": 2,
  "app": "Storyboard Sim",
  "cameras": [{
    "id": "cam-1", "name": "Camera 1",
    "position": {"x": 0, "y": 20, "z": 50},
    "rotation": {"isEuler": true, "_x": -0.38, "_y": 0, "_z": 0, "_order": "XYZ"},
    "near": 0.1, "far": 2000, "focalLength": 35,
    "target": {"x": 0, "y": 0, "z": 0}
  }],
  "activeCameraName": "Camera 1",
  "models": [{
    "uuid": "m-1", "name": "Ref",
    "modelPath": "C:/proj/ref.png",
    "position": {"x": 0, "y": 10, "z": 0},
    "rotation": {"x": 0, "y": 0, "z": 0},
    "scale": {"x": 1, "y": 1, "z": 1},
    "media": {"kind": "image", "opacity": 1, "planeHeight": 20, "doubleSided": true, "layer": 0, "alphaMode": "auto"}
  }],
  "folders": [],
  "timeline": {"fps": 24, "start": 0, "end": 47, "cameraCuts": [{"frame": 0, "cameraId": "cam-1"}],
    "cameras": {"cam-1": [
      {"frame": 0,  "position": [0,20,50], "quaternion": [-0.19,0,0,0.98], "target": [0,0,0], "focalLength": 35, "interp": "ease"},
      {"frame": 47, "position": [20,20,40], "quaternion": [-0.19,0.2,0.04,0.96], "target": [0,0,0], "focalLength": 50, "interp": "linear"}
    ]}}
}
```

Para el PNG: `base64(utf8(JSON))` en un chunk `tEXt` con keyword `storyboarder.scene`, antes de `IEND`.

## Aplicable a mmorch

- No aplica al ruteo. Es research de formato para el proyecto ssb.
- El schema es checkeable con un oráculo: un validador que replique `r5`/`n5`/`Pl`, más un round-trip en la app real. No hace falta un LLM-juez.

## Objeciones

- La nota sale de lectura estática de código minificado. Ningún PNG real se decodificó todavía.
- Una escena con `cameras: []` vacía `ctx.cameras` y no activa ninguna cámara. El efecto en el render no se verificó.
- `hdri.file` gana sobre `lighting.hdriPath` al cargar. El orden exacto con archivos legacy solo se infiere.
- Los valores de `morphs` no se clampan. El rango real depende del modelo.
- La forma de `rotation` en PNG reales (`_x/_order` frente a `x/order`) sale de three.js. Falta confirmarla con un archivo real.

## Veredicto cross-family

- No verificado. Pendiente: validar con PNGs y MP4 reales del usuario y un round-trip en la app.

## Links

- Ticket: `C:/Users/map12/Desktop/Proyectos/ssb/.scratch/ssb/issues/02-schema-de-escena.md`
