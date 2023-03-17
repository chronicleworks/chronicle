# syntax = docker/dockerfile:1.4
# Copyright 2022 Blockchain Technology Partners, LLC
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
FROM node:18 AS chronicle-test

RUN npm install -g -f graphqurl

COPY docker/chronicle-test/chronicle-test /usr/local/bin/chronicle-test.sh
COPY docker/chronicle-test/wait-for-it /usr/local/bin/wait-for-it

RUN chmod +rx /usr/local/bin/*

ENTRYPOINT [ "/usr/local/bin/chronicle-test.sh" ]
