"""Generate Cargo package contracts from reviewed ownership and existing versions.

The planner never invents a registry dependency or version. Dependency discovery
is conservative: every identifier, including test-only code, may name a crate.
Keeping an extra existing dependency is safe; dropping an unresolved one is not.
Cargo/compiler acceptance is a separate prerequisite for the generated tree.
"""
from copy import deepcopy
import json
import posixpath
import tomllib

from migration_plan import RANK
from rewrite_paths import lexemes
from source_relocation import PACKAGE_DIRS

SUPPORT = 'engine-test-support'


def optional_copy(spec):
    result = deepcopy(spec) if isinstance(spec, dict) else {'version': spec}
    result['optional'] = True
    return result


def support_dependencies(data):
    """Expose only already used dev crates to the explicit test-helper build.

    Normal implementations keep their original dependency specifications.
    A dev-only crate gets an optional copy of its exact original specification;
    selecting the support feature unifies it with that same dev dependency.
    """
    enables = data['features'][SUPPORT]
    for name, spec in sorted(data['dev-dependencies'].items()):
        if name not in data['dependencies']:
            data['dependencies'][name] = optional_copy(spec)
            enables.append('dep:' + name)
        else:
            normal = data['dependencies'][name]
            existing = normal.get('features', []) if isinstance(normal, dict) else []
            extra = spec.get('features', []) if isinstance(spec, dict) else []
            enables.extend(name + '/' + feature for feature in extra if feature not in existing)


def value(item):
    if isinstance(item, bool):
        return str(item).lower()
    if isinstance(item, str):
        return json.dumps(item, ensure_ascii=False)
    if isinstance(item, (int, float)):
        return str(item)
    if isinstance(item, list):
        return '[' + ', '.join(map(value, item)) + ']'
    if isinstance(item, dict):
        return '{ ' + ', '.join(key(k) + ' = ' + value(v)
                               for k, v in sorted(item.items())) + ' }'
    raise ValueError(f'Unsupported Cargo value: {type(item)}')


def key(name):
    return name if all(c.isalnum() or c in '_-' for c in name) else value(name)


def serialize(data):
    lines = ['# Контракт фазы HTML/CSS; версии зависимостей наследуются из исходного крейта.']
    for section, entries in data.items():
        if isinstance(entries, list):
            for entry in entries:
                lines.extend(['', '[[' + section + ']]'])
                lines.extend(key(k) + ' = ' + value(v) for k, v in sorted(entry.items()))
        else:
            lines.extend(['', '[' + section + ']'])
            lines.extend(key(k) + ' = ' + value(v) for k, v in sorted(entries.items()))
    return '\n'.join(lines) + '\n'


def dependencies(entries, identifiers, old_directory, new_directory):
    result = {}
    for name, spec in sorted(entries.items()):
        library = {'xml-rs': 'xml'}.get(name, name.replace('-', '_'))
        if library not in identifiers:
            continue
        spec = deepcopy(spec)
        if isinstance(spec, dict) and 'path' in spec:
            target = posixpath.normpath(posixpath.join(old_directory, spec['path']))
            spec['path'] = posixpath.relpath(target, new_directory)
        result[name] = spec
    return result


def original_contract(manifests):
    """Recover the same external contract from either side of the migration.

    External specifications may occur in several packages but must agree.
    Generated internal dependencies and feature forwarding are identified by
    the explicit reviewed package set, never by a broad name prefix.
    """
    upper = 'crates/html/Cargo.toml'
    documents = {path: tomllib.loads(text) for path, text in manifests.items()}
    if upper not in documents:
        raise ValueError('Missing kamin-html manifest')
    base = deepcopy(documents[upper])
    for section in ('dependencies', 'dev-dependencies'):
        combined = {}
        for path, document in sorted(documents.items()):
            if document['package']['name'] not in RANK:
                raise ValueError(f'Unexpected package in migration input: {path}')
            for name, spec in document.get(section, {}).items():
                if name in RANK:
                    continue
                support = document.get('features', {}).get(SUPPORT, [])
                dev = document.get('dev-dependencies', {}).get(name)
                if (section == 'dependencies' and 'dep:' + name in support
                        and document['package']['name'] != 'kamin-html'):
                    if dev is None or spec != optional_copy(dev):
                        raise ValueError(f'Conflicting external test dependency: {path}: {name}')
                    continue
                spec = deepcopy(spec)
                if isinstance(spec, dict) and 'path' in spec:
                    target = posixpath.normpath(posixpath.join(posixpath.dirname(path), spec['path']))
                    spec['path'] = posixpath.relpath(target, 'crates/html')
                if name in combined and combined[name] != spec:
                    raise ValueError(f'Conflicting external dependency contracts: {section}.{name}')
                combined[name] = spec
        base[section] = combined
    feature_names = {name for document in documents.values()
                     for name in document.get('features', {})} - {SUPPORT}
    for path, document in documents.items():
        lower = document['package']['name'] != 'kamin-html'
        expected = feature_names | ({SUPPORT} if lower else set())
        if set(document.get('features', {})) != expected:
            raise ValueError(f'Inconsistent phase features: {path}')
        for name, enables in document.get('features', {}).items():
            generated = {package + '/' + name for package in RANK}
            if name == SUPPORT:
                probe = deepcopy(document)
                probe['features'][SUPPORT] = []
                # Remove generated copies, then derive the permitted selectors
                # again from the unchanged dev-dependency specifications.
                for dependency in list(probe['dependencies']):
                    dev = probe.get('dev-dependencies', {}).get(dependency)
                    if dev is not None and probe['dependencies'][dependency] == optional_copy(dev):
                        del probe['dependencies'][dependency]
                support_dependencies(probe)
                generated = set(probe['features'][SUPPORT])
            if set(enables) - generated:
                raise ValueError(f'Unreviewed feature contract: {path}: {name}')
    base['features'] = {name: [] for name in sorted(feature_names)}
    return serialize(base)


