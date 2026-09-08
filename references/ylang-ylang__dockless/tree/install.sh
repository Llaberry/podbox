#!/usr/bin/env bash
# dockless install.sh — 无 docker 平台（无 DinD/userns）的 docker 补丁安装器。
#
# 检测链：
#   1. python3            —— 缺失则 apt 安装 python3 python3-pip
#   2. 本机真实 docker    —— 存在则 warn（dockless 会 shadow PATH；docker 可用主机不需要它）
#   3. pip / udocker      —— pip 装 udocker；失败/离线则提示 vendor 兜底
#   4. 引擎包             —— 离线安装 vendor/udocker-englib-*.tar.gz（UDOCKER_TARBALL 指向本地文件）
#   5. PATH 注册          —— bin/docker、bin/dockless-compose 软链到 ~/.local/bin
#   6. 验收               —— `docker --help` 必须退出 0
#
# 用法：
#   ./install.sh          完整安装
#   ./install.sh --dry-run  只跑到检测逻辑层，打印计划，不落盘
set -euo pipefail

DOCKLESS_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DRY_RUN=0
[[ "${1:-}" == "--dry-run" ]] && DRY_RUN=1

say()  { printf '\033[1;34m[dockless]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[dockless warn]\033[0m %s\n' "$*"; }
die()  { printf '\033[1;31m[dockless error]\033[0m %s\n' "$*" >&2; exit 1; }

# run: dry-run 模式下只打印计划，否则真正执行
run() {
  if [ "$DRY_RUN" -eq 1 ]; then
    printf '\033[1;36m[dockless plan]\033[0m %s\n' "$*"
  else
    "$@"
  fi
}

# pip 安装 udocker：默认源失败后回退镜像源（S3 慢源与墙：
# gpu8 实测 files.pythonhosted.org 仅 ~12KB/s，清华镜像 7-9 分钟完成）
PIP_INDEX_FALLBACK="${DOCKLESS_PIP_INDEX:-https://pypi.tuna.tsinghua.edu.cn/simple}"
pip_install_udocker() {
  if run "$PYTHON" -m pip install --user udocker; then
    return 0
  fi
  warn "默认 pip 源安装失败，回退镜像: $PIP_INDEX_FALLBACK（可 DOCKLESS_PIP_INDEX 覆盖）"
  run "$PYTHON" -m pip install --user -i "$PIP_INDEX_FALLBACK" udocker
}

say "dockless install (dry-run=$DRY_RUN) root=$DOCKLESS_ROOT"

# ------------------------------------------------------- 0. 前置预检（S4 资源预估缺失）
# gpu8 复盘：root 盘仅 30G 被 4 个 venv（~26G）撑爆，部署被迫迁到数据盘
# /root/autodl-tmp（2TB）。装到一半才爆盘是典型「资源预估缺失」事故，
# 预检在下载/安装前 fail-fast。
# 大盘自动选择（S4 延伸）：镜像/容器状态可能几十 GB，默认放可用空间最大的
# 候选数据盘，而不是盲目用 $HOME。UDOCKER_DIR 显式设置时尊重用户。
if [ -z "${UDOCKER_DIR:-}" ]; then
  _best_dir="$HOME/.udocker"; _best_kb=0
  for _cand in ${DOCKLESS_DATA_ROOT:+"$DOCKLESS_DATA_ROOT"} /root/autodl-tmp /data /mnt/data "$HOME"; do
    [ -d "$_cand" ] && [ -w "$_cand" ] || continue
    _kb="$(df -Pk "$_cand" 2>/dev/null | awk 'NR==2 {print $4}')"
    [ -n "$_kb" ] && [ "$_kb" -gt "$_best_kb" ] && { _best_kb="$_kb"; _best_dir="$_cand/.udocker"; }
  done
  UDOCKER_DIR="$_best_dir"
  say "UDOCKER_DIR 自动选择: $UDOCKER_DIR（可用 $((_best_kb / 1024 / 1024))G；DOCKLESS_DATA_ROOT 或 UDOCKER_DIR 可覆盖）"
fi
MIN_FREE_GB="${DOCKLESS_MIN_FREE_GB:-5}"
# UDOCKER_DIR 可能尚不存在：取其最近存在的祖先做 df 目标
_disk_path="$UDOCKER_DIR"
while [ ! -e "$_disk_path" ] && [ "$_disk_path" != "/" ]; do
  _disk_path="$(dirname "$_disk_path")"
