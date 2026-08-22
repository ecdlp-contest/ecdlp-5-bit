param(
  [Parameter(Mandatory = $true)]
  [string] $NoteFile,

  [Parameter(Mandatory = $true)]
  [string] $Model,

  [string] $ManifestPath = "benchmark.json",
  [string] $OutDir = "dist",
  [string] $ClaimedScore = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$cliPath = Join-Path $repoRoot "ecdlp.js"
if (-not (Test-Path -LiteralPath $cliPath -PathType Leaf)) {
  throw "repo-local CLI not found: $cliPath"
}

$cliArgs = @(
  $cliPath,
  "package",
  "--note-file", $NoteFile,
  "--model", $Model,
  "--manifest", $ManifestPath,
  "--out", $OutDir
)
if ($ClaimedScore.Trim().Length -gt 0) {
  $cliArgs += @("--claimed-score", $ClaimedScore)
}

Push-Location $repoRoot
try {
  & node @cliArgs
  if ($LASTEXITCODE -ne 0) {
    throw "repo-local ecdlp package failed with exit code $LASTEXITCODE"
  }
}
finally {
  Pop-Location
}
