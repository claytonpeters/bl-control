## Keyboard Backlight Dimmer

A tiny Rust program to dim the keyboard backlight on keyboards using the ITE
8291 controller after a period of inactivity. It does this by monitoring for
input events on the relevant `/dev/input/event*` device and altering the
backlight as necessary if no key presses have been made in a specified timeout.


### Building

This requires `libusb-1.0-0-dev` (or however it's named for your distribution) 
to be installed. From there, a simple `cargo build` should be enough to build
for debug versions or `cargo build -r` for the release version.

The code makes use of the `tokio`, `futures`, `clap`, `clap-num` and `libusb`
Rust crates.


### Running

Once built, the binary can be found in the `targets/debug/` and or
`targets/release` directory, and is called `bl-control`. It can be run as
follows:

```
./bl-control -v 0x048d -p 0x6004 -t 60
```

The parameters are as follows:
* `-v` / `--vendor-id`: The vendor ID of the USB device
* `-p` / `--product-id`: The product ID of the USB edvice
* `-t` / `--timeout`: The number of seconds to leave the backlight on after the 
last keypress before dimming the backlight
* `-l` / `--lock`: Dim the backlight immediately when Meta+L is pressed (i.e.
when the lockscreen is triggered)

The vendor ID will almost certainly alays be `0x048d` and this is the default if
it is not given. The product ID can vary depending on the chip in use. This
program was tested on a PC Specialist Recoil Series laptop (Tongfang GM5ZN8W).
In that case the product ID was `0x6004`, but the output from `lsusb` will be
more useful in determining the IDs required:

```
$ lsusb
...
Bus 003 Device 004: ID 048d:6004 Integrated Technology Express, Inc. ITE Device(8291)
...
```

At present, the program determines the input device by looking for the first
device in `/sys/class/input` whose name contains `keyboard`. This will
inevitably not work if you have an external keyboard connected too.


## Installing as a systemd service

Copy the binary to a sensible location, e.g. `/usr/local/bin` and then create
a systemd unit file, for example `/etc/systemd/system/bl-control.service` with
the following contents:

```
[Unit]
Description=Keyboard backlight control

[Service]
Type=simple
ExecStart=/usr/local/bin/bl-control -v 0x048d -p 0x6004 -t 60 -l

[Install]
WantedBy=multi-user.target
```

Adjust the command line of `ExecStart` with the correct path, IDs and timeout
as necessary. Then just enable and start the service:

```
systemctl enable bl-control
systemctl start bl-control
```
