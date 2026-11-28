"""Optional profiler commands kept separate from stack folding and comparison."""

import memprof_build
import memprof_peak
import memprof_idle


def register(sub, run):
    memprof_build.add_parser(sub)
    memprof_peak.add_parser(sub)
    memprof_idle.add_parser(sub)
    run.add_argument("--peak", action="store_true",
                     help="also export the recorded live-heap peak; implies --owners")
    run.add_argument("--owner-walk", help="with --peak: root walk number (1-based) or last")


def dispatch(args, profiler):
    if args.command == "build":
        memprof_build.build_command(args)
    elif args.command == "peak":
        memprof_peak.command(args, profiler)
    elif args.command == "idle":
        memprof_idle.command(args)
    else:
        return False
    return True
