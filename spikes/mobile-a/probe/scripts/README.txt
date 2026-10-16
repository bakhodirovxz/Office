Throwaway harness for Spike 3 A2/A3 (docs/phase0/mobile-a-kill.md). Written for this machine (Git Bash + Windows python + adb).
Gboard key coordinates are for 1080x2400 portrait, Gboard 15.1, EN/RU/UZ layouts. glide.py injects touches through the
emulator console (needs ~/.emulator_console_auth_token); adb input motionevent is too slow for Gboard to accept a glide.
Never use `adb shell input keyevent` while testing: it flips Gboard into its physical-keyboard compact toolbar.
