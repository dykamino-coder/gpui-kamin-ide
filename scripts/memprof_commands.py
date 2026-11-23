"""Optional profiler commands kept separate from stack folding and comparison."""

import memprof_build
import memprof_peak


def register(sub, run):
    memprof_build.add_parser(sub)
    memprof_peak.add_parser(sub)
    run.add_argument("--peak", action="store_true",
                     help="also export the recorded live-heap peak; implies --owners")


def dispatch(args, profiler):
    if args.command == "build":
        memprof_build.build_command(args)
    elif args.command == "peak":
        memprof_peak.command(args, profiler)
    else:
        return False
    return True
