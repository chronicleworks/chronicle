use std::convert::Infallible;

use clap::{
	*,
	builder::{PossibleValuesParser, StringValueParser},
};
use thiserror::Error;
use tokio::sync::broadcast::error::RecvError;
use tracing::info;
use user_error::UFE;

use api::ApiError;
use api::commands::{ActivityCommand, AgentCommand, ApiCommand, EntityCommand};
use chronicle_signing::SecretError;
use common::{
	attributes::{Attribute, Attributes},
	opa::std::{FromUrlError, OpaExecutorError, PolicyLoaderError},
	prov::{
		ActivityId, AgentId, DomaintypeId, EntityId, ExternalId,
		ExternalIdPart, json_ld::CompactionError, operations::DerivationType, ParseIriError,
	},
};
use protocol_substrate::SubxtClientError;

use crate::{
	codegen::{
		ActivityDef, AgentDef, AttributeDef, ChronicleDomainDef, CliName, EntityDef, TypeName,
	},
	PrimitiveType,
};

#[derive(Debug, Error)]
pub enum CliError {
    #[error("Missing argument: {arg}")]
    MissingArgument { arg: String },

    #[error("Invalid argument {arg} expected {expected} got {got}")]
    InvalidArgument { arg: String, expected: String, got: String },

    #[error("Bad argument: {0}")]
    ArgumentParsing(
        #[from]
        #[source]
        clap::Error,
    ),

    #[error("Invalid IRI: {0}")]
    InvalidIri(
        #[from]
        #[source]
        iri_string::validate::Error,
    ),

    #[error("Invalid Chronicle IRI: {0}")]
    InvalidChronicleIri(
        #[from]
        #[source]
        ParseIriError,
    ),

    #[error("Invalid JSON: {0}")]
    InvalidJson(
        #[from]
        #[source]
        serde_json::Error,
    ),

    #[error("Invalid URI: {0}")]
    InvalidUri(
        #[from]
        #[source]
        url::ParseError,
    ),

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(
        #[from]
        #[source]
        chrono::ParseError,
    ),

    #[error("Invalid coercion: {arg}")]
    InvalidCoercion { arg: String },

    #[error("API failure: {0}")]
    ApiError(
        #[from]
        #[source]
        ApiError,
    ),

    #[error("Secrets : {0}")]
    Secrets(
        #[from]
        #[source]
        SecretError,
    ),

    #[error("IO error: {0}")]
    InputOutput(
        #[from]
        #[source]
        std::io::Error,
    ),

    #[error("Invalid configuration file: {0}")]
    ConfigInvalid(
        #[from]
        #[source]
        toml::de::Error,
    ),

    #[error("Invalid path: {path}")]
    InvalidPath { path: String },

    #[error("Invalid JSON-LD: {0}")]
    Ld(
        #[from]
        #[source]
        CompactionError,
    ),

    #[error("Failure in commit notification stream: {0}")]
    CommitNoticiationStream(
        #[from]
        #[source]
        RecvError,
    ),

    #[error("Policy loader error: {0}")]
    OpaPolicyLoader(
        #[from]
        #[source]
        PolicyLoaderError,
    ),

    #[error("OPA executor error: {0}")]
    OpaExecutor(
        #[from]
        #[source]
        OpaExecutorError,
    ),

    #[error("Sawtooth communication error: {source}")]
    SubstrateError {
        #[from]
        #[source]
        source: SubxtClientError,
    },

    #[error("UTF-8 error: {0}")]
    Utf8Error(
        #[from]
        #[source]
        std::str::Utf8Error,
    ),

    #[error("Url conversion: {0}")]
    FromUrlError(
        #[from]
        #[source]
        FromUrlError,
    ),

    #[error("No on chain settings, but they are required by Chronicle")]
    NoOnChainSettings,
}

impl CliError {
    pub fn missing_argument(arg: impl Into<String>) -> Self {
        Self::MissingArgument { arg: arg.into() }
    }
}

/// Ugly but we need this until ! is stable, see <https://github.com/rust-lang/rust/issues/64715>
impl From<Infallible> for CliError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl UFE for CliError {}

pub trait SubCommand {
    fn as_cmd(&self) -> Command;
    fn matches(&self, matches: &ArgMatches) -> Result<Option<ApiCommand>, CliError>;
}

pub struct AttributeCliModel {
    pub attribute: AttributeDef,
    pub attribute_name: String,
    pub attribute_help: String,
}

