# 普通构建与正式发布共用同一目录布局，不保留源码路径层级。
param(
    [Parameter(Mandatory = $true)][string]$ExePath,
    [Parameter(Mandatory = $true)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
$repositoryRoot = Split-Path $PSScriptRoot -Parent
$files = @(
    (Get-Item -LiteralPath $ExePath).FullName,
    (Join-Path $repositoryRoot 'README.md'),
    (Join-Path $PSScriptRoot 'Measure-MiniDeck.ps1')
)
foreach ($file in $files) {
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "缺少打包文件：$file" }
}
if (Test-Path -LiteralPath $OutputDirectory) { throw '打包目录已存在，请使用新目录以避免混入旧文件。' }
$null = New-Item -ItemType Directory -Path $OutputDirectory
Copy-Item -LiteralPath $files[0] -Destination (Join-Path $OutputDirectory 'mini-deck.exe')
Copy-Item -LiteralPath $files[1] -Destination (Join-Path $OutputDirectory 'README.md')
Copy-Item -LiteralPath $files[2] -Destination (Join-Path $OutputDirectory 'Measure-MiniDeck.ps1')
$expected = @('mini-deck.exe', 'README.md', 'Measure-MiniDeck.ps1') | Sort-Object
$actual = @(Get-ChildItem -LiteralPath $OutputDirectory -Force | ForEach-Object Name | Sort-Object)
if (Compare-Object $expected $actual) { throw '下载包文件清单不正确。' }
if (Get-ChildItem -LiteralPath $OutputDirectory -Directory) { throw '下载包不应包含子目录。' }
Write-Host "下载包已整理到 $OutputDirectory，直接运行 mini-deck.exe。"
