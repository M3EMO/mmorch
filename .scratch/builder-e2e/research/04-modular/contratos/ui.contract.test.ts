import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { validarBruto } from "../src/validacion.ts";

const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
const cuenta = (re: RegExp) => (html.match(re) || []).length;

test("test_R1_contrato_del_driver: contrato del driver: input, boton y salida con sus ids", () => {
  assert.equal(cuenta(/id="bruto"/g), 1);
  assert.equal(cuenta(/data-testid="calcular"/g), 1);
  assert.equal(cuenta(/data-testid="neto"/g), 1);
  for (const k of ["jubilacion", "pami", "obra-social", "ganancias"]) assert.equal(cuenta(new RegExp(`data-testid="${k}"`, "g")), 1, k);
});

test("test_R4_input_con_etiqueta: el campo del bruto tiene una etiqueta accesible (hallazgo de axe, walkthrough F2)", () => {
  const etiquetado = /<label[^>]*for="bruto"/.test(html) || /<input[^>]*id="bruto"[^>]*aria-label="[^"]+"/.test(html)
    || /<input[^>]*aria-label="[^"]+"[^>]*id="bruto"/.test(html);
  assert.ok(etiquetado, "falta <label for=\"bruto\"> o aria-label en el input");
});

test("test_R5_desglose_con_etiquetas: cada monto tiene su etiqueta visible justo antes (hallazgo del juez en sombra)", () => {
  const etiquetas: [string, RegExp][] = [["neto", /neto/i], ["jubilacion", /jubilaci[oó]n/i], ["pami", /pami/i],
    ["obra-social", /obra social/i], ["ganancias", /ganancias/i]];
  for (const [id, etiqueta] of etiquetas) {
    const i = html.indexOf(`data-testid="${id}"`);
    const antes = html.slice(Math.max(0, i - 300), i).replace(/<[^>]*>/g, " ").replace(/\s+/g, " ").trim();
    assert.ok(i >= 0 && etiqueta.test(antes.slice(-60)), `la salida "${id}" no tiene su etiqueta visible justo antes`);
  }
});

test("test_R2_metadatos_seo: metadatos SEO", () => {
  assert.match(html, /<html[^>]*lang="es-AR"/);
  const title = (html.match(/<title>([^<]*)<\/title>/) || [])[1] || "";
  assert.ok(title.length >= 30 && title.length <= 60, `title de ${title.length}`);
  const desc = (html.match(/<meta name="description" content="([^"]*)"/) || [])[1] || "";
  assert.ok(desc.length >= 70 && desc.length <= 160, `description de ${desc.length}`);
  assert.equal(cuenta(/<h1[\s>]/g), 1);
  assert.match(html, /<link rel="canonical" href="https:\/\//);
});

test("test_R3_modulo_compilado_sin_externos: carga el modulo compilado y nada externo", () => {
  assert.match(html, /<script type="module" src="dist\/app\.js"><\/script>/);
  assert.equal(cuenta(/(src|href)="https?:\/\/(?!ejemplo\.com\.ar)/g), 0);
  assert.ok(existsSync(new URL("../src/app.ts", import.meta.url)));
});

test("test_R6_entrada_validada: rechaza montos absurdos, negativos o no numericos con un error visible (parada 2)", () => {
  for (const t of ["10e100", "1e6", "1000000000.01", "99999999999", "-5", "abc", "", "  ", "Infinity", "NaN", "1.2.3", "5,123"]) {
    const r = validarBruto(t);
    assert.ok(!r.ok && r.error.trim().length > 0, `acepto "${t}"`);
  }
  const validos: [string, number][] = [["3500000", 3500000], ["0", 0], ["1000000000", 1e9], [" 2500000 ", 2500000],
    ["1234567.89", 1234567.89], ["1234567,5", 1234567.5]];
  for (const [t, v] of validos) {
    const r = validarBruto(t);
    assert.ok(r.ok && Math.abs(r.bruto - v) < 1e-9, `rechazo "${t}"`);
  }
  assert.match(html, /<[^>]*data-testid="error"[^>]*(role="alert"|aria-live="(polite|assertive)")|<[^>]*(role="alert"|aria-live="(polite|assertive)")[^>]*data-testid="error"/,
    "falta el elemento data-testid=\"error\" con role=\"alert\" o aria-live");
});
