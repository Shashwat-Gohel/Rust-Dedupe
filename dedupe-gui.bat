<# ::
@echo off
setlocal
cd /d "%~dp0"
set "_src=%~f0"
set "_root=%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$src='%_src%'; $rootDir='%_root%'.TrimEnd('\'); $enc=New-Object System.Text.UTF8Encoding($false); $txt=[System.IO.File]::ReadAllText($src,$enc); $inject='$Script:RealRoot = ''' + $rootDir + ''''; $txt=$inject+[Environment]::NewLine+$txt; $tmp=[System.IO.Path]::GetTempFileName()+'.ps1'; [System.IO.File]::WriteAllText($tmp,$txt,$enc); & powershell -NoProfile -ExecutionPolicy Bypass -File $tmp; Remove-Item $tmp -Force"
endlocal
exit /b 0
#>
# =============================================================================
#  dedupe TUI  --  Interactive Terminal Interface
#  Single .bat file  (PowerShell hybrid: batch bootstraps into PowerShell)
# =============================================================================

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName PresentationFramework

# -------------------------------------------------------------------------------
#  CONFIGURATION
# -------------------------------------------------------------------------------
# $Script:RealRoot is injected by the .bat bootstrap as the directory of the .bat
if (-not $Script:RealRoot) { $Script:RealRoot = $PSScriptRoot }
$Script:DEDUPE  = Join-Path $Script:RealRoot "bin\dedupe.exe"
$Script:SnapDir = Join-Path $Script:RealRoot "snapshots"


# -------------------------------------------------------------------------------
#  ANSI COLOUR HELPERS
# -------------------------------------------------------------------------------
$ESC = [char]27
function e($c) { "${ESC}[${c}m" }

$RESET  = e "0"
$BOLD   = e "1"
$DIM    = e "2"

$FG = @{
    White   = e "97"
    Cyan    = e "96"
    Yellow  = e "93"
    Green   = e "92"
    Red     = e "91"
    Blue    = e "94"
    Magenta = e "95"
    Gray    = e "90"
    Orange  = e "38;5;214"
    Purple  = e "38;5;135"
    Teal    = e "38;5;43"
    Pink    = e "38;5;211"
    Black   = e "30"
}

$BG = @{
    Teal    = e "48;5;23"
    DkGray  = e "48;5;236"
}

function Clr($fg, $text, $bg = $null) {
    if ($bg) { "${fg}${bg}${text}${RESET}" } else { "${fg}${text}${RESET}" }
}

# -------------------------------------------------------------------------------
#  TERMINAL HELPERS
# -------------------------------------------------------------------------------
function Clear-Screen   { [Console]::Clear() }
function W($t = "")     { [Console]::Write($t) }
function WL($t = "")    { [Console]::WriteLine($t) }

function Center($text, $width = $Host.UI.RawUI.BufferSize.Width) {
    $plain = $text -replace [char]27 + '\[[0-9;]*m', ''
    $pad   = [Math]::Max(0, [int](($width - $plain.Length) / 2))
    (" " * $pad) + $text
}

function HRule($ch = "-", $colour = $FG.Gray) {
    $w = $Host.UI.RawUI.BufferSize.Width - 2
    WL "${colour}$($ch * $w)${RESET}"
}

function Cursor($row, $col) { [Console]::SetCursorPosition($col, $row) }

# -------------------------------------------------------------------------------
#  FIGLET ENGINE  (Convert text to FIGlet ASCII font)
# -------------------------------------------------------------------------------
$Script:FigletGlyphs = @{
    'A' = @("    ___   ", "   /   |  ", "  / /| |  ", " / ___ |  ", "/_/  |_|  ")
    'B' = @("   ____   ", "  / __ )  ", " / __  |  ", "/ /_/ /   ", "/_____/   ")
    'C' = @("   ______ ", "  / ____/ ", " / /      ", "/ /___    ", "\____/    ")
    'D' = @("    ____  ", "   / __ \ ", "  / / / / ", " / /_/ /  ", "/_____/   ")
    'E' = @("    ______", "   / ____/", "  / __/   ", " / /___   ", "/_____/   ")
    'F' = @("    ______", "   / ____/", "  / /_    ", " / __/    ", "/_/       ")
    'G' = @("   ______ ", "  / ____/ ", " / / __   ", "/ /_/ /   ", "\____/    ")
    'H' = @("    __  __", "   / / / /", "  / /_/ / ", " / __  /  ", "/_/ /_/   ")
    'I' = @("    ____", "   /  _/", "   / /  ", " _/ /   ", "/___/   ")
    'J' = @("       __ ", "      / / ", " __  / /  ", "/ /_/ /   ", "\____/    ")
    'K' = @("    __ __ ", "   / //_/ ", "  / ,<    ", " / /| |   ", "/_/ |_|   ")
    'L' = @("    __    ", "   / /    ", "  / /     ", " / /___   ", "/_____/   ")
    'M' = @("    __  ___", "   /  |/  /", "  / /|_/ / ", " / /  / /  ", "/_/  /_/   ")
    'N' = @("    _  __ ", "   / |/ / ", "  /    /  ", " / /|  /  ", "/_/ |_/   ")
    'O' = @("   ____   ", "  / __ \  ", " / / / /  ", "/ /_/ /   ", "\____/    ")
    'P' = @("    ____  ", "   / __ \ ", "  / /_/ / ", " / ____/  ", "/_/       ")
    'Q' = @("   ____   ", "  / __ \  ", " / / / /  ", "/ /_/ /   ", "\___\_\   ")
    'R' = @("    ____  ", "   / __ \ ", "  / /_/ / ", " / _, _/  ", "/_/ |_|   ")
    'S' = @("   _____  ", "  / ___/  ", "  \__ \   ", " ___/ /   ", "/____/    ")
    'T' = @("   ______ ", "  /_  __/ ", "   / /    ", "  / /     ", " /_/      ")
    'U' = @("   __  __ ", "  / / / / ", " / / / /  ", "/ /_/ /   ", "\____/    ")
    'V' = @("   _   __ ", "  | | / / ", "  | |/ /  ", "  |  /    ", "  |_/     ")
    'W' = @("   _      __", "  | | /| / /", "  | |/ |/ / ", "  |__/|__/  ", "            ")
    'X' = @("   _  __  ", "  | |/ /  ", "  | ' /   ", " / /| |   ", "/_/ |_|   ")
    'Y' = @("   __  __ ", "  \ \/ /  ", "   \  /   ", "   / /    ", "  /_/     ")
    'Z' = @("   _____  ", "  /__  /  ", "    / /   ", "   / /___ ", "  /_____/ ")
    ' ' = @("   ", "   ", "   ", "   ", "   ")
}

