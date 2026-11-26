"""Inline local stylesheets and preserve resource bases when moving candidates."""

from html import escape
from pathlib import Path
import re
from urllib.parse import urljoin, urlparse
from urllib.request import url2pathname

from .css import Stylesheet
from .html import ATTR, Document, apply


def local_path(url):
    parsed = urlparse(url)
    if parsed.scheme != "file" or parsed.netloc not in ("", "localhost"):
        raise ValueError(f"stylesheet must be a local file: {url}")
    return Path(url2pathname(parsed.path))


def css_urls(source, base):
    # Tokenize comments and strings first; do not rewrite a literal "url(...)".
    pattern = r'''/\*.*?\*/|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|\burl\(\s*(?:"((?:\\.|[^"\\])*)"|'((?:\\.|[^'\\])*)'|([^)'"\s]*))\s*\)'''
    def replace(match):
        value = next((v for v in match.groups() if v is not None), None)
        if value is None or not value or value.startswith(("#", "data:")):
            return match.group()
        # CSS escapes need a full CSS tokenizer to resolve as a path. Retain them
        # rather than silently rebasing to a different resource.
        if "\\" in value:
            raise ValueError("escaped CSS resource URL cannot be safely rebased")
        return 'url("' + urljoin(base, value).replace('"', "%22") + '")'
    return re.sub(pattern, replace, source, flags=re.S | re.I)


def stylesheet(path, active=()):
    path = path.resolve()
    if path in active:
        # CSS import cycles are ignored by browsers.
        return ""
    text = path.read_text(encoding="utf-8-sig")
    text = inline_imports(text, path.as_uri(), (*active, path))
    # Imported URLs are already absolute; rebasing them again is idempotent.
    return css_urls(text, path.as_uri())


def prepare(source, page, inline=False):
    base = page.resolve().as_uri()
    doc = Document(source)
    base_node = next((node for node in doc.nodes() if node.tag == "base"), None)
    if base_node:
        base = urljoin(base, dict(base_node.attrs).get("href") or "")
    changes = []
    for node in doc.nodes():
        attrs = dict(node.attrs)
        if inline and node.tag == "link" and "stylesheet" in (attrs.get("rel") or "").lower().split():
            if "disabled" in attrs or "alternate" in (attrs.get("rel") or "").lower().split():
                raise ValueError("disabled/alternate stylesheets need explicit selection before inlining")
            target = local_path(urljoin(base, attrs.get("href") or ""))
            body = stylesheet(target)
            media = attrs.get("media")
            if media:
                body = f"@media {media}{{{body}}}"
            if "</style" in body.lower():
                raise ValueError("stylesheet contains an HTML style end tag")
            changes.append((node.start, node.end, "<style>" + body + "</style>"))
            continue
        if node.tag == "style":
            body = source[node.open_end:node.close_start]
            # Inline @imports use the page base; reuse the import expander by
            # processing imports in place rather than creating a temporary CSS file.
            if inline and re.search(r"@import\b", body, re.I):
                body = inline_imports(body, base)
            rebased = css_urls(body, base)
            if rebased != source[node.open_end:node.close_start]:
                changes.append((node.open_end, node.close_start, rebased))
        opening = source[node.start:node.open_end]
        tag_end = re.match(r"<\s*[^\s/>]+", opening).end()
        for match in ATTR.finditer(opening, tag_end):
            name, raw = match.groups()
            if raw is None:
                continue
            value = dict(node.attrs).get(name.lower())
            if name.lower() in ("src", "href", "poster", "background", "data") and value and not value.startswith("#"):
                replacement = f'{name}="{escape(urljoin(base, value), quote=True)}"'
            elif name.lower() == "style" and value:
                replacement = f'{name}="{escape(css_urls(value, base), quote=True)}"'
            elif name.lower() == "srcset":
                raise ValueError("srcset needs explicit rebasing before reduction")
            else:
                continue
            changes.append((node.start + match.start(), node.start + match.end(), replacement))
    return apply(source, changes)


def inline_imports(text, base, active=()):
    edits = []
    for rule in Stylesheet(text).rules:
        raw = re.sub(r'^(?:\s|/\*.*?\*/)*', '', text[rule.start:rule.end], flags=re.S)
        match = re.fullmatch(r'''@import\s+(?:url\(\s*(?:"([^"]*)"|'([^']*)'|([^\s)]+))\s*\)|"([^"]*)"|'([^']*)')\s*([^;]*);''', raw, re.I | re.S)
        if match:
            url = next(v for v in match.groups()[:5] if v is not None)
            if "\\" in url:
                raise ValueError("escaped CSS import URL cannot be safely rebased")
            body = stylesheet(local_path(urljoin(base, url)), active)
            media = match[6].strip()
            if re.search(r'\b(?:layer\b|supports\s*\()', media, re.I):
                raise ValueError("@import layer/supports qualifiers are not yet supported")
            edits.append((rule.start, rule.end, f"@media {media}{{{body}}}" if media else body))
        elif re.match(r'@import\b', raw, re.I):
            raise ValueError("unsupported CSS import syntax")
    return apply(text, edits)
