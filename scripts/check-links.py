#!/usr/bin/env python3
"""Markdown 相对链接校验器（治理规范铁律第 4 条）。

规则：
- 相对链接（含 ../、锚点 #section）→ 解析到当前文件所在目录，校验目标存在
- http(s):// 外链、mailto: → 跳过（不做可达性校验）
- 页内锚点 #xxx（无路径部分）→ 跳过
- 文件系统绝对路径（C:/、D:\、/d/、file:// 等）→ FAIL（禁止使用）

用法：
    uv run --quiet scripts/check-links.py [目录...]
    默认扫描仓库根目录下全部 *.md

退出码：0 = 全部通过；1 = 存在失败链接。
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

# Windows runner（Python 3.12 及以下）默认 stdout 编码为 cp1252，
# 输出中文会 UnicodeEncodeError；强制 UTF-8 保证 CI 与本地一致。
if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

LINK_RE = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")
IMG_RE = re.compile(r"!\[[^\]]*\]\(([^)]+)\)")
AUTO_LINK_RE = re.compile(r"<([^<>]+)>")
REF_LINK_RE = re.compile(r"\[([^\]]+)\]:\s*(\S+)")

# 绝对路径判定（Windows 盘符 / UNC / Unix 绝对路径 / file:// / mailto: / 协议）
ABSOLUTE_PREFIXES = (
    "file:",
    "mailto:",
    "tel:",
)
PROTOCOL_RE = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.-]*://")
WIN_DRIVE_RE = re.compile(r"^[a-zA-Z]:[\\/]")
UNC_RE = re.compile(r"^\\\\")
UNIX_ABS_RE = re.compile(r"^/")


def strip_anchor(target: str) -> str:
    """剥离 #锚点 部分，返回纯路径部分。"""
    return target.split("#", 1)[0]


def is_external(target: str) -> bool:
    """是否外部/不可校验链接。"""
    t = target.strip()
    if not t:
        return True
    if t.startswith(ABSOLUTE_PREFIXES):
        return True
    if PROTOCOL_RE.match(t):
        return True
    return False


def is_absolute_path(target: str) -> bool:
    """是否文件系统绝对路径（违禁）。"""
    t = target.strip()
    if WIN_DRIVE_RE.match(t) or UNC_RE.match(t) or UNIX_ABS_RE.match(t):
        return True
    return False


def is_anchor_only(target: str) -> bool:
    """是否页内锚点（#xxx）。"""
    t = target.strip()
    return t.startswith("#")


FENCE_RE = re.compile(r"^\s*(```|~~~)")
INLINE_CODE_RE = re.compile(r"``[^`]+``|`[^`]+`")


def strip_code_blocks(text: str) -> str:
    """剔除围栏代码块（含 mermaid）与行内代码，块内内容不做链接校验。"""
    lines = text.splitlines()
    out: list[str] = []
    in_fence = False
    for line in lines:
        if FENCE_RE.match(line):
            in_fence = not in_fence
            continue
        if not in_fence:
            line = INLINE_CODE_RE.sub("", line)
            out.append(line)
    return "\n".join(out)


def check_file(md: Path, repo_root: Path) -> list[str]:
    """校验单个 Markdown 文件，返回失败项列表。"""
    failures: list[str] = []
    try:
        text = md.read_text(encoding="utf-8")
    except OSError as e:
        failures.append(f"{md.relative_to(repo_root)}: 读取失败 {e}")
        return failures

    text = strip_code_blocks(text)
    base = md.parent

    def check(target: str, kind: str) -> None:
        rel = md.relative_to(repo_root)
        t = target.strip()
        if is_external(t) or is_anchor_only(t) or not t:
            return
        if is_absolute_path(t):
            failures.append(f"{rel}: {kind} 使用绝对路径（违禁）: {target}")
            return
        path_part = strip_anchor(t)
        if not path_part:
            return
        # 兼容反斜杠路径
        norm = path_part.replace("\\", "/")
        resolved = (base / norm).resolve()
        if not resolved.exists():
            failures.append(f"{rel}: {kind} 目标不存在: {target}")

    for m in LINK_RE.finditer(text):
        check(m.group(1), "链接")
    for m in IMG_RE.finditer(text):
        check(m.group(1), "图片")
    for m in AUTO_LINK_RE.finditer(text):
        check(m.group(1), "自动链接")
    for m in REF_LINK_RE.finditer(text):
        check(m.group(2), "引用式链接定义")
    return failures


def main() -> int:
    repo_root = Path.cwd()
    roots = [Path(p) for p in sys.argv[1:]] if len(sys.argv) > 1 else [repo_root]

    md_files: list[Path] = []
    for root in roots:
        r = root if root.is_absolute() else (repo_root / root)
        md_files.extend(sorted(r.rglob("*.md")))

    # 跳过 .git / target / node_modules
    md_files = [f for f in md_files if not any(
        part in (".git", "target", "node_modules", ".workbuddy")
        for part in f.parts
    )]

    all_failures: list[str] = []
    for f in md_files:
        all_failures.extend(check_file(f, repo_root))

    if all_failures:
        print(f"FAIL: {len(all_failures)} 处链接问题")
        for line in all_failures:
            print(f"  - {line}")
        return 1

    print(f"OK: {len(md_files)} 个 Markdown 文件链接校验全部通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
