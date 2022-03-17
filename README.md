# Chronicle

A tool for recording and querying provenance onto distributed ledgers.

## Provenance
![PROV-O](doc/images/starting-points.svg)
Chronicle implements the [PROV-O starting point terms](https://www.w3.org/TR/2013/REC-prov-o-20130430/#description-starting-point-terms), encoding them in [JSON-LD compact form](https://json-ld.org/spec/latest/json-ld-api/#compaction) onto a backend ledger - currently sawtooth or an in memory stub for testing purposes.


## Chronicle vocabulary

As well as core prov, we have additional vocabulary to describe cryptographic proceses, identities and prevent conflict between multiple domains on-chain using namespaces.

### chronicle:Identity

Represents a cryptographic identity for an agent, supporting a single current signing identity via chronicle:hasIdentity and historical identities via chronicle:hadIdentity.

### chronicle:publicKey

A value containing a hex encoded ecdsa public key.

Domain: Identity

### chronicle:hasIdentity

The current cryptographic identity of a prov:Agent.

Domain: prov:Agent
Range: chronicle:Identity

### chronicle:hadIdentity

A historical cryptographic identity for a prov:Agent.

Domain: prov:Agent
Range: chronicle:Identity

### chronicle:Namespace

An IRI containing a name and uuid part, used for disambigation. Discrete chronicle instances that with to work on the same namespace must share the uuid part.

### chronicle:hasNamespace

Allows disambiguation of potentially conflicting IRIs on the same chain, used as a component for address generation.

Domain: All prov and chronicle resouces

### chronicle:Attachment

A resource describing an external resource, linked to a prov:Entity and signed by an agent.

### chronicle:hasAttachment

The current attachment for a prov:Entity.

Domain: prov:Entity
Range: chronicle:Attachment

### chronicle:hadAttachment

A historical attachment for a prov:Entity.

Domain: prov:Entity
Range: chronicle:Attachment

### chronicle:entitySignature

A hex encoded ecdsa signature for the resource represented by the attachment.

Domain: Attachment

### chronicle:entityLocator

An arbitrary value describing the attachment, likely an external IRI

Domain: Attachment

### chronicle:signedAtTime

The date / time when the attachment was created

Domain: chronicle:Attachment

### chronicle:signedBy

The chronicle:Identity (and by inference, prov:Agent) that signed the attachment

Domain: chronicle:Attachment
Range: chronicle:Identity

## Deployment

Chronicle is a self contained binary executable that can be used as an ephemeral command line interface for provenance recording and interrogation or as a grapql server to provide an interface for higher level services. It embeds sqlite for local syncronisation with a backend ledger and is capable of basic key management using a file system.

Chroncicle instances do not share state directly as they have individual data stores, so syncronise via ledger updates. The abstract transaction processor should process an API operation in under a miliscond.

## Transaction processing

Chronicle records provenance by running an abstract deterministic transaction processor both locally and as part of consensus. This transaction model is designed to be infallible - barring infrastructure issues, provenance will always be recorded for any operation that succeeds locally.



