# Chronicle

A tool for recording and querying provenance onto distributed ledgers.

## Provenance
![PROV-O](doc/images/starting-points.svg)
Chronicle implements the [PROV-O starting point terms](https://www.w3.org/TR/2013/REC-prov-o-20130430/#description-starting-point-terms), encoding them in [JSON-LD compact form](https://json-ld.org/spec/latest/json-ld-api/#compaction) onto a backend ledger - currently sawtooth or an in memory stub for testing purposes.

## Deployment

Chronicle is a self contained binary executable that can be used as an ephemeral command line interface for provenance recording and interrogation or as a grapql server to provide an interface for higher level services. It embeds sqlite for local syncronisation with a backend ledger and is capable of basic key management using a file system.

Chroncicle instances do not share state directly as they have individual data stores, so syncronise via ledger updates. The abstract transaction processor should process an API operation in under a miliscond.

## Transaction processing

Chronicle records provenance by running an abstract deterministic transaction processor both locally and as part of consensus. This transaction model is designed to be infallible - barring infrastructure issues, provenance will always be recorded for any operation that succeeds locally.



