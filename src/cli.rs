use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

/// Root: evalúa el valor de un proyecto y rutea trabajo entre modelos.
#[derive(Debug, Parser)]
#[command(name = "r00t", version, about)]
pub struct Cli {
    /// Imprime un solo objeto JSON en stdout.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub comando: Comando,
}

#[derive(Debug, Subcommand)]
pub enum Comando {
    /// Elige el modelo para un rol y tipo de tarea.
    Route {
        #[arg(long, value_enum)]
        rol: RolArg,
        #[arg(long)]
        tipo: String,
        #[arg(long)]
        checkeable: bool,
        #[arg(long)]
        tokens: Option<u32>,
    },
    /// Llama a un modelo con un prompt.
    Llama {
        #[arg(long, value_enum)]
        rol: RolArg,
        #[arg(long)]
        tipo: String,
        #[arg(long)]
        prompt_file: PathBuf,
        #[arg(long)]
        corrida: String,
        /// Tope de tokens de salida. Entra al chequeo previo de costo.
        #[arg(long, default_value_t = 4096)]
        max_tokens: u32,
    },
    /// Evalúa o calibra el Oracle.
    Oraculo {
        #[command(subcommand)]
        accion: OraculoAccion,
    },
    /// Prepara paquetes para correr el Oracle en Colab.
    Colab {
        #[command(subcommand)]
        accion: ColabAccion,
    },
    /// Registra y resume lo que pasa en una corrida.
    Observa {
        #[command(subcommand)]
        accion: ObservaAccion,
    },
    /// Guarda y compara filas de resultados.
    Track {
        #[command(subcommand)]
        accion: TrackAccion,
    },
    /// Calcula la política de ruteo a partir del registro.
    Aprende {
        #[arg(long)]
        replay: bool,
    },
    /// Corre una tarea con una configuración, o revisa sus lecturas con `revisar-lectura`.
    #[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
    Corre {
        #[arg(long)]
        tarea: Option<String>,
        #[arg(long)]
        config: Option<String>,
        /// Retoma una corrida cortada: su id `corre-<tarea>-<config>-...`, con su carpeta en `$HOME`.
        #[arg(long)]
        reanuda: Option<String>,
        #[command(subcommand)]
        accion: Option<CorreAccion>,
    },
    /// Verifica o autoriza el GOAL.
    Goal {
        #[command(subcommand)]
        accion: GoalAccion,
    },
    /// Importa el log de métricas de mmorch en solo lectura.
    #[command(name = "importar-mmorch")]
    ImportarExterno {
        #[arg(long)]
        desde: PathBuf,
    },
    /// Imprime el JSON Schema de un tipo.
    Esquema {
        /// Nombre del tipo, por ejemplo `veredicto`.
        tipo: String,
    },
    /// Revisa la ficha de un SPEC.md o arma el borrador de `tarea.toml`.
    Ficha {
        #[command(subcommand)]
        accion: FichaAccion,
    },
    /// Inicia el servidor MCP por stdio.
    Mcp,
}

/// Roles que acepta `route` y `llama`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum RolArg {
    Planifica,
    Organiza,
    Construye,
    Verifica,
    Juzga,
    Delega,
}

impl From<RolArg> for crate::tipos::Rol {
    fn from(rol: RolArg) -> Self {
        match rol {
            RolArg::Planifica => crate::tipos::Rol::Planifica,
            RolArg::Organiza => crate::tipos::Rol::Organiza,
            RolArg::Construye => crate::tipos::Rol::Construye,
            RolArg::Verifica => crate::tipos::Rol::Verifica,
            RolArg::Juzga => crate::tipos::Rol::Juzga,
            RolArg::Delega => crate::tipos::Rol::Delega,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum FichaAccion {
    /// Revisa la ficha del spec con reglas deterministas. Con `--juez-*`, también el juez.
    Revisar {
        #[arg(long)]
        spec: PathBuf,
        /// Proveedor del juez de ambigüedades. Exige `--juez-modelo`.
        #[arg(long, value_enum, requires = "juez_modelo")]
        juez_proveedor: Option<ProveedorJuez>,
        /// Modelo del juez de ambigüedades. Exige `--juez-proveedor`.
        #[arg(long, requires = "juez_proveedor")]
        juez_modelo: Option<String>,
        /// Tope de tokens de salida del juez. Exige `--juez-modelo`. Por defecto, 32 000.
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..), requires = "juez_modelo")]
        juez_max_tokens: Option<u32>,
        /// Solo reglas deterministas: no corre el juez y la revisión no se guarda. Excluye `--juez-*`.
        #[arg(
            long,
            conflicts_with_all = ["juez_proveedor", "juez_modelo", "juez_max_tokens"]
        )]
        solo_deterministas: bool,
    },
    /// Arma el borrador de `tarea.toml` desde la ficha.
    Tarea {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        nombre: String,
    },
}

