//! Juez de la configuración de Cargo (decisión 30). Para cada entrada de `.cargo/config.toml` o `config`,
//! dice si el proyecto la necesita: `necesaria`, `innecesaria` o `riesgosa`. Es un juicio: no decide qué
//! usa `cargo` ni abre o cierra la red. Sus hallazgos de entrega entran al informe.
//! Refuta por default y su JSON es estricto. Un juicio que no cita el config o el `Cargo.toml` se descarta.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::hash_corto;
use super::inventario::{EntradaConfig, Inventario, redactar};
use crate::config::Config;
use crate::oracle::ErrorOraculo;
use crate::oracle::juez::{JuezProveedor, ModeloJuez, desenvuelto};
use crate::oracle::tarea::Tarea;
use crate::proveedores::{MAX_TOKENS_JUEZ, Prompt, causa_tope_alcanzado};
use crate::route;
use crate::tipos::{Dimension, Eleccion, Fuente, Hallazgo, Proveedor, Rol, Via};

/// Fase del flujo que va al `Evento` de la llamada.
const FASE: &str = "descarga-juez-config";
/// Modelo que se muestra en la fuente del hallazgo: el juez no lo expone acá.
const MODELO_JUEZ_CONFIG: &str = "juez-config";
const ARCHIVO_CARGO: &str = "Cargo.toml";
const CAUSA_JSON: &str = "JSON inválido del juez";

/// Prompt fijo del juez. Refuta por default y pide solo un arreglo JSON.
const PROMPT_SISTEMA: &str = "Sos un revisor de la configuración de Cargo de un repo Rust. Decidís si cada entrada de .cargo/config.toml o de config hace falta para bajar y compilar las dependencias del proyecto.
REFUTA POR DEFAULT: ante la duda, `necesaria`. Solo marcá `innecesaria` o `riesgosa` con un motivo concreto que cite el config o el Cargo.toml.
- `necesaria`: el proyecto la usa, por ejemplo porque un crate de Cargo.toml vive en ese registro o en esa fuente.
- `innecesaria`: ningún crate de Cargo.toml ni fuente del inventario la necesita.
- `riesgosa`: cambia de dónde salen las dependencias o por dónde sale el tráfico de red, y eso puede traer código o tráfico que el proyecto no declara.
Cada respuesta cita la entrada por su clave y da el motivo. La cita es texto textual del config o del Cargo.toml, con los mismos caracteres.
No opinás sobre estilo. No tenés código fuente del proyecto.
Respondé SOLO un arreglo JSON con esta forma exacta:
[{\"clave\":\"<clave de la entrada>\",\"necesidad\":\"necesaria|innecesaria|riesgosa\",\"motivo\":\"...\",\"cita\":\"<texto textual del config o del Cargo.toml>\"}]";

/// Qué decide el juez sobre una entrada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Necesidad {
    Necesaria,
    Innecesaria,
    Riesgosa,
}

/// Un juicio que pasó el gate de evidencia: la entrada, el motivo y la cita textual.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JuicioEntrada {
    pub clave: String,
    pub necesidad: Necesidad,
    pub motivo: String,
    pub cita: String,
}

/// Resultado del juez de configuración. `SinVeredicto` no da hallazgos y no corta la descarga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResultadoJuicio {
    Juzgado {
        juicios: Vec<JuicioEntrada>,
        /// Claves de las respuestas que el gate descartó o que repiten una clave ya juzgada.
        descartados: Vec<String>,
    },
    SinVeredicto {
        causa: String,
    },
}

