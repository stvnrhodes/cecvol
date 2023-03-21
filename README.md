# CECVol

Control TV volume control over CEC via a raspberry pi.

![Screenshot of web ui](/docs/screenshot-2023-01-28.png "Screenshot")

There's a web ui, a fitbit watch app, and maybe one day a way to do voice commands. I've only tried it out with my personal setup of a raspberry pi hooked up to a LG OLED-C9. It works fine except for a quirk where the tv sometimes inappropriately switches to the pi input.

The LG IP code works by opening a TCP connection and sending symmetrically-encrypted packets with simple text commands. See (WesSouza/lgtv-ip-control)[https://github.com/WesSouza/lgtv-ip-control].

The CEC code works by calling ioctls on `/dev/vchiq` so it only works on a raspberry pi.

Use the following command to build a debian package that works on Raspbian.

```shell
CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_LINKER=/usr/bin/arm-linux-gnueabihf-gcc cargo deb --target=armv7-unknown-linux-gnueabihf
```

TODO before making public:

- Stop hardcoding the HMAC and client secret (and rotate the secrets)
- Stop hardcoding the MAC address for WOL
- Stop hardcoding the bearer token for the fitbit app
- Document installation instructions
- Remove these notes

TODO ideas:

- switch to a /etc/cecvol/cecvol.conf for configuration
- oauth
- json directly
- https://developers.google.com/assistant/smarthome/develop/process-intent
- custom extension for wol
