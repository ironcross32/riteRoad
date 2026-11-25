# RØDE Wireless Micro receiver — HID protocol notes

Reverse-engineered by capturing RØDE Central's HID traffic (`tools/capture.py`).
Status: work in progress. Values are hex unless noted.

## Device

- USB VID `19F7`, PID `006A`, product "Wireless MICRO".
- Interface 0 (`MI_00`) is a vendor-defined HID collection: the control channel.
- Interface 1 is USB audio.
- Output reports are written with `WriteFile`, 2017 bytes (report ID + payload,
  zero padded). Replies are read as 64-byte input reports.

## Framing

Request/response. Two report-ID pairs are in use:

| Request ID | Reply ID | Seen for |
|---|---|---|
| `09` | `0a` | receiver settings, clock, most commands |
| `01` | `02` | transmitter status (`55`) |

Request: `<id> <cmd> <args...>`. Reply: `<id+1> <cmd> <status> <data...>`.

Status byte: `41` ('A') = acknowledged, `4e` ('N') = not acknowledged
(seen for the clock commands `01`, `0d`, `0e`).

For `09 3e`, the first argument looks like a read/write flag: `01` = read
(reply carries the value), `00` = write (followed by the value).

## Commands

### `09 14` — output mode (split / merged)

- Read: `09 14 01` → `0a 14 41 <value>` (polled every ~500 ms)
- Write: `09 14 00 <value>` → `0a 14 41`

| UI | value |
|---|---|
| Split (stereo, TX1 left / TX2 right) | `00` |
| Merged (mono sum) | `01` |

### `09 3d` — LED brightness (Device tab)

- Read: `09 3d 01` → `0a 3d 41 <value>` (polled every ~500 ms)
- Write: `09 3d 00 00 <value>` → `0a 3d 41` (note the extra `00`, meaning unknown)

| UI | value |
|---|---|
| Dim | `00` |
| Bright | `01` |

Read `00` (Dim) before it was first touched, so Dim is probably the default.

### `09 3e` — receiver output gain (preset)

- Read: `09 3e 01` → `0a 3e 41 <value>`
- Write: `09 3e 00 <value>` → `0a 3e 41`

| UI | value |
|---|---|
| 0% | `0a` |
| 50% | `0e` |
| 100% | `10` |

Only these three positions exist in RØDE Central.

### `09 51` — transmitter noise reduction

- Write: `09 51 <tx> <value>` → `0a 51 41`, where `<tx>` is `01` / `02`.

| UI | value |
|---|---|
| Off | `00` |
| Low | `03` |
| High | `06` |

(No Medium option.) Current value is also reported in the `55` status reply.

### `01 55` — transmitter status (polled every ~500 ms)

`01 55 <tx>` → `02 55 41 31 02 00 00 00 23 <b9> <battery> 00 <nr>`

When the transmitter is not connected (e.g. docked in the charging case), the
reply is `02 55 41 00 00 00 00 ff`.

| Byte (0-based) | Meaning |
|---|---|
| 3 | `31` = connected, `00` = not connected |
| 9 | unknown; `28`/`2a`, changed `2a` → `1a` after TX2 reconnected (signal quality?) |
| 10 | battery %, decimal (`60` = 96); matches the percentage RØDE Central draws on screen (its screen reader text says 100% regardless) |
| 12 | noise reduction value (see `51`) |

### Startup and on transmitter reconnect

- `09 01 00 <u32 LE unix time>` → `0a 01 4e` (set clock?)
- `09 0d 06 <u32 LE unix time>` → `0a 0d 4e`
- `09 0e 07 <u32 LE unix time>` → `0a 0e 4e`

`0d 06` is sent when TX1 reconnects and `0e 07` when TX2 reconnects, so they
are "sync clock to TX1/TX2". The device NAKs them; probably harmless to skip.

## Known issues

- Windows delivers every input report to every open handle. A write ACK
  (`<id> <cmd> 41`, no value) from another program is indistinguishable from
  a read reply whose value is `00`. `Receiver` drains queued reports before
  each request and retries reads that decode to an unknown value, but a value
  that is valid when read as `00` (e.g. Split, Dim) can still be misread if
  RØDE Central writes during the few ms we wait for a reply. Avoid running
  both programs while changing settings.

- RØDE Central exited (no crash report, Frida reason "process-terminated")
  ~0.1 s after TX2 reconnected from the charging case. Unclear whether it is
  RØDE's bug or caused by the capture hook.

## To do

- Meaning of the extra `00` in the LED write, and of status byte 9
- Whether the app exit after TX2 reconnect was RØDE's bug (did not recur on
  the TX1 test)
