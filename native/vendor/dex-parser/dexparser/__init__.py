"""DEX parser: Rust core with Python bindings."""

from dexparser_rs import (
    ClassHelper,
    DEX as _RustDEX,
    DEXHelper,
    FieldHelper,
    MethodHelper,
    is_dex,
)

__all__ = [
    "DEX",
    "DEXHelper",
    "ClassHelper",
    "MethodHelper",
    "FieldHelper",
    "is_dex",
    "DEX_from_source",
]


def DEX_from_source(source):
    """
    Parse a DEX from a file path, bytes, or a readable stream.

    Examples::

        DEX_from_source("classes.dex")
        DEX_from_source(open("classes.dex", "rb"))
        DEX_from_source(bytes_data)
    """
    if isinstance(source, (bytes, bytearray)):
        return _RustDEX(bytes(source))
    if isinstance(source, str):
        return _RustDEX.from_path(source)
    if hasattr(source, "read"):
        data = source.read()
        if isinstance(data, str):
            data = data.encode("latin-1")
        return _RustDEX(data)
    raise TypeError(f"unsupported DEX source: {type(source)!r}")


def DEX(source):
    """Parse a DEX file from a path, bytes, or stream (alias for :func:`DEX_from_source`)."""
    return DEX_from_source(source)


# Class methods on the Rust type, also available on the factory
DEX.from_path = _RustDEX.from_path
DEX.from_bytes = _RustDEX.from_bytes
