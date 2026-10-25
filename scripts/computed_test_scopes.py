"""Identify explicit test-only Rust items without filename-based exemptions.

Only exact cfg(test) and test attributes establish test ownership. Default-path
external modules inherit it, including descendants. Custom path modules and other
cfg branches remain production, conservatively; this inventory is not rustc.
"""

from rust_layer_paths import file_module, tokens


def scopes(sources):
    streams, test_ranges, test_modules = {}, {}, []
    for path, source in sources.items():
        words = [word for word, _ in tokens(source)]
        streams[path] = words
        module = file_module(path)
        parents, ranges = [], []
        index = 0
        while index < len(words):
            exact_cfg = words[index:index + 7] == ['#', '[', 'cfg', '(', 'test', ')', ']']
            exact_test = words[index:index + 4] == ['#', '[', 'test', ']']
            if exact_cfg or exact_test:
                start, end = index, index + (7 if exact_cfg else 4)
                # Include preceding attributes; a custom path must not exempt an
                # unrelated file at the default module path.
                while start and words[start - 1] == ']':
                    previous, depth = start - 2, 1
                    while previous >= 0 and depth:
                        depth += (words[previous] == ']') - (words[previous] == '[')
                        previous -= 1
                    if previous < 0 or words[previous] != '#':
                        break
                    start = previous
                while end < len(words) and words[end] not in ('{', ';'):
                    end += 1
                declaration = words[start:end]
                if 'mod' in declaration and 'path' not in declaration:
                    at = declaration.index('mod')
                    test_modules.append(module + (declaration[at + 1],))
                if end < len(words) and words[end] == '{':
                    depth, end = 1, end + 1
                    while end < len(words) and depth:
                        depth += (words[end] == '{') - (words[end] == '}')
                        end += 1
                ranges.append((start, end))
            if words[index] == '{':
                parents.append(module)
                if index >= 2 and words[index - 2] == 'mod':
                    module += (words[index - 1],)
            elif words[index] == '}' and parents:
                module = parents.pop()
            index += 1
        test_ranges[path] = ranges
    result = {}
    for path, words in streams.items():
        module = file_module(path)
        whole = any(module[:len(prefix)] == prefix for prefix in test_modules)
        result[path] = words, whole, test_ranges[path]
    return result
