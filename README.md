# riteRoad

A screenreader accessible control panel for the RØDE Wireless Micro kit.

## Disclaimer

This is an independent project and is not supported or endorsed by Rode. It does not now, nor will it ever handle updating firmware of the transmitters and receiver. Use at your own risk.

## Downloading

Download the latest release for your PC:

- [Windows x64](https://github.com/ironcross32/riteRoad/releases/latest/download/riteroad-x64.zip)
  (most PCs)
- [Windows ARM64](https://github.com/ironcross32/riteRoad/releases/latest/download/riteroad-arm64.zip)

Unzip it anywhere and run `riteroad.exe`. Every release is also listed on the
[releases page](https://github.com/ironcross32/riteRoad/releases).

## Installation

Installation is not necessary unless you want riteRoad to start with Windows. To install it, Navigate to File > Settings and press Install to this PC. That copies riteRoad
to `%LOCALAPPDATA%\Programs\riteRoad`, with a Start menu shortcut.

## Usage

The controls are fairly self-explanatory. The status area shows a breakdown of what pieces of the kit are connected and the battery levels of any transmitters it detects. Next are the receiver-specific controls, first among which is the output mode. Merged is summed mono from both transmitters, split forms a stereo pair. These are terms the Rode Central app uses. so they're retained here.

The output gain provides three presets, and these are the same presets the official app supports, thus, riteRoad cannot offer any more granularity. The LED brightness controls the light on the dongle, and supports bright and dim, but not off.

The noise reduction controls are transmitter-specific because each transmitter has its own independent control. riteRoad offers the ability to set the level of noise reduction of both mics at the same time, a feature the official app does not support. First, set the transmitter combobox to transmitter 1, transmitter 2, or both, then tab to the level combobox. If you see that it reports mixed, it means that transmitters 1 and 2 are set to different levels of noise reduction. Making a change will update that.

## The speak status hotkey

Ctrl+Shift+F9 speaks the receiver's mode and each transmitter's battery from anywhere. This may be altered in Settings.

## Settings

Open settings by navigating to File > Settings or pressing CTRL+comma. In this dialog, you can modify hotkeys, switch on and off various automatic announcements, and set your preferences for startup, install, and automatic updates as well as perform a manual update check.

## Updates

By default, riteRoad checks for updates at startup. This can be adjusted in Settings.