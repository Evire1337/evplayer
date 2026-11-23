# Measures a program the way the README's numbers were taken: starts it, waits, then sums the
# memory of the program and every process it started (WebView2 runs as a tree of processes),
# and how much processor time the whole tree used while it sat idle.
#
#   powershell -File native\tools\measure.ps1 -Exe path\to\program.exe [-Arguments "--hidden"] [-Settle 10] [-Idle 10]
#
# Nothing is played: the program is started, left alone, and closed.
param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [string]$Arguments = "",
  [int]$Settle = 10,
  [int]$Idle = 10,
  [string]$Label = ""
)

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
# time until the main window exists (a rough "started" for both builds)
$shown = $null
for ($i = 0; $i -lt 300; $i++) {
  $proc.Refresh()
  if ($proc.MainWindowHandle -ne 0) { $shown = ((Get-Date) - $start).TotalMilliseconds; break }
  Start-Sleep -Milliseconds 20
}
Start-Sleep -Seconds $Settle
$ids = Tree $proc.Id
$a = Sample $ids
Start-Sleep -Seconds $Idle
$ids = Tree $proc.Id
$b = Sample $ids
$name = if ($Label) { $Label } else { [IO.Path]::GetFileName($Exe) }
"{0}: processes={1} threads={2} WS={3:N1} MB private={4:N1} MB start-cpu={8:N2} s idle-cpu={5:N3} s per {6} s window={7}" -f $name, $b.n, $b.threads, ($b.ws / 1MB), ($b.priv / 1MB), ($b.cpu - $a.cpu), $Idle, $(if ($shown) { "{0:N0} ms" -f $shown } else { "hidden" }), $a.cpu
foreach ($id in $ids) { Stop-Process -Id $id -Force -ErrorAction SilentlyContinue }
