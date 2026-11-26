"""Select one root walk without mixing the owners of different documents."""


def count(path):
    walks = 0
    gpu = False
    with open(path, encoding="utf-8") as stream:
        for line in stream:
            if line.startswith("o ") and (not walks or gpu):
                walks += 1
                gpu = False
            elif line.startswith("g "):
                gpu = True
    return walks


def resolve(path, requested):
    walks = count(path)
    if requested is None:
        if walks > 1:
            raise SystemExit("peak export needs one owner walk; select --owner-walk N or last")
        return 1, walks
    try:
        chosen = walks if requested == "last" else int(requested)
    except ValueError:
        raise SystemExit("--owner-walk expects a positive number or last") from None
    if not 1 <= chosen <= walks:
        raise SystemExit(f"--owner-walk {requested}: file contains {walks} owner walks")
    return chosen, walks


def select(stream, wanted):
    """Keep allocations before the chosen walk and only that walk's labels.

    Renderer GPU records terminate each walk. Allocations made while earlier
    walks ran remain allocations of this process; they are not erased.
    """
    walk = 0
    gpu = False
    for line in stream:
        if line.startswith("o "):
            if not walk or gpu:
                walk += 1
                gpu = False
            if walk == wanted:
                yield line
        elif line.startswith("g "):
            gpu = True
            if walk == wanted:
                yield line
        elif line.startswith("#") or walk < wanted:
            yield line
