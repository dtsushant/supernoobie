#!/usr/bin/env bash
# First run on a fresh VPS. Idempotent — safe to run twice.
#
#     scp deploy/own/setup.sh root@<ip>:/tmp/
#     ssh root@<ip> 'bash /tmp/setup.sh <your-ssh-public-key-file-contents>'
#
# Everything here is either "so it survives a reboot" or "so a stranger cannot
# walk in". Nothing about the game.
set -euo pipefail

KEYS="${1:-}"
WHO="${STUDIO_USER:-studio}"

say() { printf '\n=== %s\n' "$1"; }

# ---------------------------------------------------------------- swap
# **Before Docker, because this is what usually goes wrong.** A release build of
# the workspace peaks around 1.5 GB, and a small VPS has 1 GB. Without swap the
# kernel kills the compiler, `docker build` reports a mysterious `signal 9`, and
# nothing explains itself.
#
# A file rather than a partition, since it can be removed as easily as made.
say "swap"
if [ ! -f /swapfile ]; then
  fallocate -l 2G /swapfile || dd if=/dev/zero of=/swapfile bs=1M count=2048
  chmod 600 /swapfile
  mkswap /swapfile >/dev/null
  swapon /swapfile
  grep -q '^/swapfile' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
  # Lean on it only when really short: swap is for not dying, not for running.
  sysctl -w vm.swappiness=10 >/dev/null
  grep -q '^vm.swappiness' /etc/sysctl.conf || echo 'vm.swappiness=10' >> /etc/sysctl.conf
  echo "  2G added"
else
  echo "  already there"
fi

say "packages"
export DEBIAN_FRONTEND=noninteractive
apt-get update -qq
apt-get install -yqq ca-certificates curl ufw fail2ban unattended-upgrades >/dev/null
echo "  done"

say "docker"
if ! command -v docker >/dev/null; then
  curl -fsSL https://get.docker.com | sh >/dev/null
  systemctl enable --now docker
  echo "  installed"
else
  echo "  already there"
fi

# ---------------------------------------------------------------- a user
# Not root. A deploy that goes wrong as `studio` breaks the studio; the same
# mistake as root breaks the machine.
say "user"
if ! id "$WHO" >/dev/null 2>&1; then
  adduser --disabled-password --gecos "" "$WHO" >/dev/null
  echo "  $WHO created"
else
  echo "  $WHO already there"
fi
# **Outside the branch, deliberately.** This used to sit inside the "created"
# arm, so running against a machine that already had the user -- which is the
# ordinary case, since the host gives you one -- left them unable to talk to
# Docker, and every later command failed with a permission error that looked
# like Docker being broken.
usermod -aG docker "$WHO"
echo "  $WHO can use docker"

if [ -n "$KEYS" ]; then
  install -d -m 700 -o "$WHO" -g "$WHO" "/home/$WHO/.ssh"
  echo "$KEYS" > "/home/$WHO/.ssh/authorized_keys"
  chmod 600 "/home/$WHO/.ssh/authorized_keys"
  chown "$WHO:$WHO" "/home/$WHO/.ssh/authorized_keys"
  echo "  key installed for $WHO"
else
  echo "  NO KEY GIVEN -- copy one in before turning passwords off"
fi

# ---------------------------------------------------------------- the wall
# **In this order, and check ssh first.** Enabling a firewall that does not
# allow 22 is how somebody locks themselves out of a machine they have owned for
# ten minutes.
say "firewall"
ufw allow 22/tcp >/dev/null
ufw allow 80/tcp >/dev/null
ufw allow 443/tcp >/dev/null
ufw allow 443/udp >/dev/null
ufw --force enable >/dev/null
ufw status numbered | sed 's/^/  /'

say "unattended security updates"
cat > /etc/apt/apt.conf.d/20auto-upgrades <<'CONF'
APT::Periodic::Update-Package-Lists "1";
APT::Periodic::Unattended-Upgrade "1";
CONF
echo "  on"

cat <<DONE

=== done. What is NOT done, on purpose:

  * Password logins are still allowed. Turn them off only once you have
    confirmed the key works, in ANOTHER terminal, without closing this one:

      ssh $WHO@<this server>          # must succeed first

    then:  sed -i 's/^#*PasswordAuthentication.*/PasswordAuthentication no/' \\
             /etc/ssh/sshd_config && systemctl reload ssh

    Doing it before that check is the commonest way to lose a server.

  * Nothing is deployed yet. DNS has to point here first, or the certificate
    request fails -- see deploy/own/README.md.

DONE
