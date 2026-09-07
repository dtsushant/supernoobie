# Running the studio on a server

    docker build -t ludo:latest .
    docker compose -p ludo -f deploy/docker-compose.yml up -d

Then point a reverse proxy at `ludo-app:8088` under a name whose certificate you
already hold.

## Why there is a proxy at all

The game would be perfectly happy on a bare port. **A browser will not give a
page a microphone unless the page is a secure context** — `https`, or
`localhost` — and on plain `http` `getUserMedia` is not refused, it is
*absent*, so the failure is a `TypeError` about `undefined` rather than
anything a person could act on.

So the proxy is not for the game. It is for the four people talking to each
other.

## If the host already runs something else

Two rules, learned from a host that did:

**Do not take anything down to make room.** Add a name beside what is there.
A proxy already terminating TLS will serve another host for the cost of one
block, and rolling that back is deleting the block.

**Validate before reloading.** A proxy config that will not parse takes down
everything it was already serving, not just the thing being added:

    caddy validate --config /etc/caddy/Caddyfile     # first
    caddy reload   --config /etc/caddy/Caddyfile     # only then

Keep a copy of the config as it was, so a rollback is a file and not a memory.
And check the *other* hosts after reloading, not just the new one — the failure
worth catching is the one you did not mean to cause.

## Building

Built on the server rather than shipped as a binary, because the machine this
was written on is aarch64 and servers commonly are not, and a cross toolchain
is a great deal of setup to avoid one `docker build`.

Two stages, so what runs is about 90 MB rather than a Rust toolchain. Only
`samples/*.easel` travel — the HTML, JavaScript and CSS are `include_str!`d
into the binary, which also means the container has nothing to read but the
drawings.

## Before letting anybody in

**There is no authentication.** Anyone with the URL can open the studio, join
any room, and edit any drawing in it. That is fine for an afternoon with four
friends and is not fine indefinitely.

What contains it: a read-only root filesystem, a non-root user, no privilege
escalation, nothing in the image but the sample drawings, and no published
port. What does not contain it: anything at all about who is asking.

## Taking it away

    docker compose -p ludo -f deploy/docker-compose.yml down
    docker rmi ludo:latest

and remove the block from the proxy config.
