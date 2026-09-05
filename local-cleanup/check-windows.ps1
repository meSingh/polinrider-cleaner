<#
.SYNOPSIS
  PolinRider check and cleanup for Windows.

.DESCRIPTION
  Default is a dry run: it inspects and reports, and changes nothing.
  -Apply moves confirmed artifacts into a quarantine directory. It never deletes.
  Requires Windows PowerShell 5.1, which ships with Windows 10 and 11. Nothing to install.

.PARAMETER Roots
  Directories holding your code. Defaults to source, repos, code, dev, projects,
  Documents under your user profile. Give the real ones, the scan is only as good
  as its roots.

.PARAMETER Apply
  Move confirmed artifacts to quarantine instead of only reporting them.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\check-windows.ps1
  powershell -ExecutionPolicy Bypass -File .\check-windows.ps1 -Roots C:\work -Apply

.NOTES
  Exit codes: 0 clean, 1 review items only, 2 confirmed indicator hit.
#>
[CmdletBinding()]
param(
  [string[]]$Roots,
  [switch]$Apply,
  [string]$Quarantine,
  [string]$Report
)

$ErrorActionPreference = 'Continue'
$ts = (Get-Date).ToUniversalTime().ToString('yyyyMMddTHHmmssZ')
if (-not $Quarantine) { $Quarantine = Join-Path $env:USERPROFILE "polinrider-quarantine-$ts" }
if (-not $Report)     { $Report     = Join-Path $env:USERPROFILE "polinrider-report-$ts.txt" }
if (-not $Roots) {
  $Roots = @('source','repos','code','dev','projects','Documents') |
           ForEach-Object { Join-Path $env:USERPROFILE $_ }
}

$script:Hits   = 0
$script:Review = 0

function Say  ($m) { Write-Host $m;                Add-Content -Path $Report -Value $m }
function Hdr  ($m) { Say ""; Say "== $m ==" }
function Bad  ($m) { $script:Hits++;   Say "  [HIT]    $m" }
function Warn ($m) { $script:Review++; Say "  [review] $m" }
function Ok   ($m) { Say "  [ok]     $m" }

# --- indicators ------------------------------------------------------------
$iocDir = Join-Path (Split-Path -Parent $PSScriptRoot) 'ioc'
function Read-Iocs ([string[]]$files) {
  $out = @()
  foreach ($f in $files) {
    $p = Join-Path $iocDir $f
    if (-not (Test-Path $p)) { Write-Error "indicator file not found: $p"; exit 2 }
    $out += Get-Content $p | Where-Object { $_ -and $_ -notmatch '^\s*#' }
  }
  return $out
}
$Strong   = Read-Iocs @('strong.txt','bad-packages.txt')
$Weak     = Read-Iocs @('weak.txt')
$BadPkgs  = Read-Iocs @('bad-packages.txt')
$Net      = Read-Iocs @('network.txt')
if ($Strong.Count -eq 0) { Write-Error "indicator set is empty"; exit 2 }

function Test-Strong ([string]$path) {
  try { return [bool](Select-String -Path $path -SimpleMatch -Pattern $Strong -List -ErrorAction SilentlyContinue) }
  catch { return $false }
}
function Test-Weak ([string]$path) {
  try { return [bool](Select-String -Path $path -SimpleMatch -Pattern $Weak -List -ErrorAction SilentlyContinue) }
  catch { return $false }
}

# --- quarantine ------------------------------------------------------------
if ($Apply) {
  New-Item -ItemType Directory -Force -Path (Join-Path $Quarantine 'files') | Out-Null
  Set-Content -Path (Join-Path $Quarantine 'manifest.tsv') -Value "original_path`tquarantined_path`treason"
  Set-Content -Path (Join-Path $Quarantine 'RESTORE.txt') -Value @'
Nothing here was deleted. To put a file back, read manifest.tsv and move each
quarantined_path back to its original_path. Keep this directory until the
incident is closed. It is evidence.
'@
}
function Move-ToQuarantine ([string]$src, [string]$reason) {
  if (-not $Apply) { Say "           would quarantine: $src"; return }
  $rel  = $src -replace '^[A-Za-z]:\\', ''
  $dest = Join-Path (Join-Path $Quarantine 'files') $rel
  try {
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $dest) | Out-Null
    Move-Item -LiteralPath $src -Destination $dest -Force
    Add-Content -Path (Join-Path $Quarantine 'manifest.tsv') -Value ("{0}`t{1}`t{2}" -f $src, $dest, $reason)
    Say "           quarantined -> $dest"
  } catch {
    Say "           QUARANTINE FAILED (check permissions): $src"
  }
}

