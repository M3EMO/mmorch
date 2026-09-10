# spec.md — Motor del agente (port de `backend/reference/demo.py` a Java 17)

Etapa 1 de 6 del SDLC. Brazo B del A/B. Reemplaza al prototipo Python por un jar plano de Maven en `backend/`, sin Spring. El oráculo normativo es `backend/reference/demo.py`; ante cualquier duda, gana el Python línea por línea.

## 1. Alcance

**Dentro:** catálogo local, matcher de intents por keywords + fuzzy, sesiones en memoria por `sid`, flujo de reserva, constantes públicas. Todo lo que el `BotAcceptanceTest.java` (incluido en el repo, no se modifica) ejercita.

**Fuera:** LLM, integración WhatsApp, Postgres, dashboard, multi-tenant, HTTP server, fetch del catálogo a la API de WooCommerce (`--refresh` no se porta).

## 2. Layout de módulos

```
backend/
  pom.xml
  reference/
    demo.py                 (oráculo, no se toca)
    test_demo.py            (oráculo, no se toca)
    catalogo.json           (cargado por Catalog.load)
  src/main/java/com/qtp/bot/
    Catalog.java
    Product.java
    Variant.java
    Bot.java
    Reply.java
    Msg.java
    Norm.java               (interno, static helpers)
    Ars.java                (interno, formato de precio)
    SeqMatch.java           (interno, port de SequenceMatcher.ratio)
  src/test/java/com/qtp/bot/
    BotAcceptanceTest.java  (provisto, no se modifica)
```

`pom.xml`: `maven-compiler-plugin` con `release=17`; dependencias `com.fasterxml.jackson.core:jackson-databind` y `org.junit.jupiter:junit-jupiter` (scope test); `maven-surefire-plugin` ≥ 3.0 (JUnit 5). Working dir del test = `backend/` (implica `cd backend && mvn -q -B test`).

## 3. Contrato (paquete `com.qtp.bot`)

Firmas exactas que el test compila.

### 3.1 `Product`
Inmutable, campos expuestos por getters o acceso público: `String name`, `double price`, `double regular`, `List<String> images`, `List<String> cats`, `List<Variant> variants`. Jackson mapea por nombre de campo (`@JsonProperty` si hace falta). `images` y `cats` pueden venir vacíos; `variants` nunca null.

### 3.2 `Variant`
Inmutable: `String color`, `String talle`, `boolean stock`. `color` y `talle` default `"?"` si faltan en el JSON (`attrs.get(..., "?")` del Python).

### 3.3 `Catalog`
```java
public static List<Product> load(Path json) throws IOException
```
Lee un JSON array de objetos con las claves `name`, `price`, `regular`, `images`, `cats`, `variants[{color,talle,stock}]`. Usa Jackson. El orden del array es significativo (insertion order para `findProduct`).

### 3.4 `Msg`
```java
public String text()   // nunca null; puede ser "" (fotos sin texto)
public String image()  // nullable
```

### 3.5 `Reply`
```java
public List<Msg> replies()  // nunca null, puede ser vacía
public boolean paused()
public String order()       // nullable; solo no-null desde el cierre de reserva
```

### 3.6 `Bot`
```java
public Bot(List<Product> catalog)
public Reply reply(String sid, String text)

public static final String MENU;
public static final String HANDOFF;
public static final String MAYORISTA;
```
Sesiones en un `Map<String,Session>` en memoria (ver §5). `reply` crea la sesión si no existe (`setdefault`). Aunque el test solo usa `MENU/HANDOFF/MAYORISTA`, se declaran además `SALUDO`, `HORARIO`, `ENVIOS`, `NOENTIENDO`, `VOLVER` como `static final` (visibilidad interna/package-private alcanza).

## 4. Normalización y helpers internos

### 4.1 `Norm.apply(String s)` — idéntico a `norm` del Python
1. `toLowerCase(Locale.ROOT)`.
2. `Normalizer.normalize(..., Form.NFD)`.
3. Eliminar todo code point con `Character.getType(cp) == Character.NON_SPACING_MARK` (equivale a `unicodedata.category(c) != "Mn"`).
4. `replaceAll("[^a-z0-9/ ]", "")`.

