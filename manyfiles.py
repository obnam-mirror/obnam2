#!/usr/bin/python3
#
# Create the desired number of empty files in a directory. A thousand files per
# subdirectory.

import os
import sys


def subdir(dirname, dirno):
    pathname = os.path.join(dirname, str(dirno))
    os.mkdir(pathname)
    return pathname


def create(filename):
    open(filename, "w").close()


DIRFILES = 1000

dirname = sys.argv[1]
n = int(sys.argv[2])

dirno = 0
subdirpath = subdir(dirname, dirno)
fileno = 0
thisdir = 0

while fileno < n:
    filename = os.path.join(subdirpath, str(thisdir))
    create(filename)

    fileno += 1
    thisdir += 1
    if thisdir >= DIRFILES:
        dirno += 1
        subdirpath = subdir(dirname, dirno)
        thisdir = 0
