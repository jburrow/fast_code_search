<#
.SYNOPSIS
  Registers (or removes) a Task Scheduler task that starts fast_code_search_server
  at logon of the current user. See docs/RUN-AT-STARTUP.md.

.PARAMETER Server
  Path to fast_code_search_server.exe (default: found on PATH).
.PARAMETER Config
  Path to the configuration file (default: %APPDATA%\fast_code_search\config.toml).
.PARAMETER Uninstall
  Remove the task instead of creating it.
#>
param(
    [string]$Server = "",
    [string]$Config = (Join-Path $env:APPDATA "fast_code_search\config.toml"),
    [switch]$Uninstall
)

$TaskName = "FastCodeSearch"

if ($Uninstall) {
    if (Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue) {
        Stop-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
        Unregister-ScheduledTask -TaskName $TaskName -Confirm:$false
        Write-Host "Removed scheduled task $TaskName"
    } else {
        Write-Host "Task $TaskName is not registered"
    }
    exit 0
}

if ($Server -eq "") {
    $cmd = Get-Command fast_code_search_server.exe -ErrorAction SilentlyContinue
    if (-not $cmd) { throw "fast_code_search_server.exe not found on PATH; pass -Server <path>" }
    $Server = $cmd.Source
}
if (-not (Test-Path $Config)) {
    throw "Configuration not found at $Config. Create it with: fast_code_search_server --init `"$Config`""
}

$log = Join-Path (Split-Path $Config) "server.log"
# cmd.exe redirects the log; the task itself runs hidden.
$action = New-ScheduledTaskAction -Execute "cmd.exe" `
    -Argument "/c `"`"$Server`" --config `"$Config`" >> `"$log`" 2>&1`""
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
$settings = New-ScheduledTaskSettingsSet -Hidden -ExecutionTimeLimit ([TimeSpan]::Zero) `
    -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) `
    -StartWhenAvailable -MultipleInstances IgnoreNew
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Limited

Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger `
    -Settings $settings -Principal $principal -Force | Out-Null
Write-Host "Registered scheduled task $TaskName (runs at logon; log: $log)"
Write-Host "Start it now with: Start-ScheduledTask $TaskName"
