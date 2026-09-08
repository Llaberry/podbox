# dockless

无 DinD / userns 平台的 **docker 补丁**：在既没有 docker 守护进程、也没有内核
容器能力的主机（典型如 gpu8 autodl 容器机）上，用 `udocker` + PRoot 提供
`docker` / `docker compose` 命令语义的子集。

**这不是 docker 的替代品，是补丁**：有 docker 的主机请直接用 docker。

## 背景（为什么有它）

gpu8（root@connect.weste.seetacloud.com:11337）无 docker、无内核容器能力，
但需要跑 postgres:17 / rabbitmq:3.13 原镜像。udocker 路线已签字：

- PRoot 下跑原镜像（postgres/rabbitmq）稳定；
- 实测开销 ~3.4%（可接受）；
- 进程形态：无守护进程，容器 = 独立 udocker（PRoot）进程，setsid + pidfile 管理。

本仓库把这个路线的工程形态固定下来：安装器 + docker drop-in 路由 + compose
子集骨架。**shim 核心（compose 解析/运行时/编排）是下一步**，当前 commit 只
立住骨架、安装路径与文档。

## 三条命令上手

```bash
./install.sh          # ① 安装：python3 检测 → pip 装 udocker → 引擎包离线安装 → PATH 注册
docker compose up -d  # ② 起栈（compose 子集，见能力边界）
docker compose down   # ③ 停栈
```

安装器检测链（`./install.sh --dry-run` 可只跑检测层看计划）：

0. **前置预检**：UDOCKER_DIR 所在盘剩余空间 ≥ DOCKLESS_MIN_FREE_GB（默认 5G，
   不足 fail-fast；gpu8 教训：root 盘 30G 曾撑爆，把 UDOCKER_DIR 指到大盘）；
   root 下探测 CAP_MKNOD（hooks 设备节点需要，缺失 warn）；
1. `python3` 缺失 → `apt install python3 python3-pip`（root 或 sudo）；
2. 本机已有真实 docker → **warn**（dockless 会 shadow PATH，docker 主机不需要它）；
3. `pip install udocker`（默认源失败自动回退 DOCKLESS_PIP_INDEX 清华镜像；
   离线失败 → 把 udocker 发行包放入 `vendor/` 兜底）；
4. 引擎包多源安装：`vendor/` 离线 tarball → `DOCKLESS_ENGINE_URL` →
   `udocker --version` 声明的官方 tarball URL（SSOT，版本跟随）→ 在线
   `udocker install`（已知慢源 ~30KB/s，最后兜底）；
5. `bin/docker`、`bin/dockless-compose` 软链到 `~/.local/bin`；
6. 验收：`docker --help` 必须退出 0。

## 能力边界

### compose 子集（已列入路线，shim 实现前统一 fail-fast 报"未实现"）

| 子命令 | 语义 |
|---|---|
| `config` | YAML → 四层语义模型（compose_parser） |
| `up` / `down` | 拓扑排序编排、depends_on、healthcheck（lifecycle） |
| `ps` / `logs` | pidfile + 进程存活状态，无守护进程 |
| `pull` / `rm` | 镜像层 / 容器层（runtime） |
| `exec` / `stop` / `start` / `restart` | 容器生命周期 |

### 单容器透传（bin/docker → udocker）

`run create ps rm rmi pull search images load import verify setup install version help names`
—— root 下自动加 `--allow-root`。

`run` 走 `dockless/run.py` 拦截（语义护栏 + env 白名单），其余子命令直传。

## 踩坑固化：udocker 语义差异速查（gpu8 复盘 S5）

以下差异全部在 gpu8 全部署中实测踩过，dockless 已处理或登记。**按 docker
心智操作 udocker 会出事**；先查这张表：

| docker 心智 | udocker 实际 | dockless 处理 |
|---|---|---|
| `run --rm` 创建临时容器、退出即删 | **对已存在容器会删除其容器元数据**（曾误删 pg17 容器，数据因 bind-mount 无损） | run 护栏 fail-fast（S1）：--rm 目标为已存在容器 → 报错退出；一次性命令用镜像名仍放行 |
| `run --name x` | 仅 create 支持，run 报 invalid container name | run 护栏 fail-fast 并提示 create 用法（S1） |
| `exec` 进容器执行 | 1.3.17 无 exec | fail-fast；容器内命令用一次性 `udocker run --rm <image> <cmd>`（服务容器内操作用 `--entrypoint` 一次性实例） |
| `--hostenv`（udocker 特有） | 全量透传宿主环境变量 | 白名单化：仅透传 `DOCKLESS_HOSTENV_ALLOWLIST`（默认空），其余用 `-e/--env` 显式传入（S2）。背景：TZ=Asia/Shanghai 传染进 postgres 曾致 hatchet seed UTC 校验 panic |
| `-e NAME` 透传宿主同名变量 | 不支持 | fail-fast，要求显式 `KEY=VAL`（S2） |
| `-p 8080:80` 端口映射 | 仅 host 网络直 bind，无 NAT（--publish 仅 Rn/runc 模式） | 透传提示；compose 端口收敛为容器内端口=宿主端口 |
| HEALTHCHECK 指令 | 被忽略 | 宿主侧一次性探测：`udocker run --rm IMG pg_isready -h 127.0.0.1` / `rabbitmq-diagnostics -q ping` |
| restart 策略/守护 | 无守护进程 | setsid + pidfile + 外部 watchdog（lifecycle 规划中，S6） |
| Docker Hub 拉镜像 | 直连被墙 | 用 daocloud 镜像（docker.m.daocloud.io），manifest 与 Hub 逐层 digest 一致 |
| root 下运行 | 拒绝，需 --allow-root | install/bin 自动加（gpu8 仅 root 用户场景） |

