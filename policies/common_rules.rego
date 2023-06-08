package common_rules

import future.keywords.in
import input

allowed := {"chronicle", "anonymous", "jwt"}

allowed_users {
  input.type in allowed
}

allow_defines {
  data.context.operation in ["Mutation", "Submission"]
  startswith(data.context.state[0], "define")
}
