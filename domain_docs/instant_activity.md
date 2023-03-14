# `chronicle:instantActivity`

A convenience operation that will set both start and end
time in the same operation.

Eliding the time parameter will use the current system time. Time stamps
should be in [RFC3339](https://www.rfc-editor.org/rfc/rfc3339.html) format.

## Example

```graphql
mutation {
  instantActivity(
    id: { id: "chronicle:activity:september-2018-review" },
    time: "2002-10-02T15:00:00Z"
  )
}
```
