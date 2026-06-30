param(
  [Parameter(Mandatory = $true)]
  [string] $SubmitterName,

  [Parameter(Mandatory = $true)]
  [string] $SubmitterEmail,

  [Parameter(Mandatory = $true)]
  [string] $Model,

  [string] $Title = "Accept 5-bit Shor ECDLP submission",
  [string] $Note = "accepted submission"
)

$ErrorActionPreference = "Stop"

function Invoke-NativeChecked {
  param(
    [Parameter(Mandatory = $true)]
    [string] $FilePath,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Arguments
  )

  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$FilePath failed with exit code $LASTEXITCODE"
  }
}

Push-Location (Split-Path -Parent $PSScriptRoot)
try {
  Invoke-NativeChecked cargo fmt --check
  Invoke-NativeChecked powershell -NoProfile -ExecutionPolicy Bypass -File .\setup.ps1
  Invoke-NativeChecked powershell -NoProfile -ExecutionPolicy Bypass -File .\benchmark.ps1 -Note $Note
  Invoke-NativeChecked powershell -ExecutionPolicy Bypass -File tools\package-submission.ps1 -NoteFile src\shor_oracle\memory\README.md -Model $Model

  $score = Get-Content score.json | ConvertFrom-Json
  if ($score.status -ne "ranked") {
    throw "score.json status is not ranked"
  }
  if ($score.validation.shots -ne 9024 -or $score.validation.gate -ne "fiat_shamir_shor_ecdlp_5bit_in_place_field_arithmetic_oracle_v1") {
    throw "score.json does not show the required 9024-shot Fiat-Shamir oracle gate"
  }
  foreach ($requiredCheck in @("oracle correctness", "in-place F_31 field arithmetic composition", "input preservation", "phase cleanliness", "ancilla cleanup")) {
    if ($score.validation.checks -notcontains $requiredCheck) {
      throw "score.json validation.checks must include '$requiredCheck'"
    }
  }
  if ($score.score_model -ne "balanced-qubit-toffoli-depth-v1") {
    throw "score.json score_model is not balanced-qubit-toffoli-depth-v1"
  }

  $message = @"
$Title

Score: $($score.score)
Score model: $($score.score_model)
Toffoli: $($score.metrics.toffoli)
Toffoli depth: $($score.metrics.toffoli_depth)
CCX: $($score.metrics.ccx)
CCZ: $($score.metrics.ccz)
Clifford: $($score.metrics.clifford)
Qubits: $($score.metrics.qubits)
Ops: $($score.metrics.ops)
Artifact: $($score.artifact)
Validation: 9024 Fiat-Shamir 5-bit Shor ECDLP variable-base oracle and point-operation shots
Model: $Model

Co-authored-by: $SubmitterName <$SubmitterEmail>
"@

  Set-Content -Path ACCEPTANCE_COMMIT_MESSAGE.txt -Value $message -Encoding utf8
  Write-Host "Wrote ACCEPTANCE_COMMIT_MESSAGE.txt"
  Write-Host ""
  Write-Host $message
}
finally {
  Pop-Location
}
