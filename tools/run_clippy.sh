#!/usr/bin/env bash

# Check to see if we can execute `cargo clippy`.
# We don't want to force an installation onto the user, so for we
# will only notify them of the issue.
if ! rustup component list | grep 'clippy.*(installed)' -q; then
    echo "Could not check formatting with clippy, 'clippy' must be installed!"
    exit 1	
fi

# TODO: What arguments do we want to pass to clippy?
CLIPPY_ARGS="-D warnings"

# Clippy entire workspace, havivng warnings be treated normally
echo "Running clippy on entire workspace, treating warnings as warnings..."
cargo clippy 

# However, for tockloader and tockloader-lib, we will treat them as errors.
echo "Running clippy on tockloader and tockloader-lib, treating warnings as errors..."
cargo clippy -p tockloader -- $CLIPPY_ARGS
cargo clippy -p tockloader-lib -- $CLIPPY_ARGS