/// Arma el prompt del juez de configuración. Solo lleva las entradas, el `Cargo.toml` (redactado), las
/// fuentes del inventario y la ficha. Nunca código fuente del proyecto.
pub fn prompt_config(
    entradas: &[EntradaConfig],
    cargo_toml: &str,
    inv: &Inventario,
    ficha: Option<&str>,
) -> Prompt {
    let mut usuario = String::from("ENTRADAS DE CONFIG:\n");
    for entrada in entradas {
        usuario.push_str(&format!(
            "- {} | {}:{} | {}\n",
            entrada.clave,
            entrada.archivo,
            entrada.linea,
            redactar(&entrada.cita)
        ));
    }
    usuario.push_str("\nCARGO.TOML:\n");
    usuario.push_str(&redactar(cargo_toml));
    usuario.push_str("\nFUENTES DEL INVENTARIO:\n");
    if inv.fuentes.is_empty() {
        usuario.push_str("ninguna\n");
    }
    for fuente in &inv.fuentes {
        usuario.push_str(&format!(
            "- {:?} | {} | {}\n",
            fuente.tipo,
            redactar(&fuente.direccion),
            fuente.archivo
        ));
    }
    usuario.push_str("\nFICHA DEL SPEC:\n");
    usuario.push_str(&ficha.map(redactar).unwrap_or_else(|| "ninguna".into()));
    Prompt {
        sistema: Some(PROMPT_SISTEMA.into()),
        usuario,
        max_tokens: MAX_TOKENS_JUEZ,
        fase: FASE.into(),
    }
}

/// Pide el juicio de cada entrada y aplica el parseo estricto y el gate de evidencia.
/// `textos` son (archivo, contenido) del config y del `Cargo.toml`: la cita tiene que aparecer en uno.
pub fn juzgar_config(
    juez: &dyn ModeloJuez,
    entradas: &[EntradaConfig],
    textos: &[(String, String)],
    cargo_toml: &str,
    inv: &Inventario,
    ficha: Option<&str>,
) -> ResultadoJuicio {
    if entradas.is_empty() {
        return ResultadoJuicio::Juzgado {
            juicios: Vec::new(),
            descartados: Vec::new(),
        };
    }
    let prompt = prompt_config(entradas, cargo_toml, inv, ficha);
    let salida = match juez.consultar(&prompt) {
        Ok(salida) => salida,
        Err(error) => {
            return ResultadoJuicio::SinVeredicto {
                causa: redactar(&error.to_string()),
            };
        }
    };
    // Antes de parsear: una respuesta cortada por el tope no es JSON roto, es truncada.
    if salida.truncada {
        return ResultadoJuicio::SinVeredicto {
            causa: causa_tope_alcanzado(prompt.max_tokens),
        };
    }
    let crudos = match parsear(&salida.texto) {
        Ok(crudos) => crudos,
        Err(causa) => return ResultadoJuicio::SinVeredicto { causa },
    };
    let mut juicios: Vec<JuicioEntrada> = Vec::new();
    let mut descartados = Vec::new();
    for cruda in crudos {
        let valido =
            entradas.iter().any(|e| e.clave == cruda.clave) && cita_en_textos(&cruda.cita, textos);
        // Una clave repetida vale la primera que pasa el gate, como en el juez de diseño.
        if !valido || juicios.iter().any(|j| j.clave == cruda.clave) {
            descartados.push(cruda.clave);
            continue;
        }
        juicios.push(JuicioEntrada {
            clave: cruda.clave,
            necesidad: cruda.necesidad,
            motivo: redactar(&cruda.motivo),
            cita: redactar(&cruda.cita),
        });
    }
    ResultadoJuicio::Juzgado {
        juicios,
        descartados,
    }
}

/// Lee lo que el juez ve del repo (el config, el `Cargo.toml` y la ficha del spec) y juzga las entradas.
pub(super) fn juzgar_repo(
    juez: &dyn ModeloJuez,
    repo: &Path,
    tarea: &Tarea,
    inv: &Inventario,
) -> ResultadoJuicio {
    let textos = textos_de(repo, inv);
    let cargo_toml = textos
        .iter()
        .find(|(archivo, _)| archivo == ARCHIVO_CARGO)
        .map(|(_, texto)| texto.clone())
        .unwrap_or_default();
    let ficha = ficha_de(tarea);
    juzgar_config(
        juez,
        &inv.config_cargo,
        &textos,
        &cargo_toml,
        inv,
        ficha.as_deref(),
    )
}

/// Archivos del config de la entrada y el `Cargo.toml`, con su contenido. Los que no existen se omiten.
fn textos_de(repo: &Path, inv: &Inventario) -> Vec<(String, String)> {
    let mut archivos: Vec<String> = inv.config_cargo.iter().map(|e| e.archivo.clone()).collect();
    archivos.push(ARCHIVO_CARGO.into());
    archivos.sort();
    archivos.dedup();
    archivos
        .into_iter()
        .filter_map(|archivo| {
            let texto = fs::read_to_string(repo.join(&archivo)).ok()?;
            Some((archivo, texto))
        })
        .collect()
}

