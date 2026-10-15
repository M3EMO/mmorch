# Builder de punta a punta: Brainstorm / Discovery Notes
Date: 2026-10-14 · Goal: responder el ticket 01 del mapa `.scratch/builder-e2e/` (destino y alcance del builder con oráculo agente).

Contexto de partida:

- Mapa: `.scratch/builder-e2e/map.md`. El destino es provisorio hasta este grilling.
- Ticket 02 resuelto: el SDLC sirve del módulo 2 en adelante; faltan el bootstrap del repo vacío, la descomposición y el encadenado de módulos.
- El modo contenedor ya existe (podman-docker en Linux, port a Windows sin probar) y sirve para cualquier lenguaje.
- El usuario usa mucho Rust, Java y TypeScript.
- Cartera de ingresos (`~/Proyectos/2026-08-25-proyectos-ingresos.md`): F1 newsletter y bot de alertas, F2 sitios SEO de calculadoras, M2 agentes B2B (ChatBot), M1 e-commerce. Reglas: soporte humano ~cero, 5-10 h/semana, tope US$15/mes por apuesta.

## Summary / key decisions

- Propósito: herramienta GENERAL que arma entregables (productos, servicios, flujos, funcionalidades), medida como banco de mmorch (éxito, costo y horas del usuario por entregable). La cartera de ingresos es la primera fuente de entregables, no el límite.
- Primer producto: F2, calculadora de sueldo neto (repo vacío; 3 calculadoras + layout = 4 módulos; oráculo computable con casos de neto conocido; juicio acotado a texto SEO y aspecto visual).
- Fin del builder: el sitio construido y verificado por el agente en un preview local, dentro del contenedor. Publicar (hosting, dominio, AdSense) lo hace OTRO agente, aparte.
- Veredicto: capa computable que bloquea (casos de neto, agente que maneja el preview, consola sin errores, Lighthouse, metadatos) + capa de juicio en sombra. El usuario quiere enriquecer el juicio con rúbricas para hacerlo más consistente y comparable entre productos.
- Rúbrica valor × esfuerzo × riesgo: ORDENA módulos en la descomposición (rebanada vertical primero), no aprueba trabajo. Esfuerzo = `_tamano()` del SDLC; riesgo = lectores acoplados del informe de impacto; valor = juicio (rúbrica o usuario). Banco: esfuerzo estimado contra real (USD, minutos, escalones del fix ladder).
- Capa de juicio: rúbricas con ejemplos ancla y comparaciones de a pares entre productos; el juez ordena y no puntúa (medido 2026-09-17); siempre en sombra hasta medirlas.
- Humano: dos paradas por producto, menos de 1 h en total. Parada 1 = walkthrough tipo presentación tras el plan y el boceto (qué agrega y cómo, todo entendible); el usuario pregunta, revisa, corrige y da oks. Incluye aprobar módulos, orden y tabla de casos del oráculo. Parada 2 = revisar el preview verificado antes del agente de publicación. Sin aprobación por módulo; humano solo si el fix ladder se agota.
- Walkthrough: Artifact privado de claude.ai con comentarios por sección y un ok por sección. Secciones: idea en una oración, boceto (pantallas o flujo), módulos ordenados con valor/esfuerzo/riesgo, tabla de casos del oráculo con fuente, cómo verifica el agente cada módulo, costo estimado (USD y cupo). Entendible = qué y por qué, sin código, términos explicados, ETS. El builder arranca con todos los oks.
- Alcance: el builder no arma solo productos; también servicios, flujos, funcionalidades y otras cosas. F2 es solo el primer caso.
- Presupuesto: APIs US$5 por entregable (tope nuevo que suma sus módulos; usd_max por módulo sigue en US$3). Cupo de Claude: más que walkthrough + revisión 5b + escaladas.
- Roles de Claude: Opus descompone en módulos y arma el walkthrough; Sonnet planea cada módulo (medido contra el reasoner); Claude con navegador maneja el preview como oráculo agente. Coder sigue en DeepSeek v4 pro; juez en sombra en DeepSeek reasoner.
- Coder: Haiku 5.5 o DeepSeek v4 pro por sorteo por módulo (mitad y mitad, semilla fija, registrado en el run-log con fricción, éxito y costo en USD y cupo).
- Destino del mapa: el builder arma la calculadora de sueldo neto de punta a punta con (1) capa computable verde, (2) solo las dos paradas y menos de 1 h del usuario, (3) APIs < US$5, (4) run-log completo para el banco (coder sorteado, fricción, USD, cupo, horas), (5) opinión del juez en sombra con rúbricas. Un segundo entregable de otro tipo es destino del mapa siguiente.
- Reglas generales (no solo F2): (a) un entregable nuevo vive en un repo privado nuevo del GitHub del usuario, creado con su ok; una funcionalidad sobre algo existente vive en su repo; (b) el stack lo propone Opus en el walkthrough y el usuario lo aprueba ahí; (c) la tabla de casos del oráculo la arma el agente desde fuentes primarias (para F2, las tablas de ARCA) y el usuario la aprueba en el walkthrough.

## Q&A log

### Q1 — Propósito del builder
- Asked: ¿fábrica de productos (apuestas de ingresos) o banco de medición de mmorch?
- Captured: "fábrica de productos medida como banco". El builder arma productos reales de la cartera; cada producto deja datos para mmorch (éxito, costo, horas del usuario). No hay tarea sintética aparte.
- Flags: ninguno.

