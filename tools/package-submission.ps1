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
$MaxArchitectureBytes = 1024 * 1024
$RequiredShots = 9024
$RequiredGate = "fiat_shamir_shor_ecdlp_5bit_in_place_field_arithmetic_oracle_v1"
$RequiredBenchmark = "shor-ecdlp-5bit"
$RequiredScoreModel = "balanced-qubit-toffoli-depth-v1"
$RequiredArtifact = "ops.bin"
$RequiredArchitecturePath = "src/shor_oracle/architecture.mmd"
$RequiredArchitectureTarget = "Target oracle: aP + bQ using in-place F_31 field arithmetic"
$RequiredFieldArithmeticPath = "src/shor_oracle/field_arithmetic.rs"
$RequiredValidationChecks = @(
  "oracle correctness",
  "in-place F_31 field arithmetic composition",
  "input preservation",
  "phase cleanliness",
  "ancilla cleanup"
)
$FieldArithmeticBannedPatterns = @(
  @{ Pattern = "\bQubitId\b"; Message = "must not name raw QubitId values; use opaque FieldInput/FieldOutput handles" },
  @{ Pattern = "\bRegisterId\b"; Message = "must not observe or construct register IDs" },
  @{ Pattern = "\bOperationType\b"; Message = "must not construct primitive operations directly" },
  @{ Pattern = "\bOp\b"; Message = "must not construct or inspect primitive operations directly" },
  @{ Pattern = "crate\s*::\s*circuit"; Message = "must not import the raw circuit module" },
  @{ Pattern = "Signal\s*::\s*Qubit"; Message = "must not manufacture signals for arbitrary qubits" },
  @{ Pattern = "\bBuilder\b"; Message = "must not access the trusted oracle builder" },
  @{ Pattern = "\bunsafe\b"; Message = "unsafe code is not allowed in the editable field boundary" },
  @{ Pattern = "\btransmute\b"; Message = "must not inspect opaque field handles by transmutation" },
  @{ Pattern = "\bstatic\s+mut\b"; Message = "must not keep mutable global state across field-kernel calls" },
  @{ Pattern = "\b(thread_local|OnceLock|LazyLock|Mutex|RwLock|Atomic[A-Za-z0-9_]*)\b"; Message = "must not use global state to key behavior by call order" },
  @{ Pattern = "\b(include_bytes|include_str|std\s*::\s*fs|std\s*::\s*process|std\s*::\s*net|std\s*::\s*env)\b"; Message = "must not load external data or depend on process/environment state" }
)

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

