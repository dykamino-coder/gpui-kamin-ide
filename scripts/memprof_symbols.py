"""Symbols of one engine_bench image for the profilers (dbghelp, PDB beside the exe).

Resolves the return addresses a probe recorded to function names, and tells what the PDB cannot
name: an image without function records, and functions the linker folded into one body (/OPT:ICF).
"""

import collections
import pathlib
import re

HASH = re.compile(r"::h[0-9a-f]{16}$")


class Symbols:
    """One image loaded at the base address the process ran it at (dbghelp, PDB beside the exe)."""

    count = 0

    def __init__(self, exe, base):
        import ctypes as c
        import ctypes.wintypes as w

        self.c = c
        self.dbg = c.WinDLL("dbghelp", use_last_error=True)
        self.dbg.SymInitializeW.argtypes = [w.HANDLE, w.LPCWSTR, w.BOOL]
        self.dbg.SymLoadModuleExW.argtypes = [
            w.HANDLE, w.HANDLE, w.LPCWSTR, w.LPCWSTR, c.c_uint64, w.DWORD, c.c_void_p, w.DWORD,
        ]
        self.dbg.SymLoadModuleExW.restype = c.c_uint64
        self.dbg.SymFromAddr.argtypes = [w.HANDLE, c.c_uint64, c.POINTER(c.c_uint64), c.c_void_p]
        self.each = c.WINFUNCTYPE(w.BOOL, c.c_void_p, w.ULONG, c.c_void_p)
        self.dbg.SymEnumSymbolsW.argtypes = [w.HANDLE, c.c_uint64, w.LPCWSTR, self.each, c.c_void_p]
        Symbols.count += 1
        self.handle = 0x5A0000 + Symbols.count  # any unique value: no live process is attached
        exe = pathlib.Path(exe).resolve()
        if not self.dbg.SymInitializeW(self.handle, str(exe.parent), False):
            raise OSError(f"SymInitialize failed: {c.get_last_error()}")
        self.base = self.dbg.SymLoadModuleExW(self.handle, None, str(exe), None, base, 0, None, 0)
        if not self.base:
            raise OSError(f"cannot load symbols of {exe}: {c.get_last_error()}")
        self.names = {}
        self.scanned = None
        self.chains = {}
        self.dbg.SymAddrIncludeInlineTrace.argtypes = [w.HANDLE, c.c_uint64]
        self.dbg.SymAddrIncludeInlineTrace.restype = w.DWORD
        self.dbg.SymQueryInlineTrace.argtypes = [w.HANDLE, c.c_uint64, w.DWORD, c.c_uint64, c.c_uint64,
                                                 c.POINTER(w.DWORD), c.POINTER(w.DWORD)]
        self.dbg.SymFromInlineContext.argtypes = [w.HANDLE, c.c_uint64, w.ULONG, c.POINTER(c.c_uint64), c.c_void_p]

    def add_module(self, path, base):
        """Another image of the same process (a system DLL: its exported names, or its PDB if present)."""
        return bool(self.dbg.SymLoadModuleExW(self.handle, None, str(path), None, base, 0, None, 0))

    def frames(self, address):
        """Function names at a return address, innermost first: the functions inlined there, then the
        function whose body holds the address. An optimised build inlines most small functions; the
        PDB's line tables keep where each inlined body came from."""
        chain = self.chains.get(address)
        if chain is None:
            c = self.c
            chain, at = [], address - 1
            count = self.dbg.SymAddrIncludeInlineTrace(self.handle, at)
            context, index = c.c_uint32(), c.c_uint32()
            if count and self.dbg.SymQueryInlineTrace(self.handle, at, 0, at, at, c.byref(context), c.byref(index)):
                info = c.create_string_buffer(88 + 2048)
                offset = c.c_uint64()
                for i in range(count):
                    c.c_uint32.from_buffer(info, 0).value = 88
                    c.c_uint32.from_buffer(info, 80).value = 2048
                    if self.dbg.SymFromInlineContext(self.handle, at, context.value + i, c.byref(offset), info):
                        length = c.c_uint32.from_buffer(info, 76).value
                        chain.append(HASH.sub("", info.raw[84:84 + length].decode("utf-8", "replace")))
            chain.append(self.name(address))
            chain = self.chains[address] = tuple(chain)
        return chain

    def scan(self):
        """(function records in the PDB, {name: every function name at its address} of folded functions).

        The linker's identical-code folding (/OPT:ICF) leaves one body for several functions; the PDB
        keeps a function record for each of them at the shared address, and a stack frame there gets
        the name of any one of them."""
        if self.scanned is None:
            c = self.c
            at = collections.defaultdict(set)

            def visit(info, _size, _context):
                # SYMBOL_INFOW: Address at 56, Tag at 72, NameLen at 76, Name at 84.
                if c.c_uint32.from_address(info + 72).value == 5:  # SymTagFunction
                    name = c.wstring_at(info + 84, c.c_uint32.from_address(info + 76).value)
                    at[c.c_uint64.from_address(info + 56).value].add(HASH.sub("", name.rstrip("\0")))
                return True

            self.dbg.SymEnumSymbolsW(self.handle, self.base, "*", self.each(visit), None)
            folded = {}
            for names in at.values():
                if len(names) > 1:
                    group = sorted(names)
                    folded.update((name, group) for name in group)
            self.scanned = (len(at), sum(len(names) for names in at.values()), folded)
        return self.scanned

    def name(self, address):
        if address not in self.names:
            c = self.c
            info = c.create_string_buffer(88 + 2048)  # SYMBOL_INFO followed by the name
            c.c_uint32.from_buffer(info, 0).value = 88
            c.c_uint32.from_buffer(info, 80).value = 2048
            offset = c.c_uint64()
            # A return address belongs to the call before it.
            if self.dbg.SymFromAddr(self.handle, address - 1, c.byref(offset), info):
                length = c.c_uint32.from_buffer(info, 76).value
                text = info.raw[84 : 84 + length].decode("utf-8", "replace")
                self.names[address] = HASH.sub("", text)
            else:
                self.names[address] = "?"
        return self.names[address]
