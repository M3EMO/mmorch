//! Informe del PM (Tarea 13b). La decisión sale de reglas deterministas sobre el `Veredicto`. Un LLM
//! solo redacta el resumen y los textos de riesgos y próximos pasos, y cada oración debe citar un id real.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Deserialize;

use super::ErrorOraculo;
use super::evidencia::oraciones_con_id;
use super::juez::desenvuelto;
use super::perfiles::tiene_perfil;
use super::requisitos::PREFIJO_CONTRATO;
use super::rubrica::Rubrica;
use super::tarea::Tarea;
use crate::config::ReglasDecision;
use crate::proveedores::{self, Prompt, causa_tope_alcanzado};
use crate::tipos::{
    Accion, AvisoPrevio, Confianza, Cumplimiento, Decision, Dimension, Eleccion, Estado,
    EstadoRequisito, Fuente, Hallazgo, Informe, ParteSinVeredicto, Veredicto,
};

/// Máximo de riesgos y de próximos pasos.
pub const MAX_ITEMS: usize = 5;
/// Tope de salida del redactor. Es un techo: Root paga solo lo que el modelo escribe.
pub const MAX_TOKENS_REDACTOR: u32 = 8_192;
/// Fase del `Evento` de la llamada del redactor.
const FASE: &str = "informe";
/// Peso de un requisito que bloquea cuando queda en nivel 0.
const PESO_BLOQUEANTE: u8 = 3;
/// Severidad que hace bloqueante a un hallazgo de seguridad, y que lo pone primero en próximos pasos.
const SEVERIDAD_BLOQUEANTE: u8 = 3;
/// Severidad mínima de un hallazgo para ser riesgo.
const SEVERIDAD_RIESGO: u8 = 2;
/// Instrucciones del redactor. Solo redacta: la decisión ya está tomada.
const SISTEMA: &str = "Sos un PM que redacta el informe de una evaluación. La decisión ya está tomada: no la discutas ni la cambies.
Redactás el resumen y los textos de riesgos y próximos pasos a partir del informe JSON que recibís.
Cada oración debe citar al menos un id de requisito o de hallazgo tal como aparece en el informe. Una oración sin id se descarta.
No inventes requisitos, hallazgos ni ids.
Respondé SOLO un objeto JSON: {\"resumen\": \"...\", \"riesgos\": [\"...\"], \"proximos_pasos\": [\"...\"]}. Las listas van en el mismo orden que en el informe.";

/// Textos que propone el redactor, sin filtrar. `informar_con` les aplica el gate de evidencia.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Redaccion {
    pub resumen: String,
    /// Textos alternativos de `riesgos`, en el mismo orden.
    pub riesgos: Vec<String>,
    /// Textos alternativos de `proximos_pasos`, en el mismo orden.
    pub proximos_pasos: Vec<String>,
    /// Costo de la llamada en USD. Cero si el proveedor no lo informa.
    pub costo_usd: f64,
}

/// Un modelo que redacta el informe a partir del determinista. Redacta, no juzga.
pub trait Redactor {
    fn redactar(&self, informe: &Informe) -> Result<Redaccion, ErrorOraculo>;
}

/// Redactor de producción: llama al proveedor de `eleccion`, la elección de Route para `delega`.
pub struct RedactorProveedor {
    pub eleccion: Eleccion,
    pub corrida: String,
    /// Carpeta de `root.toml` y `eventos.jsonl`: el `Gate` y el evento de cada llamada van aquí.
    pub home: PathBuf,
}

/// Forma JSON que devuelve el redactor.
#[derive(Deserialize)]
struct Salida {
    #[serde(default)]
    resumen: String,
    #[serde(default)]
    riesgos: Vec<String>,
    #[serde(default)]
    proximos_pasos: Vec<String>,
}

impl Redactor for RedactorProveedor {
    fn redactar(&self, informe: &Informe) -> Result<Redaccion, ErrorOraculo> {
        let usuario = serde_json::to_string_pretty(informe)
            .map_err(|e| ErrorOraculo::Infra(format!("informe sin JSON: {e}")))?;
        let prompt = Prompt {
            sistema: Some(SISTEMA.into()),
            usuario,
            max_tokens: MAX_TOKENS_REDACTOR,
            fase: FASE.into(),
            reintento_juez: false,
        };
        let respuesta = proveedores::llamar_en(&self.home, &self.eleccion, &prompt, &self.corrida)
            .map_err(|e| ErrorOraculo::Infra(e.to_string()))?;
        if respuesta.truncada {
            return Err(ErrorOraculo::Infra(causa_tope_alcanzado(
                MAX_TOKENS_REDACTOR,
            )));
        }
        let salida: Salida = serde_json::from_str(desenvuelto(&respuesta.texto))
            .map_err(|e| ErrorOraculo::Infra(format!("el redactor no devolvió JSON: {e}")))?;
        Ok(Redaccion {
            resumen: salida.resumen,
            riesgos: salida.riesgos,
            proximos_pasos: salida.proximos_pasos,
            costo_usd: respuesta.cost_usd,
        })
    }
}

/// Envoltorio que suma el costo de las redacciones. `evaluar_con` lo lee para `costo_usd`.
pub struct RedactorMedido<'a> {
    redactor: &'a dyn Redactor,
    costo: Cell<f64>,
}

impl<'a> RedactorMedido<'a> {
    pub fn nuevo(redactor: &'a dyn Redactor) -> Self {
        RedactorMedido {
            redactor,
            costo: Cell::new(0.0),
        }
    }

    /// Costo de las redacciones que salieron bien.
    pub fn costo(&self) -> f64 {
        self.costo.get()
    }
}

impl Redactor for RedactorMedido<'_> {
    fn redactar(&self, informe: &Informe) -> Result<Redaccion, ErrorOraculo> {
        let redaccion = self.redactor.redactar(informe)?;
        self.costo.set(self.costo.get() + redaccion.costo_usd);
        Ok(redaccion)
    }
}

/// Estado del veredicto según su informe: `Listo` es verde; `NoListo` y `Bloqueado`, rojo; `SinDecision`
/// y sin informe, sin veredicto.
pub fn estado_de(informe: Option<&Informe>) -> Estado {
    match informe.map(|i| i.decision) {
        Some(Decision::Listo) => Estado::Verde,
        Some(Decision::NoListo | Decision::Bloqueado) => Estado::Rojo,
        Some(Decision::SinDecision) | None => Estado::SinVeredicto,
    }
}

/// Arma el informe con las rúbricas embebidas. Ver `informar_con`.
pub fn informar(
    v: &Veredicto,
    tarea: &Tarea,
    reglas: &ReglasDecision,
    presentes: &BTreeSet<String>,
    redactor: Option<&dyn Redactor>,
) -> Result<Informe, ErrorOraculo> {
    let rubricas = super::rubrica::cargar_embebidas()
        .map_err(|e| ErrorOraculo::Infra(format!("rúbricas embebidas: {e}")))?;
    informar_con(v, tarea, reglas, &rubricas, presentes, redactor)
}

/// Arma el informe: decisión, requisitos, bloqueantes, riesgos, próximos pasos, resumen y confianza.
/// El redactor, si hay, ve una copia sin nombres de tests ocultos. Su texto pasa el gate de evidencia.
pub fn informar_con(
    v: &Veredicto,
    tarea: &Tarea,
    reglas: &ReglasDecision,
    rubricas: &[Rubrica],
    presentes: &BTreeSet<String>,
    redactor: Option<&dyn Redactor>,
) -> Result<Informe, ErrorOraculo> {
    let decidido = decidir(v, tarea, reglas, rubricas);
    let motivo = if decidido.disparos.is_empty() {
        "listo: ninguna regla de bloqueo ni de no listo disparó".to_string()
    } else {
        let nombres: Vec<&str> = decidido.disparos.iter().map(|d| d.regla.as_str()).collect();
        nombres.join("; ")
    };
    let bloqueantes: Vec<String> = if decidido.decision == Decision::Bloqueado {
        decidido
            .disparos
            .iter()
            .filter_map(|d| d.hallazgo.clone())
            .collect()
    } else {
        Vec::new()
    };
    let mut informe = Informe {
        avisos_previos: avisos_previos(v),
        decision: decidido.decision,
        motivo,
        requisitos: v.requisitos.clone(),
        riesgos: riesgos(v, tarea),
        proximos_pasos: proximos_pasos(v, tarea, &bloqueantes, presentes),
        bloqueantes,
        resumen: resumen(v, &decidido.disparos),
        confianza: confianza(v, tarea),
    };
    if let Some(redactor) = redactor {
        let mut para_redactor = informe.clone();
        let visibles = ocultar_tests(&mut para_redactor, &ids_ocultos(v));
        match redactor.redactar(&para_redactor) {
            Ok(redaccion) => {
                aplicar_redaccion(&mut informe, redaccion, &ids_citables(v), &visibles);
            }
            Err(e) => informe.confianza.avisos.push(format!(
                "redactor sin resultado: {e}; el resumen y los textos son deterministas"
            )),
        }
    }
    // Después de la redacción: el redactor no reescribe esta línea, que no cita ids.
    if let Some(linea) = linea_avisos_previos(&informe.avisos_previos) {
        informe.resumen = format!("{} {linea}", informe.resumen);
    }
    Ok(informe)
}

/// Sonda de cargo-audit. Su `cita` es el crate y la versión.
const SONDA_CARGO_AUDIT: &str = "cargo-audit";

/// Avisos de Seguridad previos que no son secretos, en orden del veredicto. Un secreto va con los bloqueos.
fn avisos_previos(v: &Veredicto) -> Vec<AvisoPrevio> {
    v.hallazgos
        .iter()
        .filter(|h| h.dimension == Dimension::Seguridad && h.previo && !es_secreto(h))
        .map(|h| {
            let objeto = match &h.fuente {
                Fuente::Sonda(nombre) if nombre == SONDA_CARGO_AUDIT => h.cita.clone(),
                _ => h.archivo.clone(),
            };
            AvisoPrevio {
                fuente: nombre_fuente(&h.fuente),
                aviso: aviso_de(&h.detalle),
                objeto,
                severidad: h.severidad,
            }
        })
        .collect()
}

/// Nombre de quien produjo el hallazgo: la sonda, o el juez con su modelo.
fn nombre_fuente(fuente: &Fuente) -> String {
    match fuente {
        Fuente::Sonda(nombre) => nombre.clone(),
        Fuente::Juez { modelo } => format!("juez {modelo}"),
    }
}

/// El aviso de un detalle: el id RUSTSEC si lo trae. Si no, la primera frase.
fn aviso_de(detalle: &str) -> String {
    let rustsec = detalle
        .split_whitespace()
        .map(|t| t.trim_end_matches(':'))
        .find(|t| t.starts_with("RUSTSEC-"));
    match rustsec {
        Some(id) => id.to_string(),
        None => detalle
            .split(". ")
            .next()
            .unwrap_or(detalle)
            .trim_end_matches('.')
            .to_string(),
    }
}

/// Frase del resumen con los avisos previos. `None` si no hay ninguno.
fn linea_avisos_previos(avisos: &[AvisoPrevio]) -> Option<String> {
    match avisos.len() {
        0 => None,
        1 => Some("1 aviso previo de Seguridad, no bloquea.".into()),
        n => Some(format!("{n} avisos previos de Seguridad, no bloquean.")),
    }
}

/// Un disparo de regla: el texto del motivo, la frase del resumen y, si bloquea por un hallazgo, su id.
#[derive(Debug, Clone, PartialEq)]
struct Disparo {
    regla: String,
    frase: String,
    hallazgo: Option<String>,
}

/// La decisión y las reglas que la dispararon.
struct Decidido {
    decision: Decision,
    disparos: Vec<Disparo>,
}

/// Reglas en orden de evidencia (Tarea 32, decisión 49 "primero la evidencia"): `Bloqueado` gana a todo,
/// después `NoListo`, y `SinDecision` solo reemplaza a un `Listo`. Una dimensión sin veredicto no tapa
/// la evidencia de otra dimensión.
fn decidir(
    v: &Veredicto,
    tarea: &Tarea,
    reglas: &ReglasDecision,
    rubricas: &[Rubrica],
) -> Decidido {
    let bloqueos = bloqueos(v, rubricas);
    if !bloqueos.is_empty() {
        return Decidido {
            decision: Decision::Bloqueado,
            disparos: bloqueos,
        };
    }
    let faltas = no_listo(v, tarea, reglas);
    if !faltas.is_empty() {
        return Decidido {
            decision: Decision::NoListo,
            disparos: faltas,
        };
    }
    let sin = sin_decision(v, rubricas);
    if !sin.is_empty() {
        return Decidido {
            decision: Decision::SinDecision,
            disparos: sin,
        };
    }
    Decidido {
        decision: Decision::Listo,
        disparos: Vec::new(),
    }
}

/// Dimensiones sin veredicto que dejan la decisión en `SinDecision` si no hay evidencia de `Bloqueado` ni de
/// `NoListo` (decisión 48, desempate 9 y Tarea 32). Una dimensión cuya rúbrica tiene un ítem `bloquea = true`
/// lo hace. Completitud lo hace siempre, aunque su rúbrica no bloquee. Una dimensión sin veredicto sin ítems
/// que bloquean no cambia la decisión.
fn sin_decision(v: &Veredicto, rubricas: &[Rubrica]) -> Vec<Disparo> {
    let mut salida = Vec::new();
    for p in &v.puntajes {
        let Some(motivo) = p.sin_veredicto.as_deref() else {
            continue;
        };
        if bloquea_la_dimension(p.dimension, rubricas) {
            salida.push(Disparo {
                regla: format!(
                    "dimension_que_bloquea_sin_veredicto: {}",
                    p.dimension.clave()
                ),
                frase: format!(
                    "{} no tiene veredicto y su rúbrica tiene ítems que bloquean: {motivo}.",
                    nombre_dimension(p.dimension)
                ),
                hallazgo: None,
            });
        } else if p.dimension == Dimension::Completitud {
            salida.push(completitud_sin_veredicto(Some(motivo)));
        }
    }
    let hay_completitud = v
        .puntajes
        .iter()
        .any(|p| p.dimension == Dimension::Completitud);
    if !hay_completitud {
        salida.push(completitud_sin_veredicto(None));
    }
    salida
}

/// Verdadero si la referencia `prueba` cubre el test `test`: el mismo id, o un id dentro de ese archivo.
/// Es el mismo criterio para `pruebas`, `bordes` y `riesgos`.
fn cubre(prueba: &str, test: &str) -> bool {
    test == prueba || test.starts_with(&format!("{prueba}::"))
}

