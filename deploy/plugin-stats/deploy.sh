#!/usr/bin/env bash
# Install root-owned as /usr/local/sbin/toolforge-plugin-stats-deploy.
# All input is streamed on stdin; neither identity can choose a filesystem path.
set -euo pipefail
umask 077
[[ $EUID == 0 ]] || { echo 'Run through the restricted administrator-installed entrypoint' >&2; exit 1; }
if [[ ${1:-} == ssh ]]; then
  allowed=${2:-}
  original=${3:-}
  read -r -a request <<< "$original"
  [[ $allowed == deploy || $allowed == catalog ]] || exit 1
  [[ ${request[0]:-} == "$allowed" ]] || { echo 'SSH identity is not authorized for this operation' >&2; exit 1; }
  set -- "${request[@]}"
fi
operation=${1:-}
base=/opt/toolforge-plugin-stats
state=/var/lib/toolforge-plugin-stats-deploy
config=/etc/toolforge-plugin-stats
install -d -o root -g root -m 700 "$state"
install -d -o root -g root -m 755 "$base" "$base/releases"
exec 9> "$state/deploy.lock"
flock -w 120 9
scratch=$(mktemp -d "$state/request.XXXXXX")
trap 'rm -rf "$scratch"' EXIT
with_env() (
  local file=$1 user=$2
  shift 2
  [[ -f $file && ! -L $file && $(stat -c %u "$file") == 0 ]] || { echo 'Unsafe service configuration ownership' >&2; exit 1; }
  local mode
  mode=$(stat -c %a "$file")
  (( (8#$mode & 0027) == 0 )) || { echo 'Service configuration must not be group writable or accessible to other users' >&2; exit 1; }
  # Start with a clean environment: SSH and deployment variables never reach DB commands.
  exec env -i PATH=/usr/sbin:/usr/bin:/sbin:/bin /bin/bash --noprofile --norc -c "
    set -aeuo pipefail
    source \"\$1\"
    shift
    role=\$1
    shift
    exec /usr/sbin/runuser -u \"\$role\" -- \"\$@\"
  " toolforge-config "$file" "$user" "$@"
)
ready() {
  local expected=$1
  curl --fail --silent --show-error --connect-timeout 1 --max-time 2 http://127.0.0.1:8088/readyz 2>/dev/null | jq -e --arg sha "$expected" '.status == "ready" and .buildSha == $sha' >/dev/null
}
case "$operation" in
  deploy)
    [[ $# == 3 && $2 =~ ^[a-f0-9]{40}$ && $3 =~ ^[a-f0-9]{64}$ ]] || { echo 'Usage: deploy <commit-sha> <binary-sha256>' >&2; exit 1; }
    commit=$2
    checksum=$3
    head -c 67108865 > "$scratch/plugin-stats"
    [[ $(stat -c %s "$scratch/plugin-stats") -le 67108864 ]] || { echo 'Binary exceeds upload limit' >&2; exit 1; }
    printf '%s  %s\n' "$checksum" "$scratch/plugin-stats" | sha256sum --check --status
    release="$base/releases/$commit"
    if [[ -e $release ]]; then
      [[ -d $release && ! -L $release ]] || exit 1
      printf '%s  %s\n' "$checksum" "$release/plugin-stats" | sha256sum --check --status
    else
      install -d -o root -g root -m 755 "$release"
      install -o root -g root -m 755 "$scratch/plugin-stats" "$release/plugin-stats"
    fi
    # Additive migrations only; credentials belong to a separate low-privilege OS/DB identity.
    with_env "$config/migrate.env" toolforge-stats-migrate "$release/plugin-stats" migrate up
    previous=''
    if [[ -L $base/current ]]; then
      previous=$(readlink -f "$base/current")
    elif [[ -e $base/current ]]; then
      echo 'Current release must be a symlink' >&2
      exit 1
    fi
    if [[ -n $previous ]]; then
      [[ $previous == "$base/releases/"* && -x $previous/plugin-stats ]] || { echo 'Unsafe previous release link' >&2; exit 1; }
    fi
    ln -sfn "$release" "$base/current.next"
    mv -Tf "$base/current.next" "$base/current"
    success=false
    if systemctl restart toolforge-plugin-stats; then
      for ((attempt=0; attempt<30; attempt++)); do
        if ready "$commit"; then success=true; break; fi
        sleep 1
      done
    fi
    if [[ $success != true ]]; then
      if [[ -n $previous ]]; then
        ln -sfn "$previous" "$base/current.rollback"
        mv -Tf "$base/current.rollback" "$base/current"
        systemctl restart toolforge-plugin-stats
        restored=false
        for ((attempt=0; attempt<15; attempt++)); do
          if ready "$(basename "$previous")"; then restored=true; break; fi
          sleep 1
        done
        [[ $restored == true ]] || { echo 'Deployment and service rollback both failed; administrator attention required' >&2; exit 1; }
      else
        systemctl stop toolforge-plugin-stats
        rm -f "$base/current"
      fi
      echo 'New service failed readiness; previous release restored. Database migrations were not reversed.' >&2
      exit 1
    fi
    logger -t toolforge-plugin-stats "deployed build $commit"
    echo "Ready: $commit"
    ;;
  catalog)
    [[ $# == 2 && $2 =~ ^[a-f0-9]{64}$ ]] || { echo 'Usage: catalog <sha256>' >&2; exit 1; }
    head -c 16777217 > "$scratch/catalog.json"
    [[ $(stat -c %s "$scratch/catalog.json") -le 16777216 ]] || { echo 'Catalog exceeds upload limit' >&2; exit 1; }
    printf '%s  %s\n' "$2" "$scratch/catalog.json" | sha256sum --check --status
    # Give the import user read access only to this verified catalog, never to the root staging tree.
    import_dir=$(mktemp -d /tmp/toolforge-catalog.XXXXXX)
    trap 'rm -rf "$scratch" "$import_dir"' EXIT
    chown toolforge-stats-catalog:toolforge-stats-catalog "$import_dir"
    install -o toolforge-stats-catalog -g toolforge-stats-catalog -m 400 "$scratch/catalog.json" "$import_dir/catalog.json"
    with_env "$config/catalog.env" toolforge-stats-catalog "$base/current/plugin-stats" catalog import "$import_dir/catalog.json"
    install -o root -g root -m 600 "$scratch/catalog.json" "$state/catalog-$2.json"
    logger -t toolforge-plugin-stats "imported catalog $2"
    ;;
  *) echo 'Only deploy and catalog operations are supported' >&2; exit 1 ;;
esac