Se conserva el espacio y la barra `/`; se descarta todo lo demás (incluido `_`, signos, acentos ya descompuestos). Aplicar exactamente en ese orden.

### 4.2 `Ars.format(double n)` — idéntico a `ars`
`"$ " + formatear(n)` donde el número se imprime con **0 decimales**, separador de miles `.`, redondeo a entero. Ej.: `1234.56 → "$ 1.235"`, `999 → "$ 999"`, `12000 → "$ 12.000"`.

### 4.3 `SeqMatch.ratio(String a, String b)` — port de `difflib.SequenceMatcher.ratio()`
`ratio = 2·M / (|a| + |b|)`, con `M` = suma de longitudes de los bloques coincidentes que devuelve el algoritmo recursivo **Ratcliff–Obershelp** (`get_matching_blocks` → `find_longest_match`). No equivale a Levenshtein, Jaro-Winkler ni `commons-text:FuzzyScore`; hay que portar `find_longest_match` (recurse izquierda/derecha del mejor bloque, tie-break: mayor tamaño, luego menor `i`, luego menor `j`). `isjunk=null` y `autojunk` irrelevante (nombres < 200 chars).

### 4.4 Utilidades
- `talles(List<Variant>)`: `Set<String>` de talles distintos ordenado por el entero inicial de cada talle (`Integer.parseInt` del prefijo `\d+`).
- `colors(List<Variant>)`: `Set<String>` de `color` distintos ordenado lexicográficamente (code point), unido con `", "`.
- `porColor(List<Variant>)`: por cada color ordenado, `"{color} ({t1, t2, ...})"` unido con `" · "`, talles ordenados como en 4.4.
- `alts(Product p, List<Variant> vs, String talle)`: colores disponibles para ese talle → `"En talle {talle} de {name} tengo: {colores}."`; si no hay → `"Talle {talle} de {name} no me queda. Tengo: {porColor(vs)}."`.
- `shown(Product p, String colorNorm)`: primer `v.color` original con `norm(v.color).equals(colorNorm)`, o el propio `colorNorm` si no aparece.
- `title(String s)`: port de `str.title()` — capitaliza la primera letra de cada corrida de letras y baja el resto (`"la plata" → "La Plata"`).

## 5. Estado de sesión

Campos por `sid`:
```
boolean paused        = false;
int     fails         = 0;
Product last          = null;
String  mode          = "menu";   // "menu" | "catalog"
List<Product> items   = List.of(); // usados en mode "catalog"
String  step          = null;     // null | "confirm" | "entrega" | "localidad"
Offer   offer         = null;     // {name, color, talle, price} puesto por stockAnswer
```
`Offer` es una clase interna con `name`, `color`, `talle` (String) y `price` (double).

El numerador de reserva es `1000 + sessions.size()` (cuenta **todas** las sesiones creadas, incluidas pausadas).

## 6. Comportamiento de `Bot.reply(sid, text)`

Sea `t = Norm.apply(text)` y `n = t.strip()`. Se evalúa en este orden exacto; la **primera** regla que matchea corta y devuelve.

