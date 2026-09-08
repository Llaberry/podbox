"""dockless.runtime — image 层 + container 层的 udocker 封装（骨架）。

职责
----
四层语义里的前两层：镜像获取与容器生命周期。全部实现 TODO（下一步）。

image 层（→ udocker）
---------------------
- pull:   docker pull REF    → udocker pull REF（registry 走 docker hub）
- import: docker import TAR  → udocker import TAR [name]
- load:   docker load < TAR  → udocker load < TAR
- 缓存：udocker 本地镜像仓库（$UDOCKER_DIR/repos），不做重复 pull

container 层（→ udocker create/run）
-----------------------------------
- create: docker create … → udocker create --name=… IMAGE（参数翻译表 TODO）
- run:    docker run …    → udocker run（前台）/
          udocker run -d → setsid + pidfile（后台，生命周期归 lifecycle）
- exec:   docker exec …   → 暂不支持（udocker 无 exec；PRoot 下需 re-exec，
          登记到 fail-fast 清单，见 bin/docker UNSUPPORTED_FAST）
- flag 翻译：-e/--env、-p/--publish、-v/--volume、-u/--user、--entrypoint、
  -w/--workdir、--name、--rm（udocker run 不要用 --rm：会删容器元数据，坑）
- 原镜像 + per-image hook：hooks/*.sh（如 postgres.sh 的 hook_apply/hook_env）
  在 create 后、run 前应用文件系统 quirks 与环境变量

TODO（shim 核心，下一步）
-------------------------
- [ ] udocker 子进程封装（subprocess.run，超时/错误归一化）
- [ ] run flag 翻译表（docker run 参数 → udocker run 参数）
- [ ] --rm 语义修正：udocker 不支持，显式报错而非静默
- [ ] -d 后台化：setsid + pidfile（对齐 gpu8 上 pg/rmq 容器的既有模式）
- [ ] hooks/postgres.sh 这类 per-image hook 的加载与执行
- [ ] 镜像存在性检查（pull 前查 udocker 仓库）
"""

from __future__ import annotations


class RuntimeError_(Exception):
    """docker 语义错误（fail-fast，与解析错误区分）。"""


def pull_image(ref: str) -> None:
    """udocker pull <ref>；骨架阶段仅抛 NotImplementedError。"""
    raise NotImplementedError(
        "dockless shim 核心尚未实现：runtime.pull_image 是下一步"
    )


def run_container(image_ref: str, *args: str, **kwargs: object) -> int:
    """容器生命周期入口：翻译 docker run 参数并驱动 udocker。

    骨架阶段仅抛 NotImplementedError；参数翻译表见模块 docstring TODO。
    """
    raise NotImplementedError(
        "dockless shim 核心尚未实现：runtime.run_container 是下一步"
    )
