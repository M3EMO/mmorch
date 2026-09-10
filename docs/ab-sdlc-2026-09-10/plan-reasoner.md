# plan.md — Motor del agente (port de `demo.py` a Java 17)

Objetivo: jar plano de Maven en `backend/` sin Spring que reemplace al prototipo Python. Oráculo normativo: `backend/reference/demo.py`. Ante duda, gana el Python línea por línea. Entrada esperada: `BotAcceptanceTest.java` (ya en el repo, no se toca) en verde.

## Archivos

Orden de construcción (de abajo hacia arriba: utilidades → datos → motor).

1. `pom.xml` — Maven plano: `release=17`, `project.build.sourceEncoding=UTF-8`, deps `jackson-databind` + `junit-jupiter` (test), surefire ≥ 3.0 para JUnit 5. Working dir del test = `backend/`.
2. `src/main/java/com/qtp/bot/SeqMatch.java` — `ratio(a,b)=2M/(|a|+|b|)` portando Ratcliff–Obershelp (`find_longest_match` recursivo, tie-break: mayor tamaño → menor i → menor j). Sin dependencias externas.
3. `src/main/java/com/qtp/bot/Norm.java` — `apply(String)`: `toLowerCase(Locale.ROOT)` → NFD → filtrar code points tipo `NON_SPACING_MARK` → `replaceAll("[^a-z0-9/ ]","")`, en ese orden exacto.
4. `src/main/java/com/qtp/bot/Ars.java` — `format(double)`: `"$ " + n` con 0 decimales, separador de miles `.`, redondeo a entero (`1234.56 → "$ 1.235"`). Usar `DecimalFormat` con `groupingSeparator='.'` o `%,.0f` + replace.
5. `src/main/java/com/qtp/bot/Variant.java` — inmutable `{color,talle,stock}`, `color`/`talle` default `"?"` si faltan (Jackson `@JsonProperty`).
6. `src/main/java/com/qtp/bot/Product.java` — inmutable `{name,price,regular,images,cats,variants}`; `images`/`cats` pueden venir vacíos, `variants` nunca null.
7. `src/main/java/com/qtp/bot/Msg.java` — `text()` nunca null (puede `""`), `image()` nullable.
8. `src/main/java/com/qtp/bot/Reply.java` — `replies()` nunca null (puede vacía), `paused()`, `order()` nullable (solo no-null al cierre de reserva).
9. `src/main/java/com/qtp/bot/Catalog.java` — `static List<Product> load(Path)` vía Jackson, preserva insertion order del array (usado por `findProduct`).
10. `src/main/java/com/qtp/bot/Bot.java` — motor completo: constantes literales (`MENU/HANDOFF/MAYORISTA/SALUDO/HORARIO/ENVIOS/NOENTIENDO/VOLVER`), `Map<String,Session>` estable, `reply(sid,text)` con las 19 reglas en orden exacto, helpers `findProduct`/`findColor`/`stockAnswer`/`catalogReply`/`productReply`/`orderStep`/`orderDone`/`ok`, utilidades `talles`/`colors`/`porColor`/`alts`/`shown`/`title`, y `Session`+`Offer` internos. Único consumidor del catálogo.

Archivos del oráculo que **no** se tocan: `reference/demo.py`, `reference/test_demo.py`, `reference/catalogo.json`, `src/test/java/com/qtp/bot/BotAcceptanceTest.java`.

## Riesgos

- **Normalización fuera de orden (§8.1):** si se filtra `[^a-z0-9/ ]` antes de sacar las marcas `Mn`, los acentos descompuestos sobreviven y los `\b` fallan en silencio. Respetar el orden toLowerCase→NFD→Mn→regex, con `Locale.ROOT`.
- **`SeqMatch` aproximado (§8.2):** cualquier atajo (Levenshtein, Jaro, FuzzyScore) cambia los `hits` de `findProduct` y rompe p. ej. `"tenes borcegos en 38"`. Portar `find_longest_match` con el tie-break exacto.
- **`\b` sobre texto normalizado (§8.3):** tras `norm` el texto es ASCII, los patrones deliberadamente abiertos por derecha (`\bhorario`, `\bretir`, `\benvi`, `CATS[k]`) dependen de eso. No "cerrarlos".
- **Precio ARS (§8.4):** `String.format("%,.0f")` usa `,` → post-reemplazar o `DecimalFormat` con `groupingSeparator='.'`. Documentar half-to-even en `.5` (no observado en el catálogo real).
- **Orden de `findColor` (§8.5):** ser determinista (longitud desc, luego orden de aparición). No inventar otro orden que rompa el test.
- **Empates de `findProduct` (§8.6):** `LinkedHashMap` en orden de inserción, comparar con `>` estricto (primer máximo gana), dedup por `Norm.apply(name)`.
- **Gate de `step` (§8.10):** todo input que no sea `"0"`/`"menu"` pasa por `orderStep` antes que cualquier intent, incluidos saludos. Regla 1 antes que 2–6.
- **Encoding (§8.15):** `UTF-8` en `pom.xml`, editor y build. Un fuente en Latin-1 corrompe `MENU`/`SALUDO` y rompe aserciones; `MENU` usa keycaps de 3 code points (`\uFE0F\u20E3`), no emoji precompuesto.
- **Fotos de variante (§8.11):** `images[1:4]` = índices 1,2,3 (hasta 3 extras), `Msg.text=""` no null. El primer producto del catálogo real debe dar `replies().size() >= 2`.
- **Numeración de reservas (§8.8):** `1000 + sessions.size()`, cuenta todas las sesiones (incl. pausadas). `Offer.price` es `double`; el resumen usa `Ars.format`.
- **`cats ∩ p.cats` (§8.12):** comparar contra nombres originales del JSON; el match de categoría del usuario usa claves de `CATS` normalizadas con `Norm.apply`.
- **`isdigit` (§8.7):** usar `n.matches("\\d+")`; no `Integer.parseInt` sobre vacío.
- **Filtro de categoría `catalogReply` (§6.4):** el patrón `\b` + `k` ancla solo al inicio, sin `\b` final.

## Prueba

`cd backend && mvn -q -B test`

Condición de aceptación: `BotAcceptanceTest.java` (sin modificar) en verde. Cubre flujo principal (saludo, horarios, envíos, stock con imagen, color, follow-up de talle, handoff humano, handoff mayorista, 3 fallos→pausa), catálogo (lista única, detalle con fotos, filtro por categoría, `NOENTIENDO`/`MENU`, `4`), `stockYGenerico` (constantes sin "Maxi"), talles (`_por_color` con `"Amarillo ("`, oferta `"¡Sí!"` con `"39/40"`) y reserva (`zuecos rayadas 40` → `"¡Sí!"`, `si` → `"retir"`). Cualquier divergencia con `demo.py` es bug del port, no del test.