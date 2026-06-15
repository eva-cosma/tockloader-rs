#!/usr/bin/env bash

set -euo pipefail

TOCK_HASH="236be95a8a7747b91ca855ffe245ae475587258d"

echo "-=-= Start test pipeline =-=-"

echo " [0/5] Fully erase the board "

probe-rs erase --chip nrf52840_xxAA 2>&1

echo " [1/5] Download tock "
pushd .
TEMP_DIR=$(mktemp -d)
echo "Created temporary directory: $TEMP_DIR"

cd "$TEMP_DIR"
git clone --no-checkout --filter=blob:none https://github.com/tock/tock.git 2>&1
cd tock
git checkout $TOCK_HASH 2>&1

echo "Clone tock into $TEMP_DIR/tock and checked out commit $TOCK_HASH"

echo " [2/5] Build and flash tock "

cd boards/nordic/nrf52840dk/
make flash-openocd 2>&1

echo "Flashed tock to nrf52840dk"
popd

echo " [3/5] Prepare board with tockloader "

cd test_data
tockloader install ./c_hello.tab 2>&1
cd ..

echo " [4/5] Build tockloader-rs "

apt install -y libudev-dev
cargo build --release -p tockloader 2>&1

echo " Built tockloader-rs"

echo " [5/5] Test tockloader-rs "
LIST_OUTPUT=$(cargo run --release -p tockloader -- list --board nrf52840dk)
echo "${LIST_OUTPUT}" > list_output.txt

if [[ "$LIST_OUTPUT" != *"c_hello"* ]]; then
    echo "Error: 'list' command did not output expected application 'c_hello'"
    echo "Output was:"
    echo "$LIST_OUTPUT"
    exit 1
else
    echo "Success: 'list' command output contains expected application 'c_hello'"
fi

echo "-=-= End test pipeline =-=-"