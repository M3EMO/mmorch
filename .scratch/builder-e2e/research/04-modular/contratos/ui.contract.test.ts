import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";

const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
const cuenta = (re: RegExp) => (html.match(re) || []).length;

test("test_R1_contrato_del_driver: contrato del driver: input, boton y salida con sus ids", () => {
  assert.equal(cuenta(/id="bruto"/g), 1);
  assert.equal(cuenta(/data-testid="calcular"/g), 1);
  assert.equal(cuenta(/data-testid="neto"/g), 1);
  for (const k of ["jubilacion", "pami", "obra-social", "ganancias"]) assert.equal(cuenta(new RegExp(`data-testid="${k}"`, "g")), 1, k);
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