### Q2 — Primer producto
- Asked: ¿cuál es el primer producto que arma el builder?
- Captured: "F2 con sueldo neto". Motivos aceptados: arranca de repo vacío (hueco del ticket 02), módulos naturales, oráculo computable, juicio acotado. F1 queda para después (marca sin definir y datos de prueba en portfolio.db).
- Flags: ninguno.

### Q3 — Dónde termina el builder
- Asked: ¿repo con tests verdes, sitio verificado en preview local, o publicado con dominio y AdSense?
- Captured: "2, pero armaríamos otro agente para el paso 3". El builder termina en el preview local verificado. La publicación es un agente separado.
- Flags: el agente de publicación toca zona roja de GOAL.md (cuentas, dominio, dinero): necesita gate humano y su propio diseño -> usuario, como esfuerzo aparte del builder.

### Q4 — Juicio contra cómputo
- Asked: ¿la capa que bloquea es solo computable (casos de neto, agente en el preview, consola, Lighthouse, metadatos) y el juicio (texto SEO, aspecto visual) queda en sombra?
- Captured: sin objeción a la división. Propuesta del usuario: "agregarle más funcionalidades al juicio agéntico y armarlos a través de rúbricas para darle un sentido más objetivo y tener comparaciones internas".
- Captured: pregunta del usuario: ¿sirve una rúbrica valor × esfuerzo × riesgo (como la figura de un reporte de agente sobre bugs y tareas) para el trabajo del SDLC?
- Flags: ninguno (resuelto en Q4b).

### Q4b — Rúbricas y valor × esfuerzo × riesgo
- Asked: ¿rúbrica valor × esfuerzo × riesgo para ordenar módulos (esfuerzo y riesgo calculados), y rúbricas con anclas y comparaciones de a pares en la capa de juicio, en sombra?
- Captured: "confirmo las dos".
- Flags: ninguno.

### Q5 — Intervención humana
- Asked: ¿dos paradas por producto (arranque: módulos, orden y tabla de casos; final: preview verificado), menos de 1 h en total?
- Captured: "confirmo las dos paradas y sumo algo más. Después del armado del plan y bocetaje el agente hace un walkthrough de lo que quiere agregar y cómo va a hacer, como una presentación. Tiene que todo ser entendible. Y ahí el humano (yo) mete preguntas, revisiones, correcciones y oks."
- Captured: la parada 1 es ese walkthrough; llega después del plan y del boceto.
- Flags: `human_interventions` del SDLC nunca se incrementa (ticket 02): contarlo de verdad para medir horas del usuario -> builder.

### Q6 — Forma del walkthrough
- Asked: ¿página web como Artifact privado, comentarios por sección, ok por sección, seis secciones?
- Captured: "confirmo el walkthrough como Artifact". Agregado: "también pueden ser no solo productos sino servicios, flujos, funcionalidades, y otras cosas".
- Captured: consecuencia: las secciones del walkthrough y el tipo de oráculo dependen de qué se arma (pantallas en un sitio, pasos en un flujo, contrato en un servicio). Eso alimenta el ticket 03 (evidencia por tipo).
- Flags: definir los tipos de entregable y su oráculo -> ticket 03 del mapa.

### Q7 — Presupuesto
- Asked: ¿APIs US$5 por entregable; cupo de Claude solo en walkthrough, revisión 5b y escaladas, con tope a medir en F2?
- Captured: "confirmo los topes excepto en lo de cupo de Claude. Usemos más."
- Flags: dónde gastar el cupo extra -> Q7b.

### Q7b — Roles de Claude
- Asked: ¿Claude en descomposición + walkthrough (Opus), planner por módulo (Sonnet) y agente que maneja el preview?
- Captured: "confirmo los tres roles para Claude. También poder usar a Haiku 5.5 de subagente. En intercambio con DeepSeek v4 pro de vez en cuando."
- Flags: qué rol cubre Haiku y cómo se decide el intercambio -> Q7c.

### Q7c — Alternancia Haiku / DeepSeek
- Asked: ¿Haiku 5.5 reemplaza a DeepSeek v4 pro como coder por sorteo por módulo, con semilla y registro?
- Captured: "confirmo el sorteo por módulo". Motivo aceptado: elegir a mano mezcla el criterio con el resultado.
- Flags: ninguno.

### Q8 — Destino del mapa
- Asked: ¿el mapa termina con sueldo neto de punta a punta bajo cinco condiciones, sin segundo entregable?
- Captured: "confirmo el destino sin el segundo entregable".
- Flags: segundo entregable de otro tipo (flujo o servicio) -> mapa siguiente.

### Q9 — Backstop: repo, stack y tabla de casos
- Asked: ¿repo privado nuevo con ok, stack propuesto por Opus en el walkthrough, tabla de casos armada por el agente desde fuentes oficiales y aprobada en el walkthrough?
- Captured: "Pero recordá que esto es una herramienta general. Acepto." Las tres propuestas quedan como reglas generales del builder, no como decisiones de F2.
- Flags: ninguno.

## Open flags (pending input)
- `human_interventions` del SDLC nunca se incrementa: arreglarlo para el banco -> implementación del builder.
- Tipos de entregable (producto, servicio, flujo, funcionalidad) y su oráculo agente -> ticket 03 del mapa.
- Haiku 5.5 como coder choca con un invariante medido en tests: la familia anthropic no entra al REGISTRY de mmorch (`test_anthropic_not_a_node`). Rutearlo como el planner `claude:<modelo>` (CLI, gasta cupo), no por el REGISTRY -> implementación del builder.
- Agente de publicación (hosting, dominio, AdSense): esfuerzo aparte, con gate humano por zona roja -> usuario decide cuándo abrirlo.
