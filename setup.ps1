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
    $env:CARGO_TARGET_DIR = Join-Path $PSScriptRoot "target"
  }
  Invoke-NativeChecked -FilePath rustup -Arguments @("show")
  Invoke-NativeChecked -FilePath cargo -Arguments @("fetch", "--locked")
  Invoke-NativeChecked -FilePath cargo -Arguments @("build", "--release", "--locked", "--bin", "build_circuit", "--bin", "eval_circuit")
}
finally {
  Pop-Location
}
