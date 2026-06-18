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

$MaxSubmissionNoteBytes = 10 * 1024
$MaxSubmissionArchiveBytes = 25 * 1024 * 1024
$RequiredShots = 9024
$RequiredGate = "fiat_shamir_shor_ecdlp_5bit_variable_q_oracle"
$RequiredBenchmark = "shor-ecdlp-5bit-v1"
$RequiredScoreModel = "primitive-ccx-ccz-v1"
$RequiredArtifact = "ops.bin"

function Resolve-RepoPath([string] $RepoRoot, [string] $RepoPath) {
  $relative = $RepoPath -replace "/", [System.IO.Path]::DirectorySeparatorChar
  return Join-Path $RepoRoot $relative
}

function Assert-RepoRelativePath([string] $RepoPath, [string] $FieldName) {
  $normalized = ($RepoPath -replace "\\", "/").Trim("/")
  if ($normalized.Length -eq 0) {
    throw "$FieldName must not be empty"
  }
  if ([System.IO.Path]::IsPathRooted($RepoPath)) {
    throw "$FieldName must be repo-relative: $RepoPath"
  }
  $parts = $normalized -split "/"
  if ($parts -contains "..") {
    throw "$FieldName must not contain '..': $RepoPath"
  }
  if ($normalized -eq "benchmark.json") {
    throw "$FieldName must not be benchmark.json"
  }
  return $normalized
}

$RepoRoot = Split-Path -Parent $PSScriptRoot
$Utf8NoBom = New-Object System.Text.UTF8Encoding($false)

Push-Location $RepoRoot
try {
  $manifestFile = Resolve-RepoPath $RepoRoot $ManifestPath
  if (-not (Test-Path -LiteralPath $manifestFile)) {
    throw "manifest not found: $ManifestPath"
  }
  $manifest = Get-Content -LiteralPath $manifestFile -Raw | ConvertFrom-Json
  if ($manifest.schemaVersion -ne 1) {
    throw "benchmark.json schemaVersion must be 1"
  }
  if ($manifest.name -ne $RequiredBenchmark) {
    throw "benchmark.json name must be $RequiredBenchmark"
  }
  if ($manifest.scoreModel -ne $RequiredScoreModel) {
    throw "benchmark.json scoreModel must be $RequiredScoreModel"
  }
  if ($manifest.scorePath -ne "score.json") {
    throw "benchmark.json scorePath must be score.json"
  }

  $editablePaths = @($manifest.editablePaths)
  if ($editablePaths.Count -eq 0) {
    throw "benchmark.json editablePaths must not be empty"
  }

  $normalizedPaths = @()
  $seen = @{}
  foreach ($path in $editablePaths) {
    $normalized = Assert-RepoRelativePath $path "editablePaths"
    if ($seen.ContainsKey($normalized)) {
      throw "editablePaths contains duplicate path: $normalized"
    }
    $seen[$normalized] = $true
    $fullPath = Resolve-RepoPath $RepoRoot $normalized
    if (-not (Test-Path -LiteralPath $fullPath)) {
      throw "editable path does not exist: $normalized"
    }
    $normalizedPaths += $normalized
  }

  $sorted = $normalizedPaths | Sort-Object
  for ($i = 0; $i -lt ($sorted.Count - 1); $i++) {
    if ($sorted[$i + 1].StartsWith($sorted[$i] + "/")) {
      throw "editablePaths must not overlap: $($sorted[$i]) and $($sorted[$i + 1])"
    }
  }

  $notePath = Resolve-RepoPath $RepoRoot $NoteFile
  if (-not (Test-Path -LiteralPath $notePath)) {
    throw "note file not found: $NoteFile"
  }
  $rawNote = Get-Content -LiteralPath $notePath -Raw
  if ($rawNote.Trim().Length -eq 0) {
    throw "submission note must not be empty"
  }
  if ($Model.Trim().Length -eq 0) {
    throw "submission model is required"
  }
  $submissionNote = "Model: $($Model.Trim())`n`n$rawNote"
  $noteBytes = $Utf8NoBom.GetByteCount($submissionNote)
  if ($noteBytes -gt $MaxSubmissionNoteBytes) {
    throw "submission note must be at most $MaxSubmissionNoteBytes bytes ($noteBytes bytes provided)"
  }

  $scorePath = Resolve-RepoPath $RepoRoot $manifest.scorePath
  if (-not (Test-Path -LiteralPath $scorePath)) {
    throw "score.json is missing; run the trusted evaluator before packaging"
  }
  $score = Get-Content -LiteralPath $scorePath -Raw | ConvertFrom-Json
  if ($score.status -ne "ranked") {
    throw "score.json status is not ranked"
  }
  if ($score.validation.shots -ne $RequiredShots -or $score.validation.gate -ne $RequiredGate) {
    throw "score.json does not show the required $RequiredShots-shot Fiat-Shamir gate"
  }
  if ($score.score_model -ne $RequiredScoreModel) {
    throw "score.json score_model must be $RequiredScoreModel"
  }
  if ($score.artifact -ne $RequiredArtifact) {
    throw "score.json artifact must be $RequiredArtifact"
  }
  foreach ($metricName in @("toffoli", "ccx", "ccz", "clifford", "qubits", "ops")) {
    if (-not $score.metrics.PSObject.Properties.Name.Contains($metricName)) {
      throw "score.json metrics.$metricName is missing"
    }
  }

  $outDirPath = Resolve-RepoPath $RepoRoot $OutDir
  New-Item -ItemType Directory -Force -Path $outDirPath | Out-Null
  $archivePath = Join-Path $outDirPath "submission.tar.gz"
  $noteOutPath = Join-Path $outDirPath "submission-note.md"
  $metadataPath = Join-Path $outDirPath "submission-metadata.json"

  if (Test-Path -LiteralPath $archivePath) {
    Remove-Item -LiteralPath $archivePath -Force
  }
  & tar -czf $archivePath -C $RepoRoot @normalizedPaths
  if ($LASTEXITCODE -ne 0) {
    throw "tar failed with exit code $LASTEXITCODE"
  }

  $archiveBytes = (Get-Item -LiteralPath $archivePath).Length
  if ($archiveBytes -gt $MaxSubmissionArchiveBytes) {
    throw "submission archive must be at most $MaxSubmissionArchiveBytes bytes ($archiveBytes bytes produced)"
  }

  [System.IO.File]::WriteAllText($noteOutPath, $submissionNote, $Utf8NoBom)

  $metadata = [ordered]@{
    schemaVersion = 1
    benchmark = $manifest.name
    editablePaths = $normalizedPaths
    archive = "submission.tar.gz"
    archiveBytes = $archiveBytes
    note = "submission-note.md"
    noteBytes = $noteBytes
    model = $Model.Trim()
    claimedScore = if ($ClaimedScore.Trim().Length -eq 0) { $null } else { [double] $ClaimedScore }
    localScore = $score.score
    scoreModel = $score.score_model
    metrics = $score.metrics
    validation = $score.validation
    artifact = $score.artifact
    generatedAt = (Get-Date).ToUniversalTime().ToString("o")
  }
  [System.IO.File]::WriteAllText(
    $metadataPath,
    (($metadata | ConvertTo-Json -Depth 8) + "`n"),
    $Utf8NoBom
  )

  Write-Host "Packaged editable paths: $($normalizedPaths -join ', ')"
  Write-Host "Archive: $archivePath ($archiveBytes bytes)"
  Write-Host "Note: $noteOutPath ($noteBytes bytes)"
  Write-Host "Metadata: $metadataPath"
}
finally {
  Pop-Location
}
