#!/usr/bin/env bash
# Publish verified immutable COS objects, import catalog, then expose the new registry.
set -euo pipefail
umask 077
dir=${1:-dist/plugins}
xtask=${XTASK_BIN:-target/release/xtask}
coscli=${COSCLI_BIN:-coscli}
: "${COS_CONFIG_PATH:?Set a temporary COSCLI YAML configuration path}"
: "${PLUGIN_CATALOG_SSH_TARGET:?Set the restricted catalog SSH identity target}"
: "${PLUGIN_SSH_KEY_PATH:?Set the catalog SSH private-key file path}"
: "${GITHUB_REPOSITORY:?Set owner/repository}"
bucket=${COS_BUCKET:-toolforge-1258585472}
asset_base=${PLUGIN_ASSET_BASE_URL:-https://plugins.toolforge.devlive.org}
stats_base=${PLUGIN_STATS_BASE_URL:-https://stats.toolforge.devlive.org}
[[ $bucket =~ ^[a-z0-9-]+-[0-9]+$ && $asset_base =~ ^https://[a-zA-Z0-9.-]+$ && $stats_base =~ ^https://[a-zA-Z0-9.-]+$ ]] || { echo 'Invalid publication configuration' >&2; exit 1; }
[[ $PLUGIN_CATALOG_SSH_TARGET =~ ^[a-z_][a-z0-9_-]*@[a-zA-Z0-9.-]+$ ]] || { echo 'Invalid SSH target' >&2; exit 1; }
[[ $GITHUB_REPOSITORY =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || exit 1
[[ -x $xtask && -f $COS_CONFIG_PATH && -f $PLUGIN_SSH_KEY_PATH ]] || { echo 'Missing publishing prerequisites' >&2; exit 1; }
"$xtask" plugin verify "$dir"
jq -e --arg base "$stats_base" '.schemaVersion == 1 and (.generation > 0) and (.sourceCommit | test("^[a-f0-9]{40}$")) and (.plugins | type == "array")' "$dir/release-catalog.json" >/dev/null
jq -e --arg base "$stats_base" 'all(.plugins[]; .url == ($base + "/v1/plugins/" + .id + "/versions/" + .version + "/download") and .signed == true)' "$dir/registry.json" >/dev/null
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
# Build a full content-addressed upload inventory; every catalog row needs its original bytes.
python3 - "$dir" "$scratch/inventory.tsv" <<'PY'
import hashlib, json, pathlib, sys
root, target = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
files = {}
for path in root.rglob('*.tfp'):
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    files[digest] = path
catalog = json.loads((root / 'release-catalog.json').read_text())
with target.open('w') as output, target.with_name('mirror.tsv').open('w') as mirror:
    for item in catalog['plugins']:
        digest = item['sha256']
        path = files.get(digest)
        if path is None or path.stat().st_size != item['size']:
            raise SystemExit('Missing or changed historical package')
        expected = f"packages/{item['id']}/{item['version']}/{digest}.tfp"
        if item['objectKey'] != expected or any(c in str(path) + expected for c in '\t\r\n'):
            raise SystemExit('Invalid immutable object path')
        output.write(f'{path}\t{expected}\t{digest}\t{item["size"]}\n')
        mirror.write(f'{path}\t{item["id"]}-{item["version"]}.tfp\n')
PY
read_object() {
  # The bucket is private: read through authenticated COSCLI, never anonymous HTTP.
  rm -f "$scratch/readback"
  "$coscli" cp "cos://$bucket/$1" "$scratch/readback" -c "$COS_CONFIG_PATH" --disable-log --fail-output=false --process-log=false >/dev/null 2>&1
}
check_readback() {
  # $1=key $2=digest $3=size
  [[ $(wc -c < "$scratch/readback" | tr -d ' ') == "$3" ]] && printf '%s  %s\n' "$2" "$scratch/readback" | sha256sum --check --status
}
put_immutable() {
  local source=$1 key=$2 digest=$3 size=$4
  if read_object "$key" && check_readback "$key" "$digest" "$size"; then
    return
  fi
  # COS also enforces non-overwrite, protecting against concurrent writers between GET and PUT.
  if ! "$coscli" cp "$source" "cos://$bucket/$key" -c "$COS_CONFIG_PATH" --disable-log --fail-output=false --process-log=false --forbid-overwrite --meta 'Cache-Control:public,max-age=31536000,immutable'; then
    echo 'Upload did not complete; checking whether an identical object already exists' >&2
  fi
  read_object "$key" || { echo "COS readback verification failed: $key" >&2; exit 1; }
  check_readback "$key" "$digest" "$size" || { echo "COS readback verification failed: $key" >&2; exit 1; }
}
while IFS=$'\t' read -r source key digest size; do
  put_immutable "$source" "$key" "$digest" "$size"
done < "$scratch/inventory.tsv"
for file in release-catalog registry; do
  digest=$(sha256sum "$dir/$file.json" | cut -d ' ' -f 1)
  size=$(wc -c < "$dir/$file.json" | tr -d ' ')
  put_immutable "$dir/$file.json" "snapshots/$file/$digest.json" "$digest" "$size"
done
catalog_digest=$(sha256sum "$dir/release-catalog.json" | cut -d ' ' -f 1)
ssh -T -i "$PLUGIN_SSH_KEY_PATH" -o BatchMode=yes -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new "$PLUGIN_CATALOG_SSH_TARGET" "sudo -n /usr/local/sbin/toolforge-plugin-stats-deploy catalog $catalog_digest" < "$dir/release-catalog.json"
# Import rejects stale generations. Mutable pointers are updated only after its
# success. They stay publicly readable (object ACL) because installed apps fetch
# them anonymously; the bucket itself remains private.
for file in release-catalog registry; do
  "$coscli" cp "$dir/$file.json" "cos://$bucket/$file.json" -c "$COS_CONFIG_PATH" --disable-log --fail-output=false --process-log=false --acl public-read --meta 'Content-Type:application/json#Cache-Control:public,max-age=60,must-revalidate'
done
# Retain a GitHub mirror for installed older clients. Never clobber .tfp release assets.
release_id=$(jq -r '.id // empty' "$dir/previous-release.json")
if [[ -z $release_id ]]; then
  gh release create plugins --repo "$GITHUB_REPOSITORY" --prerelease --title Plugins --notes 'Official signed ToolForge plugins. Downloads are routed through the statistics API.'
fi
mkdir -p "$scratch/mirror"
while IFS=$'\t' read -r source name; do
  if jq -e --arg name "$name" 'any(.assets[]; .name == $name)' "$dir/previous-release.json" >/dev/null; then
    continue
  fi
  # A recovered COS-only historical package also needs its interrupted GitHub mirror completed.
  cp "$source" "$scratch/mirror/$name"
  gh release upload plugins "$scratch/mirror/$name" --repo "$GITHUB_REPOSITORY"
done < "$scratch/mirror.tsv"
gh release upload plugins "$dir/registry.json" "$dir/release-catalog.json" --repo "$GITHUB_REPOSITORY" --clobber
echo 'Published verified COS objects, imported catalog, registry, and compatibility mirror'
