"""Conservative lexical field/owner/access aggregates, independent of source lines.

Not Rust type inference: same-named fields are deliberately over-counted. Owners
are top-level HTML modules, so moving a file within a phase does not change debt.
Style construction may write operands; non-style writes/projections and mutable
escape APIs are ratcheted. Whole-value mutation must also be reviewed manually.
"""

from collections import Counter, defaultdict
import re

from computed_test_scopes import scopes as test_scopes

PREFIX = 'crates/html/src/'
DEFINITIONS = PREFIX + 'style/computed/fields.rs'
USED = {'used_mut', 'used_rc', 'layout_made', 'into_computed', 'lay_used'}
MUTABLE = {'make_mut', 'get_mut', 'borrow_mut', 'deref_mut', 'unwrap_or_clone'}


def definitions(source):
    owner, result = None, defaultdict(list)
    for line in source.splitlines():
        if match := re.match(r'^pub(?:\([^)]*\))? struct (Computed|InheritedStyle)\b', line):
            owner = match[1]
        if owner and (match := re.match(r'^    pub(?:\([^)]*\))? (\w+):', line)):
            result[match[1]].append(owner + '.' + match[1])
        if line == '}':
            owner = None
    if not result:
        raise ValueError('no Computed/InheritedStyle field definitions')
    return result


def access(words, index):
    end = index + 1
    # Indexed fields and nested projections still write their owning operand.
    while end < len(words):
        if words[end] == '[':
            depth = 1
            end += 1
            while end < len(words) and depth:
                depth += (words[end] == '[') - (words[end] == ']')
                end += 1
        elif (end + 1 < len(words) and words[end] == '.'
              and (words[end + 1].isidentifier() or words[end + 1].isdigit())):
            if end + 2 < len(words) and words[end + 2] == '(':
                break
            end += 2
        else:
            break
    tail = words[end:end + 3]
    if tail[:1] == ['='] and tail[:2] not in (['=', '='], ['=', '>']):
        return 'write'
    if len(tail) >= 2 and tail[0] in '+-*/%&|^' and tail[1] == '=':
        return 'write'
    if tail in (['<', '<', '='], ['>', '>', '=']):
        return 'write'
    start = index - 1
    while start >= 0 and words[start] not in (';', '{', '}', '=', ',', '('):
        if words[start:start + 2] == ['&', 'mut']:
            return 'mutable'
        start -= 1
    if start >= 2 and words[start] == '(' and words[start - 2:start] == ['&', 'mut']:
        return 'mutable'
    return 'read'


def aggregate(sources):
    fields = definitions(sources[DEFINITIONS])
    counts = Counter()
    for path, (words, test_file, ranges) in sorted(test_scopes(sources).items()):
        owner = path.removeprefix(PREFIX).split('/')[0].removesuffix('.rs')
        scopes = []
        for index, word in enumerate(words):
            context = owner + ':test' if test_file or any(a <= index < b for a, b in ranges) else owner
            before = words[index - 1] if index else None
            if word == '{':
                scopes.append(before if before in ('Computed', 'InheritedStyle') else None)
            elif word == '}' and scopes:
                scopes.pop()
            # Only explicit style constructors/patterns, not arbitrary width type labels.
            following = words[index + 1:index + 2]
            initialized = bool(scopes and scopes[-1] in ('Computed', 'InheritedStyle') and (
                following == [':'] or (before in ('{', ',') and following in ([','], ['}']))))
            if word in fields and (before == '.' or initialized):
                kind = access(words, index) if before == '.' else 'initialize'
                for field in fields[word]:
                    counts[field, context, kind] += 1
            # Calls only: declarations/imports do not create new escapes.
            called = words[index + 1:index + 2] == ['('] and before in ('.', '::')
            if called and (word in USED or (owner != 'style' and word in MUTABLE)):
                counts['@' + word, context, 'escape'] += 1
            if owner != 'style' and words[index:index + 3] == ['&', 'mut', 'Computed']:
                counts['@mut-Computed', context, 'escape'] += 1
    return counts


def guarded(key):
    field, owner, kind = key
    return not owner.endswith(':test') and (
        kind == 'escape' or (owner != 'style' and kind in ('write', 'mutable', 'initialize')))


def growth(current, previous, label):
    return [f'{label}: {"/".join(key)} grew {previous.get(key, 0)} -> {count}'
            for key, count in sorted(current.items())
            if guarded(key) and count > previous.get(key, 0)]
