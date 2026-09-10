# supervision.md — brazo C (híbrido: automático + Claude en los gates)

Cada entrada es una intervención de Claude. Se cuenta, se fecha y se justifica.

## I1 — gate de plan (revisión antes del build) · 2026-09-10 14:58
Leí `spec.md` (20 KB, reasoner) y `plan.md` (10 archivos, reasoner).
Decisión: **aprobado sin cambios de fondo**. Una nota al coder:
- El ítem 1 del plan lista `pom.xml`. El pom del baseline ya tiene Jackson, JUnit 5 y
  surefire 3.2.5. No se regenera. El build empieza en `SeqMatch.java`.
Riesgos del plan que voy a vigilar en el test: orden de normalización (§8.1), port de
`SequenceMatcher` (§8.2), formato ARS (§8.4), keycaps de 3 code points en `MENU` (§8.15).
Tiempo: 3 min de lectura.

## I2 — gate de test, vuelta 0 · 14:58
Fallo: el test no compila. `Catalog.load(Path)` declara `throws IOException`; el test
lo llama sin manejarlo. El contrato del test no admite excepciones chequeadas.
Diagnóstico: 20 segundos. Es una lectura del error, no del comportamiento.
Acción: corrección manual de 1 método (envolver en `UncheckedIOException`). Sin llamada al modelo.
Nota: el plan de reasoner decía `static List<Product> load(Path)` sin excepción. El coder
la agregó por costumbre Java. Un gate sintetizado "las firmas del contrato coinciden con
el test" lo habría atrapado a costo cero antes del test.

## Resultado · 15:05
`mvn -B test`: Tests run 5, Failures 0, Errors 0. BUILD SUCCESS.
Intervenciones de Claude: 2 (I1 revisión de plan, 3 min; I2 fix de 1 método, 1 min).
Llamadas delegadas: 9, todas deepseek-v4-pro, en el build. Cero llamadas de fix.
