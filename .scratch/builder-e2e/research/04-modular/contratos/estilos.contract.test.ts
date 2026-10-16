import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";

// Parada 2 de F2: "muy basico". Estos son los minimos computables; el resto lo opinan el juez visual y el usuario.
const html = readFileSync(new URL("../index.html", import.meta.url), "utf8");
const ruta = new URL("../estilos.css", import.meta.url);
const css = existsSync(ruta) ? readFileSync(ruta, "utf8") : "";
const bloque = (re: RegExp) => { const i = css.search(re); return i < 0 ? "" : css.slice(i, css.indexOf("}", css.indexOf("{", i)) + 1); };

test("test_R1_hoja_local_enlazada: index.html enlaza estilos.css, local y sin estilos en linea", () => {
  assert.match(html, /<link rel="stylesheet" href="estilos\.css">/);
  assert.ok(css.trim().length > 0, "estilos.css no existe o esta vacio");
  assert.doesNotMatch(html, /\sstyle="/, "estilos en linea: van en estilos.css");
});

test("test_R2_tokens_y_tema_oscuro: colores como variables en :root, redefinidas para el tema oscuro", () => {
  const raiz = bloque(/:root\s*\{/);
  const tokens = raiz.match(/--[\w-]+\s*:/g) || [];
  assert.ok(tokens.length >= 4, `:root define ${tokens.length} variables (minimo 4)`);
  const i = css.search(/@media\s*\(prefers-color-scheme:\s*dark\)/);
  assert.ok(i >= 0, "falta @media (prefers-color-scheme: dark)");
  assert.ok(tokens.some(t => css.slice(i).includes(t)), "el tema oscuro no redefine ninguna variable de :root");
  assert.match(bloque(/(^|[\s,}])body\s*\{/), /background(-color)?\s*:\s*var\(--/, "body sin fondo tomado de una variable");
});

test("test_R3_neto_resalta: el neto tiene su propia regla con un tamaño de letra mayor", () => {
  assert.match(bloque(/\[data-testid="?neto"?\][^{]*\{/), /font-size\s*:/, "falta una regla para [data-testid=neto] con font-size");
});

test("test_R4_ancho_y_cifras: contenido con ancho maximo y cifras tabulares que no desbordan", () => {
  assert.match(css, /max-width\s*:/, "falta max-width en el contenedor");
  assert.match(css, /font-variant-numeric\s*:\s*tabular-nums/, "faltan cifras tabulares en los montos");
  assert.match(css, /overflow-wrap\s*:\s*(anywhere|break-word)|word-break\s*:\s*break-(all|word)/, "un monto largo puede desbordar");
});
