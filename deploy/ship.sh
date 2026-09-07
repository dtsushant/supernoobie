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
SSH=(ssh -i "$KEY" -o ConnectTimeout=20)

echo "packing"
tar --exclude=target --exclude=.git --exclude='*.png' -czf /tmp/ludo_src.tgz .

echo "shipping to $HOST"
scp -i "$KEY" -q /tmp/ludo_src.tgz "$HOST:/tmp/ludo_src.tgz"

echo "building and restarting"
"${SSH[@]}" "$HOST" "set -e
  mkdir -p ~/$DIR && cd ~/$DIR
  tar -xzf /tmp/ludo_src.tgz && rm -f /tmp/ludo_src.tgz
  docker build -q -t ludo:latest . >/dev/null
  LUDO_NET='$NET' docker compose -p ludo -f deploy/docker-compose.yml up -d --force-recreate >/dev/null
  sleep 3
  docker ps --filter name=ludo-app --format '  {{.Names}}  {{.Status}}'"

# The exit code is the answer, not the log. A build that half-failed and left
# the old container running looks exactly like a deploy that worked.
echo "shipped"
