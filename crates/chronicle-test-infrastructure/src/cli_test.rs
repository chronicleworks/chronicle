use api::commands::ApiCommand;
use chronicle::{
	bootstrap::{CliModel, SubCommand},
	codegen::ChronicleDomainDef,
	PrimitiveType,
};

use common::{
	identity::AuthId,
	prov::{json_ld::ToJson, ActivityId, AgentId, ChronicleIri, EntityId, ProvModel},
};

use crate::substitutes::test_api;

fn get_api_cmd(command_line: &str) -> ApiCommand {
	let cli = test_cli_model();
	let matches = cli.as_cmd().get_matches_from(command_line.split_whitespace());
	cli.matches(&matches).unwrap().unwrap()
}

async fn parse_and_execute(command_line: &str, cli: CliModel) -> Box<ProvModel> {
	let mut api = test_api().await;

	let matches = cli.as_cmd().get_matches_from(command_line.split_whitespace());

	let cmd = cli.matches(&matches).unwrap().unwrap();

	let identity = AuthId::chronicle();

	api.dispatch(cmd, identity).await.unwrap().unwrap().0
}

fn test_cli_model() -> CliModel {
	CliModel::from(
		ChronicleDomainDef::build("test")
			.with_attribute_type("testString", None, PrimitiveType::String)
			.unwrap()
			.with_attribute_type("testBool", None, PrimitiveType::Bool)
			.unwrap()
			.with_attribute_type("testInt", None, PrimitiveType::Int)
			.unwrap()
			.with_attribute_type("testJSON", None, PrimitiveType::JSON)
			.unwrap()
			.with_activity("testActivity", None, |b| {
				b.with_attribute("testString")
					.unwrap()
					.with_attribute("testBool")
					.unwrap()
					.with_attribute("testInt")
			})
			.unwrap()
			.with_agent("testAgent", None, |b| {
				b.with_attribute("testString")
					.unwrap()
					.with_attribute("testBool")
					.unwrap()
					.with_attribute("testInt")
			})
			.unwrap()
			.with_entity("testEntity", None, |b| {
				b.with_attribute("testString")
					.unwrap()
					.with_attribute("testBool")
					.unwrap()
					.with_attribute("testInt")
			})
			.unwrap()
			.build(),
	)
}

