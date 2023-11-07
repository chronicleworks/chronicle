use lazy_static::lazy_static;
use serde_json::{json, Value};

lazy_static! {
	pub static ref PROV: Value = json!({
		"@version": 1.1,
		"prov": "http://www.w3.org/ns/prov#",
		"provext": "https://openprovenance.org/ns/provext#",
		"xsd": "http://www.w3.org/2001/XMLSchema#",
		"rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
		"chronicle":"http://chronicle.works/chronicle/ns#",
		"chronicle_op":"http://chronicle.works/chronicleoperations/ns#",
		"entity": {
			"@id": "prov:entity",
			"@type": "@id"
		},

		"activity": {
			"@id": "prov:activity",
			"@type": "@id"
		},

		"agent": {
			"@id": "prov:agent",
			"@type": "@id"
		},

		"externalId" : {
		   "@id": "chronicle:externalId",
		},

		"namespace": {
			"@id": "chronicle:hasNamespace",
			"@type": "@id"
		},

		"publicKey": {
			"@id": "chronicle:hasPublicKey",
		},

		"identity": {
			"@id": "chronicle:hasIdentity",
			"@type" : "@id",
		},

		"previousIdentities": {
			"@id": "chronicle:hadIdentity",
			"@type" : "@id",
			"@container": "@set"
		},

		"wasAssociatedWith": {
			"@id": "prov:wasAssociatedWith",
			"@type" : "@id",
			"@container": "@set"
		},

		"wasAttributedTo": {
			"@id": "prov:wasAttributedTo",
			"@type" : "@id",
			"@container": "@set"
		},

		"wasDerivedFrom": {
			"@id": "prov:wasDerivedFrom",
			"@type" : "@id",
			"@container": "@set"
		},

		"hadPrimarySource": {
			"@id": "prov:hadPrimarySource",
			"@type" : "@id",
			"@container": "@set"
		},

		"actedOnBehalfOf": {
			"@id": "prov:actedOnBehalfOf",
			"@type" : "@id",
			"@container": "@set"
		},

		"wasQuotedFrom": {
			"@id": "prov:wasQuotedFrom",
			"@type" : "@id",
			"@container": "@set"
		},

		"wasRevisionOf": {
			"@id": "prov:wasRevisionOf",
			"@type" : "@id",
			"@container": "@set"
		},

		"used": {
			"@id": "prov:used",
			"@type" : "@id",
			"@container": "@set"
		},

		"wasGeneratedBy": {
			"@id": "prov:wasGeneratedBy",
			"@type" : "@id",
			"@container": "@set"
		},

		"startTime": {
			 "@id": "prov:startedAtTime",
		},

		"endTime": {
			 "@id": "prov:endedAtTime",
		},
		"value": {
			"@id": "chronicle:Value",
			"@type" : "@json",
		},
	});
}
