# Chronicle Vocabulary

Chronicle extends core PROV-O by adding vocabulary to describe cryptographic
processes and identities, as well as preventing conflict between multiple Chronicle
domains on-chain by employing namespaces.

## chronicle:Identity

Represents a cryptographic identity for an agent, supporting a single current
signing identity via `chronicle:hasIdentity` and historical identities via
`chronicle:hadIdentity`.

## chronicle:publicKey

A value containing a hex-encoded ECDSA public key.

Domain: `chronicle:Identity`

## chronicle:hasIdentity

The current cryptographic identity of a `prov:Agent`.

Domain: `prov:Agent`  
Range: `chronicle:Identity`

## chronicle:hadIdentity

A historical cryptographic identity for a `prov:Agent`.

Domain: `prov:Agent`  
Range: `chronicle:Identity`

## chronicle:Namespace

An IRI containing an external ID and UUID part, used for disambiguation.
In order to work on the same [namespace](./namespaces.md), discrete Chronicle
instances must share the UUID part.

Domain: All prov and Chronicle resources

## chronicle:hasNamespace

Allows disambiguation of potentially conflicting IRIs on the same chain, used
as a component for address generation.

Domain: All prov and Chronicle resources
