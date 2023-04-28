no cutscene kiseki
==================

This is a mod for the PC version of Trails of Cold Steel III to automatically skip cutscenes.

Every cutscene that can be skipped via the "skip cutscene? yes/no" menu in the original game will be skipped automatically with no user input.
Cutscenes that cannot be skipped this way in the original game are not changed: They will play out as normal with no input, but can still be fast-forwarded.

[Video demo (on youtube)](https://www.youtube.com/watch?v=2BkCd9lfyQQ)

The only currently supported version is the version 1.05 of the NISA PC port.
French and Japanese languages have not been extensively tested.

The mod is intended for speedrunning but not currently allowed on the leaderboards.

Usage
-----

![A screenshot of the launcher for the modded game](screenshot.png)

- Download and run the mod .exe
- If the game folder hasn't been detected automatically, select it yourself.
  The game folder you need to choose contains (among others) the file `Sen3Launcher.exe`, as well as folders `bin` and `data`.
- Click either "Launch Game" or "Launch Game & Exit"

If your game is installed to a nonstandard location and you want to avoid manually choosing the game folder every time, you can copy the mod .exe to that game folder and it should be recognized automatically.

Known issues
------------

- When auto fast-forward is enabled (L1), the skipped cutscenes will briefly flash instead of the screen staying black.
  The flashes are likely slow enough to not trigger PSE since there tend to be ~1-2 s between flashes, but at the very least it's a bit annoying.
  In runs, the only time this is likely to happen is in the chapter 4 end sequence where that one gate scene isn't implemented as a (skippable) cutscene.

Credits
-------

Thanks to:

- Fontaino for testing
- ⠋Sewer56⣠ and uyjulian for help with patching the running process
- Twn and Kyuuhachi for help with reverse engineering
- AdmiralCurtiss for [SenPatcher](https://github.com/AdmiralCurtiss/SenPatcher) which was useful for cross-referencing.

Releases
--------

## [version 1](https://git.sr.ht/~quf/no-cutscene-kiseki/refs/download/no-cutscene-kiseki-v1/no-cutscene-kiseki-v1-launcher.exe)

- initial public release
