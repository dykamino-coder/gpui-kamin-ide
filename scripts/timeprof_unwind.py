"""Win64 stack unwinder over a copy of a thread's stack (timeprof sampler).

The sampler keeps a thread suspended only while it copies the registers and the used part of the
stack; the walk happens here afterwards, on the copy, so the target runs meanwhile. dbghelp's
StackWalk64 on the live thread costs about a millisecond per sample with the thread stopped.

The walk is the documented x64 exception-handling scheme, the same data RtlVirtualUnwind reads: each
image lists its functions in .pdata (RUNTIME_FUNCTION: begin, end, unwind info); the unwind info is the
function's prolog written as undo steps (pushed registers, stack allocation, frame register, saved
registers), optionally chained to a parent record. A frame is undone by applying the steps the
program counter has passed, then popping the return address. A leaf function has no record: its
return address is on top of the stack. In an epilog (stack release, pops, ret or tail jump) the
remaining instructions are simulated instead, as the system unwinder does.

Tables and code bytes come from the image files on disk, mapped once per module.
"""

import bisect
import mmap
import struct

RSP, RBP = 4, 5
PUSH_NONVOL, ALLOC_LARGE, ALLOC_SMALL, SET_FPREG, SAVE_NONVOL, SAVE_NONVOL_FAR = 0, 1, 2, 3, 4, 5
EPILOG, SPARE, SAVE_XMM128, SAVE_XMM128_FAR, PUSH_MACHFRAME = 6, 7, 8, 9, 10
CHAININFO = 4
U64 = struct.Struct("<Q")


class Lost(Exception):
    """The walk left the copied stack or the known images."""


class Image:
    """Function table and code bytes of one loaded image, from its file."""

    def __init__(self, path, base, size):
        self.path, self.base, self.end = path, base, base + size
        self.infos = {}
        self.begins, self.entries, self.sections, self.data = [], [], [], b""
        try:
            with open(path, "rb") as handle:
                self.data = mmap.mmap(handle.fileno(), 0, access=mmap.ACCESS_READ)
            self._parse()
        except (OSError, ValueError, struct.error):
            self.begins, self.entries = [], []

    def _parse(self):
        data = self.data
        pe = struct.unpack_from("<I", data, 0x3C)[0]
        count, optional_size = struct.unpack_from("<H", data, pe + 6)[0], struct.unpack_from("<H", data, pe + 20)[0]
        optional = pe + 24
        if struct.unpack_from("<H", data, optional)[0] != 0x20B:
            return
        table = optional + optional_size
        self.sections = [struct.unpack_from("<IIII", data, table + 40 * i + 8) for i in range(count)]
        rva, size = struct.unpack_from("<II", data, optional + 112 + 8 * 3)  # exception directory
        raw = self.read(rva, size - size % 12) if rva else b""
        self.entries = [entry for entry in struct.iter_unpack("<III", raw) if entry[0]]
        self.entries.sort()
        self.begins = [entry[0] for entry in self.entries]

    def read(self, rva, length):
        """Bytes of the image at a relative address (file contents; empty outside the file's sections)."""
        for size, address, raw_size, raw in self.sections:
            if address <= rva < address + max(size, raw_size):
                start = raw + rva - address
                return self.data[start:min(start + length, raw + raw_size)]
        return b""

    def function(self, rva):
        """(begin, end, unwind info address) of the function holding `rva`, or None for a leaf."""
        index = bisect.bisect_right(self.begins, rva) - 1
        if index >= 0 and rva < self.entries[index][1]:
            return self.entries[index]
        return None

    def primary(self, function):
        """The first record of a function split into chained fragments."""
        for _ in range(32):
            chained = self.info(function[2])[4]
            if chained is None:
                break
            function = chained
        return function[0]

    def info(self, address):
        """Parsed unwind info: (prolog size, frame register, frame offset, steps, chained function)."""
        known = self.infos.get(address)
        if known is None:
            known = self.infos[address] = self._info(address)
        return known

    def _info(self, address):
        if address & 1:  # an indirection to another function record
            return 0, 0, 0, (), struct.unpack("<III", self.read(address - 1, 12))
        head = self.read(address, 4)
        flags, prolog, count, frame = head[0] >> 3, head[1], head[2], head[3]
        codes = self.read(address + 4, 2 * count + 16)
        steps, i = [], 0
        while i < count:
            offset, op, value = codes[2 * i], codes[2 * i + 1] & 15, codes[2 * i + 1] >> 4
            slots, operand = 1, 0
            if op == ALLOC_LARGE:
                slots = 2 if value == 0 else 3
                operand = (struct.unpack_from("<H", codes, 2 * i + 2)[0] * 8 if value == 0
                           else struct.unpack_from("<I", codes, 2 * i + 2)[0])
            elif op in (SAVE_NONVOL, SAVE_XMM128):
                slots, operand = 2, struct.unpack_from("<H", codes, 2 * i + 2)[0] * 8
            elif op in (SAVE_NONVOL_FAR, SAVE_XMM128_FAR):
                slots, operand = 3, struct.unpack_from("<I", codes, 2 * i + 2)[0]
            elif op == ALLOC_SMALL:
                operand = value * 8 + 8
            elif op == SPARE:
                slots = 2
            steps.append((offset, op, value, operand))
            i += slots
        chained = None
        if flags & CHAININFO:
            chained = struct.unpack_from("<III", codes, 2 * (count + (count & 1)))
        return prolog, frame & 15, (frame >> 4) * 16, tuple(steps), chained


class Stack:
    """A copy of thread stack memory starting at `base`."""

    def __init__(self, base, data):
        self.base, self.data, self.size = base, data, len(data)

    def word(self, address):
        offset = address - self.base
        if offset < 0 or offset + 8 > self.size:
            raise Lost
        return U64.unpack_from(self.data, offset)[0]