Set-Content -Path $Report -Value ""
Say ("PolinRider local check - Windows - " + (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))
Say ("host: $env:COMPUTERNAME   user: $env:USERNAME")
Say ("roots: " + ($Roots -join ', '))
if ($Apply) { Say "mode: APPLY - confirmed artifacts will be moved to quarantine" }
else        { Say "mode: dry run - nothing will be changed" }

# --- 1. IDE extensions -----------------------------------------------------
Hdr "IDE extensions"
$extRoots = @('.vscode\extensions','.vscode-insiders\extensions','.cursor\extensions',
              '.windsurf\extensions','.vscode-oss\extensions') |
            ForEach-Object { Join-Path $env:USERPROFILE $_ }
$anyExt = $false
foreach ($d in $extRoots) {
  if (-not (Test-Path $d)) { continue }
  $anyExt = $true
  Get-ChildItem -Path $d -Directory -ErrorAction SilentlyContinue | ForEach-Object {
    $ext = $_
    $hit = Get-ChildItem -Path $ext.FullName -Recurse -File -ErrorAction SilentlyContinue |
           Where-Object { $_.Length -lt 20MB } |
           Where-Object { Test-Strong $_.FullName } | Select-Object -First 1
    if ($hit) {
      Bad "extension contains an indicator: $($ext.FullName)"
      Move-ToQuarantine $ext.FullName 'ide-extension'
    }
  }
}
if (-not $anyExt) { Ok "no IDE extension directories found" }
Say ""
Say "  Extensions installed or updated in the last 60 days, review by hand:"
foreach ($d in $extRoots) {
  if (-not (Test-Path $d)) { continue }
  Get-ChildItem -Path $d -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.LastWriteTime -gt (Get-Date).AddDays(-60) } |
    ForEach-Object { Say ("    " + $_.Name) }
}

# --- 2. tasks.json ---------------------------------------------------------
Hdr "Workspace tasks that run on folder open"
foreach ($root in $Roots) {
  if (-not (Test-Path $root)) { continue }
  Get-ChildItem -Path $root -Recurse -Filter 'tasks.json' -File -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like '*\.vscode\*' -and $_.FullName -notlike '*\node_modules\*' } |
    ForEach-Object {
      if (Select-String -Path $_.FullName -SimpleMatch -Pattern 'folderOpen' -Quiet -ErrorAction SilentlyContinue) {
        if (Test-Strong $_.FullName) {
          Bad "tasks.json runs on folder open AND contains an indicator: $($_.FullName)"
          Move-ToQuarantine $_.FullName 'malicious-tasks-json'
        } else {
          Warn "tasks.json runs on folder open, verify the command by hand: $($_.FullName)"
        }
      }
    }
}

# --- 3. build configs ------------------------------------------------------
Hdr "Build configs with code after the module end"
$cfgNames = @('postcss.config.*','tailwind.config.*','eslint.config.*','vite.config.*',
              'next.config.*','rollup.config.*','webpack.config.*','babel.config.*')
foreach ($root in $Roots) {
  if (-not (Test-Path $root)) { continue }
  foreach ($pattern in $cfgNames) {
    Get-ChildItem -Path $root -Recurse -Filter $pattern -File -ErrorAction SilentlyContinue |
      Where-Object { $_.FullName -notlike '*\node_modules\*' -and $_.FullName -notlike '*\.git\*' } |
      ForEach-Object {
        $f = $_.FullName
        if (Test-Strong $f) {
          Bad "config file contains an indicator: $f"
          Say "           do not edit this file. Delete the clone and re-clone after the remote is clean."
        } else {
          $lines = @(Get-Content -LiteralPath $f -ErrorAction SilentlyContinue)
          $end = 0
          for ($i = 0; $i -lt $lines.Count; $i++) {
            if ($lines[$i] -match '^(export default|module\.exports)') { $end = $i + 1 }
          }
          if ($end -gt 0 -and $lines.Count -gt ($end + 15)) {
            Warn "content after module end ($($lines.Count) lines, module ends at $end): $f"
          }
          if ($lines | Where-Object { $_.Length -gt 4000 }) {
            Warn "line longer than 4000 characters, an obfuscation tell: $f"
          }
        }
      }
  }
}

