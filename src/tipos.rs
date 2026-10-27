//! Tipos compartidos de Root. Sus formas JSON forman parte de la interfaz fija.

use schemars::JsonSchema;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Versión del formato de las líneas JSONL y de los objetos persistidos.
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Rol {
    Planifica,
    Organiza,
    Construye,
    Verifica,
    Juzga,
    Delega,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Proveedor {
    Anthropic,
    Deepseek,
}

/// `Oraculo` significa no llamar a ningún modelo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Via {
    Cupo,
    Api,
    Oraculo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Funciona,
    Seguridad,
    Eficiencia,
    Estructura,
    Arquitectura,
    Diseno,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    Verde,
    Rojo,
    SinVeredicto,
}

/// Quién produjo un hallazgo: una sonda determinista o un juez con modelo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Fuente {
    Sonda(String),
    Juez { modelo: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pedido {
    pub rol: Rol,
    pub tipo: String,
    pub checkeable: bool,
    pub tokens_estimados: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Eleccion {
    pub via: Via,
    pub proveedor: Option<Proveedor>,
    pub modelo: Option<String>,
    pub razonamiento: bool,
    pub motivo: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Conteo {
    pub pasa: u32,
    pub total: u32,
}

/// `severidad` va de 1 a 3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hallazgo {
    pub dimension: Dimension,
    pub severidad: u8,
    pub archivo: String,
    pub linea: u32,
    pub cita: String,
    pub detalle: String,
    pub fuente: Fuente,
}

/// `valor` va de 0 a 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Puntaje {
    pub dimension: Dimension,
    pub valor: f32,
    pub calibrada: bool,
    pub exactitud_balanceada: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Veredicto {
    #[serde(deserialize_with = "version_valida")]
    pub v: u32,
    pub tarea: String,
    pub repo: String,
    pub commit: String,
    pub contrato: Conteo,
    pub oculto: Conteo,
    pub puntajes: Vec<Puntaje>,
    pub hallazgos: Vec<Hallazgo>,
    pub global: Option<f32>,
    pub estado: Estado,
    pub costo_usd: f64,
}

impl Default for Veredicto {
    fn default() -> Self {
        Veredicto {
            v: VERSION,
            tarea: String::new(),
            repo: String::new(),
            commit: String::new(),
            contrato: Conteo::default(),
            oculto: Conteo::default(),
            puntajes: Vec::new(),
            hallazgos: Vec::new(),
            global: None,
            estado: Estado::SinVeredicto,
            costo_usd: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TipoEvento {
    Llamada,
    Sesion,
    Humano,
    PlanAprobado,
    Gate,
    OraculoLeido,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Evento {
    #[serde(deserialize_with = "version_valida")]
    pub v: u32,
    pub ts: f64,
    pub iso: String,
    pub corrida: String,
    pub fase: String,
    pub tipo: TipoEvento,
    pub via: Option<Via>,
    pub modelo: Option<String>,
    pub familia: Option<String>,
    pub in_tokens: u64,
    pub out_tokens: u64,
    pub cost_usd: f64,
    pub latency_s: f64,
    pub extra: serde_json::Map<String, Value>,
}

impl Default for Evento {
    fn default() -> Self {
        Evento {
            v: VERSION,
            ts: 0.0,
            iso: String::new(),
            corrida: String::new(),
            fase: String::new(),
            tipo: TipoEvento::Llamada,
            via: None,
            modelo: None,
            familia: None,
            in_tokens: 0,
            out_tokens: 0,
            cost_usd: 0.0,
            latency_s: 0.0,
            extra: serde_json::Map::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Resumen {
    pub usd_api: f64,
    pub usd_cupo: f64,
    pub minutos: f64,
    pub turnos: u32,
    pub intervenciones: u32,
    pub invalida: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fila {
    #[serde(deserialize_with = "version_valida")]
    pub v: u32,
    pub id: String,
    pub ts: f64,
    pub tarea: String,
    pub config: String,
    pub commit_root: String,
    pub veredicto: Veredicto,
    pub resumen: Resumen,
}

impl Default for Fila {
    fn default() -> Self {
        Fila {
            v: VERSION,
            id: String::new(),
            ts: 0.0,
            tarea: String::new(),
            config: String::new(),
            commit_root: String::new(),
            veredicto: Veredicto::default(),
            resumen: Resumen::default(),
        }
    }
}

/// Acepta cualquier `v` hasta `VERSION`. Los lectores aceptan versiones anteriores.
fn version_valida<'de, D: Deserializer<'de>>(d: D) -> Result<u32, D::Error> {
    let v = u32::deserialize(d)?;
    if v <= VERSION {
        Ok(v)
    } else {
        Err(D::Error::custom(format!(
            "versión {v} posterior a la soportada ({VERSION})"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    use serde_json::json;
    use std::fmt::Debug;

    fn ida_y_vuelta<T>(original: &T)
    where
        T: Serialize + DeserializeOwned + PartialEq + Debug,
    {
        let texto = serde_json::to_string(original).expect("serializa");
        let vuelta: T = serde_json::from_str(&texto).expect("deserializa");
        assert_eq!(&vuelta, original, "json: {texto}");
    }

    fn resumen() -> Resumen {
        Resumen {
            usd_api: 0.5,
            usd_cupo: 1.25,
            minutos: 12.5,
            turnos: 31,
            intervenciones: 2,
            invalida: false,
        }
    }

    fn veredicto() -> Veredicto {
        Veredicto {
            v: VERSION,
            tarea: "memora".into(),
            repo: "/repos/memora-ecc".into(),
            commit: "abc1234".into(),
            contrato: Conteo {
                pasa: 61,
                total: 61,
            },
            oculto: Conteo {
                pasa: 48,
                total: 48,
            },
            puntajes: vec![Puntaje {
                dimension: Dimension::Seguridad,
                valor: 0.95,
                calibrada: true,
                exactitud_balanceada: Some(0.97),
            }],
            hallazgos: vec![Hallazgo {
                dimension: Dimension::Seguridad,
                severidad: 2,
                archivo: "app.py".into(),
                linea: 14,
                cita: "password = 'x'".into(),
                detalle: "secreto literal".into(),
                fuente: Fuente::Sonda("bandit".into()),
            }],
            global: Some(0.95),
            estado: Estado::Verde,
            costo_usd: 0.42,
        }
    }

    fn evento() -> Evento {
        let mut extra = serde_json::Map::new();
        extra.insert("origen".into(), json!("mmorch"));
        extra.insert("reintentos".into(), json!(2));
        Evento {
            v: VERSION,
            ts: 1_761_580_800.25,
            iso: "2025-10-27T16:00:00Z".into(),
            corrida: "c-01".into(),
            fase: "construye".into(),
            tipo: TipoEvento::Llamada,
            via: Some(Via::Api),
            modelo: Some("deepseek-reasoner".into()),
            familia: Some("deepseek".into()),
            in_tokens: 1200,
            out_tokens: 340,
            cost_usd: 0.0021,
            latency_s: 3.5,
            extra,
        }
    }

    fn fila() -> Fila {
        Fila {
            v: VERSION,
            id: "f-001".into(),
            ts: 1_761_580_900.5,
            tarea: "memora".into(),
            config: "ecc-haiku".into(),
            commit_root: "def5678".into(),
            veredicto: veredicto(),
            resumen: resumen(),
        }
    }

    #[test]
    fn tipos_ida_y_vuelta_de_enums_unitarios() {
        for x in [
            Rol::Planifica,
            Rol::Organiza,
            Rol::Construye,
            Rol::Verifica,
            Rol::Juzga,
            Rol::Delega,
        ] {
            ida_y_vuelta(&x);
        }
        for x in [Proveedor::Anthropic, Proveedor::Deepseek] {
            ida_y_vuelta(&x);
        }
        for x in [Via::Cupo, Via::Api, Via::Oraculo] {
            ida_y_vuelta(&x);
        }
        for x in [
            Dimension::Funciona,
            Dimension::Seguridad,
            Dimension::Eficiencia,
            Dimension::Estructura,
            Dimension::Arquitectura,
            Dimension::Diseno,
        ] {
            ida_y_vuelta(&x);
        }
        for x in [Estado::Verde, Estado::Rojo, Estado::SinVeredicto] {
            ida_y_vuelta(&x);
        }
        for x in [
            TipoEvento::Llamada,
            TipoEvento::Sesion,
            TipoEvento::Humano,
            TipoEvento::PlanAprobado,
            TipoEvento::Gate,
            TipoEvento::OraculoLeido,
        ] {
            ida_y_vuelta(&x);
        }
    }

    #[test]
    fn tipos_nombres_de_enums_en_snake_case() {
        assert_eq!(
            serde_json::to_value(Estado::SinVeredicto).unwrap(),
            "sin_veredicto"
        );
        assert_eq!(serde_json::to_value(Via::Oraculo).unwrap(), "oraculo");
        assert_eq!(
            serde_json::to_value(TipoEvento::OraculoLeido).unwrap(),
            "oraculo_leido"
        );
        assert_eq!(serde_json::to_value(Dimension::Diseno).unwrap(), "diseno");
    }

    #[test]
    fn tipos_ida_y_vuelta_de_fuente_con_datos() {
        ida_y_vuelta(&Fuente::Sonda("bandit".into()));
        ida_y_vuelta(&Fuente::Juez {
            modelo: "deepseek-reasoner".into(),
        });
        assert_eq!(
            serde_json::to_value(Fuente::Sonda("bandit".into())).unwrap(),
            json!({"Sonda": "bandit"})
        );
    }

    #[test]
    fn tipos_ida_y_vuelta_de_pedido_eleccion_conteo_puntaje_hallazgo() {
        ida_y_vuelta(&Pedido {
            rol: Rol::Construye,
            tipo: "bulk".into(),
            checkeable: true,
            tokens_estimados: 4000,
        });
        ida_y_vuelta(&Eleccion {
            via: Via::Api,
            proveedor: Some(Proveedor::Deepseek),
            modelo: Some("deepseek-chat".into()),
            razonamiento: false,
            motivo: "cupo al 95%".into(),
        });
        ida_y_vuelta(&Eleccion {
            via: Via::Oraculo,
            proveedor: None,
            modelo: None,
            razonamiento: false,
            motivo: "checkeable".into(),
        });
        ida_y_vuelta(&Conteo {
            pasa: 15,
            total: 16,
        });
        ida_y_vuelta(&Puntaje {
            dimension: Dimension::Eficiencia,
            valor: 0.5,
            calibrada: false,
            exactitud_balanceada: None,
        });
        ida_y_vuelta(&Hallazgo {
            dimension: Dimension::Arquitectura,
            severidad: 3,
            archivo: "src/almacen.rs".into(),
            linea: 7,
            cita: "use crate::comandos;".into(),
            detalle: "importa una capa anterior".into(),
            fuente: Fuente::Juez {
                modelo: "deepseek-reasoner".into(),
            },
        });
    }

    #[test]
    fn tipos_ida_y_vuelta_de_veredicto_evento_resumen_fila() {
        ida_y_vuelta(&veredicto());
        ida_y_vuelta(&evento());
        ida_y_vuelta(&resumen());
        ida_y_vuelta(&fila());
    }

    #[test]
    fn tipos_veredicto_evento_y_fila_llevan_v_igual_a_1() {
        assert_eq!(VERSION, 1);
        assert_eq!(serde_json::to_value(veredicto()).unwrap()["v"], 1);
        assert_eq!(serde_json::to_value(evento()).unwrap()["v"], 1);
        assert_eq!(serde_json::to_value(fila()).unwrap()["v"], 1);
        assert_eq!(Veredicto::default().v, VERSION);
        assert_eq!(Evento::default().v, VERSION);
        assert_eq!(Fila::default().v, VERSION);
    }

    #[test]
    fn tipos_lee_registros_con_v_1() {
        let texto = r#"{"v":1,"id":"f-1","ts":1.0,"tarea":"memora","config":"c",
            "commit_root":"x","veredicto":{"v":1,"tarea":"memora","repo":"r",
            "commit":"c","contrato":{"pasa":1,"total":1},"oculto":{"pasa":0,"total":0},
            "puntajes":[],"hallazgos":[],"global":null,"estado":"sin_veredicto",
            "costo_usd":0.0},"resumen":{"usd_api":0.0,"usd_cupo":0.0,"minutos":0.0,
            "turnos":0,"intervenciones":0,"invalida":false}}"#;
        let fila: Fila = serde_json::from_str(texto).expect("v 1 se acepta");
        assert_eq!(fila.veredicto.estado, Estado::SinVeredicto);
    }

    #[test]
    fn tipos_rechaza_v_posterior_a_la_version() {
        let texto = r#"{"v":2,"ts":1.0,"iso":"","corrida":"c","fase":"f","tipo":"llamada",
            "via":null,"modelo":null,"familia":null,"in_tokens":0,"out_tokens":0,
            "cost_usd":0.0,"latency_s":0.0,"extra":{}}"#;
        assert!(serde_json::from_str::<Evento>(texto).is_err());
    }

    #[test]
    fn tipos_resumen_con_valores_reales() {
        let valor = serde_json::to_value(resumen()).unwrap();
        assert_eq!(valor["usd_cupo"], 1.25);
        assert_eq!(valor["turnos"], 31);
        assert_eq!(valor["intervenciones"], 2);
        assert_eq!(valor["invalida"], false);
    }

    #[test]
    fn tipos_evento_extra_con_valores_reales() {
        let valor = serde_json::to_value(evento()).unwrap();
        assert_eq!(valor["extra"]["origen"], "mmorch");
        assert_eq!(valor["extra"]["reintentos"], 2);
        assert_eq!(valor["via"], "api");
        assert_eq!(valor["tipo"], "llamada");
    }
}
