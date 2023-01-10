use std::{collections::BTreeMap, convert::Infallible, path::PathBuf};

use api::ApiError;
use clap::{builder::PossibleValuesParser, *};
use common::{
    attributes::{Attribute, Attributes},
    commands::{
        ActivityCommand, AgentCommand, ApiCommand, EntityCommand, KeyImport, KeyRegistration,
        PathOrFile,
    },
    prov::{
        operations::DerivationType, ActivityId, AgentId, CompactionError, DomaintypeId, EntityId,
        ExternalId, ExternalIdPart, ParseIriError,
    },
    signing::SignerError,
};
use iref::Iri;
use tokio::sync::broadcast::error::RecvError;
use user_error::UFE;

use crate::{
    codegen::{
        ActivityDef, AgentDef, AttributeDef, ChronicleDomainDef, CliName, EntityDef, TypeName,
    },
    PrimitiveType,
};

custom_error::custom_error! {pub CliError
    MissingArgument{arg: String}                    = "Missing argument: {arg}",
    InvalidArgument{arg: String, expected: String, got: String } = "Invalid argument {arg} expected {expected} got {got}",
    ArgumentParsing{source: clap::Error}            = "Bad argument: {source}",
    InvalidIri{source: iref::Error}                 = "Invalid IRI: {source}",
    InvalidChronicleIri{source: ParseIriError}      = "Invalid Chronicle IRI: {source}",
    InvalidJson{source: serde_json::Error}          = "Invalid JSON: {source}",
    InvalidTimestamp{source: chrono::ParseError}    = "Invalid timestamp: {source}",
    InvalidCoercion{arg: String}                    = "Invalid coercion: {arg}",
    ApiError{source: ApiError}                      = "API failure: {source}",
    Keys{source: SignerError}                       = "Key storage: {source}",
    FileSystem{source: std::io::Error}              = "Cannot locate configuration file: {source}",
    ConfigInvalid{source: toml::de::Error}          = "Invalid configuration file: {source}",
    InvalidPath{path: String}                       = "Invalid path: {path}",
    Ld{source: CompactionError}                     = "Invalid JSON-LD: {source}",
    CommitNoticiationStream {source: RecvError}     = "Failure in commit notification stream: {source}",
}

/// Ugly but we need this until ! is stable, see <https://github.com/rust-lang/rust/issues/64715>
impl From<Infallible> for CliError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl UFE for CliError {}

pub(crate) trait SubCommand {
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
            define_about: format!("Define an agent of type {} with the given external_id or IRI, re-definition with different attribute values is not allowed", agent.as_type_name())
        }
    }
}

fn name_from<'a, Id>(
    args: &'a ArgMatches,
    name_param: &str,
    id_param: &str,
) -> Result<ExternalId, CliError>
where
    Id: 'a + TryFrom<Iri<'a>, Error = ParseIriError> + ExternalIdPart,
{
    if let Some(external_id) = args.get_one::<String>(name_param) {
        Ok(ExternalId::from(external_id))
    } else if let Some(id) = args.get_one::<String>(id_param) {
        let iri = Iri::from_str(id)?;
        let id = Id::try_from(iri)?;
        Ok(id.external_id_part().to_owned())
    } else {
        Err(CliError::MissingArgument {
            arg: format!("Missing {} and {}", name_param, id_param),
        })
    }
}

fn id_from<'a, Id>(args: &'a ArgMatches, id_param: &str) -> Result<Id, CliError>
where
    Id: 'a + TryFrom<Iri<'a>, Error = ParseIriError> + ExternalIdPart,
{
    if let Some(id) = args.get_one::<String>(id_param) {
        Ok(Id::try_from(Iri::from_str(id)?)?)
    } else {
        Err(CliError::MissingArgument {
            arg: format!("Missing {} ", id_param),
        })
    }
}

fn id_from_option<'a, Id>(args: &'a ArgMatches, id_param: &str) -> Result<Option<Id>, CliError>
where
    Id: 'a + TryFrom<Iri<'a>, Error = ParseIriError> + ExternalIdPart,
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
        Err(CliError::MissingArgument {
            arg: "namespace".to_owned(),
        })
    }
}

