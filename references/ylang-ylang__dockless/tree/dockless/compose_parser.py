"""dockless.compose_parser — compose YAML → 四层语义模型（骨架）。

职责
----
把 docker-compose.yml 解析成 dockless.LAYER_MODEL 形状的纯数据模型，
不执行任何外部命令、不触 udocker。所有解析逻辑都是 TODO（下一步实现）。

四层语义归属（本模块只做「读」）：
- images      ← services[].image / services[].build（build 不支持，fail-fast 标记）
- containers  ← services[] 展开：env/ports/volumes/user/command/depends_on/healthcheck
- networks    ← top-level networks + services[].networks（当前仅 host 模式）
- volumes     ← top-level volumes + services[].volumes（named + bind）

TODO（shim 核心，下一步）
-------------------------
- [ ] YAML 读取与 schema 校验（compose spec 子集）
- [ ] ${VAR} 插值（env_file/.env 支持）
- [ ] services[].build 检测 → fail-fast（构建不在能力边界）
- [ ] depends_on 提取（拓扑排序在 lifecycle 做，这里只收集边）
- [ ] ports 归一化："8080:80" / "127.0.0.1:8080:80" / 裸 "80"（随机高位端口 → 显式端口）
- [ ] volumes 归一化：named volume / bind mount / ro 后缀
- [ ] healthcheck 提取（test/interval/timeout/retries/start_period）
- [ ] 四层模型输出（LAYER_MODEL 形状），未知字段 warn 不 fail（docker 兼容性）
"""

from __future__ import annotations

from typing import Any


def parse_compose_file(path: str) -> dict[str, Any]:
    """解析 compose 文件 → 四层语义模型（images/containers/networks/volumes）。

    骨架阶段：仅校验入参并抛 NotImplementedError，语义见模块 docstring。
    """
    if not path:
        raise ValueError("compose file path 不能为空")
    raise NotImplementedError(
        "dockless shim 核心尚未实现：compose_parser.parse_compose_file 是下一步"
    )
