#!/usr/bin/env bash
# Ship the studio to a server and restart it.
#
#     DEPLOY_HOST=user@host DEPLOY_KEY=~/.ssh/id LUDO_NET=proxy_net bash deploy/ship.sh
#
# Nothing about any particular server lives in this file. It takes where to go
# from the environment, so the repository never learns the name of a machine
# somebody happens to run it on.
#
# It builds ON THE SERVER, because a laptop is often a different architecture
# from a server and a cross toolchain is a great deal of setup to avoid one
# `docker build`.
set -euo pipefail

HOST="${DEPLOY_HOST:?set DEPLOY_HOST, e.g. user@host}"
KEY="${DEPLOY_KEY:-$HOME/.ssh/id_rsa}"
DIR="${DEPLOY_DIR:-ludo}"
NET="${LUDO_NET:-web}"
# Which stack. `own` is a server of our own -- its own proxy, its own
# certificates, nothing else on the box. The default is the other case: sharing
# a host, where something else already holds 80 and 443.
COMPOSE="${COMPOSE:-deploy/docker-compose.yml}"
SSH=(ssh -i "$KEY" -o ConnectTimeout=20)

echo "packing"
tar --exclude=target --exclude=.git --exclude='*.png' -czf /tmp/ludo_src.tgz .

echo "shipping to $HOST"
scp -i "$KEY" -q /tmp/ludo_src.tgz "$HOST:/tmp/ludo_src.tgz"

# The build takes a few minutes -- it compiles the workspace on the server --
# so anything wrapping this needs a longer patience than a shell's default.
echo "building and restarting (a few minutes)"
"${SSH[@]}" "$HOST" "set -e
  mkdir -p ~/$DIR && cd ~/$DIR
  tar -xzf /tmp/ludo_src.tgz && rm -f /tmp/ludo_src.tgz
  docker build -q -t ludo:latest . >/dev/null
  LUDO_NET='$NET' docker compose -p ludo -f '$COMPOSE' up -d --force-recreate >/dev/null
  sleep 3
  docker ps --filter name=ludo-app --format '  {{.Names}}  {{.Status}}'"

# **The exit code is the answer, and so is the served page.** Not the log.
#
# Twice this was believed to have deployed when it had not: the scp failed, the
# script stopped, and a `grep` for "shipped" in a log file found the line from
# the PREVIOUS run -- because the redirection had not truncated the file yet
# when the check ran. Two rounds of fixes were tested against a build from two
# days earlier and reported as not working.
#
# So: check something the new build serves, rather than something a script
# said about itself.
echo "shipped"
