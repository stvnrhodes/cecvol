# CECVol

Control TV volume control over CEC via a raspberry pi.

![Screenshot of web ui](/docs/screenshot-2023-01-28.png "Screenshot")

There's a web ui, a fitbit app, and maybe one day a way to do voice commands.

This only works on a raspberry pi because it directly manipulates .

Use the following command to build a debian package.

```shell
CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/usr/bin/arm-linux-gnueabihf-gcc cargo deb --target=armv7-unknown-linux-gnueabihf
```

TODO:

- switch to a /etc/cecvol/cecvol.conf for configuration
- oauth
- json directly
- https://developers.google.com/assistant/smarthome/develop/process-intent
- custom extension for wol
