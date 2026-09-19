#!/usr/bin/env bash
# Starts a throw-away Samba server on 127.0.0.1:1445 for local testing.
# Runs entirely as your user under .devtools/samba; nothing system-wide changes.
#
#   brew install samba          # once
#   scripts/dev-smb.sh          # start (Ctrl-C to stop)
#   scripts/dev-smb.sh reset    # wipe and recreate the sample shares
#
# Credentials: user = $USER, password = nas-test-pass. Shares: Public (rw), Media (ro).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)/.devtools/samba"
SMBD="$(ls /opt/homebrew/Cellar/samba/*/sbin/samba-dot-org-smbd 2>/dev/null | tail -1 || true)"
[ -n "$SMBD" ] || { echo "samba not found; run: brew install samba" >&2; exit 1; }
PDBEDIT="$(dirname "$SMBD")/../bin/pdbedit"
PASS="${SMB_TEST_PASS:-nas-test-pass}"
PORT="${SMB_TEST_PORT:-1445}"

if [ "${1:-}" = "reset" ]; then rm -rf "$ROOT"; fi

# RPC helper daemons from a previous run keep stale sockets around and make
# share enumeration time out, so clear them along with the runtime state.
pkill -f "[s]amba-dcerpcd" 2>/dev/null || true
pkill -f "[r]pcd_" 2>/dev/null || true
for d in locks cache pid ncalrpc state; do rm -rf "$ROOT/$d"; mkdir -p "$ROOT/$d"; done

if [ ! -f "$ROOT/smb.conf" ]; then
  mkdir -p "$ROOT"/{private,locks,state,cache,pid,log,ncalrpc} \
           "$ROOT/shares/Public/Photos/2024" "$ROOT/shares/Public/文档/项目资料" \
           "$ROOT/shares/Media/Movies" "$ROOT/shares/Media/Music"
  echo "hello from NAS" > "$ROOT/shares/Public/readme.txt"
  echo "中文内容测试" > "$ROOT/shares/Public/文档/说明.txt"
  printf 'name,value\n1,2\n' > "$ROOT/shares/Public/文档/数据表.csv"
  echo "nested" > "$ROOT/shares/Public/文档/项目资料/计划.md"
  printf '%%PDF-1.4\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 400 300]/Contents 4 0 R/Resources<</Font<</F1 5 0 R>>>>>>endobj\n4 0 obj<</Length 60>>stream\nBT /F1 28 Tf 40 150 Td (Neat NAS PDF preview) Tj ET\nendstream\nendobj\n5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\ntrailer<</Root 1 0 R>>\n' > "$ROOT/shares/Public/文档/说明书.pdf"
  cp "$(dirname "$0")/../src-tauri/icons/icon.png" "$ROOT/shares/Public/Photos/2024/neatnas-icon.png" 2>/dev/null || true
  sips -s format jpeg "$ROOT/shares/Public/Photos/2024/neatnas-icon.png" --out "$ROOT/shares/Public/Photos/2024/IMG_0003.jpg" >/dev/null 2>&1 || true
  : > "$ROOT/shares/Public/empty.txt"
  head -c 3000000 /dev/urandom > "$ROOT/shares/Public/Photos/2024/IMG_0001.jpg"
  head -c 1200000 /dev/urandom > "$ROOT/shares/Public/Photos/2024/IMG_0002.jpg"
  head -c 40000000 /dev/urandom > "$ROOT/shares/Media/Movies/sample-40mb.mkv"
  head -c 5000000 /dev/urandom > "$ROOT/shares/Media/Music/track01.flac"
  cat > "$ROOT/smb.conf" <<CONF
[global]
   workgroup = WORKGROUP
   server string = Test NAS
   netbios name = TESTNAS
   server role = standalone server
   smb ports = $PORT
   bind interfaces only = yes
   interfaces = lo0 127.0.0.1/8
   disable netbios = yes
   security = user
   passdb backend = tdbsam:$ROOT/private/passdb.tdb
   private dir = $ROOT/private
   lock directory = $ROOT/locks
   state directory = $ROOT/state
   cache directory = $ROOT/cache
   pid directory = $ROOT/pid
   ncalrpc dir = $ROOT/ncalrpc
   log file = $ROOT/log/smbd.log
   log level = 1
   server min protocol = SMB2_02
   server max protocol = SMB3
   map to guest = Never
   load printers = no
   printing = bsd
   printcap name = /dev/null
   disable spoolss = yes
   unix charset = UTF-8
   # Running as a normal user: skip ACL/ownership fix-ups that need root.
   nt acl support = no
   inherit acls = no
   store dos attributes = no
   map archive = no

[Public]
   path = $ROOT/shares/Public
   comment = Shared documents
   read only = no
   guest ok = no

[Media]
   path = $ROOT/shares/Media
   comment = Movies and music
   read only = yes
   guest ok = no
CONF
  printf '%s\n%s\n' "$PASS" "$PASS" | "$PDBEDIT" -s "$ROOT/smb.conf" -a -u "$USER" -t >/dev/null
  echo "created $ROOT (user=$USER password=$PASS)"
fi

echo "smbd on 127.0.0.1:$PORT  (user=$USER, password=$PASS). Ctrl-C to stop."
exec "$SMBD" --foreground --debug-stdout --no-process-group -s "$ROOT/smb.conf"