function Assert-ArchitectureDiagram([string] $RepoRoot) {
  $diagramPath = Resolve-RepoPath $RepoRoot $RequiredArchitecturePath
  if (-not (Test-Path -LiteralPath $diagramPath -PathType Leaf)) {
    throw "$RequiredArchitecturePath is required"
  }
  $diagramBytes = (Get-Item -LiteralPath $diagramPath).Length
  if ($diagramBytes -le 0 -or $diagramBytes -gt $MaxArchitectureBytes) {
    throw "$RequiredArchitecturePath must be between 1 and $MaxArchitectureBytes bytes"
  }
  $text = [System.IO.File]::ReadAllText($diagramPath, $Utf8NoBom)
  $lines = $text -split "\r?\n" |
    ForEach-Object { ($_ -replace "%%.*$", "").Trim() } |
    Where-Object { $_.Length -gt 0 }
  if ($lines.Count -eq 0 -or $lines[0] -notmatch "^(flowchart|graph)\s+(TD|TB|BT|LR|RL)\b") {
    throw "$RequiredArchitecturePath must start with a Mermaid flowchart or graph declaration"
  }

  $idsByLabel = @{}
  foreach ($line in $lines) {
    foreach ($label in @($RequiredArchitectureTarget, "Algorithm", "Optimization")) {
      $pattern = "([A-Za-z][\w-]*)\s*(?:\[|\(|\{)\s*`"$([regex]::Escape($label))`"\s*(?:\]|\)|\})"
      foreach ($match in [regex]::Matches($line, $pattern)) {
        if (-not $idsByLabel.ContainsKey($label)) {
          $idsByLabel[$label] = @()
        }
        $idsByLabel[$label] += $match.Groups[1].Value
      }
    }
  }

  foreach ($label in @($RequiredArchitectureTarget, "Algorithm", "Optimization")) {
    if (-not $idsByLabel.ContainsKey($label) -or $idsByLabel[$label].Count -eq 0) {
      throw "$RequiredArchitecturePath must contain exact anchor label '$label'"
    }
  }

  $targetIds = $idsByLabel[$RequiredArchitectureTarget]
  $algorithmIds = $idsByLabel["Algorithm"]
  $optimizationIds = $idsByLabel["Optimization"]
  $hasAlgorithmEdge = $false
  $hasOptimizationEdge = $false
  foreach ($line in $lines) {
    foreach ($targetId in $targetIds) {
      foreach ($algorithmId in $algorithmIds) {
        $pattern = "^\s*$([regex]::Escape($targetId))(?:\[[^\]]+\]|\([^\)]+\)|\{[^\}]+\})?\s*(?:-->|---?>|==>)\s*$([regex]::Escape($algorithmId))\b"
        if ($line -match $pattern) {
          $hasAlgorithmEdge = $true
        }
      }
      foreach ($optimizationId in $optimizationIds) {
        $pattern = "^\s*$([regex]::Escape($targetId))(?:\[[^\]]+\]|\([^\)]+\)|\{[^\}]+\})?\s*(?:-->|---?>|==>)\s*$([regex]::Escape($optimizationId))\b"
        if ($line -match $pattern) {
          $hasOptimizationEdge = $true
        }
      }
    }
  }
  if (-not $hasAlgorithmEdge) {
    throw "$RequiredArchitecturePath must have Target branching to Algorithm"
  }
  if (-not $hasOptimizationEdge) {
    throw "$RequiredArchitecturePath must have Target branching to Optimization"
  }
}

function Get-FirstMatchingLine([string] $Text, [string] $Pattern) {
  $lines = $Text -split "\r?\n"
  for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -cmatch $Pattern) {
      return $i + 1
    }
  }
  return 1
}

function Assert-FieldArithmeticBoundary([string] $RepoRoot) {
  $fieldPath = Resolve-RepoPath $RepoRoot $RequiredFieldArithmeticPath
  if (-not (Test-Path -LiteralPath $fieldPath -PathType Leaf)) {
    throw "$RequiredFieldArithmeticPath is required"
  }
  $text = [System.IO.File]::ReadAllText($fieldPath, $Utf8NoBom)
  foreach ($entry in $FieldArithmeticBannedPatterns) {
    if ($text -cmatch $entry.Pattern) {
      $line = Get-FirstMatchingLine $text $entry.Pattern
      throw "$RequiredFieldArithmeticPath`:$line`: $($entry.Message)"
    }
  }
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
  Assert-ArchitectureDiagram $RepoRoot
  Assert-FieldArithmeticBoundary $RepoRoot
  $architecturePath = Resolve-RepoPath $RepoRoot $RequiredArchitecturePath
  $architectureBytes = (Get-Item -LiteralPath $architecturePath).Length
  $architectureSha256 = (Get-FileHash -LiteralPath $architecturePath -Algorithm SHA256).Hash.ToLowerInvariant()

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
  foreach ($requiredCheck in $RequiredValidationChecks) {
    if ($score.validation.checks -notcontains $requiredCheck) {
      throw "score.json validation.checks must include '$requiredCheck'"
    }
  }
  if ($score.score_model -ne $RequiredScoreModel) {
    throw "score.json score_model must be $RequiredScoreModel"
  }
  if ($score.artifact -ne $RequiredArtifact) {
    throw "score.json artifact must be $RequiredArtifact"
  }
  $artifactPath = Resolve-RepoPath $RepoRoot $score.artifact
  if (-not (Test-Path -LiteralPath $artifactPath)) {
    throw "score.json artifact is missing: $($score.artifact)"
  }
  $artifactBytes = (Get-Item -LiteralPath $artifactPath).Length
  if ($artifactBytes -le 0) {
    throw "score.json artifact must not be empty: $($score.artifact)"
  }
  Write-Host "Hashing artifact: $($score.artifact) ($artifactBytes bytes)"
  $artifactSha256 = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
  foreach ($metricName in @("toffoli", "ccx", "ccz", "toffoli_depth", "clifford", "qubits", "ops")) {
    if (-not $score.metrics.PSObject.Properties.Name.Contains($metricName)) {
      throw "score.json metrics.$metricName is missing"
    }
  }
  $expectedScore = [math]::Round([double] $score.metrics.qubits) * [math]::Sqrt(
    [math]::Round([double] $score.metrics.toffoli) * [math]::Round([double] $score.metrics.toffoli_depth)
  )
  $actualScore = [double] $score.score
  $scoreTolerance = 2.220446049250313e-16 * [math]::Max(
    1.0,
    [math]::Max([math]::Abs($actualScore), [math]::Abs($expectedScore))
  ) * 8
  if ([double]::IsNaN($actualScore) -or [double]::IsInfinity($actualScore) -or [math]::Abs($actualScore - $expectedScore) -gt $scoreTolerance) {
    throw "score.json score must equal metrics.qubits * sqrt(round(metrics.toffoli) * round(metrics.toffoli_depth)) ($expectedScore)"
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
    artifactBytes = $artifactBytes
    artifactSha256 = $artifactSha256
    architectureDiagram = [ordered]@{
      path = $RequiredArchitecturePath
      bytes = $architectureBytes
      sha256 = $architectureSha256
    }
    generatedAt = (Get-Date).ToUniversalTime().ToString("o")
  }
  [System.IO.File]::WriteAllText(
    $metadataPath,
    (($metadata | ConvertTo-Json -Depth 8) + "`n"),
    $Utf8NoBom
  )

  Write-Host "Packaged editable paths: $($normalizedPaths -join ', ')"
  Write-Host "Archive: $archivePath ($archiveBytes bytes)"
  Write-Host "Artifact: $($score.artifact) ($artifactBytes bytes, sha256 $artifactSha256)"
  Write-Host "Note: $noteOutPath ($noteBytes bytes)"
  Write-Host "Metadata: $metadataPath"
}
finally {
  Pop-Location
}
