//! Tipos compartidos de Root. Sus formas JSON forman parte de la interfaz fija.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Versión del formato de las líneas JSONL y de los objetos persistidos.
pub const VERSION: u32 = 1;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
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
    /// Sesión de Claude en la nube. Route nunca la elige; solo `observe::uso` la cuenta.
    Nube,
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
    Completitud,
    Entrega,
    Pruebas,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    Verde,
    Rojo,
    SinVeredicto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Cumplimiento {
    Cumplido,
    Parcial,
    Ausente,
    SinVeredicto,
    Pendiente,
}

/// Test que falla por el entorno y no se cobra al producto. Figura en el informe con su motivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Pendiente {
    /// Id del test con forma `archivo::nombre`.
    pub test: String,
    pub motivo: String,
    /// Resultado importado de Colab: "pasa", "falla" u "omite". Ausente si no se importó.
    #[serde(default)]
    pub colab: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Listo,
    NoListo,
    Bloqueado,
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

/// Gasto medido en USD equivalentes de lista. Lo arma `observe::uso`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Uso {
    /// Gasto de cupo del bloque vigente.
    pub cupo_bloque_usd: f64,
    /// Epoch en segundos del primer evento del bloque vigente. `None` si no hay bloque.
    #[serde(default)]
    pub cupo_bloque_inicio: Option<f64>,
    /// Gasto de API del mes calendario.
    pub api_mes_usd: f64,
    /// Gasto de sesiones en la nube del mes calendario.
    pub nube_mes_usd: f64,
}

/// No hay vía con margen. `reabre` es el epoch en segundos del próximo momento con margen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SinPresupuesto {
    pub reabre: f64,
    pub motivo: String,
}

impl std::fmt::Display for SinPresupuesto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "sin presupuesto hasta el epoch {}: {}",
            self.reabre, self.motivo
        )
    }
}

impl std::error::Error for SinPresupuesto {}

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
    /// Estable: hash de dimensión, archivo y cita.
    #[serde(default)]
    pub id: String,
    /// Ids de los requisitos que afecta.
    #[serde(default)]
    pub impacto: Vec<String>,
}

/// Nivel de un ítem de rúbrica. `nivel` va de 0 a 3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NivelItem {
    pub item: String,
    pub nivel: u8,
    pub evidencia: Vec<String>,
    pub fuente: Fuente,
}

/// `valor` va de 0 a 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Puntaje {
    pub dimension: Dimension,
    pub valor: f32,
    pub calibrada: bool,
    pub exactitud_balanceada: Option<f32>,
    #[serde(default)]
    pub items: Vec<NivelItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EstadoRequisito {
    pub id: String,
    pub titulo: String,
    pub peso: u8,
    pub cumplimiento: Cumplimiento,
    /// Ids de tests, escenarios o hallazgos.
    pub evidencia: Vec<String>,
    /// Nivel de completitud de 0 a 3. `None` si no hay veredicto o está pendiente (Tarea 9c).
    #[serde(default)]
    pub nivel: Option<u8>,
}

/// `refs` lleva ids de requisitos o hallazgos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Accion {
    pub prioridad: u8,
    pub texto: String,
    pub refs: Vec<String>,
}

/// `riesgos` y `proximos_pasos` tienen como máximo 5 entradas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Informe {
    pub decision: Decision,
    pub motivo: String,
    pub requisitos: Vec<EstadoRequisito>,
    /// Ids de hallazgos.
    pub bloqueantes: Vec<String>,
    pub riesgos: Vec<Accion>,
    pub proximos_pasos: Vec<Accion>,
    /// Prosa. Cada oración cita un id.
    pub resumen: String,
}