0. Si `paused` → `Reply(replies=[], paused=true, order=null)` (el humano responde por fuera).
1. Si `step != null` **y** `n` no es `"0"` ni `"menu"` → `orderStep(s, t)`.
2. Si `t` matchea `\b(humano|persona|maxi|alguien|jefe|hablar con)\b` → `paused=true`; `[HANDOFF]`.
3. Si `t` matchea `\b(mayorista|revend|por mayor|reventa)` → `paused=true`; `[MAYORISTA]`.
4. Si `t` matchea `\b(horario|abren|cierran|atienden|hora)` → `ok(HORARIO)`.
5. Si `t` matchea `\b(envio|envios|mandan|correo|llega|andreani)\b` → `ok(ENVIOS)`.
6. Si `t` matchea `\b(hola|buenas|buen dia|buenos dias|buenas tardes)\b` y `t.split()` tiene ≤ 4 palabras → `ok(SALUDO)`.
7. Si `n ∈ {"0","menu"}` → `mode="menu"`, `step=null`; `ok(MENU)`.
8. Si `mode == "catalog"` y `n` son solo dígitos y `1 ≤ n ≤ items.size()` → `fails=0`; `productReply(items[n-1])`.
9. Si `n == "4"` → `paused=true`; `[HANDOFF]`.
10. Si `n == "3"` → `paused=true`; `[MAYORISTA]`.
11. Si `n == "2"` → `ok(HORARIO + "\n\n" + ENVIOS + VOLVER)`.
12. Si `n == "1"` o `t` matchea `\b(modelos?|que tenes|que tienen|que hay|catalogo|productos?|mostrame|opciones|que vend|para (hombre|mujer))` → `catalogReply(t)`.
13. `talle`: primer match de `\b(3[4-9]|4[0-6])(?:/4[0-9])?\b` en `t` (grupo 0 completo), o `null`.
14. `color`: `findColor(t)`.
15. `followup = talle != null || color != null || t` matchea `\b(talle|color|precio|sale|cuesta|hay)\b`.
16. `p = findProduct(t)`; si `null` y `followup` → `p = last`.
17. Si `p == null`: `fails++`; si `fails >= 3` → `paused=true`, `[HANDOFF]`; si no, `[NOENTIENDO]`.
18. `last = p`; `ok(stockAnswer(p, talle, color), image = images[0] != null ? images[0] : null)`.

`ok(s, text, image=null)`: `fails = 0`; `Reply(replies=[Msg(text, image)], paused=false, order=null)`. `ok(s, text)` fija `image=null`.

### 6.1 `findProduct(t)` — port de `find_product`
Sea `words = t.split()` (por espacios). Para cada nombre normalizado `key` (en orden de inserción del catálogo, deduplicado por `Norm.apply(name)`), `hits(key)` = cantidad de tokens `k` de `key.split()` tales que **existe** un `w ∈ words` con `SeqMatch.ratio(k, w) > 0.8`. Se elige el `key` con mayor `(hits, -key.length())`; en empate estricto gana el **primero** en orden de inserción (`max` con `>` estricto, no `>=`). Si `hits(mejor) == 0` → `null`.

### 6.2 `findColor(t)` — port de `find_color`
Lista de colores normalizados `Norm.apply(v.color)` **únicos**, ordenada por longitud descendente (empate: orden de aparición estable; ver §8.5). Para cada `c`: si `c` contiene espacio, `stem = c`; si no, `stem = c[:-1]` (quita el último carácter). Si `t` matchea `\b` + `Pattern.quote(stem)` (búsqueda, no ancla final) → devuelve `c`. Si ninguno matchea → `null`.

### 6.3 `stockAnswer(p, talle, color)` — port de `stock_answer`
`vs = variantes con stock`. `precio = Ars.format(p.price)` y, si `p.regular > p.price`, se anexa ` (antes {Ars.format(p.regular)}, {round(100 - p.price*100/p.regular)}% off)` (round a entero).

- `vs` vacío → `"{name}: ahora mismo sin stock 😔. Si querés te aviso cuando entre."` (U+1F614).
- `fits(v)` ⇔ `talle` ∈ `v.talle.split("/")` (talle doble: `"40"` matchea `"39/40"`).
- **`talle != null && color != null`**:
  - `hit` = primera variante con `fits(v) && Norm.apply(v.color).equals(color)`.
  - Si hay → guarda `offer` y `step="confirm"`; texto `"¡Sí! Tengo {name} en {hit.color} talle {hit.talle}. Sale {precio}. ¿Te lo reservo?"`.
  - Si no → `"En {shown(p,color)} talle {talle} no me queda. " + alts(p, vs, talle)`.
