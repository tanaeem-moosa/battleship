#!/usr/bin/env python3
import os
import shutil
import sys

# Prefer system cc/gcc if present on PATH, otherwise link directly via /usr/bin/ld on SteamOS
cc = shutil.which("cc") or shutil.which("gcc") or shutil.which("clang")
if cc:
    os.execv(cc, [cc] + sys.argv[1:])

args = [
    "/usr/bin/ld",
    "-pie",
    "--eh-frame-hdr",
    "-m",
    "elf_x86_64",
    "-dynamic-linker",
    "/lib64/ld-linux-x86-64.so.2",
    "/usr/lib/Scrt1.o",
    "/usr/lib/crti.o",
    "-L/usr/lib",
]
for a in sys.argv[1:]:
    if a.startswith("-Wl,"):
        for part in a[4:].split(","):
            if part:
                args.append(part)
    elif a == "-lgcc_s":
        args.append("/usr/lib/libgcc_s.so.1")
    elif (
        a in ("-m64", "-pie", "-nodefaultlibs", "-nostartfiles")
        or a.startswith("-B")
        or a.startswith("-fuse-ld=")
    ):
        pass
    else:
        args.append(a)
args.append("/usr/lib/crtn.o")
os.execv("/usr/bin/ld", args)
