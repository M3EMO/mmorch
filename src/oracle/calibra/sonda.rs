//! Sonda de mutación: inserta un `panic!` al inicio del cuerpo de una fn.
//! Función pura. Lee el texto y devuelve una copia con la sonda, sin tocar archivos.

/// Sentencia que la sonda inserta como primera del cuerpo.
const SENTENCIA_SONDA: &str = "panic!(\"root-sonda\");";

/// Devuelve `fuente` con la sonda al inicio del cuerpo de la fn que contiene `linea`.
/// `linea` es 1-indexada. `None` si la línea no está en una fn con cuerpo.
#[allow(dead_code)]
pub(super) fn con_panic_en_funcion(fuente: &str, linea: usize) -> Option<String> {
    let chars: Vec<char> = fuente.chars().collect();
    let codigo = enmascarar(&chars);
    let saltos: Vec<usize> = (0..chars.len()).filter(|&i| chars[i] == '\n').collect();

    // Gana la fn que empieza más tarde: es la más interna que contiene la línea.
    let mut elegida: Option<(usize, usize)> = None;
    for kw in 0..codigo.len() {
        if !es_palabra_fn(&codigo, kw) {
            continue;
        }
        let Some((apertura, cierre)) = cuerpo_de_fn(&codigo, kw) else {
            continue;
        };
        let inicio_linea = linea_de(&saltos, kw);
        let fin_linea = linea_de(&saltos, cierre);
        if inicio_linea <= linea && linea <= fin_linea && elegida.is_none_or(|(k, _)| kw > k) {
            elegida = Some((kw, apertura));
        }
    }

    let (kw, apertura) = elegida?;
    let sangria = sangria_de_linea(&chars, kw);
    let fin_de_linea = if fuente.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };

    let mut salida = String::with_capacity(fuente.len() + SENTENCIA_SONDA.len() + 16);
    salida.extend(&chars[..=apertura]);
    salida.push_str(fin_de_linea);
    salida.push_str(&sangria);
    salida.push_str(SENTENCIA_SONDA);
    salida.extend(&chars[apertura + 1..]);
    Some(salida)
}

/// Número de línea (1-indexada) del carácter `idx`, según las posiciones de los `\n`.
fn linea_de(saltos: &[usize], idx: usize) -> usize {
    saltos.partition_point(|&s| s < idx) + 1
}

/// Sangría de la línea donde empieza `kw`, más un nivel de cuatro espacios.
fn sangria_de_linea(chars: &[char], kw: usize) -> String {
    let inicio = chars[..kw]
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |p| p + 1);
    let base: String = chars[inicio..]
        .iter()
        .take_while(|&&c| c == ' ' || c == '\t')
        .collect();
    base + "    "
}

/// Verdadero si `codigo[kw..]` empieza con la palabra `fn` como token suelto.
fn es_palabra_fn(codigo: &[char], kw: usize) -> bool {
    let antes_ok = kw == 0 || !es_identificador(codigo[kw - 1]);
    antes_ok
        && codigo.get(kw..kw + 2) == Some(&['f', 'n'][..])
        && codigo.get(kw + 2).is_some_and(|c| c.is_whitespace())
}

