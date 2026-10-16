"""heaptype.py PID -> which heap the process's default heap is (64-bit processes only).

Reads PEB.ProcessHeap and the signature at offset 0x10 of the heap header:
0xDDEEDDEE = segment heap, 0xFFEEFFEE = NT (LFH) heap.
"""
import ctypes
import sys
from ctypes import wintypes

PROCESS_QUERY_INFORMATION, PROCESS_VM_READ = 0x0400, 0x0010


class PBI(ctypes.Structure):
    _fields_ = [("Reserved1", ctypes.c_void_p), ("PebBaseAddress", ctypes.c_void_p), ("Reserved2", ctypes.c_void_p * 2),
                ("UniqueProcessId", ctypes.c_void_p), ("Reserved3", ctypes.c_void_p)]


def heap_type(pid):
    k32, nt = ctypes.WinDLL("kernel32", use_last_error=True), ctypes.WinDLL("ntdll")
    k32.OpenProcess.restype = wintypes.HANDLE
    h = k32.OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, False, pid)
    if not h:
        raise OSError(ctypes.get_last_error())
    try:
        pbi = PBI()
        st = nt.NtQueryInformationProcess(h, 0, ctypes.byref(pbi), ctypes.sizeof(pbi), None)
        if st != 0:
            raise OSError(f"NtQueryInformationProcess {st:#x}")

        def read(addr, n):
            buf = ctypes.create_string_buffer(n)
            got = ctypes.c_size_t()
            if not k32.ReadProcessMemory(h, ctypes.c_void_p(addr), buf, n, ctypes.byref(got)):
                raise OSError(ctypes.get_last_error())
            return buf.raw

        heap = int.from_bytes(read(pbi.PebBaseAddress + 0x30, 8), "little")
        sig = int.from_bytes(read(heap + 0x10, 4), "little")
        return {0xDDEEDDEE: "segment", 0xFFEEFFEE: "nt"}.get(sig, f"unknown {sig:#x}")
    finally:
        k32.CloseHandle(h)


if __name__ == "__main__":
    print(heap_type(int(sys.argv[1])))