class Unwinder:
    def __init__(self):
        self.bases, self.images = [], []

    def add(self, path, base, size):
        index = bisect.bisect_left(self.bases, base)
        if index < len(self.bases) and self.bases[index] == base:
            return
        self.bases.insert(index, base)
        self.images.insert(index, Image(path, base, size))

    def image(self, pc):
        index = bisect.bisect_right(self.bases, pc) - 1
        if index >= 0 and pc < self.images[index].end:
            return self.images[index]
        return None

    def walk(self, registers, pc, stack, limit=512):
        """Program counters from the leaf to the root: the sampled one, then return addresses."""
        pcs = [pc]
        registers = list(registers)
        try:
            image = self.image(pc)
            if image is None:
                raise Lost
            pc = self.frame(image, pc, pc - image.base, registers, stack, leaf=True)
            while pc and len(pcs) < limit:
                pcs.append(pc)
                image = self.image(pc)
                if image is None:
                    break
                # A return address may be the first byte after its function: look up the call itself.
                pc = self.frame(image, pc, pc - 1 - image.base, registers, stack, leaf=False)
        except (Lost, IndexError, struct.error):
            pass
        return pcs

    def frame(self, image, pc, rva, registers, stack, leaf):
        """Undoes one frame in `registers`; returns the caller's program counter."""
        function = image.function(rva)
        if function is None:
            return self.pop(registers, stack)
        begin, end, address = function
        offset = pc - image.base - begin
        prolog, frame_register, frame_offset, steps, chained = image.info(address)
        if leaf and offset >= prolog and self.epilog(image, pc - image.base, begin, end, registers, stack):
            return self.pop(registers, stack)
        chain = 0
        while True:
            established = registers[RSP]
            if frame_register and (chain or offset >= prolog or any(
                    op == SET_FPREG and offset >= at for at, op, _, _ in steps)):
                established = registers[frame_register] - frame_offset
            for at, op, value, operand in steps:
                if not chain and offset < at:
                    continue
                if op == PUSH_NONVOL:
                    registers[value] = stack.word(registers[RSP])
                    registers[RSP] += 8
                elif op in (ALLOC_LARGE, ALLOC_SMALL):
                    registers[RSP] += operand
                elif op == SET_FPREG:
                    registers[RSP] = registers[frame_register] - frame_offset
                elif op in (SAVE_NONVOL, SAVE_NONVOL_FAR):
                    registers[value] = stack.word(established + operand)
                elif op == PUSH_MACHFRAME:
                    at_frame = registers[RSP] + (8 if value else 0)
                    caller = stack.word(at_frame)
                    registers[RSP] = stack.word(at_frame + 24)
                    return caller
            if chained is None:
                return self.pop(registers, stack)
            chain += 1
            if chain > 32:
                raise Lost
            prolog, frame_register, frame_offset, steps, chained = image.info(chained[2])

    @staticmethod
    def pop(registers, stack):
        caller = stack.word(registers[RSP])
        registers[RSP] += 8
        return caller

    @staticmethod
    def epilog(image, rva, begin, end, registers, stack):
        """Simulates the rest of an epilog when `rva` is inside one; False when it is not."""
        code = image.read(rva, 40)
        at, moves = 0, []
        if code[:3] in (b"\x48\x83\xc4", b"\x48\x81\xc4"):  # add rsp, imm8 | imm32
            wide = code[1] == 0x81
            amount = struct.unpack_from("<i" if wide else "<b", code, 3)[0]
            moves.append(("add", amount))
            at = 7 if wide else 4
        elif len(code) > 3 and code[0] in (0x48, 0x49) and code[1] == 0x8D and code[2] & 0x38 == 0x20 \
                and code[2] >> 6 in (1, 2) and code[2] & 7 != 4:  # lea rsp, [reg + disp]
            wide = code[2] >> 6 == 2
            register = (code[2] & 7) + (8 if code[0] == 0x49 else 0)
            moves.append(("lea", register, struct.unpack_from("<i" if wide else "<b", code, 3)[0]))
            at = 7 if wide else 4
        while at < len(code):
            if 0x58 <= code[at] <= 0x5F:
                moves.append(("pop", code[at] - 0x58))
                at += 1
            elif code[at] == 0x41 and at + 1 < len(code) and 0x58 <= code[at + 1] <= 0x5F:
                moves.append(("pop", code[at + 1] - 0x58 + 8))
                at += 2
            else:
                break
        tail = code[at:at + 6]
        if not tail:
            return False
        if tail[0] in (0xC3, 0xC2) or tail[:2] == b"\xf3\xc3":
            pass
        elif tail[0] in (0xEB, 0xE9):  # a jump out of the function is a tail call
            wide = tail[0] == 0xE9
            if len(tail) < (5 if wide else 2):
                return False
            target = rva + at + (5 if wide else 2) + struct.unpack_from("<i" if wide else "<b", tail, 1)[0]
            if begin <= target < end:
                return False
            # A jump between fragments of one function (chained records) is not a tail call either.
            here, there = image.function(rva), image.function(target)
            if here and there and image.primary(here) == image.primary(there):
                return False
        elif tail[:2] == b"\xff\x25" or tail[:3] == b"\x48\xff\x25" or (
                tail[0] == 0x48 and tail[1:2] == b"\xff" and len(tail) > 2 and tail[2] & 0x38 == 0x20):
            pass  # jmp through memory or register: a tail call
        else:
            return False
        for move in moves:
            if move[0] == "add":
                registers[RSP] += move[1]
            elif move[0] == "lea":
                registers[RSP] = registers[move[1]] + move[2]
            else:
                registers[move[1]] = stack.word(registers[RSP])
                registers[RSP] += 8
        return True
