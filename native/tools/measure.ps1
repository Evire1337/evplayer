# Measures a program the way the README's numbers were taken: starts it, waits, then sums the
# memory of the program and every process it started (WebView2 runs as a tree of processes),
# and how much processor time the whole tree used while it sat idle.
#
#   powershell -File native\tools\measure.ps1 -Exe path\to\program.exe [-Arguments "--hidden"] [-Settle 10] [-Idle 10] [-Minimize 15]
#
# "window" is when the program has a visible top-level window. With -Minimize N the window is
# then minimized and the memory measured again N seconds later: a player mostly lives
# minimized or in the tray. Nothing is played: the program is started, left alone, and closed
# (only the processes it started are closed, and the tray icons they leave behind are cleared).
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [string]$Arguments = "",
  [int]$Settle = 10,
  [int]$Idle = 10,
  [string]$Label = "",
  [int]$Minimize = 0
)

Add-Type @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class Win {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  /// A program that is killed cannot take its tray icon away, and the taskbar only notices the
  /// dead icon when the mouse passes over it: pass a mouse over the tray and its overflow.
  public static void SweepTray() {
    var tray = FindWindowEx(IntPtr.Zero, IntPtr.Zero, "Shell_TrayWnd", null);
    var notify = FindWindowEx(tray, IntPtr.Zero, "TrayNotifyWnd", null);
    var pager = FindWindowEx(notify, IntPtr.Zero, "SysPager", null);
    Sweep(FindWindowEx(pager, IntPtr.Zero, "ToolbarWindow32", null));
    var overflow = FindWindowEx(IntPtr.Zero, IntPtr.Zero, "NotifyIconOverflowWindow", null);
    Sweep(FindWindowEx(overflow, IntPtr.Zero, "ToolbarWindow32", null));
  }
  static void Sweep(IntPtr bar) {
    if (bar == IntPtr.Zero) return;
    RECT r; GetClientRect(bar, out r);
    for (int y = 0; y < r.B; y += 4)
      for (int x = 0; x < r.R; x += 4)
        SendMessage(bar, 0x0200, IntPtr.Zero, (IntPtr)((y << 16) | (x & 0xFFFF)));
  }
  /// the program's own visible window titled "EvPlayer" (both builds have one; the native build
  /// also has an untitled 16x16 helper window of its window library that draws nothing, which
  /// .NET's MainWindowHandle takes for the main one)
  public static IntPtr Find(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p != pid || !IsWindowVisible(h)) return true;
      var sb = new StringBuilder(64); GetWindowText(h, sb, 64);
      if (sb.ToString() == "EvPlayer") { found = h; return false; }
      return true;
    }, IntPtr.Zero);
    return found;
  }
}
"@

function Tree([int]$root) {
  $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
  $ids = @($root)
  $grew = $true
  while ($grew) {
    $grew = $false
    foreach ($p in $all) {
      if ($ids -contains $p.ParentProcessId -and -not ($ids -contains $p.ProcessId)) { $ids += $p.ProcessId; $grew = $true }
    }
  }
  return $ids
}

function Sample([int[]]$ids) {
  $ws = 0; $priv = 0; $cpu = 0; $threads = 0; $n = 0
  foreach ($id in $ids) {
    $p = Get-Process -Id $id -ErrorAction SilentlyContinue
    if ($p) { $ws += $p.WorkingSet64; $priv += $p.PrivateMemorySize64; $cpu += $p.TotalProcessorTime.TotalSeconds; $threads += $p.Threads.Count; $n++ }
  }
  return @{ ws = $ws; priv = $priv; cpu = $cpu; threads = $threads; n = $n }
}

$start = Get-Date
if ($Arguments) { $proc = Start-Process -FilePath $Exe -ArgumentList $Arguments -PassThru } else { $proc = Start-Process -FilePath $Exe -PassThru }
# time until the program's window titled "EvPlayer" is visible (a rough "started" for both)
$shown = $null
$main = [IntPtr]::Zero
if (-not $Arguments) {
  for ($i = 0; $i -lt 500; $i++) {
    $main = [Win]::Find([uint32]$proc.Id)
    if ($main -ne [IntPtr]::Zero) { $shown = ((Get-Date) - $start).TotalMilliseconds; break }
    Start-Sleep -Milliseconds 10
  }
}
Start-Sleep -Seconds $Settle
$ids = Tree $proc.Id
$a = Sample $ids
Start-Sleep -Seconds $Idle
$ids = Tree $proc.Id
$b = Sample $ids
$name = if ($Label) { $Label } else { [IO.Path]::GetFileName($Exe) }
$line = "{0}: processes={1} threads={2} WS={3:N1} MB private={4:N1} MB start-cpu={8:N2} s idle-cpu={5:N3} s per {6} s window={7}" -f $name, $b.n, $b.threads, ($b.ws / 1MB), ($b.priv / 1MB), ($b.cpu - $a.cpu), $Idle, $(if ($shown) { "{0:N0} ms" -f $shown } else { "hidden" }), $a.cpu
if ($Minimize -gt 0 -and $main -ne [IntPtr]::Zero) {
  [void][Win]::ShowWindow($main, 6)
  Start-Sleep -Seconds $Minimize
  $ids = Tree $proc.Id
  $c = Sample $ids
  $line += " | minimized {0} s: processes={1} WS={2:N1} MB private={3:N1} MB" -f $Minimize, $c.n, ($c.ws / 1MB), ($c.priv / 1MB)
}
$line
foreach ($id in $ids) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
Start-Sleep -Milliseconds 500
[Win]::SweepTray()
