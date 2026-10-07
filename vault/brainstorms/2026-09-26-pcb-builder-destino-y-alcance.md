---
title: pcb-builder — destino y alcance (grilling ticket 01)
mision: Confirmar para quien es pcb-builder y que tan lejos llega la v1.
status: seed
tags: [brainstorm, pcb-builder]
sources: []
created: 2026-09-26
---

# pcb-builder: destino y alcance
Date: 2026-09-26 · Goal: resolver el ticket 01 del mapa wayfinder (usuario objetivo, tipo de placa, etapas de la v1).

## Summary / key decisions
- Usuario: uso propio hoy, evoluciona a producto comercial. Arquitectura no bloquea multiusuario.
- Placa: multicapa (4+) desde la v1.
- Etapas v1: las cuatro, completas. Editor de esquematicos propio. Componentes: libreria KiCad + propia.
- Aceptacion v1: placa de desarrollo con MCU, 4 capas, ~50 componentes, sale fabricable.
- v2: autobuilder. Texto libre -> LLM -> esquematico -> placer -> routing -> builder, sin intervencion.

## Q&A log
### Q1 — usuario objetivo
- Asked: para quien es la v1: uso propio, makers, o producto comercial.
- Captured: "1 hacia 3". Arranca como uso propio y evoluciona hacia producto comercial.
- Implicacion: la v1 no necesita cuentas ni onboarding, pero la arquitectura no debe bloquear multiusuario.
### Q2 — tipo de placa
- Asked: 2 capas 100x100, 2 capas sin limite, o multicapa.
- Captured: multicapa (4+). Planos internos de GND y VCC desde la v1.
- Implicacion: routing y DRC deben modelar capas internas y vias desde el dia 1.

### Q3 — etapas de la v1
- Asked: las cuatro minimas, solo placer+routing, o solo schematic+placer.
- Captured: "las cuatro, completas". No minimas: cada etapa completa.
- Flag: que significa "completa" por etapa se define en tickets 07 y 08 -> usuario.

### Q4 — origen de componentes
- Asked: libreria KiCad, propia, o importar de JLCPCB/LCSC.
- Captured: KiCad + propia ("1 y 2"). Base KiCad, extension con footprints propios.
### Q5 — placa de aceptacion
- Asked: que placa prueba la v1.
- Captured: placa de desarrollo con MCU (ESP32/STM32, regulador, USB, headers), 4 capas, ~50 componentes.

### Q6 — entrada del esquematico
- Asked: editor propio, importar .kicad_sch, o DSL en codigo.
- Captured: editor propio. La etapa schematic incluye dibujar.

### Q7 — destination
- Asked: confirmar "spec ejecutable de la v1".
- Captured: si, MAS un autobuilder punta a punta "solo pidiendo la funcionalidad del componente".
- Implicacion: nueva capa arriba de las cuatro etapas. Entrada = descripcion funcional, salida = placa fabricable. Falta precisar que es "pedir la funcionalidad".
### Q8 — forma de entrada del autobuilder
- Asked: texto libre via LLM, bloques predefinidos, o LLM sobre catalogo.
- Captured: texto libre, un LLM lo traduce. Ej: "sensor de temperatura con WiFi a bateria".

### Q9 — fase del autobuilder
- Asked: v1 o v2.
- Captured: v2, sobre la v1 manual. La v1 cierra con la placa MCU dibujada a mano.

## Open flags (pending input)
- Que significa "completa" en copper guiding y builder -> usuario, tickets 07 y 08.
- Que contrato necesita el autobuilder v2 de la v1 -> usuario, ticket 09.

## Ticket 02 — vocabulario (misma sesion)
- Placer, Copper guiding, Builder: nombres del usuario, canonicos. Builder = solo emite el Fab package.
- Correccion: todo en ingles, definiciones incluidas. El usuario se arrepintio del mixto.
- Component = objeto logico con Symbol + Footprint, unidos por Reference.
- Plane = Layer interna de una sola Net, distinto de Trace.
- Design = unidad de trabajo completa. Evitar Project.
- Resultado: CONTEXT.md en la raiz del repo.

## Ticket 07 — copper guiding (misma sesion)
- Nivel: autorouter completo (freerouting) MAS ruteo asistido por Net. Edicion manual y DRC despues.
- Design Rules: perfiles por fab (JLCPCB, PCBWay) precargados y editables.
- Planes: el usuario asigna Net a cada Plane. La conexion de Pads y Vias al Plane es automatica.
- Orden acordado: 07 -> 08 -> 05. El stack depende de las dos etapas.

## Ticket 08 — builder (misma sesion)
- Fab package v1: Gerber X2 + drill + BOM + Placement file. Via kicad-cli, verificado con gerbonara.
- DRC es gate: si falla, no hay Fab package.
- Pedido a la fab: no en v1. El Autobuilder v2 puede subir a la API de la fab.

## Ticket 05 — stack (misma sesion)
- UI: web app local, luego hosteada. Editor con canvas en TypeScript.
- Nucleo: Python. Placer SA con numpy, freerouting y kicad-cli por subprocess.
- KiCad 9 instalado es requisito de runtime.
- ADR: docs/adr/0001-stack.md en el repo.

## Ticket 09 — contrato del autobuilder (misma sesion)
- Salida del LLM: Netlist JSON es el contrato. .kicad_sch se parsea a JSON. Script Python corre en sandbox y produce JSON.
- Invocacion: la misma API HTTP que usa la UI. Endpoints por etapa.
- Fallas: reporte JSON estructurado. El LLM reintenta hasta N veces y luego entrega el Design parcial.
- Verificacion: oraculos deterministas (Footprint existe, Pin en Net, ERC) MAS aprobacion humana una vez, tras la Netlist validada.

## Ticket 12 — alcance del editor (2026-09-27)
- Elementos v1: Symbols de KiCad, cables, etiquetas de Net, buses, hojas jerarquicas. Sin editor de Symbols propios (los .kicad_sym propios se escriben a mano).
- Canvas: PixiJS (WebGL, MIT). Sirve tambien para el Board.
- Persistencia: directo en .kicad_sch. Un solo formato.
- Ticket 13 creado: instalar KiCad 9 (task HITL). Bloquea el prototipo 06. Java 17 presente.
