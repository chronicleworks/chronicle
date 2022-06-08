use clap::*;

use crate::codegen::{
    ActivityDef, AgentDef, AttributeDef, ChronicleDomainDef, CliName, EntityDef, TypeName,
};

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
    pub name: String,
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
            name: agent.as_cli_name(),
            about: format!("Operations on {} agents", agent.as_type_name()),
            define_about: format!("Define an agent of type {} with the given name or IRI, re-defintion with different attribute values is not allowed", agent.as_type_name())
        }
    }

    pub fn as_cmd(&self) -> Command {
        let cmd = Command::new(&*self.name).about(&*self.about);

        let mut define = Command::new("define")
                        .about(&*self.define_about)
                        .arg(Arg::new("name")
                            .help("An externally meaningful identifier for the agent, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("id")
                            .help("A valid chronicle agent IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["name","id"])
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
                                .arg(Arg::new("name")
                                    .help("An externally meaningful identifier for the agent, e.g. a URI or relational id")
                                    .takes_value(true))
                                .arg(Arg::new("id")
                                    .help("A valid chronicle agent IRI")
                                    .takes_value(true))
                                .group(ArgGroup::new("identifier")
                                            .args(&["name","id"])
                                            .required(true))
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
                                        .help("Import the private key at the specifed path to the configured key store, ensure you have configured the key store to be in an appropriate location")
                                        .short('k')
                                        .long("privatekey")
                                        .required_unless_present_any(vec!["generate","publickey"])
                                        .value_hint(ValueHint::FilePath)
                                        .required(false)
                                        .takes_value(true),
                                ))
                            .subcommand(Command::new("use")
                                .about("Make the specified agent the context for activities and entities")
                                .arg(Arg::new("name")
                                    .help("An externally meaningful identifier for the agent, e.g. a URI or relational id")
                                    .takes_value(true))
                                .arg(Arg::new("id")
                                    .help("A valid chronicle agent IRI")
                                    .takes_value(true))
                                .group(ArgGroup::new("identifier")
                                            .args(&["name","id"])
                                            .required(true))
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
}

pub struct ActivityCliModel {
    pub activity: ActivityDef,
    pub attributes: Vec<AttributeCliModel>,
    pub about: String,
    pub define_about: String,
    pub name: String,
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
                                name: activity.as_cli_name(),
                                about: format!("Operations on {} activities", activity.as_type_name()),
                                define_about: format!("Define an activity of type {} with the given name or IRI, re-defintion with different attribute values is not allowed", activity.as_type_name()),
                            }
    }

    fn as_cmd(&self) -> Command {
        let cmd = Command::new(&*self.name).about(&*self.about);

        let mut define =
                                        Command::new("define")
                                            .about(&*self.define_about)
                                            .arg(Arg::new("name")
                                                .help("An externally meaningful identifier for the activity, e.g. a URI or relational id")
                                                .takes_value(true))
                                            .arg(Arg::new("id")
                                                .help("A valid chronicle activity IRI")
                                                .takes_value(true))
                                            .group(ArgGroup::new("identifier")
                                                        .args(&["name","id"])
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
                                            .about("Record this activity as started at the current time")

                                            .arg(Arg::new("name")
                                        .help("An externally meaningful identifier for the activity, e.g. a URI or relational id")
                                        .takes_value(true))
                                    .arg(Arg::new("id")
                                        .help("A valid chronicle activity IRI")
                                        .takes_value(true))
                                    .group(ArgGroup::new("identifier")
                                                .args(&["name","id"])
                                                .required(true))
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
                                    Command::new("end")
                                        .about("Record this activity as ended at the current time")
                                        .arg(Arg::new("name")
                            .help("An externally meaningful identifier for the activity, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("id")
                            .help("A valid chronicle activity IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["name","id"])
                                    .required(true))
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
                    Command::new("use")
                        .about("Record this activity as having used the specified entity, creating it if required")
                        .arg(Arg::new("entity_name")
                            .help("An externally meaningful identifier for the entity, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("entity id")
                            .help("A valid chronicle entity IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["entity_name","entity_id"])
                                    .required(true))
                        .arg(Arg::new("activity_name")
                            .help("An externally meaningful identifier for the activity, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("activity id")
                            .help("A valid chronicle activity IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["activity_name","activity_id"])
                                    .required(true))
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
                            .arg(Arg::new("entity_name")
                            .help("An externally meaningful identifier for the entity, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("entity id")
                            .help("A valid chronicle entity IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["entity_name","entity_id"])
                                    .required(true))
                        .arg(Arg::new("activity_name")
                            .help("An externally meaningful identifier for the activity, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("activity id")
                            .help("A valid chronicle activity IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["activity_name","activity_id"])
                                    .required(true))
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
}

pub struct EntityCliModel {
    pub entity: EntityDef,
    pub attributes: Vec<AttributeCliModel>,
    pub about: String,
    pub define_about: String,
    pub name: String,
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
            name: entity.as_cli_name(),
            about: format!("Operations on {} entities", entity.as_type_name()),
            define_about: format!("Define an entity of type {} with the given name or IRI, re-defintion with different attribute values is not allowed", entity.as_type_name()),
        }
    }

    pub fn as_cmd(&self) -> Command {
        let cmd = Command::new(&self.name).about(&*self.about);

        let mut define =
                    Command::new("define")
                        .about(&*self.define_about)
                        .arg(Arg::new("name")
                           .help("An externally meaningful identifier for the entity, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("id")
                            .help("A valid chronicle entity IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["name","id"])
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

        cmd.subcommand(
                                        Command::new("attach")
                                            .about("Sign the input file and record it against the entity")
                                            .arg(Arg::new("entity_name")
                            .help("An externally meaningful identifier for the activity, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("entity_id")
                            .help("A valid chronicle activity IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["entity_name","entity_id"])
                                    .required(true))
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

                        .arg(Arg::new("agent_name")
                            .help("An externally meaningful identifier for the activity, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("agent_id")
                            .help("A valid chronicle activity IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["agent_name","entity_id"])
                                    .required(false))
                        .arg(Arg::new("activity_name")
                            .help("An externally meaningful identifier for the activity, e.g. a URI or relational id")
                            .takes_value(true))
                        .arg(Arg::new("activity_id")
                            .help("A valid chronicle activity IRI")
                            .takes_value(true))
                        .group(ArgGroup::new("identifier")
                                    .args(&["activity_name","entity_id"])
                                    .required(true))
                )
    }
}

pub struct CliModel {
    pub domain: ChronicleDomainDef,
    pub agents: Vec<AgentCliModel>,
    pub entities: Vec<EntityCliModel>,
    pub activities: Vec<ActivityCliModel>,
}

impl From<ChronicleDomainDef> for CliModel {
    fn from(val: ChronicleDomainDef) -> Self {
        CliModel {
            agents: val.agents.iter().map(AgentCliModel::new).collect(),
            entities: val.entities.iter().map(EntityCliModel::new).collect(),
            activities: val.activities.iter().map(ActivityCliModel::new).collect(),
            domain: val,
        }
    }
}

impl CliModel {
    pub fn as_cmd(&self) -> Command {
        let mut app = Command::new("chronicle")
            .version("1.0")
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
                Arg::new("completions")
                    .long("completions")
                    .value_name("completions")
                    .help("Generate shell completions and exit"),
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
            .arg(Arg::new("console-logging").long("console-logging").help(
                "Instrument using RUST_LOG environment, writing in human readable format to stdio",
            ))
            .arg(
                Arg::new("export-schema")
                    .long("export-schema")
                    .takes_value(false)
                    .help("Print SDL and exit"),
            )
            .arg(
                Arg::new("gql")
                    .long("gql")
                    .required(false)
                    .takes_value(false)
                    .help("Start the graphql server"),
            )
            .arg(
                Arg::new("open")
                    .long("open")
                    .required(false)
                    .takes_value(false)
                    .help("Open apollo studio sandbox"),
            )
            .arg(
                Arg::new("gql-interface")
                    .long("gql-interface")
                    .required(false)
                    .takes_value(true)
                    .default_value("127.0.0.1:9982")
                    .help("The graphql server address"),
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
}

pub fn cli(domain: ChronicleDomainDef) -> CliModel {
    CliModel::from(domain)
}
