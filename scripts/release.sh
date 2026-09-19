#!/bin/bash
# One-shot release. Bumps the version everywhere, commits, tags, pushes, waits
# for the Release workflow to build macOS + Windows on GitHub, publishes the
# draft it creates, and verifies that the updater manifest is live.
#
#   scripts/release.sh 1.0.1            # notes from docs/marketing/release-notes-v1.0.1.md if present
#   scripts/release.sh 1.0.1 notes.md   # explicit release notes
set -euo pipefail
cd "$(dirname "$0")/.."
version="${1:?usage: release.sh X.Y.Z [notes.md]}"
tag="v$version"
repo="jason-jm/neat-nas"
notes="${2:-docs/marketing/release-notes-$tag.md}"

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must look like 1.2.3"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "commit or stash your changes first"; git status --short; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "gh is not logged in"; exit 1; }
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && { echo "tag $tag already exists"; exit 1; }

echo "== bumping to $version"
python3 - "$version" <<'PY'
import json, pathlib, re, sys
v = sys.argv[1]
p = pathlib.Path("package.json"); d = json.loads(p.read_text()); d["version"] = v; p.write_text(json.dumps(d, indent=2, ensure_ascii=False) + "\n")
p = pathlib.Path("src-tauri/Cargo.toml"); s = p.read_text()
p.write_text(re.sub(r'^version = "[^"]+"', f'version = "{v}"', s, count=1, flags=re.M))
p = pathlib.Path("src-tauri/tauri.conf.json"); s = p.read_text()
p.write_text(re.sub(r'"version": "[^"]+"', f'"version": "{v}"', s, count=1))
PY
(cd src-tauri && cargo update -w --offline >/dev/null 2>&1 || cargo update -w >/dev/null)
git add package.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -q -m "Release $version"
git tag -a "$tag" -m "Neat NAS $version"
git push
git push origin "$tag"

echo "== waiting for the Release workflow"
run_id=""
for _ in $(seq 1 30); do
  run_id=$(gh run list --repo "$repo" --workflow Release --branch "$tag" --limit 1 --json databaseId --jq '.[0].databaseId // empty')
  [ -n "$run_id" ] && break
  sleep 10
done
[ -n "$run_id" ] || { echo "the Release workflow did not start; check https://github.com/$repo/actions"; exit 1; }
echo "run: https://github.com/$repo/actions/runs/$run_id"
for _ in $(seq 1 180); do
  status=$(gh run view "$run_id" --repo "$repo" --json status --jq .status)
  [ "$status" = "completed" ] && break
  sleep 20
done
conclusion=$(gh run view "$run_id" --repo "$repo" --json conclusion --jq .conclusion)
[ "$conclusion" = "success" ] || { echo "workflow finished with: $conclusion"; gh run view "$run_id" --repo "$repo" --log-failed | tail -40; exit 1; }

echo "== publishing the draft release"
[ -f "$notes" ] && gh release edit "$tag" --repo "$repo" --notes-file "$notes" >/dev/null
gh release edit "$tag" --repo "$repo" --draft=false --latest >/dev/null
gh release view "$tag" --repo "$repo" --json url,assets --jq '.url, (.assets[] | "  \(.name)")'

echo "== verifying the updater manifest"
for _ in $(seq 1 12); do
  live=$(curl -sL "https://github.com/$repo/releases/latest/download/latest.json" | python3 -c 'import json,sys; print(json.load(sys.stdin)["version"])' 2>/dev/null || true)
  [ "$live" = "$version" ] && break
  sleep 5
done
[ "$live" = "$version" ] || { echo "latest.json still reports '$live'"; exit 1; }
echo "released $tag: updater manifest live, website download buttons follow automatically"