/// Tasa de aprobación de un test inestable, con la cota inferior de Wilson al 90%.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TasaTest {
    pub pasa: u32,
    pub total: u32,
    pub wilson_inf: f64,
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
    #[serde(default)]
    pub informe: Option<Informe>,
    /// Hashes de rúbrica, por ejemplo "diseno@<hash>".
    #[serde(default)]
    pub rubricas: Vec<String>,
    /// Repeticiones del oculto que corrieron. Un veredicto viejo lee 0.
    #[serde(default)]
    pub repeticiones: u32,
    /// Ids ordenados de los tests inestables del oculto. Un veredicto viejo lee vacío.
    #[serde(default)]
    pub inestables: Vec<String>,
    /// Ids ordenados de los tests inestables del contrato. Un veredicto viejo lee vacío.
    #[serde(default)]
    pub inestables_contrato: Vec<String>,
    /// Tasa de cada test inestable. Claves `contrato:<id>` u `oculto:<id>`. Un veredicto viejo lee vacío.
    #[serde(default)]
    pub tasas: BTreeMap<String, TasaTest>,
    /// Tests de `[[entorno]]`, con su motivo. No entran al conteo. Un veredicto viejo lee vacío.
    /// Completitud y el nivel por requisito llegan con la Tarea 9c; por ahora solo se listan.
    #[serde(default)]
    pub pendientes: Vec<Pendiente>,
    /// Problemas que no tumban la evaluación, por ejemplo un `colab.json` ilegible.
    #[serde(default)]
    pub avisos: Vec<String>,
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
            informe: None,
            rubricas: Vec::new(),
            repeticiones: 1,
            inestables: Vec::new(),
            inestables_contrato: Vec::new(),
            tasas: BTreeMap::new(),
            pendientes: Vec::new(),
            avisos: Vec::new(),
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
    /// Monto que una llamada a la API reserva antes de enviar el pedido. Lo cierra el `Llamada`.
    Reserva,
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

