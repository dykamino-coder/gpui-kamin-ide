"""HTML source ranges, deliberately without HTML5 tree repair or normalization."""

from dataclasses import dataclass, field
from html.parser import HTMLParser
import re


VOID = frozenset("area base br col embed hr img input link meta param source track wbr".split())
ATTR = re.compile(r'''([^\s=<>/]+)(?:\s*=\s*("[^"]*"|'[^']*'|[^\s>]+))?''')


@dataclass
class Node:
    tag: str
    start: int
    open_end: int
    close_start: int
    end: int
    children: list = field(default_factory=list)
    attrs: list = field(default_factory=list)


class Document(HTMLParser):
    def __init__(self, source):
        super().__init__(convert_charrefs=False)
        self.source = source
        self.lines = [0] + [m.end() for m in re.finditer("\n", source)]
        self.root = Node("", 0, 0, len(source), len(source))
        self.stack = [self.root]
        self.text = []
        self.comments = []
        self.feed(source)
        self.close()
        for node in self.stack[1:]:
            node.close_start = node.end = len(source)

    def source_offset(self):
        row, col = self.getpos()
        return self.lines[row - 1] + col

    def handle_starttag(self, tag, attrs):
        start = self.source_offset()
        end = start + len(self.get_starttag_text())
        node = Node(tag, start, end, end, end, attrs=attrs)
        self.stack[-1].children.append(node)
        if tag not in VOID:
            self.stack.append(node)

    def handle_startendtag(self, tag, attrs):
        self.handle_starttag(tag, attrs)
        if self.stack[-1].tag == tag and tag not in VOID:
            self.stack.pop()

    def handle_endtag(self, tag):
        start = self.source_offset()
        end = self.source.find(">", start) + 1
        for index in range(len(self.stack) - 1, 0, -1):
            if self.stack[index].tag == tag:
                for unclosed in self.stack[index + 1:]:
                    unclosed.close_start = unclosed.end = start
                node = self.stack[index]
                node.close_start, node.end = start, end
                del self.stack[index:]
                break

    def handle_data(self, data):
        self.text.append((self.source_offset(), self.source_offset() + len(data), self.stack[-1].tag))

    def handle_comment(self, data):
        start = self.source_offset()
        end = self.source.find("-->", start)
        if end >= 0:
            self.comments.append((start, end + 3, ""))

    def serialize(self):
        # Untouched tokens are never normalized (entities, casing, quotes, doctypes).
        return self.source

    def levels(self):
        level = self.root.children
        while level:
            yield level
            level = [child for node in level for child in node.children]

    def nodes(self):
        return [node for level in self.levels() for node in level]


def attribute_edits(source, node):
    opening = source[node.start:node.open_end]
    tag_end = re.match(r"<\s*[^\s/>]+", opening).end()
    for match in ATTR.finditer(opening, tag_end):
        name, value = match.groups()
        start, end = node.start + match.start(), node.start + match.end()
        yield (start, end, "")
        if name.lower() in ("class", "id") and value:
            raw = value[1:-1] if value[0] in "\"'" else value
            tokens = raw.split()
            if len(tokens) > 1:
                for index in range(len(tokens)):
                    kept = " ".join(tokens[:index] + tokens[index + 1:])
                    yield (start, end, f'{name}="{kept}"')


def apply(source, edits):
    """Apply disjoint source edits; reject overlap instead of silently corrupting."""
    result, previous = [], 0
    for start, end, replacement in sorted(edits):
        if start < previous or not (0 <= start <= end <= len(source)):
            raise ValueError("overlapping or invalid source ranges")
        result.extend((source[previous:start], replacement))
        previous = end
    result.append(source[previous:])
    return "".join(result)