function Convert-ToFiglet([string]$text) {
    $text = $text.ToUpper()
    $lines = @("", "", "", "", "")
    for ($i = 0; $i -lt $text.Length; $i++) {
        $sKey = [string]$text[$i]
        $glyph = if ($Script:FigletGlyphs.ContainsKey($sKey)) { $Script:FigletGlyphs[$sKey] } else { $Script:FigletGlyphs[' '] }
        for ($row = 0; $row -lt 5; $row++) {
            $lines[$row] += $glyph[$row]
        }
    }
    return $lines
}

# -------------------------------------------------------------------------------
#  BANNER  (dynamic FIGlet header)
# -------------------------------------------------------------------------------
function Show-Banner($titleText = "DEDUPE") {
    $w = $Host.UI.RawUI.BufferSize.Width
    WL ""
    $art = Convert-ToFiglet $titleText
    for ($i = 0; $i -lt $art.Length; $i++) {
        WL (Center "$($FG.Cyan)${BOLD}$($art[$i])${RESET}" $w)
    }

    WL ""
    WL (Center "$($FG.Gray)${DIM}High-Performance Disk Audit & Deduplication Engine  |  Rust / blake3${RESET}" $w)
    WL ""
}

# -------------------------------------------------------------------------------
#  WINDOWS FOLDER PICKER
# -------------------------------------------------------------------------------
function Pick-Folder($prompt = "Select a folder") {
    $userProfile = [System.Environment]::GetFolderPath([System.Environment+SpecialFolder]::UserProfile)
    $desktopPath = [System.Environment]::GetFolderPath([System.Environment+SpecialFolder]::Desktop)

    $dlg = New-Object System.Windows.Forms.FolderBrowserDialog
    $dlg.Description = $prompt
    $dlg.ShowNewFolderButton = $true
    # Set RootFolder to Desktop so Desktop, Downloads, User Profile, Drives & Network are all accessible
    $dlg.RootFolder = [System.Environment+SpecialFolder]::Desktop
    if (Test-Path $desktopPath) {
        $dlg.SelectedPath = $desktopPath
    } elseif (Test-Path $userProfile) {
        $dlg.SelectedPath = $userProfile
    }

    $form = New-Object System.Windows.Forms.Form
    $form.TopMost = $true
    $form.StartPosition = [System.Windows.Forms.FormStartPosition]::CenterScreen
    $form.Size = New-Object System.Drawing.Size(1, 1)
    $form.ShowInTaskbar = $false

    $r = $dlg.ShowDialog($form)
    $form.Dispose()

    if ($r -eq [System.Windows.Forms.DialogResult]::OK -and [string]::IsNullOrWhiteSpace($dlg.SelectedPath) -eq $false) {
        return $dlg.SelectedPath
    }

    # Fallback: allow manual path input if GUI window was closed or cancelled
    WL "  $($FG.Yellow)No folder selected in dialog window.${RESET}"
    $manual = Read-Prompt "Type or paste folder path manually (or ENTER to cancel)" ""
    if (-not [string]::IsNullOrWhiteSpace($manual) -and (Test-Path $manual)) {
        return (Resolve-Path $manual).ProviderPath
    }
    return $null
}

