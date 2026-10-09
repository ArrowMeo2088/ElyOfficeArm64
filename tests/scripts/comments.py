#!/usr/bin/env python3
"""Comments stay on one line and under eleven words."""
import re
import sys
from pathlib import Path

root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).resolve().parents[2]
PATTERNS = [
    "crates/**/*.rs",
    "apps/**/*.rs",
    "scripts/*.sh",
    "tests/**/*.sh",
    "tests/**/*.py",
    "packaging/**/*.sh",
    "packaging/**/*.nsi",
    "packaging/**/*.py",
    "packaging/**/*.swift",
    "tests/engines/*.c",
    "crates/**/*.c",
    "crates/**/*.h",
    "patches/**/*.patch",
    "tests/**/*.swift",
    "apps/*/web/src/**/*.js",
    "site/src/**/*.ts",
    "site/src/**/*.css",
    "site/test/**/*.ts",
    "site/video/**/*.py",
    "site/video/**/*.sh",
]
LINE_COMMENT = {
    ".rs": re.compile(r"^\s*//[!/]?\s?(.*)$"),
    ".c": re.compile(r"^\s*(?://[!/]?\s?(.*)|/\*\s?(.*?)\s*\*/)$"),
    ".h": re.compile(r"^\s*(?://[!/]?\s?(.*)|/\*\s?(.*?)\s*\*/)$"),
    ".nsi": re.compile(r"^\s*;\s?(.*)$"),
    ".swift": re.compile(r"^\s*//\s?(.*)$"),
    ".js": re.compile(r"^\s*(?://\s?(.*)|/\*\*?\s?(.*?)\s*\*/)$"),
    ".ts": re.compile(r"^\s*(?://\s?(.*)|/\*\*?\s?(.*?)\s*\*/)$"),
    ".css": re.compile(r"^\s*/\*\s?(.*?)\s*\*/$"),
}


def comment_re(path):
    if path.suffix == ".patch":
        return re.compile(r"^\+\s*(?://[!/]?\s?(.*)|/\*\s?(.*?)\s*\*/)$")
    return LINE_COMMENT.get(path.suffix, re.compile(r"^\s*#\s?(.*)$"))


too_long = []
repeated = {}
for pattern in PATTERNS:
    for path in sorted(root.glob(pattern)):
        comment = comment_re(path)
        previous = None
        lines = path.read_text().split("\n")
        if path.suffix == ".py":
            start = 1 if lines and lines[0].startswith("#!") else 0
            if start < len(lines) and lines[start].startswith('"""'):
                doc = []
                for line in lines[start:]:
                    doc.append(line.strip('" '))
                    if line.count('"""') >= 2 or (len(doc) > 1 and '"""' in line):
                        break
                lines = lines[:start] + [f"# {text}" for text in doc] + lines[start + len(doc):]
        for number, line in enumerate(lines, 1):
            if line.startswith("#!"):
                continue
            # A block comment left open spans lines.
            if path.suffix in (".c", ".h", ".js") and re.search(r"/\*(?!.*\*/)", line):
                repeated.setdefault(path, number)
            match = comment.match(line)
            said = match and next(group for group in match.groups() if group is not None)
            if match and len(said.split()) > 10:
                too_long.append(f"{path.relative_to(root)}:{number}: {said[:60]}")
            if match and previous == number - 1:
                repeated.setdefault(path, number)
            previous = number if match else None

problems = too_long + [
    f"{path.relative_to(root)}:{number}: comment runs past one line"
    for path, number in sorted(repeated.items())
]
if problems:
    sys.exit("FAIL: comments break the one-line ten-word rule:\n" + "\n".join(problems))
print("comments: every comment is one line and ten words or fewer")