/// Tests ocultos que fallan y que ningún requisito prueba. Un requisito prueba un test si lo nombra en
/// `pruebas` o `bordes`, por id exacto o por archivo. Una referencia `contrato:` apunta al contrato y no
/// cubre un test oculto.
fn tests_fuera_de_requisitos(v: &Veredicto, tarea: &Tarea) -> Vec<String> {
    v.ocultos_fallidos
        .iter()
        .filter(|test| {
            !tarea.requisito.iter().any(|r| {
                r.pruebas
                    .iter()
                    .chain(&r.bordes)
                    .any(|prueba| cubre(prueba, test))
            })
        })
        .cloned()
        .collect()
}

/// Tests del contrato que fallan y que ningún requisito prueba. Un requisito lo prueba si nombra el test con
/// `contrato:` en `pruebas` o `bordes`, por id exacto o por archivo. Una referencia sin prefijo apunta a la
/// suite oculta y no lo cubre.
fn tests_contrato_fuera_de_requisitos(v: &Veredicto, tarea: &Tarea) -> Vec<String> {
    v.contrato_fallidos
        .iter()
        .filter(|test| {
            !tarea.requisito.iter().any(|r| {
                r.pruebas.iter().chain(&r.bordes).any(|prueba| {
                    prueba
                        .strip_prefix(PREFIJO_CONTRATO)
                        .is_some_and(|referencia| cubre(referencia, test))
                })
            })
        })
        .cloned()
        .collect()
}

/// Verdadero si la rúbrica de `dimension` tiene algún ítem con `bloquea = true`.
fn bloquea_la_dimension(dimension: Dimension, rubricas: &[Rubrica]) -> bool {
    rubricas
        .iter()
        .filter(|r| r.id == dimension.clave())
        .flat_map(|r| r.item.iter())
        .any(|i| i.bloquea)
}

/// Sonda que encuentra secretos. Sus hallazgos son secretos.
const SONDA_SECRETOS: &str = "gitleaks";

/// Verdadero si el hallazgo es un secreto: lo encontró gitleaks.
fn es_secreto(h: &Hallazgo) -> bool {
    matches!(&h.fuente, Fuente::Sonda(sonda) if sonda == SONDA_SECRETOS)
}

/// Reglas que bloquean: un requisito de peso 3 en nivel 0, un hallazgo de seguridad de severidad 3 que el
/// cambio introduce (un secreto bloquea aunque sea previo), y un ítem de rúbrica con `bloquea = true` en nivel 0.
fn bloqueos(v: &Veredicto, rubricas: &[Rubrica]) -> Vec<Disparo> {
    let mut salida = Vec::new();
    for e in v
        .requisitos
        .iter()
        .filter(|e| e.peso == PESO_BLOQUEANTE && e.nivel == Some(0))
    {
        salida.push(Disparo {
            regla: format!(
                "bloqueo_requisito: {} {} queda en nivel 0 con peso {PESO_BLOQUEANTE}",
                e.id, e.titulo
            ),
            frase: format!(
                "{} {} queda en nivel 0 y pesa {PESO_BLOQUEANTE}.",
                e.id, e.titulo
            ),
            hallazgo: None,
        });
    }
    // Un aviso previo no bloquea. Un secreto bloquea siempre, sea previo o no.
    for h in v.hallazgos.iter().filter(|h| {
        h.dimension == Dimension::Seguridad
            && h.severidad == SEVERIDAD_BLOQUEANTE
            && (!h.previo || es_secreto(h))
    }) {
        let frase = if h.previo {
            format!(
                "{} es un hallazgo de seguridad con severidad {SEVERIDAD_BLOQUEANTE}; estaba en el estado inicial; los secretos bloquean siempre.",
                h.id
            )
        } else {
            format!(
                "{} es un hallazgo de seguridad con severidad {SEVERIDAD_BLOQUEANTE}.",
                h.id
            )
        };
        salida.push(Disparo {
            regla: format!(
                "bloqueo_seguridad: hallazgo {} de seguridad con severidad {SEVERIDAD_BLOQUEANTE}",
                h.id
            ),
            frase,
            hallazgo: Some(h.id.clone()),
        });
    }
    let bloquean: BTreeSet<&str> = rubricas
        .iter()
        .flat_map(|r| r.item.iter())
        .filter(|i| i.bloquea)
        .map(|i| i.id.as_str())
        .collect();
    for item in v
        .puntajes
        .iter()
        .flat_map(|p| p.items.iter())
        .filter(|i| i.nivel == 0 && bloquean.contains(i.item.as_str()))
    {
        salida.push(Disparo {
            regla: format!(
                "bloqueo_rubrica: {} queda en nivel 0 con bloquea = true",
                item.item
            ),
            frase: format!("{} queda en nivel 0 y bloquea la decisión.", item.item),
            hallazgo: None,
        });
    }
    salida
}

/// Reglas de no listo: un producto o sus tests que no compilan, un test oculto o del contrato que falla y
/// ningún requisito prueba, la completitud con veredicto bajo su mínimo, y una dimensión calibrada bajo su mínimo de
/// `[decision.minimo]`. Una completitud sin veredicto no dispara aquí: `sin_decision` la cubre si no hay evidencia.
fn no_listo(v: &Veredicto, tarea: &Tarea, reglas: &ReglasDecision) -> Vec<Disparo> {
    let mut salida = Vec::new();
    if v.no_compila {
        salida.push(Disparo {
            regla: "no_compila: el producto o sus tests no compilan".into(),
            frase: "El producto o sus tests no compilan, así que no está listo.".into(),
            hallazgo: None,
        });
    }
    for id in tests_fuera_de_requisitos(v, tarea) {
        salida.push(Disparo {
            regla: format!("test_oculto_fuera_de_requisitos: {id}"),
            frase: format!("El test oculto {id} falla y ningún requisito lo prueba."),
            hallazgo: None,
        });
    }
    for id in tests_contrato_fuera_de_requisitos(v, tarea) {
        salida.push(Disparo {
            regla: format!("test_contrato_fuera_de_requisitos: {id}"),
            frase: format!("El test del contrato {id} falla y ningún requisito lo prueba."),
            hallazgo: None,
        });
    }
    // Completitud sin veredicto no entra: `sin_decision` la cubre. Aquí solo se mide el mínimo.
    let completitud = v
        .puntajes
        .iter()
        .find(|p| p.dimension == Dimension::Completitud && p.sin_veredicto.is_none());
    if let Some(p) = completitud {
        let minima = reglas.completitud_minima as f32;
        if p.valor < minima {
            let (valor_txt, minima_txt) = numeros_contra_umbral(p.valor, minima);
            let sin_cumplir: Vec<&str> = v
                .requisitos
                .iter()
                .filter(|e| sin_cumplir(e))
                .map(|e| e.id.as_str())
                .collect();
            let cita = if sin_cumplir.is_empty() {
                String::new()
            } else {
                format!("; sin cumplir: {}", unir(&sin_cumplir))
            };
            salida.push(Disparo {
                regla: format!("completitud_bajo_minimo: completitud {valor_txt} < {minima_txt}"),
                frase: format!("La completitud es {valor_txt} y el mínimo es {minima_txt}{cita}."),
                hallazgo: None,
            });
        }
    }
    for p in v
        .puntajes
        .iter()
        .filter(|p| p.calibrada && p.sin_veredicto.is_none())
    {
        let Some(minimo) = reglas.minimo_de(p.dimension) else {
            continue;
        };
        if p.valor < minimo as f32 {
            let nombre = nombre_dimension(p.dimension);
            let hallazgos: Vec<&str> = v
                .hallazgos
                .iter()
                .filter(|h| h.dimension == p.dimension && !h.id.is_empty())
                .map(|h| h.id.as_str())
                .collect();
            let cita = if hallazgos.is_empty() {
                String::new()
            } else {
                format!("; hallazgos: {}", unir(&hallazgos))
            };
            let (valor_txt, minimo_txt) = numeros_contra_umbral(p.valor, minimo as f32);
            salida.push(Disparo {
                regla: format!(
                    "minimo_de_dimension: {nombre} calibrada en {valor_txt} < mínimo {minimo_txt}"
                ),
                frase: format!(
                    "{nombre} calibrada queda en {valor_txt} y su mínimo es {minimo_txt}{cita}."
                ),
                hallazgo: None,
            });
        }
    }
    salida
}

/// Disparo de la completitud sin veredicto: la decisión queda cerrada en no listo.
fn completitud_sin_veredicto(motivo: Option<&str>) -> Disparo {
    let motivo = motivo.unwrap_or("sin puntaje de completitud");
    Disparo {
        regla: format!("completitud_sin_veredicto: {motivo}"),
        frase: format!("La completitud no tiene veredicto: {motivo}."),
        hallazgo: None,
    }
}

/// Verdadero si el requisito no está cumplido: ausente o parcial.
fn sin_cumplir(e: &EstadoRequisito) -> bool {
    matches!(
        e.cumplimiento,
        Cumplimiento::Ausente | Cumplimiento::Parcial
    )
}

/// Resumen determinista: una frase por disparo, con sus ids. Sin disparos, dice qué cumple.
fn resumen(v: &Veredicto, disparos: &[Disparo]) -> String {
    if disparos.is_empty() {
        let cumplen: Vec<&str> = v
            .requisitos
            .iter()
            .filter(|e| e.cumplimiento == Cumplimiento::Cumplido)
            .map(|e| e.id.as_str())
            .collect();
        return if cumplen.is_empty() {
            "Listo: ninguna regla de bloqueo ni de no listo disparó.".to_string()
        } else {
            format!("Listo: cumplen {}.", unir(&cumplen))
        };
    }
    let frases: Vec<&str> = disparos.iter().map(|d| d.frase.as_str()).collect();
    frases.join(" ")
}

/// Un candidato a acción con sus claves de orden: el peso del requisito afectado y la severidad.
struct Candidato {
    accion: Accion,
    peso: u8,
    severidad: u8,
}

/// Ordena por peso y severidad, de mayor a menor, y se queda con `MAX_ITEMS`. Las prioridades van de 1.
fn ordenar(mut candidatos: Vec<Candidato>) -> Vec<Accion> {
    candidatos.sort_by_key(|c| std::cmp::Reverse((c.peso, c.severidad)));
    candidatos
        .into_iter()
        .take(MAX_ITEMS)
        .enumerate()
        .map(|(i, mut c)| {
            c.accion.prioridad = (i + 1) as u8;
            c.accion
        })
        .collect()
}

/// Peso más alto de los requisitos que nombran `ids`. Cero si ninguno.
fn peso_de(ids: &[String], v: &Veredicto) -> u8 {
    ids.iter()
        .filter_map(|id| v.requisitos.iter().find(|e| &e.id == id).map(|e| e.peso))
        .max()
        .unwrap_or(0)
}

/// Los ids de `impacto` que son requisitos del veredicto.
fn impacto_real(v: &Veredicto, impacto: &[String]) -> Vec<String> {
    impacto
        .iter()
        .filter(|id| v.requisitos.iter().any(|e| &e.id == *id))
        .cloned()
        .collect()
}

/// Riesgos: hallazgos de severidad 2 o más, y tests inestables que ningún hallazgo nombra y que un
/// requisito prueba. Un test del contrato se nombra con `contrato:`. Cada uno cita sus ids reales.
fn riesgos(v: &Veredicto, tarea: &Tarea) -> Vec<Accion> {
    let mut candidatos = Vec::new();
    for h in v
        .hallazgos
        .iter()
        .filter(|h| h.severidad >= SEVERIDAD_RIESGO && !h.id.is_empty())
    {
        let impacto = impacto_real(v, &h.impacto);
        let afecta = if impacto.is_empty() {
            String::new()
        } else {
            format!(" (afecta {})", unir(&impacto))
        };
        let mut refs = vec![h.id.clone()];
        refs.extend(impacto.iter().cloned());
        candidatos.push(Candidato {
            peso: peso_de(&impacto, v),
            severidad: h.severidad,
            accion: Accion {
                prioridad: 0,
                texto: format!(
                    "{}: {} severidad {}: {} ({}:{}){afecta}",
                    h.id,
                    nombre_dimension(h.dimension),
                    h.severidad,
                    h.detalle,
                    h.archivo,
                    h.linea
                ),
                refs,
            },
        });
    }
    for (clave, tasa) in &v.tasas {
        // Un test oculto se nombra sin prefijo, como en las referencias. Uno del contrato se nombra con
        // `contrato:`, también en las referencias y en la cita de sus hallazgos.
        let test = match clave.strip_prefix("oculto:") {
            Some(id) => id,
            None if clave.starts_with(PREFIJO_CONTRATO) => clave.as_str(),
            None => continue,
        };
        if v.hallazgos.iter().any(|h| h.cita == test) {
            continue;
        }
        let refs: Vec<String> = tarea
            .requisito
            .iter()
            .filter(|r| {
                r.pruebas
                    .iter()
                    .chain(&r.bordes)
                    .any(|prueba| cubre(prueba, test))
            })
            .map(|r| r.id.clone())
            .filter(|id| v.requisitos.iter().any(|e| &e.id == id))
            .collect();
        if refs.is_empty() {
            continue;
        }
        candidatos.push(Candidato {
            peso: peso_de(&refs, v),
            severidad: SEVERIDAD_RIESGO,
            accion: Accion {
                prioridad: 0,
                texto: format!(
                    "{}: el test {test} es inestable ({} de {} corridas pasan)",
                    unir(&refs),
                    tasa.pasa,
                    tasa.total
                ),
                refs,
            },
        });
    }
    ordenar(candidatos)
}

/// Próximos pasos: los bloqueantes primero, luego los requisitos ausentes o parciales. Cada uno cita
/// su requisito o su hallazgo.
fn proximos_pasos(
    v: &Veredicto,
    tarea: &Tarea,
    bloqueantes: &[String],
    presentes: &BTreeSet<String>,
) -> Vec<Accion> {
    let mut candidatos = Vec::new();
    for h in v
        .hallazgos
        .iter()
        .filter(|h| !h.id.is_empty() && bloqueantes.contains(&h.id))
    {
        candidatos.push(Candidato {
            peso: PESO_BLOQUEANTE,
            severidad: SEVERIDAD_BLOQUEANTE,
            accion: Accion {
                prioridad: 0,
                texto: format!(
                    "{}: corregir {} ({}:{})",
                    h.id, h.detalle, h.archivo, h.linea
                ),
                refs: vec![h.id.clone()],
            },
        });
    }
    let modulos: BTreeMap<&str, &[String]> = tarea
        .requisito
        .iter()
        .map(|r| (r.id.as_str(), r.modulos.as_slice()))
        .collect();
    for e in v.requisitos.iter().filter(|e| sin_cumplir(e)) {
        let ausente = e.cumplimiento == Cumplimiento::Ausente;
        let estado = if ausente { "ausente" } else { "parcial" };
        // Sin perfil, Root no conoce los módulos del repo: nombra los tests que no pasan (decisión 56).
        let del_requisito: &[String] = if tiene_perfil(&tarea.lenguaje) {
            modulos.get(e.id.as_str()).copied().unwrap_or(&[])
        } else {
            &[]
        };
        let falta = faltante(e, del_requisito, presentes);
        candidatos.push(Candidato {
            peso: e.peso,
            severidad: if ausente { 2 } else { 1 },
            accion: Accion {
                prioridad: 0,
                texto: format!("{} {}: {estado}{falta}", e.id, e.titulo),
                refs: vec![e.id.clone()],
            },
        });
    }
    ordenar(candidatos)
}

