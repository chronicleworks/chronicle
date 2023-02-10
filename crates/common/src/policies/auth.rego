package auth

import input

default is_authorized = false

is_authorized {
  input.type == "chronicle"
}
