# Chronicle Rancher by SUSE Cookbook

Recipe for installing Chronicle using a helm chart published in the Rancher Apps
and Marketplace.

## Useful Links

* [Chronicle Overview](https://docs.chronicle.works/)
* [Chronicle Docs](https://docs.chronicle.works/)
* [Chronicle Examples](https://examples.chronicle.works)

## Prerequisites

You will need the following:

* Kubernetes cluster as specified below, and managed by
  [Rancher by SUSE](https://www.suse.com/products/suse-rancher/) v2.6 or later.

* [kubectl](https://kubernetes.io/docs/tasks/tools/#kubectl) configured to
  access your cluster

### Cluster Configuration

* **Nodes**: Chronicle uses Sawtooth as its backing ledger. Therefore it needs
  to be deployed on a Kubernetes cluster with a minimum of 4 standard nodes,
  each with at least 4 vCPU and 16GB memory.

* **Persistent Storage**: Chronicle requires a 40Gi PVC if deployed with an
  internal Postgres database, and Sawtooth requires a 40Gi PVC per node.

### Longhorn

If your Kubernetes environment doesn't have persistent storage enabled by
default then we recommend that you use [Longhorn](https://longhorn.io/).
Like Chronicle, this can also be installed using a helm chart published in
the Rancher Apps and Marketplace. Instructions on how to do this can be found
[here](https://longhorn.io/docs/1.4.2/deploy/install/install-with-rancher/).

## Chronicle Stack

This diagram illustrates the key layers in the Chronicle stack. By default the
untyped Chronicle platform is deployed by Rancher but, as discussed at the end
of these instructions, in practice this should be replaced with a docker image
that implements your particular Chronicle Domain.

![Chronicle Stack](/assets/rancher/chronicle-stack.png)

## Install Chronicle

Log in to Rancher and select the cluster you want to install Chronicle on.
In our example, this will be the `local` cluster. From the left menu, select
_Apps_ and then _Charts_ as shown below.

![Partner Charts](/assets/rancher/partner-charts.png)

Choose the Chronicle chart from the list of partner charts:
This will take you to the following screen.

![Chronicle Chart](/assets/rancher/chronicle-chart.png)

Click on the _Install_ button at the top right of the page. This will take you
to Step 1 of the installation.

![Install Step 1](/assets/rancher/install-step-1.png)

Here, you will need to specify the _namespace_ for your Chronicle
installation. In our example, we will use the existing `chronicle` namespace.

Now, click the _Next_ button on the bottom right of the page. This will take
you to Step 2 of the installation.

![Install Step 2](/assets/rancher/install-step-2.png)

Here, you can configure your Chronicle installation. On the left hand
side, you will find three options:

* **Chronicle Settings** - Here you can configure the Chronicle image and tag
  that you want to use. We will leave these as the defaults for now, using the
  [untyped](https://docs.chronicle.works/chronicle/untyped_chronicle/) version of
  Chronicle. You can also enable a development GraphQL playground, however in
  this example we will leave this disabled, and use the Altair GraphQL client
  instead.

* **Ingress Settings** - If you'd like to enable an ingress for Chronicle,
  you can specify this here. This is optional.

* **Database Settings** - If you'd like to use an external Postgres database,
  you can specify this here. This is also optional.

Now, click the _Install_ button on the bottom right of the page.

Rancher will now install Chronicle on your chosen cluster, in our
example the `local` cluster. It may take a few minutes for the Chronicle
images to be pulled down, and for the underlying Hyperledger Sawtooth network
to be deployed as shown below.

![Installing Chronicle](/assets/rancher/installing-chronicle.png)

## Test your Deployment

We will now switch to a local terminal window to test our Chronicle install.

Once you've opened a local terminal, start by confirming that you can connect to
your Kubernetes cluster using `kubectl` by running this command:

```bash
kubectl get pods -n chronicle
```

The output should look something like this:

```text
NAME                    READY   STATUS    RESTARTS   AGE
chronicle-chronicle-0   2/2     Running   0          7m
chronicle-sawtooth-0    7/7     Running   0          7m
chronicle-sawtooth-1    7/7     Running   0          7m
chronicle-sawtooth-2    7/7     Running   0          7m
chronicle-sawtooth-3    7/7     Running   0          7m
chronicle-tp-0          1/1     Running   0          7m
chronicle-tp-1          1/1     Running   0          7m
chronicle-tp-2          1/1     Running   0          7m
chronicle-tp-3          1/1     Running   0          7m
```

We now need to set up a port forward so that we can access the Chronicle api
on our local machine.
Run the following command to set up a port forward:

```bash
kubectl port-forward chronicle-chronicle-0 9982:9982 -n chronicle
```

This will set up a port forward to your Chronicle install, and make it
accessible on your local machine. The output should look something like this:

```text
Forwarding from 127.0.0.1:9982 -> 9982
Forwarding from [::1]:9982 -> 9982
```

You can now access the Chronicle GraphQL API using the following URL at
[http://127.0.0.1:9982](http://127.0.0.1:9982)

We will use the
[Altair GraphQL client](https://github.com/altair-graphql/altair)
to test our Chronicle install in the browser

Open the Altair GraphQL client in your browser, and paste in the following
URL `http://127.0.0.1:9982`

![Altair GraphQL Client](/assets/rancher/altair-client.png)

Copy and paste the following query into the Altair GraphQL client:

```graphql
mutation {
  defineAgent(externalId: "test", namespace: "test", attributes: {}) {
    context
    txId
  }
}
```

Click _Send Request_ to run the query, and you should see a
response on the right hand side:

![Altair Response](/assets/rancher/altair-response.png)

Congratulations, you have successfully installed Chronicle on your Kubernetes
cluster using Rancher!

## Customize your Deployment

As noted above, by default the Chronicle docker image deployed is
the [untyped](https://docs.chronicle.works/chronicle/untyped_chronicle/) version of
Chronicle. However, this should only be used for testing deployments because
the real power of Chronicle is using it with a custom Chronicle domain.

Therefore, you should now build a docker image using your own domain, or one of
the [Chronicle examples](https://examples.chronicle.works).

Once you've built this image, you can edit your running Chronicle deployment
using Rancher and update the repository/tag details in Step 2. Once you
confirm these changes Rancher will automatically update the deployment.
The same process can be employed whenever you rebuild your image and release it
with a new tag.

The [Chronicle Bootstrap](https://github.com/chronicleworks/chronicle-bootstrap) repo
provides instructions and example scripts for building your own docker image.

For more details on modeling your domain, see the
[Chronicle Docs](https://docs.chronicle.works/chronicle).

## Commercial Support

While both Chronicle and Rancher are 100% open source for mission critical
or production use we recommend subscribing to Chronicle Enterprise and
[Rancher Prime by SUSE](https://www.suse.com/solutions/enterprise-container-management/#rancher-product)
respectively.
