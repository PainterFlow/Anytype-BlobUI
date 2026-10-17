"""Run the real stdin -> Rebuild -> HTTP -> install flow with fake API keys.

Windows uses native DPAPI/ACLs. Linux stubs only Windows encryption and ACL
functions inside a temporary copy of the module, never in production sources.
Run with BLOBUI_TEST_POWERSHELL pointing to pwsh if it is not on PATH.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[2]
POWERSHELL = os.environ.get('BLOBUI_TEST_POWERSHELL') or shutil.which('pwsh') or shutil.which('powershell')
if not POWERSHELL:
    raise SystemExit('Set BLOBUI_TEST_POWERSHELL to a PowerShell executable.')

requests = []
class FakeAnytype(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        pass

    def do_GET(self):
        requests.append(self.path)
        key = self.headers.get('Authorization', '')
        if key not in ('Bearer fixture-key-one', 'Bearer fixture-key-two'):
            status, body = 401, {'message': 'Invalid key'}
        elif self.path.startswith('/v2/spaces?'):
            status, body = 200, {'data': [{'id': 'fixture-space', 'name': 'Fixture'}], 'has_more': False}
        else:
            # A valid, configured key can hit a calendar endpoint failure.
            status, body = 500, {'message': 'Calendar data unavailable'}
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.end_headers()
        self.wfile.write(json.dumps(body).encode())

scratch = ROOT / '.test-run'
scratch.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='connection-process-', dir=scratch) as directory:
    fixture = Path(directory)
    source = fixture / 'Source'
    source.mkdir()
    for folder in ('Dev', 'assets'):
        shutil.copytree(ROOT / folder, source / folder)
    (source / 'scripts').mkdir()
    for script in (ROOT / 'scripts').iterdir():
        if script.is_file():
            shutil.copy2(script, source / 'scripts' / script.name)
    if os.name != 'nt':
        module = source / 'scripts/BlobUI.Core.psm1'
        text = module.read_text()
        stubs = '''
# Fixture-only Windows platform stubs. These are never shipped with BlobUI.
function Get-Acl { param($LiteralPath, $ErrorAction) return 'fixture-acl' }
function Set-Acl { param($LiteralPath, $AclObject, $ErrorAction) }
function Set-BlobUiCredentialAcl { param($Path) }
function New-BlobUiProtectedCredential { param($ApiKey) return ('fixture-only:' + $ApiKey) }
'''
        module.write_text(text.replace('Export-ModuleMember -Function @(', stubs + '\nExport-ModuleMember -Function @('))
    packaged_executable = source / 'settings-app/dist/blobui-settings.exe'
    packaged_executable.parent.mkdir(parents=True)
    packaged_executable.write_bytes(b'fixture-executable-v1')
    install = fixture / 'Anytype Data'
    blob = install / 'blob'
    blob.mkdir(parents=True)
    data_path = blob / 'data.json'
    data_path.write_text(json.dumps({'schemaVersion': 1, 'installDirectory': str(install),
        'settings': {'fontFamily': 'Libron', 'codeFontFamily': 'Consolas', 'accentColor': '#DE50DE'},
        'calendarObjectColors': {}}))
    server = ThreadingHTTPServer(('127.0.0.1', 0), FakeAnytype)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    def configure(key='', enabled=True, port=None):
        return subprocess.run([POWERSHELL, '-NoProfile', '-NonInteractive', '-File',
            str(source / 'scripts/Rebuild.ps1'), '-InstallDirectory', str(install),
            '-ConfigureConnectionFromStdin'], input=json.dumps({'enabled': enabled,
                'apiKey': key, 'port': port if port is not None else server.server_port}),
            text=True, capture_output=True, cwd=source, timeout=60)
    try:
        first = configure('fixture-key-one')
        assert first.returncode == 0, 'Valid key was rejected when optional calendar refresh failed.'
        assert 'BlobUI_CALENDAR_CACHE=cached' in first.stdout
        assert json.loads(data_path.read_text())['apiConnection']['port'] == server.server_port
        credential = blob / 'credentials/anytype-api-key.dpapi'
        first_credential = credential.read_bytes()
        runtime_executable = blob / 'runtime/settings-app.exe'
        previous_runtime = runtime_executable.read_bytes()
        packaged_executable.write_bytes(b'fixture-executable-v2')
        # Source-folder key changes must not inspect or replace runtime binaries.
        if os.name != 'nt':
            runtime_executable.chmod(0)
        try:
            rotated = configure('fixture-key-two')
        finally:
            if os.name != 'nt':
                runtime_executable.chmod(0o600)
        assert runtime_executable.read_bytes() == previous_runtime, 'Changing a key redeployed the runtime executable.'
        assert rotated.returncode == 0, 'Replacing an existing valid key failed.'
        assert credential.read_bytes() != first_credential, 'Key rotation left the old credential unchanged.'
        assert 'fixture-key-two' not in data_path.read_text(), 'Key leaked into settings data.'
        saved_data, saved_key = data_path.read_bytes(), credential.read_bytes()
        rejected = configure('invalid-fixture-key')
        assert rejected.returncode != 0
        assert 'BLOBUI_CONNECTION_ERROR=unauthorized' in rejected.stdout
        assert data_path.read_bytes() == saved_data and credential.read_bytes() == saved_key
        # Valid authentication plus broken CSS must report compilation, not a bad key.
        broken_css = source / 'Dev/99-broken-fixture.css'
        broken_css.write_text('.fixture { color: {{undefinedFixtureValue}}; }')
        compile_failed = configure('fixture-key-one')
        assert compile_failed.returncode != 0
        assert 'BLOBUI_CONNECTION_ERROR=compile' in compile_failed.stdout
        assert data_path.read_bytes() == saved_data and credential.read_bytes() == saved_key
        broken_css.unlink()
        # An ACL failure must identify credential permissions and roll back.
        module = source / 'scripts/BlobUI.Core.psm1'
        original_module = module.read_text()
        permission_failure = "function Set-BlobUiCredentialAcl { param($Path) throw [UnauthorizedAccessException]::new('Fixture permission failure') }\n"
        module.write_text(original_module.replace('Export-ModuleMember -Function @(', permission_failure + 'Export-ModuleMember -Function @('))
        try:
            permission_failed = configure('fixture-key-one')
            assert permission_failed.returncode != 0
            assert 'BLOBUI_CONNECTION_ERROR=install-credential-permissions:' in permission_failed.stdout
            assert data_path.read_bytes() == saved_data and credential.read_bytes() == saved_key
        finally:
            module.write_text(original_module)
        count = len(requests)
        disabled = configure(enabled=False, port=31009)
        assert disabled.returncode == 0 and len(requests) == count, 'No API mode made an API request.'
        assert json.loads(data_path.read_text())['apiConnection']['enabled'] is False
    finally:
        server.shutdown()
print('Connection process checks passed: valid key with calendar failure, key rotation, custom port, locked runtime preservation, 401 rollback, compile/credential-permission diagnostics, and no API mode.')
if os.name != 'nt':
    print('Windows DPAPI/ACL operations used fixture-only stubs on this host.')
