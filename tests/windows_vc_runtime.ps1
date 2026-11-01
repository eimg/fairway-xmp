# Exercise prerequisite decisions without changing this machine's runtime.
. "$PSScriptRoot/../scripts/install_windows_vc_runtime.ps1"
$readInstalledVersion = ${function:Get-FairwayVcInstalledVersion}

function Get-FairwayVcInstallerVersion { return [version]'14.50.35719.0' }
function Get-FairwayVcInstalledVersion { return $script:Installed }
function Invoke-FairwayVcInstaller {
    $script:Calls++
    $script:Installed = $script:After
    return $script:Code
}

$cases = @(
    @{ Name = 'missing'; Before = $null; After = '14.50.35719.0'; Code = 0; Calls = 1 },
    @{ Name = 'older'; Before = '14.44.35211.0'; After = '14.50.35719.0'; Code = 0; Calls = 1 },
    @{ Name = 'equal'; Before = '14.50.35719.0'; Calls = 0 },
    @{ Name = 'newer'; Before = '14.51.36000.0'; Calls = 0 },
    @{ Name = 'reboot'; Before = $null; After = '14.50.35719.0'; Code = 3010; Calls = 1 },
    @{ Name = 'concurrent newer'; Before = $null; After = '14.51.36000.0'; Code = 1638; Calls = 1 },
    @{ Name = 'failed'; Before = $null; Code = 1603; Calls = 1; Fails = $true },
    @{ Name = 'false success'; Before = $null; Code = 0; Calls = 1; Fails = $true },
    @{ Name = 'still older'; Before = '14.44.35211.0'; After = '14.44.35211.0'; Code = 0; Calls = 1; Fails = $true },
    @{ Name = 'unverified conflict'; Before = $null; Code = 1638; Calls = 1; Fails = $true }
)
foreach ($case in $cases) {
    $script:Installed = if ($case.Before) { [version]$case.Before } else { $null }
    $script:After = if ($case.After) { [version]$case.After } else { $null }
    $script:Code = $case.Code
    $script:Calls = 0
    $failed = $false
    try { Install-FairwayVcRuntime 'verified-installer.exe' } catch { $failed = $true }
    if ($failed -ne [bool]$case.Fails -or $script:Calls -ne $case.Calls) {
        throw "VC++ prerequisite test failed: $($case.Name)"
    }
    Write-Host "Passed: $($case.Name)"
}

Set-Item Function:Get-FairwayVcInstalledVersion $readInstalledVersion
function Get-FairwayVcRegistryVersion([string]$View) { return $script:Views[$View] }
foreach ($views in @(
    @{ Registry64 = [version]'14.50.35719.0' },
    @{ Registry32 = [version]'14.50.35719.0' },
    @{ Registry64 = [version]'14.44.35211.0'; Registry32 = [version]'14.50.35719.0' }
)) {
    $script:Views = $views
    if ((Get-FairwayVcInstalledVersion) -ne [version]'14.50.35719.0') {
        throw 'Failed to detect the x64 runtime across registry views.'
    }
}
Write-Host 'Passed: x64 runtime detection in either registry view.'
