// Standalone, read-only probe. Run with: node scripts/Test-Api-V1.cjs
// Optional environment variables: ANYTYPE_API_KEY, ANYTYPE_API_PORT.
// Otherwise reads the existing Windows DPAPI credential without changing it.
const http = require('node:http');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

async function main() {
  let key = process.env.ANYTYPE_API_KEY;
  if (!key) {
    const credential = path.join(process.env.APPDATA, 'anytype', 'blob', 'credentials', 'anytype-api-key.dpapi');
    try {
      key = execFileSync('pwsh.exe', ['-NoProfile', '-NonInteractive', '-Command',
      "$ErrorActionPreference='Stop'; $s=(Get-Content -Raw -LiteralPath $env:BLOBUI_PROBE_CREDENTIAL).Trim() | ConvertTo-SecureString; $p=[Runtime.InteropServices.Marshal]::SecureStringToBSTR($s); try { [Console]::Write([Runtime.InteropServices.Marshal]::PtrToStringBSTR($p)) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($p) }"
    ], { encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
      env: { ...process.env, BLOBUI_PROBE_CREDENTIAL: credential } }).trim();
    } catch (error) {
      const code = /^[A-Z_]+$/.test(error.code || '') ? error.code : `exit ${error.status}`;
      throw new Error(`Credential helper failed (${code}).`);
    }
  }
  if (!key || /[\r\n]/.test(key)) throw new Error('Invalid API key');
  const port = Number(process.env.ANYTYPE_API_PORT || 31009);
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('Invalid port');
  for (const version of ['v2', 'v1']) {
    await new Promise((resolve, reject) => {
      const route = `/${version}/spaces?limit=1`;
      const headers = { Authorization: `Bearer ${key}`, Accept: 'application/json', Connection: 'close' };
      if (version === 'v1') headers['Anytype-Version'] = '2025-11-08';
      const req = http.get({ hostname: '127.0.0.1', port, path: route, headers }, res => {
        // Discard response content so no space names, IDs, or secrets are logged.
        res.resume();
        res.on('end', () => { console.log(`GET ${route} -> HTTP ${res.statusCode}`); resolve(); });
        res.on('error', reject);
      });
      req.setTimeout(10000, () => req.destroy(new Error('Request timed out')));
      req.on('error', error => reject(new Error(
        `GET ${route} at 127.0.0.1:${port} failed (${/^[A-Z_]+$/.test(error.code || '') ? error.code : 'connection error'}).`
      )));
    });
  }
  key = undefined;
}
main().catch(error => {
  console.error(error.message);
  process.exitCode = 1;
});
