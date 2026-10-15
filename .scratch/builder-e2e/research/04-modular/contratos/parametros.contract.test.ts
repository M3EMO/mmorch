import { test } from "node:test";
import assert from "node:assert/strict";
import { PARAMETROS } from "../src/parametros.ts";

test("test_R1_valores_arca_anses_y_aportes: valores de ARCA jul-dic 2026, tope de ANSES oct 2026 y aportes", () => {
  assert.equal(PARAMETROS.gananciaNoImponible, 5585736.93);
  assert.equal(PARAMETROS.deduccionEspecial, 26811537.29);
  assert.equal(PARAMETROS.topeBaseImponible, 4769631.49);
  assert.deepEqual(PARAMETROS.aportes, { jubilacion: 0.11, pami: 0.03, obraSocial: 0.03 });
});

test("test_R2_escala_coherente: escala de 9 tramos, ordenada y coherente: el fijo de cada tramo es el impuesto acumulado del anterior", () => {
  const e = PARAMETROS.escala;
  assert.equal(e.length, 9);
  assert.deepEqual(e.map((t) => t.tasa), [0.05, 0.09, 0.12, 0.15, 0.19, 0.23, 0.27, 0.31, 0.35]);
  assert.deepEqual([e[0].desde, e[0].fijo], [0, 0]);
  for (let i = 1; i < e.length; i++) {
    assert.ok(e[i].desde > e[i - 1].desde);
    assert.ok(Math.abs(e[i - 1].fijo + e[i - 1].tasa * (e[i].desde - e[i - 1].desde) - e[i].fijo) < 1, `tramo ${i}`);
  }
});

test("test_R3_vigencia_y_fuentes: vigencia julio a diciembre 2026 y fuentes oficiales https", () => {
  assert.deepEqual(PARAMETROS.vigencia, { desde: "2026-07", hasta: "2026-12" });
  assert.ok(PARAMETROS.fuentes.length >= 2);
  assert.ok(PARAMETROS.fuentes.every((f) => f.startsWith("https://")));
});
