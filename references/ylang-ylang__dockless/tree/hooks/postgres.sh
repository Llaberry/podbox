# per-image hook declaration: postgres (docker.io/library/postgres:17)
# chroot-poc hook contract:
#   hook_apply <rootfs> [data_dir]  -- run BEFORE initdb/start; applies quirks
#   hook_env                        -- runtime env vars, printed as KEY=VAL per line

HOOK_IMAGE="postgres:17"
HOOK_USER="postgres"
HOOK_UID=999
HOOK_GID=999
HOOK_BIN_DIR="/usr/lib/postgresql/17/bin"

# device nodes postgres needs inside chroot (created with mknod -m 666)
HOOK_DEV_NODES="null zero random urandom"

# postgresql.conf quirks appended at init time, "key=value"
#  - dynamic_shared_memory_type=mmap : chroot has no /dev/shm (posix shm would fail)
#  - unix_socket_directories=/tmp    : host /var/run not visible; /tmp is in-rootfs
#  - listen_addresses=0.0.0.0        : host network, bind directly
HOOK_PGCONF=(
  "dynamic_shared_memory_type=mmap"
  "listen_addresses='0.0.0.0'"
  "port=5432"
  "unix_socket_directories='/tmp'"
  "logging_collector=off"
)

hook_apply() {
  local rootfs="$1" data_dir="${2:-/data}"
  mkdir -p "$rootfs$data_dir" "$rootfs/tmp" "$rootfs/var/run/postgresql"
  chmod 1777 "$rootfs/tmp"
  chown -R "$HOOK_UID:$HOOK_GID" "$rootfs$data_dir" "$rootfs/var/run/postgresql"
}

hook_env() {
  echo "POSTGRES_PASSWORD=${POSTGRES_PASSWORD:?set POSTGRES_PASSWORD}"
  echo "PGDATA=/data"
}
