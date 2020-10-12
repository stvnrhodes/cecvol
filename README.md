sudo apt install cmake libudev-dev  g++-arm-linux-gnueabihf
rustup target add armv7-unknown-linux-gnueabihf
cargo build --target=armv7-unknown-linux-gnueabihf