- **`talle != null`** (sin color) → `alts(p, vs, talle) + " Sale {precio}."`.
- **`color != null`** (sin talle):
  - `ts = talles(vs filtradas por color)`.
  - Si `ts` no vacío → `"{name} en {shown(p,color)}: tengo talles {ts unidos por ", "}. Sale {precio}. ¿Qué talle usás?"`.
  - Si no → `"{name} en {shown(p,color)} no me queda. Tengo: {porColor(vs)}."`.
- **ninguno** → `"{name}: sale {precio}. Tengo: {porColor(vs)}. ¿Qué talle y color querés?"`.

### 6.4 `catalogReply(s, t)` — port de `catalog_reply`
`cats = { CATS[k] : k ∈ CATS ∧ t matchea "\b" + k }` (atención: solo ancla inicial, sin `\b` final). `CATS` es:

```
hombre→Hombre, mujer→Mujer, invierno→Invierno, otono→Otoño,
primavera→Primavera, verano→Verano, frio→Invierno, calor→Verano
```

`ps` = productos con al menos una variante con stock **y** (`cats` vacío **o** `cats ∩ p.cats ≠ ∅`), ordenados por `price` ascendente. Si `ps` vacío → `ok("Para eso no tengo stock ahora 😔." + VOLVER)` (U+1F614). Si no:
- `mode="catalog"`, `items=ps`, `fails=0`.
- `head = "Esto tengo con stock" + (cats vacío ? "" : " para " + join(sorted(cats), ", ").toLowerCase()) + ":\n\n"`.
- `lines` = por índice i (1-based): `"{i}. {name} — {Ars.format(price)}"` (guion em U+2014), unido por `"\n"`.
- `tail = "\n\nEscribí el número del modelo (1 a {ps.size()}) y te mando fotos, talles y colores." + VOLVER`.
- Devuelve un único `Msg(head + lines + tail, image=null)`, `paused=false`.

### 6.5 `productReply(s, p)` — port de `product_reply`
`last = p`, `mode = "menu"`. `msgs[0] = Msg(stockAnswer(p, null, null) + VOLVER, image = p.images[0])`. Luego, por cada `src ∈ p.images[1..3]` (índices 1, 2, 3 — `[1:4]`), agrega `Msg("", image=src)`. `paused=false`.

### 6.6 `orderStep(s, t)` — máquina confirm → entrega → localidad
- `step == "confirm"`: si `t` matchea `\b(si|dale|ok|bueno|reserv|quiero|listo)\b` → `step="entrega"`, texto `"Perfecto 👍 ¿Retirás por el local (Dr. Bonfiglio 78, Salto) o te lo envío? Escribí *retiro* o *envío*."` (U+1F44D). Si no → `step=null`, texto `"Sin problema. ¿Querés ver otro modelo? Escribí 1 para el catálogo."`.
- `step == "entrega"`: si `t` matchea `\b(retir|local|paso|busco)` → `orderDone(s, "Retiro por el local")`. Si `t` matchea `\b(envi|mand|correo)` → `step="localidad"`, texto `"Dale. ¿A qué localidad te lo mando?"`. Si no → `"¿Retiro o envío?"`.
- `step == "localidad"`: `orderDone(s, "Envío a " + title(t.strip()) + " (costo a confirmar)")`.

Todos los textos usan `ok(...)` (resetea `fails`, `paused=false`, sin `order`).

### 6.7 `orderDone(s, entrega)` — cierre de reserva
`num = 1000 + sessions.size()`. Con `o = offer`:
```
resumen = "🛒 Reserva #" + num + "\n" + o.name + " · " + o.color + " · talle " + o.talle
          + "\n" + Ars.format(o.price) + "\n" + entrega
link    = "https://mpago.la/demo-" + num
texto   = resumen + "\n\nTe dejo el link de pago de Mercado Pago:\n" + link
          + "\n\nCon el pago confirmado te aviso por acá. ¡Gracias! 💛" + VOLVER
```
(U+1F6D2 🛒, U+1F49B 💛.) `step=null`. Devuelve `Reply([Msg(texto, null)], paused=false, order = resumen.replace("\n", " · "))`. `order` es la única vía por la que `Reply.order()` sale no-null.

