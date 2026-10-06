`frame-v1.hex` is one snapshot frame, hex-encoded, that both sides test against: the firmware
decodes it (`firmware/test/test_protocol`) and the Mac app encodes the same content to these exact
bytes (`mac/Tests/DeskCoreTests`).

Content: flags = swap buttons; 12:34:56; night 23–08; brightness 80 % day, 12 % night;
1. id 0x01020304, Claude, NEEDS YOU, 42 s, "servarium-05", "perm"
2. id 0xA0B0C0D0, Codex, WORKING, 3 subagents, 3600 s, "Миграция базы", ""