/// Forma de una corrida (decisión 28): roles en orden, si hubo verificador y la configuración.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
pub struct Forma {
    pub roles: Vec<Rol>,
    pub verificador: bool,
    pub config: String,
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
    /// Modelo que cumplió cada rol en la corrida.
    #[serde(default)]
    pub modelos: BTreeMap<Rol, String>,
    /// Forma de la corrida. Las filas anteriores a la decisión 28 la leen vacía.
    #[serde(default)]
    pub forma: Forma,
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
            modelos: BTreeMap::new(),
            forma: Forma::default(),
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
                items: Vec::new(),
            }],
            hallazgos: vec![Hallazgo {
                dimension: Dimension::Seguridad,
                severidad: 2,
                archivo: "app.py".into(),
                linea: 14,
                cita: "password = 'x'".into(),
                detalle: "secreto literal".into(),
                fuente: Fuente::Sonda("bandit".into()),
                id: "h-1".into(),
                impacto: vec!["req.login".into()],
            }],
            global: Some(0.95),
            estado: Estado::Verde,
            costo_usd: 0.42,
            informe: None,
            rubricas: Vec::new(),
            repeticiones: 3,
            inestables: vec!["test_f::t_flaky".into()],
            inestables_contrato: Vec::new(),
            tasas: BTreeMap::new(),
            pendientes: Vec::new(),
            avisos: Vec::new(),
        }
    }

    fn evento() -> Evento {
        let mut extra = serde_json::Map::new();
        extra.insert("origen".into(), json!("otra-fuente"));
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
            modelos: BTreeMap::new(),
            forma: Forma::default(),
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
            TipoEvento::Reserva,
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
            items: Vec::new(),
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
            id: "h-7".into(),
            impacto: vec!["req.almacen".into()],
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
        assert_eq!(valor["extra"]["origen"], "otra-fuente");
        assert_eq!(valor["extra"]["reintentos"], 2);
        assert_eq!(valor["via"], "api");
        assert_eq!(valor["tipo"], "llamada");
    }

    #[test]
    fn tipos_estado_requisito_v1_sin_nivel_se_lee_como_none() {
        let v1 = r#"{"id":"R1","titulo":"Login","peso":2,"cumplimiento":"parcial","evidencia":["a::t1"]}"#;
        let estado: EstadoRequisito = serde_json::from_str(v1).expect("lee el formato v1");
        assert_eq!(estado.nivel, None);
        ida_y_vuelta(&EstadoRequisito {
            nivel: Some(2),
            ..estado
        });
    }

    fn informe() -> Informe {
        Informe {
            decision: Decision::NoListo,
            motivo: "hay un bloqueante".into(),
            requisitos: vec![EstadoRequisito {
                id: "req.login".into(),
                titulo: "Login con contraseña".into(),
                peso: 3,
                cumplimiento: Cumplimiento::Parcial,
                evidencia: vec!["test_login".into()],
                nivel: Some(1),
            }],
            bloqueantes: vec!["h-1".into()],
            riesgos: vec![Accion {
                prioridad: 1,
                texto: "No hay límite de intentos".into(),
                refs: vec!["req.login".into()],
            }],
            proximos_pasos: vec![Accion {
                prioridad: 2,
                texto: "Agregar prueba de bloqueo".into(),
                refs: Vec::new(),
            }],
            resumen: "El login falla. Ver h-1.".into(),
        }
    }

    #[test]
    fn tipos_veredicto_v1_sin_campos_nuevos_se_lee() {
        let texto = r#"{"v":1,"tarea":"memora","repo":"r","commit":"c",
            "contrato":{"pasa":1,"total":1},"oculto":{"pasa":0,"total":0},
            "puntajes":[{"dimension":"seguridad","valor":0.5,"calibrada":true,
            "exactitud_balanceada":null}],
            "hallazgos":[{"dimension":"seguridad","severidad":1,"archivo":"a.py",
            "linea":3,"cita":"x","detalle":"d","fuente":{"Sonda":"bandit"}}],
            "global":null,"estado":"sin_veredicto","costo_usd":0.0}"#;
        let v: Veredicto = serde_json::from_str(texto).expect("v1 se acepta");
        assert_eq!(v.informe, None);
        assert!(v.rubricas.is_empty());
        assert!(v.puntajes[0].items.is_empty());
        assert_eq!(v.hallazgos[0].id, "");
        assert!(v.hallazgos[0].impacto.is_empty());
    }

    #[test]
    fn repeticiones_veredicto_v1_se_lee_con_cero_y_vacio() {
        let texto = r#"{"v":1,"tarea":"memora","repo":"r","commit":"c",
            "contrato":{"pasa":1,"total":1},"oculto":{"pasa":0,"total":0},
            "puntajes":[],"hallazgos":[],"global":null,"estado":"sin_veredicto",
            "costo_usd":0.0}"#;
        let v: Veredicto = serde_json::from_str(texto).expect("v1 se acepta");
        assert_eq!(v.repeticiones, 0);
        assert!(v.inestables.is_empty());
    }

    #[test]
    fn inestable_veredicto_viejo_lee_tasas_y_contrato_vacios() {
        let texto = r#"{"v":1,"tarea":"t","repo":"r","commit":"","contrato":{"pasa":1,"total":1},
            "oculto":{"pasa":1,"total":1},"puntajes":[],"hallazgos":[],"global":null,
            "estado":"sin_veredicto","costo_usd":0.0}"#;
        let v: Veredicto = serde_json::from_str(texto).expect("v1 se acepta");
        assert!(v.tasas.is_empty());
        assert!(v.inestables_contrato.is_empty());
    }

    #[test]
    fn inestable_tasa_test_ida_y_vuelta_en_veredicto() {
        let mut v = Veredicto::default();
        v.tasas.insert(
            "oculto:test_f::t_flaky".into(),
            TasaTest {
                pasa: 5,
                total: 10,
                wilson_inf: 0.3,
            },
        );
        v.inestables_contrato = vec!["test_c::t".into()];
        ida_y_vuelta(&v);
    }

    #[test]
    fn repeticiones_veredicto_nuevo_vale_1_por_defecto() {
        assert_eq!(Veredicto::default().repeticiones, 1);
        assert!(Veredicto::default().inestables.is_empty());
    }

    #[test]
    fn tipos_fila_v1_sin_modelos_se_lee_con_mapa_vacio() {
        let texto = r#"{"v":1,"id":"f-1","ts":1.0,"tarea":"memora","config":"c",
            "commit_root":"x","veredicto":{"v":1,"tarea":"memora","repo":"r",
            "commit":"c","contrato":{"pasa":1,"total":1},"oculto":{"pasa":0,"total":0},
            "puntajes":[],"hallazgos":[],"global":null,"estado":"sin_veredicto",
            "costo_usd":0.0},"resumen":{"usd_api":0.0,"usd_cupo":0.0,"minutos":0.0,
            "turnos":0,"intervenciones":0,"invalida":false}}"#;
        let f: Fila = serde_json::from_str(texto).expect("v1 se acepta");
        assert!(f.modelos.is_empty());
    }

    #[test]
    fn tipos_fila_con_modelos_ida_y_vuelta() {
        let mut f = fila();
        f.modelos.insert(Rol::Construye, "haiku".into());
        f.modelos.insert(Rol::Organiza, "sonnet".into());
        ida_y_vuelta(&f);
        let valor = serde_json::to_value(&f).unwrap();
        assert_eq!(valor["modelos"]["construye"], "haiku");
        assert_eq!(valor["modelos"]["organiza"], "sonnet");
    }

    #[test]
    fn tipos_ida_y_vuelta_de_informe_en_veredicto() {
        ida_y_vuelta(&informe());
        let mut v = veredicto();
        v.informe = Some(informe());
        v.rubricas = vec!["diseno@abc".into()];
        ida_y_vuelta(&v);
    }

    #[test]
    fn tipos_ida_y_vuelta_de_nivelitem_requisito_y_accion() {
        ida_y_vuelta(&NivelItem {
            item: "diseno.modularidad".into(),
            nivel: 2,
            evidencia: vec!["src/tipos.rs:10".into()],
            fuente: Fuente::Juez {
                modelo: "deepseek-reasoner".into(),
            },
        });
        ida_y_vuelta(&EstadoRequisito {
            id: "req.login".into(),
            titulo: "Login".into(),
            peso: 2,
            cumplimiento: Cumplimiento::SinVeredicto,
            evidencia: Vec::new(),
            nivel: None,
        });
        ida_y_vuelta(&Accion {
            prioridad: 3,
            texto: "Separar el módulo".into(),
            refs: vec!["h-2".into()],
        });
        for x in [
            Cumplimiento::Cumplido,
            Cumplimiento::Parcial,
            Cumplimiento::Ausente,
            Cumplimiento::SinVeredicto,
            Cumplimiento::Pendiente,
        ] {
            ida_y_vuelta(&x);
        }
        for x in [Decision::Listo, Decision::NoListo, Decision::Bloqueado] {
            ida_y_vuelta(&x);
        }
    }

    #[test]
    fn tipos_dimension_nuevas_al_final_en_snake_case() {
        for (dimension, texto) in [
            (Dimension::Completitud, "completitud"),
            (Dimension::Entrega, "entrega"),
            (Dimension::Pruebas, "pruebas"),
        ] {
            assert_eq!(serde_json::to_value(dimension).unwrap(), texto);
            ida_y_vuelta(&dimension);
        }
        assert_eq!(
            serde_json::to_value(Dimension::Funciona).unwrap(),
            "funciona"
        );
        assert_eq!(serde_json::to_value(Dimension::Diseno).unwrap(), "diseno");
    }

    #[test]
    fn pendiente_cumplimiento_serializa_pendiente() {
        assert_eq!(
            serde_json::to_value(Cumplimiento::Pendiente).unwrap(),
            "pendiente"
        );
        ida_y_vuelta(&Cumplimiento::Pendiente);
    }

    #[test]
    fn pendiente_ida_y_vuelta_con_y_sin_colab() {
        ida_y_vuelta(&Pendiente {
            test: "test_f::t_gpu".into(),
            motivo: "lavapipe no emula la GPU".into(),
            colab: None,
        });
        ida_y_vuelta(&Pendiente {
            test: "test_f::t_gpu".into(),
            motivo: "lavapipe no emula la GPU".into(),
            colab: Some("pasa".into()),
        });
    }

    #[test]
    fn pendiente_veredicto_con_pendientes_ida_y_vuelta() {
        let mut v = Veredicto::default();
        assert!(v.pendientes.is_empty());
        v.pendientes.push(Pendiente {
            test: "test_f::t_gpu".into(),
            motivo: "entorno".into(),
            colab: None,
        });
        ida_y_vuelta(&v);
    }

    #[test]
    fn pendiente_veredicto_v1_sin_pendientes_se_lee_vacio() {
        let texto = r#"{"v":1,"tarea":"memora","repo":"r","commit":"c",
            "contrato":{"pasa":1,"total":1},"oculto":{"pasa":0,"total":0},
            "puntajes":[],"hallazgos":[],"global":null,"estado":"sin_veredicto",
            "costo_usd":0.0}"#;
        let v: Veredicto = serde_json::from_str(texto).expect("v1 se acepta");
        assert!(v.pendientes.is_empty());
    }
}
