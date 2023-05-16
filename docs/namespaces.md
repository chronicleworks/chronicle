# Namespaces

Chronicle namespaces allow you to separate Chronicle data, avoiding collisions
between identities and avoiding returning unnecessary results with your queries.

A namespace consists of a label part and a UUID. If a namespace label does not
exist, then using it will create a new namespace with that label and a random
UUID.

This can mean that multiple instances of Chronicle can potentially have the same
namespace _label_, but are in fact referring to different namespaces. We provide
a namespace binding section in the [configuration](/config) file for this.

```toml
    [namespace_bindings]
    default = "fd717fd6-70f1-44c1-81de-287d5e101089"
```

Setting this ensures that 2 instances of Chronicle will refer to the same
namespace as 'default'.

## Built-In Namespaces

### default

The default namespace is the namespace that is used when you do not specify one
in a graphql mutation or query. It can be bound to any UUID, and should be set
up in configuration if you are using multiple Chronicle instances.

### chronicle-system

This namespace uses the uuid "00000000-0000-0000-0000-000000000001" and is
reserved for Chronicle.

## Important

You must not use the nil uuid '00000000-0000-0000-0000-000000000000' as a
binding in any circumstances, as this is used to indicate null values in the
index database.
