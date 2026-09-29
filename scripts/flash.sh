#!/bin/sh
set -eu

export PATH="$HOME/.cargo/bin:$PATH"
# Locate the Python helper without shell command substitution or a cwd assumption.
exec python3 - "$0" "$@" <<'PY'
from pathlib import Path
import runpy
import sys

helper = Path(sys.argv.pop(1)).resolve().with_suffix('.py')
sys.argv[0] = str(helper)
runpy.run_path(str(helper), run_name='__main__')
PY
