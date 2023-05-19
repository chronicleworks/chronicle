# syntax = docker/dockerfile:1.4
# Copyright 2023 Blockchain Technology Partners, LLC
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
FROM ubuntu:latest AS define-agent-test

RUN apt-get update && apt-get install -y \
    curl \
    jq

RUN curl -sL https://deb.nodesource.com/setup_14.x | bash -
RUN apt-get install -y nodejs
RUN npm install -g -f graphqurl --yes
RUN npm install --yes

COPY docker/chronicle-helm-test/wait-for-it /usr/local/bin/wait-for-it
COPY docker/chronicle-helm-test/subscribe-submit-test /usr/local/bin/subscribe-submit-test

RUN chmod +rx /usr/local/bin/*

ENTRYPOINT [ "/usr/local/bin/subscribe-submit-test" ]
