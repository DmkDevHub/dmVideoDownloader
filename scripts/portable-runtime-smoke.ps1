param([Parameter(Mandatory)][string]$Archive)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$archivePath = (Resolve-Path -LiteralPath $Archive).Path
$testRoot = Join-Path $root '.test-output'
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$testRoot = (Resolve-Path -LiteralPath $testRoot).Path
if ((Split-Path -Parent $testRoot) -ne $root) { throw 'Test output must stay in the project.' }
$work = Join-Path $testRoot ("portable-runtime-" + [Guid]::NewGuid().ToString('N').Substring(0, 10))
New-Item -ItemType Directory -Path $work | Out-Null
$work = (Resolve-Path -LiteralPath $work).Path

# Launch the unmodified release executable on a separate desktop without switching to it.
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;

public sealed class PortableRuntimeProcess : IDisposable
{
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    private struct StartupInfo
    {
        public int cb;
        public string reserved, desktop, title;
        public int x, y, width, height, xCount, yCount, fill, flags;
        public short showWindow, reservedSize;
        public IntPtr reservedBytes, input, output, error;
    }
    [StructLayout(LayoutKind.Sequential)]
    private struct ProcessInfo
    {
        public IntPtr process, thread;
        public int processId, threadId;
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern IntPtr CreateDesktop(string name, IntPtr device, IntPtr mode,
        int flags, uint access, IntPtr attributes);
    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CreateProcess(string app, IntPtr commandLine, IntPtr processAttributes,
        IntPtr threadAttributes, bool inheritHandles, uint flags, IntPtr environment, string currentDirectory,
        ref StartupInfo startup, out ProcessInfo process);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(IntPtr process, uint exitCode);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);

    private IntPtr desktop;
    private IntPtr process;
    public bool Running { get { return WaitForSingleObject(process, 0) == 258; } }

    public PortableRuntimeProcess(string app, string cwd)
    {
        string name = "dmVideoPortable-" + Guid.NewGuid().ToString("N");
        desktop = CreateDesktop(name, IntPtr.Zero, IntPtr.Zero, 0, 0x10000000, IntPtr.Zero);
        if (desktop == IntPtr.Zero) throw new Win32Exception();
        var startup = new StartupInfo { cb = Marshal.SizeOf(typeof(StartupInfo)), desktop = name };
        ProcessInfo info;
        if (!CreateProcess(app, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, false, 0,
            IntPtr.Zero, cwd, ref startup, out info))
        {
            var error = new Win32Exception();
            CloseDesktop(desktop);
            throw error;
        }
        process = info.process;
        CloseHandle(info.thread);
    }

    public void Dispose()
    {
        if (Running) TerminateProcess(process, 0);
        WaitForSingleObject(process, 5000);
        CloseHandle(process);
        CloseDesktop(desktop);
    }
}
'@

& "$env:SystemRoot\System32\tar.exe" -xf $archivePath -C $work
if ($LASTEXITCODE -ne 0) { throw 'Portable ZIP extraction failed.' }
$original = (Resolve-Path -LiteralPath (Join-Path $work 'VideoDownloaderPortable')).Path
$moved = Join-Path $work '이동한 앱 폴더'
if ((Split-Path -Parent $original) -ne $work -or (Split-Path -Parent $moved) -ne $work -or (Test-Path -LiteralPath $moved)) {
    throw 'Invalid isolated move paths.'
}
$downloads = Join-Path $work '저장 폴더'
New-Item -ItemType Directory -Path (Join-Path $original 'data'), $downloads | Out-Null
$settings = @{
    schemaVersion = 1; downloadDirectory = $downloads; quality = '720'; format = 'mkv'
    filename = 'title'; concurrentDownloads = 1; retryCount = 0; browserAutoDetect = $false
    browserExtensionId = ''; completionNotification = $false
}
$settings | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $original 'data/settings.json') -Encoding utf8NoBOM
$settingsHash = (Get-FileHash -LiteralPath (Join-Path $original 'data/settings.json')).Hash

function Test-PortableStart([string]$Folder, [string]$WebViewFolder) {
    $settingsPath = Join-Path $Folder 'data/settings.json'
    $lock = Join-Path $Folder 'data/settings.lock'
    if (Test-Path -LiteralPath $lock) { [IO.File]::Delete($lock) }
    $previous = $env:WEBVIEW2_USER_DATA_FOLDER
    try {
        $env:WEBVIEW2_USER_DATA_FOLDER = $WebViewFolder
        $app = [PortableRuntimeProcess]::new((Join-Path $Folder 'VideoDownloader.exe'), $work)
        try {
            $deadline = [DateTime]::UtcNow.AddSeconds(30)
            while ($app.Running -and !(Test-Path -LiteralPath $lock) -and [DateTime]::UtcNow -lt $deadline) {
                Start-Sleep -Milliseconds 100
            }
            if (!$app.Running -or !(Test-Path -LiteralPath $lock)) {
                throw 'Release executable did not mount and read its adjacent settings.'
            }
            if ((Get-FileHash -LiteralPath $settingsPath).Hash -ne $settingsHash) { throw 'Settings bytes changed.' }
            if (!(Test-Path -LiteralPath $WebViewFolder)) { throw 'Isolated WebView profile was not initialized.' }
        } finally { $app.Dispose() }
    } finally { $env:WEBVIEW2_USER_DATA_FOLDER = $previous }
}

Test-PortableStart $original (Join-Path $work 'webview-before')
Move-Item -LiteralPath $original -Destination $moved
if ((Test-Path -LiteralPath $original) -or (Resolve-Path -LiteralPath $moved).Path -ne $moved) {
    throw 'Portable move did not complete.'
}
Test-PortableStart $moved (Join-Path $work 'webview-after')
$principal = [Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
$elevated = $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
"PORTABLE_RUNTIME_PASS: original and moved release EXE, isolated WebView, unchanged settings, elevated=$elevated"