/// Proveedores que puede usar el juez de la ficha.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ProveedorJuez {
    Deepseek,
    Anthropic,
}

impl From<ProveedorJuez> for crate::tipos::Proveedor {
    fn from(proveedor: ProveedorJuez) -> Self {
        match proveedor {
            ProveedorJuez::Deepseek => crate::tipos::Proveedor::Deepseek,
            ProveedorJuez::Anthropic => crate::tipos::Proveedor::Anthropic,
        }
    }
}

/// Subcomandos de `corre`. Sin subcomando, `corre` corre una tarea.
#[derive(Debug, Subcommand)]
pub enum CorreAccion {
    /// Cuenta las lecturas del oráculo en un transcript. Solo imprime conteos; no escribe nada.
    #[command(name = "revisar-lectura")]
    RevisarLectura {
        #[arg(long)]
        tarea: String,
        /// Transcript de `claude -p --output-format stream-json`.
        #[arg(long)]
        transcript: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum ColabAccion {
    /// Arma el paquete de calibración para Colab: binario, herramientas, tareas y calibración.
    Empaquetar {
        /// Binario `r00t` para x86-64 que entra al paquete.
        #[arg(long)]
        binario: PathBuf,
        /// Trabajos `tarea:dimension`, separados por coma. Sin `--corre`, es obligatorio.
        #[arg(
            long,
            value_delimiter = ',',
            required_unless_present = "corre",
            conflicts_with = "corre"
        )]
        trabajos: Vec<String>,
        /// Repo bueno de una tarea, como `tarea=ruta`. Se repite por cada tarea. Sin `--corre`, es obligatorio.
        #[arg(
            long,
            num_args = 1,
            required_unless_present = "corre",
            conflicts_with = "corre"
        )]
        repo: Vec<String>,
        /// SHA-256 en hexadecimal (64 caracteres) del instalador de rustup. Siempre es obligatorio.
        #[arg(long, required = true)]
        rustup_sha256: Option<String>,
        /// Archivo `.tar.gz` de salida.
        #[arg(long)]
        salida: PathBuf,
        /// Arma el paquete de `corre` para Colab. Exige `--tareas`, `--config` y `--pasada`.
        #[arg(
            long,
            conflicts_with_all = ["trabajos", "repo", "n", "semilla", "juez", "tope_usd"],
            requires_all = ["tareas", "config", "pasada"]
        )]
        corre: bool,
        /// Tareas de `corre`, separadas por coma, en el orden del manifiesto. Exige `--corre`.
        #[arg(long, value_delimiter = ',', requires = "corre")]
        tareas: Vec<String>,
        /// Config de la corrida: `<ROOT_HOME>/configs/<config>.toml`. Exige `--corre`.
        #[arg(long, requires = "corre")]
        config: Option<String>,
        /// Pasada de la corrida. Exige `--corre`.
        #[arg(long, requires = "corre")]
        pasada: Option<u32>,
        /// Repo de Root: de ahí sale `tools/colab/`. Por defecto, el directorio actual.
        #[arg(long)]
        raiz_root: Option<PathBuf>,
        /// Repeticiones por defecto de cada trabajo.
        #[arg(long, default_value_t = crate::colab_calibra::N_POR_DEFECTO)]
        n: u32,
        /// Semilla del sorteo.
        #[arg(long, default_value_t = crate::colab_calibra::SEMILLA_POR_DEFECTO)]
        semilla: u64,
        /// Incluye las dimensiones con ítems del juez. Exige Colab Secrets. Fija el tope de gasto de S2.
        #[arg(long)]
        juez: bool,
        /// Tope de gasto de API del paquete, en USD. Exige `--juez`. Con él, el tope es el menor entre el saldo de S2 y este valor.
        #[arg(long)]
        tope_usd: Option<f64>,
    },
    /// Importa la vuelta de una calibración de Colab: revisa el paquete y agrega sus filas.
    Importar {
        /// Paquete `.tar.gz` de vuelta que trajo Colab. Sin `--corre`, es obligatorio.
        #[arg(required_unless_present = "corre", conflicts_with = "corre")]
        archivo: Option<PathBuf>,
        /// Repo de Root: de ahí sale el commit de la laptop. Por defecto, el directorio actual.
        #[arg(long)]
        raiz_root: Option<PathBuf>,
        /// SHA-256 en hexadecimal (64 caracteres) del paquete. Si no coincide, no se importa nada.
        #[arg(long)]
        sha: Option<String>,
        /// Paquete de ida. Con él, el archivo es la vuelta cruda del runtime. Exige `--sha`.
        #[arg(long, requires = "sha")]
        ida: Option<PathBuf>,
        /// Vuelta de una tarea de `corre` (`vuelta-<tarea>.tar.gz`). Exige `--sha` e `--ida`.
        #[arg(long, requires_all = ["sha", "ida"])]
        corre: Option<PathBuf>,
    },
    /// Calibra en Colab: corre cada paquete de `colab empaquetar` en su runtime de CPU e importa la vuelta.
    Calibrar {
        /// Paquete `.tar.gz` de `colab empaquetar`. Se repite: cada paquete va a su runtime.
        #[arg(long, num_args = 1, required = true)]
        paquete: Vec<PathBuf>,
        /// Repo de Root: de ahí sale el commit de la laptop. Por defecto, el directorio actual.
        #[arg(long)]
        raiz_root: Option<PathBuf>,
        /// No libera el runtime al terminar. Queda abierto y gasta unidades, cerca de 0,08 por hora.
        /// Cierra el runtime con `colab stop` u `oraculo pendientes --liberar`.
        #[arg(long)]
        sin_stop: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum OraculoAccion {
    /// Evalúa un repositorio con una tarea.
    Evaluar {
        #[arg(long)]
        tarea: String,
        #[arg(long)]
        repo: PathBuf,
    },
    /// Arma el informe del PM: la decisión, su regla, lo que falta y qué arreglar primero.
    Informe {
        #[arg(long)]
        tarea: String,
        #[arg(long)]
        repo: PathBuf,
        /// Imprime markdown legible en vez de JSON.
        #[arg(long)]
        md: bool,
    },
    /// Arma el paquete de los tests pendientes por entorno para correrlos en Colab.
    /// Con `--importar`, guarda el junit que trajo de vuelta.
    Pendientes {
        /// Tarea. Obligatoria salvo con `--liberar` o `--vigilar`.
        #[arg(long, required_unless_present_any = ["liberar", "vigilar"])]
        tarea: Option<String>,
        /// Repo. Obligatorio salvo con `--liberar` o `--vigilar`.
        #[arg(long, required_unless_present_any = ["liberar", "vigilar"])]
        repo: Option<PathBuf>,
        /// Junit que dejó `correr.sh`: uno por repetición (`pendientes-N.xml`). Sin este flag se arma el paquete.
        #[arg(long, num_args = 1.., conflicts_with_all = ["colab", "vigilar"])]
        importar: Vec<PathBuf>,
        /// Corre el paquete en un runtime T4 de Colab, importa el junit y libera el runtime.
        #[arg(long, conflicts_with_all = ["importar", "vigilar"])]
        colab: bool,
        /// Libera el runtime de Colab anotado en `activo.json`, si hay uno, y sale.
        #[arg(long, conflicts_with_all = ["colab", "importar", "vigilar"])]
        liberar: bool,
        /// Revisa el runtime anotado en `activo.json`: lo libera si cumplió lo que pedía o si pasó
        /// su tope. Un runtime vigente no se toca. Es la red de seguridad de `--colab`.
        #[arg(long, conflicts_with_all = ["colab", "importar", "liberar"])]
        vigilar: bool,
    },
    /// Calibra una dimensión del Oracle.
    Calibrar {
        #[arg(long)]
        tarea: String,
        #[arg(long)]
        dimension: String,
        #[arg(long, default_value_t = 20)]
        n: u32,
        #[arg(long, default_value_t = 42)]
        semilla: u64,
        /// Repo bueno. Sin él, se usa `[calibracion] repo` de `tarea.toml`.
        #[arg(long)]
        repo: Option<PathBuf>,
        /// Tope de gasto de API de la calibración, en USD (decisión 48). Debe ser finito y mayor que 0.
        /// Antes de cada caso, si el gasto más el peor caso del caso no entra, la calibración se detiene.
        #[arg(long, allow_negative_numbers = true)]
        tope_usd: Option<f64>,
    },
    /// Mide si el juicio de Root coincide con resultados conocidos (Tarea 12b). Con `--preparar-ciego`,
    /// arma los paquetes a ciegas de `--fuentes` en `casos/ciego/`.
    Validar {
        /// Carpeta de casos: `orden.toml`, `defectos.toml` y `ciego/`.
        #[arg(long)]
        casos: PathBuf,
        /// Arma los paquetes a ciegas en vez de medir. Exige `--fuentes`.
        #[arg(long, requires = "fuentes")]
        preparar_ciego: bool,
        /// Manifiesto con las `[[fuente]]` del paquete a ciegas. Exige `--preparar-ciego`.
        #[arg(long, requires = "preparar_ciego")]
        fuentes: Option<PathBuf>,
        /// Semilla de los ids del paquete a ciegas. Sin ella, vale el reloj. Exige `--preparar-ciego`.
        #[arg(long, requires = "preparar_ciego")]
        semilla: Option<u64>,
    },
    /// Audita un repo con el juez de Root y el reporte de gitleaks. Escribe la auditoría en `--salida`.
    Auditar {
        /// Repo a auditar.
        #[arg(long)]
        repo: PathBuf,
        /// Archivo JSON donde se escribe la auditoría.
        #[arg(long)]
        salida: PathBuf,
        /// Tope de gasto de API de la auditoría, en USD. Debe ser finito y mayor que 0. Sin `--solo-medir`, es obligatorio.
        #[arg(
            long,
            allow_negative_numbers = true,
            required_unless_present = "solo_medir",
            conflicts_with = "solo_medir"
        )]
        tope_usd: Option<f64>,
        /// Reporte JSON de gitleaks sobre el repo. Es obligatorio, también con `--solo-medir`.
        #[arg(long, required = true)]
        gitleaks: Option<PathBuf>,
        /// Ruta a incluir en la auditoría. Se repite: `--incluir a.rs --incluir b.rs`.
        #[arg(long, num_args = 1)]
        incluir: Vec<PathBuf>,
        /// Archivo TOML con defectos conocidos del repo.
        #[arg(long)]
        defectos: Option<PathBuf>,
        /// Una llamada por archivo, con el mismo prompt de sistema. Un archivo que no cabe se parte en partes.
        #[arg(long, conflicts_with = "solo_medir")]
        por_archivo: bool,
        /// Candidatos JSON de otro modelo. Aplica el gate y la medición sin llamar a ningún modelo.
        #[arg(long)]
        solo_medir: Option<PathBuf>,
    },
    /// Propone las capas del repo. Con `--tarea`, verifica las capas de la tarea y sale con 1 si no pasan.
    Capas {
        /// Repo a leer.
        #[arg(long)]
        repo: PathBuf,
        /// Tarea cuyas capas se verifican. Sin ella, se propone una línea `capas = [...]`.
        #[arg(long)]
        tarea: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum ObservaAccion {
    /// Registra la salida JSON de `claude -p`. Con `--via nube`, registra una sesión en la nube.
    Claude {
        #[arg(long = "archivo")]
        archivo: PathBuf,
        #[arg(long)]
        corrida: String,
        /// Vía de los eventos: `cupo` (default) o `nube`.
        #[arg(long, value_enum)]
        via: Option<ViaRegistro>,
    },
    /// Muestra el gasto y el margen de cada tope.
    Uso,
    /// Registra un costo de API que Root no vio, con su motivo (decisión 46).
    Costo {
        /// Monto en USD. Debe ser finito y mayor que 0.
        #[arg(long, allow_negative_numbers = true)]
        usd: f64,
        #[arg(long)]
        motivo: String,
        /// Fecha: `AAAA-MM-DD` (12:00 locales) o RFC 3339. Sin ella, vale la hora actual.
        #[arg(long)]
        fecha: Option<String>,
    },
    /// Registra un evento manual de la corrida.
    Evento {
        #[arg(long)]
        corrida: String,
        #[arg(long, value_enum)]
        tipo: TipoEvento,
        #[arg(long)]
        detalle: Option<String>,
    },
    /// Muestra el resumen de una corrida.
    Resumen {
        #[arg(long)]
        corrida: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TipoEvento {
    Humano,
    PlanAprobado,
    Gate,
}

impl From<TipoEvento> for crate::tipos::TipoEvento {
    fn from(tipo: TipoEvento) -> Self {
        match tipo {
            TipoEvento::Humano => crate::tipos::TipoEvento::Humano,
            TipoEvento::PlanAprobado => crate::tipos::TipoEvento::PlanAprobado,
            TipoEvento::Gate => crate::tipos::TipoEvento::Gate,
        }
    }
}

/// Vías que acepta `observa claude --via`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ViaRegistro {
    Cupo,
    Nube,
}

#[derive(Debug, Subcommand)]
pub enum TrackAccion {
    /// Agrega una fila desde un archivo JSON.
    Agregar {
        #[arg(long)]
        fila: PathBuf,
    },
    /// Lista las filas, opcionalmente de una tarea.
    Listar {
        #[arg(long)]
        tarea: Option<String>,
    },
    /// Compara dos corridas de una tarea.
    Comparar {
        #[arg(long)]
        a: String,
        #[arg(long)]
        b: String,
        #[arg(long)]
        tarea: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum GoalAccion {
    /// Verifica que GOAL.md coincida con GOAL.hash.
    Verificar,
    /// Autoriza un cambio al GOAL. Exige una TTY y confirmación tecleada.
    Autorizar,
}

impl Comando {
    /// Nombre del subcomando tal como lo escribe el usuario.
    pub fn ruta(&self) -> &'static str {
        match self {
            Comando::Route { .. } => "route",
            Comando::Llama { .. } => "llama",
            Comando::Oraculo { accion } => match accion {
                OraculoAccion::Evaluar { .. } => "oraculo evaluar",
                OraculoAccion::Informe { .. } => "oraculo informe",
                OraculoAccion::Pendientes { .. } => "oraculo pendientes",
                OraculoAccion::Calibrar { .. } => "oraculo calibrar",
                OraculoAccion::Validar { .. } => "oraculo validar",
                OraculoAccion::Auditar { .. } => "oraculo auditar",
                OraculoAccion::Capas { .. } => "oraculo capas",
            },
            Comando::Colab { accion } => match accion {
                ColabAccion::Empaquetar { .. } => "colab empaquetar",
                ColabAccion::Importar { .. } => "colab importar",
                ColabAccion::Calibrar { .. } => "colab calibrar",
            },
            Comando::Observa { accion } => match accion {
                ObservaAccion::Claude { .. } => "observa claude",
                ObservaAccion::Uso => "observa uso",
                ObservaAccion::Costo { .. } => "observa costo",
                ObservaAccion::Evento { .. } => "observa evento",
                ObservaAccion::Resumen { .. } => "observa resumen",
            },
            Comando::Track { accion } => match accion {
                TrackAccion::Agregar { .. } => "track agregar",
                TrackAccion::Listar { .. } => "track listar",
                TrackAccion::Comparar { .. } => "track comparar",
            },
            Comando::Aprende { .. } => "aprende",
            Comando::Corre { accion, .. } => match accion {
                Some(CorreAccion::RevisarLectura { .. }) => "corre revisar-lectura",
                None => "corre",
            },
            Comando::Goal { accion } => match accion {
                GoalAccion::Verificar => "goal verificar",
                GoalAccion::Autorizar => "goal autorizar",
            },
            Comando::ImportarExterno { .. } => "importar-mmorch",
            Comando::Esquema { .. } => "esquema",
            Comando::Ficha { accion } => match accion {
                FichaAccion::Revisar { .. } => "ficha revisar",
                FichaAccion::Tarea { .. } => "ficha tarea",
            },
            Comando::Mcp => "mcp",
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::error::ErrorKind;

    use super::*;

    /// SHA-256 falso de 64 caracteres, para las banderas que lo exigen.
    const SHA_FALSO: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    /// Argumentos base de `oraculo auditar`, con todos los obligatorios.
    fn base() -> Vec<&'static str> {
        vec![
            "r00t",
            "oraculo",
            "auditar",
            "--repo",
            "/repo",
            "--salida",
            "/salida.json",
            "--tope-usd",
            "0.5",
            "--gitleaks",
            "/gitleaks.json",
        ]
    }

    #[test]
    fn oraculo_auditar_parsea_flags_e_incluir_repetido_en_orden() {
        let mut args = base();
        args.extend([
            "--incluir",
            "src/a.rs",
            "--incluir",
            "src/b.rs",
            "--defectos",
            "/defectos.toml",
        ]);
        let cli = Cli::try_parse_from(args).expect("los flags de auditar parsean");
        assert_eq!(cli.comando.ruta(), "oraculo auditar");
        match cli.comando {
            Comando::Oraculo {
                accion:
                    OraculoAccion::Auditar {
                        repo,
                        salida,
                        tope_usd,
                        gitleaks,
                        incluir,
                        defectos,
                        por_archivo,
                        solo_medir,
                    },
            } => {
                assert_eq!(solo_medir, None);
                assert_eq!(repo, PathBuf::from("/repo"));
                assert_eq!(salida, PathBuf::from("/salida.json"));
                assert_eq!(tope_usd, Some(0.5));
                assert_eq!(gitleaks, Some(PathBuf::from("/gitleaks.json")));
                assert_eq!(
                    incluir,
                    vec![PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")]
                );
                assert_eq!(defectos, Some(PathBuf::from("/defectos.toml")));
                assert!(!por_archivo);
            }
            otro => panic!("se esperaba oraculo auditar, llegó {otro:?}"),
        }
    }

    #[test]
    fn oraculo_auditar_por_archivo_parsea_a_true() {
        let mut args = base();
        args.push("--por-archivo");
        let cli = Cli::try_parse_from(args).expect("--por-archivo parsea");
        match cli.comando {
            Comando::Oraculo {
                accion: OraculoAccion::Auditar { por_archivo, .. },
            } => assert!(por_archivo),
            otro => panic!("se esperaba oraculo auditar, llegó {otro:?}"),
        }
    }

    #[test]
    fn oraculo_auditar_sin_tope_usd_es_error_de_uso_de_clap() {
        let args: Vec<&str> = base()
            .into_iter()
            .filter(|a| *a != "--tope-usd" && *a != "0.5")
            .collect();
        let error = Cli::try_parse_from(args).expect_err("falta --tope-usd");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn solo_medir_no_pide_tope_pero_si_gitleaks() {
        let args = vec![
            "r00t",
            "oraculo",
            "auditar",
            "--repo",
            "/repo",
            "--salida",
            "/salida.json",
            "--gitleaks",
            "/gitleaks.json",
            "--solo-medir",
            "/candidatos.json",
        ];
        let cli = Cli::try_parse_from(args).expect("solo-medir no pide tope, con gitleaks parsea");
        match cli.comando {
            Comando::Oraculo {
                accion: OraculoAccion::Auditar { solo_medir, .. },
            } => assert_eq!(solo_medir, Some(PathBuf::from("/candidatos.json"))),
            otro => panic!("se esperaba oraculo auditar, llegó {otro:?}"),
        }

        let sin_gitleaks = vec![
            "r00t",
            "oraculo",
            "auditar",
            "--repo",
            "/repo",
            "--salida",
            "/salida.json",
            "--solo-medir",
            "/candidatos.json",
        ];
        let error = Cli::try_parse_from(sin_gitleaks).expect_err("solo-medir sin gitleaks falla");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn solo_medir_con_tope_usd_es_conflicto() {
        let mut args = base();
        args.extend(["--solo-medir", "/candidatos.json"]);
        let error = Cli::try_parse_from(args).expect_err("tope y solo-medir chocan");
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn oraculo_auditar_sin_gitleaks_es_error_de_uso_de_clap() {
        let args: Vec<&str> = base()
            .into_iter()
            .filter(|a| *a != "--gitleaks" && *a != "/gitleaks.json")
            .collect();
        let error = Cli::try_parse_from(args).expect_err("falta --gitleaks");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn oraculo_capas_parsea_solo_repo_y_propone() {
        let cli = Cli::try_parse_from(["r00t", "oraculo", "capas", "--repo", "/repo"])
            .expect("oraculo capas con --repo parsea");
        assert_eq!(cli.comando.ruta(), "oraculo capas");
        match cli.comando {
            Comando::Oraculo {
                accion: OraculoAccion::Capas { repo, tarea },
            } => {
                assert_eq!(repo, PathBuf::from("/repo"));
                assert_eq!(tarea, None);
            }
            otro => panic!("se esperaba oraculo capas, llegó {otro:?}"),
        }
    }

    #[test]
    fn oraculo_capas_con_tarea_parsea_el_nombre() {
        let cli = Cli::try_parse_from([
            "r00t", "oraculo", "capas", "--repo", "/repo", "--tarea", "mi-tarea",
        ])
        .expect("oraculo capas con --tarea parsea");
        match cli.comando {
            Comando::Oraculo {
                accion: OraculoAccion::Capas { repo, tarea },
            } => {
                assert_eq!(repo, PathBuf::from("/repo"));
                assert_eq!(tarea.as_deref(), Some("mi-tarea"));
            }
            otro => panic!("se esperaba oraculo capas, llegó {otro:?}"),
        }
    }

    /// Argumentos base de `colab empaquetar`, sin `--n` ni `--semilla`.
    fn empaquetar_base() -> Vec<&'static str> {
        vec![
            "r00t",
            "colab",
            "empaquetar",
            "--binario",
            "b",
            "--trabajos",
            "t:d",
            "--repo",
            "t=r",
            "--rustup-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "--salida",
            "s.tar.gz",
        ]
    }

    #[test]
    fn colab_empaquetar_parsea_n_y_semilla() {
        let mut args = empaquetar_base();
        args.extend(["--n", "8", "--semilla", "7"]);
        let cli = Cli::try_parse_from(args).expect("colab empaquetar con --n y --semilla parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Empaquetar { n, semilla, .. },
            } => {
                assert_eq!(n, 8);
                assert_eq!(semilla, 7);
            }
            otro => panic!("se esperaba colab empaquetar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_empaquetar_parsea_juez() {
        let mut args = empaquetar_base();
        args.push("--juez");
        let cli = Cli::try_parse_from(args).expect("colab empaquetar con --juez parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Empaquetar { juez, .. },
            } => assert!(juez),
            otro => panic!("se esperaba colab empaquetar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_empaquetar_parsea_tope_usd() {
        let mut args = empaquetar_base();
        args.extend(["--juez", "--tope-usd", "8"]);
        let cli = Cli::try_parse_from(args).expect("colab empaquetar con --tope-usd parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Empaquetar { tope_usd, .. },
            } => assert_eq!(tope_usd, Some(8.0)),
            otro => panic!("se esperaba colab empaquetar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_empaquetar_sin_tope_usd_por_defecto() {
        let cli = Cli::try_parse_from(empaquetar_base()).expect("colab empaquetar parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Empaquetar { tope_usd, .. },
            } => assert_eq!(tope_usd, None),
            otro => panic!("se esperaba colab empaquetar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_empaquetar_sin_juez_por_defecto() {
        let cli = Cli::try_parse_from(empaquetar_base()).expect("colab empaquetar parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Empaquetar { juez, .. },
            } => assert!(!juez),
            otro => panic!("se esperaba colab empaquetar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_empaquetar_sin_banderas_usa_defectos() {
        let cli = Cli::try_parse_from(empaquetar_base()).expect("colab empaquetar sin --n parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Empaquetar { n, semilla, .. },
            } => {
                assert_eq!(n, crate::colab_calibra::N_POR_DEFECTO);
                assert_eq!(semilla, crate::colab_calibra::SEMILLA_POR_DEFECTO);
                assert_eq!(n, 4);
                assert_eq!(semilla, 42);
            }
            otro => panic!("se esperaba colab empaquetar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_empaquetar_corre_parsea_tareas_config_y_pasada() {
        let cli = Cli::try_parse_from([
            "r00t",
            "colab",
            "empaquetar",
            "--corre",
            "--binario",
            "b",
            "--tareas",
            "t1,t2",
            "--config",
            "corrida",
            "--pasada",
            "3",
            "--rustup-sha256",
            SHA_FALSO,
            "--salida",
            "s.tar.gz",
        ])
        .expect("colab empaquetar --corre parsea");
        match cli.comando {
            Comando::Colab {
                accion:
                    ColabAccion::Empaquetar {
                        corre,
                        tareas,
                        config,
                        pasada,
                        ..
                    },
            } => {
                assert!(corre);
                assert_eq!(tareas, vec!["t1".to_string(), "t2".to_string()]);
                assert_eq!(config.as_deref(), Some("corrida"));
                assert_eq!(pasada, Some(3));
            }
            otro => panic!("se esperaba colab empaquetar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_empaquetar_sin_corre_parsea_como_antes_y_corre_es_falso() {
        let cli = Cli::try_parse_from(empaquetar_base()).expect("colab empaquetar parsea");
        match cli.comando {
            Comando::Colab {
                accion:
                    ColabAccion::Empaquetar {
                        corre,
                        trabajos,
                        repo,
                        rustup_sha256,
                        tareas,
                        config,
                        pasada,
                        ..
                    },
            } => {
                assert!(!corre);
                assert_eq!(trabajos, vec!["t:d".to_string()]);
                assert_eq!(repo, vec!["t=r".to_string()]);
                assert_eq!(rustup_sha256.as_deref().map(str::len), Some(64));
                assert!(tareas.is_empty());
                assert_eq!(config, None);
                assert_eq!(pasada, None);
            }
            otro => panic!("se esperaba colab empaquetar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_empaquetar_corre_con_trabajos_falla() {
        let mut args = vec![
            "r00t",
            "colab",
            "empaquetar",
            "--corre",
            "--binario",
            "b",
            "--tareas",
            "t1",
            "--config",
            "corrida",
            "--pasada",
            "1",
            "--rustup-sha256",
            SHA_FALSO,
            "--salida",
            "s.tar.gz",
        ];
        args.extend(["--trabajos", "t:d"]);
        let error = Cli::try_parse_from(args).expect_err("--corre con --trabajos falla");
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn colab_empaquetar_corre_sin_pasada_falla() {
        let error = Cli::try_parse_from([
            "r00t",
            "colab",
            "empaquetar",
            "--corre",
            "--binario",
            "b",
            "--tareas",
            "t1",
            "--config",
            "corrida",
            "--rustup-sha256",
            SHA_FALSO,
            "--salida",
            "s.tar.gz",
        ])
        .expect_err("--corre sin --pasada falla");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn colab_empaquetar_corre_sin_rustup_sha256_falla() {
        let error = Cli::try_parse_from([
            "r00t",
            "colab",
            "empaquetar",
            "--corre",
            "--binario",
            "b",
            "--tareas",
            "t1",
            "--config",
            "corrida",
            "--pasada",
            "1",
            "--salida",
            "s.tar.gz",
        ])
        .expect_err("--corre sin --rustup-sha256 falla");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn colab_empaquetar_sin_corre_sin_trabajos_falla() {
        let error = Cli::try_parse_from([
            "r00t",
            "colab",
            "empaquetar",
            "--binario",
            "b",
            "--repo",
            "t=r",
            "--rustup-sha256",
            "0000000000000000000000000000000000000000000000000000000000000000",
            "--salida",
            "s.tar.gz",
        ])
        .expect_err("sin --corre y sin --trabajos falla");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    /// Argumentos base de `colab importar`, con el paquete y sin banderas.
    fn importar_base() -> Vec<&'static str> {
        vec!["r00t", "colab", "importar", "paquete.tar.gz"]
    }

    #[test]
    fn colab_importar_parsea_sha() {
        let mut args = importar_base();
        args.extend(["--sha", "abc"]);
        let cli = Cli::try_parse_from(args).expect("colab importar con --sha parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Importar { sha, ida, .. },
            } => {
                assert_eq!(sha.as_deref(), Some("abc"));
                assert!(ida.is_none());
            }
            otro => panic!("se esperaba colab importar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_importar_ida_sin_sha_es_error_de_clap() {
        let mut args = importar_base();
        args.extend(["--ida", "ida.tar.gz"]);
        let error = Cli::try_parse_from(args).expect_err("--ida sin --sha debe fallar");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    /// Argumentos base de `colab calibrar`, con un paquete y sin banderas.
    fn calibrar_base() -> Vec<&'static str> {
        vec!["r00t", "colab", "calibrar", "--paquete", "x.tar.gz"]
    }

    #[test]
    fn colab_calibrar_con_sin_stop_lo_activa() {
        let mut args = calibrar_base();
        args.push("--sin-stop");
        let cli = Cli::try_parse_from(args).expect("colab calibrar con --sin-stop parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Calibrar { sin_stop, .. },
            } => assert!(sin_stop),
            otro => panic!("se esperaba colab calibrar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_calibrar_sin_la_bandera_da_falso() {
        let cli = Cli::try_parse_from(calibrar_base()).expect("colab calibrar parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Calibrar { sin_stop, .. },
            } => assert!(!sin_stop),
            otro => panic!("se esperaba colab calibrar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_importar_corre_con_sha_e_ida_parsea() {
        let cli = Cli::try_parse_from([
            "r00t", "colab", "importar", "--corre", "v.tar.gz", "--sha", "abc", "--ida", "p.tar.gz",
        ])
        .expect("colab importar --corre con --sha e --ida parsea");
        match cli.comando {
            Comando::Colab {
                accion:
                    ColabAccion::Importar {
                        archivo,
                        corre,
                        sha,
                        ida,
                        ..
                    },
            } => {
                assert!(archivo.is_none());
                assert_eq!(corre, Some(PathBuf::from("v.tar.gz")));
                assert_eq!(sha.as_deref(), Some("abc"));
                assert_eq!(ida, Some(PathBuf::from("p.tar.gz")));
            }
            otro => panic!("se esperaba colab importar --corre, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_importar_corre_sin_ida_falla() {
        let error = Cli::try_parse_from([
            "r00t", "colab", "importar", "--corre", "v.tar.gz", "--sha", "abc",
        ])
        .expect_err("--corre sin --ida debe fallar");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn colab_importar_corre_con_paquete_posicional_falla() {
        let error = Cli::try_parse_from([
            "r00t",
            "colab",
            "importar",
            "paquete.tar.gz",
            "--corre",
            "v.tar.gz",
            "--sha",
            "abc",
            "--ida",
            "p.tar.gz",
        ])
        .expect_err("el paquete posicional y --corre no van juntos");
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn colab_importar_sin_corre_parsea_el_paquete_como_siempre() {
        let cli = Cli::try_parse_from(importar_base()).expect("colab importar parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Importar { archivo, corre, .. },
            } => {
                assert_eq!(archivo, Some(PathBuf::from("paquete.tar.gz")));
                assert!(corre.is_none());
            }
            otro => panic!("se esperaba colab importar, llegó {otro:?}"),
        }
    }

    #[test]
    fn colab_importar_ida_con_sha_parsea() {
        let mut args = importar_base();
        args.extend(["--sha", "abc", "--ida", "ida.tar.gz"]);
        let cli = Cli::try_parse_from(args).expect("colab importar con --sha e --ida parsea");
        match cli.comando {
            Comando::Colab {
                accion: ColabAccion::Importar { ida, .. },
            } => assert_eq!(ida, Some(PathBuf::from("ida.tar.gz"))),
            otro => panic!("se esperaba colab importar, llegó {otro:?}"),
        }
    }

    #[test]
    fn corre_revisar_lectura_parsea_tarea_y_transcript() {
        let cli = Cli::try_parse_from([
            "r00t",
            "corre",
            "revisar-lectura",
            "--tarea",
            "t",
            "--transcript",
            "p",
        ])
        .expect("corre revisar-lectura parsea");
        match cli.comando {
            Comando::Corre {
                accion: Some(CorreAccion::RevisarLectura { tarea, transcript }),
                ..
            } => {
                assert_eq!(tarea, "t");
                assert_eq!(transcript, PathBuf::from("p"));
            }
            otro => panic!("se esperaba corre revisar-lectura, llegó {otro:?}"),
        }
    }

    #[test]
    fn corre_con_tarea_y_config_sigue_parseando_igual() {
        let cli = Cli::try_parse_from(["r00t", "corre", "--tarea", "t", "--config", "c"])
            .expect("corre con tarea y config parsea");
        match cli.comando {
            Comando::Corre {
                accion: None,
                tarea: Some(tarea),
                config: Some(config),
                reanuda: None,
            } => assert_eq!((tarea.as_str(), config.as_str()), ("t", "c")),
            otro => panic!("se esperaba corre plano, llegó {otro:?}"),
        }
    }

    #[test]
    fn corre_sin_nada_no_parsea_o_llega_sin_tarea_ni_config() {
        match Cli::try_parse_from(["r00t", "corre"]) {
            Err(_) => {}
            Ok(cli) => match cli.comando {
                Comando::Corre {
                    accion: None,
                    tarea: None,
                    config: None,
                    reanuda: None,
                } => {}
                otro => panic!("corre sin nada no debe llenar nada: {otro:?}"),
            },
        }
    }

    #[test]
    fn corre_mezclar_flags_planos_con_revisar_lectura_es_conflicto() {
        let error = Cli::try_parse_from([
            "r00t",
            "corre",
            "--tarea",
            "t",
            "revisar-lectura",
            "--transcript",
            "p",
        ])
        .expect_err("los flags planos y el subcomando se excluyen");
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict);
    }
}
