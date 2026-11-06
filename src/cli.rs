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
    /// Corre una tarea con una configuración.
    Corre {
        #[arg(long)]
        tarea: String,
        #[arg(long)]
        config: String,
        /// Retoma una corrida cortada: su id `corre-<tarea>-<config>-...`, con su carpeta en `$HOME`.
        #[arg(long)]
        reanuda: Option<String>,
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
        /// Reporte JSON de gitleaks sobre el repo. Sin `--solo-medir`, es obligatorio.
        #[arg(long, required_unless_present = "solo_medir")]
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
            Comando::Corre { .. } => "corre",
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
    fn solo_medir_no_pide_tope_ni_gitleaks() {
        let args = vec![
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
        let cli = Cli::try_parse_from(args).expect("solo-medir no pide tope ni gitleaks");
        match cli.comando {
            Comando::Oraculo {
                accion: OraculoAccion::Auditar { solo_medir, .. },
            } => assert_eq!(solo_medir, Some(PathBuf::from("/candidatos.json"))),
            otro => panic!("se esperaba oraculo auditar, llegó {otro:?}"),
        }
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
}
