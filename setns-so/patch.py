#!/usr/bin/env python3

# https://lief.quarkslab.com/doc/latest/tutorials/08_elf_bin2lib.html#warning-for-glibc-2-29-users

import lief
import sys
import os

path = sys.argv[1]
os.rename(path, path + ".old")

bin_ = lief.parse(path + ".old")
bin_[lief.ELF.DynamicEntry.TAG.FLAGS_1].remove(lief.ELF.DynamicEntryFlags.FLAG.PIE)
bin_.write(path)

os.system("chmod +x " + path)
