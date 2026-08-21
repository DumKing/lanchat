param(
  [string]$Tag = "v0.5.2",
  [string]$OutputDir = "release-assets/vision-models"
)

$ErrorActionPreference = "Stop"
if ([string]::IsNullOrWhiteSpace($env:VISION_CATALOG_SIGNING_KEY)) {
  throw "缺少 VISION_CATALOG_SIGNING_KEY。请从 GitHub Secret 或本机安全环境提供目录签名密钥。"
}

$root = Split-Path -Parent $PSScriptRoot
$output = Join-Path $root $OutputDir
$staging = Join-Path $output "staging"
$config = Get-Content (Join-Path $PSScriptRoot "vision-model-profiles.json") -Raw | ConvertFrom-Json

function Invoke-Checked {
  param([string]$Description, [scriptblock]$Command)
  & $Command
  if ($LASTEXITCODE -ne 0) {
    throw "$Description 失败，退出码：$LASTEXITCODE"
  }
}

if (Test-Path -LiteralPath $output) { Remove-Item -LiteralPath $output -Recurse -Force }
New-Item -ItemType Directory -Path $output -Force | Out-Null
Invoke-Checked "生成视觉模型档位" { node (Join-Path $PSScriptRoot "build-vision-model-packages.mjs") $staging }
foreach ($profile in $config.profiles) {
  $source = Join-Path $staging $profile.profileId
  $destination = Join-Path $output $profile.assetName
  Compress-Archive -Path (Join-Path $source "*") -DestinationPath $destination -Force
}
Remove-Item -LiteralPath $staging -Recurse -Force

$unsignedCatalog = Join-Path $output "vision-catalog.unsigned.json"
$signedCatalog = Join-Path $output "vision-catalog.json"
Invoke-Checked "生成视觉模型目录" { node (Join-Path $PSScriptRoot "write-vision-catalog.mjs") $output $Tag $unsignedCatalog }
Push-Location (Join-Path $root "src-tauri")
try {
  Invoke-Checked "签名视觉模型目录" { cargo run --quiet --bin sign_vision_catalog -- $unsignedCatalog $signedCatalog }
  Invoke-Checked "验证视觉模型目录" { cargo run --quiet --bin verify_vision_catalog -- $signedCatalog }
} finally {
  Pop-Location
}
Remove-Item -LiteralPath $unsignedCatalog -Force
Get-ChildItem -LiteralPath $output -File | Select-Object Name, Length
