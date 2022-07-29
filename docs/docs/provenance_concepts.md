# Provenance concepts

Chronicle uses the [W3C Provenance Ontology](https://www.w3.org/TR/prov-o/) as the basis for provenance modelling. We present here a summary of the concepts used and their corresponding mutations and graphql query structure.

## Agent

> An agent is something that bears some form of responsibility for an activity taking place, for the existence of an entity, or for another agent's activity.

Agents in chronicle can be subtyped and contain attributes, specified [by the domain model agents section](./domain_modelling.md#agent). Agents can be recorded using [the typed agent mutations](./recording_provenance.md#agent) or can be left [untyped](./untyped_chronicle.md#creating-an-agent-in-untyped-chronicle).


## Activity

> An activity is something that occurs over a period of time and acts upon or with entities; it may include consuming, processing, transforming, modifying, relocating, using, or generating entities. Just as entities cover a broad range of notions, activities can cover a broad range of notions: information processing activities may for example move, copy, or duplicate digital entities; physical activities can include driving a car between two locations or printing a book.

Activities in Chronicle can be subtyped and contain attributes, specified [by the domain model section](./domain_modelling.md#activity). Agents can be recorded using [the typed activity mutations](./recording_provenance.md#activity) or can be left [untyped](./untyped_chronicle.md#creating-an-activity-in-untyped-chronicle).


## Entity

> In PROV, things we want to describe the provenance of are called entities and have some fixed aspects. The term "things" encompasses a broad diversity of notions, including digital objects such as a file or web page, physical things such as a mountain, a building, a printed book, or a car as well as abstract concepts and ideas.
> An entity is a physical, digital, conceptual, or other kind of thing with some fixed aspects; entities may be real or imaginary.

Entities in Chronicle can be subtyped and contain attributes, specified [by the domain model section](./domain_modelling.md#entity). Agents can be recorded using [the typed activity mutations](./recording_provenance.md#entity) or can be left [untyped](./untyped_chronicle.md#creating-an-entity-in-untyped-chronicle).

