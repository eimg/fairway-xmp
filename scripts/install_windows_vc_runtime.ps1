param([string]$Installer)

$ErrorActionPreference = 'Stop'

function Get-FairwayVcRegistryVersion([Microsoft.Win32.RegistryView]$View) {
    $key = [Microsoft.Win32.RegistryKey]::OpenBaseKey('LocalMachine', $View)
    try {
        $runtime = $key.OpenSubKey('SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64')
        if ($null -eq $runtime) { return $null }
        try {
            if ($runtime.GetValue('Installed') -ne 1) { return $null }
            $value = [string]$runtime.GetValue('Version')
            if ($value -notmatch '^v?(\d+\.\d+\.\d+\.\d+)$') { return $null }
            return [version]$Matches[1]
        } finally { $runtime.Dispose() }
    } finally { $key.Dispose() }
}

function Get-FairwayVcInstalledVersion {
    $installed = $null
    # Redistributable generations have used both registry views for x64 metadata.
    foreach ($view in 'Registry64', 'Registry32') {
        $version = Get-FairwayVcRegistryVersion $view
        if ($null -ne $version -and ($null -eq $installed -or $version -gt $installed)) {
            $installed = $version
        }
    }
    return $installed
}

function Get-FairwayVcInstallerVersion([string]$Path) {
    $info = (Get-Item -LiteralPath $Path).VersionInfo
    if ($info.FileMajorPart -ne 14) { throw 'Expected a Microsoft Visual C++ v14 installer.' }
    return [version]::new($info.FileMajorPart, $info.FileMinorPart, $info.FileBuildPart, $info.FilePrivatePart)
}

function Invoke-FairwayVcInstaller([string]$Path) {
    $process = Start-Process -FilePath $Path -ArgumentList '/install', '/quiet', '/norestart' -Wait -PassThru
    return $process.ExitCode
}

function Install-FairwayVcRuntime([string]$Path) {
    $required = Get-FairwayVcInstallerVersion $Path
    $installed = Get-FairwayVcInstalledVersion
    if ($null -ne $installed -and $installed -ge $required) {
        Write-Host "Visual C++ x64 $installed already satisfies bundled $required."
        return
    }
    $code = Invoke-FairwayVcInstaller $Path
    # 1638 is only acceptable if another installer supplied a sufficient version.
    if ($code -notin 0, 3010, 1638) { throw "Visual C++ runtime installation failed ($code)." }
    $installed = Get-FairwayVcInstalledVersion
    if ($null -eq $installed -or $installed -lt $required) {
        throw "Visual C++ runtime $required is still missing or outdated after installation ($code)."
    }
    if ($code -eq 3010) { Write-Warning 'Visual C++ requested a reboot; PHP verification must still pass on this runner.' }
    Write-Host "Visual C++ x64 $installed is ready."
}

if ($MyInvocation.InvocationName -ne '.') {
    if (-not $Installer) { throw 'Pass the SHA256-verified bundled installer with -Installer.' }
    Install-FairwayVcRuntime $Installer
}
