---
title: Formatos abiertos de intercambio PCB
created: 2026-09-24
tags: [research, pcb-builder, research, formatos, kicad, gerber]
status: seed
sources: [https://dev-docs.kicad.org/en/file-formats/sexpr-schematic/, https://dev-docs.kicad.org/en/file-formats/sexpr-pcb/, https://gitlab.com/kicad/services/kicad-dev-docs, https://docs.kicad.org/9.0/en/cli/cli.html, https://dev-docs.kicad.org/en/apis-and-binding/ipc-api/, https://www.ucamco.com/en/gerber/downloads, https://www.ucamco.com/files/downloads/file_en/456/gerber-layer-format-specification-revision-2024-05_en.pdf, https://www.ucamco.com/the_xnc_file_format_specification.pdf, https://www.ipc2581.com/, https://shop.ipc.org/ipc-2581/ipc-2581-standard-only, https://odbplusplus.com/design/odb-design-format-specification/, https://github.com/mvnmgrx/kiutils, https://pypi.org/project/kiutils/, https://pypi.org/project/sexpdata/, https://github.com/jaseg/gerbonara, https://pypi.org/project/gerbonara/, https://pypi.org/project/pygerber/, https://github.com/curtacircuitos/pcb-tools, https://github.com/tracespace/tracespace, https://registry.npmjs.org/@tracespace/parser, https://github.com/ShiboSoftwareDev/kicadts, https://github.com/ulikoehler/ODBPy, https://github.com/nam20485/OdbDesign]
---
---
mision: ¿Qué formato abierto usa pcb-builder como contrato en cada etapa (esquemático, placa, fabricación, ensamblado)?
verifier: none (sin verificacion cross-family todavia)
---

## Tronco

KiCad S-expression (.kicad_sch / .kicad_pcb) como contrato interno, Gerber X2 + Excellon/XNC + .gbrjob como salida de fabricación, CSV de posiciones + BOM CSV como salida de ensamblado; IPC-2581 y ODB++ quedan fuera del núcleo.

## Qué es

Relevamiento de formatos por etapa. Fecha: 2026-09-24. Fuentes primarias: docs KiCad, Ucamco, IPC, Siemens, repos GitHub/PyPI/npm.

### Etapa 1: esquemático

**KiCad `.kicad_sch`**
- Spec: https://dev-docs.kicad.org/en/file-formats/sexpr-schematic/ (KiCad 6+).
- Apertura: la doc vive en GitLab `kicad/services/kicad-dev-docs` bajo GPLv3. KiCad es GPLv3. Nadie cobra por usar el formato.
- Texto legible: sí. S-expression UTF-8, versionado por fecha YYYYMMDD.
- Secciones: header, uuid, page, lib_symbols, junctions, wires/buses, labels, symbols, sheets, sheet_instances.

**Netlist KiCad (`.net`, s-expr)**
- No hay spec formal en dev-docs. La referencia es el código (`NETLIST_EXPORTER_KICAD`, `KICAD_NETLIST_PARSER`).
- `kicad-cli sch export netlist` emite: kicadsexpr (default), kicadxml, cadstar, orcadpcb2, spice, spicemodel, pads, allegro.
- Texto legible: sí. Estructura `(export (components ...) (nets ...))`.

### Etapa 2: placa

**KiCad `.kicad_pcb`**
- Spec: https://dev-docs.kicad.org/en/file-formats/sexpr-pcb/ (KiCad 6+, introducido en 4.0).
- Apertura: igual que el esquemático (GPLv3 docs, formato sin licencia de uso).
- Texto legible: sí. S-expression.
- Secciones: header, general, layers, setup, nets, footprints, gráficos, tracks (segment/via/arc), zones, groups.

### Etapa 3: fabricación

**Gerber RS-274X / X2 / X3 (Ucamco)**
- Spec: "Gerber Layer Format Specification". Última revisión 2026.05 (mayo 2026). Leí la 2024.05. Descarga gratis en https://www.ucamco.com/en/gerber/downloads.
- Apertura: el PDF tiene copyright Ucamco ("all rights reserved"). Ucamco no otorga licencia explícita sobre la propiedad intelectual. Pero permite implementar lectores y escritores con condiciones: no renombrar, no extender sin permiso, no asociar datos no conformes, cumplir la sección 2.12 Conformance. De hecho, es el estándar de facto que toda herramienta implementa. Riesgo legal práctico: bajo.
- Texto legible: sí. UTF-8, 27 comandos.
- X2 = atributos (TF/TA/TO) sobre RS-274X. Rev J1 (2014). X3 = capas de componentes (`.C`, `.Cxxx`, sección 6.9 "Component Data"). Rev 2020.09.
- Gerber Job File `.gbrjob`: JSON. Spec rev 2020.08 + schema JSON (2023). Lleva stackup, reglas y lista de archivos.

**Excellon / XNC**
- Excellon original: sin spec pública clara. Ambigüedades conocidas: sin punto decimal, sin unidad.
- XNC: subconjunto estricto de IPC-NC-349. Lo definen Ucamco + KiCad + Pentalogix. Spec gratis: https://www.ucamco.com/the_xnc_file_format_specification.pdf (rev 2021.11).
- Texto legible: sí. Header M48...%, cuerpo con T y X/Y.
- KiCad exporta Excellon o taladros en Gerber X2.

**IPC-2581**
- Spec: IPC-2581C (XML). Se compra en https://shop.ipc.org/ipc-2581/ipc-2581-standard-only. El consorcio (ipc2581.com) ofrece viewers y test cases gratis. Hay XSD en repos de terceros (ej. Chentai-Kao/ipc2581_to_odb), no confirmé descarga oficial gratis del XSD C.
- Apertura: estándar "abierto y neutral" según el consorcio. El documento es pago. Sin declaración de royalty-free encontrada.
- Texto legible: sí (XML, archivo único, grande).
- KiCad exporta IPC-2581 desde la v8 (`kicad-cli pcb export ipc2581`).

**ODB++**
- Spec: ODB++Design 8.1 Update 4 (ago 2024), https://odbplusplus.com. Descarga gratis con registro.
- Apertura: propiedad de Siemens/Mentor. El sitio dice que la descarga NO otorga licencia para desarrollar software. Requiere licencia "Solutions Development Partner". Formato controlado por un vendor.
- Texto legible: parcial. Directorio comprimido (.tgz) con archivos de texto propios.
- KiCad exporta ODB++ desde la v9 (`kicad-cli pcb export odb`), basado en spec 8.1.

### Etapa 4: ensamblado

**BOM**
- Sin estándar único. KiCad 8+/9: `kicad-cli sch export bom` emite CSV con campos configurables. `python-bom` emite XML intermedio (legacy).
- Texto legible: sí.

**Pick-and-place (posiciones)**
- Sin estándar único. KiCad: `kicad-cli pcb export pos` con formatos ascii, csv, gerber (X3). Campos: Ref, Val, Package, PosX, PosY, Rot, Side.
- Gerber X3 es la única variante estandarizada. Los fabricantes (JLC, PCBWay) siguen pidiendo CSV con sus propias cabeceras.
- Texto legible: sí.

## Parsers disponibles (verificado 2026-09-24)

| Formato | Lib | Lenguaje | Licencia | Estado |
|---|---|---|---|---|
| .kicad_sch/.kicad_pcb/.kicad_mod/.kicad_sym | kiutils (mvnmgrx) | Python | GPL-3.0 | Último release 1.4.8, feb 2024. 10 issues abiertos. Sin release en 2.5 años. Riesgo con KiCad 9. |
| S-expr genérico | sexpdata | Python | BSD-2 | 1.0.2, ene 2024. Estable, mínimo. |
| KiCad vivo | IPC API (protobuf + NNG) | Python | GPLv3 | Estable desde KiCad 9. Necesita KiCad corriendo. |
| .kicad_* | kicadts (tscircuit) | TypeScript | MIT | 0.0.58, activo en 2025. Genera y parsea. |
| .kicad_* | kicad-to-json, kicad-utils (cho45) | TS/JS | varias | Menor mantenimiento. |
| Gerber + Excellon + IPC-356 | gerbonara (jaseg) | Python | Apache-2.0 | 1.6.3, abr 2026. Activo. Lee y escribe. |
| Gerber X2/X3 | PyGerber | Python | MIT | 2.4.3, mar 2025. Basado en spec 2023.03. Sin Excellon. |
| Gerber + Excellon | pcb-tools | Python | Apache-2.0 | Archivado jun 2024. No usar. |
| Gerber + drill | @tracespace/parser | TS | MIT | 5.0.0-next, ene 2023. Autor declara "hiato indefinido". |
| ODB++ | ODBPy (ulikoehler) | Python | Apache-2.0 | Alpha declarado. 42 stars. |
| ODB++ | OdbDesign (nam20485) | C++ | abierto | Activo, con REST/gRPC. |
| IPC-2581 | ninguno en Python/JS maduro | — | — | XML: se parsea con lxml + XSD. |

## Evidencia / mecanismo

- Gerber sigue siendo lo que todo fabricante acepta. Ucamco lo dice en el prefacio y la práctica lo confirma.
- Los formatos "únicos" (IPC-2581, ODB++) tienen barrera: spec paga (IPC) o licencia de vendor (Siemens). Sus parsers abiertos son inmaduros.
- KiCad cubre todo el pipeline con formatos texto y una CLI. Reutilizar sus formatos permite abrir cualquier resultado en KiCad y usar KiCad como oráculo de comparación.
- La CLI `kicad-cli` genera Gerber, Excellon, pos, BOM, IPC-2581 y ODB++ desde un `.kicad_pcb`. Eso permite delegar la salida de fabricación sin escribir un writer propio.

## Aplicable a mmorch

- Checker de fabricación: `kicad-cli pcb export gerbers` sobre el `.kicad_pcb` generado = oráculo. Comparar contra el Gerber del builder propio con gerbonara.
- Checker de placa: parsear `.kicad_pcb` con kiutils o sexpdata y validar nets/DRC con `kicad-cli pcb drc`.
- No usar LLM para validar formatos. Todo es checkeable.

## Objeciones

- kiutils lleva 2.5 años sin release. Puede romper con `.kicad_pcb` de KiCad 9. Falta el dato: probar kiutils sobre un archivo real de KiCad 9.
- Licencia GPL-3.0 de kiutils contagia si pcb-builder se distribuye. Alternativa: sexpdata (BSD) + modelo propio.
- No verifiqué la licencia exacta del texto del spec Gerber 2026.05, solo la 2024.05. Asumo igual.
- No confirmé si el XSD de IPC-2581C está gratis en un sitio oficial.

## Veredicto cross-family
- pendiente. Sin verificacion todavia.

## Recomendación

1. Contrato esquemático: `.kicad_sch` + netlist `kicadsexpr`.
2. Contrato placa: `.kicad_pcb`.
3. Contrato fabricación: Gerber X2 (una capa por archivo) + Excellon/XNC + `.gbrjob`. Generar con `kicad-cli`. Verificar con gerbonara.
4. Contrato ensamblado: CSV de posiciones (Ref, Val, Package, PosX, PosY, Rot, Side) + BOM CSV. Gerber X3 opcional.
5. IPC-2581 y ODB++: solo como export opcional vía `kicad-cli`. No parsear.
6. Parsers: Python con sexpdata (BSD) o kiutils (GPL, validar contra KiCad 9); Gerber con gerbonara.

## Links
- [[pcb-builder]]
