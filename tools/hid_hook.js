// Frida agent: log HID traffic between RODE Central and the RODE dongle (VID 0x19F7).
// Hooks the HidD_* feature/report calls plus ReadFile/WriteFile/DeviceIoControl
// on handles that belong to a RODE HID device.

const RODE_VID = 0x19f7;
const FILE_TYPE_CHAR = 2;
const FILE_TYPE_PIPE = 3;

function exp(mod, name) {
  try {
    return Process.getModuleByName(mod).getExportByName(name);
  } catch (e) {
    return null;
  }
}

function hex(ptr, len) {
  if (ptr.isNull() || len <= 0) return '';
  const bytes = new Uint8Array(ptr.readByteArray(len));
  let s = '';
  for (let i = 0; i < bytes.length; i++) {
    s += (bytes[i] < 16 ? '0' : '') + bytes[i].toString(16);
    if (i + 1 < bytes.length) s += ' ';
  }
  return s;
}

function log(dir, api, handle, ptr, len, extra) {
  send({ dir, api, handle: handle.toString(), len, hex: hex(ptr, len), extra: extra || '' });
}

// ---- Handle classification -------------------------------------------------
const handleInfo = new Map(); // handle string -> {rode: bool, pid}
let HidD_GetAttributes = null;
let GetFileType = null;

function classify(handle) {
  const key = handle.toString();
  if (handleInfo.has(key)) return handleInfo.get(key);
  if (!HidD_GetAttributes) return { rode: false }; // hid.dll not loaded yet; don't cache
  let info = { rode: false };
  handleInfo.set(key, info); // placeholder guards against re-entry via DeviceIoControl
  try {
    const ft = GetFileType(handle);
    if (ft !== FILE_TYPE_CHAR && ft !== FILE_TYPE_PIPE && HidD_GetAttributes) {
      const attrs = Memory.alloc(12);
      attrs.writeU32(12);
      if (HidD_GetAttributes(handle, attrs)) {
        const vid = attrs.add(4).readU16();
        const pid = attrs.add(6).readU16();
        info = { rode: vid === RODE_VID, vid, pid };
        if (info.rode) {
          send({ dir: 'info', api: 'handle', handle: key, len: 0, hex: '',
                 extra: `RODE HID handle vid=${vid.toString(16)} pid=${pid.toString(16)}` });
        }
      }
    }
  } catch (e) { /* ignore */ }
  handleInfo.set(key, info);
  return info;
}

function isRode(handle) {
  return classify(handle).rode;
}

// ---- hid.dll hooks ---------------------------------------------------------
let hidHooked = false;

function hookHid() {
  if (hidHooked) return;
  const getAttr = exp('hid.dll', 'HidD_GetAttributes');
  if (!getAttr) return;
  hidHooked = true;
  HidD_GetAttributes = new NativeFunction(getAttr, 'int', ['pointer', 'pointer']);

  // BOOLEAN HidD_X(HANDLE, PVOID buffer, ULONG length)
  const reportApis = [
    ['HidD_SetFeature', 'out'],
    ['HidD_GetFeature', 'in'],
    ['HidD_SetOutputReport', 'out'],
    ['HidD_GetInputReport', 'in'],
  ];
  for (const [name, dir] of reportApis) {
    const addr = exp('hid.dll', name);
    if (!addr) continue;
    Interceptor.attach(addr, {
      onEnter(args) {
        this.h = args[0];
        this.buf = args[1];
        this.len = args[2].toUInt32();
        this.rode = isRode(this.h);
        if (this.rode && dir === 'out') log('TX', name, this.h, this.buf, this.len);
      },
      onLeave(ret) {
        if (!this.rode) return;
        if (dir === 'in') {
          if (ret.toInt32()) log('RX', name, this.h, this.buf, this.len);
          else log('RX', name, this.h, this.buf, 0, 'FAILED');
        } else if (!ret.toInt32()) {
          log('info', name, this.h, this.buf, 0, 'previous TX FAILED');
        }
      },
    });
  }
  send({ dir: 'info', api: 'hook', handle: '0', len: 0, hex: '', extra: 'hid.dll hooked' });
}

