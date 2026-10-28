//! Juez de ambigüedades: un modelo busca frases del spec con dos lecturas posibles.
//! Refuta por default. Su JSON es estricto, y el gate de evidencia descarta lo que el spec no cita.

use serde::Deserialize;
use serde_json::Value;

use super::{Ambiguedad, ContextoRevision, ErrorFicha, Revision, revisar};
use crate::oracle::evidencia::cita_en_archivo;
use crate::proveedores::{self, Prompt};
use crate::tipos::Eleccion;

/// Tokens de salida por llamada del juez.
const MAX_TOKENS: u32 = 4000;
/// Fase del flujo que va al `Evento` de la llamada.
const FASE: &str = "ficha";
const JUEZ_OK: &str = "ok";
const SIN_VEREDICTO: &str = "sin_veredicto";

/// Prompt fijo del juez. Pide un arreglo JSON y refuta por default.
const PROMPT_SISTEMA: &str = "Sos un revisor de specs de software. Tu trabajo es encontrar frases ambiguas del SPEC.
REFUTA POR DEFAULT: reporta una frase solo si es realmente ambigua. Si el spec es claro, responde un arreglo vacío.
Una frase es ambigua solo si admite DOS lecturas distintas. Cada lectura debe llevar a una implementación que un test podría distinguir.
Ejemplo: \"en el espacio local\" puede significar los ejes locales del nodo o los ejes del padre.
No reportes estilo, redacción, ni falta de detalle que no cambia el comportamiento.
Reglas de la respuesta:
- \"frase\" es una cita textual exacta del SPEC, con los mismos caracteres.
- \"lectura_a\" y \"lectura_b\" son dos lecturas distintas, una oración cada una.
- Responde SOLO con un arreglo JSON, sin texto antes ni después.
Formato: [{\"frase\": \"<cita textual exacta>\", \"lectura_a\": \"...\", \"lectura_b\": \"...\"}]
Sin ambigüedades: responde []";

/// Un modelo que responde al prompt del juez.
pub trait JuezAmbiguedad {
    fn consultar(&self, prompt: &Prompt) -> Result<String, ErrorFicha>;
}

/// Juez real: llama al proveedor de `eleccion`. Un error del proveedor no expone claves.
pub struct JuezProveedor {
    pub eleccion: Eleccion,
    pub corrida: String,
}

impl JuezAmbiguedad for JuezProveedor {
    fn consultar(&self, prompt: &Prompt) -> Result<String, ErrorFicha> {
        proveedores::llamar(&self.eleccion, prompt, &self.corrida)
            .map(|respuesta| respuesta.texto)
            .map_err(|e| ErrorFicha::Juez(e.to_string()))
    }
}

/// Corre las reglas deterministas y luego el juez. Un fallo del juez deja `sin_veredicto`.
pub fn revisar_con_juez(
    spec: &str,
    ctx: &ContextoRevision,
    juez: &dyn JuezAmbiguedad,
) -> Result<Revision, ErrorFicha> {
    let mut revision = revisar(spec, ctx)?;
    let prompt = Prompt {
        sistema: Some(PROMPT_SISTEMA.into()),
        usuario: spec.into(),
        max_tokens: MAX_TOKENS,
        fase: FASE.into(),
    };
    match juicio(juez, &prompt) {
        Ok(crudas) => {
            let (ambiguedades, descartadas) = aplicar_gate(spec, crudas);
            revision.ambiguedades = ambiguedades;
            revision.descartadas = descartadas;
            revision.juez = JUEZ_OK.into();
        }
        Err(causa) => revision.juez = format!("{SIN_VEREDICTO}: {causa}"),
    }
    Ok(revision)
}

/// Pide el juicio y lo parsea. El error es la causa en texto, lista para `juez`.
fn juicio(juez: &dyn JuezAmbiguedad, prompt: &Prompt) -> Result<Vec<Cruda>, String> {
    let texto = juez.consultar(prompt).map_err(|e| e.to_string())?;
    parsear(&texto)
}

/// Una ambigüedad tal como la devuelve el modelo, antes del gate.
#[derive(Debug, Deserialize)]
struct Cruda {
    frase: String,
    lectura_a: String,
    lectura_b: String,
}

/// Parseo estricto: un arreglo JSON, con o sin un bloque ```json que lo envuelva.
fn parsear(texto: &str) -> Result<Vec<Cruda>, String> {
    let valor: Value =
        serde_json::from_str(desenvuelto(texto)).map_err(|e| format!("JSON inválido: {e}"))?;
    let Value::Array(items) = valor else {
        return Err("se esperaba un arreglo JSON".into());
    };
    items
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            serde_json::from_value::<Cruda>(item).map_err(|e| format!("elemento {i}: {e}"))
        })
        .collect()
}

/// Quita un bloque ```json ... ``` que envuelve todo el texto. Cualquier otro texto queda igual.
fn desenvuelto(texto: &str) -> &str {
    let t = texto.trim();
    let Some(resto) = t.strip_prefix("```") else {
        return t;
    };
    let Some((info, cuerpo)) = resto.split_once('\n') else {
        return t;
    };
    if !matches!(info.trim(), "" | "json") {
        return t;
    }
    match cuerpo.trim_end().strip_suffix("```") {
        Some(interior) => interior.trim(),
        None => t,
    }
}

/// Separa las ambigüedades que pasan el gate de las frases descartadas.
fn aplicar_gate(spec: &str, crudas: Vec<Cruda>) -> (Vec<Ambiguedad>, Vec<String>) {
    let mut aceptadas = Vec::new();
    let mut descartadas = Vec::new();
    for cruda in crudas {
        match verificar(spec, &cruda) {
            Some(ambiguedad) => aceptadas.push(ambiguedad),
            None => descartadas.push(cruda.frase),
        }
    }
    (aceptadas, descartadas)
}

/// Acepta una ambigüedad solo si su frase está en el spec, con lecturas distintas y no vacías.
fn verificar(spec: &str, cruda: &Cruda) -> Option<Ambiguedad> {
    let frase = cruda.frase.trim();
    let a = cruda.lectura_a.trim();
    let b = cruda.lectura_b.trim();
    if frase.is_empty() || a.is_empty() || b.is_empty() || a == b {
        return None;
    }
    let linea = linea_de(spec, frase)?;
    if !cita_en_archivo(spec, u32::try_from(linea).ok()?, frase) {
        return None;
    }
    Some(Ambiguedad {
        frase: frase.into(),
        linea,
        lectura_a: a.into(),
        lectura_b: b.into(),
    })
}

/// Línea 1-based donde empieza `frase`, sin contar espacios. `None` si no aparece.
fn linea_de(spec: &str, frase: &str) -> Option<usize> {
    let aguja: Vec<char> = frase.chars().filter(|c| !c.is_whitespace()).collect();
    if aguja.is_empty() {
        return None;
    }
    let texto: Vec<(char, usize)> = spec
        .lines()
        .enumerate()
        .flat_map(|(i, linea)| {
            linea
                .chars()
                .filter(|c| !c.is_whitespace())
                .map(move |c| (c, i + 1))
        })
        .collect();
    texto
        .windows(aguja.len())
        .position(|ventana| ventana.iter().zip(&aguja).all(|((c, _), a)| c == a))
        .map(|p| texto[p].1)
}
