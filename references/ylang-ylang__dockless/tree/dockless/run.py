#!/usr/bin/env python3
"""dockless.run — `docker run` 拦截：语义护栏 + env 白名单后透传 udocker。

复盘来源（gpu8 全部署复盘，结构问题见 ego-anno@WTs/docs/native-deploy-proposal.md）：
- S1 容器语义缺失：udocker `run --rm` 遇到**已存在容器**会在退出时删除该容器
  元数据（gpu8 曾误删 pg17 容器，数据因 bind-mount 无损）；`--name` 仅 create
  支持，run 传 --name 报 invalid container name。本模块在透传前 fail-fast，
  把 docker 心智与 udocker 语义的差异变成响亮报错而非静默误操作。
- S2 环境污染传染：`--hostenv` 全量透传宿主环境变量（TZ=Asia/Shanghai 曾传染
  进 postgres 容器，导致 hatchet seed DATABASE_ENFORCE_UTC_TIMEZONE panic）。
  本模块把 --hostenv 替换为显式白名单 DOCKLESS_HOSTENV_ALLOWLIST（默认空：
  一个宿主变量都不传），容器需要的变量用 -e/--env 显式给出。

用法（由 bin/docker 调用；bash 侧负责解析 udocker 路径与 root 判定）：
    DOCKLESS_UDOCKER=<udocker> DOCKLESS_ALLOW_ROOT=0|1 \
        PYTHONPATH=<repo> python3 -m dockless.run <docker run 的全部参数>

除 env 相关参数与护栏外，其余参数原样透传 udocker（保持骨架透传语义）。
"""

from __future__ import annotations

import os
import re
import subprocess
import sys

# docker run 中带值的 flag（用于定位 IMAGE 与 --rm 目标）。
# 不全量覆盖也安全：解析失败的兜底是 warn（见 guard_run），不会静默放行。
_VALUE_FLAGS = {
    "--add-host", "--cap-add", "--cap-drop", "--cgroupns", "--cidfile",
    "--cpu-shares", "--cpus", "--cpuset-cpus", "--cpuset-mems",
    "--device", "--dns", "--dns-option", "--dns-search", "--domainname",
    "--entrypoint", "--env", "--env-file", "--expose", "--group-add",
    "--health-cmd", "--health-interval", "--health-retries",
    "--health-start-interval", "--health-start-period", "--health-timeout",
    "--hostname", "--ip", "--ip6", "--ipc", "--label", "--log-driver",
    "--log-opt", "--mac-address", "--memory", "--mount", "--name",
    "--network", "--network-alias", "--pid", "--platform", "--pids-limit",
    "--publish", "--pull", "--restart", "--runtime", "--security-opt",
    "--shm-size", "--stop-signal", "--stop-timeout", "--storage-opt",
    "--sysctl", "--tmpfs", "--ulimit", "--user", "--userns", "--uts",
    "--volume", "--volume-driver", "--volumes-from", "--workdir",
    "-e", "-l", "-m", "-p", "-u", "-v", "-w",
}

# 短 flag 附着值形式：-eFOO=1 / -p8080:80 / -v/a:/b（docker 允许）
_SHORT_VALUE_PREFIXES = {"e", "l", "m", "p", "u", "v", "w"}


def split_run_args(args: list[str]) -> tuple[dict[str, list[str]], list[str]]:
    """把 docker run 参数拆成 {flag: [values]} 与 positionals（IMAGE/CMD）。

    只关心：--rm 是否出现、IMAGE（第一个 positional）。
    带值 flag 消费下一个 token；--flag=value 形式不消费。
    """
    flags: dict[str, list[str]] = {}
    positionals: list[str] = []
    i = 0
    while i < len(args):
        a = args[i]
        if a == "--":
            positionals.extend(args[i + 1:])
            break
        if a.startswith("--") and "=" in a:
            name, _, value = a.partition("=")
            flags.setdefault(name, []).append(value)
            i += 1
            continue
        if a.startswith("--"):
            if a in _VALUE_FLAGS:
                if i + 1 < len(args) and not args[i + 1].startswith("-"):
                    flags.setdefault(a, []).append(args[i + 1])
                    i += 2
                else:
                    flags.setdefault(a, []).append("")
                    i += 1
            else:
                flags.setdefault(a, [])
                i += 1
            continue
        if a.startswith("-") and a != "-":
            if len(a) > 2 and a[1] in _SHORT_VALUE_PREFIXES:
                flags.setdefault(a[:2], []).append(a[2:])
                i += 1
                continue
            if a in _VALUE_FLAGS:
                if i + 1 < len(args) and not args[i + 1].startswith("-"):
                    flags.setdefault(a, []).append(args[i + 1])
                    i += 2
                else:
                    flags.setdefault(a, []).append("")
                    i += 1
            else:
                flags.setdefault(a, [])
                i += 1
            continue
        positionals.append(a)
        i += 1
    return flags, positionals


