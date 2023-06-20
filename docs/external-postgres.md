# Using an external database with the Chronicle Helm chart

By default, Chronicle will use an embedded PostgreSQL database.
However, you can configure Chronicle to use an
external PostgreSQL database instead.

## Configure Chronicle to use an external PostgreSQL database

The default values.yaml for the Chronicle helm chart configures an
internal database.
To use an external database, you must override these values in your
own values.yaml file.

NOTE:   The database must exist, Chronicle will not create it for you.
        Chronicle will create the necessary tables inside an empty database.

The following example values.yaml file configures Chronicle to use
an external PostgreSQL database.

```yaml
postgres:
  enabled: false
  host: my-postgres-host
  port: 5432
  database: my-postgres-database
  user: my-postgres-username
  password: my-postgres-password
```

Rather than setting the password directly in the values.yaml file, it is
recommended that you use a Kubernetes secret containing the password.

The following example values.yaml file configures Chronicle to use an existing
secret containing the password for an external PostgreSQL database.

```yaml
postgres:
  enabled: false
  host: my-postgres-host
  port: 5432
  database: my-postgres-database
  user: my-postgres-username
  existingPasswordSecret: my-postgres-password-secret
  existingPasswordSecretKey: password
```

## Create a Kubernetes secret for the external database password

Some PostgreSQL helm charts will create a secret containing the database password.
If you are using such a chart, you can use the existing secret in your
Chronicle values.yaml file.
If you are not using such a chart, you can create a secret manually
and apply it to your cluster.

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: my-postgres-password-secret
type: Opaque
data:
  password: <base64 encoded password>
```
