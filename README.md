# Chronicle

A tool for recording and querying provenance data stored on distributed ledgers.

## Getting Started

We recommend you start by exploring the Chronicle examples repo, and trying
to build and experiment with one of the runnable application domains there: <https://github.com/btpworks/chronicle-examples>

## Documentation

- Examples: <https://examples.btp.works/>
- Chronicle: <https://docs.btp.works/chronicle/>

## Provenance

Chronicle is built on W3C open standards:

- [PROV-O - The PROV Ontology](https://www.w3.org/TR/prov-o/)
- [JSON-LD](https://www.w3.org/TR/json-ld11/)

Chronicle implements the PROV-O
[starting point terms](https://www.w3.org/TR/2013/REC-prov-o-20130430/#description-starting-point-terms)
shown below, encoding them using the JSON-LD
[compaction algorithm](https://json-ld.org/spec/latest/json-ld-api/#compaction)
onto a backend ledger - currently Hyperledger Sawtooth - or an in-memory stub
for testing purposes.

![PROV-O](https://www.w3.org/TR/prov-o/diagrams/starting-points.svg)

Chronicle extends the core PROV-O vocabulary as described
[here](docs/chronicle_vocabulary.md).

## Deployment

Chronicle is a self contained binary executable that can be used as an ephemeral
command line interface for provenance recording and interrogation or as a
GraphQL server to provide an interface for higher level services. It embeds
PostgreSQL for local synchronization with a backend ledger and is capable of
basic key management using a file system.

Chronicle instances have individual data stores, so they do not share state
directly and synchronize via ledger updates. The abstract transaction processor
should process an API operation in under a millisecond.

## Transaction Processing

Chronicle records provenance by running a deterministic transaction
processor both locally and as part of consensus. Local execution ensures that
duplicated provenance will not waste resources in consensus and that most
contradictions will also be caught early. Provenance will be sent to a validator
and recorded on chain unless it contradicts previous provenance.

## Call For Open-Source Contributions

You can read all about our rationale for open sourcing Chronicle on Medium where
our Chief Strategy Officer, Csilla Zsigri, published this article
[Chronicle: You Say Provenance, We Say Open Source](https://medium.com/btpworks/chronicle-you-say-provenance-we-say-open-source-737c506dc9c0).

We will be publishing a roadmap shortly at which point all developers are
invited to contribute to our efforts to make assets trustworthy.

You can participate in the following ways:

1. Join our [Slack group](https://communityinviter.com/apps/chronicleworks/joinus)
   to chat
1. Submit an issue or PR on GitHub

## License

See the [LICENSE](LICENSE) file.
