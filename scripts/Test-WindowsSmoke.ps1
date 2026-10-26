[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$ExePath)
$ErrorActionPreference = 'Stop'
$ExePath = (Resolve-Path $ExePath).Path
$directory = Join-Path ([IO.Path]::GetTempPath()) ('mini-deck-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $directory | Out-Null
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$port = $listener.LocalEndpoint.Port
$listener.Stop()
$base = "http://127.0.0.1:$port"
$deckProcess = $null
$config = Join-Path $directory 'config.json'
function Start-Deck {
    $log = Join-Path $directory ([Guid]::NewGuid().ToString('N') + '.log')
    $script:deckProcess = Start-Process -FilePath $ExePath -ArgumentList @('--mock', '--listen', "127.0.0.1:$port", '--config', "`"$config`"") -PassThru -RedirectStandardOutput $log -RedirectStandardError ($log + '.err')
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while ($true) {
        if ($script:deckProcess.HasExited) { throw "服务提前退出：$(Get-Content ($log + '.err') -Raw)" }
        try { $null = Invoke-RestMethod "$base/api/health" -TimeoutSec 1; break } catch {
            if ([DateTime]::UtcNow -gt $deadline) { throw '服务启动超时。' }
            Start-Sleep -Milliseconds 100
        }
    }
    $content = Get-Content $log -Raw -Encoding UTF8
    if ($content -notmatch '配对码：([a-f0-9]{8})') { throw '找不到配对码。' }
    return $Matches[1]
}
function Stop-Deck {
    if ($script:deckProcess -and -not $script:deckProcess.HasExited) {
        Stop-Process -Id $script:deckProcess.Id
        $script:deckProcess.WaitForExit()
    }
}
function Assert-Unauthorized($headers) {
    try { $null = Invoke-RestMethod "$base/api/buttons" -Headers $headers; throw '撤销后的令牌仍然有效。' }
    catch { if (-not $_.Exception.Response -or [int]$_.Exception.Response.StatusCode -ne 401) { throw } }
}
try {
    $code = Start-Deck
    $body = @{code = $code; remember = $true; name = '自动化测试设备'} | ConvertTo-Json
    $pair = Invoke-RestMethod "$base/api/pair" -Method Post -ContentType 'application/json' -Body ([Text.Encoding]::UTF8.GetBytes($body))
    $headers = @{Authorization = "Bearer $($pair.token)"}
    $null = Invoke-RestMethod "$base/api/buttons/0/press" -Headers $headers -Method Post
    $connect = Invoke-WebRequest "$base/connect" -UseBasicParsing
    if ($connect.Content -notmatch '<svg') { throw '未生成二维码。' }
    & (Join-Path $PSScriptRoot 'Measure-MiniDeck.ps1') -ProcessId $deckProcess.Id -WarmupSeconds 0 -DurationSeconds 2 -IntervalSeconds 1 -HealthUrl "$base/api/health" -OutputDirectory (Join-Path $directory 'measure')
    if (-not (Test-Path (Join-Path $directory 'measure/summary.json'))) { throw '未生成测量报告。' }
    Stop-Deck
    $null = Start-Deck
    $null = Invoke-RestMethod "$base/api/buttons" -Headers $headers
    $devices = @(Invoke-RestMethod "$base/api/devices" -Headers $headers)
    if ($devices.Count -ne 1 -or $devices[0].name -ne '自动化测试设备') { throw '设备持久化验证失败。' }
    $null = Invoke-RestMethod "$base/api/devices/$($devices[0].id)" -Headers $headers -Method Delete
    Assert-Unauthorized $headers
    Stop-Deck
    $null = Start-Deck
    Assert-Unauthorized $headers
    Write-Host 'Windows 冒烟测试通过：二维码、记住设备、重启恢复、撤销持久化和测量脚本。所有按键均为 Mock。'
} finally {
    Stop-Deck
    Remove-Item -LiteralPath $directory -Recurse -Force
}