impl AttributeCliModel {
    pub fn new(attribute: AttributeDef) -> Self {
        Self {
            attribute_name: format!("{}-attr", attribute.as_cli_name()),
            attribute_help: format!("The value of the {} attribute", attribute.as_type_name()),
            attribute,
        }
    }

    pub fn as_arg(&self) -> Arg {
        Arg::new(&*self.attribute_name)
            .long(&self.attribute_name)
            .help(&*self.attribute_help)
            .takes_value(true)
            .required(true)
    }
}

pub struct AgentCliModel {
    pub agent: AgentDef,
    pub attributes: Vec<AttributeCliModel>,
    pub about: String,
    pub define_about: String,
    pub external_id: String,
}

impl AgentCliModel {
    pub fn new(agent: &AgentDef) -> Self {
        let attributes = agent
            .attributes
            .iter()
            .map(|attr| AttributeCliModel::new(attr.clone()))
            .collect();
        Self {
            agent: agent.clone(),
            attributes,
            external_id: agent.as_cli_name(),
            about: format!("Operations on {} agents", agent.as_type_name()),
            define_about: format!("Define an agent of type {} with the given external_id or IRI, redefinition with different attribute values is not allowed", agent.as_type_name()),
		}
    }
}

fn name_from<'a, Id>(
    args: &'a ArgMatches,
    name_param: &str,
    id_param: &str,
) -> Result<ExternalId, CliError>
    where
        Id: 'a + TryFrom<String, Error=ParseIriError> + ExternalIdPart,
{
    if let Some(external_id) = args.get_one::<String>(name_param) {
        Ok(ExternalId::from(external_id))
    } else if let Some(iri) = args.get_one::<String>(id_param) {
        let id = Id::try_from(iri.to_string())?;
        Ok(id.external_id_part().to_owned())
    } else {
        Err(CliError::MissingArgument { arg: format!("Missing {name_param} and {id_param}") })
    }
}

fn id_from<'a, Id>(args: &'a ArgMatches, id_param: &str) -> Result<Id, CliError>
    where
        Id: 'a + TryFrom<String, Error=ParseIriError> + ExternalIdPart,
{
    if let Some(id) = args.get_one::<String>(id_param) {
        Ok(Id::try_from(id.to_string())?)
    } else {
        Err(CliError::MissingArgument { arg: format!("Missing {id_param} ") })
    }
}

fn id_from_option<'a, Id>(args: &'a ArgMatches, id_param: &str) -> Result<Option<Id>, CliError>
    where
        Id: 'a + TryFrom<String, Error=ParseIriError> + ExternalIdPart,
{
    match id_from(args, id_param) {
        Err(CliError::MissingArgument { .. }) => Ok(None),
        Err(e) => Err(e),
        Ok(id) => Ok(Some(id)),
    }
}

fn namespace_from(args: &ArgMatches) -> Result<ExternalId, CliError> {
    if let Some(namespace) = args.get_one::<String>("namespace") {
        Ok(ExternalId::from(namespace))
    } else {
        Err(CliError::MissingArgument { arg: "namespace".to_owned() })
    }
}

