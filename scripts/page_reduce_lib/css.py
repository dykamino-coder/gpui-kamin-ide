"""Small lossless CSS scanner. Unrecognized syntax stays in the source."""

from dataclasses import dataclass, field
import re


def tokens(source):
    """Yield punctuation outside comments/strings, including balanced delimiters."""
    index = 0
    while index < len(source):
        char = source[index]
        if source.startswith("/*", index):
            end = source.find("*/", index + 2)
            index = len(source) if end < 0 else end + 2
        elif char in "\"'":
            quote = char
            index += 1
            while index < len(source):
                if source[index] == "\\":
                    index += 2
                elif source[index] == quote:
                    index += 1
                    break
                else:
                    index += 1
        elif char == "\\":
            index += 2
        else:
            if char in "{}[]();,":
                yield index, char
            index += 1


def split_ranges(source, separator=";"):
    start, depth = 0, 0
    for index, char in tokens(source):
        if char in "{([":
            depth += 1
        elif char in "})]":
            depth -= 1
        elif char == separator and depth == 0:
            yield start, index + (separator == ";")
            start = index + 1
    if start < len(source):
        yield start, len(source)


@dataclass
class Rule:
    start: int
    end: int
    opening: int | None = None
    closing: int | None = None
    children: list = field(default_factory=list)


class Stylesheet:
    def __init__(self, source):
        self.source = source
        self.rules = self.parse(0, len(source))

    def parse(self, start, end):
        rules, cursor, opening, depth = [], start, None, 0
        parens = 0
        for relative, char in tokens(self.source[start:end]):
            index = start + relative
            if char in "([":
                parens += 1
            elif char in ")]":
                parens -= 1
            elif parens == 0:
                if char == "{":
                    if depth == 0:
                        opening = index
                    depth += 1
                elif char == "}" and depth:
                    depth -= 1
                    if depth == 0:
                        rule = Rule(cursor, index + 1, opening, index)
                        prelude = re.sub(r"/\*.*?\*/", "", self.source[cursor:opening], flags=re.S).strip()
                        if re.match(r"@(media|supports|layer|container|scope|(?:-\w+-)?keyframes)\b", prelude, re.I):
                            rule.children = self.parse(opening + 1, index)
                        rules.append(rule)
                        cursor = index + 1
                elif char == ";" and depth == 0:
                    rules.append(Rule(cursor, index + 1))
                    cursor = index + 1
        # An unfinished rule is preserved, never guessed or dropped by the parser.
        return rules

    def serialize(self):
        return self.source

    def levels(self):
        level = self.rules
        while level:
            yield level
            level = [child for rule in level for child in rule.children]


def edits(source, kind):
    sheet = Stylesheet(source)
    for level in sheet.levels():
        group = []
        for rule in level:
            if kind == "rules":
                group.append((rule.start, rule.end, ""))
            elif rule.opening is not None:
                body = source[rule.opening + 1:rule.closing]
                prelude = source[rule.start:rule.opening]
                if kind == "unwrap" and re.match(r"\s*(?:/\*.*?\*/\s*)*@(media|supports)\b", prelude, re.S | re.I):
                    group.append((rule.start, rule.end, body))
                elif kind == "declarations" and not rule.children and not prelude.lstrip().startswith("@"):
                    for start, end in split_ranges(body):
                        if body[start:end].strip():
                            group.append((rule.opening + 1 + start, rule.opening + 1 + end, ""))
                elif kind == "selectors" and not prelude.lstrip().startswith("@"):
                    selectors = list(split_ranges(prelude, ","))
                    if len(selectors) > 1:
                        for index, (start, end) in enumerate(selectors):
                            if index < len(selectors) - 1:
                                end += 1
                            else:
                                start -= 1
                            group.append((rule.start + start, rule.start + end, ""))
        if group:
            yield group


def trivia_edits(source):
    # Reuse the lexical rules so strings, URLs and escaped spaces are untouched.
    pattern = r'''/\*.*?\*/|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|\\.|\s+'''
    for match in re.finditer(pattern, source, re.S):
        text = match.group()
        if text.startswith("/*"):
            yield match.start(), match.end(), " "
        elif text.isspace() and text != " ":
            yield match.start(), match.end(), " "