# -------------------------------------------------------------------------------
#  WINDOWS FILE OPEN PICKER
# -------------------------------------------------------------------------------
function Pick-File($prompt = "Select a file", $initDir = $null) {
    if (-not $initDir -or -not (Test-Path $initDir)) { $initDir = $Script:SnapDir }
    if (-not (Test-Path $initDir)) { $initDir = $Script:RealRoot }

    $dlg = New-Object System.Windows.Forms.OpenFileDialog
    $dlg.Title  = $prompt
    $dlg.Filter = "JSON files (*.json)|*.json|All files (*.*)|*.*"
    $dlg.InitialDirectory = $initDir

    $form = New-Object System.Windows.Forms.Form
    $form.TopMost = $true
    $form.StartPosition = [System.Windows.Forms.FormStartPosition]::CenterScreen
    $form.Size = New-Object System.Drawing.Size(1, 1)
    $form.ShowInTaskbar = $false

    $r = $dlg.ShowDialog($form)
    $form.Dispose()
    if ($r -eq [System.Windows.Forms.DialogResult]::OK -and [string]::IsNullOrWhiteSpace($dlg.FileName) -eq $false) {
        return $dlg.FileName
    }

    WL "  $($FG.Yellow)No file selected in dialog window.${RESET}"
    $manual = Read-Prompt "Type or paste file path manually (or ENTER to cancel)" ""
    if (-not [string]::IsNullOrWhiteSpace($manual) -and (Test-Path $manual)) {
        return (Resolve-Path $manual).ProviderPath
    }
    return $null
}

# -------------------------------------------------------------------------------
#  WINDOWS FILE SAVE PICKER
# -------------------------------------------------------------------------------
function Pick-SaveFile($prompt = "Save as", $defaultName = "snapshot.json") {
    if (-not (Test-Path $Script:SnapDir)) {
        New-Item -ItemType Directory -Path $Script:SnapDir -Force | Out-Null
    }
    $dlg = New-Object System.Windows.Forms.SaveFileDialog
    $dlg.Title = $prompt
    $dlg.Filter = "JSON files (*.json)|*.json"
    $dlg.FileName = $defaultName
    $dlg.InitialDirectory = $Script:SnapDir

    $form = New-Object System.Windows.Forms.Form
    $form.TopMost = $true
    $form.StartPosition = [System.Windows.Forms.FormStartPosition]::CenterScreen
    $form.Size = New-Object System.Drawing.Size(1, 1)
    $form.ShowInTaskbar = $false

    $r = $dlg.ShowDialog($form)
    $form.Dispose()
    if ($r -eq [System.Windows.Forms.DialogResult]::OK -and [string]::IsNullOrWhiteSpace($dlg.FileName) -eq $false) {
        return $dlg.FileName
    }

    WL "  $($FG.Yellow)No file path chosen in save dialog.${RESET}"
    $manual = Read-Prompt "Type or paste save path manually (or ENTER to cancel)" ""
    if (-not [string]::IsNullOrWhiteSpace($manual)) {
        return $manual
    }
    return $null
}

# -------------------------------------------------------------------------------
#  STATUS BAR
# -------------------------------------------------------------------------------
function StatusBar($msg = "Up/Down to navigate  |  ENTER to select  |  Q to quit") {
    $w = $Host.UI.RawUI.BufferSize.Width
    $h = $Host.UI.RawUI.WindowSize.Height
    Cursor ([Math]::Max(0, $h - 2)) 0
    W (Clr $FG.Black "  $msg  ".PadRight($w) $BG.Teal)
}

# -------------------------------------------------------------------------------
#  MENU RENDERER  (Sleek Boxed TUI)
# -------------------------------------------------------------------------------
function Show-Menu {
    param(
        [string]   $Title,
        [string[]] $Items,
        [string[]] $Icons = @(),
        [int]      $Selected = 0
    )

    Clear-Screen
    Show-Banner

    $termWidth = [Math]::Max(64, $Host.UI.RawUI.BufferSize.Width)
    $boxW      = 62
    $lpad      = [int](($termWidth - $boxW) / 2)
    if ($lpad -lt 0) { $lpad = 0 }
    $ind       = " " * $lpad

    $topBorder = "+" + ("=" * ($boxW - 2)) + "+"
    $midBorder = "+" + ("-" * ($boxW - 2)) + "+"
    $botBorder = "+" + ("=" * ($boxW - 2)) + "+"

    WL "${ind}$($FG.Cyan)${topBorder}${RESET}"
    $tText   = "   $Title   "
    $tPad    = [Math]::Max(0, [int](($boxW - 2 - $tText.Length) / 2))
    $tPadded = ((" " * $tPad) + $tText).PadRight($boxW - 2)
    WL "${ind}$($FG.Cyan)|$($FG.Yellow)${BOLD}${tPadded}$($FG.Cyan)|${RESET}"
    WL "${ind}$($FG.Cyan)${midBorder}${RESET}"

    for ($i = 0; $i -lt $Items.Count; $i++) {
        $ic = if ($i -lt $Icons.Count) { $Icons[$i] } else { "        " }
        $line = "  $ic  $($Items[$i])"
        $padded = $line.PadRight($boxW - 2)

        if ($i -eq $Selected) {
            $content = "$($BG.Teal)$($FG.White)${BOLD}$padded${RESET}"
            WL "${ind}$($FG.Cyan)|${content}$($FG.Cyan)|${RESET}"
        } else {
            $content = "$($FG.White)$padded${RESET}"
            WL "${ind}$($FG.Cyan)|${content}$($FG.Cyan)|${RESET}"
        }
    }

    WL "${ind}$($FG.Cyan)${botBorder}${RESET}"
    WL ""
    StatusBar
}

