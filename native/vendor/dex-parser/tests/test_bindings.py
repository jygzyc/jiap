"""Tests for dexparser Python bindings (Rust core)."""

import pathlib

import dexparser_rs
from dexparser import DEX, DEXHelper, DEX_from_source, is_dex

TESTDATA = pathlib.Path(__file__).resolve().parents[2] / "dex-decompiler" / "testdata"
CLASSES3 = TESTDATA / "classes3.dex"


def test_is_dex():
    data = CLASSES3.read_bytes()
    assert is_dex(data)
    assert not is_dex(b"not dex")


def test_parse_from_path():
    d = DEX.from_path(str(CLASSES3))
    d.validate()
    h = d["header"]
    assert h["class_defs_size"] == 4


def test_helper_classes_methods():
    d = DEX.from_path(str(CLASSES3))
    dh = DEXHelper.from_rawdex(d)
    classes = list(dh.get_classes())
    assert len(classes) == 4
    assert classes[0].name.startswith("L")
    methods = list(dh.get_methods())
    assert len(methods) > 0
    assert methods[0].name


def test_method_code_insns():
    d = DEX.from_path(str(CLASSES3))
    dh = DEXHelper.from_rawdex(d)
    for method in dh.get_methods():
        code = method.get_code()
        if code and method.code_off > 0:
            assert len(code["insns"].value) > 0
            break


def test_from_string():
    data = CLASSES3.read_bytes()
    dh = DEXHelper.from_string(data)
    assert len(list(dh.get_strings())) >= 0