# --- 4. font masquerade ----------------------------------------------------
Hdr "Font files that are not fonts"
function Get-Magic ([string]$path) {
  try {
    $fs = [System.IO.File]::OpenRead($path)
    $buf = New-Object byte[] 4
    $n = $fs.Read($buf, 0, 4)
    $fs.Close()
    if ($n -lt 4) { return '' }
    return [System.Text.Encoding]::ASCII.GetString($buf)
  } catch { return '' }
}
foreach ($root in $Roots) {
  if (-not (Test-Path $root)) { continue }
  Get-ChildItem -Path $root -Recurse -File -Include '*.woff','*.woff2' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -notlike '*\node_modules\*' -and $_.FullName -notlike '*\.git\*' -and $_.Length -gt 0 } |
    ForEach-Object {
      $magic = Get-Magic $_.FullName
      if ($magic -ne 'wOFF' -and $magic -ne 'wOF2' -and $magic -ne 'vers') {
        Bad "font file is not a font (magic='$magic'): $($_.FullName)"
        Move-ToQuarantine $_.FullName 'font-masquerade'
      }
    }
}

# --- 5. propagation artifact ----------------------------------------------
Hdr "Propagation artifact temp_auto_push.bat"
# Searching the whole user profile walks AppData and takes minutes. The script
# only ever lands in a working copy, so search the code roots plus the folders
# people actually clone into.
$found = $false
$searchRoots = @($Roots) + @('Desktop','Downloads','Documents' | ForEach-Object { Join-Path $env:USERPROFILE $_ })
foreach ($r in ($searchRoots | Select-Object -Unique)) {
  if (-not (Test-Path $r)) { continue }
  Get-ChildItem -Path $r -Recurse -Filter 'temp_auto_push.bat' -File -ErrorAction SilentlyContinue |
    Select-Object -First 20 | ForEach-Object {
      $found = $true
      Bad "propagation script present: $($_.FullName)"
      Move-ToQuarantine $_.FullName 'propagation-script'
    }
}
if (-not $found) { Ok "temp_auto_push.bat not found under the scanned roots" }

# --- 6. known-bad packages -------------------------------------------------
Hdr "Known-bad packages"
$pkgFound = $false
foreach ($root in $Roots) {
  if (-not (Test-Path $root)) { continue }
  Get-ChildItem -Path $root -Recurse -File -Include 'package.json','package-lock.json','pnpm-lock.yaml','yarn.lock' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -notlike '*\node_modules\*' } |
    ForEach-Object {
      if (Select-String -Path $_.FullName -SimpleMatch -Pattern $BadPkgs -List -ErrorAction SilentlyContinue) {
        $pkgFound = $true
        Bad "known-bad package referenced: $($_.FullName)"
        Say "           remove the dependency, delete node_modules and the lockfile entry, reinstall."
      }
    }
}
if (-not $pkgFound) { Ok "no known-bad package names in manifests or lockfiles" }

# --- 7. persistence --------------------------------------------------------
Hdr "Persistence: Run keys, Startup folder, scheduled tasks"
$runKeys = @('HKCU:\Software\Microsoft\Windows\CurrentVersion\Run',
             'HKCU:\Software\Microsoft\Windows\CurrentVersion\RunOnce',
             'HKLM:\Software\Microsoft\Windows\CurrentVersion\Run',
             'HKLM:\Software\Microsoft\Windows\CurrentVersion\RunOnce')
