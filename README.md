# CECVol

Control TV volume, power, and inputs remotely.

![Screenshot of web ui](/docs/screenshot-2023-01-28.png "Screenshot")

There's a web ui and and a wearos watch app, and maybe one day a way to do voice commands. I've only tried it out with my personal setup, originally a raspberry pi hooked up to a LG OLED-C9 and controlling it over CEC. It worked fine except for a quirk where the tv sometimes inappropriately switches to the pi input. I later switched to talking to the TV over the LAN, but I kept the CEC name.

The LG IP code works by opening a TCP connection and sending symmetrically-encrypted packets with simple text commands. See [WesSouza/lgtv-ip-control](https://github.com/WesSouza/lgtv-ip-control).

The CEC code works by calling ioctls on `/dev/vchiq` so it only works on a raspberry pi.

There's no authentication within the server code. It expects to be run behind a proxy that terminates TLS and provides HTTP basic authentication.

The API is designed around being used as a Google Smart Home Action, but that feature isn't currently functional.