def generate(original, package_sources, edges):
    """Return workspace-relative manifest paths -> deterministic TOML text.

    Input sources include examples/tests in the owning package. No dependency
    edge is guessed from naming conventions; all internal edges come from the
    source planner and must point down the reviewed phase order.
    """
    manifest = tomllib.loads(original)
    allowed = {'package', 'features', 'example', 'dependencies', 'dev-dependencies', 'lints'}
    unknown = set(manifest) - allowed
    if unknown:
        raise ValueError(f'Unreviewed original manifest sections: {sorted(unknown)}')
    if manifest['package']['name'] != 'kamin-html':
        raise ValueError('Expected the original kamin-html manifest')
    if any(isinstance(spec, dict) and spec.get('optional')
           for spec in manifest.get('dependencies', {}).values()):
        raise ValueError('Original optional dependencies require an explicit recovery policy')
    for source, target in edges:
        if source not in RANK or target not in RANK or RANK[source] <= RANK[target]:
            raise ValueError(f'Invalid dependency direction: {source} -> {target}')
    result = {}
    for package in sorted(RANK):
        directory = 'crates/' + PACKAGE_DIRS[package]
        identifiers = {word for text in package_sources.get(package, [])
                       for word, _, _, _ in lexemes(text) if word is not None}
        upper = package == 'kamin-html'
        data = {'package': deepcopy(manifest['package'])}
        data['package']['name'] = package
        if not upper:
            forbidden = set(data['package']) - {'name', 'version', 'edition'}
            if forbidden:
                raise ValueError(f'Package metadata needs an explicit copy policy: {forbidden}')
        features = deepcopy(manifest.get('features', {}))
        if SUPPORT in features:
            raise ValueError('The original upper crate must not define engine-test-support')
        for name, enables in features.items():
            if enables:
                raise ValueError(f'Feature {name} needs an explicit propagation policy')
        if not upper:
            features[SUPPORT] = []
        data['features'] = features
        if upper and 'example' in manifest:
            data['example'] = deepcopy(manifest['example'])
        data['dependencies'] = dependencies(manifest.get('dependencies', {}), identifiers,
                                            'crates/html', directory)
        data['dev-dependencies'] = dependencies(manifest.get('dev-dependencies', {}), identifiers,
                                                'crates/html', directory)
        for source, target in sorted(edges):
            if source != package:
                continue
            data['dependencies'][target] = {'path': '../' + PACKAGE_DIRS[target]}
            for name in features:
                if name != SUPPORT:
                    features[name].append(target + '/' + name)
        # Test cases accepting the upper callback table are compiled into the
        # same lower library instance. GPUI's real test types must be available
        # when that support feature is selected through upper dev-dependencies.
        if not upper:
            support_dependencies(data)
        if upper:
            # Activate every lower helper directly from the upper test owner;
            # normal lower-to-lower dependencies never forward this feature.
            for target in sorted(RANK.keys() - {'kamin-html'}):
                data['dev-dependencies'][target] = {
                    'path': '../' + PACKAGE_DIRS[target], 'features': ['engine-test-support']}
        data['lints'] = deepcopy(manifest.get('lints', {}))
        text = serialize(data)
        if tomllib.loads(text) != data:
            raise ValueError('Cargo serialization did not preserve the generated contract')
        result[directory + '/Cargo.toml'] = text
    return result
