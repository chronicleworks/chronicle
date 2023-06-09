# Resetting Chronicle

## How to reset a Chronicle Helm deployment

Warning: This will delete all data in the Chronicle deployment.

Chronicle, and the underlying Sawtooth network, store their data in Persistent
Volumes. These volumes, and therefore the data, will persist even if the
containers are stopped and restarted.

The following steps will reset all data and start Chronicle from scratch.

1. Uninstall the Chronicle deployment:

    ```bash
    helm uninstall <deployment-name>
    ```

1. Delete the Chronicle Persistent Volume Claims.

    ```bash
    kubectl delete pvc -l release=<deployment-name>
    ```

1. Delete the Chronicle Persistent Volumes.

    ```bash
    kubectl delete pv -l release=<deployment-name>
    ```

Next you need to delete the Sawtooth data. This is done by changing the
value of `Genesis Seed` in your `values.yaml` file.
This will cause Sawtooth to delete all existing data and keys on startup,
then rerun genesis.

The Sawtooth helm chart, which is a dependency of the
chronicle-on-sawtooth chart, sets a default value for `sawtooth.genesis.seed`

If you set a new random string for the genesis seed in your `values.yaml` file
this will override the default value.

```yaml
sawtooth:
  sawtooth:
    genesis:
      seed: ef9350c8-abd8-45a1-a85c-2973652bb06e
```

Note, you must save your new genesis seed value, and make sure it is included
your `values.yaml` file for any further helm operations, as if it is not
present, the default value will be used, causing your data to be reset again.

  Finally, you can reinstall the Chronicle deployment:

  ```bash
  helm install <deployment-name> -f values.yaml btp-stable/chronicle-on-sawtooth
  ```