foreach ($k in $runKeys) {
  if (-not (Test-Path $k)) { continue }
  $props = Get-ItemProperty -Path $k -ErrorAction SilentlyContinue
  foreach ($p in $props.PSObject.Properties) {
    if ($p.Name -like 'PS*') { continue }
    $val = [string]$p.Value
    $isStrong = $false
    foreach ($s in $Strong) { if ($val -like "*$s*") { $isStrong = $true; break } }
    if ($isStrong) {
      Bad "run key entry contains an indicator: $k\$($p.Name) = $val"
      Say "           remove it: Remove-ItemProperty -Path '$k' -Name '$($p.Name)'"
    } elseif ($val -match '(curl|wget|powershell|node|mshta|certutil).*(http|-enc|iex)') {
      Warn "run key entry runs a network or interpreter command: $k\$($p.Name) = $val"
    }
  }
}
$startup = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Startup'
if (Test-Path $startup) {
  Get-ChildItem -Path $startup -File -ErrorAction SilentlyContinue | ForEach-Object {
    if (Test-Strong $_.FullName) {
      Bad "startup item contains an indicator: $($_.FullName)"
      Move-ToQuarantine $_.FullName 'startup-item'
    } else {
      Warn "startup item present, verify by hand: $($_.FullName)"
    }
  }
}
try {
  Get-ScheduledTask -ErrorAction Stop | Where-Object { $_.TaskPath -notlike '\Microsoft\*' } | ForEach-Object {
    $acts = ($_.Actions | ForEach-Object { "$($_.Execute) $($_.Arguments)" }) -join ' '
    $isStrong = $false
    foreach ($s in $Strong) { if ($acts -like "*$s*") { $isStrong = $true; break } }
    if ($isStrong) {
      Bad "scheduled task contains an indicator: $($_.TaskPath)$($_.TaskName)"
      Say "           remove it: Unregister-ScheduledTask -TaskName '$($_.TaskName)' -TaskPath '$($_.TaskPath)'"
    } elseif ($acts -match '(curl|wget|powershell|node|mshta|certutil).*(http|-enc|iex)') {
      Warn "scheduled task runs a network or interpreter command: $($_.TaskPath)$($_.TaskName)"
    }
  }
} catch { Warn "could not enumerate scheduled tasks (needs Windows 8 or later)" }

# --- 8. PowerShell profiles ------------------------------------------------
Hdr "PowerShell profiles"
foreach ($p in @($PROFILE.AllUsersAllHosts, $PROFILE.AllUsersCurrentHost,
                 $PROFILE.CurrentUserAllHosts, $PROFILE.CurrentUserCurrentHost)) {
  if (-not $p -or -not (Test-Path $p)) { continue }
  if (Test-Strong $p) {
    Bad "PowerShell profile contains an indicator: $p"
    Say "           edit it by hand and remove the line. Profiles are never quarantined."
  } elseif (Select-String -Path $p -Pattern '(iex|Invoke-Expression).*(http|DownloadString)' -Quiet -ErrorAction SilentlyContinue) {
    Bad "PowerShell profile downloads and executes code: $p"
  } else {
    Ok "clean: $p"
  }
}

# --- 9. git configuration and hooks ---------------------------------------
Hdr "Git configuration and hooks"
if (Get-Command git -ErrorAction SilentlyContinue) {
  $hooksPath = (& git config --global core.hooksPath 2>$null)
  if ($hooksPath) { Warn "global core.hooksPath is set to: $hooksPath" } else { Ok "no global core.hooksPath" }
  (& git config --global --list 2>$null) |
    Where-Object { $_ -match '(url\..*insteadof|http\..*proxy|credential\.helper)' } |
    ForEach-Object { Say ("    " + $_) }
} else {
  Warn "git is not on PATH, global git config checks skipped"
}
foreach ($root in $Roots) {
  if (-not (Test-Path $root)) { continue }
  Get-ChildItem -Path $root -Recurse -Directory -Filter 'hooks' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -like '*\.git\hooks' } | ForEach-Object {
      Get-ChildItem -Path $_.FullName -File -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -notlike '*.sample' } | ForEach-Object {
          if (Test-Strong $_.FullName) {
            Bad "git hook contains an indicator: $($_.FullName)"
            Move-ToQuarantine $_.FullName 'git-hook'
          } else {
            Warn "active git hook, verify by hand: $($_.FullName)"
          }
        }
    }
}

# --- 10. npm ---------------------------------------------------------------
Hdr "npm configuration"
$npmrc = Join-Path $env:USERPROFILE '.npmrc'
if (Test-Path $npmrc) {
  Say "  ~\.npmrc, tokens redacted:"
  Get-Content $npmrc | ForEach-Object { Say ("    " + ($_ -replace '(_authToken|_auth|_password)=.*', '$1=<REDACTED-ROTATE-THIS>')) }
  if (Select-String -Path $npmrc -SimpleMatch -Pattern '_authToken' -Quiet -ErrorAction SilentlyContinue) {
    Warn "an npm auth token is stored on disk. Rotate it regardless of this scan."
  }
  $reg = Select-String -Path $npmrc -Pattern '^registry=' -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($reg -and ($reg.Line -notmatch 'registry\.npmjs\.org')) { Bad "a non-default npm registry is configured" }
} else { Ok "no ~\.npmrc" }
if (Get-Command npm -ErrorAction SilentlyContinue) {
  $ign = (& npm config get ignore-scripts 2>$null)
  if ($ign -eq 'true') { Ok "npm ignore-scripts is on" }
  elseif ($ign)        { Warn "npm ignore-scripts is '$ign'. Recommended: npm config set ignore-scripts true" }
}