## 不支持项：**fail-fast 承诺**

以下一律响亮报错退出非零，**绝不静默降级、绝不假装成功**：

- compose 子集外：`build push run create scale attach top events images`；
- udocker 无对应能力：`exec cp logs stats top inspect port start stop restart kill rename
  wait build login push tag commit save export diff history attach events network volume
  system context builder buildx swarm node service stack secret config plugin manifest
  scout trust image container`；
- 语义级不支持：镜像构建、registry 登录/推送、跨容器虚拟网络（bridge/link）、
  资源限额（cgroup）。

扩展点：`bin/docker` 的 `UDOCKER_PASSTHROUGH` / `UNSUPPORTED_FAST` 两张表，
`bin/dockless-compose` 的 `SUBSET` / `UNSUPPORTED` 表。

## 架构

```
                ┌──────────────────────────┐
   docker cmd   │  bin/docker (drop-in)    │
   ───────────▶ │  路由骨架                 │
                └──────┬─────────┬─────────┘
                       │         │
          compose 子命令│         │ 其他子命令
                       ▼         ▼
        ┌──────────────────┐  ┌────────────────────┐
        │ bin/dockless-    │  │ udocker (root 时   │
        │ compose          │  │ 自动 --allow-root) │
        └───────┬──────────┘  └─────────┬──────────┘
                ▼                       ▼
   ┌─────────────────────────────────────────────┐
   │ dockless 包（shim 核心，下一步）              │
   │  compose_parser: YAML → 四层语义模型          │
   │  runtime:       image/container 层 → udocker │
   │  lifecycle:     network/volume 层 + 编排      │
   └──────────────────────┬──────────────────────┘
                          ▼
   ┌─────────────────────────────────────────────┐
   │ udocker 容器（PRoot 进程，无守护进程）        │
   │  postgres:17 / rabbitmq:3.13 原镜像          │
   │  setsid + pidfile（$DOCKLESS_RUN/*.pid）     │
   └──────────────────────┬──────────────────────┘
                          ▼
                    宿主内核（host 网络直通）
```

### PRoot 原理（一段）

PRoot 是一个用户态程序，通过 `ptrace` 拦截容器的系统调用，在用户态重写
路径解析（伪根文件系统：把容器 rootfs 假装成 `/`）和权限/身份检查
（伪 UID/GID 映射），**不需要 root、不需要内核模块、不需要特权**。udocker
用它把普通 tar 镜像"跑"成容器：镜像解包到 `$UDOCKER_DIR` 下的目录，PRoot
进程把它当根；设备节点、`/dev/shm` 这类内核对象缺失时，用 per-image hook
补齐（见 `hooks/postgres.sh`：mknod + `dynamic_shared_memory_type=mmap`）。
代价是每次 syscall 的 ptrace 开销——实测 ~3.4%，对 postgres/rabbitmq 这类
IO 密集但非 syscall 密集的负载可接受。

### 四层语义

compose 的声明被拆成 docker 对象模型的四层，逐层映射到 udocker 原语
（详见 `dockless/__init__.py`）：

1. **image 层** —— pull/import/load，镜像缓存 → `dockless/runtime`；
2. **container 层** —— create/run/exec，flag 翻译、后台化 → `dockless/runtime`；
3. **network 层** —— 仅 host 直通（无虚拟网卡），端口直 bind → `dockless/lifecycle`；
4. **volume 层** —— named volume / bind mount → rootfs 目录 + PRoot `-b` → `dockless/lifecycle`。

## 目录结构

```
install.sh                    # 安装器（预检：磁盘/CAP_MKNOD；python3/udocker 检测、引擎多源安装、PATH 注册）
bin/docker                    # docker drop-in 路由骨架（compose → dockless-compose，run → dockless.run，其余 → udocker）
bin/dockless-compose          # compose 子集路由（子集内"未实现"，子集外 fail-fast）
dockless/__init__.py          # 包说明 + 四层语义模型契约（LAYER_MODEL）
dockless/compose_parser.py    # compose YAML → 四层语义模型（骨架）
dockless/runtime.py           # image/container 层 udocker 封装（骨架）
dockless/run.py               # docker run 拦截：--rm/--name 语义护栏（S1）+ env 白名单重写（S2）
dockless/lifecycle.py         # network/volume 层 + up/down 编排（骨架）
hooks/postgres.sh             # per-image hook 样例（源自 gpu8 chroot-poc 脚本集）
vendor/udocker-englib-1.2.11.tar.gz  # 引擎包，离线安装用（udocker install）
```

`hooks/postgres.sh` 的 hook 契约（`hook_apply <rootfs> [data_dir]` /
`hook_env`）来自 gpu8 上的 `chroot-poc-scripts.tar.gz`，是「原镜像 + 平台
quirk」组合的既定模式：**镜像不动，quirk 由 hook 注入**。

## 下一步（shim 核心）

1. `compose_parser`：YAML → 四层模型（含 `${VAR}` 插值、ports/volumes 归一化）；
2. `runtime`：udocker 子进程封装 + run flag 翻译表补全 + `-d` setsid/pidfile
   （run 拦截已就位：护栏与 env 白名单见 dockless/run.py）；
3. `lifecycle`：depends_on 拓扑排序 + healthcheck 轮询 + up/down 状态机 +
   watchdog/restart 语义（S6 恢复力：gpu8 上 pg/rmq 已用 setsid+pidfile+外部
   watchdog 模式，此处收口为通用实现）；
4. 把 `bin/docker` / `bin/dockless-compose` 两张路由表里的 TODO 逐个接上实现。
