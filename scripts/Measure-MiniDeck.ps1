[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][ValidateRange(1, 2147483647)][int]$ProcessId,
    [ValidateRange(1, 604800)][int]$DurationSeconds = 600,
    [ValidateRange(1, 3600)][int]$IntervalSeconds = 10,
    [ValidateRange(0, 3600)][int]$WarmupSeconds = 30,
    [string]$HealthUrl = '',
    [string]$OutputDirectory = (Join-Path $PWD ('mini-deck-measure-' + (Get-Date -Format 'yyyyMMdd-HHmmss')))
)
$ErrorActionPreference = 'Stop'
if ($env:OS -ne 'Windows_NT') { throw '此脚本仅用于 Windows 真机测量。' }
$process = Get-Process -Id $ProcessId
$started = $process.StartTime.ToUniversalTime()
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$csvPath = Join-Path $OutputDirectory 'samples.csv'
if (Test-Path $csvPath) { throw '输出目录已有采样文件，请使用新的目录，避免混入旧数据。' }
Write-Host "预热 $WarmupSeconds 秒；采样 $DurationSeconds 秒；PID $ProcessId。"
if ($WarmupSeconds -gt 0) { Start-Sleep -Seconds $WarmupSeconds }
$process.Refresh()
$previousCpu = $process.TotalProcessorTime.TotalSeconds
$clock = [Diagnostics.Stopwatch]::StartNew()
$previousTime = 0.0
$latencies = @()
if ($HealthUrl) {
    $uri = [Uri]$HealthUrl
    if ($uri.Scheme -ne 'http' -or $uri.AbsolutePath -ne '/api/health') { throw 'HealthUrl 必须是 HTTP /api/health 地址。' }
    1..20 | ForEach-Object {
        $requestClock = [Diagnostics.Stopwatch]::StartNew()
        $result = Invoke-WebRequest -Uri $uri -UseBasicParsing -TimeoutSec 5
        if ($result.StatusCode -ne 200) { throw '健康检查未成功。' }
        $latencies += $requestClock.Elapsed.TotalMilliseconds
    }
}
$stoppedEarly = $false
while ($true) {
    try {
        $sample = Get-Process -Id $ProcessId -ErrorAction Stop
        if ($sample.StartTime.ToUniversalTime() -ne $started) { throw 'PID 已被其他进程复用。' }
        $elapsed = $clock.Elapsed.TotalSeconds
        $cpu = $sample.TotalProcessorTime.TotalSeconds
        $percent = if ($elapsed -gt $previousTime) { 100 * ($cpu - $previousCpu) / ($elapsed - $previousTime) } else { 0 }
        [pscustomobject]@{
            Time = (Get-Date).ToUniversalTime().ToString('o')
            ElapsedSeconds = $elapsed
            WorkingSetMB = $sample.WorkingSet64 / 1MB
            PrivateMemoryMB = $sample.PrivateMemorySize64 / 1MB
            CPUSeconds = $cpu
            OneCoreCPUPercent = $percent
            Threads = $sample.Threads.Count
            Handles = $sample.HandleCount
        } | Export-Csv -Path $csvPath -Append -NoTypeInformation -Encoding UTF8
        $previousCpu = $cpu
        $previousTime = $elapsed
    } catch {
        Write-Warning "采样提前结束：$_"
        $stoppedEarly = $true
        break
    }
    $remaining = $DurationSeconds - $clock.Elapsed.TotalSeconds
    if ($remaining -le 0) { break }
    Start-Sleep -Milliseconds ([int]([Math]::Min($IntervalSeconds, $remaining) * 1000))
}
$rows = @(Import-Csv -Path $csvPath)
$summary = [ordered]@{
    ProcessId = $ProcessId
    ProcessStartedUTC = $started.ToString('o')
    Samples = $rows.Count
    DurationSeconds = $clock.Elapsed.TotalSeconds
    StoppedEarly = $stoppedEarly
    LogicalProcessors = [Environment]::ProcessorCount
    OS = [Environment]::OSVersion.VersionString
    PeakWorkingSetMB = ($rows | Measure-Object -Property WorkingSetMB -Maximum).Maximum
    PeakPrivateMemoryMB = ($rows | Measure-Object -Property PrivateMemoryMB -Maximum).Maximum
    CPUSecondsDuringMeasurement = $previousCpu - [double]$rows[0].CPUSeconds
    Note = 'CPU 百分比按单个逻辑核心计算；外部测量脚本本身有开销，健康请求包含客户端与建连成本。'
}
if ($latencies.Count -gt 0) {
    $sorted = @($latencies | Sort-Object)
    $summary.HealthRequests = $sorted.Count
    $summary.HealthMedianMS = ($sorted[9] + $sorted[10]) / 2
    $summary.HealthP95MS = $sorted[18]
}
$summary | ConvertTo-Json | Set-Content -Path (Join-Path $OutputDirectory 'summary.json') -Encoding UTF8
Write-Host "测量完成：$OutputDirectory"
$summary | Format-List