// ---- kernelbase hooks ------------------------------------------------------
GetFileType = new NativeFunction(exp('kernelbase.dll', 'GetFileType'), 'uint32', ['pointer']);

// Pending overlapped reads: overlapped ptr string -> {buf, handle}
const pendingReads = new Map();

function dumpOverlapped(handle, ov, bytes) {
  const key = ov.toString();
  const p = pendingReads.get(key);
  if (!p) return;
  pendingReads.delete(key);
  log('RX', 'ReadFile', handle, p.buf, bytes);
}

Interceptor.attach(exp('kernelbase.dll', 'WriteFile'), {
  onEnter(args) {
    if (isRode(args[0])) log('TX', 'WriteFile', args[0], args[1], args[2].toUInt32());
  },
});

Interceptor.attach(exp('kernelbase.dll', 'ReadFile'), {
  onEnter(args) {
    this.h = args[0];
    this.rode = isRode(this.h);
    if (!this.rode) return;
    this.buf = args[1];
    this.pRead = args[3];
    this.ov = args[4];
  },
  onLeave(ret) {
    if (!this.rode) return;
    if (ret.toInt32()) {
      let n = 0;
      if (!this.pRead.isNull()) n = this.pRead.readU32();
      else if (!this.ov.isNull()) n = this.ov.add(Process.pointerSize).readULong().valueOf();
      log('RX', 'ReadFile', this.h, this.buf, n);
    } else if (!this.ov.isNull()) {
      pendingReads.set(this.ov.toString(), { buf: this.buf, handle: this.h });
    }
  },
});

for (const name of ['GetOverlappedResult', 'GetOverlappedResultEx']) {
  const addr = exp('kernelbase.dll', name);
  if (!addr) continue;
  Interceptor.attach(addr, {
    onEnter(args) {
      this.h = args[0];
      this.ov = args[1];
      this.pBytes = args[2];
      this.watch = pendingReads.has(this.ov.toString());
    },
    onLeave(ret) {
      if (this.watch && ret.toInt32() && !this.pBytes.isNull()) {
        dumpOverlapped(this.h, this.ov, this.pBytes.readU32());
      }
    },
  });
}

Interceptor.attach(exp('kernelbase.dll', 'DeviceIoControl'), {
  onEnter(args) {
    this.h = args[0];
    this.rode = isRode(this.h);
    if (!this.rode) return;
    this.code = args[1].toUInt32();
    this.outBuf = args[4];
    this.pRet = args[6];
    log('TX', 'DeviceIoControl', this.h, args[2], args[3].toUInt32(),
        'ioctl=0x' + this.code.toString(16));
  },
  onLeave(ret) {
    if (!this.rode || !ret.toInt32() || this.pRet.isNull() || this.outBuf.isNull()) return;
    log('RX', 'DeviceIoControl', this.h, this.outBuf, this.pRet.readU32(),
        'ioctl=0x' + this.code.toString(16));
  },
});

Interceptor.attach(exp('kernelbase.dll', 'CloseHandle'), {
  onEnter(args) {
    handleInfo.delete(args[0].toString());
  },
});

// hid.dll may not be loaded yet (e.g. when spawning); watch for it.
hookHid();
if (!hidHooked) {
  try {
    Process.attachModuleObserver({
      onAdded(m) {
        if (m.name.toLowerCase() === 'hid.dll') hookHid();
      },
    });
  } catch (e) {
    send({ dir: 'info', api: 'hook', handle: '0', len: 0, hex: '', extra: 'module observer failed: ' + e });
  }
  // Fallback: poll until hid.dll shows up.
  const timer = setInterval(() => {
    hookHid();
    if (hidHooked) clearInterval(timer);
  }, 50);
}
send({ dir: 'info', api: 'hook', handle: '0', len: 0, hex: '', extra: 'agent ready' });
