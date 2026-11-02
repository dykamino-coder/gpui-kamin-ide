"""Conservative lexical field/owner/access aggregates, independent of source lines.

Unknown receivers are deliberately over-counted; declared unrelated field owners
can exclude homonyms. Owners are top-level HTML modules, so moving a file within a phase does not change debt.
Style construction may write operands; non-style writes/projections and mutable
escape APIs are ratcheted. Whole-value mutation must also be reviewed manually.
"""

from collections import Counter, defaultdict
import re

from computed_test_scopes import scopes as test_scopes
from computed_receiver_types import ReceiverTypes

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
            if 'StyleGroup<' in line:
                continue
            result[match[1]].append(owner + '.' + match[1])
        if line == '}':
            owner = None
    if not result and not re.search(r'\bstruct (Computed|InheritedStyle)\b', source):
        raise ValueError('no Computed/InheritedStyle field definitions')
    return result


def grouped_definitions(sources):
    """Keep historical property IDs across COW record boundaries.

    Handles are ownership edges; the payload's fields remain the guarded CSS
    operands. Record literals/patterns count just like the former flat literals.
    """
    fields = definitions(sources[DEFINITIONS])
    constructors = {'Computed', 'InheritedStyle'}
    for path, source in sources.items():
        if not path.startswith(PREFIX + 'style/computed/groups/'):
            continue
        owner = None
        for line in source.splitlines():
            if match := re.match(r'^pub struct ((Reset|Inherited)\w+)\s*\{', line):
                constructors.add(match[1])
                owner = 'Computed' if match[2] == 'Reset' else 'InheritedStyle'
            if owner and (match := re.match(r'^    pub (\w+):', line)):
                field = owner + '.' + match[1]
                if field not in fields[match[1]]:
                    fields[match[1]].append(field)
            if line == '}':
                owner = None
    return fields, constructors


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
    fields, constructors = grouped_definitions(sources)
    group_handles = set(re.findall(
        r'^    pub (\w+): StyleGroup<', sources[DEFINITIONS], re.MULTILINE))
    inherited_groups = set(re.findall(
        r'^    pub (\w+): StyleGroup<',
        sources[DEFINITIONS].split('pub struct InheritedStyle {', 1)[-1].split('}', 1)[0],
        re.MULTILINE)) if 'pub struct InheritedStyle {' in sources[DEFINITIONS] else set()
    counts = Counter()
    streams = test_scopes(sources)
    receivers = ReceiverTypes({path: words for path, (words, _, _) in streams.items()})
    for path, (words, test_file, ranges) in sorted(streams.items()):
        owner = path.removeprefix(PREFIX).split('/')[0].removesuffix('.rs')
        scopes = []
        for index, word in enumerate(words):
            context = owner + ':test' if test_file or any(a <= index < b for a, b in ranges) else owner
            before = words[index - 1] if index else None
            if word == '{':
                scopes.append(before if before in constructors else None)
            elif word == '}' and scopes:
                scopes.pop()
            # Only explicit style constructors/patterns, not arbitrary width type labels.
            following = words[index + 1:index + 2]
            initialized = bool(scopes and scopes[-1] in constructors and (
                following == [':'] or (before in ('{', ',') and following in ([','], ['}']))))
            # A grouped inheritance projection owns no second CSS operand.
            # Keep whole-handle writes guarded, and count its payload below.
            group_projection = (word == 'inh' and words[index + 1:index + 2] == ['.']
                                and index + 2 < len(words)
                                and words[index + 2] in inherited_groups)
            # Moving CSS operands behind handles must not create an unguarded
            # whole-record mutation path outside style. Leaf projections keep
            # their historical IDs; whole handles have zero new escape budget.
            if (owner != 'style' and word in group_handles and before == '.'
                    and following not in (['.'], ['['])):
                kind = access(words, index)
                if kind in ('write', 'mutable') and receivers.may_write(path, words, index):
                    counts['@style-group-' + kind, context, 'escape'] += 1
            if word in fields and (before == '.' or initialized) and not group_projection:
                kind = access(words, index) if before == '.' else 'initialize'
                if kind in ('write', 'mutable') and not receivers.may_write(path, words, index):
                    continue
                for field in fields[word]:
                    counts[field, context, kind] += 1
            # Calls only: declarations/imports do not create new escapes.
            called = words[index + 1:index + 2] == ['('] and before in ('.', '::')
            if called and (word in USED or (owner != 'style' and word in MUTABLE and (word not in {'make_mut', 'get_mut', 'borrow_mut'} or receivers.may_escape(path, words, index)))):
                counts['@' + word, context, 'escape'] += 1
            if owner != 'style' and words[index:index + 3] == ['&', 'mut', 'Computed']:
                counts['@mut-Computed', context, 'escape'] += 1
            if (owner != 'style' and words[index:index + 2] == ['&', 'mut']
                    and words[index + 2:index + 3]
                    and words[index + 2] in constructors - {'Computed', 'InheritedStyle'}):
                counts['@mut-style-group', context, 'escape'] += 1
    return counts


def guarded(key):
    field, owner, kind = key
    return not owner.endswith(':test') and (
        kind == 'escape' or (owner != 'style' and kind in ('write', 'mutable', 'initialize')))


def growth(current, previous, label):
    return [f'{label}: {"/".join(key)} grew {previous.get(key, 0)} -> {count}'
            for key, count in sorted(current.items())
            if guarded(key) and count > previous.get(key, 0)]
