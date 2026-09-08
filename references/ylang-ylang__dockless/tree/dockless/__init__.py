"""dockless — 无 DinD/userns 平台的 docker 补丁（Python 包骨架）。

定位
----
在「没有 docker 守护进程、没有内核 userns/DinD 能力」的主机（如 gpu8 autodl
容器机）上，用 udocker + PRoot 提供 docker 命令语义的子集。本包是 shim 核心的
骨架：**实现是下一步**，当前只立住模块边界、数据模型与 TODO。

四层语义（docker 对象模型 → udocker 原语映射，全部 TODO）
----------------------------------------------------------
docker 世界有四个对象层，compose 文件是对它们的声明式描述。
dockless 把 compose 语义拆成这四层，逐层映射到 udocker/PRoot：

1. image 层   —— 镜像获取与解析：pull/import/load → udocker pull/import/load，
                  本地 registry 镜像缓存管理。           → dockless.runtime
2. container 层 —— 容器生命周期：create/run/exec，env/端口/卷/用户 flag
                  → udocker create/run 参数翻译，PID 管理。→ dockless.runtime
3. network 层 —— 网络语义：compose networks/ports/expose → 当前仅支持
                  host 网络直通（PRoot 无虚拟网卡）；端口映射在 host 上
                  bind 监听（如 postgres hook 的 listen_addresses=0.0.0.0）。
                                                          → dockless.lifecycle
4. volume 层  —— 数据卷语义：named volumes/bind mounts → 根文件系统目录
                  （rootfs 子目录挂载，PRoot 的 -b 绑定）。→ dockless.lifecycle

compose 文件（services/networks/volumes）由 dockless.compose_parser 解析成
上面的四层模型；up/down 编排（depends_on 拓扑排序、健康检查、启动顺序）由
dockless.lifecycle 负责；单容器运行时细节在 dockless.runtime。

模块职责
--------
- compose_parser : compose YAML → 四层语义模型（纯数据，无副作用）
- runtime        : image 层 + container 层（udocker 子进程封装）
- lifecycle      : network 层 + volume 层 + up/down 编排（状态机/pidfile）

能力边界
--------
- 只承诺 README「compose 子集」+ 透传子命令；子集外一律 fail-fast。
- 不支持：镜像构建（build）、registry 登录推送、跨容器虚拟网络、资源限额。
- 每个容器最终都是一个独立 udocker 容器（PRoot 进程），无守护进程。
"""

__version__ = "0.1.0"

# 四层语义模型的字典形状（compose_parser 输出，lifecycle/runtime 消费）。
# 下一步实现时以此为接口契约，不另起数据结构。
LAYER_MODEL = {
    "images": [],       # [{ref, local_name, source}]            image 层
    "containers": [],   # [{service, image_ref, env, ports, volumes, user, cmd, depends_on, healthcheck}]
    "networks": [],     # [{name, mode: "host"}]                 当前仅 host
    "volumes": [],      # [{name, target, bind}]                 volume 层
}

__all__ = [
    "LAYER_MODEL",
    "compose_parser",
    "runtime",
    "lifecycle",
]
