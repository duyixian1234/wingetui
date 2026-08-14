"""Generate sanitized text-table fixtures for wingetui tests.

Alignment mimics winget: each column is padded to max(header, data) display width + 2
gap in display terms (CJK = 2 columns). All package names are public/generic (sanitized).
"""
import os

FIXTURE_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "crates", "winget", "tests", "fixtures")

def disp_w(ch):
    o = ord(ch)
    if (0x1100 <= o <= 0x115F or 0x2E80 <= o <= 0xA4CF or 0xAC00 <= o <= 0xD7A3
            or 0xF900 <= o <= 0xFAFF or 0xFE30 <= o <= 0xFE4F or 0xFF00 <= o <= 0xFF60
            or 0xFFE0 <= o <= 0xFFE6 or 0x20000 <= o <= 0x3FFFD):
        return 2
    return 1

def disp(s):
    return sum(disp_w(c) for c in s)

def render_table(header_tokens, rows):
    """rows: list of tuples; None means empty cell (e.g. no 可用 column value)."""
    ncols = len(header_tokens)
    widths = []
    for c in range(ncols):
        w = disp(header_tokens[c])
        for row in rows:
            val = row[c] if c < len(row) else None
            if val:
                w = max(w, disp(val))
        widths.append(w)
    lines = []
    # header
    hdr = []
    for c, tok in enumerate(header_tokens):
        if c == ncols - 1:
            hdr.append(tok)
        else:
            hdr.append(tok + " " * (widths[c] - disp(tok) + 2))
    lines.append("".join(hdr))
    # separator
    lines.append("-" * disp(lines[0]))
    # data rows
    for row in rows:
        cells = []
        for c in range(ncols):
            val = row[c] if c < len(row) and row[c] else ""
            if c == ncols - 1:
                cells.append(val)
            else:
                cells.append(val + " " * (widths[c] - disp(val) + 2))
        lines.append("".join(cells))
    return "\r\n".join(lines) + "\r\n"

def write(name, content, encoding="utf-8"):
    path = os.path.join(FIXTURE_DIR, name)
    with open(path, "w", encoding=encoding, newline="") as f:
        f.write(content)
    print(f"wrote {name} ({len(content.encode(encoding))} bytes, {encoding})")

search_header = ["名称", "ID", "版本", "匹配", "源"]
search_rows = [
    ("PowerShell", "Microsoft.PowerShell", "7.4.5", None, "winget"),
    ("Git", "Git.Git", "2.45.1", "Tag: git", "winget"),
    ("Visual Studio Code", "Microsoft.VisualStudioCode", "1.90.2", None, "winget"),
]
write("search.txt", render_table(search_header, search_rows))

upgradeable_header = ["名称", "ID", "版本", "可用", "源"]
upgradeable_rows = [
    ("Git", "Git.Git", "2.45.1", "2.46.0", "winget"),
    ("PowerShell", "Microsoft.PowerShell", "7.4.5", "7.5.0", "winget"),
]
write("upgradeable.txt", render_table(upgradeable_header, upgradeable_rows))

installed_header = ["名称", "ID", "版本", "可用", "源"]
installed_rows = [
    ("PowerShell", "Microsoft.PowerShell", "7.4.5", None, "winget"),
    ("Git", "Git.Git", "2.45.1", None, "winget"),
]
write("installed.txt", render_table(installed_header, installed_rows))

# __notfound__: header + separator + status line (no data rows)
empty = render_table(search_header, []) 
write("search-empty.txt", empty + "未找到与输入条件匹配的程序包。\r\n")

# __malformed__: text without any header row
write("malformed.txt", "这不是一个表格\r\n没有任何表头行\r\n")

# GBK fixture: search table encoded in GBK (header 名称/版本/匹配/源 are CJK)
gbk_text = render_table(search_header, search_rows[:2])
write("search-gbk.txt", gbk_text, encoding="gbk")

print("done")
