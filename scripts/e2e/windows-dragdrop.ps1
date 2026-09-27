# End-to-end drag and drop on a real Windows desktop (the GitHub windows runner).
#
#   1. Shares a folder over SMB on this machine and points a fresh Neat NAS at it.
#   2. Drags a file and a folder from the app into an Explorer window and
#      checks the copies byte for byte.
#   3. Drops a file from another window onto the app and checks that it is
#      uploaded to the share.
#   Before that, a toolbar tour photographs the toolbar in several languages
#   and widths (contact sheets in the logs folder).
#
# Input is simulated with SendInput, so this needs an interactive desktop.
# Usage (pwsh, as an administrator): scripts/e2e/windows-dragdrop.ps1 [-App path\to\neatnas.exe]
param(
  [string]$App = "src-tauri\target\debug\neatnas.exe",
  [string]$Logs = "e2e-logs"
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
using System.Threading;

public static class Desk {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [StructLayout(LayoutKind.Sequential)] struct MOUSEINPUT { public int dx, dy; public uint mouseData, dwFlags, time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)] struct INPUT { public uint type; public MOUSEINPUT mi; }
  [StructLayout(LayoutKind.Sequential)] struct KEYBDINPUT { public ushort wVk, wScan; public uint dwFlags, time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Explicit, Size = 40)] struct KINPUT { [FieldOffset(0)] public uint type; [FieldOffset(8)] public KEYBDINPUT ki; }
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  struct DEVMODE {
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
    public short dmSpecVersion, dmDriverVersion, dmSize, dmDriverExtra;
    public int dmFields, dmPositionX, dmPositionY, dmDisplayOrientation, dmDisplayFixedOutput;
    public short dmColor, dmDuplex, dmYResolution, dmTTOption, dmCollate;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
    public short dmLogPixels;
    public int dmBitsPerPel, dmPelsWidth, dmPelsHeight, dmDisplayFlags, dmDisplayFrequency, dmICMMethod, dmICMIntent, dmMediaType, dmDitherType, dmReserved1, dmReserved2, dmPanningWidth, dmPanningHeight;
  }
  [DllImport("user32.dll", EntryPoint = "SendInput")] static extern uint SendKeyInput(uint count, KINPUT[] inputs, int size);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern bool EnumDisplaySettings(string device, int mode, ref DEVMODE dm);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int ChangeDisplaySettings(ref DEVMODE dm, int flags);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr hdc, uint flags);

  /// 0 when the display switched to w x h.
  public static int SetResolution(int w, int h) {
    var dm = new DEVMODE();
    dm.dmSize = (short)Marshal.SizeOf(typeof(DEVMODE));
    if (!EnumDisplaySettings(null, -1, ref dm)) return -100;
    dm.dmPelsWidth = w; dm.dmPelsHeight = h; dm.dmFields = 0x80000 | 0x100000;
    return ChangeDisplaySettings(ref dm, 0);
  }
  public static void Key(ushort vk, bool up) {
    var k = new KINPUT(); k.type = 1; k.ki.wVk = vk; k.ki.dwFlags = up ? 2u : 0u;
    if (SendKeyInput(1, new[] { k }, Marshal.SizeOf(typeof(KINPUT))) != 1) throw new Exception("SendInput (key) failed: " + Marshal.GetLastWin32Error());
  }
  public static void Press(ushort vk) { Key(vk, false); Key(vk, true); }
  public static void Chord(ushort modifier, ushort vk) { Key(modifier, false); Key(vk, false); Key(vk, true); Key(modifier, true); }

  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern int GetSystemMetrics(int index);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref POINT p);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] static extern uint SendInput(uint count, INPUT[] inputs, int size);

  const uint MOVE = 0x0001, LEFTDOWN = 0x0002, LEFTUP = 0x0004, ABSOLUTE = 0x8000;

  static void Send(uint flags, int x, int y) {
    int w = GetSystemMetrics(0), h = GetSystemMetrics(1);
    var input = new INPUT { type = 0 };
    input.mi.dx = (int)Math.Round(x * 65535.0 / (w - 1));
    input.mi.dy = (int)Math.Round(y * 65535.0 / (h - 1));
    input.mi.dwFlags = flags | ABSOLUTE | MOVE;
    if (SendInput(1, new[] { input }, Marshal.SizeOf(typeof(INPUT))) != 1)
      throw new Exception("SendInput failed: " + Marshal.GetLastWin32Error());
  }
  public static void Move(int x, int y) { Send(0, x, y); }
  public static void Down(int x, int y) { Send(LEFTDOWN, x, y); }
  public static void Up(int x, int y) { Send(LEFTUP, x, y); }
  /// Move in small steps, so every window on the way sees the pointer pass.
  public static void Glide(int x0, int y0, int x1, int y1, int steps, int pauseMs) {
    for (int i = 1; i <= steps; i++) {
      Move(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps);
      Thread.Sleep(pauseMs);
    }
  }
  /// Glide and release on a thread of its own (for while a drag loop runs here).
  public static Thread GlideLater(int x0, int y0, int x1, int y1, int delayMs, int holdMs) {
    var t = new Thread(() => { Thread.Sleep(delayMs); Glide(x0, y0, x1, y1, 40, 25); Thread.Sleep(holdMs); Up(x1, y1); });
    t.IsBackground = true;
    t.Start();
    return t;
  }
  public static POINT Cursor() { POINT p; GetCursorPos(out p); return p; }
  public static POINT ClientOrigin(IntPtr hwnd) { var p = new POINT(); ClientToScreen(hwnd, ref p); return p; }
}
"@
[void][Desk]::SetProcessDPIAware()

