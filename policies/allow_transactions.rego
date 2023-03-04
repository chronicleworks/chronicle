package allow_transactions

import data.common_rules
import future.keywords.in
import input

default allowed_users = false
allowed_users {
  common_rules.allowed_users
}

default allow_defines = false
allow_defines {
  common_rules.allow_defines
}

default deny_all = false