# -------------------------------------------------------------------------------
#  INTERACTIVE MENU LOOP
# -------------------------------------------------------------------------------
function Navigate-Menu {
    param(
        [string]   $Title,
        [string[]] $Items,
        [string[]] $Icons = @()
    )

    $sel = 0
    while ($true) {
        Show-Menu -Title $Title -Items $Items -Icons $Icons -Selected $sel
        $key = [Console]::ReadKey($true)
        switch ($key.Key) {
            "UpArrow"   { $sel = ($sel - 1 + $Items.Count) % $Items.Count }
            "DownArrow" { $sel = ($sel + 1) % $Items.Count }
            "Enter"     { return $sel }
            "Q"         { return -1  }
            "Escape"    { return -1  }
        }
    }
}

# -------------------------------------------------------------------------------
#  PAGER
# -------------------------------------------------------------------------------
function Show-Pager($lines, $title = "Output", [string[]]$extraHints = @()) {
    $visH  = $Host.UI.RawUI.WindowSize.Height - 12
    if ($visH -lt 3) { $visH = 3 }
    $top   = 0
    $tot   = $lines.Count
    $maxTop = [Math]::Max(0, $tot - $visH)
    $result = $null

    while ($true) {
        Clear-Screen
        Show-Banner
        WL (Center "$($FG.Yellow)${BOLD} $title ${RESET}")
        HRule

        $end = [Math]::Min($top + $visH - 1, $tot - 1)
        for ($i = $top; $i -le $end; $i++) { WL "  $($lines[$i])" }

        $blank = $visH - ($end - $top + 1)
        for ($b = 0; $b -lt $blank; $b++) { WL "" }

        WL ""
        HRule
        $pct = if ($maxTop -gt 0) { [int]($top * 100 / $maxTop) } else { 100 }
        $hints = "Up/Down to scroll  |  Q = back"
        if ($extraHints.Count -gt 0) { $hints += "  |  " + ($extraHints -join "  |  ") }
        WL "$($FG.Gray)${DIM}  [ $pct% ]  $hints${RESET}"
        StatusBar $hints

        $key = [Console]::ReadKey($true)
        switch ($key.Key) {
            "UpArrow"   { if ($top -gt 0)      { $top-- } }
            "DownArrow" { if ($top -lt $maxTop) { $top++ } }
            "Home"      { $top = 0 }
            "End"       { $top = $maxTop }
            "PageUp"    { $top = [Math]::Max(0, $top - $visH) }
            "PageDown"  { $top = [Math]::Min($maxTop, $top + $visH) }
            "Q"         { $result = 'BACK'; break }
            "Escape"    { $result = 'BACK'; break }
            default {
                $ch = $key.KeyChar
                if ($ch -ne [char]0) { $result = [string]$ch.ToString().ToUpper(); break }
            }
        }
        if ($null -ne $result) { return $result }
    }
}

# -------------------------------------------------------------------------------
#  TEXT INPUT
# -------------------------------------------------------------------------------
function Read-Prompt($label, $default = "") {
    if ($default) {
        W "  $($FG.Cyan)$label $($FG.Gray)[$default]: ${RESET}"
    } else {
        W "  $($FG.Cyan)${label}: ${RESET}"
    }
    $v = [Console]::ReadLine()
    if ([string]::IsNullOrWhiteSpace($v) -and $default) { return $default }
    return $v
}

# -------------------------------------------------------------------------------
#  RUN dedupe COMMAND
# -------------------------------------------------------------------------------
function Invoke-Dedupe($ArgList) {
    $pi = New-Object System.Diagnostics.ProcessStartInfo
    $pi.FileName  = $Script:DEDUPE
    $pi.Arguments = $ArgList -join " "
    $pi.RedirectStandardOutput = $true
    $pi.RedirectStandardError  = $true
    $pi.UseShellExecute = $false
    $pi.CreateNoWindow  = $true

    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $pi
    [void]$p.Start()

    $out = $p.StandardOutput.ReadToEnd()
    $err = $p.StandardError.ReadToEnd()
    $p.WaitForExit()

    return @{ ExitCode = $p.ExitCode; Stdout = $out; Stderr = $err }
}

# -------------------------------------------------------------------------------
#  FORMAT BYTES
# -------------------------------------------------------------------------------
function Fmt-Bytes([long]$b) {
    if     ($b -ge 1073741824) { "{0:F2} GB" -f ($b / 1073741824) }
    elseif ($b -ge 1048576)    { "{0:F2} MB" -f ($b / 1048576)    }
    elseif ($b -ge 1024)       { "{0:F2} KB" -f ($b / 1024)       }
    else                       { "$b B"                            }
}

# -------------------------------------------------------------------------------
#  ERROR HANDLER HELPER
# -------------------------------------------------------------------------------
function Show-Error($result) {
    WL ""
    WL "  $($FG.Red)${BOLD}[ERROR]${RESET}"
    $result.Stderr -split "`n" | ForEach-Object { WL "  $($FG.Red)$_${RESET}" }
    WL ""
    WL "  $($FG.Gray)Press any key to go back...${RESET}"
    [void][Console]::ReadKey($true)
}