$root = (Resolve-Path .).Path
$App = (Resolve-Path $App).Path
New-Item -ItemType Directory -Force $Logs | Out-Null
$Logs = (Resolve-Path $Logs).Path
$appLog = Join-Path $Logs "neatnas.log"
$failures = [System.Collections.Generic.List[string]]::new()

function Step($text) { Write-Host "`n== $text" }
function Fail($text) { Write-Host "FAIL: $text"; $failures.Add($text) }
function Pass($text) { Write-Host "ok: $text" }

function Wait-Until([scriptblock]$Condition, [int]$Seconds, [string]$What) {
  $deadline = (Get-Date).AddSeconds($Seconds)
  while ((Get-Date) -lt $deadline) {
    if (& $Condition) { return $true }
    Start-Sleep -Milliseconds 250
  }
  Write-Host "timed out after ${Seconds}s waiting for $What"
  return $false
}

function Log-Text { if (Test-Path $appLog) { Get-Content $appLog -Raw -ErrorAction SilentlyContinue } else { "" } }

# The top strip of a window: from the screen when it fits, else PrintWindow.
function Snap-Window([IntPtr]$hwnd, [string]$name, [int]$w, [int]$h, [string]$label) {
  $path = Join-Path $Logs "$name.png"
  $o = [Desk]::ClientOrigin($hwnd)
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  if ($o.X -ge 0 -and $o.X + $w -le $screenW) {
    $g.CopyFromScreen($o.X, $o.Y, 0, 0, $bmp.Size)
  } else {
    $hdc = $g.GetHdc(); [void][Desk]::PrintWindow($hwnd, $hdc, 2); $g.ReleaseHdc($hdc)
  }
  $g.Dispose(); $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
  [pscustomobject]@{ Label = $label; Path = $path; Width = $w; Height = $h }
}