done
_avail_kb="$(df -Pk "$_disk_path" 2>/dev/null | awk 'NR==2 {print $4}' || true)"
if [ -n "$_avail_kb" ]; then
  _avail_gb=$((_avail_kb / 1024 / 1024))
  if [ "$_avail_gb" -lt "$MIN_FREE_GB" ]; then
    die "磁盘空间不足：UDOCKER_DIR=$UDOCKER_DIR 所在盘仅 ${_avail_gb}G 可用（要求 ≥ ${MIN_FREE_GB}G，DOCKLESS_MIN_FREE_GB 可调）。gpu8 教训：root 盘 30G 曾撑爆；请把 UDOCKER_DIR 指到大盘（如 /root/autodl-tmp/udocker3）。"
  fi
  say "ok: 磁盘 ${_avail_gb}G 可用（UDOCKER_DIR=$UDOCKER_DIR，阈值 ${MIN_FREE_GB}G）"
else
  warn "无法读取磁盘剩余空间（df 失败），跳过磁盘预检"
fi
# 能力检测：CAP_MKNOD（hooks/*.sh 的 mknod 设备节点需要；缺失时 warn 而非 fail，
# 因为不是所有镜像都需要设备节点）
if [ "$(id -u)" -eq 0 ]; then
  _capeff="$(awk '/^CapEff:/ {print $2}' /proc/self/status 2>/dev/null)"
  if [ -n "$_capeff" ] && [ $((0x$_capeff & (1 << 27))) -eq 0 ]; then
    warn "缺少 CAP_MKNOD：需要设备节点的镜像（如 postgres 的 null/random/urandom）hook 会失败；可给容器方案换 chroot 或调整 hook"
  fi
fi

# ---------------------------------------------------------------- 1. python3
if command -v python3 >/dev/null 2>&1; then
  say "ok: python3 = $(command -v python3) ($(python3 --version 2>&1))"
elif [ "$(id -u)" -ne 0 ]; then
  # 无 python3 且非 root：apt 需要 sudo
  if command -v sudo >/dev/null 2>&1; then
    warn "python3 缺失，将通过 sudo apt 安装"
    run sudo apt-get update
    run sudo apt-get install -y python3 python3-pip
  else
    die "python3 缺失且无 sudo/root，无法 apt 安装；请先安装 python3"
  fi
else
  warn "python3 缺失，将通过 apt 安装"
  run apt-get update
  run apt-get install -y python3 python3-pip
fi
PYTHON="$(command -v python3)"

# ----------------------------------------------------- 2. 本机真实 docker
if command -v docker >/dev/null 2>&1; then
  warn "检测到本机已有 docker（$(command -v docker)，$(docker --version 2>/dev/null || echo '?')）"
  warn "dockless 定位是无 DinD/userns 的平台补丁；有 docker 的主机不需要它。"
  warn "安装后 bin/docker 会 shadow PATH 上的真实 docker（~/.local/bin 优先时）。"
  warn "若确认要继续：PATH 中真实 docker 将不可见（除非用绝对路径 /usr/bin/docker）。"
fi

# ----------------------------------------------------------- 3. pip / udocker
UDOCKER_BIN="$(command -v udocker || true)"
if [ -z "$UDOCKER_BIN" ] && "$PYTHON" -c 'import udocker' >/dev/null 2>&1; then
  # pip --user 装的模块：可执行文件通常在 ~/.local/bin（可能不在 PATH）
  [ -x "$HOME/.local/bin/udocker" ] && UDOCKER_BIN="$HOME/.local/bin/udocker"
fi
if [ -n "$UDOCKER_BIN" ]; then
  say "ok: udocker = $UDOCKER_BIN"
else
  warn "未发现 udocker，尝试 pip 安装（默认源失败自动回退镜像，见 S3）"
  if command -v pip3 >/dev/null 2>&1; then
    pip_install_udocker
  else
    warn "pip3 缺失，尝试 python3 -m pip"
    pip_install_udocker
  fi
  if [ "$DRY_RUN" -eq 1 ]; then
    # dry-run：未真正安装，假设成功，继续打印后续计划
    UDOCKER_BIN="udocker"
    say "（dry-run：假定 pip 安装成功，继续计划）"
  else
    UDOCKER_BIN="$(command -v udocker || true)"
    if [ -z "$UDOCKER_BIN" ] && [ -x "$HOME/.local/bin/udocker" ]; then
      UDOCKER_BIN="$HOME/.local/bin/udocker"
    fi
    if [ -z "$UDOCKER_BIN" ]; then
      # vendor 兜底：englib tarball 只含引擎工具/库，不含 udocker 本体；
      # 离线场景需另行提供 udocker 发行包（udocker-<ver>.tar.gz 或 *.whl）放入 vendor/。
      warn "pip 装 udocker 失败（大概率离线）。vendor 兜底："
      warn "  1. 把 udocker 发行包放入 vendor/（udocker-*.tar.gz 解压即用，或 *.whl 走 pip）"
      warn "  2. 引擎包已就位：vendor/$(basename "${ENGINE_TARBALL:-udocker-englib-*.tar.gz}")"
      die "udocker 不可用，安装中止（引擎包离线安装未执行）"
    fi
    say "ok: udocker 安装成功 = $UDOCKER_BIN"
  fi