# -------------------------------------------------------------------------------
#  CHECK dedupe.exe EXISTS
# -------------------------------------------------------------------------------
function Assert-Dedupe {
    if (-not (Test-Path $Script:DEDUPE)) {
        Clear-Screen; Show-Banner
        WL (Center "$($FG.Red)${BOLD} [ERROR] dedupe.exe not found! ${RESET}")
        WL ""
        WL "  Expected: $($FG.Yellow)$($Script:DEDUPE)${RESET}"
        WL ""
        WL "  $($FG.Gray)Build with: $($FG.Cyan)cargo build --release${RESET}"
        WL "  $($FG.Gray)Then copy target/release/dedupe.exe to bin/dedupe.exe${RESET}"
        WL ""
        WL "  $($FG.Gray)Press any key to exit...${RESET}"
        [void][Console]::ReadKey($true)
        exit 1
    }
}

# -------------------------------------------------------------------------------
#  [1] SCAN
# -------------------------------------------------------------------------------
function Run-Scan {
    Clear-Screen; Show-Banner
    WL (Center "$($FG.Cyan)${BOLD} [SCAN] Find Duplicate Files ${RESET}")
    WL ""

    WL "  $($FG.Gray)Step 1 of 2: Pick a folder to scan${RESET}"
    WL "  $($FG.Yellow)  --> Opening folder picker window...${RESET}"
    $folder = Pick-Folder "Select folder to scan for duplicates"
    if (-not $folder) {
        WL "  $($FG.Red)Cancelled.${RESET}"
        Start-Sleep -Milliseconds 900; return
    }
    WL "  $($FG.Green)[OK]${RESET} $folder"
    WL ""

    WL "  $($FG.Gray)Step 2 of 2: Options (press ENTER for defaults)${RESET}"
    $ts = Read-Prompt "Worker threads" ([System.Environment]::ProcessorCount.ToString())
    $t  = [int]$ts; if ($t -lt 1 -or $t -gt 64) { $t = 4 }
    $ms = Read-Prompt "Minimum file size (bytes)" "1"
    $m  = [long]$ms; if ($m -lt 0) { $m = 1 }

    WL ""
    WL "  $($FG.Gray)Running scan...${RESET}"
    HRule

    $res = Invoke-Dedupe @("scan", "`"$folder`"", "--threads", "$t", "--min-size", "$m", "--json")
    if ($res.ExitCode -ne 0) { Show-Error $res; return }

    $data    = $res.Stdout | ConvertFrom-Json
    $groups  = $data.duplicate_groups
    $reclaim = $data.human_reclaimable
    $rBytes  = $data.total_reclaimable_bytes

    $out = @()
    $out += "$($FG.Yellow)${BOLD} SCAN RESULTS ${RESET}"
    $out += "$($FG.Gray)Directory :${RESET} $folder"
    $out += "$($FG.Gray)Threads   :${RESET} $t  |  Min size: $m bytes"
    $out += ""

    if ($groups.Count -eq 0) {
        $out += "$($FG.Green)${BOLD}  [OK] No duplicates found! Your folder is clean.${RESET}"
        Show-Pager $out "Scan Results"
    } else {
        $out += "$($FG.Red)${BOLD}  [!!] $($groups.Count) duplicate group(s) found${RESET}"
        $out += "$($FG.Yellow)  Total reclaimable: $reclaim ($rBytes bytes)${RESET}"
        $out += ""

        $out += "$($FG.Cyan)${BOLD}  $("-"*110)${RESET}"
        $out += "$($FG.Cyan)${BOLD}  $("{0,-9} | {1,-12} | {2,-15} | {3}" -f 'GROUP', 'SIZE', 'RECLAIMABLE', 'FILES')${RESET}"
        $out += "$($FG.Cyan)${BOLD}  $("-"*110)${RESET}"

        $filesToDelete = 0
        for ($gi = 0; $gi -lt $groups.Count; $gi++) {
            $g = $groups[$gi]
            $c = $g.files.Count
            $gs = $g.file_size
            $rs = ($c - 1) * $gs
            $filesToDelete += ($c - 1)
            
            $rowStr = "{0,-9} | {1,-12} | {2,-15} | {3}" -f ("Group " + ($gi+1)), (Fmt-Bytes $gs), (Fmt-Bytes $rs), ""
            $out += "  $($FG.Yellow)$rowStr${RESET}"
            
            for ($fi = 0; $fi -lt $c; $fi++) {
                $f = $g.files[$fi]
                $action = if ($fi -eq 0) { "$($FG.Green)[KEEP]  ${RESET}" } else { "$($FG.Red)[DELETE]${RESET}" }
                $rowStr2 = "{0,-9} | {1,-12} | {2,-15} | {3} {4}" -f '', '', '', $action, $f
                $out += "  $rowStr2"
            }
            $out += "$($FG.Cyan)${DIM}  $("-"*110)${RESET}"
        }

        $pagerResult = Show-Pager $out "Scan Results" @("D = Recycle duplicates ($filesToDelete files  /  $reclaim)")

        if ($pagerResult -eq "D") {
            Clear-Screen
            Show-Banner
            WL (Center "$($FG.Yellow)${BOLD} [RECYCLE] Sending Duplicates to Recycle Bin ${RESET}")
            WL ""
            WL "  $($FG.Gray)This will KEEP the first file in each group.${RESET}"
            WL "  $($FG.Gray)All other copies will be moved to the Recycle Bin (recoverable).${RESET}"
            WL "  $($FG.Yellow)Files to recycle: $filesToDelete  |  Space: $reclaim${RESET}"
            WL ""
            $shell = New-Object -ComObject Shell.Application
            $recycledCount = 0
            $recycledBytes = 0
            $failedFiles   = @()
            foreach ($g in $groups) {
                for ($fi = 1; $fi -lt $g.files.Count; $fi++) {
                    $f = $g.files[$fi]
                    try {
                        $dir  = [System.IO.Path]::GetDirectoryName($f)
                        $name = [System.IO.Path]::GetFileName($f)
                        $folder = $shell.Namespace($dir)
                        $item   = $folder.ParseName($name)
                        if ($item) {
                            $item.InvokeVerb("delete")
                            $recycledCount++
                            $recycledBytes += $g.file_size
                            WL "  $($FG.Green)[RECYCLED]${RESET} $name"
                        } else {
                            $failedFiles += $f
                            WL "  $($FG.Red)[SKIP]${RESET} Not found: $f"
                        }
                    } catch {
                        $failedFiles += $f
                        WL "  $($FG.Red)[ERROR]${RESET} $f  --  $($_.Exception.Message)"
                    }
                }
            }
            WL ""
            WL "  $($FG.Green)${BOLD}[DONE] Recycled $recycledCount files. Freed $(Fmt-Bytes $recycledBytes).${RESET}"
            if ($failedFiles.Count -gt 0) {
                WL "  $($FG.Red)${BOLD}$($failedFiles.Count) file(s) could not be recycled.${RESET}"
            }
        }

        WL ""
        WL "  $($FG.Gray)Press any key to return to menu...${RESET}"
        [void][Console]::ReadKey($true)
    }
}

# -------------------------------------------------------------------------------
#  [2] REPORT
# -------------------------------------------------------------------------------
function Run-Report {
    Clear-Screen; Show-Banner
    WL (Center "$($FG.Cyan)${BOLD} [REPORT] Disk Usage Analysis ${RESET}")
    WL ""

    WL "  $($FG.Gray)Step 1 of 2: Pick a folder to analyse${RESET}"
    WL "  $($FG.Yellow)  --> Opening folder picker window...${RESET}"
    $folder = Pick-Folder "Select folder for disk usage report"
    if (-not $folder) {
        WL "  $($FG.Red)Cancelled.${RESET}"
        Start-Sleep -Milliseconds 900; return
    }
    WL "  $($FG.Green)[OK]${RESET} $folder"
    WL ""

    WL "  $($FG.Gray)Step 2 of 2: Options (press ENTER for defaults)${RESET}"
    $topS   = Read-Prompt "Top N largest files" "10"
    $top    = [int]$topS; if ($top -lt 1) { $top = 10 }
    $stalS  = Read-Prompt "Stale threshold (months)" "6"
    $stale  = [int]$stalS

    WL ""
    WL "  $($FG.Gray)Running report...${RESET}"
    HRule

    $res = Invoke-Dedupe @("report", "`"$folder`"", "--top", "$top", "--stale-months", "$stale", "--json")
    if ($res.ExitCode -ne 0) { Show-Error $res; return }

    $data = $res.Stdout | ConvertFrom-Json

    $out = @()
    $out += "$($FG.Yellow)${BOLD} DISK USAGE REPORT ${RESET}"
    $out += "$($FG.Gray)Directory  :${RESET} $folder"
    $out += "$($FG.Gray)Total files:${RESET} $($data.total_files)"
    $out += ""

    $out += "$($FG.Cyan)${BOLD}  --- TOP $top LARGEST FILES $('-'*40)${RESET}"
    $rank = 1
    foreach ($entry in $data.top_files) {
        $p  = $entry[0]
        $sz = $entry[1]
        $out += "  $($FG.Yellow)$(($rank++).ToString().PadLeft(3)).${RESET}  $((Fmt-Bytes $sz).PadLeft(10))  $p"
    }
    $out += ""

    $out += "$($FG.Cyan)${BOLD}  --- TOP DIRECTORIES BY SIZE $('-'*35)${RESET}"
    foreach ($d in $data.top_directories) {
        $out += "  $($FG.Magenta)$((Fmt-Bytes $d.total_bytes).PadLeft(10))${RESET}  [$($d.file_count) files]  $($d.path)"
    }
    $out += ""

    $out += "$($FG.Cyan)${BOLD}  --- EXTENSION BREAKDOWN $('-'*39)${RESET}"
    foreach ($ex in $data.extensions) {
        $barLen = [Math]::Min(20, [Math]::Max(1, [int]($ex.count * 20 / [Math]::Max(1, $data.total_files))))
        $bar = "#" * $barLen
        $out += "  $($FG.Blue).$($ex.ext.PadRight(8))${RESET} $($FG.Green)$bar${RESET}  $($ex.count) files  $(Fmt-Bytes $ex.total_bytes)"
    }
    $out += ""

    $out += "$($FG.Cyan)${BOLD}  --- HYGIENE AUDIT $('-'*44)${RESET}"
    $out += "  $($FG.Gray)Empty files         :${RESET} $($data.empty_files_count)"
    $out += "  $($FG.Gray)Empty directories   :${RESET} $($data.empty_dirs_count)"
    $out += "  $($FG.Yellow)Stale files (>$stale mo) :${RESET} $($data.stale_files_count)"

    Show-Pager $out "Disk Report"
}

# -------------------------------------------------------------------------------
#  [3] SNAPSHOT
# -------------------------------------------------------------------------------
function Run-Snapshot {
    Clear-Screen; Show-Banner
    WL (Center "$($FG.Cyan)${BOLD} [SNAPSHOT] Create Point-in-Time Index ${RESET}")
    WL ""

    WL "  $($FG.Gray)Step 1 of 2: Pick a folder to snapshot${RESET}"
    WL "  $($FG.Yellow)  --> Opening folder picker window...${RESET}"
    $folder = Pick-Folder "Select folder to snapshot"
    if (-not $folder) {
        WL "  $($FG.Red)Cancelled.${RESET}"
        Start-Sleep -Milliseconds 900; return
    }
    WL "  $($FG.Green)[OK]${RESET} $folder"
    WL ""

    $ts  = Get-Date -Format "yyyyMMdd_HHmmss"
    $def = "snapshot_$ts.json"
    WL "  $($FG.Gray)Step 2 of 2: Where to save the snapshot${RESET}"
    WL "  $($FG.Yellow)  --> Opening save dialog window...${RESET}"
    $savePath = Pick-SaveFile "Save snapshot as" $def
    if (-not $savePath) {
        WL "  $($FG.Red)Cancelled.${RESET}"
        Start-Sleep -Milliseconds 900; return
    }
    WL "  $($FG.Green)[OK]${RESET} Will save to: $savePath"
    WL ""
    WL "  $($FG.Gray)Creating snapshot...${RESET}"
    HRule

    $res = Invoke-Dedupe @("snapshot", "`"$folder`"", "-o", "`"$savePath`"")
    if ($res.ExitCode -ne 0) { Show-Error $res; return }

    WL ""
    WL "  $($FG.Green)${BOLD}[SUCCESS] Snapshot created!${RESET}"
    WL ""
    $res.Stdout -split "`n" | ForEach-Object { WL "  $($FG.White)$_${RESET}" }
    WL ""
    WL "  $($FG.Gray)Saved to: $($FG.Yellow)$savePath${RESET}"
    WL ""
    WL "  $($FG.Gray)Press any key to continue...${RESET}"
    [void][Console]::ReadKey($true)
}

# -------------------------------------------------------------------------------
#  [4] DIFF
# -------------------------------------------------------------------------------
function Run-Diff {
    Clear-Screen; Show-Banner
    WL (Center "$($FG.Cyan)${BOLD} [DIFF] Compare Two Snapshots ${RESET}")
    WL ""

    WL "  $($FG.Gray)Step 1 of 2: Pick the BASELINE (old) snapshot${RESET}"
    WL "  $($FG.Yellow)  --> Opening file picker window...${RESET}"
    $oldSnap = Pick-File "Select BASELINE (old) snapshot" $Script:SnapDir
    if (-not $oldSnap) {
        WL "  $($FG.Red)Cancelled.${RESET}"
        Start-Sleep -Milliseconds 900; return
    }
    WL "  $($FG.Green)[OK]${RESET} Old snapshot: $oldSnap"
    WL ""

    WL "  $($FG.Gray)Step 2 of 2: Pick the TARGET (new) snapshot${RESET}"
    WL "  $($FG.Yellow)  --> Opening file picker window...${RESET}"
    $newSnap = Pick-File "Select TARGET (new) snapshot" $Script:SnapDir
    if (-not $newSnap) {
        WL "  $($FG.Red)Cancelled.${RESET}"
        Start-Sleep -Milliseconds 900; return
    }
    WL "  $($FG.Green)[OK]${RESET} New snapshot: $newSnap"
    WL ""
    WL "  $($FG.Gray)Running diff...${RESET}"
    HRule

    $res = Invoke-Dedupe @("diff", "`"$oldSnap`"", "`"$newSnap`"", "--json")
    if ($res.ExitCode -ne 0) { Show-Error $res; return }

    $data = $res.Stdout | ConvertFrom-Json

    $out = @()
    $out += "$($FG.Yellow)${BOLD} SNAPSHOT DIFF REPORT ${RESET}"
    $out += "$($FG.Gray)Old :${RESET} $oldSnap"
    $out += "$($FG.Gray)New :${RESET} $newSnap"
    $out += "$($FG.Gray)Unchanged: ${RESET}$($data.unchanged_count) files"
    $out += ""

    $out += "$($FG.Green)${BOLD}  --- ADDED ($($data.added.Count)) $('-'*50)${RESET}"
    if ($data.added.Count -eq 0) { $out += "  $($FG.Gray)(none)${RESET}" }
    else { foreach ($f in $data.added) { $out += "  $($FG.Green)+${RESET} $f" } }
    $out += ""

    $out += "$($FG.Red)${BOLD}  --- REMOVED ($($data.removed.Count)) $('-'*47)${RESET}"
    if ($data.removed.Count -eq 0) { $out += "  $($FG.Gray)(none)${RESET}" }
    else { foreach ($f in $data.removed) { $out += "  $($FG.Red)-${RESET} $f" } }
    $out += ""

    $out += "$($FG.Yellow)${BOLD}  --- MODIFIED ($($data.modified.Count)) $('-'*46)${RESET}"
    if ($data.modified.Count -eq 0) { $out += "  $($FG.Gray)(none)${RESET}" }
    else { foreach ($f in $data.modified) { $out += "  $($FG.Yellow)*${RESET} $f" } }
    $out += ""

    $out += "$($FG.Blue)${BOLD}  --- MOVED / RENAMED ($($data.moved.Count)) $('-'*40)${RESET}"
    if ($data.moved.Count -eq 0) { $out += "  $($FG.Gray)(none)${RESET}" }
    else { foreach ($pair in $data.moved) { $out += "  $($FG.Blue)>>>${RESET} $($pair[0])  -->  $($pair[1])" } }

    Show-Pager $out "Snapshot Diff"
}

# -------------------------------------------------------------------------------
#  [5] ABOUT
# -------------------------------------------------------------------------------
function Show-About {
    Clear-Screen; Show-Banner
    WL (Center "$($FG.Yellow)${BOLD} [ ABOUT ] ${RESET}")
    WL ""
    WL "  $($FG.Cyan)${BOLD}dedupe${RESET}  v0.1.0  -- High-performance disk audit and deduplication"
    WL ""
    WL "  $($FG.Gray)Engine     :${RESET}  Rust  (blake3 parallel hashing)"
    WL "  $($FG.Gray)Binary     :${RESET}  $($Script:DEDUPE)"
    WL "  $($FG.Gray)Snapshots  :${RESET}  $($Script:SnapDir)"
    WL ""
    HRule
    WL "  $($FG.Cyan)${BOLD}Architecture${RESET}"
    WL "  $($FG.Gray)1.${RESET} Directory walk  -- recursive, skips symlinks"
    WL "  $($FG.Gray)2.${RESET} Size grouping   -- eliminates ~93%+ unique files before any hashing"
    WL "  $($FG.Gray)3.${RESET} Parallel blake3 -- 64 KB chunks, configurable worker threads"
    WL "  $($FG.Gray)4.${RESET} Dedup + reports + snapshot diff"
    WL ""
    HRule
    WL "  $($FG.Cyan)${BOLD}Commands${RESET}"
    WL "  $($FG.Yellow)scan${RESET}      Find duplicate files"
    WL "  $($FG.Yellow)report${RESET}    Disk usage: top files, dirs, extensions, hygiene"
    WL "  $($FG.Yellow)snapshot${RESET}  Create a cryptographic point-in-time directory index"
    WL "  $($FG.Yellow)diff${RESET}      Compare two snapshots: added/removed/modified/moved"
    WL ""
    HRule
    WL "  $($FG.Cyan)${BOLD}Benchmark highlights${RESET}"
    WL "  $($FG.Gray)*${RESET}  Size pre-filter saves 93.6% of hashing work  (8.95x speedup)"
    WL "  $($FG.Gray)*${RESET}  8-thread pool delivers ~2.70x speedup over single-threaded"
    WL ""
    WL "  $($FG.Gray)Press any key to return to menu...${RESET}"
    [void][Console]::ReadKey($true)
}

# -------------------------------------------------------------------------------
#  MAIN
# -------------------------------------------------------------------------------
function Main {
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
    [Console]::CursorVisible  = $false
    try {
        $null = $Host.UI.RawUI.set_BackgroundColor([System.ConsoleColor]::Black)
        $null = $Host.UI.RawUI.set_ForegroundColor([System.ConsoleColor]::Gray)
    } catch {}

    Assert-Dedupe

    $items = @(
        "Scan for Duplicate Files",
        "Disk Usage Report",
        "Create Snapshot",
        "Compare Snapshots  (Diff)",
        "About dedupe",
        "Quit"
    )
    $icons = @(
        "[SCAN]  ",
        "[REPORT]",
        "[SNAP]  ",
        "[DIFF]  ",
        "[INFO]  ",
        "[EXIT]  "
    )

    while ($true) {
        $choice = Navigate-Menu -Title "MAIN MENU" -Items $items -Icons $icons
        switch ($choice) {
            0   { Run-Scan }
            1   { Run-Report }
            2   { Run-Snapshot }
            3   { Run-Diff }
            4   { Show-About }
            5   { break }
            -1  { break }
        }
        if ($choice -eq 5 -or $choice -eq -1) { break }
    }

    [Console]::CursorVisible = $true
    Clear-Screen
    WL ""
    WL "  $($FG.Cyan)${BOLD}  Goodbye! Happy deduping.${RESET}"
    WL ""
}

Main
