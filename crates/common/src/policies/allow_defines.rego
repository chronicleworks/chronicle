package allow_defines

import future.keywords.in

default allow = false

allow {
  data.parent_type in ["Mutation", "Submission"]
  startswith(data.resolve_path[0], "define")
}