#[tokio::test]
async fn agent_define() {
	let command_line = r#"chronicle test-agent-agent define test_agent --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:agent:test_agent",
       "@type": [
         "prov:Agent",
         "chronicle:domaintype:testAgent"
       ],
       "externalId": "test_agent",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {
         "TestBool": false,
         "TestInt": 23,
         "TestString": "test"
       }
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn agent_define_id() {
	let id = ChronicleIri::from(common::prov::AgentId::from_external_id("test_agent"));
	let command_line = format!(
		r#"chronicle test-agent-agent define --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns --id {id} "#
	);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(&command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:agent:test_agent",
       "@type": [
         "prov:Agent",
         "chronicle:domaintype:testAgent"
       ],
       "externalId": "test_agent",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {
         "TestBool": false,
         "TestInt": 23,
         "TestString": "test"
       }
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn agent_use() {
	let mut api = test_api().await;

	// note, if you don't supply all three types of attribute this won't run
	let command_line = r#"chronicle test-agent-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 23 "#;

	let cmd = get_api_cmd(command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:agent:testagent",
       "@type": [
         "prov:Agent",
         "chronicle:domaintype:testAgent"
       ],
       "externalId": "testagent",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {
         "TestBool": true,
         "TestInt": 23,
         "TestString": "test"
       }
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);

	let id = AgentId::from_external_id("testagent");

	let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
	let cmd = get_api_cmd(&command_line);

	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	let id = ActivityId::from_external_id("testactivity");
	let command_line = format!(
		r#"chronicle test-activity-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
	);
	let cmd = get_api_cmd(&command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": "prov:Activity",
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "prov:qualifiedAssociation": {
         "@id": "chronicle:association:testagent:testactivity:role="
       },
       "startTime": "2014-07-08T09:10:11+00:00",
       "value": {},
       "wasAssociatedWith": [
         "chronicle:agent:testagent"
       ]
     },
     {
       "@id": "chronicle:association:testagent:testactivity:role=",
       "@type": "prov:Association",
       "agent": "chronicle:agent:testagent",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "prov:hadActivity": {
         "@id": "chronicle:activity:testactivity"
       }
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn entity_define() {
	let command_line = r#"chronicle test-entity-entity define test_entity --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;
	let _delta = parse_and_execute(command_line, test_cli_model());

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:test_entity",
       "@type": [
         "prov:Entity",
         "chronicle:domaintype:testEntity"
       ],
       "externalId": "test_entity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {
         "TestBool": false,
         "TestInt": 23,
         "TestString": "test"
       }
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn entity_define_id() {
	let id = ChronicleIri::from(common::prov::EntityId::from_external_id("test_entity"));
	let command_line = format!(
		r#"chronicle test-entity-entity define --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns --id {id} "#
	);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(&command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:test_entity",
       "@type": [
         "prov:Entity",
         "chronicle:domaintype:testEntity"
       ],
       "externalId": "test_entity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {
         "TestBool": false,
         "TestInt": 23,
         "TestString": "test"
       }
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn entity_derive_abstract() {
	let mut api = test_api().await;

	let generated_entity_id = EntityId::from_external_id("testgeneratedentity");
	let used_entity_id = EntityId::from_external_id("testusedentity");

	let command_line = format!(
		r#"chronicle test-entity-entity derive {generated_entity_id} {used_entity_id} --namespace testns "#
	);
	let cmd = get_api_cmd(&command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:testgeneratedentity",
       "@type": "prov:Entity",
       "externalId": "testgeneratedentity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {},
       "wasDerivedFrom": [
         "chronicle:entity:testusedentity"
       ]
     },
     {
       "@id": "chronicle:entity:testusedentity",
       "@type": "prov:Entity",
       "externalId": "testusedentity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {}
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn entity_derive_primary_source() {
	let mut api = test_api().await;

	let generated_entity_id = EntityId::from_external_id("testgeneratedentity");
	let used_entity_id = EntityId::from_external_id("testusedentity");

	let command_line = format!(
		r#"chronicle test-entity-entity derive {generated_entity_id} {used_entity_id} --namespace testns --subtype primary-source "#
	);
	let cmd = get_api_cmd(&command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:testgeneratedentity",
       "@type": "prov:Entity",
       "externalId": "testgeneratedentity",
       "hadPrimarySource": [
         "chronicle:entity:testusedentity"
       ],
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {}
     },
     {
       "@id": "chronicle:entity:testusedentity",
       "@type": "prov:Entity",
       "externalId": "testusedentity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {}
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn entity_derive_revision() {
	let mut api = test_api().await;

	let generated_entity_id = EntityId::from_external_id("testgeneratedentity");
	let used_entity_id = EntityId::from_external_id("testusedentity");

	let command_line = format!(
		r#"chronicle test-entity-entity derive {generated_entity_id} {used_entity_id} --namespace testns --subtype revision "#
	);
	let cmd = get_api_cmd(&command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:testgeneratedentity",
       "@type": "prov:Entity",
       "externalId": "testgeneratedentity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {},
       "wasRevisionOf": [
         "chronicle:entity:testusedentity"
       ]
     },
     {
       "@id": "chronicle:entity:testusedentity",
       "@type": "prov:Entity",
       "externalId": "testusedentity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {}
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn entity_derive_quotation() {
	let mut api = test_api().await;

	let generated_entity_id = EntityId::from_external_id("testgeneratedentity");
	let used_entity_id = EntityId::from_external_id("testusedentity");

	let command_line = format!(
		r#"chronicle test-entity-entity derive {generated_entity_id} {used_entity_id} --namespace testns --subtype quotation "#
	);
	let cmd = get_api_cmd(&command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:entity:testgeneratedentity",
       "@type": "prov:Entity",
       "externalId": "testgeneratedentity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {},
       "wasQuotedFrom": [
         "chronicle:entity:testusedentity"
       ]
     },
     {
       "@id": "chronicle:entity:testusedentity",
       "@type": "prov:Entity",
       "externalId": "testusedentity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {}
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn activity_define() {
	let command_line = r#"chronicle test-activity-activity define test_activity --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns "#;

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:test_activity",
       "@type": [
         "prov:Activity",
         "chronicle:domaintype:testActivity"
       ],
       "externalId": "test_activity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {
         "TestBool": false,
         "TestInt": 23,
         "TestString": "test"
       }
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn activity_define_id() {
	let id = ChronicleIri::from(common::prov::ActivityId::from_external_id("test_activity"));
	let command_line = format!(
		r#"chronicle test-activity-activity define --test-bool-attr false --test-string-attr "test" --test-int-attr 23 --namespace testns --id {id} "#
	);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &parse_and_execute(&command_line, test_cli_model()).await.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:test_activity",
       "@type": [
         "prov:Activity",
         "chronicle:domaintype:testActivity"
       ],
       "externalId": "test_activity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {
         "TestBool": false,
         "TestInt": 23,
         "TestString": "test"
       }
     },
     {
       "@id": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "@type": "chronicle:Namespace",
       "externalId": "testns"
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn activity_start() {
	let mut api = test_api().await;

	let command_line = r#"chronicle test-agent-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
	let cmd = get_api_cmd(command_line);

	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
	let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
	let cmd = get_api_cmd(&command_line);
	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	let id = ChronicleIri::from(ActivityId::from_external_id("testactivity"));
	let command_line = format!(
		r#"chronicle test-activity-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
	);
	let cmd = get_api_cmd(&command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": "prov:Activity",
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "prov:qualifiedAssociation": {
         "@id": "chronicle:association:testagent:testactivity:role="
       },
       "startTime": "2014-07-08T09:10:11+00:00",
       "value": {},
       "wasAssociatedWith": [
         "chronicle:agent:testagent"
       ]
     },
     {
       "@id": "chronicle:association:testagent:testactivity:role=",
       "@type": "prov:Association",
       "agent": "chronicle:agent:testagent",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "prov:hadActivity": {
         "@id": "chronicle:activity:testactivity"
       }
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn activity_end() {
	let mut api = test_api().await;

	let command_line = r#"chronicle test-agent-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
	let cmd = get_api_cmd(command_line);

	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
	let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
	let cmd = get_api_cmd(&command_line);
	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	let id = ChronicleIri::from(ActivityId::from_external_id("testactivity"));
	let command_line = format!(
		r#"chronicle test-activity-activity start {id} --namespace testns --time 2014-07-08T09:10:11Z "#
	);
	let cmd = get_api_cmd(&command_line);
	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	// Should end the last opened activity
	let id = ActivityId::from_external_id("testactivity");
	let command_line = format!(
		r#"chronicle test-activity-activity end --namespace testns --time 2014-08-09T09:10:12Z {id} "#
	);
	let cmd = get_api_cmd(&command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": "prov:Activity",
       "endTime": "2014-08-09T09:10:12+00:00",
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "prov:qualifiedAssociation": {
         "@id": "chronicle:association:testagent:testactivity:role="
       },
       "startTime": "2014-07-08T09:10:11+00:00",
       "value": {},
       "wasAssociatedWith": [
         "chronicle:agent:testagent"
       ]
     },
     {
       "@id": "chronicle:association:testagent:testactivity:role=",
       "@type": "prov:Association",
       "agent": "chronicle:agent:testagent",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "prov:hadActivity": {
         "@id": "chronicle:activity:testactivity"
       }
     }
   ]
 }
 "###);
}

#[tokio::test]
async fn activity_generate() {
	let mut api = test_api().await;

	let command_line = r#"chronicle test-activity-activity define testactivity --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
	let cmd = get_api_cmd(command_line);

	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	let activity_id = ActivityId::from_external_id("testactivity");
	let entity_id = EntityId::from_external_id("testentity");
	let command_line = format!(
		r#"chronicle test-activity-activity generate --namespace testns {entity_id} {activity_id} "#
	);
	let cmd = get_api_cmd(&command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@id": "chronicle:entity:testentity",
   "@type": "prov:Entity",
   "externalId": "testentity",
   "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
   "value": {},
   "wasGeneratedBy": [
     "chronicle:activity:testactivity"
   ]
 }
 "###);
}

#[tokio::test]
async fn activity_use() {
	let mut api = test_api().await;

	let command_line = r#"chronicle test-agent-agent define testagent --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
	let cmd = get_api_cmd(command_line);

	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	let id = ChronicleIri::from(AgentId::from_external_id("testagent"));
	let command_line = format!(r#"chronicle test-agent-agent use --namespace testns {id} "#);
	let cmd = get_api_cmd(&command_line);
	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	let command_line = r#"chronicle test-activity-activity define testactivity --namespace testns --test-string-attr "test" --test-bool-attr true --test-int-attr 40 "#;
	let cmd = get_api_cmd(command_line);
	api.dispatch(cmd, AuthId::chronicle()).await.unwrap();

	let activity_id = ActivityId::from_external_id("testactivity");
	let entity_id = EntityId::from_external_id("testentity");
	let command_line = format!(
		r#"chronicle test-activity-activity use --namespace testns {entity_id} {activity_id} "#
	);

	let cmd = get_api_cmd(&command_line);

	insta::assert_snapshot!(
          serde_json::to_string_pretty(
          &api.dispatch(cmd, AuthId::chronicle()).await.unwrap().unwrap().0.to_json().compact_stable_order().await.unwrap()
        ).unwrap() , @r###"
 {
   "@context": "http://chronicle.works/chr/1.0/c.jsonld",
   "@graph": [
     {
       "@id": "chronicle:activity:testactivity",
       "@type": [
         "prov:Activity",
         "chronicle:domaintype:testActivity"
       ],
       "externalId": "testactivity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "used": [
         "chronicle:entity:testentity"
       ],
       "value": {
         "TestBool": true,
         "TestInt": 40,
         "TestString": "test"
       }
     },
     {
       "@id": "chronicle:entity:testentity",
       "@type": "prov:Entity",
       "externalId": "testentity",
       "namespace": "chronicle:ns:testns:5a0ab5b8-eeb7-4812-9fe3-6dd69bd20cea",
       "value": {}
     }
   ]
 }
 "###);
}
