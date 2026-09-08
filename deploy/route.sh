#!/usr/bin/env bash
# Put the studio's route back into a proxy config that has been replaced.
#
#     DEPLOY_HOST=user@host DEPLOY_KEY=~/.ssh/id \
#     ROUTE_HOST=studio.example CADDY=/home/you/other/Caddyfile CONTAINER=proxy \
#     bash deploy/route.sh
#
# ## Why this exists
#
# The studio is fronted by a proxy belonging to something else, and that
# something else ships its own config on every deploy. So its next deploy
# silently deletes this route: the container keeps running, the page is still
# served on its own network, and the hostname answers **200 with an empty
# body** -- which looks like the application being broken rather than the route
# being gone.
#
# Self-removing was the right default for a temporary test. Being able to put it
# back in one command is the right answer to the second time.
#
# Idempotent: run it twice and nothing happens twice.
#
# ## A note on quoting
#
# The remote script is fed on stdin inside a **quoted** heredoc, so nothing in
# it is expanded here before it travels. The first version of this file built
# the remote script inside double quotes and every `$`, backslash and brace in
# it was mangled on the way -- it failed with no output at all, which is the
# worst way for a deployment script to fail. Settings cross as environment
# variables, which need no quoting at all.
set -euo pipefail

HOST="${DEPLOY_HOST:?set DEPLOY_HOST}"
KEY="${DEPLOY_KEY:-$HOME/.ssh/id_rsa}"

ssh -i "$KEY" -o ConnectTimeout=20 \
    -o SendEnv=none "$HOST" \
    "ROUTE_HOST='${ROUTE_HOST:?set ROUTE_HOST}' \
     CADDY='${CADDY:?set CADDY, the config path on the server}' \
     CONTAINER='${CONTAINER:?set CONTAINER, the proxy container}' \
     CERT_DIR='${CERT_DIR:-/certs}' \
     UPSTREAM='${UPSTREAM:-ludo-app:8088}' bash -s" <<'REMOTE'
set -euo pipefail

if grep -q "^${ROUTE_HOST} {" "$CADDY"; then
  echo "  already routed"
else
  # A copy before touching it, so a rollback is a file and not a memory.
  cp -n "$CADDY" "$CADDY.before-ludo" 2>/dev/null || true

  cat >> "$CADDY" <<BLOCK

# TEMPORARY -- the studio. A deploy of this application ships its own copy of
# this file, so this block goes with it. deploy/route.sh puts it back.
${ROUTE_HOST} {
	tls ${CERT_DIR}/bundle.crt ${CERT_DIR}/private.key
	encode gzip
	reverse_proxy ${UPSTREAM}
}
BLOCK

  # VALIDATE BEFORE RELOADING. A config that will not parse takes down
  # everything this proxy was already serving, not only the thing being added.
  if ! docker exec -w /etc/caddy "$CONTAINER" caddy validate --config /etc/caddy/Caddyfile >/dev/null 2>&1; then
    echo "  config invalid -- put back, nothing changed"
    cp "$CADDY.before-ludo" "$CADDY"
    exit 1
  fi

  # Reload, not restart: nobody's connection is dropped.
  docker exec -w /etc/caddy "$CONTAINER" caddy reload --config /etc/caddy/Caddyfile >/dev/null 2>&1
  echo "  routed"
fi

# Check EVERY host this proxy serves, not only ours. The failure worth catching
# is the one you did not mean to cause.
sleep 1
echo "  --- every host this proxy answers for ---"
grep -oE '^[A-Za-z0-9.-]+\.[A-Za-z]+[A-Za-z0-9.,[:space:]-]*\{' "$CADDY" \
  | tr -d '{' | tr ',' '\n' | tr -d '[:blank:]' | grep . | sort -u \
  | while read -r h; do
      printf '  %-34s ' "$h"
      curl -sk -o /dev/null -w '%{http_code}  %{size_download} bytes\n' \
        --resolve "$h:443:127.0.0.1" "https://$h/" || echo '-'
    done
REMOTE