/// Ficha del spec de la tarea, si el spec la tiene. Sin ficha, el juez no ve ninguna.
fn ficha_de(tarea: &Tarea) -> Option<String> {
    let texto = fs::read_to_string(tarea.dir.join(&tarea.spec)).ok()?;
    crate::ficha::extraer(&texto)
        .ok()
        .map(|(_, bloque)| bloque.texto)
}

/// La cita vale si no está vacía y aparece textual en alguno de los textos del repo.
fn cita_en_textos(cita: &str, textos: &[(String, String)]) -> bool {
    !cita.trim().is_empty() && textos.iter().any(|(_, texto)| texto.contains(cita))
}

/// Forma de cada elemento del arreglo que devuelve el juez. Los campos que faltan quedan vacíos y el gate
/// los descarta. Un valor de `necesidad` desconocido rompe el arreglo entero.
#[derive(Debug, Deserialize)]
struct JuicioCrudo {
    #[serde(default)]
    clave: String,
    necesidad: Necesidad,
    #[serde(default)]
    motivo: String,
    #[serde(default)]
    cita: String,
}

/// Parseo estricto: el arreglo JSON, con o sin un bloque ```json que lo envuelva. Cualquier otro texto,
/// antes o después, da la causa fija.
fn parsear(texto: &str) -> Result<Vec<JuicioCrudo>, String> {
    serde_json::from_str(desenvuelto(texto)).map_err(|_| CAUSA_JSON.into())
}

/// Un hallazgo de entrega por cada `innecesaria` (severidad 1) o `riesgosa` (severidad 2), con la cita del juicio.
pub fn hallazgos_de_juicio(r: &ResultadoJuicio, entradas: &[EntradaConfig]) -> Vec<Hallazgo> {
    let ResultadoJuicio::Juzgado { juicios, .. } = r else {
        return Vec::new();
    };
    juicios
        .iter()
        .filter_map(|juicio| {
            let severidad = match juicio.necesidad {
                Necesidad::Necesaria => return None,
                Necesidad::Innecesaria => 1,
                Necesidad::Riesgosa => 2,
            };
            let entrada = entradas.iter().find(|e| e.clave == juicio.clave)?;
            let cita = redactar(&juicio.cita);
            Some(Hallazgo {
                dimension: Dimension::Entrega,
                severidad,
                archivo: entrada.archivo.clone(),
                linea: entrada.linea,
                detalle: format!(
                    "el juez considera {} la entrada {}: {}",
                    etiqueta(juicio.necesidad),
                    juicio.clave,
                    redactar(&juicio.motivo)
                ),
                id: hash_corto(format!("entrega|{}|{}", entrada.archivo, cita).as_bytes()),
                cita,
                fuente: Fuente::Juez {
                    modelo: MODELO_JUEZ_CONFIG.into(),
                },
                impacto: Vec::new(),
            })
        })
        .collect()
}

fn etiqueta(necesidad: Necesidad) -> &'static str {
    match necesidad {
        Necesidad::Necesaria => "necesaria",
        Necesidad::Innecesaria => "innecesaria",
        Necesidad::Riesgosa => "riesgosa",
    }
}

/// Juez real de la configuración: el modelo de `[jueces].config_modelo`, vía API y con razonamiento
/// (decisión 47). El proveedor sale del nombre. La llamada pasa por `proveedores::llamar`, que chequea
/// el tope y el precio: sin precio, el juez da sin veredicto y la descarga sigue (decisión 35).
pub fn juez_config_real(cfg: &Config) -> Result<JuezProveedor, ErrorOraculo> {
    let modelo = cfg.jueces.config_modelo.clone();
    let eleccion = Eleccion {
        via: Via::Api,
        proveedor: Some(proveedor_de_modelo(&modelo)?),
        modelo: Some(modelo),
        razonamiento: true,
        motivo: "juez de la configuración: [jueces].config_modelo".into(),
        rol: Some(Rol::Juzga),
    };
    route::exigir_modelo_que_razona(&eleccion)?;
    JuezProveedor::nuevo(eleccion, FASE.into())
}

