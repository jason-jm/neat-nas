#!/bin/bash
# One-shot release.
#
#   scripts/release.sh 1.0.2            # notes from docs/marketing/release-notes-v1.0.2.md if present
#   scripts/release.sh 1.0.2 notes.md   # explicit release notes
#
# 1. Bumps the version in package.json, Cargo.toml/Cargo.lock and
#    tauri.conf.json, commits, tags and pushes.
# 2. The Release workflow builds the Windows installer and zip on GitHub into a
#    draft release, while this Mac builds the macOS app, signs it with the
#    Developer ID in the keychain and has Apple notarize it (build-mac.sh);
#    the signing key never leaves this Mac.
# 3. Uploads the macOS files to the draft and adds them to latest.json.
# 4. Copies the release into release/<version>/, checks latest.json, attaches
#    SHA256SUMS.txt, publishes the draft and verifies the live manifest.
set -euo pipefail
cd "$(dirname "$0")/.."
source "$HOME/.cargo/env" 2>/dev/null || true
version="${1:?usage: release.sh X.Y.Z [notes.md]}"
tag="v$version"
repo="jason-jm/neat-nas"
notes="${2:-docs/marketing/release-notes-$tag.md}"
dir="release/$version"

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "version must look like 1.2.3"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "commit or stash your changes first"; git status --short; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "gh is not logged in"; exit 1; }
git rev-parse -q --verify "refs/tags/$tag" >/dev/null && { echo "tag $tag already exists"; exit 1; }
scripts/build-mac.sh --preflight

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

mac_log="$(mktemp -d)/build-mac.log"
echo "== building, signing and notarizing the macOS app on this Mac (log: $mac_log)"
# Own process group, so an early exit can stop the whole build (npm, cargo, notarytool).
set -m
scripts/build-mac.sh > "$mac_log" 2>&1 &
mac_pid=$!
set +m
trap 'kill -- -"$mac_pid" 2>/dev/null || true' EXIT

echo "== waiting for the Release workflow (Windows)"
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

echo "== waiting for the macOS build"
if ! wait "$mac_pid"; then echo "the macOS build failed, so the draft stays unpublished:"; tail -n 40 "$mac_log"; exit 1; fi
trap - EXIT
grep -E "Accepted|accepted|source=" "$mac_log" | sed 's/^ */  /'

echo "== adding the macOS files to the draft"
gh release upload "$tag" --repo "$repo" --clobber \
  "$dir/Neat NAS_${version}_universal.dmg" "$dir/Neat NAS_${version}_universal-mac.zip" \
  "$dir/updater/Neat NAS.app.tar.gz" "$dir/updater/Neat NAS.app.tar.gz.sig"
manifest="$(mktemp -d)/latest.json"
gh release download "$tag" --repo "$repo" --pattern latest.json --output "$manifest" --clobber || true
python3 - "$manifest" "$version" "$dir/updater/Neat NAS.app.tar.gz.sig" "https://github.com/$repo/releases/download/$tag/Neat.NAS.app.tar.gz" <<'PY'
import datetime, json, os, sys
path, version, sig_path, url = sys.argv[1:]
if os.path.exists(path):
    manifest = json.load(open(path))
else:  # the workflow normally writes it with the Windows entries
    manifest = {"version": version, "notes": f"Neat NAS {version}",
                "pub_date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), "platforms": {}}
entry = {"signature": open(sig_path).read().strip(), "url": url}
for key in ("darwin-aarch64", "darwin-x86_64", "darwin-aarch64-app", "darwin-x86_64-app"):
    manifest.setdefault("platforms", {})[key] = entry
open(path, "w").write(json.dumps(manifest, indent=2) + "\n")
PY
gh release upload "$tag" "$manifest" --repo "$repo" --clobber

echo "== copying the release into $dir and attaching SHA256SUMS.txt"
scripts/collect-release.sh github "$tag"

echo "== checking the updater manifest before publishing"
assets=$(gh release view "$tag" --repo "$repo" --json assets --jq '.assets[].name')
python3 - "release/$version/updater/latest.json" "$version" "$tag" "$repo" "$assets" <<'PY'
import json, sys, urllib.parse
path, version, tag, repo, assets = sys.argv[1:]
assets = set(assets.split("\n"))
m = json.load(open(path))
plat = m.get("platforms", {})
problems = []
if m.get("version") != version:
    problems.append(f"version is {m.get('version')!r}, expected {version}")
prefix = f"https://github.com/{repo}/releases/download/{tag}/"
for key, suffix in {"darwin-aarch64": ".app.tar.gz", "darwin-x86_64": ".app.tar.gz", "windows-x86_64": "-setup.exe"}.items():
    entry = plat.get(key)
    if not entry:
        problems.append(f"no {key} entry")
        continue
    url = entry.get("url", "")
    name = urllib.parse.unquote(url.rsplit("/", 1)[-1])
    if not entry.get("signature"):
        problems.append(f"{key} has no signature")
    if not url.startswith(prefix):
        problems.append(f"{key} does not point at {tag}: {url}")
    if not name.endswith(suffix):
        problems.append(f"{key} is not a {suffix} file: {name}")
    if name not in assets:
        problems.append(f"{key} points at {name}, which the release does not have")
if problems:
    print("latest.json is not right, so the draft stays unpublished:\n  " + "\n  ".join(problems))
    sys.exit(1)
print("latest.json ok: " + ", ".join(sorted(plat)))
PY

echo "== checking the updater signatures with the public key built into the app"
command -v minisign >/dev/null || { echo "minisign is needed for this check: brew install minisign"; exit 1; }
python3 - "release/$version/updater/latest.json" "$dir" <<'PY'
import base64, json, os, subprocess, sys, tempfile, urllib.parse
path, root = sys.argv[1:]
work = tempfile.mkdtemp()
pub = os.path.join(work, "pub.key")
open(pub, "w").write(base64.b64decode(json.load(open("src-tauri/tauri.conf.json"))["plugins"]["updater"]["pubkey"]).decode())
bad = []
for key, entry in sorted(json.load(open(path))["platforms"].items()):
    name = urllib.parse.unquote(entry["url"].rsplit("/", 1)[-1]).replace("Neat.NAS", "Neat NAS", 1)
    file = os.path.join(root, "updater" if name.endswith(".app.tar.gz") else "", name)
    sig = os.path.join(work, key + ".minisig")
    open(sig, "wb").write(base64.b64decode(entry["signature"]))
    if subprocess.run(["minisign", "-V", "-q", "-p", pub, "-m", file, "-x", sig]).returncode != 0:
        bad.append(key)
if bad:
    print("these updater signatures do not verify, so the draft stays unpublished: " + ", ".join(bad))
    sys.exit(1)
print("all updater signatures verify")
PY
gh release upload "$tag" "release/$version/SHA256SUMS.txt" --repo "$repo" --clobber

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
