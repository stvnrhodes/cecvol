sudo apt install cmake libudev-dev g++-arm-linux-gnueabihf rustup target add
armv7-unknown-linux-gnueabihf cargo build --target=armv7-unknown-linux-gnueabihf

TODO:

- switch to a /etc/cecvol/cecvol.conf for configuration
- oauth
- json directly
- https://developers.google.com/assistant/smarthome/develop/process-intent
- custom extension for wol