## 7. Textos literales (byte-exactos)

Copia exacta de `demo.py`. Los emojis son parte del string; no reemplazar ni "mejorar" la puntuación.

```java
HORARIO    = "Atendemos de lunes a viernes de 8 a 17 hs y sabados de 9 a 15 hs. Domingos y feriados cerrado. Por aca me podés escribir a cualquier hora 😊"   // U+1F60A
ENVIOS     = "Hacemos envíos a todo el país 🚚. El costo depende de tu localidad. Decime a qué ciudad sería y te paso el valor."                                  // U+1F69A
MAYORISTA  = "¡Buenísimo! Trabajamos con revendedores en todo el país. Te paso con una persona del local para que te arme la lista mayorista. Un momento 👍"      // U+1F44D
HANDOFF    = "Dale, te paso con una persona del local. En un ratito te escribe 🙌"                                                                                 // U+1F64C
MENU       = "1️⃣ Ver catálogo y stock\n2️⃣ Horarios y envíos\n3️⃣ Soy revendedor / mayorista\n4️⃣ Hablar con una persona\n\nEscribime el número 👇"
SALUDO     = "¡Hola! Soy el asistente de Que Te Pario 👟. ¿En qué te ayudo?\n\n" + MENU                                                                            // U+1F45F
NOENTIENDO = "No te entendí bien 😅. Elegí una opción:\n\n" + MENU                                                                                                 // U+1F605
VOLVER     = "\n\n(Escribí 0 para volver al menú)"
```

Detalles que se rompen si se "arreglan":
- `HORARIO` dice `sabados` y `aca` **sin** acento, pero `podés` **con** acento.
- `MENU` usa keycaps: cada `N️⃣` = dígito ASCII + U+FE0F (variation selector-16) + U+20E3 (combining enclosing keycap). Tres code points, no un emoji precompuesto. En Java: `"1\uFE0F\u20E3 Ver catálogo..."` etc.
- Emojis fuera del BMP (👟 U+1F45F, 😊 U+1F60A, 🚚 U+1F69A, 👍 U+1F44D, 🙌 U+1F64C, 😅 U+1F605, 👇 U+1F447, 😔 U+1F614, 🛒 U+1F6D2, 💛 U+1F49B): en Java son pares surrogados, el archivo fuente debe ser UTF-8 y el `pom.xml` debe fijar `project.build.sourceEncoding=UTF-8`.
- El guion em `—` (U+2014) de las líneas del catálogo y el `·` (U+00B7) de resúmenes/porColor son literales, no `-` ni `/`.

## 8. Trampas del port (leer antes de codificar)

1. **NFD de verdad.** No alcanza con `replace('á','a')`: hay que normalizar a NFD, filtrar marcas `Mn` y **después** aplicar `[^a-z0-9/ ]`. Si se invierte el orden, los acentos descompuestos sobreviven y los regex con `\b` fallan silenciosamente. Usar `Locale.ROOT` en `toLowerCase`.

2. **`SequenceMatcher.ratio()` no es una distancia conocida.** Portar Ratcliff–Obershelp con el tie-break de `find_longest_match`. Un `ratio` aproximado (Jaro, Levenshtein normalizado, `FuzzyScore`) cambia los `hits` y hace fallar `findProduct` (p. ej. `tenes borcegos en 38`).

3. **`\b` sobre texto normalizado.** Las únicas "no-palabras" posibles son espacio, `/` y extremos. Por eso `\brevend` matchea `revendedor` y `\bhorario` matchea `horarios` (ancla solo al inicio). Varios patrones están deliberadamente abiertos por la derecha (`\bhorario`, `\bretir`, `\benvi`, los `CATS[k]`). En Java `\b` es Unicode-aware, pero el texto es ASCII tras `norm`, así que se comporta igual.

