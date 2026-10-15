import { test } from "node:test";
import assert from "node:assert/strict";
import { calcularNeto } from "../src/calculo.ts";
import { PARAMETROS as P } from "../src/parametros.ts";

const cerca = (a: number, b: number, msg: string) => assert.ok(Math.abs(a - b) <= 1, `${msg}: ${a} vs ${b}`);

test("test_R1_ejemplos_publicos: ejemplos publicos de la spec (convencion anualizada, soltero, sin SAC)", () => {
  const ej: [number, number, number, number, number][] = [   // bruto, neto, jubilacion, pami, ganancias
    [1500000, 1245000, 165000, 45000, 0],
    [3000000, 2490000, 330000, 90000, 0],
    [3500000, 2893757.86, 385000, 105000, 11242.14],
    [5000000, 3973040.42, 524659.46, 143088.94, 216122.23],
    [10000000, 7513307.95, 524659.46, 143088.94, 1675854.7],
  ];
  for (const [g, n, jub, pami, gan] of ej) {
    const d = calcularNeto(g, P);
    cerca(d.neto, n, `neto ${g}`); cerca(d.jubilacion, jub, `jubilacion ${g}`);
    cerca(d.pami, pami, `pami ${g}`); cerca(d.obraSocial, pami, `obra social ${g}`); cerca(d.ganancias, gan, `ganancias ${g}`);
  }
});

test("test_R2_desglose_suma: desglose: neto = bruto - aportes - ganancias, sin negativos", () => {
  for (const g of [0, 250000, 3252738, 4769631.49, 12345678.9, 80000000]) {
    const d = calcularNeto(g, P);
    cerca(d.neto, d.bruto - d.jubilacion - d.pami - d.obraSocial - d.ganancias, `suma ${g}`);
    for (const v of Object.values(d)) assert.ok(v >= 0, `negativo en ${g}`);
  }
});

test("test_R3_metamorficas: metamorficas: monotono, 83% bajo el umbral, aportes fijos arriba del tope", () => {
  let prev = -1;
  for (let g = 100000; g <= 30000000; g += 99991) {
    const n = calcularNeto(g, P).neto;
    assert.ok(n >= prev - 0.01, `monotono en ${g}`); prev = n;
  }
  for (const g of [100000, 1000000, 3000000]) cerca(calcularNeto(g, P).neto, 0.83 * g, `83% en ${g}`);
  const a = calcularNeto(6000000, P), b = calcularNeto(9000000, P);
  cerca(a.jubilacion + a.pami + a.obraSocial, b.jubilacion + b.pami + b.obraSocial, "aportes con tope");
});
