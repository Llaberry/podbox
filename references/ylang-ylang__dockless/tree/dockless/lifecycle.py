"""dockless.lifecycle — network 层 + volume 层 + compose up/down 编排（骨架）。

职责
----
四层语义里的后两层 + 编排：compose 的多容器生命周期、依赖排序、状态追踪。
全部实现 TODO（下一步）。

network 层（当前能力边界：host 网络直通）
-----------------------------------------
- PRoot/udocker 无虚拟网卡：容器与宿主机共享网络命名空间。
- ports 语义 → 容器内进程直接 bind 宿主端口（如 postgres hook 的
  listen_addresses=0.0.0.0）；compose 的端口映射在 host 模式下等价于直通。
- 不支持：自定义 bridge 网络、服务间隔离 DNS、link 别名（fail-fast）。

volume 层
----------
- named volume → $DOCKLESS_DATA/<name> 目录，PRoot -b 绑定到容器内目标路径
- bind mount   → 宿主绝对路径直接 -b 绑定（含 ro 后缀）
- 数据生命周期：down 不删 named volume（docker 语义），rm -v 才删

编排（up/down/ps/logs/stop/start/restart）
------------------------------------------
- depends_on → 拓扑排序（无环；环检测 fail-fast），按序启动
- healthcheck → 启动后轮询（test/interval/timeout/retries/start_period），
  失败容器标记 unhealthy 并继续（docker 行为），不阻塞 up
- 后台容器：setsid + pidfile（$DOCKLESS_RUN/<service>.pid），ps/logs/stop 靠它
- 状态查询：docker compose ps 读 pidfile + 进程存活检查（无守护进程）

TODO（shim 核心，下一步）
-------------------------
- [ ] depends_on 拓扑排序 + 环检测
- [ ] 启动顺序编排（image 就绪 → 容器 up → healthcheck）
- [ ] setsid + pidfile 后台化与状态机（up/down/stop/start/restart/ps）
- [ ] named volume 目录管理与 PRoot -b 绑定参数生成
- [ ] healthcheck 轮询实现（复用 hooks/ 的探测约定）
- [ ] down 语义：stop 全部 + 可选 -v 清卷；孤儿容器检测 warn
"""

from __future__ import annotations


def up(compose_model: dict) -> None:
    """按四层语义模型编排启动：拓扑排序 → 逐容器 run → healthcheck。

    骨架阶段仅抛 NotImplementedError；语义见模块 docstring TODO。
    """
    raise NotImplementedError(
        "dockless shim 核心尚未实现：lifecycle.up 是下一步"
    )


def down(compose_model: dict, remove_volumes: bool = False) -> None:
    """编排停止：逆拓扑序停容器；remove_volumes=True 时删 named volumes。

    骨架阶段仅抛 NotImplementedError；语义见模块 docstring TODO。
    """
    raise NotImplementedError(
        "dockless shim 核心尚未实现：lifecycle.down 是下一步"
    )