/// Proveedor de un modelo por su nombre: `claude-*` es Anthropic y `deepseek-*` es DeepSeek.
fn proveedor_de_modelo(modelo: &str) -> Result<Proveedor, ErrorOraculo> {
    if modelo.starts_with("claude-") {
        Ok(Proveedor::Anthropic)
    } else if modelo.starts_with("deepseek-") {
        Ok(Proveedor::Deepseek)
    } else {
        Err(ErrorOraculo::Eleccion(format!(
            "[jueces].config_modelo {modelo:?} no dice el proveedor: el nombre debe empezar con claude- o deepseek-"
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::proveedores::RespuestaJuez;

    const CARGO_TOML: &str = "[package]\nname = \"dep-prueba\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nhumantime = \"2\"\n";
    const CONFIG_PROXY: &str = "[http]\nproxy = \"http://127.0.0.1:9\"\n";
    const CITA_PROXY: &str = "proxy = \"http://127.0.0.1:9\"";
    const CONFIG_GIT: &str = "[net]\ngit-fetch-with-cli = true\n";

    /// Juez falso: responde un texto fijo (o falla) y cuenta las llamadas.
    struct Falso {
        respuesta: Result<RespuestaJuez, String>,
        llamadas: Cell<u32>,
    }

    impl Falso {
        fn responde(texto: &str) -> Self {
            Falso {
                respuesta: Ok(RespuestaJuez {
                    texto: texto.into(),
                    truncada: false,
                    costo_usd: 0.0,
                }),
                llamadas: Cell::new(0),
            }
        }

        /// Respuesta cortada por el tope de salida, con el texto que se le pase.
        fn truncada_con(texto: &str) -> Self {
            Falso {
                respuesta: Ok(RespuestaJuez {
                    texto: texto.into(),
                    truncada: true,
                    costo_usd: 0.0,
                }),
                llamadas: Cell::new(0),
            }
        }

        fn falla(causa: &str) -> Self {
            Falso {
                respuesta: Err(causa.into()),
                llamadas: Cell::new(0),
            }
        }
    }

    impl ModeloJuez for Falso {
        fn consultar(&self, _prompt: &Prompt) -> Result<RespuestaJuez, ErrorOraculo> {
            self.llamadas.set(self.llamadas.get() + 1);
            self.respuesta.clone().map_err(ErrorOraculo::Infra)
        }
    }

    /// Entrada de config de prueba. `seccion` es lo que va antes del punto de `clave`.
    fn entrada(clave: &str, linea: u32, cita: &str) -> EntradaConfig {
        let seccion = clave.split('.').next().unwrap_or_default().to_string();
        EntradaConfig {
            archivo: ".cargo/config.toml".into(),
            linea,
            seccion,
            clave: clave.into(),
            red: true,
            cita: cita.into(),
        }
    }

    fn inventario_con(entradas: Vec<EntradaConfig>) -> Inventario {
        Inventario {
            config_cargo: entradas,
            ..Inventario::default()
        }
    }

    /// Textos del gate: el config dado y el `Cargo.toml` de prueba.
    fn textos(config: &str) -> Vec<(String, String)> {
        vec![
            (".cargo/config.toml".into(), config.into()),
            (ARCHIVO_CARGO.into(), CARGO_TOML.into()),
        ]
    }

    fn respuesta_proxy(necesidad: &str, cita: &str) -> String {
        format!(
            "[{{\"clave\":\"http.proxy\",\"necesidad\":\"{necesidad}\",\"motivo\":\"ningún crate usa el proxy\",\"cita\":\"{cita}\"}}]"
        )
    }

    #[test]
    fn juez_config_innecesaria_da_hallazgo_de_entrega_y_no_toca_el_inventario() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let antes = inv.clone();
        let juez = Falso::responde(
            r#"[{"clave":"http.proxy","necesidad":"innecesaria","motivo":"ningún crate usa el proxy","cita":"proxy = \"http://127.0.0.1:9\""}]"#,
        );

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_PROXY),
            CARGO_TOML,
            &inv,
            None,
        );

        let ResultadoJuicio::Juzgado {
            juicios,
            descartados,
        } = &r
        else {
            panic!("se esperaba un juicio: {r:?}");
        };
        assert_eq!(juicios.len(), 1, "{juicios:?}");
        assert!(descartados.is_empty(), "{descartados:?}");
        let hallazgos = hallazgos_de_juicio(&r, &inv.config_cargo);
        assert_eq!(hallazgos.len(), 1, "{hallazgos:?}");
        assert_eq!(hallazgos[0].dimension, Dimension::Entrega);
        assert_eq!(hallazgos[0].severidad, 1);
        assert_eq!(hallazgos[0].archivo, ".cargo/config.toml");
        assert_eq!(hallazgos[0].linea, 2);
        assert_eq!(hallazgos[0].cita, CITA_PROXY);
        assert_eq!(juez.llamadas.get(), 1);
        assert_eq!(inv, antes, "el inventario no cambia según el juicio");
    }

    #[test]
    fn juez_config_json_roto_da_sin_veredicto() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let juez = Falso::responde("no es json [");

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_PROXY),
            CARGO_TOML,
            &inv,
            None,
        );

        assert_eq!(
            r,
            ResultadoJuicio::SinVeredicto {
                causa: CAUSA_JSON.into()
            }
        );
        assert!(hallazgos_de_juicio(&r, &inv.config_cargo).is_empty());
    }

    #[test]
    fn juez_config_texto_antes_o_despues_del_arreglo_da_sin_veredicto() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let json = respuesta_proxy("innecesaria", CITA_PROXY);
        for texto in [format!("Aquí va: {json}"), format!("{json} listo")] {
            let juez = Falso::responde(&texto);
            let r = juzgar_config(
                &juez,
                &inv.config_cargo,
                &textos(CONFIG_PROXY),
                CARGO_TOML,
                &inv,
                None,
            );
            assert_eq!(
                r,
                ResultadoJuicio::SinVeredicto {
                    causa: CAUSA_JSON.into()
                },
                "texto: {texto}"
            );
        }
    }

    #[test]
    fn juez_config_error_del_proveedor_da_sin_veredicto() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let juez = Falso::falla("proveedor caído");

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_PROXY),
            CARGO_TOML,
            &inv,
            None,
        );

        match &r {
            ResultadoJuicio::SinVeredicto { causa } => {
                assert!(causa.contains("proveedor caído"), "{causa}")
            }
            otro => panic!("se esperaba sin veredicto: {otro:?}"),
        }
        assert!(hallazgos_de_juicio(&r, &inv.config_cargo).is_empty());
    }

    #[test]
    fn juez_config_sin_cita_se_descarta() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let juez = Falso::responde(&respuesta_proxy("innecesaria", "texto que no está"));

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_PROXY),
            CARGO_TOML,
            &inv,
            None,
        );

        assert_eq!(
            r,
            ResultadoJuicio::Juzgado {
                juicios: Vec::new(),
                descartados: vec!["http.proxy".into()],
            }
        );
        assert!(hallazgos_de_juicio(&r, &inv.config_cargo).is_empty());
    }

    #[test]
    fn juez_config_clave_inexistente_se_descarta() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let juez = Falso::responde(
            r#"[{"clave":"http.inventada","necesidad":"innecesaria","motivo":"no existe","cita":"proxy = \"http://127.0.0.1:9\""}]"#,
        );

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_PROXY),
            CARGO_TOML,
            &inv,
            None,
        );

        assert_eq!(
            r,
            ResultadoJuicio::Juzgado {
                juicios: Vec::new(),
                descartados: vec!["http.inventada".into()],
            }
        );
        assert!(hallazgos_de_juicio(&r, &inv.config_cargo).is_empty());
    }

    #[test]
    fn juez_config_clave_repetida_vale_la_primera() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let juez = Falso::responde(
            r#"[{"clave":"http.proxy","necesidad":"innecesaria","motivo":"primero","cita":"proxy = \"http://127.0.0.1:9\""},{"clave":"http.proxy","necesidad":"riesgosa","motivo":"segundo","cita":"proxy = \"http://127.0.0.1:9\""}]"#,
        );

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_PROXY),
            CARGO_TOML,
            &inv,
            None,
        );

        match &r {
            ResultadoJuicio::Juzgado {
                juicios,
                descartados,
            } => {
                assert_eq!(juicios.len(), 1, "{juicios:?}");
                assert_eq!(juicios[0].necesidad, Necesidad::Innecesaria);
                assert_eq!(descartados, &vec!["http.proxy".to_string()]);
            }
            otro => panic!("se esperaba un juicio: {otro:?}"),
        }
    }

    #[test]
    fn juez_config_sin_entradas_no_llama_al_juez() {
        let inv = Inventario::default();
        let juez = Falso::responde("[]");

        let r = juzgar_config(&juez, &[], &[], CARGO_TOML, &inv, None);

        assert_eq!(
            r,
            ResultadoJuicio::Juzgado {
                juicios: Vec::new(),
                descartados: Vec::new(),
            }
        );
        assert_eq!(juez.llamadas.get(), 0, "sin entradas no hay llamada");
    }

    #[test]
    fn juez_config_prompt_no_lleva_codigo_ni_ocultos() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let ficha = "## Ficha\n```toml\n[producto]\nnombre = \"dep-prueba\"\n```";

        let p = prompt_config(&inv.config_cargo, CARGO_TOML, &inv, Some(ficha));

        for texto in [&p.usuario, p.sistema.as_deref().unwrap_or_default()] {
            assert!(!texto.contains("oculto"), "{texto}");
            assert!(!texto.contains("fn main"), "{texto}");
        }
        assert!(p.usuario.contains(CITA_PROXY), "{}", p.usuario);
        assert!(p.usuario.contains("[dependencies]"), "{}", p.usuario);
        assert!(
            p.usuario.contains("nombre = \"dep-prueba\""),
            "{}",
            p.usuario
        );
        assert_eq!(p.fase, "descarga-juez-config");
        assert_eq!(p.max_tokens, crate::proveedores::MAX_TOKENS_JUEZ);
    }

    // Respuesta truncada por el tope de salida: se mira antes de parsear.

    #[test]
    fn juez_config_truncada_con_texto_vacio_da_la_causa_del_tope() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let juez = Falso::truncada_con("");

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_PROXY),
            CARGO_TOML,
            &inv,
            None,
        );

        assert_eq!(
            r,
            ResultadoJuicio::SinVeredicto {
                causa: "tope de salida alcanzado (32000 tokens)".into()
            }
        );
        assert!(hallazgos_de_juicio(&r, &inv.config_cargo).is_empty());
    }

    #[test]
    fn juez_config_truncada_con_json_valido_igual_da_sin_veredicto() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let juez = Falso::truncada_con(&respuesta_proxy("innecesaria", CITA_PROXY));

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_PROXY),
            CARGO_TOML,
            &inv,
            None,
        );

        match &r {
            ResultadoJuicio::SinVeredicto { causa } => {
                assert!(causa.starts_with("tope de salida alcanzado"), "{causa}");
                assert!(!causa.contains("JSON"), "{causa}");
            }
            otro => panic!("se esperaba sin veredicto: {otro:?}"),
        }
        assert!(hallazgos_de_juicio(&r, &inv.config_cargo).is_empty());
    }

    #[test]
    fn juez_config_no_truncada_json_roto_sigue_con_causa_json() {
        let inv = inventario_con(vec![entrada("http.proxy", 2, CITA_PROXY)]);
        let juez = Falso::responde("{\"clave\": [");

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_PROXY),
            CARGO_TOML,
            &inv,
            None,
        );

        assert_eq!(
            r,
            ResultadoJuicio::SinVeredicto {
                causa: CAUSA_JSON.into()
            }
        );
    }

    #[test]
    fn juez_config_riesgosa_da_severidad_2() {
        let inv = inventario_con(vec![entrada(
            "net.git-fetch-with-cli",
            2,
            "git-fetch-with-cli = true",
        )]);
        let juez = Falso::responde(
            r#"[{"clave":"net.git-fetch-with-cli","necesidad":"riesgosa","motivo":"baja código por git","cita":"git-fetch-with-cli = true"}]"#,
        );

        let r = juzgar_config(
            &juez,
            &inv.config_cargo,
            &textos(CONFIG_GIT),
            CARGO_TOML,
            &inv,
            None,
        );

        let hallazgos = hallazgos_de_juicio(&r, &inv.config_cargo);
        assert_eq!(hallazgos.len(), 1, "{hallazgos:?}");
        assert_eq!(hallazgos[0].severidad, 2);
        assert_eq!(hallazgos[0].dimension, Dimension::Entrega);
        assert_eq!(
            hallazgos[0].fuente,
            Fuente::Juez {
                modelo: "juez-config".into()
            }
        );
    }
}