/// Lo que falta de un requisito sin cumplir. Sin módulos, sus pruebas que no pasan. Con módulos, los que
/// existen en el repo para revisar; si ninguno existe, la lista entera, que falta.
/// `proximos_pasos` pasa `modulos` vacía si la tarea no tiene perfil: Root no conoce sus módulos.
fn faltante(e: &EstadoRequisito, modulos: &[String], presentes: &BTreeSet<String>) -> String {
    if modulos.is_empty() {
        let sin_pasar: Vec<&str> = e
            .evidencia
            .iter()
            .filter(|x| !x.ends_with(": pasa"))
            .map(String::as_str)
            .take(3)
            .collect();
        return if sin_pasar.is_empty() {
            "; revisar sus pruebas".to_string()
        } else {
            format!("; sin cumplir: {}", unir(&sin_pasar))
        };
    }
    let existen: Vec<&str> = modulos
        .iter()
        .map(String::as_str)
        .filter(|m| presentes.contains(*m))
        .collect();
    match existen.as_slice() {
        [] if modulos.len() == 1 => format!("; falta el módulo {}", modulos[0]),
        [] => format!("; faltan los módulos {}", unir(modulos)),
        [uno] => format!("; revisar el módulo {uno}"),
        varios => format!("; revisar los módulos {}", unir(varios)),
    }
}

/// Ids que el redactor puede citar: requisitos y hallazgos del veredicto.
fn ids_citables(v: &Veredicto) -> BTreeSet<String> {
    v.requisitos
        .iter()
        .map(|e| e.id.clone())
        .chain(
            v.hallazgos
                .iter()
                .filter(|h| !h.id.is_empty())
                .map(|h| h.id.clone()),
        )
        .collect()
}

/// Pasa los textos del redactor por el gate de evidencia: cada oración debe citar un id real. Lo que no
/// pasa queda con el texto determinista. `visibles` dice qué acción vio el redactor: su texto `k` va a
/// la acción `visibles.riesgos[k]` del informe completo.
fn aplicar_redaccion(
    informe: &mut Informe,
    redaccion: Redaccion,
    ids: &BTreeSet<String>,
    visibles: &IndicesVisibles,
) {
    if let Some(texto) = conservadas(&redaccion.resumen, ids) {
        informe.resumen = texto;
    }
    if let Some(textos) = alineados(
        &redaccion.riesgos,
        visibles.riesgos.len(),
        &mut informe.confianza.avisos,
    ) {
        for (&pos, texto) in visibles.riesgos.iter().zip(textos) {
            if let Some(nuevo) = conservadas(texto, ids) {
                informe.riesgos[pos].texto = nuevo;
            }
        }
    }
    if let Some(textos) = alineados(
        &redaccion.proximos_pasos,
        visibles.proximos_pasos.len(),
        &mut informe.confianza.avisos,
    ) {
        for (&pos, texto) in visibles.proximos_pasos.iter().zip(textos) {
            if let Some(nuevo) = conservadas(texto, ids) {
                informe.proximos_pasos[pos].texto = nuevo;
            }
        }
    }
}

/// La lista del redactor, si tiene una entrada por cada acción visible. Si no, `None` y un aviso: el
/// texto `k` no sabe a qué acción va, así que no se aplica ninguno.
fn alineados<'a>(
    textos: &'a [String],
    visibles: usize,
    avisos: &mut Vec<String>,
) -> Option<&'a [String]> {
    if textos.len() == visibles {
        return Some(textos);
    }
    avisos.push(format!(
        "el redactor devolvió {} textos para {visibles} acciones; se conservan los originales",
        textos.len()
    ));
    None
}

/// Las oraciones de `texto` que citan un id de `ids`, unidas. `None` si no queda ninguna.
fn conservadas(texto: &str, ids: &BTreeSet<String>) -> Option<String> {
    let (buenas, _) = oraciones_con_id(texto, ids);
    (!buenas.is_empty()).then(|| buenas.join(" "))
}

/// Marca de los requisitos cuya evidencia es solo del contrato (decisión 50).
const MARCA_EVIDENCIA_VISIBLE: &str = "evidencia visible al builder";

/// Requisitos con evidencia medida solo del contrato. Entra si alguna referencia `contrato:` de `pruebas` o
/// `bordes` aporta un resultado medido (`pasa`, `falla` o `inestable`) a la evidencia del requisito, y ninguna
/// referencia sin prefijo lo aporta. Un requisito cuyo único test `contrato:` está `ausente`, `omite`,
/// `pendiente` o `excluido` no entra. Así el builder ve lo que se midió del contrato, y un test oculto medido
/// lo sigue ocultando.
fn evidencia_visible(v: &Veredicto, tarea: &Tarea) -> Vec<String> {
    tarea
        .requisito
        .iter()
        .filter_map(|r| {
            let estado = v.requisitos.iter().find(|e| e.id == r.id)?;
            let refs: Vec<&String> = r.pruebas.iter().chain(&r.bordes).collect();
            let contrato_medido = refs
                .iter()
                .filter(|x| x.starts_with(PREFIJO_CONTRATO))
                .any(|x| aporta_medida(estado, x));
            let oculto_medido = refs
                .iter()
                .filter(|x| !x.starts_with(PREFIJO_CONTRATO))
                .any(|x| aporta_medida(estado, x));
            (contrato_medido && !oculto_medido)
                .then(|| format!("{}: {MARCA_EVIDENCIA_VISIBLE}", r.id))
        })
        .collect()
}

/// Si la referencia `referencia` aporta un resultado medido a `estado`: una entrada `<clave>: <estado>` con
/// `estado` en `pasa`, `falla` o `inestable`, cuya `clave` tiene el mismo prefijo `contrato:` que la referencia
/// y queda cubierta por ella. Las entradas `pendiente: `, `excluido: `, `omite` y `ausente` no son resultados
/// medidos.
fn aporta_medida(estado: &EstadoRequisito, referencia: &str) -> bool {
    let del_contrato = referencia.starts_with(PREFIJO_CONTRATO);
    let prueba = referencia
        .strip_prefix(PREFIJO_CONTRATO)
        .unwrap_or(referencia);
    estado.evidencia.iter().any(|linea| {
        if linea.starts_with("pendiente: ") || linea.starts_with("excluido: ") {
            return false;
        }
        let Some((clave, resultado)) = linea.rsplit_once(": ") else {
            return false;
        };
        let clave_del_contrato = clave.starts_with(PREFIJO_CONTRATO);
        let clave_sin_prefijo = clave.strip_prefix(PREFIJO_CONTRATO).unwrap_or(clave);
        matches!(resultado, "pasa" | "falla" | "inestable")
            && clave_del_contrato == del_contrato
            && cubre(prueba, clave_sin_prefijo)
    })
}

/// Datos para decidir: lo que el veredicto no mide del todo.
fn confianza(v: &Veredicto, tarea: &Tarea) -> Confianza {
    Confianza {
        items_inestables: v
            .puntajes
            .iter()
            .filter(|p| !p.inestables.is_empty())
            .map(|p| (p.dimension, p.inestables.clone()))
            .collect(),
        repeticiones: v.repeticiones,
        inestables: v.tasas.clone(),
        corridas_acumuladas: v.corridas_acumuladas.clone(),
        requisitos_pendientes: v
            .requisitos
            .iter()
            .filter(|e| e.cumplimiento == Cumplimiento::Pendiente)
            .map(|e| e.id.clone())
            .collect(),
        tests_pendientes: v.pendientes.clone(),
        ambiguedades: v.ambiguedades.clone(),
        sin_calibrar: v.sin_calibrar.clone(),
        motivos_sin_calibrar: v
            .puntajes
            .iter()
            .filter_map(|p| {
                p.motivo_sin_calibrar
                    .as_ref()
                    .map(|motivo| (p.dimension, motivo.clone()))
            })
            .collect(),
        sin_veredicto: v
            .puntajes
            .iter()
            .filter_map(|p| {
                p.sin_veredicto.as_ref().map(|motivo| ParteSinVeredicto {
                    dimension: p.dimension,
                    motivo: motivo.clone(),
                })
            })
            .collect(),
        avisos: v.avisos.clone(),
        faltantes: v
            .puntajes
            .iter()
            .flat_map(|p| p.faltantes.clone())
            .collect(),
        evidencia_visible: evidencia_visible(v, tarea),
    }
}

/// Ids de tests ocultos que el veredicto menciona: inestables, tasas `oculto:` y pendientes sin prefijo
/// `contrato:` (un pendiente del contrato no es oculto).
pub fn ids_ocultos(v: &Veredicto) -> BTreeSet<String> {
    let mut ocultos: BTreeSet<String> = v.inestables.iter().cloned().collect();
    ocultos.extend(v.ocultos_fallidos.iter().cloned());
    ocultos.extend(
        v.tasas
            .keys()
            .filter_map(|k| k.strip_prefix("oculto:").map(String::from)),
    );
    ocultos.extend(
        v.pendientes
            .iter()
            .filter(|p| !p.test.starts_with(PREFIJO_CONTRATO))
            .map(|p| p.test.clone()),
    );
    ocultos
}

/// Posiciones, en el informe completo, de las acciones que `ocultar_tests` deja. El redactor recibe
/// esas acciones en orden: su texto `k` es para la posición `k` de esta lista.
#[derive(Debug, Default, PartialEq)]
pub struct IndicesVisibles {
    pub riesgos: Vec<usize>,
    pub proximos_pasos: Vec<usize>,
}

/// Quita del informe lo que cita un test oculto: las acciones que lo nombran, el motivo y el resumen si
/// lo nombran, y los tests de la confianza y de la evidencia de los requisitos. Devuelve las posiciones
/// de las acciones que quedan.
pub fn ocultar_tests(informe: &mut Informe, ocultos: &BTreeSet<String>) -> IndicesVisibles {
    let cita = |texto: &str| ocultos.iter().any(|id| texto.contains(id.as_str()));
    let riesgos = retener(&mut informe.riesgos, |a| !cita(&a.texto));
    let proximos_pasos = retener(&mut informe.proximos_pasos, |a| !cita(&a.texto));
    if cita(&informe.resumen) {
        informe.resumen = "Resumen con ids de tests ocultos: ver la CLI.".into();
    }
    if cita(&informe.motivo) {
        informe.motivo = "motivo con ids de tests ocultos: ver la CLI".into();
    }
    for e in &mut informe.requisitos {
        e.evidencia.retain(|x| !cita(x));
    }
    informe
        .confianza
        .inestables
        .retain(|clave, _| !cita(clave.as_str()));
    informe
        .confianza
        .corridas_acumuladas
        .retain(|clave, _| !cita(clave.as_str()));
    // Un pendiente del contrato (`contrato:`) sigue el informe, salvo que su `test` o su `motivo` cite un
    // oculto. Los que no llevan prefijo son ocultos y `ids_ocultos` ya los nombra: salen por `cita`.
    informe
        .confianza
        .tests_pendientes
        .retain(|p| !cita(&p.test) && !cita(&p.motivo));
    IndicesVisibles {
        riesgos,
        proximos_pasos,
    }
}

/// Como `Vec::retain`. Devuelve las posiciones originales de los elementos que se quedan.
fn retener<T>(items: &mut Vec<T>, quedar: impl Fn(&T) -> bool) -> Vec<usize> {
    let mut posiciones = Vec::new();
    let mut original = 0;
    items.retain(|item| {
        let queda = quedar(item);
        if queda {
            posiciones.push(original);
        }
        original += 1;
        queda
    });
    posiciones
}

/// Markdown legible: la decisión con su confianza, por qué, qué falta, qué arreglar primero y riesgos.
pub fn markdown(tarea: &str, informe: &Informe) -> String {
    let mut md = format!(
        "# Informe de {tarea}\n\n## Decisión\n\n**{}**\n\n{}\n\nRegla que decidió: {}.\n\n",
        titular(informe),
        informe.resumen,
        informe.motivo
    );
    if !informe.bloqueantes.is_empty() {
        md.push_str(&format!("Bloqueantes: {}.\n\n", unir(&informe.bloqueantes)));
    }
    md.push_str("## Qué falta\n\n");
    let faltan: Vec<String> = informe
        .requisitos
        .iter()
        .filter(|e| e.cumplimiento != Cumplimiento::Cumplido)
        .map(|e| {
            format!(
                "- {} {}: {}",
                e.id,
                e.titulo,
                etiqueta_cumplimiento(e.cumplimiento)
            )
        })
        .collect();
    lista(&mut md, &faltan, "Ningún requisito queda sin cumplir.");
    md.push_str("\n## Qué arreglar primero\n\n");
    let pasos: Vec<String> = informe
        .proximos_pasos
        .iter()
        .map(|a| format!("{}. {}", a.prioridad, a.texto))
        .collect();
    lista(&mut md, &pasos, "Nada que arreglar.");
    md.push_str("\n## Riesgos\n\n");
    let riesgos: Vec<String> = informe
        .riesgos
        .iter()
        .map(|a| format!("- {}", a.texto))
        .collect();
    lista(&mut md, &riesgos, "Sin riesgos registrados.");
    if !informe.avisos_previos.is_empty() {
        md.push_str("\n## Avisos previos\n\n");
        let avisos: Vec<String> = informe
            .avisos_previos
            .iter()
            .map(|a| {
                format!(
                    "- {}: {} en {} (severidad {})",
                    a.fuente, a.aviso, a.objeto, a.severidad
                )
            })
            .collect();
        lista(&mut md, &avisos, "Sin avisos previos.");
        md.push_str("\nEstaban en el estado inicial; no bloquean.\n");
    }
    md.push_str("\n## Confianza\n\n");
    md.push_str(&format!(
        "- Repeticiones del oculto: {}.\n",
        informe.confianza.repeticiones
    ));
    let c = &informe.confianza;
    let mut datos: Vec<String> = Vec::new();
    for (clave, tasa) in &c.inestables {
        // N ya es la cantidad de corridas acumuladas: no se repite en la línea.
        datos.push(format!(
            "- Inestable {clave}: pasa {} de {} (cota de Wilson {})",
            tasa.pasa,
            tasa.total,
            numero(tasa.wilson_inf as f32)
        ));
    }
    if !c.requisitos_pendientes.is_empty() {
        datos.push(format!(
            "- Requisitos pendientes por entorno: {}",
            unir(&c.requisitos_pendientes)
        ));
    }
    for entrada in &c.evidencia_visible {
        datos.push(format!("- {entrada}"));
    }
    for p in &c.tests_pendientes {
        datos.push(format!("- Test pendiente {}: {}", p.test, p.motivo));
    }
    if !c.sin_calibrar.is_empty() {
        let nombres: Vec<String> = c
            .sin_calibrar
            .iter()
            .map(|d| match c.motivos_sin_calibrar.get(d) {
                Some(motivo) => format!("{} ({motivo})", nombre_dimension(*d)),
                None => nombre_dimension(*d).to_string(),
            })
            .collect();
        datos.push(format!("- Sin calibrar: {}", unir(&nombres)));
    }
    for (dimension, items) in &c.items_inestables {
        datos.push(format!(
            "- Ítems inestables {}: {}",
            nombre_dimension(*dimension),
            items.join(", ")
        ));
    }
    for parte in &c.sin_veredicto {
        datos.push(format!(
            "- Sin veredicto {}: {}",
            nombre_dimension(parte.dimension),
            parte.motivo
        ));
    }
    for falta in &c.faltantes {
        datos.push(format!("- Sin medir {}: {}", falta.item, falta.motivo));
    }
    if c.ambiguedades.is_empty() {
        datos.push("- Ambigüedades abiertas: sin datos, el veredicto no trae la ficha.".into());
    }
    for aviso in &c.avisos {
        datos.push(format!("- Aviso: {aviso}"));
    }
    for linea in datos {
        md.push_str(&linea);
        md.push('\n');
    }
    md
}