fi

# ---------------------------------------------------- 4. 引擎包（多源下载，S3 慢源与墙）
# 优先级：vendor/ 本地离线包 → DOCKLESS_ENGINE_URL（本地路径或 URL）
#   → udocker --version 声明的官方 tarball URL（SSOT，版本跟随 udocker）
#   → 在线 udocker install（已知慢源 ~30KB/s，最后兜底）
fetch_engine_tarball() {
  local src="$1" dst
  case "$src" in
    http://*|https://*)
      if command -v curl >/dev/null 2>&1; then
        mkdir -p "$DOCKLESS_ROOT/vendor"
        dst="$DOCKLESS_ROOT/vendor/udocker-englib-downloaded.tar.gz"
        say "下载引擎包: $src"
        run curl -fL --retry 3 --connect-timeout 15 -o "$dst" "$src" || return 1
        echo "$dst"
      else
        warn "curl 不可用，跳过 URL 源: $src"
        return 1
      fi
      ;;
    *) echo "$src" ;;
  esac
}

ENGINE_TARBALL="${ENGINE_TARBALL:-}"
if [ -z "$ENGINE_TARBALL" ]; then
  ENGINE_TARBALL="$(ls "$DOCKLESS_ROOT"/vendor/udocker-englib-*.tar.gz 2>/dev/null | head -1 || true)"
fi
if [ -z "$ENGINE_TARBALL" ] && [ -n "${DOCKLESS_ENGINE_URL:-}" ]; then
  ENGINE_TARBALL="$(fetch_engine_tarball "$DOCKLESS_ENGINE_URL" || true)"
fi
if [ -z "$ENGINE_TARBALL" ]; then
  # udocker --version 会打印它期望的 englib tarball URL（官方源，国内可能 <100KB/s）
  OFFICIAL_URLS="$("${UDOCKER_BIN:-udocker}" --version 2>/dev/null | sed -n 's/^tarball: //p' || true)"
  for url in $OFFICIAL_URLS; do
    say "尝试官方引擎包源: $url"
    ENGINE_TARBALL="$(fetch_engine_tarball "$url" || true)"
    [ -n "$ENGINE_TARBALL" ] && break
  done
fi
if [ -n "$ENGINE_TARBALL" ]; then
  say "引擎包离线安装: UDOCKER_TARBALL=$ENGINE_TARBALL udocker install"
  if [ "$DRY_RUN" -eq 0 ]; then
    UDOCKER_TARBALL="$ENGINE_TARBALL" "${UDOCKER_BIN:-udocker}" install
  fi
else
  warn "vendor/ 无引擎包、DOCKLESS_ENGINE_URL 与官方源均不可达；回退在线 udocker install（已知慢源 ~30KB/s）"
  run "${UDOCKER_BIN:-udocker}" install
fi

# ------------------------------------------------------- 5. PATH 注册
if [ "$DRY_RUN" -eq 0 ]; then
  mkdir -p "$HOME/.local/bin"
fi
run ln -sf "$DOCKLESS_ROOT/bin/docker"          "$HOME/.local/bin/docker"
run ln -sf "$DOCKLESS_ROOT/bin/dockless-compose" "$HOME/.local/bin/dockless-compose"
say "PATH 注册：确保 ~/.local/bin 在 PATH 前部（多数发行版默认在）。"

# ----------------------------------------------------------- 6. 验收
say "验收：docker --help"
if [ "$DRY_RUN" -eq 0 ]; then
  DOCKLESS_ROOT="$DOCKLESS_ROOT" "$HOME/.local/bin/docker" --help >/dev/null \
    && say "ok: docker --help 退出 0，dockless 就绪" \
    || die "docker --help 失败"
else
  say "（dry-run 不执行验收）"
fi

say "完成。上手：docker compose up -d（compose 子集见 README）；docker ps / docker logs 走 udocker 透传。"