function Save-Sheet($shots, [string]$name) {
  $pad = 10; $line = 20
  $width = ($shots | Measure-Object -Property Width -Maximum).Maximum + 2 * $pad
  $height = $pad + ($shots | ForEach-Object { $line + $_.Height + $pad } | Measure-Object -Sum).Sum
  $sheet = New-Object System.Drawing.Bitmap ([int]$width), ([int]$height)
  $g = [System.Drawing.Graphics]::FromImage($sheet)
  $g.Clear([System.Drawing.Color]::FromArgb(238, 238, 238))
  $font = New-Object System.Drawing.Font "Segoe UI", 10
  $y = $pad
  foreach ($s in $shots) {
    $g.DrawString($s.Label, $font, [System.Drawing.Brushes]::Black, $pad, $y); $y += $line
    $img = [System.Drawing.Image]::FromFile($s.Path); $g.DrawImage($img, $pad, $y, $img.Width, $img.Height); $img.Dispose()
    $y += $s.Height + $pad
  }
  $g.Dispose(); $sheet.Save((Join-Path $Logs "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png); $sheet.Dispose()
}

function Same-File($a, $b) {
  try { (Test-Path $a) -and (Test-Path $b) -and ((Get-FileHash $a -ErrorAction Stop).Hash -eq (Get-FileHash $b -ErrorAction Stop).Hash) }
  catch { $false }
}

# ── Desktop ──────────────────────────────────────────────────────────────
Step "desktop"
foreach ($mode in @(@(1920, 1080), @(1680, 1050), @(1600, 900), @(1440, 900))) {
  $r = [Desk]::SetResolution($mode[0], $mode[1])
  Write-Host "display $($mode[0])x$($mode[1]): $r"
  if ($r -eq 0) { Start-Sleep -Seconds 2; break }
}
$screenW = [Desk]::GetSystemMetrics(0); $screenH = [Desk]::GetSystemMetrics(1)
Write-Host "screen ${screenW}x${screenH}, session $([System.Diagnostics.Process]::GetCurrentProcess().SessionId), interactive $([Environment]::UserInteractive), apartment $([Threading.Thread]::CurrentThread.ApartmentState)"
if ([Threading.Thread]::CurrentThread.ApartmentState -ne "STA") { throw "OLE drag and drop needs pwsh in STA mode" }
[Desk]::Move(200, 200); Start-Sleep -Milliseconds 100
$c = [Desk]::Cursor()
if ($c.X -ne 200 -or $c.Y -ne 200) { throw "simulated input does not reach this desktop (cursor at $($c.X),$($c.Y))" }
Pass "simulated input moves the cursor"

# ── SMB share ────────────────────────────────────────────────────────────
Step "SMB share"
$share = "C:\e2e-share"
$password = "N3at-Nas-E2E!2026"
Remove-Item -Recurse -Force $share -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$share\Folder\Sub" | Out-Null
[IO.File]::WriteAllText("$share\hello.txt", "hello from the Windows runner`r`n")
$bytes = New-Object byte[] (5MB + 123); (New-Object Random 42).NextBytes($bytes)
[IO.File]::WriteAllBytes("$share\Folder\big.bin", $bytes)
[IO.File]::WriteAllText("$share\Folder\Sub\deep.txt", "deep inside")
net user nastest $password /add /y | Out-Null
icacls $share /grant "nastest:(OI)(CI)F" | Out-Null
if (-not (Get-SmbShare -Name e2e -ErrorAction SilentlyContinue)) { New-SmbShare -Name e2e -Path $share -FullAccess nastest | Out-Null }
if ((Get-Service LanmanServer).Status -ne "Running") { Start-Service LanmanServer }
Get-Service LanmanServer | Format-Table -AutoSize Name, Status | Out-String | Write-Host
Pass "\\127.0.0.1\e2e serves $share"

# ── App ──────────────────────────────────────────────────────────────────
Step "app"
$configDir = Join-Path $env:APPDATA "com.neatnas.app"
New-Item -ItemType Directory -Force $configDir | Out-Null
$config = @{
  servers = @(@{ id = "e2e"; name = "E2E"; host = "127.0.0.1"; port = 445; username = "nastest"; domain = ""; lastShare = "e2e"; addedAt = 0 })
  insecurePasswords = @{ e2e = $password }
} | ConvertTo-Json -Depth 5
[IO.File]::WriteAllText((Join-Path $configDir "config.json"), $config, (New-Object Text.UTF8Encoding $false))
$env:NEATNAS_CRED_STORE = "file"
$env:RUST_LOG = "info,smb2=warn,mdns_sd=warn"

# ── Toolbar tour ─────────────────────────────────────────────────────────
Step "toolbar tour"
$tour = @("en", "zh-CN", "ja", "de", "fr", "ru")
$widths = @(1400, 1280, 1120, 1000, 920) | Where-Object { $_ -le $screenW }
$tourLog = Join-Path $Logs "tour.log"
$env:NEATNAS_DEV_AUTOPILOT = "rects," + (($tour | ForEach-Object { "lang=$_,wait=16000" }) -join ",") + ",lang=system"
$tourProc = Start-Process -FilePath $App -PassThru -RedirectStandardError $tourLog -RedirectStandardOutput (Join-Path $Logs "tour-stdout.log")
try {
  if (-not (Wait-Until { (Get-Content $tourLog -Raw -ErrorAction SilentlyContinue) -match "autopilot rect name=" } 120 "the tour listing")) { throw "tour: no listing" }
  $tourProc.Refresh(); $th = $tourProc.MainWindowHandle
  $park = { [Desk]::Move([int]($screenW / 2), $screenH - 90) }
  foreach ($lang in $tour) {
    if (-not (Wait-Until { (Get-Content $tourLog -Raw -ErrorAction SilentlyContinue) -match "autopilot step=lang=$([regex]::Escape($lang)) view" } 40 "the switch to $lang")) { Fail "tour: the app never switched to $lang"; break }
    Start-Sleep -Milliseconds 800
    $shots = [System.Collections.Generic.List[object]]::new()
    foreach ($w in $widths) {
      [void][Desk]::SetWindowPos($th, [IntPtr]::Zero, 0, 0, $w, 640, 0x0040)
      & $park; Start-Sleep -Milliseconds 700
      $shots.Add((Snap-Window $th "tour-$lang-$w" $w 58 "$lang, ${w}px"))
    }
    # Narrowest width: the search opened with Ctrl+F.
    [void][Desk]::SetWindowPos($th, [IntPtr]::Zero, 0, 0, 920, 640, 0x0040)
    [void][Desk]::SetForegroundWindow($th); Start-Sleep -Milliseconds 400
    $o = [Desk]::ClientOrigin($th)
    [Desk]::Move($o.X + 600, $o.Y + 420); [Desk]::Down($o.X + 600, $o.Y + 420); [Desk]::Up($o.X + 600, $o.Y + 420)
    & $park; Start-Sleep -Milliseconds 300
    [Desk]::Chord(0x11, 0x46); Start-Sleep -Milliseconds 700
    $shots.Add((Snap-Window $th "tour-$lang-920-search" 920 58 "$lang, 920px, search opened (Ctrl+F)"))
    [Desk]::Press(0x1B); Start-Sleep -Milliseconds 400
    if ($lang -eq "en") {
      [Desk]::Move($o.X + 897, $o.Y + 26); Start-Sleep -Milliseconds 500
      $shots.Add((Snap-Window $th "tour-en-920-hover" 920 58 "en, 920px, close button hovered"))
      & $park
    }
    Save-Sheet $shots "toolbar-$lang"
    Pass "toolbar photographed in $lang"
  }
  # Let the tour put the language back before the app closes.
  [void](Wait-Until { (Get-Content $tourLog -Raw -ErrorAction SilentlyContinue) -match "autopilot step=lang=system" } 25 "the language reset")
}
finally {
  if (-not $tourProc.HasExited) { Stop-Process -Id $tourProc.Id -Force }
  Start-Sleep -Milliseconds 800
}

# ── Drag and drop ────────────────────────────────────────────────────────
$env:NEATNAS_DEV_AUTOPILOT = "rects"
$proc = Start-Process -FilePath $App -PassThru -RedirectStandardError $appLog -RedirectStandardOutput (Join-Path $Logs "stdout.log")
try {
  if (-not (Wait-Until { (Log-Text) -match "autopilot rect name=hello\.txt" } 120 "the share listing")) {
    throw "the app never showed the share listing; see $appLog"
  }
  $proc.Refresh()
  $hwnd = $proc.MainWindowHandle
  if ($hwnd -eq [IntPtr]::Zero) { throw "no main window" }
  # Top-left, at the minimum width; the rest of the screen is for Explorer.
  [void][Desk]::SetWindowPos($hwnd, [IntPtr]::Zero, 0, 0, 920, 640, 0x0040)
  [void][Desk]::SetForegroundWindow($hwnd)
  Start-Sleep -Milliseconds 800
  Pass "listing on screen"

  # Row positions reported by the app (CSS pixels in the window).
  $rows = @{}
  foreach ($m in [regex]::Matches((Log-Text), "autopilot rect name=(.+?) x=(-?\d+) y=(-?\d+) w=(\d+) h=(\d+) dpr=([\d.]+)")) {
    $rows[$m.Groups[1].Value] = [pscustomobject]@{ X = [int]$m.Groups[2].Value; Y = [int]$m.Groups[3].Value; W = [int]$m.Groups[4].Value; H = [int]$m.Groups[5].Value; Dpr = [double]$m.Groups[6].Value }
  }
  Write-Host ("rows: " + (($rows.Keys | Sort-Object) -join ", "))
  $origin = [Desk]::ClientOrigin($hwnd)
  function Row-Point($name) {
    $r = $rows[$name]
    if (-not $r) { throw "no row named $name" }
    [pscustomobject]@{ X = [int]($origin.X + ($r.X + 60) * $r.Dpr); Y = [int]($origin.Y + ($r.Y + $r.H / 2) * $r.Dpr) }
  }

  # ── Drag out to Explorer ───────────────────────────────────────────────
  Step "drag to Explorer"
  $out = "C:\e2e-out"
  Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
  New-Item -ItemType Directory -Force $out | Out-Null
  Start-Process explorer.exe $out
  $shell = New-Object -ComObject Shell.Application
  $explorer = $null
  [void](Wait-Until {
    $found = @($shell.Windows() | Where-Object { $_.LocationURL -like "*e2e-out*" })
    if ($found.Count -gt 0) { $script:explorer = $found[0] }
    $null -ne $script:explorer
  } 30 "the Explorer window")
  if (-not $explorer) {
    Write-Host ("shell windows: " + ((@($shell.Windows()) | ForEach-Object { $_.LocationURL }) -join ", "))
    throw "Explorer did not open $out"
  }
  $ex = [IntPtr][long]$explorer.HWND
  $exW = 440; $exH = [Math]::Min(380, $screenH - 360)
  $exX = $screenW - $exW - 10; $exY = 300
  [void][Desk]::SetWindowPos($ex, [IntPtr](-1), $exX, $exY, $exW, $exH, 0x0040) # topmost, shown
  Start-Sleep -Milliseconds 1500
  $dropX = $exX + [int]($exW / 2); $dropY = $exY + [int]($exH / 2) + 20

  function Drag-Out($name) {
    $p = Row-Point $name
    [Desk]::Move($p.X, $p.Y); Start-Sleep -Milliseconds 300
    [Desk]::Down($p.X, $p.Y); Start-Sleep -Milliseconds 200
    # Past the app's 6px threshold; the app then hands the gesture to Windows.
    [Desk]::Glide($p.X, $p.Y, $p.X + 30, $p.Y + 12, 10, 25)
    Start-Sleep -Milliseconds 1200
    [Desk]::Glide($p.X + 30, $p.Y + 12, $dropX, $dropY, 40, 25)
    Start-Sleep -Milliseconds 800
    [Desk]::Up($dropX, $dropY)
  }

  Drag-Out "hello.txt"
  if (Wait-Until { Same-File "$share\hello.txt" "$out\hello.txt" } 60 "hello.txt in Explorer") { Pass "file dragged to Explorer arrived intact" }
  else { Fail "hello.txt did not arrive in $out" }

  Drag-Out "Folder"
  $folderOk = Wait-Until { (Same-File "$share\Folder\big.bin" "$out\Folder\big.bin") -and (Same-File "$share\Folder\Sub\deep.txt" "$out\Folder\Sub\deep.txt") } 120 "Folder in Explorer"
  if ($folderOk) { Pass "folder dragged to Explorer arrived intact (5 MB file and a nested folder)" }
  else { Fail "Folder did not arrive complete in $out" }
  Get-ChildItem -Recurse $out | Format-Table -AutoSize FullName, Length | Out-String | Write-Host
  [void][Desk]::SetWindowPos($ex, [IntPtr](-2), 0, 0, 0, 0, 0x0003) # no longer topmost
  $explorer.Quit()

  # ── Drop onto the app ──────────────────────────────────────────────────
  Step "drop onto the app"
  $drop = "C:\e2e-drop\dropped.txt"
  New-Item -ItemType Directory -Force (Split-Path $drop) | Out-Null
  [IO.File]::WriteAllText($drop, "dropped from Explorer at $(Get-Date -Format o)`r`n")
  [void][Desk]::SetForegroundWindow($hwnd)
  $targetX = $origin.X + 560; $targetY = $origin.Y + 420

  $form = New-Object System.Windows.Forms.Form
  $form.Text = "drag source"; $form.TopMost = $true; $form.ShowInTaskbar = $false
  $form.FormBorderStyle = "FixedToolWindow"; $form.StartPosition = "Manual"
  $form.Location = New-Object System.Drawing.Point(($screenW - 170), 360); $form.Size = New-Object System.Drawing.Size(150, 120)
  $form.Add_Shown({
    $form.Activate()
    $sx = $form.Left + 75; $sy = $form.Top + 70
    [Desk]::Move($sx, $sy); Start-Sleep -Milliseconds 200
    [Desk]::Down($sx, $sy); Start-Sleep -Milliseconds 150
    [System.Windows.Forms.Application]::DoEvents()
    $mover = [Desk]::GlideLater($sx, $sy, $targetX, $targetY, 600, 800)
    $data = New-Object System.Windows.Forms.DataObject
    $files = New-Object System.Collections.Specialized.StringCollection
    [void]$files.Add($drop)
    $data.SetFileDropList($files)
    $effect = $form.DoDragDrop($data, [System.Windows.Forms.DragDropEffects]::Copy)
    Write-Host "DoDragDrop returned $effect"
    $mover.Join()
    $form.Close()
  })
  [void]$form.ShowDialog()

  if (Wait-Until { (Log-Text) -match "file drop: 1 item" } 15 "the app to report the drop") { Pass "the app received the drop" }
  else { Fail "the app never reported the drop" }
  if (Wait-Until { Same-File $drop "$share\dropped.txt" } 60 "the upload") { Pass "dropped file was uploaded to the share" }
  else { Fail "dropped.txt did not reach $share" }
}
finally {
  if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
  Start-Sleep -Milliseconds 500
  Write-Host "`n== app log (drag, drop and errors)"
  Get-Content $appLog | Select-String -Pattern "drag|drop|error|ERROR|WARN" | Select-Object -Last 60 | ForEach-Object { Write-Host $_.Line }
}

if ($failures.Count -gt 0) {
  Write-Host "`n$($failures.Count) check(s) failed:"; $failures | ForEach-Object { Write-Host " - $_" }
  exit 1
}
Write-Host "`nall drag and drop checks passed"
