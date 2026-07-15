## Overview

A plugin manager for MHFZ. Ships with [d3d8to9](https://github.com/crosire/d3d8to9), using a slightly modified version of [egui-d3d9](https://github.com/unknowntrojan/egui-d3d9). Initial loading with [cardamom-loader](https://github.com/Relial/cardamom-loader). Features plugin hot reloading, some rendering stats, and a messy egui FFI layer ([bunny-ui](https://github.com/Relial/bunny-ui)).
Plugins use [bunny-plugin](https://github.com/Relial/bunny-plugin)

## Usage

Extract in the MHFZ game folder. dinput8.dll, d3d8.dll, and a plugins folder called "cardamom" should be in the same folder as mhf.exe.

If you'd like to use a different dinput8.dll (such as the one created by XInput Plus), rename that dinput8.dll to dinput8_c.dll before installing Bunny Manager. The dinput8.dll in your game folder should be from Bunny Manager, and the other one you want to use should be named dinput8_c.dll.

You should see a command prompt window when first launching the game.

## Linux

Run with WINEDLLOVERRIDES="dinput8,d3d8=n,b"

If running from Steam, use WINEDLLOVERRIDES="dinput8,d3d8=n,b" %command%

## Known issues

Rivatuner Statistics Server can cause this manager's UI to not appear, despite no errors appearing in the console. Starting Rivatuner after the manager has fully loaded can help.

Some other programs that draw game overlays might cause the same issue. If you don't see the manager window, try closing programs that feature game overlays.
