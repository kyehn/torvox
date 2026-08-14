#!/usr/bin/env python3
"""torvox 文档结构性约束检查（docs/ 的软件门禁）。

检查内容：
  1. arc42 — docs/architecture.md 必须含全部 12 个 arc42 章节标题
  2. ADR — docs/adr/000*.md 必须含 Nygard 模板字段
     (Status / Requirement IDs / Context / Decision / Consequences / Compliance)
  3. 需求同步 — srs.md / acceptance.md 中引用的 FR-xxx / NFR-xxx 必须
     存在于 docs/requirements/*.sdoc（避免漂移）
  4. 死链 — docs/ 下所有 markdown 相对链接按「文件所在目录」解析必须存在

单独运行：python3 docs/check-docs.py              # 全部检查
也可配合 vale / markdownlint 使用：
  vale --config=.vale.ini docs/                   # 术语与风格
  markdownlint-cli2 "docs/**/*.md" "AGENTS.md"    # markdown 语法
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOCS = os.path.join(ROOT, "docs")

ARC42_SECTIONS = [
    "1. Introduction and Goals",
    "2. Architecture Constraints",
    "3. Context and Scope",
    "4. Solution Strategy",
    "5. Building Block View",
    "6. Runtime View",
    "7. Deployment View",
    "8. Cross-cutting Concepts",
    "9. Architecture Decisions",
    "10. Quality Requirements",
    "11. Risks and Technical Debts",
    "12. Glossary",
]

ADR_FIELDS = [
    "## Status",
    "## Requirement IDs",
    "## Context",
    "## Decision",
    "## Consequences",
    "## Compliance",
]


def sdoc_uids():
    """收集 .sdoc 中的全部 UID（FR-xxx / NFR-xxx）。"""
    uids = set()
    for fn in ("functional_requirements.sdoc", "non_functional_requirements.sdoc"):
        p = os.path.join(DOCS, "requirements", fn)
        if not os.path.exists(p):
            raise SystemExit(f"缺少需求文档: {p}")
        s = open(p, encoding="utf-8").read()
        uids |= set(re.findall(r"^\s*UID:\s*([A-Z]+-\d+)\s*$", s, re.M))
    return uids


def check_arc42(issues):
    p = os.path.join(DOCS, "architecture.md")
    s = open(p, encoding="utf-8").read()
    for sec in ARC42_SECTIONS:
        # 匹配任意标题层级（## / ###）
        if not re.search(rf"^#+\s+{re.escape(sec)}\s*$", s, re.M):
            issues.append(f"arc42: docs/architecture.md 缺少章节「{sec}」")


def check_adr(issues):
    for fn in sorted(os.listdir(os.path.join(DOCS, "adr"))):
        if not re.match(r"^\d{4}-.+\.md$", fn):
            continue
        s = open(os.path.join(DOCS, "adr", fn), encoding="utf-8").read()
        for field in ADR_FIELDS:
            if field not in s:
                issues.append(f"adr: {fn} 缺少模板字段「{field}」")


def check_requirements_sync(issues, uids):
    for fn in ("srs.md", "acceptance.md"):
        p = os.path.join(DOCS, fn)
        s = open(p, encoding="utf-8").read()
        refs = set(re.findall(r"\b(?:FR|NFR)-\d{3}\b", s))
        missing = sorted(refs - uids)
        for r in missing:
            issues.append(f"requirements: {fn} 引用 {r}，但 .sdoc 中不存在")
    # traceability.yml 的 requirement key 必须与 .sdoc UID 完全一致
    p = os.path.join(DOCS, "traceability.yml")
    s = open(p, encoding="utf-8").read()
    keys = set(re.findall(r"^\s{2}((?:FR|NFR)-\d{3}):", s, re.M))
    for r in sorted(keys - uids):
        issues.append(f"requirements: traceability.yml 含 {r}，但 .sdoc 中不存在")
    for r in sorted(uids - keys):
        issues.append(f"requirements: traceability.yml 缺少 {r}（.sdoc 中存在）")


def check_glossary(issues):
    p = os.path.join(DOCS, "glossary.md")
    if not os.path.exists(p):
        issues.append("glossary: docs/glossary.md 不存在")
        return
    s = open(p, encoding="utf-8").read()
    head = s.splitlines()[:12]
    table_header = [l for l in head if l.startswith("| Term |")]
    if not table_header:
        issues.append("glossary: docs/glossary.md 必须包含 `| Term | Abbreviation | Definition | Source |` 表头（tgdp glossary 模板）")
        return
    # 术语消费链：glossary 中的标识符类术语（含 `_`/`-` 的代码符号）必须同时
    # 登记在 vale 词汇白名单 styles/config/vocabularies/Torvox/accept.txt，
    # 保证"文档术语 ↔ 机器拼写检查"不漂移。纯英语词/多词短语由 vale 词表自判。
    vocab_path = os.path.join(ROOT, "styles", "config", "vocabularies", "Torvox", "accept.txt")
    vocab = set()
    if os.path.exists(vocab_path):
        for line in open(vocab_path, encoding="utf-8").read().splitlines():
            word = line.strip()
            if word and not word.startswith("#"):
                vocab.add(word.lower())
    for row in re.findall(r"^\| ([^|]+) \|", s, re.M):
        term = row.strip()
        if not term or term == "---" or "Abbreviation" in term:
            continue  # 表头或分隔行
        if any(ch.isspace() or ch in "/、（）" for ch in term):
            continue  # 多词短语或中文
        if "_" not in term and "-" not in term:
            continue  # 纯英语词/专名由 vale 词表与白名单处理
        if term.lower() not in vocab:
            issues.append(f"glossary: 术语 `{term}` 未登记在 vale 词汇白名单（styles/config/vocabularies/Torvox/accept.txt）")


def check_decision_registry(issues):
    # rejected-technologies.md 必须是三区决策登记处：分区标题固定，
    # 防止历史"七区交错编号"结构回归。
    p = os.path.join(DOCS, "rejected-technologies.md")
    s = open(p, encoding="utf-8").read()
    for section in ["## 1. Rejected", "## 2. Deferred", "## 3. Absorbed"]:
        if section not in s:
            issues.append(f"rejected: 缺少分区 `{section}`（决策登记处三区结构被破坏）")



def github_slug(heading):
    """GitHub-style anchor slug for a markdown heading."""
    slug = re.sub(r"[^\w\- ]", "", heading.lower())
    return slug.replace(" ", "-")


def check_acceptance_toc(issues):
    p = os.path.join(DOCS, "acceptance.md")
    s = open(p, encoding="utf-8").read()
    fr_ids = re.findall(r"^### (FR-\d{3}):", s, re.M)
    dupes = sorted({x for x in fr_ids if fr_ids.count(x) > 1})
    if dupes:
        issues.append(f"acceptance: FR 编号重复 {dupes}")
    # TOC 锚点必须与真实章节标题 slug 一致（GitHub 规则）
    headings = re.findall(r"^## (.+)$", s, re.M)
    toc_links = re.findall(r"^\- \[.*?\]\(#([^)]+)\)$", s, re.M)
    slugs = {github_slug(h) for h in headings}
    for link in toc_links:
        if link not in slugs:
            issues.append(f"acceptance: TOC 锚点 `#{link}` 无对应章节标题")



def check_links(issues):
    for dirpath, _dirnames, filenames in os.walk(DOCS):
        for fn in filenames:
            if not fn.endswith(".md"):
                continue
            p = os.path.join(dirpath, fn)
            s = open(p, encoding="utf-8").read()
            for m in re.finditer(r"\]\(([^)\s]+)\)", s):
                target = m.group(1)
                if target.startswith(("http", "#", "<", "mailto:")):
                    continue
                t = target.split("#")[0]
                if not t:
                    continue
                full = os.path.normpath(os.path.join(dirpath, t))
                if not os.path.exists(full):
                    issues.append(f"link: {os.path.relpath(p, ROOT)} 链接到不存在的 {target}")


def main():
    issues = []
    check_arc42(issues)
    check_adr(issues)
    check_requirements_sync(issues, sdoc_uids())
    check_links(issues)
    check_glossary(issues)
    check_acceptance_toc(issues)
    check_decision_registry(issues)
    if issues:
        print(f"❌ {len(issues)} 个问题：")
        for i in issues:
            print("  -", i)
        sys.exit(1)
    print("✅ 文档结构检查全部通过（arc42 / ADR 模板 / 需求同步 / 链接）")


if __name__ == "__main__":
    main()