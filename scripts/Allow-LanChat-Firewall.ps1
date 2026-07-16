$ErrorActionPreference = "Stop"

$current = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = New-Object Security.Principal.WindowsPrincipal($current)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
  Write-Host "Please run this script as Administrator."
  Read-Host "Press Enter to exit"
  exit 1
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$exePath = Join-Path $scriptDir "LanChat.exe"
$profiles = "Domain,Private,Public"
$rules = @(
  @{ Name = "LanChat App Inbound"; Params = @{ Program = $exePath } },
  @{ Name = "LanChat TCP Chat 18145"; Params = @{ Protocol = "TCP"; LocalPort = 18145 } },
  @{ Name = "LanChat UDP Presence 18146"; Params = @{ Protocol = "UDP"; LocalPort = 18146 } },
  @{ Name = "LanChat mDNS UDP 5353"; Params = @{ Protocol = "UDP"; LocalPort = 5353 } }
)

foreach ($rule in $rules) {
  Get-NetFirewallRule -DisplayName $rule.Name -ErrorAction SilentlyContinue | Remove-NetFirewallRule
  $params = @{
    DisplayName = $rule.Name
    Direction = "Inbound"
    Action = "Allow"
    Profile = $profiles
  }
  foreach ($key in $rule.Params.Keys) {
    $params[$key] = $rule.Params[$key]
  }
  New-NetFirewallRule @params | Out-Null
}

Write-Host "LanChat firewall rules have been added for Domain, Private, and Public networks."
Write-Host "Allowed: LanChat.exe, TCP 18145, UDP 18146, UDP 5353."
Read-Host "Press Enter to exit"
