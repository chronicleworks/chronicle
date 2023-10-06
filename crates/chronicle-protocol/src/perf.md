# Chronicle on sawtooth performance

## Environment

4 node sawtooth cluster backed onto a 4x 4X Large AWS node Kubernetes cluster
using our default helm settings. This creates a 4 node sawtooth network with a
validator and transaction processor per AWS node, and a single chronicle service
using persistent Postgres.

## Testing methodology

Sequences of 10000 chronicle mutations (the creation of an activity and the
setting of multiple attributes) were sent from a task, through the Chronicle API
at increasing rates of transactions per-second to find the rate where system
performance begins to break down and discover failure modes.

## Results

The maximum throughput for the whole system is between 50-60 transactions per
second. This throughput is constrained by Sawtooth consensus, and there are ways
this could be significantly raised if Chronicle can increase batch sizes at high throughput
rates. Failure modes over the saturation points are resumable - Sawtooth will
eventually report its queue to be full to Chronicle, and Chronicle will send
errors to connected clients.

No data will be lost due to errors running Chronicle beyond its saturation point

## Future actions

Chronicle 0.8 has some minor performance improvements that could be back ported
to .7. Batching of transactions at high throughput rates would significantly
improve the total throughput of the network - 2 orders of magnitude is possible.