def _existing_container_names() -> set[str] | None:
    """udocker ps 的 NAMES 列（['pg17'] 形式）→ 容器名集合。

    查询失败（udocker 未装/超时）返回 None：护栏降级为 warn 而非阻塞——
    run 本身也会因 udocker 缺失而失败，阻塞无意义。
    """
    udocker = os.environ.get("DOCKLESS_UDOCKER", "udocker")
    allow_root = ["--allow-root"] if os.environ.get("DOCKLESS_ALLOW_ROOT") == "1" else []
    try:
        out = subprocess.run(
            [udocker, *allow_root, "ps"],
            capture_output=True, text=True, timeout=20,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if out.returncode != 0:
        return None
    names: set[str] = set()
    for m in re.finditer(r"\[([^\]]*)\]", out.stdout):
        for part in m.group(1).split(","):
            part = part.strip().strip("'\"").strip()
            if part:
                names.add(part)
    return names


def guard_run(flags: dict[str, list[str]], image: str | None) -> None:
    """S1 护栏：--rm 命中已存在容器 / --name 传 run → fail-fast。"""
    if "--name" in flags:
        sys.stderr.write(
            "dockless: docker run --name 在 udocker 下无效（udocker 报 invalid "
            "container name，仅 create 支持）。\n"
            "dockless: run 请直接给镜像/容器名；需要命名请先 udocker create --name。"
            "fail-fast。\n"
        )
        sys.exit(2)
    if "--rm" not in flags:
        return
    if image is None:
        sys.stderr.write(
            "dockless warn: run --rm 无法确定目标名（参数无法解析）。udocker 对"
            "已存在容器加 --rm 会在退出时删除其元数据（gpu8 曾误删 pg17 容器），"
            "服务容器严禁 --rm。\n"
        )
        return
    existing = _existing_container_names()
    if existing is None:
        sys.stderr.write(
            "dockless warn: 无法查询 udocker 容器列表（护栏降级）。--rm 用于已存在"
            "容器会在退出时删除其元数据，服务容器严禁 --rm。\n"
        )
        return
    if image in existing:
        sys.stderr.write(
            f"dockless: run --rm 目标 '{image}' 是已存在容器 —— udocker 会在退出时"
            "删除该容器元数据（gpu8 曾误删 pg17 容器）。\n"
            "dockless: 服务容器严禁 --rm；一次性命令请用镜像名"
            "（udocker run --rm <image> <cmd>）。fail-fast。\n"
        )
        sys.exit(2)


def _read_env_file(path: str) -> list[str]:
    """docker --env-file：每行 KEY=VAL，# 注释，容忍 export 前缀。"""
    if not os.path.isfile(path):
        sys.stderr.write(f"dockless: --env-file {path} 不存在。fail-fast。\n")
        sys.exit(2)
    out: list[str] = []
    with open(path, encoding="utf-8", errors="replace") as fh:
        for raw in fh:
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            if line.startswith("export "):
                line = line[7:].strip()
            if "=" in line:
                out.append(line)
    return out


def rewrite_env_args(flags: dict[str, list[str]]) -> list[str]:
    """S2：--hostenv → 显式白名单；-e/--env/--env-file → --env=KEY=VAL 列表。

    返回 udocker 可用的 env 参数，并原地移除已消费的 flag 键。
    """
    env_pairs: list[str] = []
    for key in ("-e", "--env"):
        for value in flags.pop(key, []):
            if "=" in value:
                env_pairs.append(value)
            else:
                # docker 语义 -e NAME 是透传宿主同名变量；udocker 不支持，
                # 且隐式宿主透传正是 S2 环境污染的根源 —— fail-fast。
                sys.stderr.write(
                    f"dockless: -e/--env {value} 不带 '='（docker 语义是透传宿主同名"
                    "变量，udocker 不支持）。\n"
                    "dockless: 请显式给出 KEY=VAL；宿主变量隐式透传属于环境污染，"
                    "不支持。fail-fast。\n"
                )
                sys.exit(2)
    for path in flags.pop("--env-file", []):
        env_pairs.extend(_read_env_file(path))
    if "--hostenv" in flags:
        flags.pop("--hostenv")
        allow = os.environ.get("DOCKLESS_HOSTENV_ALLOWLIST", "").split()
        for key in allow:
            value = os.environ.get(key)
            if value is not None:
                env_pairs.append(f"{key}={value}")
        shown = "（空：默认一个宿主变量都不传）" if not allow else " ".join(allow)
        sys.stderr.write(
            "dockless: --hostenv 已替换为显式白名单（S2 环境污染，TZ 曾传染致"
            f"hatchet seed panic）：仅透传 DOCKLESS_HOSTENV_ALLOWLIST={shown}；"
            "其余变量用 -e/--env 显式传入。\n"
        )
    return [f"--env={pair}" for pair in env_pairs]


def _filter_consumed(args: list[str]) -> list[str]:
    """从原始参数中剔除 rewrite_env_args 已消费的 token（含其值）。"""
    out: list[str] = []
    i = 0
    while i < len(args):
        a = args[i]
        if a in ("-e", "--env", "--env-file"):
            i += 2  # 值 token 一并跳过（split_run_args 同样消费了它）
            continue
        if a.startswith("-e") or a.startswith("--env=") or a.startswith("--env-file="):
            i += 1
            continue
        if a == "--hostenv":
            i += 1
            continue
        out.append(a)
        i += 1
    return out


def main() -> int:
    args = sys.argv[1:]
    flags, positionals = split_run_args(args)
    guard_run(flags, positionals[0] if positionals else None)  # S1
    env_args = rewrite_env_args(flags)                          # S2
    udocker = os.environ.get("DOCKLESS_UDOCKER", "udocker")
    allow_root = ["--allow-root"] if os.environ.get("DOCKLESS_ALLOW_ROOT") == "1" else []
    cmd = [udocker, *allow_root, "run", *env_args, *_filter_consumed(args)]
    os.execvp(cmd[0], cmd)  # 替换进程，保持 drop-in 语义
    return 127  # execvp 失败时到达（理论上不会）


if __name__ == "__main__":
    raise SystemExit(main())
