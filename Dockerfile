# The studio, in a container.
#
# Built on the server rather than here, because the build machine is aarch64
# and the server is x86_64 -- and a cross toolchain is a great deal of setup to
# avoid one `docker build`.
#
# Two stages so the thing that ships is a binary and a few sample files rather
# than a Rust toolchain. The runtime image is about 90 MB against roughly a
# gigabyte for the builder, which matters on a host with nine gigabytes free.

FROM rust:1-slim AS build
WORKDIR /src

# The whole workspace. `-p web` only compiles what the server needs -- `studio`
# is in the workspace but is a desktop window and pulls in X11; it is never
# built here because nothing the server runs depends on it.
COPY . .
RUN cargo build --release -p web --locked

# ---------------------------------------------------------------------------
FROM debian:stable-slim
WORKDIR /app

# The HTML, JavaScript and CSS are `include_str!`d into the binary, so the only
# files that have to travel are the drawings themselves -- which ARE read at
# runtime, by name, and are what `/list` lists.
COPY --from=build /src/target/release/web /usr/local/bin/web
COPY samples /app/samples

# Nothing here runs as root. The app reads files by name -- `.easel` and `.rec`
# only, relative, no `..` -- and the surest guard on that is a working
# directory with nothing else in it and a user who owns nothing else.
RUN useradd --system --create-home --home-dir /app --shell /usr/sbin/nologin studio \
    && chown -R studio:studio /app
USER studio

EXPOSE 8088

# `--open` binds every interface, which inside a container means "let Caddy
# reach me" rather than "let the network in": nothing publishes this port to
# the host. Caddy is the only thing that can see it.
CMD ["web", "--open", "samples/ludogame.easel"]