/// Decisión con su confianza: "Listo con 3 dimensiones sin calibrar y 1 dimensión sin veredicto".
fn titular(informe: &Informe) -> String {
    let etiqueta = match informe.decision {
        Decision::Listo => "Listo",
        Decision::NoListo => "No listo",
        Decision::Bloqueado => "Bloqueado",
        Decision::SinDecision => "Sin decisión",
    };
    let c = &informe.confianza;
    let mut clausulas = Vec::new();
    if !c.sin_calibrar.is_empty() {
        clausulas.push(contar(c.sin_calibrar.len(), "sin calibrar"));
    }
    if !c.sin_veredicto.is_empty() {
        clausulas.push(contar(c.sin_veredicto.len(), "sin veredicto"));
    }
    if clausulas.is_empty() {
        etiqueta.to_string()
    } else {
        format!("{etiqueta} con {}", clausulas.join(" y "))
    }
}

/// "1 dimensión <resto>" o "N dimensiones <resto>".
fn contar(n: usize, resto: &str) -> String {
    let palabra = if n == 1 { "dimensión" } else { "dimensiones" };
    format!("{n} {palabra} {resto}")
}

/// Escribe las líneas de `items`, o `vacio` si no hay ninguna.
fn lista(md: &mut String, items: &[String], vacio: &str) {
    if items.is_empty() {
        md.push_str(vacio);
        md.push('\n');
    }
    for item in items {
        md.push_str(item);
        md.push('\n');
    }
}

fn etiqueta_cumplimiento(c: Cumplimiento) -> &'static str {
    match c {
        Cumplimiento::Cumplido => "cumplido",
        Cumplimiento::Parcial => "parcial",
        Cumplimiento::Ausente => "ausente",
        Cumplimiento::SinVeredicto => "sin veredicto",
        Cumplimiento::Pendiente => "pendiente",
    }
}

fn nombre_dimension(d: Dimension) -> &'static str {
    match d {
        Dimension::Funciona => "Funciona",
        Dimension::Seguridad => "Seguridad",
        Dimension::Eficiencia => "Eficiencia",
        Dimension::Estructura => "Estructura",
        Dimension::Arquitectura => "Arquitectura",
        Dimension::Diseno => "Diseño",
        Dimension::Completitud => "Completitud",
        Dimension::Entrega => "Entrega",
        Dimension::Pruebas => "Pruebas",
    }
}

/// Número con dos decimales y coma, como se lee en español.
fn numero(valor: f32) -> String {
    format!("{valor:.2}").replace('.', ",")
}

/// Valor y umbral de una regla que compara contra un umbral. Usa tres decimales y coma.
/// Si quedan iguales con tres decimales, usa cuatro. Así el motivo no compara dos números iguales.
fn numeros_contra_umbral(valor: f32, umbral: f32) -> (String, String) {
    let con = |n: usize| {
        (
            format!("{:.*}", n, valor).replace('.', ","),
            format!("{:.*}", n, umbral).replace('.', ","),
        )
    };
    let (valor_3, umbral_3) = con(3);
    if valor_3 != umbral_3 {
        return (valor_3, umbral_3);
    }
    con(4)
}