/// Deserialize to a JSON value and ensure that it matches the specified primitive type, we need to force any bare literal text to be quoted
/// use of coercion afterwards will produce a proper json value type for non strings
fn attribute_value_from_param(
    arg: &str,
    value: &str,
    typ: PrimitiveType,
) -> Result<serde_json::Value, CliError> {
    let value = {
        if !value.contains('"') {
            format!(r#""{}""#, value)
        } else {
            value.to_owned()
        }
    };

    let mut value = serde_json::from_str(&value)?;
    match typ {
        PrimitiveType::Bool => {
            if let Some(coerced) = valico::json_dsl::boolean()
                .coerce(&mut value, ".")
                .map_err(|_e| CliError::InvalidCoercion {
                    arg: arg.to_owned(),
                })?
            {
                Ok(coerced)
            } else {
                Ok(value)
            }
        }
        PrimitiveType::String => {
            if let Some(coerced) =
                valico::json_dsl::string()
                    .coerce(&mut value, ".")
                    .map_err(|_e| CliError::InvalidCoercion {
                        arg: arg.to_owned(),
                    })?
            {
                Ok(coerced)
            } else {
                Ok(value)
            }
        }
        PrimitiveType::Int => {
            if let Some(coerced) =
                valico::json_dsl::i64()
                    .coerce(&mut value, ".")
                    .map_err(|_e| CliError::InvalidCoercion {
                        arg: arg.to_owned(),
                    })?
            {
                Ok(coerced)
            } else {
                Ok(value)
            }
        }
        PrimitiveType::JSON => {
            if let Some(coerced) =
                valico::json_dsl::object()
                    .coerce(&mut value, ".")
                    .map_err(|_e| CliError::InvalidCoercion {
                        arg: arg.to_owned(),
                    })?
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
    Ok(Attributes {
        typ: Some(DomaintypeId::from_external_id(typ)),
        attributes: attributes
            .iter()
            .map(|attr| {
                let value = attribute_value_from_param(
                    &attr.attribute_name,
                    args.get_one::<String>(&attr.attribute_name).unwrap(),
                    attr.attribute.primitive_type,
                )?;
                Ok::<_, CliError>((
                    attr.attribute.as_type_name(),
                    Attribute {
                        typ: attr.attribute.as_type_name(),
                        value,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?,
    })
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
                                    .args(&["external_id","id"])
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
        .subcommand(Command::new("register-key")
            .about("Register a key pair, or a public key with an agent")
            .arg(Arg::new("id")
                .help("A valid chronicle agent IRI")
                .required(true)
                .takes_value(true))
            .arg(
                Arg::new("namespace")
                    .short('n')
                    .long("namespace")
                    .default_value("default")
                    .required(false)
                    .takes_value(true),
            )
            .arg(Arg::new("generate")
                .help("Automatically generate a signing key for this agent and store it in the configured key store")
                .required_unless_present_any(vec!["publickey", "privatekey"])
                .short('g')
                .long("generate")
                .takes_value(false),
            )
            .arg(
                Arg::new("publickey")
                    .help("Import the public key at this location to the configured key store")
                    .short('p')
                    .long("publickey")
                    .value_hint(ValueHint::FilePath)
                    .required_unless_present_any(vec!["generate","privatekey"])
                    .takes_value(true),
            )
            .arg(
                Arg::new("privatekey")
                    .help("Import the private key at the specified path to the configured key store, ensure you have configured the key store to be in an appropriate location")
                    .short('k')
                    .long("privatekey")
                    .required_unless_present_any(vec!["generate","publickey"])
                    .value_hint(ValueHint::FilePath)
                    .required(false)
                    .takes_value(true),
            ))
        .subcommand(Command::new("use")
            .about("Make the specified agent the context for activities and entities")
            .arg(Arg::new("id")
                .help("A valid chronicle agent IRI")
                .required(true)
                .takes_value(true))
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
                external_id: name_from::<AgentId>(matches, "external_id", "id")?,
                namespace: namespace_from(matches)?,
                attributes: attributes_from(matches, &self.agent.external_id, &self.attributes)?,
            })));
        }
        if let Some(matches) = matches.subcommand_matches("register-key") {
            let registration = {
                if matches.contains_id("generate") {
                    KeyRegistration::Generate
                } else if matches.contains_id("privatekey") {
                    KeyRegistration::ImportSigning(KeyImport::FromPath {
                        path: matches.get_one::<PathBuf>("privatekey").unwrap().to_owned(),
                    })
                } else {
                    KeyRegistration::ImportVerifying(KeyImport::FromPath {
                        path: matches.get_one::<PathBuf>("privatekey").unwrap().to_owned(),
                    })
                }
            };
            return Ok(Some(ApiCommand::Agent(AgentCommand::RegisterKey {
                id: id_from(matches, "id")?,
                namespace: namespace_from(matches)?,
                registration,
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
            define_about: format!("Define an activity of type {} with the given external_id or IRI, re-definition with different attribute values is not allowed", activity.as_type_name()),
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
                                    .args(&["external_id","id"])
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
                external_id: name_from::<ActivityId>(matches, "external_id", "id")?,
                namespace: namespace_from(matches)?,
                attributes: attributes_from(matches, &self.activity.external_id, &self.attributes)?,
            })));
        }

        if let Some(matches) = matches.subcommand_matches("start") {
            return Ok(Some(ApiCommand::Activity(ActivityCommand::Start {
                id: id_from(matches, "id")?,
                namespace: namespace_from(matches)?,
                time: matches
                    .get_one::<String>("time")
                    .map(|t| t.parse())
                    .transpose()?,
                agent: id_from_option(matches, "agent_id")?,
            })));
        };

        if let Some(matches) = matches.subcommand_matches("end") {
            return Ok(Some(ApiCommand::Activity(ActivityCommand::End {
                id: id_from(matches, "id")?,
                namespace: namespace_from(matches)?,
                time: matches
                    .get_one::<String>("time")
                    .map(|t| t.parse())
                    .transpose()?,
                agent: id_from_option(matches, "agent_id")?,
            })));
        };

        if let Some(matches) = matches.subcommand_matches("instant") {
            return Ok(Some(ApiCommand::Activity(ActivityCommand::Instant {
                id: id_from(matches, "id")?,
                namespace: namespace_from(matches)?,
                time: matches
                    .get_one::<String>("time")
                    .map(|t| t.parse())
                    .transpose()?,
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
            define_about: format!("Define an entity of type {} with the given external_id or IRI, re-definition with different attribute values is not allowed", entity.as_type_name()),
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
                                    .args(&["external_id","id"])
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
            .subcommand(
                Command::new("attach")
                    .about("Sign the input file and record it against the entity")
                    .arg(
                        Arg::new("entity_id")
                            .help("A valid chronicle entity IRI")
                            .takes_value(true),
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
                        Arg::new("file")
                            .short('f')
                            .help("A path to the file to be signed and attached")
                            .long("file")
                            .value_hint(ValueHint::FilePath)
                            .required(true)
                            .takes_value(true),
                    )
                    .arg(
                        Arg::new("locator")
                            .short('l')
                            .long("locator")
                            .help("A url or other way of identifying the attachment")
                            .value_hint(ValueHint::Url)
                            .required(false)
                            .takes_value(true),
                    )
                    .arg(
                        Arg::new("agent_id")
                            .help("A valid chronicle agent IRI")
                            .takes_value(true),
                    )
                    .group(
                        ArgGroup::new("identifier")
                            .args(&["agent_id", "entity_id"])
                            .required(true),
                    ),
            )
    }

    fn matches(&self, matches: &ArgMatches) -> Result<Option<ApiCommand>, CliError> {
        if let Some(matches) = matches.subcommand_matches("define") {
            return Ok(Some(ApiCommand::Entity(EntityCommand::Create {
                external_id: name_from::<EntityId>(matches, "external_id", "id")?,
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
                    }),
                activity: id_from_option(matches, "activity_id")?,
                used_entity: id_from(matches, "used_entity_id")?,
            })));
        }

        if let Some(matches) = matches.subcommand_matches("attach") {
            return Ok(Some(ApiCommand::Entity(EntityCommand::Attach {
                id: id_from(matches, "entity_id")?,
                namespace: namespace_from(matches)?,
                file: PathOrFile::Path(matches.get_one::<PathBuf>("file").unwrap().to_owned()),
                agent: id_from_option(matches, "agent_id")?,
                locator: matches.get_one::<String>("locator").map(|x| x.to_owned()),
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
    short_version: String,
    long_version: String,
}

impl From<ChronicleDomainDef> for CliModel {
    fn from(val: ChronicleDomainDef) -> Self {
        let short_version = env!("CARGO_PKG_VERSION").to_string();
        let long_version = format!(
            "{} ({})",
            short_version,
            if cfg!(feature = "inmem") {
                "in memory"
            } else {
                "sawtooth"
            }
        );
        CliModel {
            agents: val.agents.iter().map(AgentCliModel::new).collect(),
            entities: val.entities.iter().map(EntityCliModel::new).collect(),
            activities: val.activities.iter().map(ActivityCliModel::new).collect(),
            domain: val,
            short_version,
            long_version,
        }
    }
}

impl SubCommand for CliModel {
    fn as_cmd(&self) -> Command {
        let mut app = Command::new("chronicle")
            .version(self.short_version.as_str())
            .long_version(self.long_version.as_str())
            .author("Blockchain technology partners")
            .about("Write and query provenance data to distributed ledgers")
            .arg(
                Arg::new("config")
                    .short('c')
                    .long("config")
                    .value_name("config")
                    .value_hint(ValueHint::FilePath)
                    .default_value("~/.chronicle/config.toml")
                    .help("Sets a custom config file")
                    .takes_value(true),
            )
            .arg(
                Arg::new("instrument")
                    .short('i')
                    .long("instrument")
                    .value_name("instrument")
                    .takes_value(true)
                    .value_hint(ValueHint::Url)
                    .help("Instrument using RUST_LOG environment"),
            )
            .arg(Arg::new("console-logging").long("console-logging")
                .takes_value(true)
                .possible_values(["pretty","json"])
                .default_value("pretty")
                .help(
                    "Instrument using RUST_LOG environment, writing in either human readable format or structured json to stdio",
             ))
             .arg(
                Arg::new("embedded-database")
                    .long("embedded-database")
                    .help("use an embedded PostgreSQL")
                    .conflicts_with("remote-database")
                    // see https://github.com/clap-rs/clap/issues/1605 - fixed in v3.x - then use ArgGroup or,
                    // .conflicts_with_all(&["remote-database", "database-host", "database-port", "database-username", "database-name"])
            )
            .arg(
                Arg::new("remote-database")
                    .long("remote-database")
                    .help("connect to a provided PostgreSQL")
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
                    .help("name of the database")
                    .default_value("chronicle"),
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
                Command::new("serve-graphql")
                    .about("Start a graphql server")
                    .arg(
                        Arg::new("open")
                            .long("open")
                            .required(false)
                            .takes_value(false)
                            .help("Open apollo studio sandbox"),
                    )
                    .arg(
                        Arg::new("interface")
                            .long("interface")
                            .required(false)
                            .takes_value(true)
                            .default_value("127.0.0.1:9982")
                            .help("The graphql server address (default 127.0.0.1:9982)"),
                    ),
            )
            .subcommand(Command::new("verify-keystore").about("Initialize and verify keystore, then exit"));

        for agent in self.agents.iter() {
            app = app.subcommand(agent.as_cmd());
        }
        for activity in self.activities.iter() {
            app = app.subcommand(activity.as_cmd());
        }
        for entity in self.entities.iter() {
            app = app.subcommand(entity.as_cmd());
        }

        #[cfg(not(feature = "inmem"))]
        {
            app.arg(
                Arg::new("sawtooth")
                    .long("sawtooth")
                    .value_name("sawtooth")
                    .value_hint(ValueHint::Url)
                    .default_value("tcp://localhost:4004")
                    .help("Sets sawtooth validator address")
                    .takes_value(true),
            )
        }
        #[cfg(feature = "inmem")]
        {
            app
        }
    }

    /// Iterate our possible subcommands via model and short circuit with the first one that matches
    fn matches(&self, matches: &ArgMatches) -> Result<Option<ApiCommand>, CliError> {
        for (agent, matches) in self.agents.iter().filter_map(|agent| {
            matches
                .subcommand_matches(&agent.external_id)
                .map(|matches| (agent, matches))
        }) {
            if let Some(cmd) = agent.matches(matches)? {
                return Ok(Some(cmd));
            }
        }
        for (entity, matches) in self.entities.iter().filter_map(|entity| {
            matches
                .subcommand_matches(&entity.external_id)
                .map(|matches| (entity, matches))
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
