use json::{object, JsonValue};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref PROV: JsonValue = object! {
        "@version": 1.1,
        "prov": "http://www.w3.org/ns/prov#",
        "provext": "https://openprovenance.org/ns/provext#",
        "xsd": "http://www.w3.org/2001/XMLSchema#",
        "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
        "rdf": "http://www.w3.org/1999/02/22-rdf-syntax-ns#",
        "chronicle":"http://blockchaintp.com/chronicle/ns#",
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

        "label" : {
           "@id": "rdfs:label",
        },

        "namespace": {
            "@id": "chronicle:hasNamespace",
            "@type": "@id"
        },

        "publicKey": {
            "@id": "chronicle:hasPublicKey",
        },

        "associated": {
            "@id": "prov:wasAssociatedWith",
            "@type" : "@id",
            "@container": "@set"
        },

        "used": {
            "@id": "prov:used",
            "@type" : "@id",
            "@container": "@set"
        },

        "generatedBy": {
            "@id": "prov:wasGeneratedBy",
            "@type" : "@id",
            "@container": "@set"
        },

        "startTime": {
             "@id": "prov:startedAtTime",
        },

        "endTime": {
             "@id": "prov:endedAtTime",
        }
    };
}