4. **Precio ARS.** `"$ " + n con separador de miles "." y 0 decimales`. `String.format(Locale.US, "%,.0f")` produce `1,234` → hay que post-reemplazar `,` por `.`. Alternativa: `DecimalFormat("#,##0", symbols)` con `groupingSeparator='.'`. Ojo con la regla de redondeo en empates exactos `.5` (Python usa half-to-even en `format`); para el catálogo real no se observan empates, pero documentarlo.

5. **Orden de `findColor`.** `colors` es un `set` Python ordenado por longitud desc con `sorted` estable; para colores de igual longitud el orden depende del hash de `str` (aleatorio entre procesos). Asumimos que un mismo `t` no matchea dos stems de igual longitud a la vez. El port debe ser determinista (longitud desc, luego orden de aparición); no debe "inventar" un orden distinto si eso rompiera el test.

6. **Empates en `findProduct`.** `max(..., key=(hits, -len))` devuelve el primer máximo en orden de iteración. El port debe comparar con `>` estricto y recorrer en orden de inserción del catálogo. Usar `LinkedHashMap<String, Product>` (último gana si hay nombres normalizados duplicados).

7. **`isdigit()` vs `Numeric`.** Tras `norm`, `n` es ASCII; usar `n.matches("\\d+")` o `codePoints().allMatch(Character::isDigit)`. No confundir con `Integer.parseInt` sobre strings vacíos.

8. **Sesiones y numeración de reservas.** El contador es `1000 + sessions.size()` (incluye sesiones pausadas y de test). `Map` con inserción estable; `size()` cuenta todas.

9. **`ok` resetea `fails`.** Todo camino que devuelve `ok(...)` (horario, envíos, saludo, menú, `2`, respuestas de stock, pasos de reserva) pone `fails=0`. `catalogReply` y la selección numérica de catálogo también.

10. **`step` gate.** Mientras hay `step` activo, **todo** input que no sea `"0"` o `"menu"` va a `orderStep` **antes** que cualquier intent. Esto incluye saludos. `"0"`/`"menu"` cancelan el paso (regla 7) y vuelven al menú.

11. **Fotos de variante.** En `productReply`, `images[1:4]` = índices 1, 2, 3 (hasta 3 adicionales). El `Msg` de foto extra lleva `text=""` (no null). Si el producto tiene 1 sola imagen, `replies().size() == 1`; el test exige `>= 2` para el primer producto del catálogo real (tiene varias).

12. **`cats ∩ p.cats`** compara contra los nombres originales del JSON (no normalizados). El filtro de categoría usa las claves de `CATS` normalizadas por `norm` del texto del usuario.

13. **`Offer.price` es `double`.** El resumen usa `Ars.format`, no el precio crudo.

14. **`Reply.order()`** es `null` salvo en el cierre de reserva; el test lo verifica implícitamente al no romper con `paused`/replies vacías.

15. **Encoding del fuente.** Emojis y acentos en el `.java`: `UTF-8` obligatorio en `pom.xml`, editor y build. Un archivo en Latin-1 corrompe `MENU`/`SALUDO` y rompe las aserciones de strings.

## 9. Aceptación

Comando: `cd backend && mvn -q -B test`.

Condición: `BotAcceptanceTest.java` (sin modificar) en verde. Los tests cubren, en orden: flujo principal (saludo, horarios, envíos, stock con imagen, color, follow-up de talle, handoff humano, handoff mayorista, 3 fallos → pausa); catálogo (lista única, detalle con fotos, filtro por categoría, `NOENTIENDO`/`MENU`, `4`); `stockYGenerico` (constantes sin "Maxi"); talles (`_por_color` con `"Amarillo ("`, oferta `"¡Sí!"` con `"39/40"`); reserva (`zuecos rayadas 40` → `"¡Sí!"`, `si` → `"retir"`).

Cualquier divergencia con `demo.py` es un bug del port, no del test.

## 10. No objetivos

Lo de §1 fuera de alcance. Adicionalmente: no persistir sesiones, no exponer HTTP, no portar el servidor web ni el HTML del panel, no portar `fetch_catalog/--refresh` (el `catalogo.json` ya existe en `reference/`), no portar `_stock_line` (código muerto en el prototipo, no se invoca).