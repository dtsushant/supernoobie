# The studio on the test server

    https://studio.example/

Public, real certificate, and therefore microphones work.

## Where it runs

The same host as the Loop the other application stack — another application and the
network operator behind one Caddy on 443. **Nothing of theirs was taken down**
to put this here, which was the original plan and turned out to be a bad one.

    ludo-app        this, on the host's existing service's docker network, no public port
    the proxy      theirs, fronts both

## The two changes on the server

1. `~/ludo/` — the source, and `docker compose -p ludo -f deploy/docker-compose.yml up -d`
2. One block appended to `the proxy config`, routing `studio.example`
   to `ludo-app:8088`.

`billing` was chosen because it is a name that already resolves publicly and
reaches the box while **nothing serves it** — Caddy answered it `200` with an
empty body, because the operator console moved to `the fourth host` and the DNS
record was left behind. So it displaces nothing.

## Taking it away

```bash
# the app
docker compose -p ludo -f ~/ludo/deploy/docker-compose.yml down
docker rmi ludo:latest                 # ~1 GB back; the host runs at 81%

# the route
cd the other application's directory
cp Caddyfile.before-ludo Caddyfile
docker exec -w /etc/caddy the proxy caddy validate --config /etc/caddy/Caddyfile
docker exec -w /etc/caddy the proxy caddy reload   --config /etc/caddy/Caddyfile
```

It also removes itself: `ship.sh` ships the Caddyfile from the other application's repo, so
the next the other application deploy overwrites the block. Convenient, and worth knowing —
a play test can end because somebody else deployed.

## Before letting anybody in

**There is no authentication.** Anyone with the URL can open the studio, join
any room, and edit any drawing in it. That is fine for an afternoon with four
friends and is not fine indefinitely — and this one is on a public hostname,
not behind the VPN.

What contains it: a read-only root filesystem, a non-root user, no privilege
escalation, nothing in the image but the sample drawings, and no published
port. What does not contain it: anything at all about who is asking.
