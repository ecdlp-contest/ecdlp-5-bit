param(
  [string] $Note = "baseline 5-bit Shor ECDLP oracle"
)

$ErrorActionPreference = "Stop"

function Invoke-NativeChecked {
  param(
    [Parameter(Mandatory = $true)]
    [string] $FilePath,

    [string[]] $Arguments = @()
  )

  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$FilePath failed with exit code $LASTEXITCODE"
  }
}

Push-Location $PSScriptRoot
try {
  if (-not $env:CARGO_TARGET_DIR) {
    $env:CARGO_TARGET_DIR = Join-Path $PSScriptRoot ".workspace\target"
  }

  Remove-Item -LiteralPath "ops.bin" -Force -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath "score.json" -Force -ErrorAction SilentlyContinue

  Invoke-NativeChecked -FilePath cargo -Arguments @("build", "--release", "--locked", "--bin", "build_circuit", "--bin", "eval_circuit")

  $exeSuffix = if ($IsWindows -or $env:OS -eq "Windows_NT") { ".exe" } else { "" }
  $buildCircuit = Join-Path $env:CARGO_TARGET_DIR "release\build_circuit$exeSuffix"
  $evalCircuit = Join-Path $env:CARGO_TARGET_DIR "release\eval_circuit$exeSuffix"

  Write-Warning "benchmark.ps1 is a local Windows developer runner and does not sandbox build_circuit. Official scoring should use ./benchmark.sh on Linux."
  Invoke-NativeChecked -FilePath $buildCircuit
  if (-not (Test-Path -LiteralPath "ops.bin") -or (Get-Item -LiteralPath "ops.bin").Length -le 0) {
    throw "build_circuit did not produce ops.bin"
  }

  Invoke-NativeChecked -FilePath $evalCircuit -Arguments @("--note", $Note)
  if (-not (Test-Path -LiteralPath "score.json")) {
    throw "eval_circuit did not produce score.json"
  }
}
finally {
  Pop-Location
}
