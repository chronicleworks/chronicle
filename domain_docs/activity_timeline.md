# `activityTimeline`

The majority of the work for provenance retrieval will be with the
activity timeline query.

## Parameters

* `activityTypes` - A list of ActivityTypes to filter the returned
    timeline by, leaving this empty will return all activity types.
    The `ProvActivity` activity type can be used to return activities
    that are not currently specified in the Chronicle domain.

* `forEntity` - A list of EntityIDs or externalIds to filter
    activities by, leaving this empty allows activities for any entity.

* `forAgent` - A list of AgentIDs or externalIds to filter
    activities by, leaving this empty allows activities for any agent.

* from - The time in RFC3339 format to return activities from.
    Not specifying this will return all activity types before the time
    specified in `to`.

* to - The time in RFC3339 format to return activities until. Not
  specifying this will return all activity types after the time
  specified in `from`.

* after - Relay cursor control, returning a page after the cursor
  you supply to this argument - for forwards pagination.

* before - Relay cursor control, returning items before the cursor
  you supply to this argument - for reverse pagination.

* first - An integer controlling page size for forward pagination.
  Defaults to 20.

* last - An integer controlling page size for reverse pagination.
  Defaults to 20.
