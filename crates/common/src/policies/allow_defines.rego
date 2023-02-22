package allow_defines

import future.keywords.in

default allow = false

allow {
  data.context.operation in ["Mutation", "Submission"]
  startswith(data.context.state[0], "define")
}
