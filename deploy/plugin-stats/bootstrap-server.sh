#!/usr/bin/env bash
# One-time server bootstrap for the plugin statistics service (run as root).
# Creates the database, OS users, /etc config, deployment entry and systemd
# service so that a dev push deploys automatically afterwards.
#
# Copy this file together with deploy.sh and toolforge-plugin-stats.service
# to the server, then run:
#   sudo bash bootstrap-server.sh --deploy-key "$(cat stats-deploy.pub)"
#
# Flags:
#   --deploy-key <file|literal>   public SSH key allowed to deploy (required)
#   --asset-base <url>            COS download domain (default plugins.toolforge.devlive.org)
#   --listen <addr>               API listen address (default 127.0.0.1:8088)
set -euo pipefail
umask 077

deploy_key=
asset_base=https://plugins.toolforge.devlive.org
listen=127.0.0.1:8088
while [[ $# -gt 0 ]]; do
  case $1 in
    --deploy-key)
      shift
      deploy_key=${1:?--deploy-key requires a value}
      ;;
    --asset-base)
      shift
      asset_base=${1:?--asset-base requires a value}
      ;;
    --listen)
      shift
      listen=${1:?--listen requires a value}
      ;;
    *) echo "Unknown flag: $1" >&2; exit 1 ;;
  esac
  shift
done

[[ $EUID == 0 ]] || { echo 'Run as root' >&2; exit 1; }
here=$(cd "$(dirname "$0")" && pwd)
[[ -f $here/deploy.sh && -f $here/toolforge-plugin-stats.service ]] || {
  echo 'Put bootstrap-server.sh next to deploy.sh and toolforge-plugin-stats.service from the repository' >&2
  exit 1
}
[[ -n $deploy_key ]] || { echo 'A deploy public key is required for automated deployment' >&2; exit 1; }

for tool in mysql jq flock sha256sum; do
  command -v "$tool" >/dev/null || { echo "Missing $tool; install it first" >&2; exit 1; }
done

echo '==> Creating OS users'
for name in toolforge-stats toolforge-stats-migrate toolforge-stats-catalog; do
  id "$name" >/dev/null 2>&1 || useradd --system --no-create-home --shell /usr/sbin/nologin "$name"
done
id stats-deploy >/dev/null 2>&1 || useradd -m -s /bin/bash stats-deploy

echo '==> Creating database and application account'
# One MySQL account covers runtime, migrations and catalog import; split roles
# later by editing the three env files if you need tighter privileges.
stats_password=$(openssl rand -base64 24 | tr -d '/+=')
mysql <<SQL
CREATE DATABASE IF NOT EXISTS toolforge_stats CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE USER IF NOT EXISTS 'toolforge_stats'@'localhost' IDENTIFIED BY '${stats_password}';
ALTER USER 'toolforge_stats'@'localhost' IDENTIFIED BY '${stats_password}';
GRANT ALL PRIVILEGES ON toolforge_stats.* TO 'toolforge_stats'@'localhost';
FLUSH PRIVILEGES;
SQL

echo '==> Writing /etc/toolforge-plugin-stats'
install -d -o root -g root -m 750 /etc/toolforge-plugin-stats
install -o root -g toolforge-stats -m 640 /dev/null /etc/toolforge-plugin-stats/runtime.env
cat > /etc/toolforge-plugin-stats/runtime.env <<EOF
MYSQL_SOCKET=/var/run/mysqld/mysqld.sock
MYSQL_DATABASE=toolforge_stats
MYSQL_USER=toolforge_stats
MYSQL_PASSWORD='${stats_password}'
STATS_LISTEN_ADDR=${listen}
PLUGIN_ASSET_BASE_URL=${asset_base}
RECEIPT_ACTIVE_KID=2026-01
RECEIPT_KEYS_JSON='{"2026-01":"$(openssl rand -base64 32 | tr -d '\n')"}'
EOF
install -o root -g root -m 600 /dev/null /etc/toolforge-plugin-stats/migrate.env
cat > /etc/toolforge-plugin-stats/migrate.env <<EOF
MYSQL_SOCKET=/var/run/mysqld/mysqld.sock
MYSQL_DATABASE=toolforge_stats
MYSQL_USER=toolforge_stats
MYSQL_PASSWORD='${stats_password}'
EOF
cp /etc/toolforge-plugin-stats/migrate.env /etc/toolforge-plugin-stats/catalog.env

echo '==> Installing deployment entry and systemd service'
install -o root -g root -m 755 "$here/deploy.sh" /usr/local/sbin/toolforge-plugin-stats-deploy
install -o root -g root -m 644 "$here/toolforge-plugin-stats.service" /etc/systemd/system/toolforge-plugin-stats.service
install -d -o root -g root -m 755 /opt/toolforge-plugin-stats/releases
install -d -o toolforge-stats -g toolforge-stats -m 700 /var/lib/toolforge-plugin-stats
install -d -o root -g root -m 700 /var/lib/toolforge-plugin-stats-deploy

install -d -o stats-deploy -g stats-deploy -m 700 ~stats-deploy/.ssh
key_file=$(mktemp)
printf '%s\n' "$deploy_key" > "$key_file"
cat "$key_file" >> ~stats-deploy/.ssh/authorized_keys
rm -f "$key_file"
chmod 600 ~stats-deploy/.ssh/authorized_keys
chown -R stats-deploy:stats-deploy ~stats-deploy/.ssh
echo 'stats-deploy ALL=(root) NOPASSWD: /usr/local/sbin/toolforge-plugin-stats-deploy *' \
  > /etc/sudoers.d/toolforge-plugin-stats
chmod 440 /etc/sudoers.d/toolforge-plugin-stats
visudo -cf /etc/sudoers.d/toolforge-plugin-stats

systemctl daemon-reload
systemctl enable toolforge-plugin-stats

host_key=$(ssh-keyscan -t ed25519 127.0.0.1 2>/dev/null | head -1 || true)
echo
echo 'Bootstrap complete. Fill these in GitHub Environment plugin-stats-production:'
echo "  STATS_DEPLOY_SSH_KEY    = the PRIVATE key matching the public key you passed"
echo "  STATS_DEPLOY_SSH_TARGET = stats-deploy@<this-server-ip>"
echo "  PLUGIN_SSH_KNOWN_HOSTS  = this server public host key:"
[[ -n $host_key ]] && echo "$host_key"
echo
echo 'The service starts on the first automated deploy (push server changes to dev).'
