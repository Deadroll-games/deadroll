# One-shot local test run with the validator in WSL Ubuntu (native Windows agave cannot create its
# ledger: genesis unpack fails with "access denied"). Builds and tests stay on Windows.
#   1. start solana-test-validator in WSL on the lane's base port (fresh ledger, --reset)
#   2. wait until it is healthy
#   3. run the test command with ANCHOR_PROVIDER_URL / ANCHOR_WALLET set
#   4. always stop the validator, also when the tests fail or the script is interrupted
# Nobody keeps a validator running between test runs.
#
# Usage (repo root, PowerShell), after `anchor build --tools-version v1.54`:
#   tools/scripts/wsl-validator.ps1 -Port 8899
#   tools/scripts/wsl-validator.ps1 -Port 8899 -Wallet .anchor/test-wallet.json -Test "pnpm --filter tests test"
# Lane ports: L2 8899, L5 8909, L6 8919, L7 8929, L4 8939.
# Ports for base P: RPC P, websocket P+1, faucet P+2, gossip P+3,
# dynamic range 20000+(P-8899)*10 .. +59.
#
# Loaded at genesis (tests run without deploy, nothing needs funding):
#   - every program in Anchor.toml [programs.localnet] whose target/deploy/<name>.so exists;
#   - [[test.validator.account]] fixtures (address + filename) as --account;
#   - [[test.genesis]] programs (address + program) as --bpf-program.
# The ledger lives in the Linux home (~/ledger-<port>), never on /mnt/d.
param(
    [Parameter(Mandatory = $true)][int]$Port,
    [string]$Wallet = ".anchor/test-wallet.json",
    [string]$Test = "pnpm --filter tests test"
)
$ErrorActionPreference = "Stop"
$Root = (Resolve-Path "$PSScriptRoot/../..").Path
Set-Location $Root

function To-Wsl([string]$WinPath) {
    $full = [System.IO.Path]::GetFullPath($WinPath)
    $drive = $full.Substring(0, 1).ToLower()
    return "/mnt/$drive" + $full.Substring(2).Replace('\', '/')
}

function Stop-Validator {
    wsl -d Ubuntu -- bash -lc "pkill -f 'solana-test-validator.*--rpc-port $Port' || true" | Out-Null
}

$cliArgs = @(
    "--ledger", "~/ledger-$Port", "--reset", "--quiet",
    "--rpc-port", "$Port", "--faucet-port", "$($Port + 2)", "--gossip-port", "$($Port + 3)",
    "--dynamic-port-range", "$(20000 + ($Port - 8899) * 10)-$(20000 + ($Port - 8899) * 10 + 59)"
)
$section = ""
$pending = @{}
foreach ($line in Get-Content (Join-Path $Root "Anchor.toml")) {
    $t = $line.Trim()
    if ($t -match '^\[\[?([^\]]+)\]\]?$') {
        $section = $Matches[1]
        $pending = @{}
        continue
    }
    if ($section -eq "programs.localnet" -and $t -match '^(\w+)\s*=\s*"(\w+)"') {
        $so = Join-Path $Root "target/deploy/$($Matches[1]).so"
        if (Test-Path $so) { $cliArgs += @("--bpf-program", $Matches[2], (To-Wsl $so)) }
    }
    if ($section -in @("test.validator.account", "test.genesis") -and $t -match '^(\w+)\s*=\s*"([^"]+)"') {
        $pending[$Matches[1]] = $Matches[2]
        if ($section -eq "test.validator.account" -and $pending.address -and $pending.filename) {
            $cliArgs += @("--account", $pending.address, (To-Wsl (Join-Path $Root $pending.filename)))
            $pending = @{}
        }
        if ($section -eq "test.genesis" -and $pending.address -and $pending.program) {
            $cliArgs += @("--bpf-program", $pending.address, (To-Wsl (Join-Path $Root $pending.program)))
            $pending = @{}
        }
    }
}

$url = "http://127.0.0.1:$Port"
$exitCode = 1
try {
    Stop-Validator # a leftover from a killed run on this port
    $cmd = "exec solana-test-validator $($cliArgs -join ' ') > ~/ledger-$Port.log 2>&1"
    Start-Process -WindowStyle Hidden -FilePath "wsl.exe" -ArgumentList @("-d", "Ubuntu", "--", "bash", "-lc", "`"$cmd`"")

    $body = '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
    $healthy = $false
    for ($i = 0; $i -lt 60 -and -not $healthy; $i++) {
        Start-Sleep -Seconds 1
        try {
            $r = Invoke-RestMethod -Uri $url -Method Post -ContentType "application/json" -Body $body -TimeoutSec 2
            $healthy = $r.result -eq "ok"
        } catch { }
    }
    if (-not $healthy) {
        wsl -d Ubuntu -- bash -lc "tail -30 ~/ledger-$Port.log"
        throw "validator did not become healthy on $url"
    }
    # RPC answers before the websocket and the first slots are ready; tests need both.
    $ready = $false
    $slotBody = '{"jsonrpc":"2.0","id":1,"method":"getSlot"}'
    for ($i = 0; $i -lt 60 -and -not $ready; $i++) {
        try {
            $tcp = New-Object System.Net.Sockets.TcpClient
            $tcp.Connect("127.0.0.1", $Port + 1)
            $tcp.Close()
            $slot = (Invoke-RestMethod -Uri $url -Method Post -ContentType "application/json" -Body $slotBody -TimeoutSec 2).result
            $ready = $slot -ge 5
        } catch { }
        if (-not $ready) { Start-Sleep -Seconds 1 }
    }
    if (-not $ready) { throw "validator websocket on port $($Port + 1) not ready" }
    Write-Host "validator healthy at $url; running: $Test"

    # `anchor test` cannot run the script on Windows (its `bash` resolves to WSL's System32
    # bash.exe), so set the same environment it would and run the test command directly.
    $env:ANCHOR_PROVIDER_URL = $url
    $env:ANCHOR_WALLET = (Resolve-Path $Wallet).Path
    cmd /c $Test
    $exitCode = $LASTEXITCODE
}
finally {
    Stop-Validator
    Write-Host "validator on port $Port stopped"
}
exit $exitCode
