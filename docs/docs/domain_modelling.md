# Modelling your provenance domain with Chronicle

Here we will present a reference domain that uses all the provence features Chronicle provides and work through modelling it using Chronicle's domain modelling configuration. This should help you understand Chronicle's capabilities and help you model your own problem domain's provenance.

## Reference domain - Medical evidence

This is a toy model of some aspects of evidence based medicine, from an initial `Question` - the area and scope that the organization wishes to research and make guidance on to revisions of a published `Guidance` document.

### Question creation

The `Question` for medical evidence can vary pretty widely, but for the purposes of this example imagine it as something along the lines of "How best to assess and refer patients who have required emergency treatment for Anaphylaxis".

Various actors and processes are involved in the production of the question, but for our purposes we van view it like this:

![file](diagrams/out/question.svg)

The `Question` is then used to inform the `Research` for the production of `Guidance`.

### Research

The `Question` is used to inform one or more searches to a search engine by a researcher, the parameters to the search engine are recorded, and the results are used to create references to `Evidence`.

![file](diagrams/out/evidence.svg)

### Authoring


### Publication


### Revision


## Conceptual design


Provenance is *immutable*. Once you have recorded it there is no way to contradict the provenance you have recorded. When translating your domain to provenance, your activities should be things that have either already take place, or in progress - so choose the past tense.

## Domain model file structure

Chronicle domain models are specified in YAML format with the following structure:

### Name

A string that names your domain, used to coordinate deployments that require multiple namespaces.

``` yaml
name: "chronicle"
```

## Attributes

Attributes are used to assign additional data to the prov terms - `Agent`, `Activity` and `Entity`. They are defined by their name - in the following example `AStringAttribute`, `AnIntegerAttribute` or `ABooleanAttribute`. They are assigned a primitive type of either `String`, `Integer` or `Boolean`. `Integer` is a 32 bit signed integer, `String` and `Boolean` should be self explanatory.

Attribute names should be meaningful to your domain - choose things like 'Title' or 'Description', and they can be re-used between any prov terms.


``` yaml
attributes:
  AStringAttribute:
    type: "String"
  AnIntegerAttribute:
    type: "Integer"
  ABooleanAttribute:
    type: "Boolean"
```

## Agent

> An agent is something that bears some form of responsibility for an activity taking place, for the existence of an entity, or for another agent's activity.

Using Chronicle's domain model definitions an agent can be subtyped and associated with attributes like other provenance terms. In the following example we define two `Agent` subtypes with a name attribute.

``` yaml
agents:
  Author:
   attributes:
      - Name
  Editor:
    attributes:
      - Name
```

## Entity

entities:
  Artwork:
    attributes:
      - Title
  ArtworkDetails:
    attributes:
      - Title
      - Description

## Activity

``` yaml
activities:
  Exhibited:
    attributes:
      - Location
  Created:
    attributes:
      - Title
  Sold:
    attributes:
      - PurchaseValue
      - PurchaseValueCurrency
  Transferred:
    attributes: []
```


## Role

``` yaml
roles:
  - Buyer
  - Seller
  - Broker
  - Editor
  - Creator
```
