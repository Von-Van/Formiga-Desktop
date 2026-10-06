#!/usr/bin/env python3
"""formiga: the Formiga developer toolkit.

  formiga validate [--quick] [--expansions] [--step NAME]... [--json]
  formiga scenario list [--json]
  formiga scenario NAME [--no-launch] [--wait] [--pinned] [--fixed-date] [--json]
  formiga capture NAME|all [--window] [--pinned] [--out PNG] [--json]

Run from anywhere: it works on the Formiga-Desktop checkout it lives in, and finds Formiga Hill and
Formiga Home beside it. Python 3.9 or newer, standard library only. See devtools/README.md.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from formiga_dev import capture, scenarios, validate  # noqa: E402
from formiga_dev.report import Result, Step, emit  # noqa: E402


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(
        prog="formiga", description="The Formiga developer toolkit.",
        epilog="Every command takes --json for one machine-readable document on stdout.")
    commands = parser.add_subparsers(dest="command", required=True)

    check = commands.add_parser("validate", help="did this change break Formiga?",
                                description=validate.__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    check.add_argument("--quick", action="store_true", help="format, lint and fixtures only")
    check.add_argument("--expansions", action="store_true",
                       help="also run Hill's and Home's tests on this Desktop")
    check.add_argument("--step", action="append", choices=validate.ALL_STEPS,
                       help="run only this step (repeatable)")
    check.add_argument("--json", action="store_true")

    scene = commands.add_parser("scenario", help="open Formiga in a known state",
                                description=scenarios.__doc__,
                                formatter_class=argparse.RawDescriptionHelpFormatter)
    scene.add_argument("name", help="a scenario name, or `list`")
    scene.add_argument("--no-launch", action="store_true",
                       help="prepare it and print the command, without opening anything")
    scene.add_argument("--wait", action="store_true", help="wait for the app to close")
    scene.add_argument("--pinned", action="store_true",
                       help="build Hill or Home on their own Desktop release, not this checkout")
    scene.add_argument("--fixed-date", action="store_true",
                       help="grow the colony to the fixtures' fixed date, not to now")
    scene.add_argument("--json", action="store_true")

    picture = commands.add_parser("capture", help="picture a scenario",
                                  description=capture.__doc__,
                                  formatter_class=argparse.RawDescriptionHelpFormatter)
    picture.add_argument("name", help="a scenario name, or `all` for every one that can be drawn")
    picture.add_argument("--window", action="store_true",
                         help="picture Hill's or Home's real window (close, not exact)")
    picture.add_argument("--pinned", action="store_true",
                         help="build Hill or Home on their own Desktop release, not this checkout")
    picture.add_argument("--out", help="where to write the PNG (default .dev/captures/NAME.png)")
    picture.add_argument("--json", action="store_true")

    args = parser.parse_args(argv)
    if args.command == "validate":
        result = validate.validate(quick=args.quick, expansions=args.expansions, only=args.step)
    elif args.command == "scenario":
        result = (scenarios.listing() if args.name == "list" else scenarios.launch(
            args.name, no_launch=args.no_launch, wait=args.wait, pinned=args.pinned,
            fixed_date=args.fixed_date))
    elif args.name == "all":
        result = capture_all(args.window, args.pinned)
    else:
        result = capture.capture(args.name, window=args.window, pinned=args.pinned, out=args.out)
    return emit(result, args.json)


def capture_all(window: bool, pinned: bool) -> Result:
    """Every scenario that can be pictured, one step each."""
    result = Result("capture all", data={"captures": []})
    for name, scenario in scenarios.SCENARIOS.items():
        if not scenario.capture or (window and scenario.capture.window is None):
            continue
        one = capture.capture(name, window=window, pinned=pinned)
        failed = [step for step in one.steps if step.status == "failed"]
        if one.error or failed:
            problems = [problem for step in failed for problem in step.problems]
            result.steps.append(Step(name, "failed", one.error or failed[0].summary,
                                     problems=problems))
        else:
            result.steps.append(Step(name, "passed", f"{one.data['width']}×{one.data['height']}"))
            result.data["captures"].append({"scenario": name, "path": one.data["path"]})
    return result


if __name__ == "__main__":
    sys.exit(main())
