# Connecting to Chronicle

## How to connect to Chronicle running in Kubernetes

When installed with the default values, Chronicle will be accessible via an
internal cluster service. This means that Chronicle will only be accessible
from within the Kubernetes cluster.

This service is named `<deployment-name>-chronicle-on-sawtooth-chronicle-api`
and exposes port `9982`
