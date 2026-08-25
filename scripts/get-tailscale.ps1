$ErrorActionPreference = "Stop"

$bundleDir = Join-Path $PSScriptRoot "..\src-tauri\tailscale-bundle"
$tempRoot = Join-Path $env:TEMP ("taildesk-fetch-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tempRoot | Out-Null
New-Item -ItemType Directory -Path $bundleDir -Force | Out-Null

try {
    Write-Host "Recherche de la derniere version stable..."
    $feed = Invoke-RestMethod -Uri "https://pkgs.tailscale.com/stable/?mode=json" -UseBasicParsing
    $msiName = $feed.MSIs.amd64
    if (-not $msiName) {
        throw "Aucun MSI amd64 trouve dans le flux stable."
    }
    $version = $feed.MSIsVersion
    $url = "https://pkgs.tailscale.com/stable/$msiName"

    Write-Host "Version : $version"
    $msiPath = Join-Path $tempRoot $msiName
    Write-Host "Telechargement de $url ..."
    Invoke-WebRequest -Uri $url -OutFile $msiPath -UseBasicParsing

    $extractDir = Join-Path $tempRoot "extract"
    New-Item -ItemType Directory -Path $extractDir | Out-Null
    Write-Host "Extraction administrative du MSI..."
    $logPath = Join-Path $tempRoot "extract.log"
    $proc = Start-Process -FilePath "msiexec.exe" -ArgumentList "/a", "`"$msiPath`"", "/qn", "TARGETDIR=`"$extractDir`"", "/L*V", "`"$logPath`"" -Wait -PassThru
    if ($proc.ExitCode -ne 0) {
        throw "msiexec a echoue (code $($proc.ExitCode)), voir $logPath"
    }

    $wanted = @("tailscale.exe", "tailscaled.exe", "wintun.dll")
    $found = @{}
    Get-ChildItem -Path $extractDir -Recurse -File | ForEach-Object {
        if ($wanted -contains $_.Name.ToLower()) {
            $found[$_.Name.ToLower()] = $_.FullName
        }
    }
    foreach ($name in $wanted) {
        if (-not $found.ContainsKey($name)) {
            throw "$name introuvable dans l'extraction."
        }
        Copy-Item -Path $found[$name] -Destination (Join-Path $bundleDir $name) -Force
    }

    Set-Content -Path (Join-Path $bundleDir "VERSION") -Value $version
    $notice = @"
Tailscale $version
https://tailscale.com

These binaries come from the official Tailscale Windows MSI package.
The Tailscale open source client is licensed under the BSD 3-Clause License.
Copyright (c) Tailscale Inc & AUTHORS.
https://github.com/tailscale/tailscale/blob/main/LICENSE
"@
    Set-Content -Path (Join-Path $bundleDir "NOTICE.txt") -Value $notice

    Write-Host "Binaires installes dans $bundleDir :"
    Get-ChildItem $bundleDir | ForEach-Object {
        Write-Host ("  {0}  {1:N0} octets" -f $_.Name, $_.Length)
    }
}
finally {
    Remove-Item -Recurse -Force $tempRoot -ErrorAction SilentlyContinue
}