/// Deserialize to a JSON value and ensure that it matches the specified primitive type, we need to
/// force any bare literal text to be quoted use of coercion afterwards will produce a proper json
/// value type for non strings
fn attribute_value_from_param(
    arg: &str,
    value: &str,
    typ: PrimitiveType,
) -> Result<serde_json::Value, CliError> {
    let value = {
        if !value.contains('"') {
            format!(r#""{value}""#)
        } else {
            value.to_owned()
        }
    };

    let mut value = serde_json::from_str(&value)?;
    match typ {
        PrimitiveType::Bool => {
            if let Some(coerced) = valico::json_dsl::boolean()
                .coerce(&mut value, ".")
                .map_err(|_e| CliError::InvalidCoercion { arg: arg.to_owned() })?
            {
                Ok(coerced)
            } else {
                Ok(value)
            }
        }
        PrimitiveType::String => {
            if let Some(coerced) = valico::json_dsl::string()
                .coerce(&mut value, ".")
                .map_err(|_e| CliError::InvalidCoercion { arg: arg.to_owned() })?
            {
                Ok(coerced)
            } else {
                Ok(value)
            }
        }
        PrimitiveType::Int => {
            if let Some(coerced) = valico::json_dsl::i64()
                .coerce(&mut value, ".")
                .map_err(|_e| CliError::InvalidCoercion { arg: arg.to_owned() })?
            {
                Ok(coerced)
            } else {
                Ok(value)
            }
        }
        PrimitiveType::JSON => {
            if let Some(coerced) = valico::json_dsl::object()
                .coerce(&mut value, ".")
                .map_err(|_e| CliError::InvalidCoercion { arg: arg.to_owned() })?
            {
                Ok(coerced)
            } else {
                Ok(value)
            }
        }
    }
}

fn attributes_from(
    args: &ArgMatches,
    typ: impl AsRef<str>,
    attributes: &[AttributeCliModel],
) -> Result<Attributes, CliError> {
    Ok(Attributes::new(
        Some(DomaintypeId::from_external_id(typ)),
        attributes
            .iter()
            .map(|attr| {
                let value = attribute_value_from_param(
                    &attr.attribute_name,
                    args.get_one::<String>(&attr.attribute_name).unwrap(),
                    attr.attribute.primitive_type,
                )?;
                Ok::<_, CliError>(Attribute {
                    typ: attr.attribute.as_type_name(),
                    value: value.into(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

impl SubCommand for AgentCliModel {
    fn as_cmd(&self) -> Command {
        let cmd = Command::new(&*self.external_id).about(&*self.about);

        let mut define = Command::new("define")
            .about(&*self.define_about)
            .arg(Arg::new("external_id")
                .help("An externally meaningful identifier for the agent, e.g. a URI or relational id")
                .takes_value(true))
            .arg(Arg::new("id")
                .help("A valid chronicle agent IRI")
                .long("id")
                .takes_value(true))
            .group(ArgGroup::new("identifier")
                .args(&["external_id", "id"])
                .required(true))
            .arg(
                Arg::new("namespace")
                    .short('n')
                    .long("namespace")
                    .default_value("default")
                    .required(false)
                    .takes_value(true),
            );

        for attr in &self.attributes {
            define = define.arg(attr.as_arg());
        }

        cmd.subcommand(define).subcommand(
            Command::new("use")
                .about("Make the specified agent the context for activities and entities")
                .arg(
                    Arg::new("id")
                        .help("A valid chronicle agent IRI")
                        .required(true)
                        .takes_value(true),
                )
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .default_value("default")
                        .required(false)
                        .takes_value(true),
                ),
        )
    }

    fn matches(&self, matches: &ArgMatches) -> Result<Option<ApiCommand>, CliError> {
        if let Some(matches) = matches.subcommand_matches("define") {
            return Ok(Some(ApiCommand::Agent(AgentCommand::Create {
                id: name_from::<AgentId>(matches, "external_id", "id")?,
                namespace: namespace_from(matches)?,
                attributes: attributes_from(matches, &self.agent.external_id, &self.attributes)?,
            })));
        }

        if let Some(matches) = matches.subcommand_matches("use") {
            return Ok(Some(ApiCommand::Agent(AgentCommand::UseInContext {
                id: id_from(matches, "id")?,
                namespace: namespace_from(matches)?,
            })));
        };

        Ok(None)
    }
}

pub struct ActivityCliModel {
    pub activity: ActivityDef,
    pub attributes: Vec<AttributeCliModel>,
    pub about: String,
    pub define_about: String,
    pub external_id: String,
}

impl ActivityCliModel {
    fn new(activity: &ActivityDef) -> Self {
        let attributes = activity
            .attributes
            .iter()
            .map(|attr| AttributeCliModel::new(attr.clone()))
            .collect();
        Self {
            activity: activity.clone(),
            attributes,
            external_id: activity.as_cli_name(),
            about: format!("Operations on {} activities", activity.as_type_name()),
            define_about: format!("Define an activity of type {} with the given external_id or IRI, redefinition with different attribute values is not allowed", activity.as_type_name()),
        }
    }
}

impl SubCommand for ActivityCliModel {
    fn as_cmd(&self) -> Command {
        let cmd = Command::new(&*self.external_id).about(&*self.about);

        let mut define =
            Command::new("define")
                .about(&*self.define_about)
                .arg(Arg::new("external_id")
                    .help("An externally meaningful identifier for the activity , e.g. a URI or relational id")
                    .takes_value(true))
                .arg(Arg::new("id")
                    .long("id")
                    .help("A valid chronicle activity IRI")
                    .takes_value(true))
                .group(ArgGroup::new("identifier")
                    .args(&["external_id", "id"])
                    .required(true))
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .default_value("default")
                        .required(false)
                        .takes_value(true),
                );

        for attr in &self.attributes {
            define = define.arg(attr.as_arg());
        }

        cmd.subcommand(define)
            .subcommand(
                Command::new("start")
                    .about("Record this activity as started at the specified time, if no time is specified the current time is used")
                    .arg(Arg::new("id")
                        .help("A valid chronicle activity IRI")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::new("agent_id")
                        .help("A valid chronicle agent IRI")
                        .long("agent")
                        .takes_value(true)
                        .required(false)
                    )
                    .arg(
                        Arg::new("namespace")
                            .short('n')
                            .long("namespace")
                            .default_value("default")
                            .required(false)
                            .takes_value(true),
                    )
                    .arg(
                        Arg::new("time")
                            .long("time")
                            .help("A valid RFC3339 timestamp")
                            .required(false)
                            .takes_value(true)
                    )
            )
            .subcommand(
                Command::new("end")
                    .about("Record this activity as ended at the specified time, if no time is specified the current time is used")
                    .arg(Arg::new("id")
                        .help("A valid chronicle activity IRI")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::new("agent_id")
                        .long("agent")
                        .help("A valid chronicle agent IRI")
                        .takes_value(true)
                        .required(false)
                    )
                    .arg(
                        Arg::new("namespace")
                            .short('n')
                            .long("namespace")
                            .default_value("default")
                            .required(false)
                            .takes_value(true),
                    )
                    .arg(
                        Arg::new("time")
                            .long("time")
                            .help("A valid RFC3339 timestamp")
                            .required(false)
                            .takes_value(true)
                    )
            )
            .subcommand(
                Command::new("instant")
                    .about("Record this activity as taking place at the specified time, if no time is specified the current time is used")
                    .arg(Arg::new("id")
                        .help("A valid chronicle activity IRI")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::new("agent_id")
                        .long("agent")
                        .help("A valid chronicle agent IRI")
                        .takes_value(true)
                        .required(false)
                    )
                    .arg(
                        Arg::new("namespace")
                            .short('n')
                            .long("namespace")
                            .default_value("default")
                            .required(false)
                            .takes_value(true),
                    )
                    .arg(
                        Arg::new("time")
                            .long("time")
                            .help("A valid RFC3339 timestamp")
                            .required(false)
                            .takes_value(true)
                    )
            )
            .subcommand(
                Command::new("use")
                    .about("Record this activity as having used the specified entity, creating it if required")
                    .arg(Arg::new("entity_id")
                        .help("A valid chronicle entity IRI")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::new("activity_id")
                        .help("A valid chronicle activity IRI")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(
                        Arg::new("namespace")
                            .short('n')
                            .long("namespace")
                            .default_value("default")
                            .required(false)
                            .takes_value(true),
                    )
            )
            .subcommand(
                Command::new("generate")
                    .about("Records this activity as having generated the specified entity, creating it if required")
                    .arg(Arg::new("entity_id")
                        .help("A valid chronicle entity IRI")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(Arg::new("activity_id")
                        .help("A valid chronicle activity IRI")
                        .takes_value(true)
                        .required(true)
                    )
                    .arg(
                        Arg::new("namespace")
                            .short('n')
                            .long("namespace")
                            .default_value("default")
                            .required(false)
                            .takes_value(true),
                    )
            )
    }

    fn matches(&self, matches: &ArgMatches) -> Result<Option<ApiCommand>, CliError> {
        if let Some(matches) = matches.subcommand_matches("define") {
            return Ok(Some(ApiCommand::Activity(ActivityCommand::Create {
                id: name_from::<ActivityId>(matches, "external_id", "id")?,
                namespace: namespace_from(matches)?,
                attributes: attributes_from(matches, &self.activity.external_id, &self.attributes)?,
            })));
        }

        if let Some(matches) = matches.subcommand_matches("start") {
            return Ok(Some(ApiCommand::Activity(ActivityCommand::Start {
                id: id_from(matches, "id")?,
                namespace: namespace_from(matches)?,
                time: matches.get_one::<String>("time").map(|t| t.parse()).transpose()?,
                agent: id_from_option(matches, "agent_id")?,
            })));
        };

        if let Some(matches) = matches.subcommand_matches("end") {
            return Ok(Some(ApiCommand::Activity(ActivityCommand::End {
                id: id_from(matches, "id")?,
                namespace: namespace_from(matches)?,
                time: matches.get_one::<String>("time").map(|t| t.parse()).transpose()?,
                agent: id_from_option(matches, "agent_id")?,
            })));
        };

        if let Some(matches) = matches.subcommand_matches("instant") {
            return Ok(Some(ApiCommand::Activity(ActivityCommand::Instant {
                id: id_from(matches, "id")?,
                namespace: namespace_from(matches)?,
                time: matches.get_one::<String>("time").map(|t| t.parse()).transpose()?,
                agent: id_from_option(matches, "agent_id")?,
            })));
        };

        if let Some(matches) = matches.subcommand_matches("use") {
            return Ok(Some(ApiCommand::Activity(ActivityCommand::Use {
                id: id_from(matches, "entity_id")?,
                namespace: namespace_from(matches)?,
                activity: id_from(matches, "activity_id")?,
            })));
        };

        if let Some(matches) = matches.subcommand_matches("generate") {
            return Ok(Some(ApiCommand::Activity(ActivityCommand::Generate {
                id: id_from(matches, "entity_id")?,
                namespace: namespace_from(matches)?,
                activity: id_from(matches, "activity_id")?,
            })));
        };

        Ok(None)
    }
}

pub struct EntityCliModel {
    pub entity: EntityDef,
    pub attributes: Vec<AttributeCliModel>,
    pub about: String,
    pub define_about: String,
    pub external_id: String,
}

impl EntityCliModel {
    pub fn new(entity: &EntityDef) -> Self {
        let attributes = entity
            .attributes
            .iter()
            .map(|attr| AttributeCliModel::new(attr.clone()))
            .collect();
        Self {
            entity: entity.clone(),
            attributes,
            external_id: entity.as_cli_name(),
            about: format!("Operations on {} entities", entity.as_type_name()),
            define_about: format!("Define an entity of type {} with the given external_id or IRI, redefinition with different attribute values is not allowed", entity.as_type_name()),
        }
    }
}

impl SubCommand for EntityCliModel {
    fn as_cmd(&self) -> Command {
        let cmd = Command::new(&self.external_id).about(&*self.about);

        let mut define =
            Command::new("define")
                .about(&*self.define_about)
                .arg(Arg::new("external_id")
                    .help("An externally meaningful identifier for the entity, e.g. a URI or relational id")
                    .takes_value(true))
                .arg(Arg::new("id")
                    .long("id")
                    .help("A valid chronicle entity IRI")
                    .takes_value(true))
                .group(ArgGroup::new("identifier")
                    .args(&["external_id", "id"])
                    .required(true))
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .default_value("default")
                        .required(false)
                        .takes_value(true),
                );

        for attr in &self.attributes {
            define = define.arg(attr.as_arg());
        }

        cmd.subcommand(define).subcommand(
            Command::new("derive")
                .about("Derivation of entities from other entities")
                .arg(
                    Arg::new("subtype")
                        .help("The derivation subtype")
                        .long("subtype")
                        .required(false)
                        .takes_value(true)
                        .value_parser(PossibleValuesParser::new([
                            "revision",
                            "quotation",
                            "primary-source",
                        ])),
                )
                .arg(
                    Arg::new("generated_entity_id")
                        .help("A valid chronicle entity IRI for the generated entity")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::new("used_entity_id")
                        .help("A valid chronicle entity IRI for the used entity")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::new("activity_id")
                        .help("The activity IRI that generated the entity")
                        .long("activity")
                        .takes_value(true)
                        .required(false),
                )
                .arg(
                    Arg::new("namespace")
                        .short('n')
                        .long("namespace")
                        .default_value("default")
                        .required(false)
                        .takes_value(true),
                ),
        )
    }

    fn matches(&self, matches: &ArgMatches) -> Result<Option<ApiCommand>, CliError> {
        if let Some(matches) = matches.subcommand_matches("define") {
            return Ok(Some(ApiCommand::Entity(EntityCommand::Create {
                id: name_from::<EntityId>(matches, "external_id", "id")?,
                namespace: namespace_from(matches)?,
                attributes: attributes_from(matches, &self.entity.external_id, &self.attributes)?,
            })));
        }

        if let Some(matches) = matches.subcommand_matches("derive") {
            return Ok(Some(ApiCommand::Entity(EntityCommand::Derive {
                namespace: namespace_from(matches)?,
                id: id_from(matches, "generated_entity_id")?,
                derivation: matches
                    .get_one::<String>("subtype")
                    .map(|v| match v.as_str() {
                        "revision" => DerivationType::Revision,
                        "quotation" => DerivationType::Quotation,
                        "primary-source" => DerivationType::PrimarySource,
                        _ => unreachable!(), // Guaranteed by PossibleValuesParser
                    })
                    .unwrap_or(DerivationType::None),
                activity: id_from_option(matches, "activity_id")?,
                used_entity: id_from(matches, "used_entity_id")?,
            })));
        }

        Ok(None)
    }
}

pub struct CliModel {
    pub domain: ChronicleDomainDef,
    pub agents: Vec<AgentCliModel>,
    pub entities: Vec<EntityCliModel>,
    pub activities: Vec<ActivityCliModel>,
}

pub const LONG_VERSION: &str = const_format::formatcp!(
	"{}:{} ({})",
	env!("CARGO_PKG_VERSION"),
	include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../.VERSION")),
	if cfg!(feature = "devmode") { "in memory" } else { "substrate" }
);

impl From<ChronicleDomainDef> for CliModel {
    fn from(val: ChronicleDomainDef) -> Self {
        info!(chronicle_version = LONG_VERSION);
        CliModel {
            agents: val.agents.iter().map(AgentCliModel::new).collect(),
            entities: val.entities.iter().map(EntityCliModel::new).collect(),
            activities: val.activities.iter().map(ActivityCliModel::new).collect(),
            domain: val,
        }
    }
}

impl SubCommand for CliModel {
    fn as_cmd(&self) -> Command {
        let mut app = Command::new("chronicle")
            .version(LONG_VERSION)
            .author("Blockchain Technology Partners")
            .about("Write and query provenance data to distributed ledgers")
            .arg(
                Arg::new("enable-otel")
                    .short('i')
                    .long("enable-otel")
                    .value_name("enable-otel")
                    .takes_value(false)
                    .help("Instrument using OLTP environment"),
            )
            .arg(Arg::new("console-logging").long("console-logging")
                .takes_value(true)
                .possible_values(["pretty", "json"])
                .default_value("pretty")
                .help(
                    "Instrument using RUST_LOG environment, writing in either human readable format or structured json to stdio",
                ))
            .arg(
                Arg::new("remote-database")
                    .long("remote-database")
                    .help("connect to a provided PostgreSQL (option is ignored and deprecated)")
            )
            .arg(
                Arg::new("database-host")
                    .long("database-host")
                    .takes_value(true)
                    .env("PGHOST")
                    .help("PostgreSQL hostname")
                    .default_value("localhost"),
            )
            .arg(
                Arg::new("database-port")
                    .long("database-port")
                    .takes_value(true)
                    .env("PGPORT")
                    .help("PostgreSQL port")
                    .default_value("5432"),
            )
            .arg(
                Arg::new("database-username")
                    .long("database-username")
                    .takes_value(true)
                    .env("PGUSER")
                    .help("PostgreSQL username")
                    .default_value("chronicle"),
            )
            .arg(
                Arg::new("database-name")
                    .long("database-name")
                    .takes_value(true)
                    .env("PGDATABASE")
                    .help("Name of the database")
                    .default_value("chronicle"),
            )
            .arg(
                Arg::new("opa-bundle-address")
                    .long("opa-bundle-address")
                    .takes_value(true)
                    .help("URL or path for loading OPA policy bundle")
            )
            .arg(
                Arg::with_name("opa-policy-name")
                    .long("opa-policy-name")
                    .help("Name of the OPA policy to be used")
                    .takes_value(true)
            )
            .arg(
                Arg::with_name("opa-policy-entrypoint")
                    .long("opa-policy-entrypoint")
                    .help("Entrypoint to the named OPA policy")
                    .takes_value(true)
            )
            .group(
                ArgGroup::with_name("opa-bundle-address-args")
                    .args(&["opa-bundle-address"])
                    .requires_all(&["opa-policy-name", "opa-policy-entrypoint"]),
            )
            .arg(
                Arg::new("namespace-binding")
                    .long("namespace-binding")
                    .takes_value(true)
                    .action(clap::ArgAction::Append)
                    .value_name("namespace-binding")
                    .help("Namespace binding")
                    .takes_value(true)
            )
            .subcommand(
                Command::new("completions")
                    .about("Generate shell completions and exit")
                    .arg(
                        Arg::new("shell")
                            .value_parser(PossibleValuesParser::new(["bash", "zsh", "fish"]))
                            .default_value("bash")
                            .help("Shell to generate completions for"),
                    ),
            )
            .subcommand(Command::new("export-schema").about("Print SDL and exit"))
            .subcommand(
                Command::new("serve-api")
                    .alias("serve-graphql")
                    .about("Start an API server")
                    .arg(
                        Arg::new("arrow-interface")
                            .long("arrow-interface")
                            .takes_value(true)
                            .min_values(1)
                            .default_values(&["localhost:9983"])
                            .env("ARROW_LISTEN_SOCKET")
                            .help("The arrow flight address"),
                    )
                    .arg(
                        Arg::new("interface")
                            .long("interface")
                            .takes_value(true)
                            .min_values(1)
                            .default_values(&["localhost:9982"])
                            .env("API_LISTEN_SOCKET")
                            .help("The API server address"),
                    ).arg(
                    Arg::new("playground")
                        .long("playground")
                        .alias("open")
                        .required(false)
                        .takes_value(false)
                        .help("Deprecated option (after v0.6.0) to make available the GraphQL Playground"),
                ).arg(
                    Arg::new("require-auth")
                        .long("require-auth")
                        .requires("oidc-endpoint-address")
                        .env("REQUIRE_AUTH")
                        .help("if JWT must be provided, preventing anonymous requests"),
                ).arg(
                    Arg::new("liveness-check")
                        .long("liveness-check")
                        .help("Turn on liveness depth charge checks and specify the interval in seconds")
                        .takes_value(true)
                        .value_name("interval")
                        .default_missing_value("1800"),
                ).arg(
                    Arg::new("jwks-address")
                        .long("jwks-address")
                        .takes_value(true)
                        .env("JWKS_URI")
                        .help("URI of the JSON key set for verifying web tokens"),
                ).arg({
                    Arg::new("userinfo-address")
                        .long("userinfo-address")
                        .takes_value(true)
                        .env("USERINFO_URI")
                        .help("URI of the OIDC UserInfo endpoint")
                }
                ).group(
                    ArgGroup::new("oidc-endpoint-address")
                        .args(&["jwks-address", "userinfo-address"])
                        .multiple(true)
                ).arg(
                    Arg::new("id-claims")
                        .long("id-claims")
                        .takes_value(true)
                        .min_values(1)
                        .default_values(&["iss", "sub"])
                        .env("JWT_ID_CLAIMS")
                        .help("JWT claims that determine Chronicle ID"),
                )
                    .arg(
                        Arg::new("jwt-must-claim")
                            .long("jwt-must-claim")
                            .multiple_occurrences(true)
                            .multiple_values(true)
                            .number_of_values(2)
                            .help("claim name and value that must be present for accepting a JWT")
                    )
                    .arg(
                        Arg::new("offer-endpoints")
                            .long("offer-endpoints")
                            .takes_value(true)
                            .min_values(1)
                            .value_parser(["data", "graphql"])
                            .default_values(&["data", "graphql"])
                            .help("which API endpoints to offer")
                    ),
            )
             .subcommand(
                Command::new("import")
                    .about("Import and apply Chronicle operations, then exit")
                    .arg(
                        Arg::new("namespace-id")
                            .value_name("NAMESPACE_ID")
                            .help("External ID of the namespace to import into")
                            .required(true)
                    )
                    .arg(
                        Arg::new("namespace-uuid")
                            .value_name("NAMESPACE_UUID")
                            .help("UUID of the namespace to import into")
                            .required(true)
                    )
                    .arg(
                        Arg::new("url")
                            .value_name("URL")
                            .default_value("import.json")
                            .value_hint(ValueHint::Url)
                            .value_parser(StringValueParser::new())
                            .help("A path or url to data import file"),
                    )
            );

        for agent in self.agents.iter() {
            app = app.subcommand(agent.as_cmd());
        }
        for activity in self.activities.iter() {
            app = app.subcommand(activity.as_cmd());
        }
        for entity in self.entities.iter() {
            app = app.subcommand(entity.as_cmd());
        }

        #[cfg(not(feature = "devmode"))]
        {
            app = app.arg(
                Arg::new("batcher-key-from-path")
                    .long("batcher-key-from-path")
                    .takes_value(true)
                    .value_parser(clap::builder::PathBufValueParser::new())
                    .value_hint(ValueHint::DirPath)
                    .help("Path to a directory containing the key for signing batches")
                    .conflicts_with("batcher-key-from-vault")
                    .conflicts_with("batcher-key-generated"),
            );

            app = app.arg(
                Arg::new("batcher-key-from-vault")
                    .long("batcher-key-from-vault")
                    .takes_value(false)
                    .help("Use Hashicorp Vault to store the batcher key")
                    .conflicts_with("batcher-key-from-path")
                    .conflicts_with("batcher-key-generated"),
            );

            app = app.arg(
                Arg::new("batcher-key-generated")
                    .long("batcher-key-generated")
                    .takes_value(false)
                    .help("Generate the batcher key in memory")
                    .conflicts_with("batcher-key-from-path")
                    .conflicts_with("batcher-key-from-vault"),
            );

            app = app.arg(
                Arg::new("chronicle-key-from-path")
                    .long("chronicle-key-from-path")
                    .takes_value(true)
                    .value_hint(ValueHint::DirPath)
                    .value_parser(clap::builder::PathBufValueParser::new())
                    .help("Path to a directory containing the key for signing identities and query results")
                    .conflicts_with("chronicle-key-from-vault")
                    .conflicts_with("chronicle-key-generated"),
            );

            app = app.arg(
                Arg::new("chronicle-key-from-vault")
                    .long("chronicle-key-from-vault")
                    .takes_value(false)
                    .help("Use Hashicorp Vault to store the Chronicle key")
                    .conflicts_with("chronicle-key-from-path")
                    .conflicts_with("chronicle-key-generated"),
            );

            app = app.arg(
                Arg::new("chronicle-key-generated")
                    .long("chronicle-key-generated")
                    .takes_value(false)
                    .help("Generate the Chronicle key in memory")
                    .conflicts_with("chronicle-key-from-path")
                    .conflicts_with("chronicle-key-from-vault"),
            );

            app = app.arg(
                Arg::new("vault-address")
                    .long("vault-address")
                    .takes_value(true)
                    .value_hint(ValueHint::Url)
                    .help("URL for connecting to Hashicorp Vault")
                    .env("VAULT_ADDRESS"),
            );

            app = app.arg(
                Arg::new("vault-token")
                    .long("vault-token")
                    .takes_value(true)
                    .help("Token for connecting to Hashicorp Vault")
                    .env("VAULT_TOKEN"),
            );

            app = app.arg(
                Arg::new("vault-mount-path")
                    .long("vault-mount-path")
                    .takes_value(true)
                    .value_hint(ValueHint::DirPath)
                    .help("Mount path for vault secrets")
                    .env("VAULT_MOUNT_PATH"),
            );

            app.arg(
                Arg::new("validator")
                    .long("validator")
                    .value_name("validator")
                    .value_hint(ValueHint::Url)
                    .help("Sets validator address")
                    .takes_value(true),
            )
                .arg(
                    Arg::new("embedded-opa-policy")
                        .long("embedded-opa-policy")
                        .takes_value(false)
                        .help(
                            "Operate without an external OPA policy, using an embedded default policy",
                        ),
                )
        }
        #[cfg(feature = "devmode")]
        {
            app
        }
    }

    /// Iterate our possible subcommands via model and short circuit with the first one that matches
    fn matches(&self, matches: &ArgMatches) -> Result<Option<ApiCommand>, CliError> {
        for (agent, matches) in self.agents.iter().filter_map(|agent| {
            matches.subcommand_matches(&agent.external_id).map(|matches| (agent, matches))
        }) {
            if let Some(cmd) = agent.matches(matches)? {
                return Ok(Some(cmd));
            }
        }
        for (entity, matches) in self.entities.iter().filter_map(|entity| {
            matches.subcommand_matches(&entity.external_id).map(|matches| (entity, matches))
        }) {
            if let Some(cmd) = entity.matches(matches)? {
                return Ok(Some(cmd));
            }
        }
        for (activity, matches) in self.activities.iter().filter_map(|activity| {
            matches
                .subcommand_matches(&activity.external_id)
                .map(|matches| (activity, matches))
        }) {
            if let Some(cmd) = activity.matches(matches)? {
                return Ok(Some(cmd));
            }
        }
        Ok(None)
    }
}

pub fn cli(domain: ChronicleDomainDef) -> CliModel {
    CliModel::from(domain)
}