fn es_identificador(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Índices de la `{` de apertura y de la `}` de cierre del cuerpo de la fn en `kw`.
/// `None` si no hay nombre tras `fn` (tipo `fn(..)`), si no hay cuerpo o si está desbalanceada.
fn cuerpo_de_fn(codigo: &[char], kw: usize) -> Option<(usize, usize)> {
    let n = codigo.len();
    let mut k = kw + 2;
    while k < n && codigo[k].is_whitespace() {
        k += 1;
    }
    if k >= n || !es_identificador(codigo[k]) {
        return None;
    }

    let (mut parentesis, mut corchetes, mut angulos) = (0usize, 0usize, 0usize);
    while k < n {
        let fuera = parentesis == 0 && corchetes == 0 && angulos == 0;
        match codigo[k] {
            '(' => parentesis += 1,
            ')' => parentesis = parentesis.checked_sub(1)?,
            '[' => corchetes += 1,
            ']' => corchetes = corchetes.checked_sub(1)?,
            '<' => angulos += 1,
            // La flecha `->` no cierra un ángulo.
            '>' if k > 0 && codigo[k - 1] == '-' => {}
            '>' => angulos = angulos.saturating_sub(1),
            '{' if fuera => return cierre_de_llave(codigo, k).map(|c| (k, c)),
            ';' | '}' if fuera => return None,
            _ => {}
        }
        k += 1;
    }
    None
}

/// Índice de la `}` que cierra la `{` en `apertura`. `None` si no cierra.
fn cierre_de_llave(codigo: &[char], apertura: usize) -> Option<usize> {
    let mut profundidad = 0usize;
    for (i, &c) in codigo.iter().enumerate().skip(apertura) {
        match c {
            '{' => profundidad += 1,
            '}' => {
                profundidad -= 1;
                if profundidad == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Copia de `chars` donde los comentarios, strings y chars quedan en blanco.
/// Los saltos de línea se conservan para que los números de línea no cambien.
fn enmascarar(chars: &[char]) -> Vec<char> {
    let n = chars.len();
    let mut salida = chars.to_vec();
    let mut i = 0;
    while i < n {
        let c = chars[i];
        let siguiente = chars.get(i + 1).copied();
        let fin = if c == '/' && siguiente == Some('/') {
            // Comentario de línea: hasta el salto de línea.
            Some((i..n).find(|&j| chars[j] == '\n').unwrap_or(n))
        } else if c == '/' && siguiente == Some('*') {
            Some(fin_de_comentario_de_bloque(chars, i))
        } else if let Some(fin_raw) = fin_de_string_raw(chars, i) {
            Some(fin_raw)
        } else if c == '"' {
            Some(fin_de_string(chars, i))
        } else if c == '\'' {
            fin_de_char(chars, i)
        } else {
            None
        };
        match fin {
            Some(hasta) => {
                blanquear(&mut salida, i, hasta);
                i = hasta;
            }
            None => i += 1,
        }
    }
    salida
}

/// Pone en blanco `salida[desde..hasta]`, salvo los saltos de línea.
fn blanquear(salida: &mut [char], desde: usize, hasta: usize) {
    for c in &mut salida[desde..hasta] {
        if *c != '\n' {
            *c = ' ';
        }
    }
}

/// Fin (exclusivo) de un comentario de bloque anidado que empieza en `i`.
fn fin_de_comentario_de_bloque(chars: &[char], i: usize) -> usize {
    let n = chars.len();
    let mut profundidad = 0usize;
    let mut j = i;
    while j < n {
        if chars[j] == '/' && chars.get(j + 1) == Some(&'*') {
            profundidad += 1;
            j += 2;
        } else if chars[j] == '*' && chars.get(j + 1) == Some(&'/') {
            profundidad -= 1;
            j += 2;
            if profundidad == 0 {
                break;
            }
        } else {
            j += 1;
        }
    }
    j.min(n)
}

/// Fin (exclusivo) de un string raw `r#"..."#` o `br"..."` que empieza en `i`. `None` si no empieza uno.
fn fin_de_string_raw(chars: &[char], i: usize) -> Option<usize> {
    let n = chars.len();
    let prefijo = if chars[i] == 'b' { i + 1 } else { i };
    if chars.get(prefijo) != Some(&'r') {
        return None;
    }
    if i > 0 && es_identificador(chars[i - 1]) {
        return None;
    }
    let mut comillas = prefijo + 1;
    while chars.get(comillas) == Some(&'#') {
        comillas += 1;
    }
    if chars.get(comillas) != Some(&'"') {
        return None;
    }
    let hashes = comillas - prefijo - 1;
    let mut j = comillas + 1;
    while j < n {
        let cierra = chars[j] == '"' && (1..=hashes).all(|h| chars.get(j + h) == Some(&'#'));
        if cierra {
            return Some(j + 1 + hashes);
        }
        j += 1;
    }
    Some(n)
}

/// Fin (exclusivo) de un string normal que empieza en la `"` de `i`.
fn fin_de_string(chars: &[char], i: usize) -> usize {
    let n = chars.len();
    let mut j = i + 1;
    while j < n {
        match chars[j] {
            '\\' => j += 2,
            '"' => return j + 1,
            _ => j += 1,
        }
    }
    n
}

/// Fin (exclusivo) de un char literal que empieza en la `'` de `i`.
/// `None` si es un lifetime, que no se enmascara.
fn fin_de_char(chars: &[char], i: usize) -> Option<usize> {
    let n = chars.len();
    if chars.get(i + 1) == Some(&'\\') {
        let mut j = i + 3;
        while j < n && chars[j] != '\'' {
            j += 1;
        }
        return Some((j + 1).min(n));
    }
    if chars.get(i + 2) == Some(&'\'') {
        return Some(i + 3);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::con_panic_en_funcion;

    #[test]
    fn firma_de_una_linea() {
        let fuente = "fn uno() -> i32 {\n    1\n}\n";
        let esperado = "fn uno() -> i32 {\n    panic!(\"root-sonda\");\n    1\n}\n";
        assert_eq!(con_panic_en_funcion(fuente, 1), Some(esperado.to_string()));
        assert_eq!(con_panic_en_funcion(fuente, 2), Some(esperado.to_string()));
    }

    #[test]
    fn firma_de_varias_lineas() {
        let fuente = "fn varias(\n    a: i32,\n    b: i32,\n) -> i32 {\n    a + b\n}\n";
        let esperado = "fn varias(\n    a: i32,\n    b: i32,\n) -> i32 {\n    panic!(\"root-sonda\");\n    a + b\n}\n";
        assert_eq!(con_panic_en_funcion(fuente, 2), Some(esperado.to_string()));
        assert_eq!(con_panic_en_funcion(fuente, 5), Some(esperado.to_string()));
    }

    #[test]
    fn clausula_where_en_una_linea() {
        let fuente = "fn con_where<T>(t: T) -> i32 where T: Fn() -> i32 {\n    t()\n}\n";
        let esperado = "fn con_where<T>(t: T) -> i32 where T: Fn() -> i32 {\n    panic!(\"root-sonda\");\n    t()\n}\n";
        assert_eq!(con_panic_en_funcion(fuente, 2), Some(esperado.to_string()));
    }

    #[test]
    fn clausula_where_en_varias_lineas() {
        let fuente = "fn con_where<T>(t: T) -> i32\nwhere\n    T: Fn() -> i32,\n{\n    t()\n}\n";
        let esperado = "fn con_where<T>(t: T) -> i32\nwhere\n    T: Fn() -> i32,\n{\n    panic!(\"root-sonda\");\n    t()\n}\n";
        assert_eq!(con_panic_en_funcion(fuente, 5), Some(esperado.to_string()));
    }

    #[test]
    fn retorno_impl_fn_con_flecha() {
        let fuente = "fn fabrica() -> impl Fn() -> i32 {\n    || 1\n}\n";
        let esperado =
            "fn fabrica() -> impl Fn() -> i32 {\n    panic!(\"root-sonda\");\n    || 1\n}\n";
        assert_eq!(con_panic_en_funcion(fuente, 2), Some(esperado.to_string()));
    }

    #[test]
    fn firma_con_lifetimes() {
        let fuente = "fn con_lifetime<'a>(x: &'a str) -> &'a str {\n    x\n}\n";
        let esperado =
            "fn con_lifetime<'a>(x: &'a str) -> &'a str {\n    panic!(\"root-sonda\");\n    x\n}\n";
        assert_eq!(con_panic_en_funcion(fuente, 2), Some(esperado.to_string()));
    }

    #[test]
    fn fn_anidada_gana_la_interna() {
        let fuente = "fn externa() -> i32 {\n    fn interna() -> i32 {\n        1\n    }\n    interna() + 2\n}\n";
        let esperado_interna = "fn externa() -> i32 {\n    fn interna() -> i32 {\n        panic!(\"root-sonda\");\n        1\n    }\n    interna() + 2\n}\n";
        let esperado_externa = "fn externa() -> i32 {\n    panic!(\"root-sonda\");\n    fn interna() -> i32 {\n        1\n    }\n    interna() + 2\n}\n";
        assert_eq!(
            con_panic_en_funcion(fuente, 3),
            Some(esperado_interna.to_string())
        );
        assert_eq!(
            con_panic_en_funcion(fuente, 5),
            Some(esperado_externa.to_string())
        );
    }

    #[test]
    fn linea_fuera_de_toda_fn_da_none() {
        let fuente = "const X: i32 = 1;\n\nfn f() -> i32 {\n    X\n}\n";
        assert_eq!(con_panic_en_funcion(fuente, 1), None);
        assert_eq!(con_panic_en_funcion(fuente, 2), None);
    }

    #[test]
    fn fn_sin_cuerpo_da_none() {
        let fuente = "trait T {\n    fn sin_cuerpo(&self);\n}\n";
        assert_eq!(con_panic_en_funcion(fuente, 2), None);
    }

    #[test]
    fn tipo_fn_no_confunde_el_cuerpo() {
        let fuente = "struct S {\n    f: fn(i32) -> i32,\n}\nfn real() -> i32 {\n    1\n}\n";
        let esperado = "struct S {\n    f: fn(i32) -> i32,\n}\nfn real() -> i32 {\n    panic!(\"root-sonda\");\n    1\n}\n";
        assert_eq!(con_panic_en_funcion(fuente, 5), Some(esperado.to_string()));
    }

    #[test]
    fn llaves_en_strings_y_comentarios_no_confunden() {
        let fuente = r##"fn con_ruido() -> &'static str {
    // cierra } y abre {
    let s = "} { fn falsa() {";
    /* { ... } */
    let c = '}';
    let r = r#"{ " }"#;
    "ok"
}
"##;
        let esperado = r##"fn con_ruido() -> &'static str {
    panic!("root-sonda");
    // cierra } y abre {
    let s = "} { fn falsa() {";
    /* { ... } */
    let c = '}';
    let r = r#"{ " }"#;
    "ok"
}
"##;
        assert_eq!(con_panic_en_funcion(fuente, 7), Some(esperado.to_string()));
    }
}
