#!/usr/bin/env python3
"""Legacy Phase2 suites; fresh Phase3 storage and strict evidence guards only."""
import sys
sys.dont_write_bytecode = True
from phase3_run import main

if __name__ == "__main__":
    main(legacy_phase2=True)
