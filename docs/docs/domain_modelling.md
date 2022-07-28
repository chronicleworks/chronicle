# Modelling your provenance domain with Chronicle

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
