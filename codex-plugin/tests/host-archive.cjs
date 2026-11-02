// Read bundled code only. Never load the app, its state, or its transports.
const fs = require('node:fs');
module.exports = function readHost(archive, pattern) {
  const fd = fs.openSync(archive, 'r');
  const result = new Map();
  try {
    const header = Buffer.alloc(16);
    fs.readSync(fd, header, 0, 16, 0);
    const data = Buffer.alloc(header.readUInt32LE(12));
    fs.readSync(fd, data, 0, data.length, 16);
    const base = 8 + header.readUInt32LE(4);
    function visit(files, prefix = '') {
      for (const [name, entry] of Object.entries(files)) {
        const path = `${prefix}/${name}`;
        if (entry.files) visit(entry.files, path);
        else if (!entry.unpacked && /\/webview\/assets\//.test(path) && pattern.test(name)) {
          const bytes = Buffer.alloc(entry.size);
          fs.readSync(fd, bytes, 0, bytes.length, base + Number(entry.offset));
          result.set(name, bytes.toString('utf8'));
        }
      }
    }
    visit(JSON.parse(data).files);
    return result;
  } finally { fs.closeSync(fd); }
};
