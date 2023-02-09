package auth

import input

default is_authenticated = false

is_authenticated {
  input.type == "Chronicle"
}