# --- 11. live connections --------------------------------------------------
Hdr "Live connections from node and Electron processes"
try {
  $conns = Get-NetTCPConnection -State Established -ErrorAction Stop | ForEach-Object {
    $proc = Get-Process -Id $_.OwningProcess -ErrorAction SilentlyContinue
    if ($proc -and $proc.ProcessName -match '(node|Code|Cursor|electron)') {
      "{0,-22} {1}:{2}" -f $proc.ProcessName, $_.RemoteAddress, $_.RemotePort
    }
  }
  if ($conns) {
    $conns | Select-Object -First 40 | ForEach-Object { Say ("    " + $_) }
    $joined = $conns -join ' '
    $onC2 = $false
    foreach ($n in $Net) { if ($joined -like "*$n*") { $onC2 = $true; break } }
    if ($onC2) { Bad "live connection to known campaign infrastructure" }
    else       { Ok  "no known campaign host or address in current connections" }
  }
  else { Ok "no established node or Electron TCP connections right now" }
} catch { Warn "could not enumerate TCP connections, skipped" }

# --- 11b. resident interpreters running inline code ------------------------
Hdr "Resident interpreters running inline code"
try {
  $inline = Get-CimInstance Win32_Process -ErrorAction Stop |
            Where-Object { $_.Name -match '^(node|python[0-9.]*)\.exe$' -and $_.CommandLine -match '\s-(e|c)\s' } |
            Select-Object -First 20
  if ($inline) {
    $inline | ForEach-Object {
      $c = $_.CommandLine; if ($c.Length -gt 200) { $c = $c.Substring(0,200) }
      Say ("    " + $_.ProcessId + " " + $c)
    }
    Warn "an interpreter is running code passed on the command line. Read each one."
  } else {
    Ok "no interpreter running inline code right now"
  }
} catch { Warn "could not enumerate processes, skipped" }

# --- 12. credential surface -----------------------------------------------
Hdr "Credential surface on this machine"
$credPaths = @('.ssh','.aws\credentials','.docker\config.json','.kube\config','_netrc','.netrc') |
             ForEach-Object { Join-Path $env:USERPROFILE $_ }
foreach ($c in $credPaths) { if (Test-Path $c) { Warn "credential material present, rotate as a precaution: $c" } }
$envCount = 0
foreach ($root in $Roots) {
  if (-not (Test-Path $root)) { continue }
  $envCount += @(Get-ChildItem -Path $root -Recurse -File -Filter '.env*' -ErrorAction SilentlyContinue |
                 Where-Object { $_.FullName -notlike '*\node_modules\*' }).Count
}
if ($envCount -gt 0) { Warn "$envCount .env files under the scanned roots. If anything above is a HIT, treat every value in them as public." }

# --- verdict ---------------------------------------------------------------
Hdr "RESULT"
Say "  confirmed indicator hits : $script:Hits"
Say "  review items             : $script:Review"
Say "  report                   : $Report"
if ($Apply) { Say "  quarantine               : $Quarantine" }
Say ""
if ($script:Hits -gt 0) {
  Say "VERDICT: COMPROMISED."
  Say "  Quarantining artifacts does not make this machine trustworthy again. The"
  Say "  payload is a remote access trojan and an infostealer, so assume every"
  Say "  credential reachable from this user account has been taken."
  Say "  1. Disconnect from the network."
  Say "  2. Rotate every credential in the section above, from a different machine."
  Say "  3. Rebuild from a clean Windows install. Do not restore a backup taken"
  Say "     after the infection date."
  Say "  4. Delete every local clone. Re-clone only after the remote is verified clean."
  exit 2
} elseif ($script:Review -gt 0) {
  Say "VERDICT: no confirmed indicator. $script:Review items need a human look."
  Say "  A clean result proves the current indicator set is absent. It does not"
  Say "  prove an older or rotated variant was never here. Rotate your GitHub"
  Say "  tokens, SSH keys and cloud keys anyway."
  exit 1
} else {
  Say "VERDICT: clean against the current indicator set."
  Say "  Rotate credentials anyway. Yours may have been taken from a different"
  Say "  machine or from a shared secret store."
  exit 0
}
