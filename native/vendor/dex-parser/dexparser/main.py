"""CLI for the DEX parser."""

import argparse
import sys

from dexparser import DEX_from_source, DEXHelper


def init_parser():
    parser = argparse.ArgumentParser(
        prog="dexparser",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        description="DEX Parser",
    )
    parser.add_argument("-i", "--input", type=str, help="Input DEX file")
    parser.add_argument(
        "-s",
        "--strings",
        action="store_true",
        help="Extract strings from the DEX",
    )
    parser.add_argument("-v", "--verbose", action="store_true", help="verbose")
    return parser.parse_args()


def app(argv=None):
    args = init_parser() if argv is None else init_parser()
    if not args.input:
        return 0

    d = DEX_from_source(args.input)
    dh = DEXHelper.from_rawdex(d)

    print(dh)
    print(d["header"])

    for cls in dh.get_classes():
        print("CLASS", cls)

    for method in dh.get_methods():
        print("METHOD", method, method.get_internal_struct())
        code = method.get_code()
        if code:
            print(
                "\t CODE",
                code.debug_info_off,
                code.insns_size,
                len(code["insns"].value),
            )

    for field in dh.get_fields():
        print("FIELD", field, field.get_internal_struct())

    if args.strings:
        for idx, s in enumerate(dh.get_strings()):
            print(f"  [{idx}] {s!r}")

    return 0


if __name__ == "__main__":
    sys.exit(app())