/// Une los textos: "a, b y c".
fn unir<S: AsRef<str>>(items: &[S]) -> String {
    match items {
        [] => String::new(),
        [uno] => uno.as_ref().to_string(),
        [resto @ .., ultimo] => {
            let previos: Vec<&str> = resto.iter().map(AsRef::as_ref).collect();
            format!("{} y {}", previos.join(", "), ultimo.as_ref())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ReglasDecision;
    use crate::oracle::rubrica::{ItemRubrica, Mide, Rubrica};
    use crate::oracle::tarea::Tarea;
    use crate::tipos::{
        AvisoPrevio, Cumplimiento, Decision, Dimension, Estado, EstadoRequisito, Fuente, Hallazgo,
        Informe, ItemFaltante, NivelItem, Pendiente, Puntaje, TasaTest, Veredicto,
    };
    use std::cell::RefCell;
    use std::fs;

    const TAREA: &str = r#"
nombre = "memora"
lenguaje = "rust"
imagen = "imagen-falsa"
spec = "spec.md"
[contrato]
cmd = "true"
[oculto]
cmd = "true"
[[requisito]]
id = "R1"
titulo = "Protocolo"
peso = 3
modulos = ["protocolo"]
pruebas = ["tests/t.py::flaky"]
[[requisito]]
id = "R2"
titulo = "Comandos"
peso = 2
modulos = ["comandos"]
[[requisito]]
id = "R3"
titulo = "Persistencia"
peso = 2
modulos = ["almacen"]
[[requisito]]
id = "R4"
titulo = "Replicación"
peso = 1
modulos = ["replica", "red"]
[[requisito]]
id = "R5"
titulo = "Expiración"
peso = 1
modulos = ["expira"]
[[requisito]]
id = "R6"
titulo = "Métricas"
peso = 1
modulos = ["metricas"]
[[requisito]]
id = "R7"
titulo = "Configuración"
peso = 1
modulos = ["config"]
"#;

    const TODAS: [Dimension; 9] = [
        Dimension::Funciona,
        Dimension::Seguridad,
        Dimension::Eficiencia,
        Dimension::Estructura,
        Dimension::Arquitectura,
        Dimension::Diseno,
        Dimension::Completitud,
        Dimension::Entrega,
        Dimension::Pruebas,
    ];

    fn tarea() -> Tarea {
        let dir = tempfile::tempdir().expect("dir");
        fs::write(dir.path().join("tarea.toml"), TAREA).expect("escribe");
        Tarea::cargar_de(dir.path()).expect("carga la tarea")
    }

    fn req(id: &str, cumplimiento: Cumplimiento, nivel: Option<u8>) -> EstadoRequisito {
        let (titulo, peso) = match id {
            "R1" => ("Protocolo", 3),
            "R2" => ("Comandos", 2),
            "R3" => ("Persistencia", 2),
            "R4" => ("Replicación", 1),
            "R5" => ("Expiración", 1),
            "R6" => ("Métricas", 1),
            _ => ("Configuración", 1),
        };
        EstadoRequisito {
            id: id.into(),
            titulo: titulo.into(),
            peso,
            cumplimiento,
            evidencia: vec![format!("test_{id}::t: falla")],
            nivel,
        }
    }

    fn cumplido(id: &str) -> EstadoRequisito {
        req(id, Cumplimiento::Cumplido, Some(3))
    }

    fn hallazgo(id: &str, dimension: Dimension, severidad: u8, impacto: &[&str]) -> Hallazgo {
        Hallazgo {
            dimension,
            severidad,
            archivo: "src/comandos.rs".into(),
            linea: 10,
            cita: "fn f() {}".into(),
            detalle: format!("detalle de {id}"),
            fuente: Fuente::Sonda("estructura".into()),
            id: id.into(),
            impacto: impacto.iter().map(|s| s.to_string()).collect(),
            previo: false,
        }
    }

    /// Veredicto con las nueve dimensiones; `completitud` None la deja sin veredicto.
    fn veredicto(
        requisitos: Vec<EstadoRequisito>,
        completitud: Option<f32>,
        hallazgos: Vec<Hallazgo>,
    ) -> Veredicto {
        let puntajes = TODAS
            .iter()
            .map(|&dimension| match (dimension, completitud) {
                (Dimension::Completitud, Some(valor)) => Puntaje {
                    inestables: Vec::new(),
                    motivo_sin_calibrar: None,
                    dimension,
                    valor,
                    calibrada: false,
                    exactitud_balanceada: None,
                    items: Vec::new(),
                    sin_veredicto: None,
                    faltantes: Vec::new(),
                },
                (Dimension::Completitud, None) => Puntaje {
                    inestables: Vec::new(),
                    motivo_sin_calibrar: None,
                    dimension,
                    valor: 0.0,
                    calibrada: false,
                    exactitud_balanceada: None,
                    items: Vec::new(),
                    sin_veredicto: Some("ningún requisito tiene nivel medido".into()),
                    faltantes: Vec::new(),
                },
                _ => Puntaje {
                    inestables: Vec::new(),
                    motivo_sin_calibrar: None,
                    dimension,
                    valor: 1.0,
                    calibrada: false,
                    exactitud_balanceada: None,
                    items: Vec::new(),
                    sin_veredicto: None,
                    faltantes: Vec::new(),
                },
            })
            .collect();
        Veredicto {
            tarea: "memora".into(),
            repo: "memora-sdlc".into(),
            commit: "abc1234".into(),
            puntajes,
            hallazgos,
            requisitos,
            repeticiones: 1,
            ..Veredicto::default()
        }
    }

    /// memora-sdlc falso: Completitud 0,5, con R2 parcial y R3 y R4 ausentes.
    fn sdlc() -> Veredicto {
        veredicto(
            vec![
                cumplido("R1"),
                req("R2", Cumplimiento::Parcial, Some(1)),
                req("R3", Cumplimiento::Ausente, Some(0)),
                req("R4", Cumplimiento::Ausente, Some(0)),
            ],
            Some(0.5),
            Vec::new(),
        )
    }

    /// memora-ecc falso: todo cumplido, con un hallazgo de estructura de severidad 2.
    fn ecc() -> Veredicto {
        veredicto(
            vec![cumplido("R1"), cumplido("R2"), cumplido("R3")],
            Some(1.0),
            vec![hallazgo(
                "h-estructura-1",
                Dimension::Estructura,
                2,
                &["R2"],
            )],
        )
    }

    fn ids_de(v: &Veredicto) -> BTreeSet<String> {
        v.requisitos
            .iter()
            .map(|r| r.id.clone())
            .chain(v.hallazgos.iter().map(|h| h.id.clone()))
            .collect()
    }

    fn informe_de(v: &Veredicto) -> Informe {
        informar_con(
            v,
            &tarea(),
            &ReglasDecision::default(),
            &[],
            &BTreeSet::new(),
            None,
        )
        .expect("informa")
    }

    /// Redactor falso que devuelve textos fijos y cuenta sus llamadas.
    struct RedactorFalso {
        redaccion: Redaccion,
    }

    impl Redactor for RedactorFalso {
        fn redactar(&self, _informe: &Informe) -> Result<Redaccion, ErrorOraculo> {
            Ok(self.redaccion.clone())
        }
    }

    /// Redactor que se cae.
    struct RedactorCaido;

    impl Redactor for RedactorCaido {
        fn redactar(&self, _informe: &Informe) -> Result<Redaccion, ErrorOraculo> {
            Err(ErrorOraculo::Infra("caída del proveedor".into()))
        }
    }

    /// Redactor que guarda el informe que recibió.
    struct RedactorEspia {
        visto: RefCell<Option<Informe>>,
    }

    impl Redactor for RedactorEspia {
        fn redactar(&self, informe: &Informe) -> Result<Redaccion, ErrorOraculo> {
            *self.visto.borrow_mut() = Some(informe.clone());
            Ok(Redaccion::default())
        }
    }

    #[test]
    fn informe_memora_sdlc_con_requisitos_ausentes_da_no_listo_por_completitud() {
        let i = informe_de(&sdlc());
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(i.motivo.contains("completitud_bajo_minimo"), "{}", i.motivo);
    }

    #[test]
    fn informe_proximos_pasos_nombra_los_requisitos_ausentes_con_su_id() {
        let i = informe_de(&sdlc());
        let r3 = i
            .proximos_pasos
            .iter()
            .find(|a| a.refs == ["R3".to_string()])
            .expect("R3 ausente va en los próximos pasos");
        assert_eq!(
            r3.texto,
            "R3 Persistencia: ausente; falta el módulo almacen"
        );
        let r4 = i
            .proximos_pasos
            .iter()
            .find(|a| a.refs == ["R4".to_string()])
            .expect("R4 ausente va en los próximos pasos");
        assert!(
            r4.texto.starts_with("R4 Replicación: ausente"),
            "{}",
            r4.texto
        );
        assert!(r4.texto.contains("replica y red"), "{}", r4.texto);
    }

    /// Tarea con un requisito R2 cuyos módulos son `modulos` (lista TOML entre corchetes, sin ellos).
    fn tarea_con_modulos(modulos: &str) -> Tarea {
        tarea_con_modulos_en("rust", modulos)
    }

    /// Como `tarea_con_modulos`, pero con el lenguaje `lenguaje` de la tarea.
    fn tarea_con_modulos_en(lenguaje: &str, modulos: &str) -> Tarea {
        let dir = tempfile::tempdir().expect("dir");
        let toml = format!(
            "nombre = \"memora\"\nlenguaje = \"{lenguaje}\"\nimagen = \"imagen-falsa\"\nspec = \"spec.md\"\n[contrato]\ncmd = \"true\"\n[oculto]\ncmd = \"true\"\n[[requisito]]\nid = \"R2\"\ntitulo = \"Comandos\"\npeso = 2\nmodulos = [{modulos}]\n"
        );
        fs::write(dir.path().join("tarea.toml"), toml).expect("escribe");
        Tarea::cargar_de(dir.path()).expect("carga la tarea")
    }

    /// Texto del próximo paso de R2 en el informe de `v` con los módulos `presentes`.
    fn paso_de_r2(v: &Veredicto, t: &Tarea, presentes: &BTreeSet<String>) -> String {
        let i =
            informar_con(v, t, &ReglasDecision::default(), &[], presentes, None).expect("informa");
        i.proximos_pasos
            .iter()
            .find(|a| a.refs == ["R2".to_string()])
            .expect("R2 va en los próximos pasos")
            .texto
            .clone()
    }

    #[test]
    fn faltante_parcial_con_un_modulo_presente_nombra_solo_ese_para_revisar() {
        let presentes = BTreeSet::from(["cmd".to_string()]);
        let v = veredicto(
            vec![req("R2", Cumplimiento::Parcial, Some(1))],
            Some(0.5),
            vec![],
        );
        let texto = paso_de_r2(&v, &tarea_con_modulos(r#""cmd", "cmd_keys""#), &presentes);
        assert_eq!(texto, "R2 Comandos: parcial; revisar el módulo cmd");
        assert!(!texto.contains("cmd_keys"), "{texto}");
    }

    #[test]
    fn faltante_ausente_con_un_modulo_presente_dice_revisar_y_no_falta() {
        let presentes = BTreeSet::from(["cmd".to_string()]);
        let v = veredicto(
            vec![req("R2", Cumplimiento::Ausente, Some(0))],
            Some(0.5),
            vec![],
        );
        let texto = paso_de_r2(&v, &tarea_con_modulos(r#""cmd""#), &presentes);
        assert_eq!(texto, "R2 Comandos: ausente; revisar el módulo cmd");
    }

    #[test]
    fn faltante_ausente_sin_ningun_modulo_presente_dice_falta_con_la_lista_entera() {
        let presentes = BTreeSet::from(["otro".to_string()]);
        let v = veredicto(
            vec![req("R2", Cumplimiento::Ausente, Some(0))],
            Some(0.5),
            vec![],
        );
        let texto = paso_de_r2(&v, &tarea_con_modulos(r#""persist", "store""#), &presentes);
        assert_eq!(
            texto,
            "R2 Comandos: ausente; faltan los módulos persist y store"
        );
    }

    #[test]
    fn faltante_parcial_sin_ningun_modulo_presente_dice_falta_con_la_lista_entera() {
        let presentes = BTreeSet::new();
        let v = veredicto(
            vec![req("R2", Cumplimiento::Parcial, Some(1))],
            Some(0.5),
            vec![],
        );
        let texto = paso_de_r2(&v, &tarea_con_modulos(r#""persist", "store""#), &presentes);
        assert_eq!(
            texto,
            "R2 Comandos: parcial; faltan los módulos persist y store"
        );
    }

    #[test]
    fn faltante_modulo_main_con_src_main_rs_dice_revisar_main() {
        let dir = tempfile::tempdir().expect("dir");
        fs::create_dir_all(dir.path().join("src")).expect("crea src");
        fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").expect("escribe main");
        let archivos = crate::oracle::perfiles::rust::archivos_rs(dir.path()).expect("lee");
        let presentes = crate::oracle::perfiles::rust::modulos_presentes(&archivos);
        let v = veredicto(
            vec![req("R2", Cumplimiento::Parcial, Some(1))],
            Some(0.5),
            vec![],
        );
        let texto = paso_de_r2(&v, &tarea_con_modulos(r#""main""#), &presentes);
        assert_eq!(texto, "R2 Comandos: parcial; revisar el módulo main");
    }

    #[test]
    fn faltante_sin_modulos_sigue_nombrando_los_tests_que_no_pasan() {
        let v = veredicto(
            vec![req("R2", Cumplimiento::Parcial, Some(1))],
            Some(0.5),
            vec![],
        );
        let texto = paso_de_r2(&v, &tarea_con_modulos(""), &BTreeSet::new());
        assert_eq!(
            texto,
            "R2 Comandos: parcial; sin cumplir: test_R2::t: falla"
        );
    }

    #[test]
    fn faltante_con_modulos_sobre_un_repo_shell_nombra_los_tests_que_no_pasan() {
        let v = veredicto(
            vec![req("R2", Cumplimiento::Parcial, Some(1))],
            Some(0.5),
            vec![],
        );
        let tarea = tarea_con_modulos_en("shell", r#""cmd", "cmd_keys""#);
        let texto = paso_de_r2(&v, &tarea, &BTreeSet::new());
        assert_eq!(
            texto,
            "R2 Comandos: parcial; sin cumplir: test_R2::t: falla"
        );
    }

    #[test]
    fn faltante_con_modulos_sobre_un_repo_shell_y_requisito_ausente_no_dice_falta_el_modulo() {
        let v = veredicto(
            vec![req("R2", Cumplimiento::Ausente, Some(0))],
            Some(0.5),
            vec![],
        );
        let tarea = tarea_con_modulos_en("shell", r#""cmd", "cmd_keys""#);
        let texto = paso_de_r2(&v, &tarea, &BTreeSet::new());
        assert_eq!(
            texto,
            "R2 Comandos: ausente; sin cumplir: test_R2::t: falla"
        );
        assert!(!texto.contains("falta el módulo"), "{texto}");
        assert!(!texto.contains("faltan los módulos"), "{texto}");
    }

    #[test]
    fn informe_memora_ecc_todo_cumplido_no_inventa_faltantes_y_cita_ids_reales() {
        let v = ecc();
        let ids = ids_de(&v);
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::Listo, "{}", i.motivo);
        assert!(i.proximos_pasos.is_empty(), "{:?}", i.proximos_pasos);
        assert!(!i.riesgos.is_empty(), "el hallazgo de estructura es riesgo");
        for accion in i.riesgos.iter().chain(&i.proximos_pasos) {
            assert!(!accion.refs.is_empty(), "{accion:?}");
            for id in &accion.refs {
                assert!(ids.contains(id), "id inventado {id} en {accion:?}");
                assert!(accion.texto.contains(id.as_str()), "{accion:?}");
            }
        }
    }

    #[test]
    fn informe_hallazgo_de_seguridad_con_severidad_3_bloquea_y_su_id_esta_en_bloqueantes() {
        let mut v = ecc();
        v.hallazgos
            .push(hallazgo("h-seguridad-3", Dimension::Seguridad, 3, &[]));
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
        assert_eq!(i.bloqueantes, vec!["h-seguridad-3".to_string()]);
        assert!(i.motivo.contains("bloqueo_seguridad"), "{}", i.motivo);
        assert_eq!(i.proximos_pasos[0].refs, vec!["h-seguridad-3".to_string()]);
    }

    /// Hallazgo de Seguridad de severidad 3 de la sonda `sonda`, previo o no.
    fn seguridad_de(id: &str, sonda: &str, previo: bool) -> Hallazgo {
        Hallazgo {
            fuente: Fuente::Sonda(sonda.into()),
            previo,
            ..hallazgo(id, Dimension::Seguridad, 3, &[])
        }
    }

    #[test]
    fn informe_siete_avisos_previos_de_cargo_audit_no_bloquean() {
        let mut v = ecc();
        v.hallazgos
            .extend((1..=7).map(|n| seguridad_de(&format!("h-audit-{n}"), "cargo-audit", true)));
        let i = informe_de(&v);
        assert_ne!(i.decision, Decision::Bloqueado, "{}", i.motivo);
        assert!(i.bloqueantes.is_empty(), "{:?}", i.bloqueantes);
    }

    #[test]
    fn informe_aviso_previo_mas_uno_nuevo_bloquea_y_cita_el_nuevo() {
        let mut v = ecc();
        v.hallazgos
            .extend((1..=7).map(|n| seguridad_de(&format!("h-audit-{n}"), "cargo-audit", true)));
        v.hallazgos
            .push(seguridad_de("h-audit-nuevo", "cargo-audit", false));
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
        assert_eq!(i.bloqueantes, vec!["h-audit-nuevo".to_string()]);
        assert!(i.motivo.contains("h-audit-nuevo"), "{}", i.motivo);
    }

    #[test]
    fn informe_secreto_previo_bloquea_y_la_frase_lo_dice() {
        let mut v = ecc();
        v.hallazgos
            .push(seguridad_de("h-secreto", "gitleaks", true));
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
        assert_eq!(i.bloqueantes, vec!["h-secreto".to_string()]);
        assert!(
            i.resumen
                .contains("estaba en el estado inicial; los secretos bloquean siempre"),
            "{}",
            i.resumen
        );
    }

    #[test]
    fn informe_veredicto_viejo_sin_previo_lee_false_y_sigue_bloqueado() {
        let mut v = ecc();
        v.hallazgos
            .push(seguridad_de("h-audit-viejo", "cargo-audit", false));
        let mut json = serde_json::to_value(&v).expect("serializa");
        for h in json["hallazgos"]
            .as_array_mut()
            .expect("lista de hallazgos")
        {
            h.as_object_mut()
                .expect("objeto")
                .remove("previo")
                .expect("el campo existe al serializar");
        }
        let viejo: Veredicto = serde_json::from_value(json).expect("lee un veredicto viejo");
        assert!(viejo.hallazgos.iter().all(|h| !h.previo));
        let i = informe_de(&viejo);
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
    }

    #[test]
    fn informe_requisito_de_peso_3_en_nivel_0_bloquea() {
        let v = veredicto(
            vec![req("R1", Cumplimiento::Ausente, Some(0)), cumplido("R2")],
            Some(0.9),
            Vec::new(),
        );
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
        assert!(i.motivo.contains("bloqueo_requisito"), "{}", i.motivo);
        assert!(i.motivo.contains("R1"), "{}", i.motivo);
        assert!(i.bloqueantes.is_empty(), "los bloqueantes son hallazgos");
    }

    #[test]
    fn informe_dimension_calibrada_bajo_su_minimo_da_no_listo_y_el_motivo_la_nombra() {
        let mut v = ecc();
        let seguridad = v
            .puntajes
            .iter_mut()
            .find(|p| p.dimension == Dimension::Seguridad)
            .expect("seguridad");
        seguridad.valor = 0.6;
        seguridad.calibrada = true;
        let reglas = ReglasDecision {
            minimo: BTreeMap::from([("seguridad".to_string(), 0.8)]),
            ..ReglasDecision::default()
        };
        let i = informar_con(&v, &tarea(), &reglas, &[], &BTreeSet::new(), None).expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(i.motivo.contains("minimo_de_dimension"), "{}", i.motivo);
        assert!(i.motivo.contains("Seguridad"), "{}", i.motivo);
        // Sin calibrar, el mínimo no aplica.
        let seguridad = v
            .puntajes
            .iter_mut()
            .find(|p| p.dimension == Dimension::Seguridad)
            .expect("seguridad");
        seguridad.calibrada = false;
        let i = informar_con(&v, &tarea(), &reglas, &[], &BTreeSet::new(), None).expect("informa");
        assert_eq!(i.decision, Decision::Listo, "{}", i.motivo);
    }

    #[test]
    fn informe_numeros_contra_umbral_aplica_la_regla_de_los_decimales() {
        let par = |a: &str, b: &str| (a.to_string(), b.to_string());
        assert_eq!(numeros_contra_umbral(0.896, 0.90), par("0,896", "0,900"));
        assert_eq!(numeros_contra_umbral(0.8996, 0.90), par("0,8996", "0,9000"));
        assert_eq!(numeros_contra_umbral(0.6, 0.8), par("0,600", "0,800"));
    }

    #[test]
    fn informe_motivo_de_completitud_0_896_contra_0_90_muestra_tres_decimales() {
        let v = veredicto(Vec::new(), Some(0.896), Vec::new());
        let reglas = ReglasDecision {
            completitud_minima: 0.90,
            ..ReglasDecision::default()
        };
        let i = informar_con(&v, &tarea(), &reglas, &[], &BTreeSet::new(), None).expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(i.motivo.contains("0,896 < 0,900"), "{}", i.motivo);
        assert!(
            i.resumen
                .contains("La completitud es 0,896 y el mínimo es 0,900"),
            "{}",
            i.resumen
        );
    }

    #[test]
    fn informe_motivo_iguales_con_tres_decimales_muestra_cuatro() {
        // 0,8996 y 0,90 son iguales con tres decimales (0,900): la regla sube a cuatro.
        let v = veredicto(Vec::new(), Some(0.8996), Vec::new());
        let reglas = ReglasDecision {
            completitud_minima: 0.90,
            ..ReglasDecision::default()
        };
        let i = informar_con(&v, &tarea(), &reglas, &[], &BTreeSet::new(), None).expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(i.motivo.contains("0,8996 < 0,9000"), "{}", i.motivo);
        assert!(
            i.resumen
                .contains("La completitud es 0,8996 y el mínimo es 0,9000"),
            "{}",
            i.resumen
        );
    }

    #[test]
    fn informe_motivo_de_dimension_calibrada_muestra_tres_decimales() {
        let mut v = ecc();
        let seguridad = v
            .puntajes
            .iter_mut()
            .find(|p| p.dimension == Dimension::Seguridad)
            .expect("seguridad");
        seguridad.valor = 0.6;
        seguridad.calibrada = true;
        let reglas = ReglasDecision {
            minimo: BTreeMap::from([("seguridad".to_string(), 0.8)]),
            ..ReglasDecision::default()
        };
        let i = informar_con(&v, &tarea(), &reglas, &[], &BTreeSet::new(), None).expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(
            i.motivo.contains("calibrada en 0,600 < mínimo 0,800"),
            "{}",
            i.motivo
        );
        assert!(
            i.resumen
                .contains("Seguridad calibrada queda en 0,600 y su mínimo es 0,800"),
            "{}",
            i.resumen
        );
    }

    #[test]
    fn informe_completitud_sin_veredicto_da_sin_decision_y_lo_anota() {
        // Desempate 9: sin veredicto de completitud, la decisión es `SinDecision`, aunque su rúbrica no
        // tenga ítem con `bloquea`.
        let v = veredicto(Vec::new(), None, Vec::new());
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::SinDecision, "{}", i.motivo);
        assert!(
            i.motivo.contains("completitud_sin_veredicto"),
            "{}",
            i.motivo
        );
        assert_eq!(estado_de(Some(&i)), Estado::SinVeredicto);
    }

    #[test]
    fn informe_seguridad_sin_veredicto_da_sin_decision_y_nombra_la_regla() {
        // Decisión 48: `seguridad.secretos` tiene `bloquea = true` en las rúbricas embebidas. Sin
        // veredicto de seguridad, la decisión no es ni `Listo` ni `NoListo`.
        let mut v = ecc();
        v.puntajes[1].valor = 0.0;
        v.puntajes[1].sin_veredicto = Some("juez no corrido".into());
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::SinDecision, "{}", i.motivo);
        assert!(
            i.motivo
                .contains("dimension_que_bloquea_sin_veredicto: seguridad"),
            "{}",
            i.motivo
        );
        assert_eq!(estado_de(Some(&i)), Estado::SinVeredicto);
        assert_eq!(crate::oracle::codigo_de_estado(estado_de(Some(&i))), 3);
        let md = markdown("memora", &i);
        assert!(md.contains("Sin decisión"), "{md}");
    }

    #[test]
    fn informe_confianza_lista_los_items_sin_medir() {
        // Un faltante de un puntaje sale en la confianza del informe y en el markdown, con su motivo.
        let mut v = ecc();
        let estructura = v
            .puntajes
            .iter_mut()
            .find(|p| p.dimension == Dimension::Estructura)
            .expect("hay estructura");
        estructura.faltantes = vec![ItemFaltante {
            item: "estructura.legibilidad".into(),
            motivo: "el juez no dio nivel".into(),
        }];
        let i = informe_de(&v);
        assert!(
            i.confianza
                .faltantes
                .iter()
                .any(|f| f.item == "estructura.legibilidad" && f.motivo == "el juez no dio nivel"),
            "{:?}",
            i.confianza.faltantes
        );
        let md = markdown("memora", &i);
        assert!(
            md.contains("- Sin medir estructura.legibilidad: el juez no dio nivel"),
            "{md}"
        );
    }

    #[test]
    fn informe_eficiencia_sin_veredicto_deja_listo_y_la_lista_en_confianza() {
        // Eficiencia no tiene ítems con `bloquea`: sin veredicto, la decisión sigue en `Listo`.
        let mut v = ecc();
        v.puntajes[2].valor = 0.0;
        v.puntajes[2].sin_veredicto = Some("sin sondas".into());
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::Listo, "{}", i.motivo);
        assert_eq!(i.confianza.sin_veredicto.len(), 1, "{:?}", i.confianza);
        assert_eq!(
            i.confianza.sin_veredicto[0].dimension,
            Dimension::Eficiencia
        );
        assert_eq!(i.confianza.sin_veredicto[0].motivo, "sin sondas");
    }

    #[test]
    fn informe_oculto_que_falla_fuera_de_requisitos_da_no_listo_y_lo_nombra() {
        // Desempate 11: un test oculto que falla y no prueba ningún requisito es una promesa del producto.
        // La regla no depende de `completitud_minima`: con el mínimo en cero, sigue en `NoListo`.
        let mut v = ecc();
        v.ocultos_fallidos = vec!["tests/extra.py::caso_suelto".into()];
        let reglas = ReglasDecision {
            completitud_minima: 0.0,
            ..ReglasDecision::default()
        };
        let i = informar(&v, &tarea(), &reglas, &BTreeSet::new(), None).expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(
            i.motivo.contains("test_oculto_fuera_de_requisitos"),
            "{}",
            i.motivo
        );
        assert!(
            i.motivo.contains("tests/extra.py::caso_suelto"),
            "{}",
            i.motivo
        );
    }

    #[test]
    fn informe_oculto_que_falla_dentro_de_un_requisito_no_dispara_la_regla() {
        // `tests/t.py::flaky` es la prueba de R1 en la tarea de prueba: el requisito lo cubre.
        let mut v = ecc();
        v.ocultos_fallidos = vec!["tests/t.py::flaky".into()];
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert!(
            !i.motivo.contains("test_oculto_fuera_de_requisitos"),
            "{}",
            i.motivo
        );
    }

    #[test]
    fn informe_test_del_contrato_que_falla_fuera_de_requisitos_da_no_listo() {
        // Decisión 50: un test del contrato que falla y que ningún requisito prueba con `contrato:` da `NoListo`.
        let mut v = ecc();
        v.contrato_fallidos = vec!["tests/contrato.py::suelto".into()];
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(
            i.motivo.contains("test_contrato_fuera_de_requisitos"),
            "{}",
            i.motivo
        );
        assert!(
            i.motivo.contains("tests/contrato.py::suelto"),
            "{}",
            i.motivo
        );
        // La regla no depende de `completitud_minima`.
        let reglas = ReglasDecision {
            completitud_minima: 0.0,
            ..ReglasDecision::default()
        };
        let i = informar(&v, &tarea(), &reglas, &BTreeSet::new(), None).expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(
            i.motivo.contains("test_contrato_fuera_de_requisitos"),
            "{}",
            i.motivo
        );
    }

    #[test]
    fn informe_no_compila_sin_bloqueos_da_no_listo_con_su_disparo() {
        let mut v = ecc();
        v.no_compila = true;
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        // La regla lleva su clave, como las demás de no listo.
        assert_eq!(
            i.motivo, "no_compila: el producto o sus tests no compilan",
            "{}",
            i.motivo
        );
    }

    #[test]
    fn informe_no_compila_con_secreto_da_bloqueado() {
        let mut v = ecc();
        v.no_compila = true;
        v.puntajes[1].items.push(NivelItem {
            item: "seguridad.secretos".into(),
            nivel: 0,
            evidencia: Vec::new(),
            fuente: Fuente::Sonda("rubrica".into()),
        });
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
    }

    #[test]
    fn informe_veredicto_viejo_sin_no_compila_lee_false_y_no_cambia_la_decision() {
        let base = ecc();
        let mut json = serde_json::to_value(&base).expect("serializa");
        json.as_object_mut()
            .expect("objeto")
            .remove("no_compila")
            .expect("el campo existe al serializar");
        let viejo: Veredicto = serde_json::from_value(json).expect("lee un veredicto viejo");
        assert!(!viejo.no_compila);
        let decision_base = informe_de(&base).decision;
        let decision_viejo = informe_de(&viejo).decision;
        assert_eq!(decision_viejo, decision_base);
    }

    #[test]
    fn informe_test_del_contrato_que_falla_dentro_de_un_requisito_no_dispara_la_regla() {
        // Decisión 50: una referencia `contrato:` cubre el test del contrato, por id exacto o por archivo.
        let mut t = tarea();
        t.requisito[0]
            .pruebas
            .push("contrato:tests/contrato.py::caso".into());
        let mut v = ecc();
        v.contrato_fallidos = vec!["tests/contrato.py::caso".into()];
        let i =
            informar(&v, &t, &ReglasDecision::default(), &BTreeSet::new(), None).expect("informa");
        assert!(
            !i.motivo.contains("test_contrato_fuera_de_requisitos"),
            "{}",
            i.motivo
        );

        let mut t = tarea();
        t.requisito[0]
            .pruebas
            .push("contrato:tests/contrato.py".into());
        let mut v = ecc();
        v.contrato_fallidos = vec!["tests/contrato.py::otro".into()];
        let i =
            informar(&v, &t, &ReglasDecision::default(), &BTreeSet::new(), None).expect("informa");
        assert!(
            !i.motivo.contains("test_contrato_fuera_de_requisitos"),
            "{}",
            i.motivo
        );
    }

    #[test]
    fn informe_id_sin_prefijo_no_cubre_un_test_del_contrato() {
        // Decisión 50: sin prefijo, el id apunta a la suite oculta. `tests/t.py::flaky` no cubre el contrato.
        let mut v = ecc();
        v.contrato_fallidos = vec!["tests/t.py::flaky".into()];
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(
            i.motivo.contains("test_contrato_fuera_de_requisitos"),
            "{}",
            i.motivo
        );
    }

    #[test]
    fn informe_id_del_contrato_no_cubre_un_test_oculto() {
        // Decisión 50, lado simétrico: `contrato:` apunta a la suite del contrato, no cubre un test oculto.
        let mut t = tarea();
        t.requisito[0].pruebas = vec!["contrato:tests/extra.py::caso_suelto".into()];
        let mut v = ecc();
        v.ocultos_fallidos = vec!["tests/extra.py::caso_suelto".into()];
        let i =
            informar(&v, &t, &ReglasDecision::default(), &BTreeSet::new(), None).expect("informa");
        assert!(
            i.motivo.contains("test_oculto_fuera_de_requisitos"),
            "{}",
            i.motivo
        );
    }

    #[test]
    fn informe_bloqueado_gana_sobre_sin_decision() {
        // Tarea 32: la evidencia primero. Un requisito de peso 3 en nivel 0 bloquea aunque la dimensión que
        // bloquea no tenga veredicto.
        let mut v = veredicto(
            vec![req("R1", Cumplimiento::Ausente, Some(0))],
            Some(0.9),
            Vec::new(),
        );
        v.puntajes[1].sin_veredicto = Some("juez no corrido".into());
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
        assert!(i.motivo.contains("bloqueo_requisito"), "{}", i.motivo);
        assert!(
            !i.motivo.contains("dimension_que_bloquea_sin_veredicto"),
            "{}",
            i.motivo
        );
    }

    #[test]
    fn informe_seguridad_sin_veredicto_con_hallazgo_de_severidad_3_bloquea() {
        // Tarea 32: un hallazgo de seguridad de severidad 3 es evidencia aunque Seguridad no tenga veredicto.
        let mut v = ecc();
        v.hallazgos
            .push(hallazgo("h-seguridad-3", Dimension::Seguridad, 3, &[]));
        let seguridad = v
            .puntajes
            .iter_mut()
            .find(|p| p.dimension == Dimension::Seguridad)
            .expect("seguridad");
        seguridad.valor = 0.0;
        seguridad.sin_veredicto = Some("juez no corrido".into());
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
        assert_eq!(i.bloqueantes, vec!["h-seguridad-3".to_string()]);
        assert!(i.motivo.contains("bloqueo_seguridad"), "{}", i.motivo);
    }

    #[test]
    fn informe_test_oculto_fuera_de_requisitos_da_no_listo_con_seguridad_sin_veredicto() {
        // Tarea 32: la evidencia de un test oculto que falla da `NoListo` aunque Seguridad no tenga veredicto.
        let mut v = ecc();
        v.ocultos_fallidos = vec!["tests/extra.py::caso_suelto".into()];
        let seguridad = v
            .puntajes
            .iter_mut()
            .find(|p| p.dimension == Dimension::Seguridad)
            .expect("seguridad");
        seguridad.valor = 0.0;
        seguridad.sin_veredicto = Some("juez no corrido".into());
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::NoListo, "{}", i.motivo);
        assert!(
            i.motivo.contains("test_oculto_fuera_de_requisitos"),
            "{}",
            i.motivo
        );
        assert!(
            !i.motivo.contains("dimension_que_bloquea_sin_veredicto"),
            "{}",
            i.motivo
        );
        assert_eq!(estado_de(Some(&i)), Estado::Rojo);
    }

    #[test]
    fn informe_completitud_igual_al_minimo_es_listo() {
        let v = veredicto(vec![cumplido("R1"), cumplido("R2")], Some(0.9), Vec::new());
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::Listo, "{}", i.motivo);
    }

    #[test]
    fn informe_regla_de_rubrica_bloquea_si_un_item_con_bloquea_queda_en_0() {
        let mut v = ecc();
        let seguridad = v
            .puntajes
            .iter_mut()
            .find(|p| p.dimension == Dimension::Seguridad)
            .expect("seguridad");
        seguridad.items.push(NivelItem {
            item: "seguridad.secretos".into(),
            nivel: 0,
            evidencia: Vec::new(),
            fuente: Fuente::Sonda("rubrica".into()),
        });
        let rubricas = [Rubrica {
            id: "seguridad".into(),
            version: 1,
            item: vec![ItemRubrica {
                id: "seguridad.secretos".into(),
                fuente: "OWASP ASVS 5.0".into(),
                mide: Mide::Juez,
                peso: 3,
                bloquea: true,
                anclas: vec!["0".into(), "1".into(), "2".into(), "3".into()],
            }],
        }];
        let i = informar_con(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &rubricas,
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
        assert!(i.motivo.contains("bloqueo_rubrica"), "{}", i.motivo);
        assert!(i.motivo.contains("seguridad.secretos"), "{}", i.motivo);
    }

    #[test]
    fn informe_rubricas_embebidas_bloquean_si_secretos_queda_en_0() {
        // Con las rúbricas embebidas, `seguridad.secretos` tiene `bloquea = true`: en nivel 0 bloquea.
        let mut v = ecc();
        v.puntajes[1].items.push(NivelItem {
            item: "seguridad.secretos".into(),
            nivel: 0,
            evidencia: Vec::new(),
            fuente: Fuente::Sonda("rubrica".into()),
        });
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::Bloqueado, "{}", i.motivo);
        assert!(i.motivo.contains("bloqueo_rubrica"), "{}", i.motivo);
    }

    #[test]
    fn informe_rubricas_embebidas_item_sin_bloquea_en_0_no_bloquea() {
        // `eficiencia.tiempo` existe en las embebidas, pero no tiene `bloquea`: en nivel 0 no bloquea.
        let mut v = ecc();
        v.puntajes[2].items.push(NivelItem {
            item: "eficiencia.tiempo".into(),
            nivel: 0,
            evidencia: Vec::new(),
            fuente: Fuente::Sonda("rubrica".into()),
        });
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            None,
        )
        .expect("informa");
        assert_eq!(i.decision, Decision::Listo, "{}", i.motivo);
    }

    #[test]
    fn informe_llm_que_cita_un_id_inexistente_pierde_esa_oracion() {
        let v = sdlc();
        let redactor = RedactorFalso {
            redaccion: Redaccion {
                resumen: "R9 queda en nivel 0. R3 falta el almacén.".into(),
                ..Redaccion::default()
            },
        };
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            Some(&redactor),
        )
        .expect("informa");
        assert_eq!(i.resumen, "R3 falta el almacén.");
    }

    #[test]
    fn informe_sin_redactor_el_resumen_es_determinista_y_cita_ids() {
        let v = sdlc();
        let i = informe_de(&v);
        assert!(!i.resumen.is_empty());
        let (_, descartadas) = oraciones_con_id(&i.resumen, &ids_de(&v));
        assert!(descartadas.is_empty(), "oraciones sin id: {descartadas:?}");
        assert_eq!(i, informe_de(&v), "el resumen no depende de nada más");
    }

    #[test]
    fn informe_redactor_que_falla_deja_los_textos_deterministas_y_avisa() {
        let v = sdlc();
        let sin = informe_de(&v);
        let con = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            Some(&RedactorCaido),
        )
        .expect("informa");
        assert_eq!(con.resumen, sin.resumen);
        assert_eq!(con.proximos_pasos, sin.proximos_pasos);
        assert!(
            con.confianza
                .avisos
                .iter()
                .any(|a| a.contains("caída del proveedor")),
            "{:?}",
            con.confianza.avisos
        );
    }

    #[test]
    fn informe_accion_que_pierde_todas_sus_oraciones_conserva_el_texto_determinista() {
        let v = sdlc();
        let sin = informe_de(&v);
        let redactor = RedactorFalso {
            redaccion: Redaccion {
                resumen: String::new(),
                riesgos: Vec::new(),
                proximos_pasos: vec!["Hacé lo que quieras.".into()],
                ..Redaccion::default()
            },
        };
        let con = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            Some(&redactor),
        )
        .expect("informa");
        assert_eq!(con.proximos_pasos, sin.proximos_pasos);
        assert_eq!(con.resumen, sin.resumen);
    }

    #[test]
    fn informe_redaccion_con_ids_reemplaza_el_texto_de_la_accion() {
        let v = sdlc();
        // Una entrada por acción visible (R3, R2 y R4), en el mismo orden.
        let redactor = RedactorFalso {
            redaccion: Redaccion {
                proximos_pasos: vec![
                    "Persistencia (R3) falta: armar el almacén.".into(),
                    "Comandos (R2) queda parcial: completar el módulo.".into(),
                    "Replicación (R4) falta: armar la réplica.".into(),
                ],
                ..Redaccion::default()
            },
        };
        let con = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            Some(&redactor),
        )
        .expect("informa");
        assert_eq!(
            con.proximos_pasos[0].texto,
            "Persistencia (R3) falta: armar el almacén."
        );
        assert_eq!(con.proximos_pasos[0].refs, vec!["R3".to_string()]);
    }

    /// Tres hallazgos de severidad 2: tres riesgos visibles. Cada id sale en el texto del redactor.
    fn tres_riesgos() -> Veredicto {
        veredicto(
            Vec::new(),
            Some(1.0),
            vec![
                hallazgo("h-1", Dimension::Estructura, 2, &[]),
                hallazgo("h-2", Dimension::Estructura, 2, &[]),
                hallazgo("h-3", Dimension::Estructura, 2, &[]),
            ],
        )
    }

    #[test]
    fn informe_redactor_con_menos_textos_que_visibles_conserva_los_originales_y_avisa() {
        let v = tres_riesgos();
        let sin = informe_de(&v);
        assert_eq!(sin.riesgos.len(), 3, "{:?}", sin.riesgos);
        // Dos textos para tres acciones: ninguno se aplica, ni siquiera el primero.
        let redactor = RedactorFalso {
            redaccion: Redaccion {
                riesgos: vec![
                    "El riesgo h-1 pide revisar el comando.".into(),
                    "El riesgo h-2 pide revisar el comando.".into(),
                ],
                ..Redaccion::default()
            },
        };
        let con = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            Some(&redactor),
        )
        .expect("informa");
        assert_eq!(con.riesgos, sin.riesgos);
        let aviso = "el redactor devolvió 2 textos para 3 acciones; se conservan los originales";
        assert!(
            con.confianza.avisos.iter().any(|a| a == aviso),
            "{:?}",
            con.confianza.avisos
        );
    }

    #[test]
    fn informe_redactor_con_menos_proximos_pasos_que_visibles_conserva_los_originales_y_avisa() {
        let v = sdlc();
        let sin = informe_de(&v);
        let visibles = sin.proximos_pasos.len();
        assert!(visibles > 1, "{:?}", sin.proximos_pasos);
        let redactor = RedactorFalso {
            redaccion: Redaccion {
                proximos_pasos: vec!["Persistencia (R3) falta: armar el almacén.".into()],
                ..Redaccion::default()
            },
        };
        let con = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            Some(&redactor),
        )
        .expect("informa");
        assert_eq!(con.proximos_pasos, sin.proximos_pasos);
        let aviso = format!(
            "el redactor devolvió 1 textos para {visibles} acciones; se conservan los originales"
        );
        assert!(
            con.confianza.avisos.iter().any(|a| a == &aviso),
            "{:?}",
            con.confianza.avisos
        );
    }

    #[test]
    fn informe_redactor_con_la_misma_cantidad_aplica_sus_textos_y_no_avisa() {
        let v = tres_riesgos();
        let sin = informe_de(&v);
        let textos = vec![
            "El riesgo h-1 pide revisar el comando.".to_string(),
            "El riesgo h-2 pide revisar el comando.".to_string(),
            "El riesgo h-3 pide revisar el comando.".to_string(),
        ];
        let redactor = RedactorFalso {
            redaccion: Redaccion {
                riesgos: textos.clone(),
                ..Redaccion::default()
            },
        };
        let con = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            Some(&redactor),
        )
        .expect("informa");
        // Mismo orden que el informe sin redactor: cada texto cae en su acción.
        assert_eq!(con.riesgos.len(), sin.riesgos.len());
        for (accion, original) in con.riesgos.iter().zip(&sin.riesgos) {
            let id = original.refs[0].as_str();
            let esperado = textos
                .iter()
                .find(|t| t.contains(id))
                .expect("texto del id");
            assert_eq!(&accion.texto, esperado);
        }
        assert!(
            !con.confianza
                .avisos
                .iter()
                .any(|a| a.contains("el redactor devolvió")),
            "{:?}",
            con.confianza.avisos
        );
    }

    #[test]
    fn informe_riesgos_y_proximos_pasos_tienen_como_maximo_5() {
        let requisitos = ["R2", "R3", "R4", "R5", "R6", "R7"]
            .iter()
            .map(|id| req(id, Cumplimiento::Ausente, Some(0)))
            .collect();
        let hallazgos = (1..=7)
            .map(|n| hallazgo(&format!("h-{n}"), Dimension::Estructura, 2, &[]))
            .collect();
        let v = veredicto(requisitos, Some(0.2), hallazgos);
        let i = informe_de(&v);
        assert_eq!(i.riesgos.len(), MAX_ITEMS);
        assert_eq!(i.proximos_pasos.len(), MAX_ITEMS);
        assert!(
            i.proximos_pasos.iter().map(|a| a.prioridad).eq(1..=5),
            "prioridades 1 a 5"
        );
    }

    #[test]
    fn informe_proximos_pasos_van_primero_los_bloqueantes_y_luego_mayor_peso() {
        let v = veredicto(
            vec![
                req("R2", Cumplimiento::Parcial, Some(1)),
                req("R4", Cumplimiento::Ausente, Some(0)),
            ],
            Some(0.4),
            vec![hallazgo("h-seguridad-3", Dimension::Seguridad, 3, &[])],
        );
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::Bloqueado);
        let refs: Vec<&str> = i
            .proximos_pasos
            .iter()
            .map(|a| a.refs[0].as_str())
            .collect();
        assert_eq!(refs, vec!["h-seguridad-3", "R2", "R4"]);
    }

    #[test]
    fn informe_muestra_el_motivo_junto_a_la_dimension_sin_calibrar() {
        let mut v = ecc();
        v.sin_calibrar = vec![Dimension::Diseno];
        let diseno = v
            .puntajes
            .iter_mut()
            .find(|p| p.dimension == Dimension::Diseno)
            .expect("ecc tiene Diseño");
        diseno.motivo_sin_calibrar = Some("calibración de otro juez".into());
        let md = markdown("memora", &informe_de(&v));
        assert!(
            md.contains("- Sin calibrar: Diseño (calibración de otro juez)"),
            "{md}"
        );
    }

    #[test]
    fn informe_nombra_los_items_inestables_de_cada_dimension() {
        let mut v = ecc();
        let diseno = v
            .puntajes
            .iter_mut()
            .find(|p| p.dimension == Dimension::Diseno)
            .expect("ecc tiene Diseño");
        diseno.inestables = vec!["diseno.errores".into(), "diseno.modularidad".into()];
        let md = markdown("memora", &informe_de(&v));
        assert!(
            md.contains("- Ítems inestables Diseño: diseno.errores, diseno.modularidad"),
            "{md}"
        );
    }

    #[test]
    fn informe_listo_con_3_dimensiones_sin_calibrar_lo_dice_en_el_markdown() {
        let mut v = ecc();
        v.sin_calibrar = vec![
            Dimension::Funciona,
            Dimension::Seguridad,
            Dimension::Eficiencia,
        ];
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::Listo);
        let md = markdown("memora", &i);
        assert!(md.contains("Listo con 3 dimensiones sin calibrar"), "{md}");
    }

    #[test]
    fn informe_estado_del_veredicto_sale_de_la_decision() {
        assert_eq!(estado_de(Some(&informe_de(&ecc()))), Estado::Verde);
        assert_eq!(estado_de(Some(&informe_de(&sdlc()))), Estado::Rojo);
        let bloqueado = veredicto(
            vec![req("R1", Cumplimiento::Ausente, Some(0))],
            Some(0.9),
            Vec::new(),
        );
        assert_eq!(estado_de(Some(&informe_de(&bloqueado))), Estado::Rojo);
        assert_eq!(estado_de(None), Estado::SinVeredicto);
    }

    #[test]
    fn informe_confianza_describe_un_inestable_con_pasa_x_de_n_una_sola_vez() {
        let mut v = ecc();
        v.tasas.insert(
            "oculto:tests/t.py::otro".into(),
            TasaTest {
                pasa: 4,
                total: 7,
                wilson_inf: 0.2,
            },
        );
        v.corridas_acumuladas
            .insert("oculto:tests/t.py::otro".into(), 7);
        let i = informe_de(&v);
        // El campo sigue en el modelo: solo cambia el texto del markdown.
        assert_eq!(
            i.confianza.corridas_acumuladas["oculto:tests/t.py::otro"],
            7
        );
        let md = markdown("memora", &i);
        assert_eq!(md.matches("pasa 4 de 7").count(), 1, "{md}");
        let linea = md
            .lines()
            .find(|l| l.starts_with("- Inestable oculto:tests/t.py::otro"))
            .unwrap_or_else(|| panic!("falta la línea del inestable: {md}"));
        assert!(!linea.contains("corridas acumuladas"), "{linea}");
        assert_eq!(linea.matches('7').count(), 1, "{linea}");
    }

    #[test]
    fn informe_cota_de_wilson_sigue_con_dos_decimales() {
        // Guarda: `numero` no cambia para la cota de Wilson, que sigue con dos decimales.
        let mut v = ecc();
        v.tasas.insert(
            "oculto:tests/t.py::otro".into(),
            TasaTest {
                pasa: 4,
                total: 7,
                wilson_inf: 0.2,
            },
        );
        let md = markdown("memora", &informe_de(&v));
        assert!(md.contains("(cota de Wilson 0,20)"), "{md}");
    }

    #[test]
    fn informe_confianza_lista_repeticiones_inestables_pendientes_y_sin_calibrar() {
        let mut v = ecc();
        v.repeticiones = 5;
        v.tasas.insert(
            "oculto:tests/t.py::otro".into(),
            TasaTest {
                pasa: 3,
                total: 5,
                wilson_inf: 0.2,
            },
        );
        v.requisitos[0] = req("R1", Cumplimiento::Pendiente, None);
        v.pendientes = vec![crate::tipos::Pendiente {
            test: "tests/x.py::lavapipe".into(),
            motivo: "lavapipe no emula la GPU".into(),
            colab: None,
            tasa: None,
        }];
        v.sin_calibrar = vec![Dimension::Eficiencia];
        v.puntajes[5].sin_veredicto = Some("juez no corrido".into());
        v.avisos = vec!["corrida de Colab con red: el oráculo pudo filtrarse".into()];
        let c = informe_de(&v).confianza;
        assert_eq!(c.repeticiones, 5);
        assert_eq!(c.inestables["oculto:tests/t.py::otro"].pasa, 3);
        assert_eq!(c.requisitos_pendientes, vec!["R1".to_string()]);
        assert_eq!(c.tests_pendientes[0].test, "tests/x.py::lavapipe");
        assert_eq!(c.sin_calibrar, vec![Dimension::Eficiencia]);
        assert_eq!(c.sin_veredicto.len(), 1);
        assert_eq!(c.sin_veredicto[0].dimension, Dimension::Diseno);
        assert_eq!(c.sin_veredicto[0].motivo, "juez no corrido");
        assert!(
            c.ambiguedades.is_empty(),
            "sin revisión guardada no hay ambigüedades"
        );
        assert!(c.avisos.iter().any(|a| a.contains("pudo filtrarse")));
    }

    #[test]
    fn informe_viejo_sin_confianza_se_lee_con_el_valor_por_defecto() {
        let mut json = serde_json::to_value(informe_de(&ecc())).expect("serializa");
        json.as_object_mut().expect("objeto").remove("confianza");
        let i: Informe = serde_json::from_value(json).expect("lee");
        assert_eq!(i.confianza, Confianza::default());
    }

    /// Aviso previo de cargo-audit sobre `time 0.1.43`, con el id `RUSTSEC-2024-000<n>`.
    fn aviso_previo_de(n: u32) -> Hallazgo {
        Hallazgo {
            archivo: "Cargo.lock".into(),
            cita: "time 0.1.43".into(),
            detalle: format!("RUSTSEC-2024-000{n}: time vulnerable"),
            ..seguridad_de(&format!("h-audit-{n}"), "cargo-audit", true)
        }
    }

    #[test]
    fn informe_avisos_previos_llena_la_lista_el_markdown_y_el_resumen() {
        let mut v = ecc();
        v.hallazgos.push(aviso_previo_de(1));
        let i = informe_de(&v);
        assert_eq!(i.decision, Decision::Listo, "{}", i.motivo);
        assert_eq!(
            i.avisos_previos,
            vec![AvisoPrevio {
                fuente: "cargo-audit".into(),
                aviso: "RUSTSEC-2024-0001".into(),
                objeto: "time 0.1.43".into(),
                severidad: 3,
            }]
        );
        let md = markdown("memora", &i);
        assert!(md.contains("## Avisos previos"), "{md}");
        assert!(md.contains("RUSTSEC-2024-0001"), "{md}");
        assert!(
            md.contains("Estaban en el estado inicial; no bloquean."),
            "{md}"
        );
        assert!(
            i.resumen
                .ends_with("1 aviso previo de Seguridad, no bloquea."),
            "{}",
            i.resumen
        );
    }

    #[test]
    fn informe_resumen_con_siete_avisos_previos_va_en_plural() {
        let mut v = ecc();
        v.hallazgos.extend((1..=7u32).map(aviso_previo_de));
        let i = informe_de(&v);
        assert!(
            i.resumen
                .ends_with("7 avisos previos de Seguridad, no bloquean."),
            "{}",
            i.resumen
        );
    }

    #[test]
    fn informe_sin_previos_no_tiene_seccion_ni_linea() {
        let i = informe_de(&ecc());
        assert!(i.avisos_previos.is_empty());
        let md = markdown("memora", &i);
        assert!(!md.contains("Avisos previos"), "{md}");
        assert!(!i.resumen.contains("previo"), "{}", i.resumen);
    }

    #[test]
    fn informe_secreto_previo_no_aparece_en_avisos_previos() {
        let mut v = ecc();
        v.hallazgos
            .push(seguridad_de("h-secreto", "gitleaks", true));
        let i = informe_de(&v);
        assert!(i.avisos_previos.is_empty(), "{:?}", i.avisos_previos);
        let md = markdown("memora", &i);
        assert!(!md.contains("Avisos previos"), "{md}");
    }

    #[test]
    fn informe_viejo_sin_avisos_previos_lee_vacio() {
        let mut json = serde_json::to_value(informe_de(&ecc())).expect("serializa");
        json.as_object_mut()
            .expect("objeto")
            .remove("avisos_previos")
            .expect("el campo existe al serializar");
        let i: Informe = serde_json::from_value(json).expect("lee");
        assert!(i.avisos_previos.is_empty());
    }

    /// Informe de `ecc()` con la tarea `t` y sin redactor.
    fn informe_con_tarea(v: &Veredicto, t: &Tarea) -> Informe {
        informar_con(
            v,
            t,
            &ReglasDecision::default(),
            &[],
            &BTreeSet::new(),
            None,
        )
        .expect("informa")
    }

    #[test]
    fn informe_confianza_marca_requisito_con_evidencia_solo_del_contrato() {
        let mut t = tarea();
        t.requisito[0].pruebas = vec!["contrato:tests/c.py::a".into()];
        t.requisito[0].bordes = Vec::new();
        // Desempate 6: el test del contrato tiene que haber dado un resultado medido.
        let v = ecc_con_evidencia_de_r1(&["contrato:tests/c.py::a: pasa"]);
        let i = informe_con_tarea(&v, &t);
        assert_eq!(
            i.confianza.evidencia_visible,
            ["R1: evidencia visible al builder"]
        );
    }

    #[test]
    fn informe_confianza_no_marca_requisito_con_evidencia_mezclada() {
        let mut t = tarea();
        // R1 mezcla un test del contrato con uno oculto que se midió: tiene evidencia oculta.
        t.requisito[0].pruebas = vec!["contrato:tests/c.py::a".into(), "tests/t.py::flaky".into()];
        let v =
            ecc_con_evidencia_de_r1(&["contrato:tests/c.py::a: pasa", "tests/t.py::flaky: falla"]);
        // R2 solo tiene referencias sin prefijo: no es del contrato.
        t.requisito[1].pruebas = vec!["tests/t.py::otro".into()];
        let i = informe_con_tarea(&v, &t);
        assert!(
            i.confianza.evidencia_visible.is_empty(),
            "{:?}",
            i.confianza
        );
        // Sin ninguna referencia, el default de `tarea()`, tampoco se marca.
        let i = informe_de(&ecc());
        assert!(
            i.confianza.evidencia_visible.is_empty(),
            "{:?}",
            i.confianza
        );
    }

    #[test]
    fn informe_confianza_marca_requisito_con_borde_del_contrato_y_pruebas_del_contrato() {
        let mut t = tarea();
        t.requisito[0].pruebas = vec!["contrato:tests/c.py::a".into()];
        t.requisito[0].bordes = vec!["contrato:tests/c.py::b".into()];
        // Desempate 6: los dos tests del contrato tienen que haber dado un resultado medido.
        let v = ecc_con_evidencia_de_r1(&[
            "contrato:tests/c.py::a: pasa",
            "contrato:tests/c.py::b: pasa",
        ]);
        let i = informe_con_tarea(&v, &t);
        assert_eq!(
            i.confianza.evidencia_visible,
            ["R1: evidencia visible al builder"]
        );
        // Un borde sin prefijo que se midió: el requisito tiene evidencia oculta.
        t.requisito[0].bordes = vec!["tests/t.py::b".into()];
        let v = ecc_con_evidencia_de_r1(&["contrato:tests/c.py::a: pasa", "tests/t.py::b: falla"]);
        let i = informe_con_tarea(&v, &t);
        assert!(
            i.confianza.evidencia_visible.is_empty(),
            "{:?}",
            i.confianza
        );
    }

    /// `ecc()` con la evidencia de R1 cambiada por `evidencia`.
    fn ecc_con_evidencia_de_r1(evidencia: &[&str]) -> Veredicto {
        let mut v = ecc();
        v.requisitos[0].evidencia = evidencia.iter().map(|e| e.to_string()).collect();
        v
    }

    #[test]
    fn informe_confianza_marca_requisito_con_contrato_verde_y_oculto_ausente() {
        // Desempate 5: un oculto sin resultado medido no quita la evidencia visible del contrato.
        let mut t = tarea();
        t.requisito[0].pruebas = vec!["contrato:tests/c.py::a".into(), "tests/t.py::b".into()];
        t.requisito[0].bordes = Vec::new();
        let v =
            ecc_con_evidencia_de_r1(&["contrato:tests/c.py::a: pasa", "tests/t.py::b: ausente"]);
        let i = informe_con_tarea(&v, &t);
        assert_eq!(
            i.confianza.evidencia_visible,
            ["R1: evidencia visible al builder"]
        );
        // Un pendiente de entorno tampoco es un resultado medido.
        let v = ecc_con_evidencia_de_r1(&[
            "contrato:tests/c.py::a: pasa",
            "pendiente: tests/t.py::b (gpu)",
        ]);
        let i = informe_con_tarea(&v, &t);
        assert_eq!(
            i.confianza.evidencia_visible,
            ["R1: evidencia visible al builder"]
        );
    }

    #[test]
    fn informe_confianza_no_marca_requisito_con_su_unico_contrato_ausente() {
        // Desempate 6: un único test del contrato sin resultado medido no entra en la evidencia visible.
        let mut t = tarea();
        t.requisito[0].pruebas = vec!["contrato:tests/c.py::a".into()];
        t.requisito[0].bordes = Vec::new();
        for linea in [
            "contrato:tests/c.py::a: ausente",
            "contrato:tests/c.py::a: omite",
            "excluido: contrato:tests/c.py::a (motivo)",
        ] {
            let v = ecc_con_evidencia_de_r1(&[linea]);
            let i = informe_con_tarea(&v, &t);
            assert!(
                i.confianza.evidencia_visible.is_empty(),
                "{linea}: {:?}",
                i.confianza
            );
        }
    }

    #[test]
    fn informe_confianza_no_marca_requisito_con_su_unico_contrato_pendiente() {
        // Desempate 6: un pendiente de entorno del contrato no es un resultado medido.
        let mut t = tarea();
        t.requisito[0].pruebas = vec!["contrato:tests/c.py::a".into()];
        t.requisito[0].bordes = Vec::new();
        let v = ecc_con_evidencia_de_r1(&["pendiente: contrato:tests/c.py::a (gpu)"]);
        let i = informe_con_tarea(&v, &t);
        assert!(
            i.confianza.evidencia_visible.is_empty(),
            "{:?}",
            i.confianza
        );
    }

    #[test]
    fn informe_confianza_no_marca_requisito_con_oculto_medido() {
        let mut t = tarea();
        t.requisito[0].pruebas = vec!["contrato:tests/c.py::a".into(), "tests/t.py::b".into()];
        t.requisito[0].bordes = Vec::new();
        for medido in ["falla", "inestable", "pasa"] {
            let oculto = format!("tests/t.py::b: {medido}");
            let v = ecc_con_evidencia_de_r1(&["contrato:tests/c.py::a: pasa", oculto.as_str()]);
            let i = informe_con_tarea(&v, &t);
            assert!(
                i.confianza.evidencia_visible.is_empty(),
                "{medido}: {:?}",
                i.confianza
            );
        }
    }

    #[test]
    fn informe_confianza_no_marca_requisito_solo_con_referencias_sin_prefijo() {
        let mut t = tarea();
        t.requisito[0].pruebas = vec!["tests/t.py::b".into()];
        t.requisito[0].bordes = Vec::new();
        let i = informe_con_tarea(&ecc_con_evidencia_de_r1(&[]), &t);
        assert!(
            i.confianza.evidencia_visible.is_empty(),
            "{:?}",
            i.confianza
        );
    }

    #[test]
    fn informe_ids_ocultos_no_incluye_pendientes_del_contrato() {
        // Desempate 1: un pendiente `contrato:` es del contrato, no un test oculto.
        let mut v = ecc();
        v.pendientes = vec![Pendiente {
            test: "contrato:tests/c.py::gpu".into(),
            motivo: "gpu".into(),
            colab: None,
            tasa: None,
        }];
        assert!(!ids_ocultos(&v).contains("contrato:tests/c.py::gpu"));
        v.pendientes.push(Pendiente {
            test: "tests/t.py::gpu".into(),
            motivo: "gpu".into(),
            colab: None,
            tasa: None,
        });
        assert!(ids_ocultos(&v).contains("tests/t.py::gpu"));
    }

    #[test]
    fn informe_markdown_lista_la_evidencia_visible_al_builder() {
        let mut t = tarea();
        t.requisito[0].pruebas = vec!["contrato:tests/c.py::a".into()];
        t.requisito[0].bordes = Vec::new();
        let v = ecc_con_evidencia_de_r1(&["contrato:tests/c.py::a: pasa"]);
        let md = markdown("memora", &informe_con_tarea(&v, &t));
        assert!(md.contains("- R1: evidencia visible al builder"), "{md}");
    }

    #[test]
    fn informe_viejo_sin_evidencia_visible_lee_vacio() {
        let mut json = serde_json::to_value(informe_de(&ecc())).expect("serializa");
        let confianza = json["confianza"].as_object_mut().expect("objeto");
        assert!(confianza.remove("evidencia_visible").is_some());
        let i: Informe = serde_json::from_value(json).expect("lee");
        assert!(i.confianza.evidencia_visible.is_empty());
    }

    #[test]
    fn informe_ocultar_tests_conserva_la_evidencia_visible() {
        let mut i = informe_de(&ecc());
        i.confianza.evidencia_visible = vec!["R1: evidencia visible al builder".into()];
        let ocultos = BTreeSet::from(["tests/t.py::flaky".to_string()]);
        ocultar_tests(&mut i, &ocultos);
        assert_eq!(
            i.confianza.evidencia_visible,
            ["R1: evidencia visible al builder"]
        );
    }

    #[test]
    fn informe_redactor_no_recibe_nombres_de_tests_ocultos() {
        let mut v = sdlc();
        v.tasas.insert(
            "oculto:tests/t.py::flaky".into(),
            TasaTest {
                pasa: 3,
                total: 5,
                wilson_inf: 0.2,
            },
        );
        v.inestables = vec!["tests/t.py::flaky".into()];
        let espia = RedactorEspia {
            visto: RefCell::new(None),
        };
        let con = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            Some(&espia),
        )
        .expect("informa");
        let visto = espia
            .visto
            .borrow()
            .clone()
            .expect("el redactor vio el informe");
        let texto = serde_json::to_string(&visto).expect("serializa");
        assert!(
            !texto.contains("flaky"),
            "el redactor vio un test oculto: {texto}"
        );
        // El informe que devuelve `informar` sí lo conserva para la CLI.
        assert!(
            serde_json::to_string(&con.confianza)
                .expect("serializa")
                .contains("flaky")
        );
    }

    #[test]
    fn informe_texto_del_redactor_cae_en_el_riesgo_visible_con_un_oculto_primero() {
        // El primer riesgo es oculto. El redactor solo ve el visible, así que su texto va a ese.
        let mut v = ecc();
        v.tasas.insert(
            "oculto:tests/t.py::flaky".into(),
            TasaTest {
                pasa: 3,
                total: 5,
                wilson_inf: 0.2,
            },
        );
        v.inestables = vec!["tests/t.py::flaky".into()];
        let texto = "El hallazgo h-estructura-1 pide revisar el comando.";
        let redactor = RedactorFalso {
            redaccion: Redaccion {
                riesgos: vec![texto.into()],
                ..Redaccion::default()
            },
        };
        let i = informar(
            &v,
            &tarea(),
            &ReglasDecision::default(),
            &BTreeSet::new(),
            Some(&redactor),
        )
        .expect("informa");
        let visible = i
            .riesgos
            .iter()
            .find(|a| a.refs.contains(&"h-estructura-1".to_string()))
            .unwrap_or_else(|| panic!("sin riesgo visible: {:?}", i.riesgos));
        assert_eq!(visible.texto, texto, "{:?}", i.riesgos);
        let oculto = i
            .riesgos
            .iter()
            .find(|a| a.texto.contains("es inestable"))
            .unwrap_or_else(|| panic!("sin riesgo del test oculto: {:?}", i.riesgos));
        assert!(oculto.texto.contains("tests/t.py::flaky"), "{oculto:?}");
    }

    #[test]
    fn informe_ocultar_tests_quita_acciones_y_confianza_con_ids_ocultos() {
        let mut i = informe_de(&sdlc());
        i.proximos_pasos.push(Accion {
            prioridad: 9,
            texto: "R1 Protocolo: el test tests/t.py::flaky es inestable.".into(),
            refs: vec!["R1".into()],
        });
        i.confianza
            .inestables
            .insert("oculto:tests/t.py::flaky".into(), TasaTest::default());
        let ocultos = BTreeSet::from(["tests/t.py::flaky".to_string()]);
        ocultar_tests(&mut i, &ocultos);
        assert!(
            i.proximos_pasos.iter().all(|a| !a.texto.contains("flaky")),
            "{:?}",
            i.proximos_pasos
        );
        assert!(i.confianza.inestables.is_empty());
    }

    #[test]
    fn informe_riesgo_de_test_del_contrato_inestable_cita_su_requisito_con_prefijo() {
        // Decisión 50: un test del contrato inestable se nombra con `contrato:` y lo cita su requisito.
        let mut t = tarea();
        t.requisito[0].bordes = vec!["contrato:tests/c.py::flaky".into()];
        let mut v = ecc();
        v.tasas.insert(
            "contrato:tests/c.py::flaky".into(),
            TasaTest {
                pasa: 2,
                total: 5,
                wilson_inf: 0.1,
            },
        );
        let i =
            informar(&v, &t, &ReglasDecision::default(), &BTreeSet::new(), None).expect("informa");
        let accion = i
            .riesgos
            .iter()
            .find(|a| a.texto.contains("contrato:tests/c.py::flaky"))
            .unwrap_or_else(|| panic!("sin riesgo del test: {:?}", i.riesgos));
        assert!(accion.refs.contains(&"R1".to_string()), "{accion:?}");

        // Un hallazgo que cita el test con su prefijo evita la acción de tasa.
        let mut h = hallazgo("h-flaky", Dimension::Pruebas, 1, &[]);
        h.cita = "contrato:tests/c.py::flaky".into();
        v.hallazgos.push(h);
        let i =
            informar(&v, &t, &ReglasDecision::default(), &BTreeSet::new(), None).expect("informa");
        assert!(
            !i.riesgos.iter().any(|a| a.texto.contains("es inestable")),
            "{:?}",
            i.riesgos
        );
    }

    #[test]
    fn informe_ids_ocultos_no_incluye_ids_del_contrato() {
        // Los ids del contrato no cuentan como ocultos: la redacción no los quita.
        let mut v = ecc();
        v.contrato_fallidos = vec!["tests/contrato.py::caso".into()];
        v.inestables_contrato = vec!["tests/contrato.py::flaky".into()];
        assert!(!ids_ocultos(&v).contains("tests/contrato.py::caso"));
        assert!(!ids_ocultos(&v).contains("tests/contrato.py::flaky"));
    }

    #[test]
    fn informe_salida_del_redactor_se_lee_como_json() {
        // El parseo del redactor de producción no llama a la red: solo lee lo que devolvió el modelo.
        let json =
            r#"{"resumen":"R3 falta.","riesgos":[],"proximos_pasos":["R3 falta el almacén."]}"#;
        let salida: Salida = serde_json::from_str(json).expect("lee");
        assert_eq!(
            salida.proximos_pasos,
            vec!["R3 falta el almacén.".to_string()]
        );
        assert!(serde_json::from_str::<Salida>("no es json").is_err());
    }
}
