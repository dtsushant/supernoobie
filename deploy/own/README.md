# supernoobie.com, on your own server

The order matters. Certificates are the reason: Let's Encrypt proves you own the
domain by **fetching a file from the server over port 80 at that name**. Do it
before DNS points there and the fetch reaches the wrong machine, the request
fails, and failures are rate-limited — five an hour.

So: DNS first, server second, certificate last and by itself.

---

## 1 · Point the domain

At the registrar you bought it from, add two records:

| type | name | value | TTL |
|---|---|---|---|
| `A` | `@` | *your server's IP* | 300 |
| `A` | `www` | *your server's IP* | 300 |

`@` means the bare name, `supernoobie.com`. Some panels want it blank instead.

**Set the TTL low** (300 = five minutes) *before* you need to change anything.
TTL is how long the rest of the internet is allowed to remember an answer, so a
mistake made at 3600 takes an hour to correct no matter how fast you fix it. Put
it back up once things are settled.

If the registrar has already filled in an A record pointing at a parking page,
**edit it rather than adding a second** — two A records means half your visitors
get the parking page, which looks exactly like an intermittent bug.

Then wait, and check from outside your own network:

```bash
nslookup supernoobie.com 8.8.8.8
```

Your own machine may cache the old answer for a while; `8.8.8.8` will not.
Do not go on until this returns your server's IP.

---

## 2 · Set the server up

```bash
scp deploy/own/setup.sh root@<ip>:/tmp/
ssh root@<ip> "bash /tmp/setup.sh '$(cat ~/.ssh/id_ed25519.pub)'"
```

That adds swap, installs Docker, makes a non-root `studio` user with your key,
and opens 22, 80 and 443 — nothing else.

**Swap matters more than it sounds.** A release build of this workspace peaks
near 1.5 GB and a small VPS has 1 GB. Without it the kernel kills the compiler
mid-build and Docker reports `signal 9`, which explains nothing.

Then check the key works **in a second terminal, without closing the first**:

```bash
ssh studio@<ip> 'echo it works'
```

Only once that succeeds, turn off password logins. Doing it before that check is
the commonest way to lose a server you have owned for ten minutes.

---

## 3 · Ship it

```bash
DEPLOY_HOST=studio@<ip> DEPLOY_KEY=~/.ssh/id_ed25519 \
  bash deploy/ship.sh
```

Then, on the server, once:

```bash
cd ~/ludo
echo "ADMIN_EMAIL=you@example.com" > deploy/own/.env
docker compose -f deploy/own/docker-compose.yml up -d
```

The email is where Let's Encrypt writes if a renewal starts failing — the only
warning you get before the site goes dark.

Watch the first certificate being issued:

```bash
docker compose -f deploy/own/docker-compose.yml logs -f caddy
```

`certificate obtained successfully` and you are done. If it fails, **read the
error before retrying** — five failures an hour is the limit, and a loop of
hopeful retries spends them in a minute. To practise without spending any,
uncomment the `acme_ca` staging line in the Caddyfile: it issues untrusted
certificates the browser will complain about, with far looser limits.

---

## 4 · Check it from outside

```bash
curl -sI https://supernoobie.com | head -3
curl -s -o /dev/null -w '%{http_code} %{ssl_verify_result}\n' https://supernoobie.com
```

`200` and `0` — the second is the certificate actually verifying, rather than
merely being present.

---

## What this ends

On the borrowed host the studio lived as a block in somebody else's proxy
config, and their deploys shipped their own copy of that file. Twice the game
"went down" when nothing was down at all: the container was running and serving
perfectly, and the hostname answered **200 with an empty body** — a proxy
politely saying *nothing here*.

Here, nothing else owns the proxy. There is no block for anybody else's deploy
to remove.

## What is still true, and worth deciding on purpose

**There is no authentication.** Anyone with the address can open the studio,
join any room, and edit any drawing in it. That was fine for an afternoon with
four friends on a borrowed host that nobody knew about. `supernoobie.com` is a
name you will give people.

What contains it: a read-only root filesystem, a non-root user, no privilege
escalation, a memory limit, nothing in the image but the sample drawings, and no
published port. What does not contain it: anything at all about who is asking.
