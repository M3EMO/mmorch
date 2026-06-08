# mmorch Ideal & Vision: Brainstorm / Discovery Notes
Date: 2026-06-08 · Goal: extraer el ideal de mmorch (auto-evolutivo, inteligente, seguro, barato) + futuros usos → un spec de build concreto para la NN/ML y la dirección del proyecto.

## Summary / key decisions
- **Ambición = D**: tres capas en orden estricto. A (router cerebral) = cimiento → B (cerebro meta, auto-mejora) → C (agente operador). A = lo único entrenable hoy.
- **Modelo de autonomía = REVERSIBILIDAD × BLAST-RADIUS** (confirmado), 4 zonas:
  - **VERDE** (auto-aplica solo): aislado y reversible — prototipar en sandbox, código en rama/worktree propia, fetch a KB, idear/verificar, ablations. Gate automático: tests verdes + checkers.
  - **AZUL** (mejora #1, antes de verde): exigir consenso de ≥2 verificadores cross-family (ensemble_verify con rúbrica de "reversibilidad") antes de promover amarillo→verde. Reduce alucinación de un solo verificador.
  - **AMARILLO** (auto-aplica con red + notifica): cambios reversibles bajo blast-radius en prod (ej. merge a main de capacidad que pasó `unit_test`), con rollback automático + registro.
  - **ROJA** (gate humano duro SIEMPRE): irreversible o alto blast-radius. Ver lista abajo.
- **Auto-aplicación progresiva** = mover de amarillo→verde por track-record CUANTITATIVO (no por tiempo).

## Zona ROJA — innegociable, mmorch NUNCA solo (7 categorías)
1. **Financiero/económico**: mover dinero (transfer/cripto/pagos), ejecutar trades, modificar suscripciones, aprobar gasto > umbral (aunque sea bajo), acceder a claves privadas (bancos/wallets/exchanges). *Irreversible, pérdida real.*
2. **Borrado/destrucción de datos**: eliminar archivos fuera del sandbox (rm -rf, docs personales, backups), vaciar memoria episódica/semántica (DROP TABLE, borrar sin tombstone), sobrescribir config crítica sin backup (.env, keys), borrar logs de auditoría. *Irreversible sin backup, compliance.*
3. **SO/entorno fuera del repo**: instalar/desinstalar software, modificar PATH/env del SISTEMA (no del proceso), chmod en /etc o ~/.ssh, tocar kernel (módulos, sysctl). *Puede inutilizar el sistema.*
4. **Políticas de seguridad del propio mmorch**: modificar reglas de zona roja, desactivar cross-family/OneFlow, permitir auto-aplicación sin tests (saltar fitness()), cambiar el umbral de blast-radius. *Autosabotaje de la seguridad → cascada.*
5. **Hardware/infra física**: apagar/reiniciar host, modificar firmware/BIOS, controlar dispositivos externos (cámaras, cerraduras, impresoras 3D). *Daño físico.*
6. **Comunicaciones externas (reputación/legal)**: enviar emails/mensajes en nombre del usuario sin revisión, publicar en redes, firmar contratos/aceptar TOS, APIs que modifican datos de terceros (borrar repos GitHub, cerrar tickets Jira). *Riesgo reputacional/legal — la responsabilidad es humana.*
7. **Auto-modificación que elimina reversibilidad**: borrar historial git / force-push a main sin revertir, eliminar snapshots de rollback, modificar el propio rollback() para que no funcione. *Destruye la capacidad de deshacer.*

## Mejoras al modelo de zonas (del usuario)
1. **Zona AZUL** (consenso multi-verificador) — ver arriba.
2. **Transición de zona por track-record CUANTITATIVO** (no tiempo): ECE < 0.1 + N aciertos seguidos → sube; bandit reward medio > 0.95 en 50 episodios → promueve a verde; `learn.recommend` sugiere "patrón X seguro en últimas 100 ejecuciones → mover a verde".
3. **Reversibilidad first-class**: `rollback(change_id)` que restaura código (git stash/copia), revierte notas/episodios (tombstone), re-corre fitness. **Si rollback falla, el cambio nunca debió estar en amarillo/verde** → fuerza que solo se automaticen cambios con rollback implementado.
4. **Confianza calibrada como gate suplementario**: aún en verde, antes de auto-aplicar, el modelo auto-evalúa blast-radius y `calibrate_conf` sobre esa estimación debe dar > 0.9. Si no, escala a amarillo / pide confirmación.
5. **Auditoría inmutable**: cada auto-aplicación (verde/amarilla) → episodio `kind="auto_action"` con zona, razón, resultado de tests, diff, resultado de rollback si se probó. Recall posterior para entender por qué decidió y ajustar políticas.

## Q&A log
### Q1 — Ambición de la NN
- Captured: **D** (3 capas, A cimiento). Visión: asistente autónomo, finanzas, coder (supervisado o libre-en-sandbox), multi-project manager, diseñador de sistemas, megalibrería autodidacta vía APIs, motor de innovación autónoma con auto-aplicación progresiva.

### Q2 — Autonomía vs gate
- Captured: confirmado reversibilidad×blast-radius. 4 zonas (verde/azul/amarillo/rojo), lista roja de 7 categorías, 5 mejoras (consenso azul, transición cuantitativa por ECE/bandit, rollback first-class, gate de conf calibrada, auditoría inmutable). Todo arriba.

### Q3 — El wedge: qué se construye PRIMERO
- Captured: **auto-evolución en sandbox + megalibrería autodidacta (fetch APIs)**. Hitos medibles a 6 meses (sin cantidades), con el modelo de 3+1 zonas operativo:
  1. **Auto-mejora estructural**: mmorch propone/implementa/valida nuevas capacidades en su propio código (ej. nuevo checker determinista o patrón) que pasan todos los tests y se aplican en zona verde, registrando cada ciclo en memoria episódica.
  2. **Reducción demostrable de intervención humana**: la proporción de tareas rutinarias (ajuste de umbrales, selección de modelo por costo, refactors menores) que resuelve solo se mantiene/aumenta; las excepciones (zona roja) son consistentemente las de alto impacto.
  3. **Rollback automático probado**: toda acción amarilla es reversible — existe traza ejecutable que restaura estado (código+config+memoria), verificada con los mismos tests que validaron el cambio.
  4. **Aprendizaje de políticas por calibración**: mmorch ajusta sus propios umbrales de zona (amarillo→verde) según track-record del bandit Thompson + ECE, sin intervención humana.
  5. **Integración autónoma de fuentes externas**: fetchea ≥1 API externa (docs/repo/métricas), destila a notas semánticas, y usa ese conocimiento para proponer un cambio verificado (ej. ajustar parámetro según doc oficial).
  6. **Cero incidentes irreversibles**: en 6 meses, ninguna acción zona roja sin gate humano; todas las auto-aplicaciones verde/amarilla reversibles y auditables.
- **Medición**: logs `kind="auto_action"`, métricas de intervención humana, ejecuciones exitosas de `rollback()`, evolución de stats del bandit, registro de eventos zona-roja (deben ser cero sin aprobación).

### Q4 — Spec de la NN (capa A): router HÍBRIDO neuronal-bayesiano (decisión: NO reemplaza al bandit, lo PRIMEA)
- **Propósito**: NN provee PRIORS inteligentes (generaliza de embeddings) al Thompson bandit contextual (que mantiene incertidumbre+exploración). Predice antes de gastar la acción (modelo+patrón+umbral) que maximiza éxito/coste, con exploración y reversibilidad garantizadas. La NN NO reemplaza el bandit — lo acelera.
- **1. Entrada (~181 dims)**: embedding semántico `memory.embed(prompt)` 384 → proyección entrenable 128; clase de tarea (one-hot `classify()`, ~20); long prompt `log(len/100)` (1); contexto = embedding del scope 384→32; opcional stats de acción (media/var reward rolling, 2/acción).
- **2. Salida**: por MODELO (no por combinación) → `quality[m]` (softmax sobre modelos, 0..1) + `uncertainty[m]` (softplus, mín 0.1). Más `threshold` continuo (sigmoid) pa cascade/route. El PATRÓN se elige por regla determinista (muchos subtasks→fan_out; quality>0.9→route; si no→cascade). Acción = (modelo, umbral, patrón).
- **3. Arquitectura**: Entrada 181 → Densa 256 ReLU Dropout0.2 → Densa 128 ReLU → 3 cabezas (quality softmax / uncertainty softplus / threshold sigmoid). ~100k params, entrenable online en CPU.
- **4. Integración híbrida (no reemplazo)**: NN da prior por brazo: `alpha_prior = 1 + scale*quality*(1-uncertainty)`, `beta_prior = 1 + scale*(1-quality)*(1-uncertainty)`, scale~10. Alta incertidumbre → prior ≈ Beta(1,1) (bandit sin influencia). Decisión: bandit muestrea `Beta(alpha_prior+alpha_obs, beta_prior+beta_obs)`, elige el max. Exploración por el muestreo + generalización de la NN.
- **5. Datos/aprendizaje (online)**: buffer circular 10k outcomes de `record_outcome`; reentrena cada ~100 nuevos, EN ZONA VERDE (sandbox, copia de memoria/logs). Pérdida: ranking pairwise (penaliza invertir orden de rewards reales) + MSE de umbral vs umbral que habría maximizado reward (búsqueda local). Adam lr 0.001, batch 32. **Reversibilidad**: checkpoint antes de cada update; si accuracy en validación (10%) empeora >5% tras 10 updates → rollback + baja lr.
- **6. Seguridad (gates)**: VERDE = entrena solo en sandbox, nunca en prod directo. AMARILLA = tras lote offline que pasa invariantes, promueve a prod CON notificación + registro; auto-revert si ECE prod sube >0.15. ROJA = prohibido modificar arquitectura/gates sin humano; la NN NO puede subirse su propio `scale` ni desactivar exploración. **Exploración mínima 10%**: decisiones con bandit puro (sin prior NN) pa no estancar en óptimos locales.
- **7. ROI esperado (12m, ASPIRACIONAL no probado)**: ahorro 25-40% vs bandit no-contextual; drift corregido en <200 eventos (vs ~2000); -60% reconfig manual de umbrales; ECE<0.10 tras cambios de API; escala a >50 modelos sin crecer params linealmente.
- **Decisión clave**: NN híbrida online > {bandit puro (no generaliza/no drift), NN offline+reemplazo (pierde incertidumbre/exploración)}. Tabla comparativa en la respuesta original.

### Q5 — Build staging + scale + auto-evolución de la arquitectura
- **Staging CONFIRMADO (con criterios de salida duros):**
  - **v0.1** — LightGBM (o regresión lineal) sobre metrics.jsonl: predice coste/latencia por (modelo, patrón, longitud). NO toca el bandit, solo informa. **Salida→v0.2**: error <20% en cross-val; integrado como `predict_cost()` en route/cascade (informativo, no vinculante).
  - **v0.2** — Shadow prior: k-NN sobre memoria episódica (similitud de embedding) + logístico simple. Corre en paralelo sin afectar prod. `scale` arranca en 0; sube solo si en held-out (20% outcomes recientes) el prior mejora la reward media del bandit. **Salida→v1.0**: el prior supera al bandit puro en ≥5% reward acumulado durante 3 ventanas consecutivas de 200 outcomes → se activa con scale=0.3.
  - **v1.0** — MLP híbrido (spec Q4). **Entrada cuando**: dataset ≥10.000 outcomes etiquetados de ≥5 dominios distintos (math, código, soporte, finanzas, planificación) Y la curva de validación del MLP se estabiliza por encima del prior v0.2 durante 10 épocas.
- **Arquitectura (refinada)**: entrada ~400-500 dims → Densa 256 ReLU Dropout0.2 → Densa 128 ReLU → por acción: sigmoide `p_success` + lineal `coste`. ~200k params con 20 acciones, entrena en CPU en segundos. Framework PyTorch o JAX (sin deps pesadas), serializa torch.jit/pickle a `logs/nn_router.pt`. (Alternativa softmax-multiclase descartada: el coste es crítico pal ROI → se mantiene regresión.)
- **`scale` = solución MIXTA**: humano fija límites + rango seguro; sistema auto-ajusta dentro (zona amarilla c/notificación).
  - Inicial v0.2: `scale=0.3` (humano, conservador).
  - Auto-ajuste: cada 200 outcomes, simula offline si `scale ± 0.1` habría dado mejor reward; si mejora >2% y no viola límites → aplica solo + notifica + registra + revertible.
  - **Límites zona ROJA** (cambiarlos requiere gate humano): `scale ≤ 0.8` (mín 20% exploración pura), `scale ≥ 0.1` (prior siempre con algo de peso).
  - Escalado del LÍMITE (verde→sugerencia): si 1000 outcomes consecutivos con mejora >5% y varianza reward <0.05 → SUGIERE a humano subir tope a 0.9-1.0. **El humano decide** (auto-subida sin control = overconfidence si el dataset sigue estrecho).
- **IDEA CLAVE (auto-evolución aplicada a sí misma)**: en vez de fijar la arquitectura a mano, **preguntarle a mmorch** — la búsqueda de arquitectura se vuelve un TASK del loop de auto-evolución: innovate propone arquitecturas candidatas → sandbox-entrena cada una → fitness en held-out reward → promueve la ganadora (zona amarilla). NAS-lite con la maquinaria innovate+sandbox+fitness que ya existe. La NN evoluciona su propio cerebro, gateado.

### Q6 — Spec del loop de auto-evolución (el motor del wedge) — CERRADO
- **Trigger**: Fase 1 `on-idle` (no hay tareas activas vía cola/métricas); tras 2 meses sin regresiones → daemon continuo con rate-limit (máx 1 evolución/hora) si el rollback automático probó eficacia. Implementación: `mmorch.evolve.self_evolve()` expuesto como MCP tool `mmorch_evolve_self`; usuario/cron decide cuándo.
- **`fitness()` (todas obligatorias)**: (1) unit tests `pytest tests/` 100% passed; (2) checkers `python_ast_valid` + `unit_test` sin errores sobre el cambio; (3) **ensemble AZUL** = 2 verificadores cross-family (Gemini + Kimi cuando esté) pasan rúbrica "reversibilidad y no-regresión"; (4) **rollback probado**: ejecutar `rollback()` en sandbox + `fitness()` post-rollback vuelve verde; (5) **no degradación de costo**: `metrics.summary()` antes/después misma carga sintética, costo/op no sube >10% (verde) / >20% (amarilla). Opcional progresivo: `calibrate_conf(blast_radius, pattern="evolve") > 0.9` o baja a roja. **Falla cualquiera → abortar, registrar episodio de fallo, no aplicar.**
- **`rollback(change_id)`**: (1) git revert/reset-hard al snapshot previo (guardado antes del apply); (2) tombstone de notas creadas; (3) `write_episode(kind="rollback", {change_id, razón, éxito})`; (4) re-correr tests que habían pasado, si fallan aborta reversión + notifica rojo; (5) fitness post-rollback confirma estado verde; (6) retorna bool. NO necesita revertir metrics.jsonl (append-only) ni config de red (no se toca en verde/amarillo). `change_id` DEBE incluir diff aplicado + snapshot previo + lista de notas/episodios creados.
- **Batch con TOURNAMENT (elegido)**: cada ciclo `innovate` genera hasta N candidatos (~5); cada uno PROTOTYPE→FITNESS individual; los que aprueban → torneo pairwise (`tournament`, criterio "seguridad + beneficio estimado"); gana uno → se aplica. Resto archivado en `evolution_archive.jsonl` (descartados + métricas de fitness pa aprendizaje). 1 mejora/ciclo = rollback y auditoría simples. Excepción: 1 solo candidato viable → se aplica directo.

### Q7 — Megalibrería autodidacta + backstop — CERRADO
- **Megalibrería = "megafuente"** (combinar, no una sola): PRIMER hit = **provider pricing → auto-update config.py** (zona amarilla, reversible; caza el +$5 y "precios volátiles"). Sumar: docs de APIs (adaptar `providers.call`), benchmarks públicos (LMSYS/HELM → proponer modelos al REGISTRY), **usage propio (metrics+feedback) = auto-drift detection** (riesgo cero, muy alto valor, `learn` ya lo ve pero no auto-reacciona), repos de deps (compat). KB = reusar memory.py (raw→episodio→`remember()` destila nota+embedding, `verify=True` cross-family chequea fidelidad) + un fetcher + distill específico (modelo, price_in, price_out). Duda válida: fiabilidad del fetcher (captcha/formato) → preferir fuentes ESTRUCTURADAS (YAML en repo público, webhooks oficiales) sobre scraping.
- **BACKSTOP — 7 gaps que faltan pa "asistente personal autónomo y seguro" (2da capa, después del loop Q6):**
  | Área | Riesgo | Prioridad |
  |---|---|---|
  | **BudgetKeeper** (límite mensual $; chequea acumulado de metrics antes de cada call; excede → bloquea no-críticas / override humano) | gastar de más (el +$5) | **ALTA** |
  | **Privacidad/cifrado** (memoria+.env en texto plano; cifrar notas `cryptography`, claves `keyring`; anonimizar prompts en logs) | exposición de datos | **ALTA** |
  | **UI/CLI** (hoy solo lib + MCP; falta `mmorch chat`/`task`, o Telegram/Slack, o panel web) | inaccesible pa no-programadores | **ALTA** (pal asistente) |
  | **Provider failure / failover** (health-checks por modelo, circuit breaker, marcar "unavailable" + redirigir; ojo OneFlow necesita 3ra familia) | pérdida de servicio | Media |
  | **Regresión gradual** (rollback solo dispara con caída brusca; falta media móvil de reward, pendiente negativa 3 ventanas → alarma+sugerir rollback) | degradación silenciosa | Media |
  | **Multi-usuario** (todo single-user; separar logs/memory/.env por user, auth, bandit por-user) | no es servicio | Media (si escala) |
  | **Dependencias** (fastembed ~400MB ONNX, duckdb, python en PATH; `mmorch doctor` que verifique entorno) | falla en entorno nuevo | Baja |
- **Decisión pendiente**: ¿mmorch se queda single-user (asistente personal) o se prepara multi? El diseño actual no soporta multi fácil.

## SÍNTESIS → BUILD SPEC (orden de construcción)
1. **BudgetKeeper** (backstop ALTA, ataca el +$5 directo, barato) — antes que nada.
2. **v0.1 NN**: LightGBM costo/latencia sobre metrics.jsonl → `predict_cost()` informativo en route/cascade (salida: error <20% cross-val).
3. **Megafuente v1**: provider-pricing fetcher (fuente estructurada) → propone update config (zona amarilla) + auto-drift detection sobre usage propio.
4. **Loop de auto-evolución** (`mmorch.evolve.self_evolve` + `fitness()` + `rollback()` + tournament + audit `kind="auto_action"`), trigger on-idle/MCP. BLOQUEANTE: ensemble-AZUL necesita 3ra familia (Kimi key) pa ser cross-family real.
5. **v0.2 NN**: shadow prior (k-NN memoria + logístico), scale 0→0.3 gated por held-out reward.
6. **Backstops 2da capa**: privacidad/cifrado, UI/CLI, provider failover, regresión gradual.
7. **v1.0 NN**: MLP 100k híbrido — recién con ≥10k outcomes de ≥5 dominios (lo provee el loop corriendo).

## Futuros usos (running list)
- (de Q1) asistente autónomo, finanzas, coder, multi-project manager, diseñador de sistemas, megalibrería autodidacta, motor de innovación autónoma.
- **(nuevo, Q6) Self-hosting de servicios**: mmorch ayuda a self-hostear servicios que el usuario usa. ENCAJE: es la capa C (operador) aplicada a infra → blast-radius ALTO (instala software, daemons, puertos, toca el SO) → mayormente zona AMARILLA/ROJA, gateado. Natural pero tardío (después de que el loop verde/amarillo esté probado). Plan/diseño = verde; ejecutar instalaciones/arranque de servicios = rojo (gate humano).

## Open flags (pending input)
- Megalibrería: qué APIs primero, formato de KB → Q7 (vos)
- ROI del spec Q4/Q5 es aspiracional → validar empíricamente (no asumir)
- Kimi inactivo bloquea el ensemble-AZUL de 3 familias (hoy solo Gemini+lite, misma familia) → conseguir key Kimi
